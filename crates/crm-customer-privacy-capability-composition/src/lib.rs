#![forbid(unsafe_code)]

//! PostgreSQL composition for promoted Customer Privacy case mutations.
//!
//! Creation validates optional predecessor lineage. Submission uses the shared
//! optimistic aggregate executor. Subject verification proves authoritative Party
//! and Identity Resolution lineage before taking the shared subject lock. Cancellation
//! derives the exact bound/rescope subject lock-set from RLS-protected case state,
//! locks it deterministically, then rechecks and holds the case under `FOR UPDATE`.

use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityRequest, TransactionalCapabilityExecutor};
use crm_core_data::{
    PostgresDataStore, PostgresTransactionalAggregateExecutor, TransactionalAggregateGuard,
};
use crm_customer_privacy::{MODULE_ID, PRIVACY_CASE_RECORD_TYPE};
use crm_customer_privacy_cancel_capability_adapter::{
    CANCEL_PRIVACY_CASE_CAPABILITY, CustomerPrivacyCaseCancelCapabilityPlanner,
    cancellation_subject_lock_ids, privacy_case_ref_from_request as cancellation_case_ref,
};
use crm_customer_privacy_capability_adapter::{
    CustomerPrivacyCaseCreateCapabilityPlanner, previous_case_id_from_request,
    previous_case_not_found, privacy_case_ref_from_id, validate_previous_case_snapshot,
};
use crm_customer_privacy_persistence_adapter::privacy_case_from_snapshot;
use crm_customer_privacy_subject_capability_adapter::{
    CustomerPrivacyCaseSubjectVerifyCapabilityPlanner, VERIFY_PRIVACY_CASE_SUBJECT_CAPABILITY,
    VERIFY_PRIVACY_CASE_SUBJECT_REQUEST_SCHEMA,
};
use crm_customer_privacy_submit_capability_adapter::CustomerPrivacyCaseSubmitCapabilityPlanner;
use crm_identity_resolution::PartyReference;
use crm_identity_resolution_topology_composition::prove_canonical_party_in_transaction;
use crm_module_sdk::{
    DataClass, ErrorCategory, ModuleId, PayloadEncoding, PortFuture, RecordId, RecordRef,
    RecordSnapshot, RetentionPolicyId, SchemaId, SchemaVersion, SdkError, TypedPayload,
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
            let row = select_case_row(transaction, &previous_case_id, CaseRowLock::Share)
                .await?
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

/// Locks the exact canonical subject set for cancellation without accepting a TOCTOU
/// transition from an unbound to a bound case. The first read is non-locking so subject
/// locks are always acquired before the case row. The second read takes the final
/// `FOR UPDATE` lock and must produce the same sorted lock-set; otherwise the caller
/// retries from fresh state. Unbound cases therefore serialize directly on the row
/// without taking a meaningless subject lock or performing a deadlock-prone lock upgrade.
#[derive(Debug, Default, Clone, Copy)]
pub struct PostgresCustomerPrivacyCancellationGuard;

impl TransactionalAggregateGuard for PostgresCustomerPrivacyCancellationGuard {
    fn check<'a>(
        &'a self,
        transaction: &'a mut Transaction<'_, Postgres>,
        request: &'a CapabilityRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            if request.context.execution.capability_id.as_str() != CANCEL_PRIVACY_CASE_CAPABILITY {
                return Err(cancellation_guard_unsupported());
            }
            let reference = cancellation_case_ref(request)?;
            let initial = load_cancellation_snapshot(
                transaction,
                request,
                &reference,
                CaseRowLock::Unlocked,
            )
            .await?;
            let initial_lock_ids = cancellation_subject_lock_ids(&initial)?;

            for subject_id in &initial_lock_ids {
                lock_customer_subject_in_transaction(
                    transaction,
                    &request.context.execution.tenant_id,
                    subject_id,
                )
                .await
                .map_err(map_cancellation_lock_error)?;
            }

            let locked = load_cancellation_snapshot(
                transaction,
                request,
                &reference,
                CaseRowLock::Update,
            )
            .await?;
            let locked_ids = cancellation_subject_lock_ids(&locked)?;
            if locked_ids != initial_lock_ids {
                return Err(cancellation_subject_changed());
            }
            Ok(())
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

pub fn postgres_case_subject_verify_executor(
    store: PostgresDataStore,
) -> Arc<dyn TransactionalCapabilityExecutor> {
    Arc::new(PostgresTransactionalAggregateExecutor::guarded(
        store,
        Arc::new(CustomerPrivacyCaseSubjectVerifyCapabilityPlanner),
        Arc::new(PostgresCustomerPrivacySubjectVerificationGuard),
    ))
}

pub fn postgres_case_cancel_executor(
    store: PostgresDataStore,
) -> Arc<dyn TransactionalCapabilityExecutor> {
    Arc::new(PostgresTransactionalAggregateExecutor::guarded(
        store,
        Arc::new(CustomerPrivacyCaseCancelCapabilityPlanner),
        Arc::new(PostgresCustomerPrivacyCancellationGuard),
    ))
}

async fn load_cancellation_snapshot(
    transaction: &mut Transaction<'_, Postgres>,
    request: &CapabilityRequest,
    reference: &RecordRef,
    lock: CaseRowLock,
) -> Result<RecordSnapshot, SdkError> {
    let row = select_case_row(transaction, &reference.record_id, lock)
        .await?
        .ok_or_else(cancellation_case_not_found)?;
    let snapshot = decode_snapshot(reference.record_id.clone(), row)?;
    if snapshot.reference != *reference {
        return Err(cancellation_case_not_found());
    }
    let privacy_case = privacy_case_from_snapshot(&snapshot).map_err(cancellation_state_invalid)?;
    if privacy_case.case_id() != &reference.record_id
        || privacy_case.tenant_id() != &request.context.execution.tenant_id
    {
        return Err(cancellation_case_not_found());
    }
    Ok(snapshot)
}

#[derive(Debug, Clone, Copy)]
enum CaseRowLock {
    Unlocked,
    Share,
    Update,
}

async fn select_case_row(
    transaction: &mut Transaction<'_, Postgres>,
    case_id: &RecordId,
    lock: CaseRowLock,
) -> Result<Option<sqlx::postgres::PgRow>, SdkError> {
    let sql = match lock {
        CaseRowLock::Share => r#"
        SELECT version, owner_module_id, schema_id, schema_version, descriptor_hash,
               data_class, payload_encoding, maximum_payload_size, retention_policy_id,
               payload_bytes
        FROM crm.records
        WHERE tenant_id = current_setting('app.tenant_id', true)
          AND owner_module_id = $1
          AND record_type = $2
          AND record_id = $3
          AND deleted_at IS NULL
        FOR SHARE
        "#,
        CaseRowLock::Update => r#"
        SELECT version, owner_module_id, schema_id, schema_version, descriptor_hash,
               data_class, payload_encoding, maximum_payload_size, retention_policy_id,
               payload_bytes
        FROM crm.records
        WHERE tenant_id = current_setting('app.tenant_id', true)
          AND owner_module_id = $1
          AND record_type = $2
          AND record_id = $3
          AND deleted_at IS NULL
        FOR UPDATE
        "#,
        CaseRowLock::Unlocked => r#"
        SELECT version, owner_module_id, schema_id, schema_version, descriptor_hash,
               data_class, payload_encoding, maximum_payload_size, retention_policy_id,
               payload_bytes
        FROM crm.records
        WHERE tenant_id = current_setting('app.tenant_id', true)
          AND owner_module_id = $1
          AND record_type = $2
          AND record_id = $3
          AND deleted_at IS NULL
        "#,
    };
    sqlx::query(sql)
        .bind(MODULE_ID)
        .bind(PRIVACY_CASE_RECORD_TYPE)
        .bind(case_id.as_str())
        .fetch_optional(&mut **transaction)
        .await
        .map_err(reference_store_unavailable)
}

fn required_party_reference(
    value: Option<&customer_wire::PartyRef>,
    field: &'static str,
) -> Result<PartyReference, SdkError> {
    let value =
        value.ok_or_else(|| SdkError::invalid_argument(field, "Party reference is required."))?;
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

fn map_cancellation_lock_error(error: SdkError) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_CANCELLATION_SUBJECT_LOCK_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "The privacy case subject could not be locked for cancellation.",
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

fn cancellation_guard_unsupported() -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_CANCELLATION_GUARD_UNSUPPORTED",
        ErrorCategory::Internal,
        false,
        "The Customer Privacy cancellation guard is not configured for this capability.",
    )
}

fn cancellation_subject_changed() -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_CANCELLATION_SUBJECT_CHANGED",
        ErrorCategory::Conflict,
        true,
        "The privacy case subject changed before cancellation could be committed.",
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
    case_id: RecordId,
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
            "privacy case payload class or encoding differs from its contract",
        ));
    }
    let descriptor_hash: [u8; 32] = descriptor_hash.try_into().map_err(|_| {
        reference_state_invalid("privacy case descriptor hash must contain exactly 32 bytes")
    })?;
    let maximum_size_bytes = u64::try_from(maximum_payload_size)
        .map_err(|_| reference_state_invalid("privacy case maximum payload size is negative"))?;

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

fn cancellation_case_not_found() -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_CASE_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The privacy case was not found.",
    )
}

fn cancellation_state_invalid(error: SdkError) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_CASE_INVALID",
        ErrorCategory::Internal,
        false,
        "The privacy case could not be loaded safely.",
    )
    .with_internal_reference(error.code)
}

fn reference_store_unavailable(reference: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_CASE_STORE_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "The privacy case could not be verified atomically.",
    )
    .with_internal_reference(reference.to_string())
}

fn reference_state_invalid(reference: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_CASE_INVALID",
        ErrorCategory::Internal,
        false,
        "The privacy case could not be loaded safely.",
    )
    .with_internal_reference(reference.to_string())
}
