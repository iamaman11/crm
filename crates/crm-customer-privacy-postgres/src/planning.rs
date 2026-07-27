use crm_core_data::{
    PostgresDataStore,
    postgres_sqlx::{self, Postgres, Row, Transaction},
};
use crm_customer_privacy::{
    ACTION_PLAN_RECORD_TYPE, ACTION_PLAN_STATE_MAXIMUM_BYTES, ACTION_PLAN_STATE_RETENTION_POLICY_ID,
    ACTION_PLAN_STATE_SCHEMA_ID, ACTION_PLAN_STATE_SCHEMA_VERSION, DiscoveryScopeSnapshot, MODULE_ID,
    PRIVACY_CASE_RECORD_TYPE, PrivacyActionPlan, PrivacyCase, PrivacyCaseStatus,
    SCOPE_SNAPSHOT_RECORD_TYPE, action_plan_state_descriptor_hash, decode_action_plan_state,
    decode_discovery_scope_snapshot_state, discovery_scope_snapshot_state_descriptor_hash,
    discovery_sha256, encode_action_plan_state, DISCOVERY_SCOPE_SNAPSHOT_STATE_MAXIMUM_BYTES,
    DISCOVERY_SCOPE_SNAPSHOT_STATE_RETENTION_POLICY_ID, DISCOVERY_SCOPE_SNAPSHOT_STATE_SCHEMA_ID,
    DISCOVERY_SCOPE_SNAPSHOT_STATE_SCHEMA_VERSION,
};
use crm_customer_privacy_application::{
    PlanningCommit, PlanningInvocation, PlanningPersistencePort, PlanningSource,
};
use crm_customer_privacy_persistence_adapter::{
    privacy_case_from_snapshot, privacy_case_persisted_payload,
};
use crm_module_sdk::{
    DataClass, ErrorCategory, ModuleId, PayloadEncoding, PortFuture, RecordId, RecordRef,
    RecordSnapshot, RecordType, RetentionPolicyId, SchemaId, SchemaVersion, SdkError, TenantId,
    TypedPayload,
};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct PostgresPlanningPersistence {
    store: Arc<PostgresDataStore>,
}

impl PostgresPlanningPersistence {
    pub fn new(store: Arc<PostgresDataStore>) -> Self {
        Self { store }
    }

    pub fn store(&self) -> &Arc<PostgresDataStore> {
        &self.store
    }
}

impl PlanningPersistencePort for PostgresPlanningPersistence {
    fn load_source<'a>(
        &'a self,
        invocation: &'a PlanningInvocation,
    ) -> PortFuture<'a, Result<PlanningSource, SdkError>> {
        Box::pin(async move {
            let mut transaction = self.store.pool().begin().await.map_err(database_error)?;
            bind_invocation(&mut transaction, invocation).await?;
            let privacy_case = load_case_in_transaction(
                &mut transaction,
                &invocation.tenant_id,
                &invocation.privacy_case_id,
                RowLock::Share,
            )
            .await?
            .ok_or_else(case_not_found)?;
            let snapshot_id = privacy_case.scope_snapshot_id().ok_or_else(|| {
                planning_conflict("privacy case has no immutable scope snapshot reference")
            })?;
            let scope_snapshot = load_snapshot_in_transaction(
                &mut transaction,
                &invocation.tenant_id,
                snapshot_id,
                RowLock::Share,
            )
            .await?
            .ok_or_else(|| planning_conflict("privacy case scope snapshot is unavailable"))?;
            let existing_plan = match privacy_case.action_plan_id() {
                Some(plan_id) => Some(
                    load_plan_in_transaction(
                        &mut transaction,
                        &invocation.tenant_id,
                        plan_id,
                        RowLock::Share,
                    )
                    .await?
                    .ok_or_else(|| planning_evidence_invalid("referenced action plan is missing"))?,
                ),
                None => None,
            };
            transaction.commit().await.map_err(database_error)?;
            Ok(PlanningSource {
                privacy_case,
                scope_snapshot,
                existing_plan,
            })
        })
    }

    fn finalize_plan<'a>(
        &'a self,
        invocation: &'a PlanningInvocation,
        plan: &'a PrivacyActionPlan,
    ) -> PortFuture<'a, Result<PlanningCommit, SdkError>> {
        Box::pin(async move {
            let mut transaction = self.store.pool().begin().await.map_err(database_error)?;
            bind_invocation(&mut transaction, invocation).await?;
            let mut privacy_case = load_case_in_transaction(
                &mut transaction,
                &invocation.tenant_id,
                &invocation.privacy_case_id,
                RowLock::Update,
            )
            .await?
            .ok_or_else(case_not_found)?;

            if matches!(
                privacy_case.status(),
                PrivacyCaseStatus::Planned | PrivacyCaseStatus::AwaitingApproval
            ) {
                let existing_id = privacy_case.action_plan_id().ok_or_else(|| {
                    planning_evidence_invalid("planned case has no action plan reference")
                })?;
                let existing = load_plan_in_transaction(
                    &mut transaction,
                    &invocation.tenant_id,
                    existing_id,
                    RowLock::Share,
                )
                .await?
                .ok_or_else(|| planning_evidence_invalid("planned action plan is missing"))?;
                if &existing != plan {
                    return Err(planning_conflict(
                        "concurrent planning replay differs from durable action plan",
                    ));
                }
                verify_link_in_transaction(
                    &mut transaction,
                    &invocation.tenant_id,
                    &privacy_case,
                    &existing,
                )
                .await?;
                insert_audit_in_transaction(
                    &mut transaction,
                    invocation,
                    &existing,
                    "planning_replayed",
                    privacy_case.version(),
                )
                .await?;
                transaction.commit().await.map_err(database_error)?;
                return Ok(PlanningCommit {
                    privacy_case,
                    action_plan: existing,
                    replayed: true,
                });
            }

            if privacy_case.status() != PrivacyCaseStatus::Scoped
                || privacy_case.action_plan_id().is_some()
            {
                return Err(planning_conflict(
                    "privacy case cannot accept an action plan in its current state",
                ));
            }
            if privacy_case.case_id() != plan.lineage().privacy_case_id()
                || privacy_case.tenant_id() != plan.lineage().tenant_id()
                || privacy_case.version() != plan.lineage().source_case_version()
                || privacy_case.kind() != plan.lineage().case_kind()
                || privacy_case.policy_version() != plan.lineage().policy_version()
            {
                return Err(planning_conflict(
                    "action plan lineage differs from the locked privacy case",
                ));
            }
            let binding = privacy_case.subject_binding().ok_or_else(|| {
                planning_conflict("locked privacy case has no verified subject binding")
            })?;
            if binding.canonical_party_id != *plan.lineage().canonical_party_id()
                || binding.identity_resolution_generation
                    != plan.lineage().identity_resolution_generation()
                || privacy_case.scope_snapshot_id() != Some(plan.lineage().scope_snapshot_id())
            {
                return Err(planning_conflict(
                    "action plan subject or snapshot lineage differs from the locked case",
                ));
            }

            let snapshot = load_snapshot_in_transaction(
                &mut transaction,
                &invocation.tenant_id,
                plan.lineage().scope_snapshot_id(),
                RowLock::Share,
            )
            .await?
            .ok_or_else(|| planning_conflict("action plan source snapshot is unavailable"))?;
            let expected = PrivacyActionPlan::build(
                &snapshot,
                privacy_case.version(),
                privacy_case.kind(),
                invocation.policy.clone(),
                invocation.proposed_planned_at_unix_nanos,
            )
            .map_err(domain_error)?;
            if &expected != plan {
                return Err(planning_conflict(
                    "action plan differs from deterministic locked inputs",
                ));
            }

            insert_plan_record_in_transaction(&mut transaction, plan).await?;
            let persisted_plan = load_plan_in_transaction(
                &mut transaction,
                &invocation.tenant_id,
                plan.plan_id(),
                RowLock::Share,
            )
            .await?
            .ok_or_else(|| planning_evidence_invalid("inserted action plan is missing"))?;
            if &persisted_plan != plan {
                return Err(planning_conflict(
                    "existing action plan record conflicts with deterministic content",
                ));
            }

            let source_case_version = privacy_case.version();
            privacy_case
                .record_plan(
                    source_case_version,
                    plan.plan_id().clone(),
                    invocation.policy.approval_required(),
                    invocation.proposed_planned_at_unix_nanos,
                )
                .map_err(domain_error)?;
            update_case_record_in_transaction(
                &mut transaction,
                source_case_version,
                &privacy_case,
                transaction_id(invocation),
            )
            .await?;
            insert_link_in_transaction(
                &mut transaction,
                &invocation.tenant_id,
                source_case_version,
                &privacy_case,
                plan,
            )
            .await?;
            insert_audit_in_transaction(
                &mut transaction,
                invocation,
                plan,
                "planning_finalized",
                privacy_case.version(),
            )
            .await?;
            transaction.commit().await.map_err(database_error)?;
            Ok(PlanningCommit {
                privacy_case,
                action_plan: persisted_plan,
                replayed: false,
            })
        })
    }
}

#[derive(Debug, Clone, Copy)]
enum RowLock {
    Share,
    Update,
}

async fn load_case_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: &TenantId,
    case_id: &RecordId,
    lock: RowLock,
) -> Result<Option<PrivacyCase>, SdkError> {
    let row = select_record_row(
        transaction,
        tenant_id,
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
    tenant_id: &TenantId,
    snapshot_id: &RecordId,
    lock: RowLock,
) -> Result<Option<DiscoveryScopeSnapshot>, SdkError> {
    let row = select_record_row(
        transaction,
        tenant_id,
        SCOPE_SNAPSHOT_RECORD_TYPE,
        snapshot_id,
        lock,
    )
    .await?;
    row.map(|row| decode_snapshot_row(tenant_id, snapshot_id, row))
        .transpose()
}

async fn load_plan_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: &TenantId,
    plan_id: &RecordId,
    lock: RowLock,
) -> Result<Option<PrivacyActionPlan>, SdkError> {
    let row = select_record_row(
        transaction,
        tenant_id,
        ACTION_PLAN_RECORD_TYPE,
        plan_id,
        lock,
    )
    .await?;
    row.map(|row| decode_plan_row(tenant_id, plan_id, row))
        .transpose()
}

async fn select_record_row(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: &TenantId,
    record_type: &str,
    record_id: &RecordId,
    lock: RowLock,
) -> Result<Option<postgres_sqlx::postgres::PgRow>, SdkError> {
    let suffix = match lock {
        RowLock::Share => " FOR SHARE",
        RowLock::Update => " FOR UPDATE",
    };
    let sql = format!(
        "SELECT version, owner_module_id, schema_id, schema_version, descriptor_hash, \
         data_class, payload_encoding, maximum_payload_size, retention_policy_id, payload_bytes \
         FROM crm.records WHERE tenant_id = $1 AND owner_module_id = $2 \
         AND record_type = $3 AND record_id = $4 AND deleted_at IS NULL{suffix}"
    );
    postgres_sqlx::query(&sql)
        .bind(tenant_id.as_str())
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
    privacy_case_from_snapshot(&snapshot).map_err(planning_state_invalid)
}

fn decode_snapshot_row(
    tenant_id: &TenantId,
    snapshot_id: &RecordId,
    row: postgres_sqlx::postgres::PgRow,
) -> Result<DiscoveryScopeSnapshot, SdkError> {
    let snapshot = decode_record_snapshot(SCOPE_SNAPSHOT_RECORD_TYPE, snapshot_id.clone(), row)?;
    validate_contract(
        &snapshot,
        DISCOVERY_SCOPE_SNAPSHOT_STATE_SCHEMA_ID,
        DISCOVERY_SCOPE_SNAPSHOT_STATE_SCHEMA_VERSION,
        discovery_scope_snapshot_state_descriptor_hash(),
        DISCOVERY_SCOPE_SNAPSHOT_STATE_MAXIMUM_BYTES,
        DISCOVERY_SCOPE_SNAPSHOT_STATE_RETENTION_POLICY_ID,
    )?;
    let value = decode_discovery_scope_snapshot_state(&snapshot.payload.bytes)?;
    if value.snapshot_id() != snapshot_id || value.lineage().tenant_id() != tenant_id {
        return Err(planning_state_invalid(
            "scope snapshot identity differs from its record envelope",
        ));
    }
    Ok(value)
}

fn decode_plan_row(
    tenant_id: &TenantId,
    plan_id: &RecordId,
    row: postgres_sqlx::postgres::PgRow,
) -> Result<PrivacyActionPlan, SdkError> {
    let snapshot = decode_record_snapshot(ACTION_PLAN_RECORD_TYPE, plan_id.clone(), row)?;
    validate_contract(
        &snapshot,
        ACTION_PLAN_STATE_SCHEMA_ID,
        ACTION_PLAN_STATE_SCHEMA_VERSION,
        action_plan_state_descriptor_hash(),
        ACTION_PLAN_STATE_MAXIMUM_BYTES,
        ACTION_PLAN_STATE_RETENTION_POLICY_ID,
    )?;
    let value = decode_action_plan_state(&snapshot.payload.bytes)?;
    if value.plan_id() != plan_id || value.lineage().tenant_id() != tenant_id {
        return Err(planning_state_invalid(
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
    let version: i64 = row.try_get("version").map_err(planning_state_invalid)?;
    let owner: String = row
        .try_get("owner_module_id")
        .map_err(planning_state_invalid)?;
    let schema_id: String = row.try_get("schema_id").map_err(planning_state_invalid)?;
    let schema_version: String = row
        .try_get("schema_version")
        .map_err(planning_state_invalid)?;
    let descriptor_hash: Vec<u8> = row
        .try_get("descriptor_hash")
        .map_err(planning_state_invalid)?;
    let data_class: String = row.try_get("data_class").map_err(planning_state_invalid)?;
    let encoding: String = row
        .try_get("payload_encoding")
        .map_err(planning_state_invalid)?;
    let maximum: i64 = row
        .try_get("maximum_payload_size")
        .map_err(planning_state_invalid)?;
    let retention: String = row
        .try_get("retention_policy_id")
        .map_err(planning_state_invalid)?;
    let bytes: Vec<u8> = row
        .try_get("payload_bytes")
        .map_err(planning_state_invalid)?;
    if data_class != "confidential" || encoding != "json" {
        return Err(planning_state_invalid(
            "planning record data class or encoding differs from its contract",
        ));
    }
    let descriptor_hash: [u8; 32] = descriptor_hash.try_into().map_err(|_| {
        planning_state_invalid("planning record descriptor hash must contain exactly 32 bytes")
    })?;
    let maximum_size_bytes = u64::try_from(maximum)
        .map_err(|_| planning_state_invalid("planning record maximum size is negative"))?;
    Ok(RecordSnapshot {
        reference: RecordRef {
            record_type: RecordType::try_new(record_type).map_err(planning_state_invalid)?,
            record_id,
        },
        version,
        payload: TypedPayload {
            owner: ModuleId::try_new(owner).map_err(planning_state_invalid)?,
            schema_id: SchemaId::try_new(schema_id).map_err(planning_state_invalid)?,
            schema_version: SchemaVersion::try_new(schema_version)
                .map_err(planning_state_invalid)?,
            descriptor_hash,
            data_class: DataClass::Confidential,
            encoding: PayloadEncoding::Json,
            maximum_size_bytes,
            retention_policy_id: RetentionPolicyId::try_new(retention)
                .map_err(planning_state_invalid)?,
            bytes,
        },
    })
}

fn validate_contract(
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
    {
        return Err(planning_state_invalid(
            "planning evidence record envelope drifted",
        ));
    }
    Ok(())
}

async fn insert_plan_record_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    plan: &PrivacyActionPlan,
) -> Result<(), SdkError> {
    let bytes = encode_action_plan_state(plan)?;
    postgres_sqlx::query(
        r#"
        INSERT INTO crm.records (
          tenant_id, record_type, record_id, version, owner_module_id,
          schema_id, schema_version, descriptor_hash, data_class,
          payload_encoding, maximum_payload_size, retention_policy_id,
          payload_bytes, last_business_transaction_id
        ) VALUES ($1,$2,$3,1,$4,$5,$6,$7,'confidential','json',$8,$9,$10,$11)
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(plan.lineage().tenant_id().as_str())
    .bind(ACTION_PLAN_RECORD_TYPE)
    .bind(plan.plan_id().as_str())
    .bind(MODULE_ID)
    .bind(ACTION_PLAN_STATE_SCHEMA_ID)
    .bind(ACTION_PLAN_STATE_SCHEMA_VERSION)
    .bind(action_plan_state_descriptor_hash().as_slice())
    .bind(i64::try_from(ACTION_PLAN_STATE_MAXIMUM_BYTES).map_err(|_| {
        planning_state_invalid("action plan maximum size exceeds PostgreSQL range")
    })?)
    .bind(ACTION_PLAN_STATE_RETENTION_POLICY_ID)
    .bind(bytes)
    .bind(format!("privacy-plan-{}", hex(&plan.digest()[..12])))
    .execute(&mut **transaction)
    .await
    .map_err(database_error)?;
    Ok(())
}

async fn update_case_record_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    expected_version: u64,
    privacy_case: &PrivacyCase,
    business_transaction_id: String,
) -> Result<(), SdkError> {
    let payload = privacy_case_persisted_payload(privacy_case)?;
    let expected_version = i64::try_from(expected_version)
        .map_err(|_| planning_state_invalid("source case version exceeds PostgreSQL range"))?;
    let resulting_version = i64::try_from(privacy_case.version())
        .map_err(|_| planning_state_invalid("resulting case version exceeds PostgreSQL range"))?;
    let maximum = i64::try_from(payload.maximum_size_bytes)
        .map_err(|_| planning_state_invalid("case maximum size exceeds PostgreSQL range"))?;
    let result = postgres_sqlx::query(
        r#"
        UPDATE crm.records
        SET version = $4,
            schema_id = $5,
            schema_version = $6,
            descriptor_hash = $7,
            data_class = 'confidential',
            payload_encoding = 'json',
            maximum_payload_size = $8,
            retention_policy_id = $9,
            payload_bytes = $10,
            last_business_transaction_id = $11,
            updated_at = clock_timestamp()
        WHERE tenant_id = $1 AND owner_module_id = $2
          AND record_type = $3 AND record_id = $12
          AND version = $13 AND deleted_at IS NULL
        "#,
    )
    .bind(privacy_case.tenant_id().as_str())
    .bind(MODULE_ID)
    .bind(PRIVACY_CASE_RECORD_TYPE)
    .bind(resulting_version)
    .bind(payload.schema_id.as_str())
    .bind(payload.schema_version.as_str())
    .bind(payload.descriptor_hash.as_slice())
    .bind(maximum)
    .bind(payload.retention_policy_id.as_str())
    .bind(payload.bytes)
    .bind(business_transaction_id)
    .bind(privacy_case.case_id().as_str())
    .bind(expected_version)
    .execute(&mut **transaction)
    .await
    .map_err(database_error)?;
    if result.rows_affected() != 1 {
        return Err(planning_conflict(
            "privacy case changed before planning could commit",
        ));
    }
    Ok(())
}

async fn insert_link_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: &TenantId,
    source_case_version: u64,
    privacy_case: &PrivacyCase,
    plan: &PrivacyActionPlan,
) -> Result<(), SdkError> {
    let source_case_version = i64::try_from(source_case_version)
        .map_err(|_| planning_state_invalid("source case version exceeds PostgreSQL range"))?;
    let resulting_case_version = i64::try_from(privacy_case.version())
        .map_err(|_| planning_state_invalid("resulting case version exceeds PostgreSQL range"))?;
    postgres_sqlx::query(
        r#"
        INSERT INTO crm.customer_privacy_action_plans (
          tenant_id, privacy_case_id, source_case_version, resulting_case_version,
          scope_snapshot_id, plan_id, plan_digest, approval_required, planned_at
        ) VALUES (
          $1,$2,$3,$4,$5,$6,$7,$8,
          TIMESTAMPTZ 'epoch' + ($9::bigint / 1000) * INTERVAL '1 microsecond'
        )
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(tenant_id.as_str())
    .bind(privacy_case.case_id().as_str())
    .bind(source_case_version)
    .bind(resulting_case_version)
    .bind(plan.lineage().scope_snapshot_id().as_str())
    .bind(plan.plan_id().as_str())
    .bind(plan.digest().as_slice())
    .bind(plan.lineage().approval_required())
    .bind(plan.planned_at_unix_nanos())
    .execute(&mut **transaction)
    .await
    .map_err(database_error)?;
    verify_link_in_transaction(transaction, tenant_id, privacy_case, plan).await
}

async fn verify_link_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: &TenantId,
    privacy_case: &PrivacyCase,
    plan: &PrivacyActionPlan,
) -> Result<(), SdkError> {
    let row = postgres_sqlx::query(
        r#"
        SELECT source_case_version, resulting_case_version, scope_snapshot_id,
               plan_id, plan_digest, approval_required,
               (extract(epoch FROM planned_at) * 1000000000)::bigint AS planned_at_unix_nanos
        FROM crm.customer_privacy_action_plans
        WHERE tenant_id = $1 AND privacy_case_id = $2
        "#,
    )
    .bind(tenant_id.as_str())
    .bind(privacy_case.case_id().as_str())
    .fetch_one(&mut **transaction)
    .await
    .map_err(database_error)?;
    let source_case_version: i64 = row
        .try_get("source_case_version")
        .map_err(database_error)?;
    let resulting_case_version: i64 = row
        .try_get("resulting_case_version")
        .map_err(database_error)?;
    let digest = digest_column(&row, "plan_digest")?;
    if source_case_version
        != i64::try_from(plan.lineage().source_case_version())
            .map_err(|_| planning_state_invalid("source case version exceeds PostgreSQL range"))?
        || resulting_case_version
            != i64::try_from(privacy_case.version()).map_err(|_| {
                planning_state_invalid("resulting case version exceeds PostgreSQL range")
            })?
        || row
            .try_get::<String, _>("scope_snapshot_id")
            .map_err(database_error)?
            != plan.lineage().scope_snapshot_id().as_str()
        || row
            .try_get::<String, _>("plan_id")
            .map_err(database_error)?
            != plan.plan_id().as_str()
        || digest != *plan.digest()
        || row
            .try_get::<bool, _>("approval_required")
            .map_err(database_error)?
            != plan.lineage().approval_required()
        || row
            .try_get::<i64, _>("planned_at_unix_nanos")
            .map_err(database_error)?
            != plan.planned_at_unix_nanos()
    {
        return Err(planning_conflict(
            "planning replay evidence conflicts with deterministic content",
        ));
    }
    Ok(())
}

async fn insert_audit_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    invocation: &PlanningInvocation,
    plan: &PrivacyActionPlan,
    event_type: &str,
    resulting_case_version: u64,
) -> Result<(), SdkError> {
    let resulting_case_version = i64::try_from(resulting_case_version)
        .map_err(|_| planning_state_invalid("audit case version exceeds PostgreSQL range"))?;
    let audit_digest = planning_audit_digest(invocation, plan, event_type, resulting_case_version);
    postgres_sqlx::query(
        r#"
        INSERT INTO crm.customer_privacy_planning_audit (
          tenant_id, audit_digest, event_type, privacy_case_id, plan_id,
          plan_digest, resulting_case_version, actor_id, request_id, occurred_at
        ) VALUES (
          $1,$2,$3,$4,$5,$6,$7,$8,$9,
          TIMESTAMPTZ 'epoch' + ($10::bigint / 1000) * INTERVAL '1 microsecond'
        )
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(invocation.tenant_id.as_str())
    .bind(audit_digest.as_slice())
    .bind(event_type)
    .bind(invocation.privacy_case_id.as_str())
    .bind(plan.plan_id().as_str())
    .bind(plan.digest().as_slice())
    .bind(resulting_case_version)
    .bind(invocation.actor_id.as_str())
    .bind(invocation.request_id.as_str())
    .bind(invocation.proposed_planned_at_unix_nanos)
    .execute(&mut **transaction)
    .await
    .map_err(database_error)?;
    Ok(())
}

async fn bind_invocation(
    transaction: &mut Transaction<'_, Postgres>,
    invocation: &PlanningInvocation,
) -> Result<(), SdkError> {
    postgres_sqlx::query(
        r#"
        SELECT set_config('app.tenant_id', $1, true),
               set_config('app.actor_id', $2, true),
               set_config('app.request_id', $3, true),
               set_config('app.capability_id', 'customer_privacy.plan.build', true),
               set_config('app.capability_version', '1.0.0', true),
               set_config('app.business_transaction_id', $4, true)
        "#,
    )
    .bind(invocation.tenant_id.as_str())
    .bind(invocation.actor_id.as_str())
    .bind(invocation.request_id.as_str())
    .bind(transaction_id(invocation))
    .execute(&mut **transaction)
    .await
    .map_err(database_error)?;
    Ok(())
}

fn transaction_id(invocation: &PlanningInvocation) -> String {
    let digest = discovery_sha256(invocation.request_id.as_str().as_bytes());
    format!("privacy-planning-{}", hex(&digest[..12]))
}

fn planning_audit_digest(
    invocation: &PlanningInvocation,
    plan: &PrivacyActionPlan,
    event_type: &str,
    resulting_case_version: i64,
) -> [u8; 32] {
    let mut bytes = Vec::new();
    for value in [
        b"crm.customer-privacy.planning-audit/v1".as_slice(),
        invocation.tenant_id.as_str().as_bytes(),
        invocation.privacy_case_id.as_str().as_bytes(),
        event_type.as_bytes(),
        plan.plan_id().as_str().as_bytes(),
        plan.digest().as_slice(),
        resulting_case_version.to_string().as_bytes(),
        invocation.actor_id.as_str().as_bytes(),
        invocation.request_id.as_str().as_bytes(),
    ] {
        bytes.extend_from_slice(&(value.len() as u64).to_be_bytes());
        bytes.extend_from_slice(value);
    }
    discovery_sha256(&bytes)
}

fn digest_column(
    row: &postgres_sqlx::postgres::PgRow,
    column: &str,
) -> Result<[u8; 32], SdkError> {
    let value: Vec<u8> = row.try_get(column).map_err(database_error)?;
    value
        .try_into()
        .map_err(|_| planning_state_invalid(format!("{column} must contain exactly 32 bytes")))
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn database_error(reference: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_PLANNING_STORE_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "Customer Privacy planning storage is unavailable.",
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

fn planning_conflict(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_PLANNING_CONFLICT",
        ErrorCategory::Conflict,
        false,
        "Customer Privacy planning conflicts with current authoritative state.",
    )
    .with_internal_reference(reference)
}

fn planning_evidence_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_PLANNING_EVIDENCE_INVALID",
        ErrorCategory::Internal,
        false,
        "Customer Privacy planning evidence is invalid.",
    )
    .with_internal_reference(reference)
}

fn planning_state_invalid(reference: impl std::fmt::Display) -> SdkError {
    planning_evidence_invalid(reference.to_string())
}

fn domain_error(error: impl std::fmt::Display) -> SdkError {
    planning_conflict(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planning_transaction_identity_is_deterministic() {
        let invocation = PlanningInvocation {
            tenant_id: TenantId::try_new("tenant-a").unwrap(),
            privacy_case_id: RecordId::try_new("case-a").unwrap(),
            actor_id: crm_module_sdk::ActorId::try_new("privacy-worker").unwrap(),
            request_id: crm_module_sdk::RequestId::try_new("request-a").unwrap(),
            correlation_id: crm_module_sdk::CorrelationId::try_new("correlation-a").unwrap(),
            trace_id: crm_module_sdk::TraceId::try_new("trace-a").unwrap(),
            request_started_at_unix_nanos: 10,
            proposed_planned_at_unix_nanos: 20,
            policy: crm_customer_privacy::ActionPlanningPolicy::new(
                SchemaVersion::try_new("privacy-policy/1").unwrap(),
                "EU",
                false,
                false,
            )
            .unwrap(),
            trusted_internal: true,
        };
        assert_eq!(transaction_id(&invocation), transaction_id(&invocation));
        assert!(transaction_id(&invocation).starts_with("privacy-planning-"));
    }
}
