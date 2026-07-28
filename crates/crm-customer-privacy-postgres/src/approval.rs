use crm_capability_runtime::{CapabilityRequest, TransactionalCapabilityExecutor};
use crm_core_data::{
    PostgresDataStore, PostgresTransactionalAggregateExecutor, TransactionalAggregateGuard,
    postgres_sqlx::{self, Postgres, Row, Transaction},
};
use crm_customer_privacy::{
    ACTION_PLAN_RECORD_TYPE, ACTION_PLAN_STATE_MAXIMUM_BYTES,
    ACTION_PLAN_STATE_RETENTION_POLICY_ID, ACTION_PLAN_STATE_SCHEMA_ID,
    ACTION_PLAN_STATE_SCHEMA_VERSION, DISCOVERY_SCOPE_SNAPSHOT_STATE_MAXIMUM_BYTES,
    DISCOVERY_SCOPE_SNAPSHOT_STATE_RETENTION_POLICY_ID, DISCOVERY_SCOPE_SNAPSHOT_STATE_SCHEMA_ID,
    DISCOVERY_SCOPE_SNAPSHOT_STATE_SCHEMA_VERSION, DiscoveryScopeSnapshot, MODULE_ID,
    PRIVACY_CASE_RECORD_TYPE, PrivacyActionPlan, PrivacyCase, PrivacyCaseStatus,
    SCOPE_SNAPSHOT_RECORD_TYPE, action_plan_state_descriptor_hash, decode_action_plan_state,
    decode_discovery_scope_snapshot_state, discovery_scope_snapshot_state_descriptor_hash,
};
use crm_customer_privacy_application::{
    APPROVE_PRIVACY_CASE_CAPABILITY, CustomerPrivacyCaseApprovalCapabilityPlanner,
    expected_version_from_request, privacy_case_ref_from_request,
};
use crm_customer_privacy_persistence_adapter::privacy_case_from_snapshot;
use crm_module_sdk::{
    DataClass, ErrorCategory, ModuleId, PayloadEncoding, PortFuture, RecordId, RecordRef,
    RecordSnapshot, RecordType, RetentionPolicyId, SchemaId, SchemaVersion, SdkError, TypedPayload,
};
use std::sync::Arc;

#[derive(Debug, Default, Clone, Copy)]
pub struct PostgresCustomerPrivacyApprovalGuard;

impl TransactionalAggregateGuard for PostgresCustomerPrivacyApprovalGuard {
    fn check<'a>(
        &'a self,
        transaction: &'a mut Transaction<'_, Postgres>,
        request: &'a CapabilityRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            if request.context.execution.capability_id.as_str()
                != APPROVE_PRIVACY_CASE_CAPABILITY
            {
                return Err(approval_guard_unsupported());
            }
            let reference = privacy_case_ref_from_request(request)?;
            let expected_version = expected_version_from_request(request)?;
            let privacy_case = load_case_in_transaction(
                transaction,
                &reference.record_id,
                RowLock::Update,
            )
            .await?
            .ok_or_else(case_not_found)?;

            if privacy_case.case_id() != &reference.record_id
                || privacy_case.tenant_id() != &request.context.execution.tenant_id
            {
                return Err(case_not_found());
            }
            if privacy_case.version() != expected_version {
                return Err(version_conflict(expected_version, privacy_case.version()));
            }
            if privacy_case.status() != PrivacyCaseStatus::AwaitingApproval
                || privacy_case.approval().is_some()
            {
                return Err(approval_conflict(
                    "only an awaiting-approval case without prior approval evidence is eligible",
                ));
            }

            let binding = privacy_case.subject_binding().ok_or_else(|| {
                approval_evidence_invalid("awaiting-approval case has no verified subject binding")
            })?;
            let snapshot_id = privacy_case.scope_snapshot_id().ok_or_else(|| {
                approval_evidence_invalid("awaiting-approval case has no scope snapshot reference")
            })?;
            let plan_id = privacy_case.action_plan_id().ok_or_else(|| {
                approval_evidence_invalid("awaiting-approval case has no action plan reference")
            })?;

            let snapshot = load_snapshot_in_transaction(transaction, snapshot_id, RowLock::Share)
                .await?
                .ok_or_else(|| approval_evidence_invalid("referenced scope snapshot is missing"))?;
            let plan = load_plan_in_transaction(transaction, plan_id, RowLock::Share)
                .await?
                .ok_or_else(|| approval_evidence_invalid("referenced action plan is missing"))?;

            validate_exact_lineage(&privacy_case, &snapshot, &plan, binding)?;
            verify_planning_link_in_transaction(transaction, &privacy_case, &plan).await
        })
    }
}

pub fn postgres_case_approval_executor(
    store: PostgresDataStore,
) -> Arc<dyn TransactionalCapabilityExecutor> {
    Arc::new(PostgresTransactionalAggregateExecutor::guarded(
        store,
        Arc::new(CustomerPrivacyCaseApprovalCapabilityPlanner),
        Arc::new(PostgresCustomerPrivacyApprovalGuard),
    ))
}

fn validate_exact_lineage(
    privacy_case: &PrivacyCase,
    snapshot: &DiscoveryScopeSnapshot,
    plan: &PrivacyActionPlan,
    binding: &crm_customer_privacy::SubjectBinding,
) -> Result<(), SdkError> {
    let snapshot_lineage = snapshot.lineage();
    let plan_lineage = plan.lineage();
    let expected_resulting_version = plan_lineage
        .source_case_version()
        .checked_add(1)
        .ok_or_else(|| approval_evidence_invalid("planning case version overflowed"))?;

    if !plan_lineage.approval_required()
        || expected_resulting_version != privacy_case.version()
        || plan_lineage.privacy_case_id() != privacy_case.case_id()
        || plan_lineage.tenant_id() != privacy_case.tenant_id()
        || plan_lineage.case_kind() != privacy_case.kind()
        || plan_lineage.policy_version() != privacy_case.policy_version()
        || plan_lineage.canonical_party_id() != &binding.canonical_party_id
        || plan_lineage.identity_resolution_generation()
            != binding.identity_resolution_generation
        || plan_lineage.scope_snapshot_id() != snapshot.snapshot_id()
        || privacy_case.scope_snapshot_id() != Some(snapshot.snapshot_id())
        || privacy_case.action_plan_id() != Some(plan.plan_id())
    {
        return Err(approval_evidence_invalid(
            "case and immutable action-plan lineage do not match exactly",
        ));
    }

    if snapshot_lineage.privacy_case_id() != privacy_case.case_id()
        || snapshot_lineage.tenant_id() != privacy_case.tenant_id()
        || snapshot_lineage.canonical_party_id() != &binding.canonical_party_id
        || snapshot_lineage.identity_resolution_generation()
            != binding.identity_resolution_generation
        || plan_lineage.scope_snapshot_binding_digest() != snapshot.binding_digest()
        || plan_lineage.scope_completeness_digest()
            != snapshot.aggregation().completeness_digest()
        || plan_lineage.registry_digest() != snapshot_lineage.registry_digest()
        || plan_lineage.purpose_code() != snapshot_lineage.purpose_code()
        || plan_lineage.effective_request_at_unix_ms()
            != snapshot_lineage.effective_request_at_unix_ms()
        || plan_lineage.snapshot_captured_at_unix_nanos()
            != snapshot.captured_at_unix_nanos()
    {
        return Err(approval_evidence_invalid(
            "scope snapshot and immutable action-plan lineage do not match exactly",
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum RowLock {
    Share,
    Update,
}

async fn load_case_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    case_id: &RecordId,
    lock: RowLock,
) -> Result<Option<PrivacyCase>, SdkError> {
    let row = select_record_row(
        transaction,
        PRIVACY_CASE_RECORD_TYPE,
        case_id,
        lock,
    )
    .await?;
    row.map(|row| decode_case_row(case_id.clone(), row))
        .transpose()
}

async fn load_snapshot_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    snapshot_id: &RecordId,
    lock: RowLock,
) -> Result<Option<DiscoveryScopeSnapshot>, SdkError> {
    let row = select_record_row(
        transaction,
        SCOPE_SNAPSHOT_RECORD_TYPE,
        snapshot_id,
        lock,
    )
    .await?;
    row.map(|row| decode_snapshot_row(snapshot_id, row)).transpose()
}

async fn load_plan_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    plan_id: &RecordId,
    lock: RowLock,
) -> Result<Option<PrivacyActionPlan>, SdkError> {
    let row = select_record_row(transaction, ACTION_PLAN_RECORD_TYPE, plan_id, lock).await?;
    row.map(|row| decode_plan_row(plan_id, row)).transpose()
}

async fn select_record_row(
    transaction: &mut Transaction<'_, Postgres>,
    record_type: &str,
    record_id: &RecordId,
    lock: RowLock,
) -> Result<Option<postgres_sqlx::postgres::PgRow>, SdkError> {
    let query = match lock {
        RowLock::Share => postgres_sqlx::query(
            r#"
            SELECT version, owner_module_id, schema_id, schema_version, descriptor_hash,
                   data_class, payload_encoding, maximum_payload_size, retention_policy_id,
                   payload_bytes
            FROM crm.records
            WHERE tenant_id = current_setting('app.tenant_id', true)
              AND owner_module_id = $1 AND record_type = $2 AND record_id = $3
              AND deleted_at IS NULL
            FOR SHARE
            "#,
        ),
        RowLock::Update => postgres_sqlx::query(
            r#"
            SELECT version, owner_module_id, schema_id, schema_version, descriptor_hash,
                   data_class, payload_encoding, maximum_payload_size, retention_policy_id,
                   payload_bytes
            FROM crm.records
            WHERE tenant_id = current_setting('app.tenant_id', true)
              AND owner_module_id = $1 AND record_type = $2 AND record_id = $3
              AND deleted_at IS NULL
            FOR UPDATE
            "#,
        ),
    };
    query
        .bind(MODULE_ID)
        .bind(record_type)
        .bind(record_id.as_str())
        .fetch_optional(&mut **transaction)
        .await
        .map_err(database_error)
}

fn decode_case_row(
    case_id: RecordId,
    row: postgres_sqlx::postgres::PgRow,
) -> Result<PrivacyCase, SdkError> {
    let snapshot = decode_record_snapshot(PRIVACY_CASE_RECORD_TYPE, case_id, row)?;
    privacy_case_from_snapshot(&snapshot).map_err(|error| {
        approval_evidence_invalid(format!("privacy case state is invalid: {}", error.code))
    })
}

fn decode_snapshot_row(
    snapshot_id: &RecordId,
    row: postgres_sqlx::postgres::PgRow,
) -> Result<DiscoveryScopeSnapshot, SdkError> {
    let snapshot = decode_record_snapshot(SCOPE_SNAPSHOT_RECORD_TYPE, snapshot_id.clone(), row)?;
    validate_immutable_contract(
        &snapshot,
        DISCOVERY_SCOPE_SNAPSHOT_STATE_SCHEMA_ID,
        DISCOVERY_SCOPE_SNAPSHOT_STATE_SCHEMA_VERSION,
        discovery_scope_snapshot_state_descriptor_hash(),
        DISCOVERY_SCOPE_SNAPSHOT_STATE_MAXIMUM_BYTES,
        DISCOVERY_SCOPE_SNAPSHOT_STATE_RETENTION_POLICY_ID,
    )?;
    let value = decode_discovery_scope_snapshot_state(&snapshot.payload.bytes)
        .map_err(|error| approval_evidence_invalid(error.to_string()))?;
    if value.snapshot_id() != snapshot_id {
        return Err(approval_evidence_invalid(
            "scope snapshot identity differs from its record envelope",
        ));
    }
    Ok(value)
}

fn decode_plan_row(
    plan_id: &RecordId,
    row: postgres_sqlx::postgres::PgRow,
) -> Result<PrivacyActionPlan, SdkError> {
    let snapshot = decode_record_snapshot(ACTION_PLAN_RECORD_TYPE, plan_id.clone(), row)?;
    validate_immutable_contract(
        &snapshot,
        ACTION_PLAN_STATE_SCHEMA_ID,
        ACTION_PLAN_STATE_SCHEMA_VERSION,
        action_plan_state_descriptor_hash(),
        ACTION_PLAN_STATE_MAXIMUM_BYTES,
        ACTION_PLAN_STATE_RETENTION_POLICY_ID,
    )?;
    let value = decode_action_plan_state(&snapshot.payload.bytes)
        .map_err(|error| approval_evidence_invalid(error.to_string()))?;
    if value.plan_id() != plan_id {
        return Err(approval_evidence_invalid(
            "action plan identity differs from its record envelope",
        ));
    }
    Ok(value)
}

fn decode_record_snapshot(
    record_type: &str,
    record_id: RecordId,
    row: postgres_sqlx::postgres::PgRow,
) -> Result<RecordSnapshot, SdkError> {
    let version: i64 = row.try_get("version").map_err(approval_evidence_invalid)?;
    let owner: String = row
        .try_get("owner_module_id")
        .map_err(approval_evidence_invalid)?;
    let schema_id: String = row.try_get("schema_id").map_err(approval_evidence_invalid)?;
    let schema_version: String = row
        .try_get("schema_version")
        .map_err(approval_evidence_invalid)?;
    let descriptor_hash: Vec<u8> = row
        .try_get("descriptor_hash")
        .map_err(approval_evidence_invalid)?;
    let data_class: String = row
        .try_get("data_class")
        .map_err(approval_evidence_invalid)?;
    let encoding: String = row
        .try_get("payload_encoding")
        .map_err(approval_evidence_invalid)?;
    let maximum: i64 = row
        .try_get("maximum_payload_size")
        .map_err(approval_evidence_invalid)?;
    let retention: String = row
        .try_get("retention_policy_id")
        .map_err(approval_evidence_invalid)?;
    let bytes: Vec<u8> = row
        .try_get("payload_bytes")
        .map_err(approval_evidence_invalid)?;
    if data_class != "confidential" || encoding != "json" {
        return Err(approval_evidence_invalid(
            "approval evidence record data class or encoding drifted",
        ));
    }
    let descriptor_hash: [u8; 32] = descriptor_hash.try_into().map_err(|_| {
        approval_evidence_invalid("approval evidence descriptor hash must contain 32 bytes")
    })?;
    Ok(RecordSnapshot {
        reference: RecordRef {
            record_type: RecordType::try_new(record_type).map_err(approval_evidence_invalid)?,
            record_id,
        },
        version,
        payload: TypedPayload {
            owner: ModuleId::try_new(owner).map_err(approval_evidence_invalid)?,
            schema_id: SchemaId::try_new(schema_id).map_err(approval_evidence_invalid)?,
            schema_version: SchemaVersion::try_new(schema_version)
                .map_err(approval_evidence_invalid)?,
            descriptor_hash,
            data_class: DataClass::Confidential,
            encoding: PayloadEncoding::Json,
            maximum_size_bytes: u64::try_from(maximum).map_err(|_| {
                approval_evidence_invalid("approval evidence maximum size is negative")
            })?,
            retention_policy_id: RetentionPolicyId::try_new(retention)
                .map_err(approval_evidence_invalid)?,
            bytes,
        },
    })
}

fn validate_immutable_contract(
    snapshot: &RecordSnapshot,
    schema_id: &str,
    schema_version: &str,
    descriptor_hash: [u8; 32],
    maximum_size_bytes: u64,
    retention_policy_id: &str,
) -> Result<(), SdkError> {
    if snapshot.version != 1
        || snapshot.payload.owner.as_str() != MODULE_ID
        || snapshot.payload.schema_id.as_str() != schema_id
        || snapshot.payload.schema_version.as_str() != schema_version
        || snapshot.payload.descriptor_hash != descriptor_hash
        || snapshot.payload.maximum_size_bytes != maximum_size_bytes
        || snapshot.payload.retention_policy_id.as_str() != retention_policy_id
        || snapshot.payload.validate().is_err()
    {
        return Err(approval_evidence_invalid(
            "immutable approval evidence record envelope drifted",
        ));
    }
    Ok(())
}

async fn verify_planning_link_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    privacy_case: &PrivacyCase,
    plan: &PrivacyActionPlan,
) -> Result<(), SdkError> {
    let row = postgres_sqlx::query(
        r#"
        SELECT source_case_version, resulting_case_version, scope_snapshot_id,
               plan_id, plan_digest, approval_required,
               (extract(epoch FROM planned_at) * 1000000000)::bigint AS planned_at_unix_nanos
        FROM crm.customer_privacy_action_plans
        WHERE tenant_id = current_setting('app.tenant_id', true)
          AND privacy_case_id = $1
        "#,
    )
    .bind(privacy_case.case_id().as_str())
    .fetch_optional(&mut **transaction)
    .await
    .map_err(database_error)?
    .ok_or_else(|| approval_evidence_invalid("immutable planning link is missing"))?;

    let source_case_version: i64 = row
        .try_get("source_case_version")
        .map_err(approval_evidence_invalid)?;
    let resulting_case_version: i64 = row
        .try_get("resulting_case_version")
        .map_err(approval_evidence_invalid)?;
    let plan_digest: Vec<u8> = row
        .try_get("plan_digest")
        .map_err(approval_evidence_invalid)?;
    let plan_digest: [u8; 32] = plan_digest.try_into().map_err(|_| {
        approval_evidence_invalid("planning link digest must contain exactly 32 bytes")
    })?;

    if source_case_version
        != i64::try_from(plan.lineage().source_case_version())
            .map_err(|_| approval_evidence_invalid("source case version exceeds PostgreSQL range"))?
        || resulting_case_version
            != i64::try_from(privacy_case.version()).map_err(|_| {
                approval_evidence_invalid("resulting case version exceeds PostgreSQL range")
            })?
        || row
            .try_get::<String, _>("scope_snapshot_id")
            .map_err(approval_evidence_invalid)?
            != plan.lineage().scope_snapshot_id().as_str()
        || row
            .try_get::<String, _>("plan_id")
            .map_err(approval_evidence_invalid)?
            != plan.plan_id().as_str()
        || plan_digest != *plan.digest()
        || !row
            .try_get::<bool, _>("approval_required")
            .map_err(approval_evidence_invalid)?
        || row
            .try_get::<i64, _>("planned_at_unix_nanos")
            .map_err(approval_evidence_invalid)?
            != plan.planned_at_unix_nanos()
    {
        return Err(approval_evidence_invalid(
            "immutable planning link conflicts with case and plan evidence",
        ));
    }
    Ok(())
}

fn database_error(reference: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_APPROVAL_STORE_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "Customer Privacy approval storage is unavailable.",
    )
    .with_internal_reference(reference.to_string())
}

fn case_not_found() -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_CASE_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The privacy case was not found.",
    )
}

fn version_conflict(expected: u64, actual: u64) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_VERSION_CONFLICT",
        ErrorCategory::Conflict,
        true,
        "The privacy case changed before approval could be committed.",
    )
    .with_internal_reference(format!("expected version {expected}, actual version {actual}"))
}

fn approval_conflict(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_APPROVAL_CONFLICT",
        ErrorCategory::Conflict,
        false,
        "The privacy case is not eligible for approval.",
    )
    .with_internal_reference(reference)
}

fn approval_evidence_invalid(reference: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_APPROVAL_EVIDENCE_INVALID",
        ErrorCategory::Internal,
        false,
        "Customer Privacy approval evidence is invalid.",
    )
    .with_internal_reference(reference.to_string())
}

fn approval_guard_unsupported() -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_APPROVAL_GUARD_UNSUPPORTED",
        ErrorCategory::InvalidArgument,
        false,
        "The Customer Privacy approval guard does not support this capability.",
    )
}
