#![forbid(unsafe_code)]

//! PostgreSQL composition for the promoted and candidate Customer Privacy case mutations.
//!
//! Case creation adds the optional predecessor `FOR SHARE` reference guard. Case
//! submission is a single-aggregate optimistic update and therefore uses the shared
//! transactional aggregate executor directly. Subject verification remains non-runtime
//! while this composition proves authoritative Party existence, canonical merge lineage,
//! exact Identity Resolution topology generation and the shared tenant + canonical Party
//! subject lock inside the same business transaction as the case update and evidence.

use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityRequest, TransactionalCapabilityExecutor};
use crm_core_data::{
    PostgresDataStore, PostgresTransactionalAggregateExecutor, TransactionalAggregateGuard,
};
use crm_customer_privacy::{MODULE_ID, PRIVACY_CASE_RECORD_TYPE};
use crm_customer_privacy_capability_adapter::{
    CustomerPrivacyCaseCreateCapabilityPlanner, previous_case_id_from_request,
    previous_case_not_found, privacy_case_ref_from_id, validate_previous_case_snapshot,
};
use crm_customer_privacy_subject_capability_adapter::{
    CustomerPrivacyCaseSubjectVerifyCapabilityPlanner, VERIFY_PRIVACY_CASE_SUBJECT_CAPABILITY,
    VERIFY_PRIVACY_CASE_SUBJECT_REQUEST_SCHEMA,
};
use crm_customer_privacy_submit_capability_adapter::CustomerPrivacyCaseSubmitCapabilityPlanner;
use crm_identity_resolution::PartyReference;
use crm_identity_resolution_topology_composition::prove_canonical_party_in_transaction;
use crm_module_sdk::{
    DataClass, ErrorCategory, ModuleId, PayloadEncoding, PortFuture, RecordId, RecordSnapshot,
    RetentionPolicyId, SchemaId, SchemaVersion, SdkError, TypedPayload,
};
use crm_party_reference_composition::lock_customer_subject_in_transaction;
use crm_proto_contracts::crm::{customer::v1 as customer_wire, customer_privacy::v1 as wire};
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

/// Final transaction-scoped guard for `customer_privacy.case.subject.verify@1.0.0`.
/// It consumes owner-side Party and Identity Resolution proof APIs and never trusts the
/// caller-provided canonical Party or generation without exact authoritative validation.
#[derive(Debug, Default, Clone, Copy)]
pub struct PostgresCustomerPrivacySubjectVerificationGuard;

impl TransactionalAggregateGuard for PostgresCustomerPrivacySubjectVerificationGuard {
    fn check<'a>(
        &'a self,
        transaction: &'a mut Transaction<'_, Postgres>,
        request: &'a CapabilityRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            if request.context.execution.capability_id.as_str()
                != VERIFY_PRIVACY_CASE_SUBJECT_CAPABILITY
            {
                return Err(subject_guard_unsupported());
            }
            let command: wire::VerifyPrivacyCaseSubjectRequest =
                support::decode_request_with_data_class(
                    request,
                    MODULE_ID,
                    VERIFY_PRIVACY_CASE_SUBJECT_REQUEST_SCHEMA,
                    DataClass::Confidential,
                )?;
            let submitted = required_party_reference(
                command.submitted_party_ref.as_ref(),
                "customer_privacy.case.subject.submitted_party_ref",
            )?;
            let canonical = required_party_reference(
                command.canonical_party_ref.as_ref(),
                "customer_privacy.case.subject.canonical_party_ref",
            )?;

            let proof = prove_canonical_party_in_transaction(
                transaction,
                &request.context.execution.tenant_id,
                &submitted,
                &canonical,
                command.identity_resolution_generation,
            )
            .await
            .map_err(map_subject_proof_error)?;

            if proof.requested_party != submitted
                || proof.canonical_party != canonical
                || proof.generation != command.identity_resolution_generation
            {
                return Err(subject_proof_invalid(
                    "owner proof identity differs from the requested subject binding",
                ));
            }

            let canonical_party_id = RecordId::try_new(canonical.as_str()).map_err(|error| {
                SdkError::invalid_argument(
                    "customer_privacy.case.subject.canonical_party_ref",
                    format!("canonical Party reference is invalid: {error}"),
                )
            })?;
            lock_customer_subject_in_transaction(
                transaction,
                &request.context.execution.tenant_id,
                &canonical_party_id,
            )
            .await
            .map_err(map_subject_lock_error)
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

/// Candidate executor only. Application production registration remains unchanged until
/// fresh PostgreSQL, real ingress and full permanent-CI proof are accepted.
pub fn postgres_case_subject_verify_executor(
    store: PostgresDataStore,
) -> Arc<dyn TransactionalCapabilityExecutor> {
    Arc::new(PostgresTransactionalAggregateExecutor::guarded(
        store,
        Arc::new(CustomerPrivacyCaseSubjectVerifyCapabilityPlanner),
        Arc::new(PostgresCustomerPrivacySubjectVerificationGuard),
    ))
}

fn required_party_reference(
    value: Option<&customer_wire::PartyRef>,
    field: &'static str,
) -> Result<PartyReference, SdkError> {
    let value = value
        .ok_or_else(|| SdkError::invalid_argument(field, "Party reference is required."))?;
    PartyReference::try_new(value.party_id.clone()).map_err(|error| {
        SdkError::invalid_argument(field, format!("Party reference is invalid: {error}"))
    })
}

fn map_subject_proof_error(error: SdkError) -> SdkError {
    let (code, category, retryable, safe_message) = match error.code.as_str() {
        "PARTY_REFERENCE_UNAVAILABLE" => (
            "CUSTOMER_PRIVACY_SUBJECT_REFERENCE_UNAVAILABLE",
            ErrorCategory::NotFound,
            false,
            "One or more subject Party references are unavailable.",
        ),
        "IDENTITY_RESOLUTION_TOPOLOGY_GENERATION_STALE" => (
            "CUSTOMER_PRIVACY_SUBJECT_GENERATION_STALE",
            ErrorCategory::Conflict,
            true,
            "The Identity Resolution state changed before subject verification was committed.",
        ),
        "IDENTITY_RESOLUTION_CANONICAL_PARTY_MISMATCH" => (
            "CUSTOMER_PRIVACY_SUBJECT_CANONICAL_REFERENCE_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "The submitted Party does not resolve to the specified canonical Party.",
        ),
        "IDENTITY_RESOLUTION_CANONICAL_REDIRECT_INVALID" => (
            "CUSTOMER_PRIVACY_SUBJECT_TOPOLOGY_INVALID",
            ErrorCategory::Internal,
            false,
            "The subject Party topology could not be verified safely.",
        ),
        _ => (
            "CUSTOMER_PRIVACY_SUBJECT_PROOF_UNAVAILABLE",
            ErrorCategory::Unavailable,
            true,
            "The subject Party proof could not be verified atomically.",
        ),
    };
    SdkError::new(code, category, retryable, safe_message)
        .with_internal_reference(format!("owner subject proof failed with {}", error.code))
}

fn map_subject_lock_error(error: SdkError) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_SUBJECT_LOCK_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "The subject could not be locked safely.",
    )
    .with_internal_reference(format!("shared subject lock failed with {}", error.code))
}

fn subject_guard_unsupported() -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_SUBJECT_GUARD_UNSUPPORTED",
        ErrorCategory::Internal,
        false,
        "The Customer Privacy subject guard is not configured for this capability.",
    )
}

fn subject_proof_invalid(reference: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_SUBJECT_PROOF_INVALID",
        ErrorCategory::Internal,
        false,
        "The subject Party proof is invalid.",
    )
    .with_internal_reference(reference.to_string())
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
