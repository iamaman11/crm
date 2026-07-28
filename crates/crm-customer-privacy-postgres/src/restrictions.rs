use crm_core_data::{
    CapabilityRequest, CustomerSubjectOperationClass, TransactionalCustomerSubjectPolicyPort,
    postgres_sqlx::{Postgres, Row, Transaction, postgres::PgRow},
};
use crm_customer_privacy::{MODULE_ID, RESTRICTION_RECORD_TYPE, RestrictionScope};
use crm_customer_privacy_persistence_adapter::processing_restriction_from_snapshot;
use crm_module_sdk::{
    DataClass, ErrorCategory, ModuleId, PayloadEncoding, PortFuture, RecordId, RecordRef,
    RecordSnapshot, RecordType, RetentionPolicyId, SchemaId, SchemaVersion, SdkError, TypedPayload,
};

const MAXIMUM_RESTRICTIONS_PER_SUBJECT: i64 = 1_000;

const SUBJECT_RESTRICTIONS_SQL: &str = r#"
    SELECT
      record_id,
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
      AND deleted_at IS NULL
      AND convert_from(payload_bytes, 'UTF8')::jsonb ->> 'canonical_party_id' = $4
    ORDER BY record_id ASC
    LIMIT $5
    FOR SHARE
"#;

/// Authoritative final Customer Privacy deny decision.
///
/// The caller supplies an already validated canonical Party identifier. This
/// implementation acquires the shared tenant + subject lock, then reloads and
/// strictly rehydrates current restriction aggregates from FORCE-RLS storage in
/// the same transaction. It never trusts an allow cache or module activation
/// state, so disabling Customer Privacy cannot turn an active directive into an
/// allow decision.
#[derive(Debug, Default, Clone, Copy)]
pub struct PostgresCustomerPrivacySubjectPolicy;

impl TransactionalCustomerSubjectPolicyPort for PostgresCustomerPrivacySubjectPolicy {
    fn lock_and_enforce<'a>(
        &'a self,
        transaction: &'a mut Transaction<'_, Postgres>,
        request: &'a CapabilityRequest,
        canonical_party_id: &'a RecordId,
        operation_class: CustomerSubjectOperationClass,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            lock_customer_subject(
                transaction,
                &request.context.execution.tenant_id,
                canonical_party_id,
            )
            .await?;

            let rows = crm_core_data::postgres_sqlx::query(SUBJECT_RESTRICTIONS_SQL)
                .bind(request.context.execution.tenant_id.as_str())
                .bind(MODULE_ID)
                .bind(RESTRICTION_RECORD_TYPE)
                .bind(canonical_party_id.as_str())
                .bind(MAXIMUM_RESTRICTIONS_PER_SUBJECT + 1)
                .fetch_all(&mut **transaction)
                .await
                .map_err(restriction_store_unavailable)?;

            if i64::try_from(rows.len()).unwrap_or(i64::MAX) > MAXIMUM_RESTRICTIONS_PER_SUBJECT {
                return Err(restriction_state_invalid(
                    "subject restriction inventory exceeds the supported bound",
                ));
            }

            for row in rows {
                let snapshot = decode_restriction_snapshot(row)?;
                let restriction = processing_restriction_from_snapshot(&snapshot)
                    .map_err(strict_rehydration_failed)?;
                if restriction.tenant_id() != &request.context.execution.tenant_id
                    || restriction.canonical_party_id() != canonical_party_id
                {
                    return Err(restriction_state_invalid(
                        "restriction identity differs from the locked tenant or canonical Party",
                    ));
                }
                if restriction.is_active_at(request.context.execution.request_started_at_unix_nanos)
                    && scope_blocks(restriction.scope(), operation_class)
                {
                    return Err(restriction_active());
                }
            }
            Ok(())
        })
    }
}

async fn lock_customer_subject(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: &crm_module_sdk::TenantId,
    canonical_party_id: &RecordId,
) -> Result<(), SdkError> {
    crm_core_data::postgres_sqlx::query("SELECT crm.lock_customer_subject($1, $2)")
        .bind(tenant_id.as_str())
        .bind(canonical_party_id.as_str())
        .execute(&mut **transaction)
        .await
        .map_err(subject_lock_unavailable)?;
    Ok(())
}

fn scope_blocks(scope: RestrictionScope, operation_class: CustomerSubjectOperationClass) -> bool {
    matches!(
        (scope, operation_class),
        (
            RestrictionScope::Processing,
            CustomerSubjectOperationClass::Processing
        ) | (
            RestrictionScope::Communication,
            CustomerSubjectOperationClass::Communication
        ) | (
            RestrictionScope::ProcessingAndCommunication,
            CustomerSubjectOperationClass::Processing
                | CustomerSubjectOperationClass::Communication
        )
    )
}

fn decode_restriction_snapshot(row: PgRow) -> Result<RecordSnapshot, SdkError> {
    let record_id: String = row.try_get("record_id").map_err(restriction_row_invalid)?;
    let version: i64 = row.try_get("version").map_err(restriction_row_invalid)?;
    let owner_module_id: String = row
        .try_get("owner_module_id")
        .map_err(restriction_row_invalid)?;
    let schema_id: String = row.try_get("schema_id").map_err(restriction_row_invalid)?;
    let schema_version: String = row
        .try_get("schema_version")
        .map_err(restriction_row_invalid)?;
    let descriptor_hash: Vec<u8> = row
        .try_get("descriptor_hash")
        .map_err(restriction_row_invalid)?;
    let data_class: String = row.try_get("data_class").map_err(restriction_row_invalid)?;
    let payload_encoding: String = row
        .try_get("payload_encoding")
        .map_err(restriction_row_invalid)?;
    let maximum_payload_size: i64 = row
        .try_get("maximum_payload_size")
        .map_err(restriction_row_invalid)?;
    let retention_policy_id: String = row
        .try_get("retention_policy_id")
        .map_err(restriction_row_invalid)?;
    let payload_bytes: Vec<u8> = row
        .try_get("payload_bytes")
        .map_err(restriction_row_invalid)?;

    if version <= 0 || data_class != "personal" || payload_encoding != "json" {
        return Err(restriction_row_invalid(
            "restriction row version, data class or encoding is invalid",
        ));
    }
    let descriptor_hash: [u8; 32] = descriptor_hash.try_into().map_err(|_| {
        restriction_row_invalid("restriction descriptor hash must contain exactly 32 bytes")
    })?;
    let maximum_size_bytes = u64::try_from(maximum_payload_size)
        .map_err(|_| restriction_row_invalid("restriction maximum payload size is negative"))?;

    Ok(RecordSnapshot {
        reference: RecordRef {
            record_type: RecordType::try_new(RESTRICTION_RECORD_TYPE)
                .map_err(configuration_error)?,
            record_id: RecordId::try_new(record_id).map_err(restriction_row_invalid)?,
        },
        version,
        payload: TypedPayload {
            owner: ModuleId::try_new(owner_module_id).map_err(restriction_row_invalid)?,
            schema_id: SchemaId::try_new(schema_id).map_err(restriction_row_invalid)?,
            schema_version: SchemaVersion::try_new(schema_version)
                .map_err(restriction_row_invalid)?,
            descriptor_hash,
            data_class: DataClass::Personal,
            encoding: PayloadEncoding::Json,
            maximum_size_bytes,
            retention_policy_id: RetentionPolicyId::try_new(retention_policy_id)
                .map_err(restriction_row_invalid)?,
            bytes: payload_bytes,
        },
    })
}

fn restriction_active() -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_RESTRICTION_ACTIVE",
        ErrorCategory::Conflict,
        false,
        "The requested customer-data operation is blocked by an active privacy restriction.",
    )
}

fn subject_lock_unavailable(error: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_SUBJECT_LOCK_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "The customer subject could not be locked for a final privacy decision.",
    )
    .with_internal_reference(error.to_string())
}

fn restriction_store_unavailable(error: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_RESTRICTION_DECISION_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "The current privacy restriction decision is unavailable.",
    )
    .with_internal_reference(error.to_string())
}

fn strict_rehydration_failed(error: SdkError) -> SdkError {
    restriction_state_invalid(format!(
        "processing restriction failed strict rehydration with {}",
        error.code
    ))
}

fn restriction_row_invalid(reference: impl std::fmt::Display) -> SdkError {
    restriction_state_invalid(format!("restriction row is invalid: {reference}"))
}

fn restriction_state_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_RESTRICTION_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "The current privacy restriction decision could not be verified safely.",
    )
    .with_internal_reference(reference.into())
}

fn configuration_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_RESTRICTION_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The privacy restriction decision is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restriction_matrix_is_deny_only() {
        assert!(scope_blocks(
            RestrictionScope::Processing,
            CustomerSubjectOperationClass::Processing
        ));
        assert!(!scope_blocks(
            RestrictionScope::Processing,
            CustomerSubjectOperationClass::Communication
        ));
        assert!(scope_blocks(
            RestrictionScope::Communication,
            CustomerSubjectOperationClass::Communication
        ));
        assert!(!scope_blocks(
            RestrictionScope::Communication,
            CustomerSubjectOperationClass::Processing
        ));
        assert!(scope_blocks(
            RestrictionScope::ProcessingAndCommunication,
            CustomerSubjectOperationClass::Processing
        ));
        assert!(scope_blocks(
            RestrictionScope::ProcessingAndCommunication,
            CustomerSubjectOperationClass::Communication
        ));
    }
}
