#![forbid(unsafe_code)]

//! PostgreSQL composition for the promoted Customer Privacy case mutations.
//!
//! Case creation adds the optional predecessor `FOR SHARE` reference guard.
//! Case submission is a single-aggregate optimistic update and therefore uses
//! the shared transactional aggregate executor directly, with no module-owned
//! SQL mutation path.

use crm_capability_runtime::{CapabilityRequest, TransactionalCapabilityExecutor};
use crm_core_data::{
    PostgresDataStore, PostgresTransactionalAggregateExecutor, TransactionalAggregateGuard,
};
use crm_customer_privacy::{MODULE_ID, PRIVACY_CASE_RECORD_TYPE};
use crm_customer_privacy_capability_adapter::{
    CustomerPrivacyCaseCreateCapabilityPlanner, previous_case_id_from_request,
    previous_case_not_found, privacy_case_ref_from_id, validate_previous_case_snapshot,
};
use crm_customer_privacy_submit_capability_adapter::CustomerPrivacyCaseSubmitCapabilityPlanner;
use crm_module_sdk::{
    DataClass, ErrorCategory, ModuleId, PayloadEncoding, PortFuture, RecordSnapshot,
    RetentionPolicyId, SchemaId, SchemaVersion, SdkError, TypedPayload,
};
use sqlx::{Postgres, Row, Transaction};
use std::sync::Arc;

#[derive(Debug, Default, Clone, Copy)]
pub struct PostgresCustomerPrivacyPreviousCaseGuard;

impl TransactionalAggregateGuard for PostgresCustomerPrivacyPreviousCaseGuard {
    fn check<'a>(
        &'a self,
        transaction: &'a mut Transaction<'_, Postgres>,
        request: &'a CapabilityRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            let Some(previous_case_id) = previous_case_id_from_request(request)? else {
                return Ok(());
            };
            let row = sqlx::query(
                r#"
                SELECT
                  version,
                  owner_module_id,
                  schema_id,
                  schema_version,
                  descriptor_hash,
                  data_class,
                  payload_encoding,
                  maximum_payload_size,
                  retention_policy_id,
                  payload_bytes
                FROM crm.records
                WHERE tenant_id = $1
                  AND owner_module_id = $2
                  AND record_type = $3
                  AND record_id = $4
                  AND deleted_at IS NULL
                FOR SHARE
                "#,
            )
            .bind(request.context.execution.tenant_id.as_str())
            .bind(MODULE_ID)
            .bind(PRIVACY_CASE_RECORD_TYPE)
            .bind(previous_case_id.as_str())
            .fetch_optional(&mut **transaction)
            .await
            .map_err(reference_store_unavailable)?
            .ok_or_else(previous_case_not_found)?;

            let snapshot = decode_snapshot(previous_case_id, row)?;
            validate_previous_case_snapshot(request, &snapshot.reference.record_id, &snapshot)
        })
    }
}

pub fn postgres_case_create_executor(
    store: PostgresDataStore,
) -> Arc<dyn TransactionalCapabilityExecutor> {
    Arc::new(PostgresTransactionalAggregateExecutor::guarded(
        store,
        Arc::new(CustomerPrivacyCaseCreateCapabilityPlanner),
        Arc::new(PostgresCustomerPrivacyPreviousCaseGuard),
    ))
}

pub fn postgres_case_submit_executor(
    store: PostgresDataStore,
) -> Arc<dyn TransactionalCapabilityExecutor> {
    Arc::new(PostgresTransactionalAggregateExecutor::new(
        store,
        Arc::new(CustomerPrivacyCaseSubmitCapabilityPlanner),
    ))
}

fn decode_snapshot(
    case_id: crm_module_sdk::RecordId,
    row: sqlx::postgres::PgRow,
) -> Result<RecordSnapshot, SdkError> {
    let version: i64 = row.try_get("version").map_err(reference_state_invalid)?;
    let owner_module_id: String = row
        .try_get("owner_module_id")
        .map_err(reference_state_invalid)?;
    let schema_id: String = row.try_get("schema_id").map_err(reference_state_invalid)?;
    let schema_version: String = row
        .try_get("schema_version")
        .map_err(reference_state_invalid)?;
    let descriptor_hash: Vec<u8> = row
        .try_get("descriptor_hash")
        .map_err(reference_state_invalid)?;
    let data_class: String = row.try_get("data_class").map_err(reference_state_invalid)?;
    let payload_encoding: String = row
        .try_get("payload_encoding")
        .map_err(reference_state_invalid)?;
    let maximum_payload_size: i64 = row
        .try_get("maximum_payload_size")
        .map_err(reference_state_invalid)?;
    let retention_policy_id: String = row
        .try_get("retention_policy_id")
        .map_err(reference_state_invalid)?;
    let payload_bytes: Vec<u8> = row
        .try_get("payload_bytes")
        .map_err(reference_state_invalid)?;

    if data_class != "confidential" || payload_encoding != "json" {
        return Err(reference_state_invalid(
            "previous case payload class or encoding differs from its contract",
        ));
    }
    let descriptor_hash: [u8; 32] = descriptor_hash.try_into().map_err(|_| {
        reference_state_invalid("previous case descriptor hash must contain exactly 32 bytes")
    })?;
    let maximum_size_bytes = u64::try_from(maximum_payload_size)
        .map_err(|_| reference_state_invalid("previous case maximum payload size is negative"))?;

    Ok(RecordSnapshot {
        reference: privacy_case_ref_from_id(&case_id)?,
        version,
        payload: TypedPayload {
            owner: ModuleId::try_new(owner_module_id).map_err(reference_state_invalid)?,
            schema_id: SchemaId::try_new(schema_id).map_err(reference_state_invalid)?,
            schema_version: SchemaVersion::try_new(schema_version)
                .map_err(reference_state_invalid)?,
            descriptor_hash,
            data_class: DataClass::Confidential,
            encoding: PayloadEncoding::Json,
            maximum_size_bytes,
            retention_policy_id: RetentionPolicyId::try_new(retention_policy_id)
                .map_err(reference_state_invalid)?,
            bytes: payload_bytes,
        },
    })
}

fn reference_store_unavailable(reference: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_PREVIOUS_CASE_STORE_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "The previous privacy case could not be verified atomically.",
    )
    .with_internal_reference(reference.to_string())
}

fn reference_state_invalid(reference: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_PREVIOUS_CASE_INVALID",
        ErrorCategory::Internal,
        false,
        "The previous privacy case could not be loaded safely.",
    )
    .with_internal_reference(reference.to_string())
}
