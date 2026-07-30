use crm_core_data::{
    PostgresDataStore,
    postgres_sqlx::{self, Postgres, Row, Transaction},
};
use crm_customer_privacy::{
    ACCESS_EXPORT_REFERENCE_RECORD_TYPE, ACCESS_EXPORT_STATE_MAXIMUM_BYTES,
    ACCESS_EXPORT_STATE_RETENTION_POLICY_ID, ACCESS_EXPORT_STATE_SCHEMA_ID,
    ACCESS_EXPORT_STATE_SCHEMA_VERSION, ACTION_PLAN_RECORD_TYPE, ACTION_PLAN_STATE_MAXIMUM_BYTES,
    ACTION_PLAN_STATE_RETENTION_POLICY_ID, ACTION_PLAN_STATE_SCHEMA_ID,
    ACTION_PLAN_STATE_SCHEMA_VERSION, MODULE_ID, PRIVACY_CASE_RECORD_TYPE,
    PrivacyAccessExportManifest, PrivacyAccessExportReference, PrivacyAccessExportStatus,
    PrivacyActionPlan, PrivacyCase, PrivacyCaseStatus, access_export_state_descriptor_hash,
    action_plan_state_descriptor_hash, decode_access_export_reference, decode_action_plan_state,
    discovery_sha256, encode_access_export_reference,
};
use crm_customer_privacy_application::{
    AccessExportInvocation, AccessExportPersistencePort, AccessExportPreparation,
    PrivacyExportTargetResult,
};
use crm_customer_privacy_persistence_adapter::privacy_case_from_snapshot;
use crm_module_sdk::{
    DataClass, ErrorCategory, ModuleId, PayloadEncoding, PortFuture, RecordId, RecordRef,
    RecordSnapshot, RecordType, RetentionPolicyId, SchemaId, SchemaVersion, SdkError, TenantId,
    TypedPayload,
};
use std::sync::Arc;

const ACCESS_EXPORT_TRANSACTION_DOMAIN: &[u8] =
    b"crm.customer-privacy.access-export-transaction/v1";
const ACCESS_EXPORT_REQUEST_HASH_DOMAIN: &[u8] = b"crm.customer-privacy.access-export-request/v1";
const ACCESS_EXPORT_EVENT_SCHEMA_ID: &str = "crm.customer-privacy.access_export_reference.event";
const ACCESS_EXPORT_EVENT_SCHEMA_VERSION: &str = "1.0.0";
const ACCESS_EXPORT_EVENT_DESCRIPTOR: &[u8] =
    b"crm.customer-privacy.access_export_reference.event/v1:reference_state";
const ACCESS_EXPORT_PREPARED_EVENT_TYPE: &str =
    "customer_privacy.access_export.internal.reference_prepared";
const ACCESS_EXPORT_COMPLETED_EVENT_TYPE: &str =
    "customer_privacy.access_export.internal.reference_completed";
const ACCESS_EXPORT_IDEMPOTENCY_SCOPE_PREFIX: &str = "customer_privacy.access_export.reference";
const AUDIT_CANONICALIZATION_PROFILE: &str = "crm.cjson/v1";
const AUDIT_LOCK_NAMESPACE: i64 = 0x4352_4d41_5544_4954;

#[derive(Debug, Clone)]
pub struct PostgresAccessExportPersistence {
    store: Arc<PostgresDataStore>,
}

impl PostgresAccessExportPersistence {
    pub fn new(store: Arc<PostgresDataStore>) -> Self {
        Self { store }
    }

    pub fn store(&self) -> &Arc<PostgresDataStore> {
        &self.store
    }
}

impl AccessExportPersistencePort for PostgresAccessExportPersistence {
    fn prepare<'a>(
        &'a self,
        invocation: &'a AccessExportInvocation,
    ) -> PortFuture<'a, Result<AccessExportPreparation, SdkError>> {
        Box::pin(async move {
            let mut transaction = self.store.pool().begin().await.map_err(database_error)?;
            bind_invocation(&mut transaction, invocation).await?;
            let source = load_locked_source(&mut transaction, invocation).await?;
            let manifest = PrivacyAccessExportManifest::build(&source.plan)?;
            let prepared = PrivacyAccessExportReference::prepare(
                manifest,
                invocation.request_started_at_unix_nanos,
            )?;
            let existing = load_reference(
                &mut transaction,
                &invocation.tenant_id,
                prepared.reference_id(),
                true,
            )
            .await?;
            if let Some(existing) = existing {
                if existing.manifest() != prepared.manifest()
                    || existing.export_job_id() != prepared.export_job_id()
                    || existing.target_idempotency_key() != prepared.target_idempotency_key()
                {
                    return Err(access_export_conflict(
                        "durable access export reference conflicts with deterministic source",
                    ));
                }
                let complete = existing.status() == PrivacyAccessExportStatus::Completed;
                transaction.commit().await.map_err(database_error)?;
                return if complete {
                    Ok(AccessExportPreparation::Complete {
                        reference: existing,
                    })
                } else {
                    Ok(AccessExportPreparation::Ready {
                        reference: existing,
                        replayed: true,
                    })
                };
            }
            let business_transaction_id =
                access_export_transaction_id(invocation, "prepared", &prepared);
            bind_business_transaction(&mut transaction, &business_transaction_id).await?;
            insert_reference(
                &mut transaction,
                &prepared,
                invocation,
                &business_transaction_id,
            )
            .await?;
            insert_access_export_transaction_evidence(
                &mut transaction,
                invocation,
                "prepared",
                ACCESS_EXPORT_PREPARED_EVENT_TYPE,
                &prepared,
                1,
                invocation.request_started_at_unix_nanos,
                &business_transaction_id,
            )
            .await?;
            transaction.commit().await.map_err(database_error)?;
            Ok(AccessExportPreparation::Ready {
                reference: prepared,
                replayed: false,
            })
        })
    }

    fn complete<'a>(
        &'a self,
        invocation: &'a AccessExportInvocation,
        prepared: &'a PrivacyAccessExportReference,
        result: &'a PrivacyExportTargetResult,
    ) -> PortFuture<'a, Result<(PrivacyAccessExportReference, bool), SdkError>> {
        Box::pin(async move {
            prepared.validate()?;
            if prepared.status() != PrivacyAccessExportStatus::Prepared
                || prepared.manifest().tenant_id() != &invocation.tenant_id
                || prepared.manifest().privacy_case_id() != &invocation.privacy_case_id
                || prepared.manifest().action_plan_id() != &invocation.action_plan_id
            {
                return Err(access_export_conflict(
                    "completion does not reference the prepared invocation",
                ));
            }
            let mut transaction = self.store.pool().begin().await.map_err(database_error)?;
            bind_invocation(&mut transaction, invocation).await?;
            let source = load_locked_source(&mut transaction, invocation).await?;
            if PrivacyAccessExportManifest::build(&source.plan)? != *prepared.manifest() {
                return Err(access_export_conflict(
                    "immutable access export source changed before completion",
                ));
            }
            let stored = load_reference(
                &mut transaction,
                &invocation.tenant_id,
                prepared.reference_id(),
                true,
            )
            .await?
            .ok_or_else(|| access_export_evidence_invalid("prepared reference is unavailable"))?;
            if stored.status() == PrivacyAccessExportStatus::Completed {
                let mut expected = prepared.clone();
                apply_target_result(&mut expected, result)?;
                if stored != expected {
                    return Err(access_export_conflict(
                        "completed replay conflicts with durable artifact evidence",
                    ));
                }
                transaction.commit().await.map_err(database_error)?;
                return Ok((stored, false));
            }
            if stored != *prepared {
                return Err(access_export_conflict(
                    "durable prepared reference differs from completion input",
                ));
            }
            let mut completed = stored;
            apply_target_result(&mut completed, result)?;
            let business_transaction_id =
                access_export_transaction_id(invocation, "completed", &completed);
            bind_business_transaction(&mut transaction, &business_transaction_id).await?;
            update_reference(
                &mut transaction,
                prepared,
                &completed,
                invocation,
                &business_transaction_id,
            )
            .await?;
            insert_access_export_transaction_evidence(
                &mut transaction,
                invocation,
                "completed",
                ACCESS_EXPORT_COMPLETED_EVENT_TYPE,
                &completed,
                2,
                result.completed_at_unix_nanos,
                &business_transaction_id,
            )
            .await?;
            transaction.commit().await.map_err(database_error)?;
            Ok((completed, true))
        })
    }
}

fn apply_target_result(
    reference: &mut PrivacyAccessExportReference,
    result: &PrivacyExportTargetResult,
) -> Result<(), SdkError> {
    reference.complete(
        &result.export_job_id,
        result.file_id.clone(),
        result.media_type.clone(),
        result.content_sha256,
        result.size_bytes,
        result.retention_policy_id.clone(),
        result.completed_at_unix_nanos,
    )
}

#[derive(Debug)]
struct LockedAccessExportSource {
    plan: PrivacyActionPlan,
}

async fn load_locked_source(
    transaction: &mut Transaction<'_, Postgres>,
    invocation: &AccessExportInvocation,
) -> Result<LockedAccessExportSource, SdkError> {
    let initial_case = load_case(
        transaction,
        &invocation.tenant_id,
        &invocation.privacy_case_id,
        false,
    )
    .await?
    .ok_or_else(case_not_found)?;
    let canonical_party_id = initial_case
        .subject_binding()
        .map(|binding| binding.canonical_party_id.clone())
        .ok_or_else(|| access_export_conflict("privacy case has no verified canonical Party"))?;
    lock_customer_subject(transaction, &invocation.tenant_id, &canonical_party_id).await?;
    let privacy_case = load_case(
        transaction,
        &invocation.tenant_id,
        &invocation.privacy_case_id,
        true,
    )
    .await?
    .ok_or_else(case_not_found)?;
    if privacy_case != initial_case {
        return Err(access_export_conflict(
            "privacy case changed while acquiring the shared subject lock",
        ));
    }
    if privacy_case.status() != PrivacyCaseStatus::Converging
        || privacy_case.action_plan_id() != Some(&invocation.action_plan_id)
    {
        return Err(access_export_conflict(
            "access export requires a converging case with the exact action plan",
        ));
    }
    let plan = load_plan(
        transaction,
        &invocation.tenant_id,
        &invocation.action_plan_id,
    )
    .await?
    .ok_or_else(|| access_export_evidence_invalid("immutable action plan is unavailable"))?;
    validate_case_plan_lineage(&privacy_case, &plan, invocation)?;
    validate_complete_checkpoint(transaction, invocation, &privacy_case, &plan).await?;
    Ok(LockedAccessExportSource { plan })
}

fn validate_case_plan_lineage(
    privacy_case: &PrivacyCase,
    plan: &PrivacyActionPlan,
    invocation: &AccessExportInvocation,
) -> Result<(), SdkError> {
    let binding = privacy_case
        .subject_binding()
        .ok_or_else(|| access_export_evidence_invalid("privacy case subject binding is missing"))?;
    if privacy_case.tenant_id() != &invocation.tenant_id
        || privacy_case.case_id() != &invocation.privacy_case_id
        || plan.plan_id() != &invocation.action_plan_id
        || plan.lineage().tenant_id() != &invocation.tenant_id
        || plan.lineage().privacy_case_id() != &invocation.privacy_case_id
        || plan.lineage().case_kind() != privacy_case.kind()
        || plan.lineage().scope_snapshot_id()
            != privacy_case.scope_snapshot_id().ok_or_else(|| {
                access_export_evidence_invalid("case snapshot reference is missing")
            })?
        || plan.lineage().canonical_party_id() != &binding.canonical_party_id
        || plan.lineage().identity_resolution_generation() != binding.identity_resolution_generation
    {
        return Err(access_export_evidence_invalid(
            "case and action-plan lineage do not match exactly",
        ));
    }
    Ok(())
}

async fn validate_complete_checkpoint(
    transaction: &mut Transaction<'_, Postgres>,
    invocation: &AccessExportInvocation,
    privacy_case: &PrivacyCase,
    plan: &PrivacyActionPlan,
) -> Result<(), SdkError> {
    let row = postgres_sqlx::query(
        r#"
        SELECT source_case_version, executing_case_version, converging_case_version,
               action_plan_id, action_plan_digest, total_items, next_sequence,
               completed_at_unix_nanos
        FROM crm.customer_privacy_owner_execution_checkpoints
        WHERE tenant_id = $1 AND privacy_case_id = $2
        FOR SHARE
        "#,
    )
    .bind(invocation.tenant_id.as_str())
    .bind(invocation.privacy_case_id.as_str())
    .fetch_optional(&mut **transaction)
    .await
    .map_err(database_error)?
    .ok_or_else(|| access_export_evidence_invalid("owner execution checkpoint is unavailable"))?;
    let action_plan_digest = digest_column(&row, "action_plan_digest")?;
    let total_items: i32 = row.try_get("total_items").map_err(database_error)?;
    let next_sequence: i32 = row.try_get("next_sequence").map_err(database_error)?;
    let converging_case_version: Option<i64> = row
        .try_get("converging_case_version")
        .map_err(database_error)?;
    let completed_at: Option<i64> = row
        .try_get("completed_at_unix_nanos")
        .map_err(database_error)?;
    let expected_items = i32::try_from(plan.items().len())
        .map_err(|_| access_export_evidence_invalid("plan item count exceeds PostgreSQL range"))?;
    if row
        .try_get::<String, _>("action_plan_id")
        .map_err(database_error)?
        != plan.plan_id().as_str()
        || action_plan_digest != *plan.digest()
        || total_items != expected_items
        || next_sequence != total_items.saturating_add(1)
        || converging_case_version
            != Some(i64::try_from(privacy_case.version()).map_err(|_| {
                access_export_evidence_invalid("case version exceeds PostgreSQL range")
            })?)
        || completed_at.is_none()
    {
        return Err(access_export_evidence_invalid(
            "owner execution checkpoint is not final or differs from immutable lineage",
        ));
    }
    Ok(())
}

async fn bind_invocation(
    transaction: &mut Transaction<'_, Postgres>,
    invocation: &AccessExportInvocation,
) -> Result<(), SdkError> {
    postgres_sqlx::query(
        r#"
        SELECT set_config('app.tenant_id', $1, true),
               set_config('app.actor_id', $2, true),
               set_config('app.request_id', $3, true),
               set_config('app.capability_id', $4, true),
               set_config('app.capability_version', $5, true)
        "#,
    )
    .bind(invocation.tenant_id.as_str())
    .bind(invocation.actor_id.as_str())
    .bind(invocation.request_id.as_str())
    .bind(invocation.initiating_capability_id.as_str())
    .bind(invocation.initiating_capability_version.as_str())
    .execute(&mut **transaction)
    .await
    .map_err(database_error)?;
    Ok(())
}

async fn bind_business_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    business_transaction_id: &str,
) -> Result<(), SdkError> {
    postgres_sqlx::query("SELECT set_config('app.business_transaction_id', $1, true)")
        .bind(business_transaction_id)
        .execute(&mut **transaction)
        .await
        .map_err(database_error)?;
    Ok(())
}

async fn lock_customer_subject(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: &TenantId,
    canonical_party_id: &RecordId,
) -> Result<(), SdkError> {
    postgres_sqlx::query("SELECT crm.lock_customer_subject($1, $2)")
        .bind(tenant_id.as_str())
        .bind(canonical_party_id.as_str())
        .execute(&mut **transaction)
        .await
        .map_err(database_error)?;
    Ok(())
}

async fn load_case(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: &TenantId,
    case_id: &RecordId,
    for_update: bool,
) -> Result<Option<PrivacyCase>, SdkError> {
    let query = if for_update {
        postgres_sqlx::query(RECORD_SELECT_FOR_UPDATE_SQL)
    } else {
        postgres_sqlx::query(RECORD_SELECT_FOR_SHARE_SQL)
    };
    query
        .bind(tenant_id.as_str())
        .bind(MODULE_ID)
        .bind(PRIVACY_CASE_RECORD_TYPE)
        .bind(case_id.as_str())
        .fetch_optional(&mut **transaction)
        .await
        .map_err(database_error)?
        .map(|row| {
            privacy_case_from_snapshot(&decode_record_snapshot(
                PRIVACY_CASE_RECORD_TYPE,
                case_id.clone(),
                row,
            )?)
        })
        .transpose()
}

async fn load_plan(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: &TenantId,
    plan_id: &RecordId,
) -> Result<Option<PrivacyActionPlan>, SdkError> {
    postgres_sqlx::query(RECORD_SELECT_FOR_SHARE_SQL)
        .bind(tenant_id.as_str())
        .bind(MODULE_ID)
        .bind(ACTION_PLAN_RECORD_TYPE)
        .bind(plan_id.as_str())
        .fetch_optional(&mut **transaction)
        .await
        .map_err(database_error)?
        .map(|row| {
            let snapshot = decode_record_snapshot(ACTION_PLAN_RECORD_TYPE, plan_id.clone(), row)?;
            validate_contract(
                &snapshot,
                ACTION_PLAN_STATE_SCHEMA_ID,
                ACTION_PLAN_STATE_SCHEMA_VERSION,
                action_plan_state_descriptor_hash(),
                ACTION_PLAN_STATE_MAXIMUM_BYTES,
                ACTION_PLAN_STATE_RETENTION_POLICY_ID,
            )?;
            let plan = decode_action_plan_state(&snapshot.payload.bytes)?;
            if plan.plan_id() != plan_id || plan.lineage().tenant_id() != tenant_id {
                return Err(access_export_evidence_invalid(
                    "action plan differs from its persistence envelope",
                ));
            }
            Ok(plan)
        })
        .transpose()
}

async fn load_reference(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: &TenantId,
    reference_id: &RecordId,
    for_update: bool,
) -> Result<Option<PrivacyAccessExportReference>, SdkError> {
    let query = if for_update {
        postgres_sqlx::query(RECORD_SELECT_FOR_UPDATE_SQL)
    } else {
        postgres_sqlx::query(RECORD_SELECT_FOR_SHARE_SQL)
    };
    query
        .bind(tenant_id.as_str())
        .bind(MODULE_ID)
        .bind(ACCESS_EXPORT_REFERENCE_RECORD_TYPE)
        .bind(reference_id.as_str())
        .fetch_optional(&mut **transaction)
        .await
        .map_err(database_error)?
        .map(|row| {
            let snapshot = decode_record_snapshot(
                ACCESS_EXPORT_REFERENCE_RECORD_TYPE,
                reference_id.clone(),
                row,
            )?;
            validate_contract(
                &snapshot,
                ACCESS_EXPORT_STATE_SCHEMA_ID,
                ACCESS_EXPORT_STATE_SCHEMA_VERSION,
                access_export_state_descriptor_hash(),
                ACCESS_EXPORT_STATE_MAXIMUM_BYTES,
                ACCESS_EXPORT_STATE_RETENTION_POLICY_ID,
            )?;
            let reference = decode_access_export_reference(&snapshot.payload.bytes)?;
            if reference.reference_id() != reference_id
                || reference.manifest().tenant_id() != tenant_id
            {
                return Err(access_export_evidence_invalid(
                    "access export reference differs from its persistence envelope",
                ));
            }
            Ok(reference)
        })
        .transpose()
}

async fn insert_reference(
    transaction: &mut Transaction<'_, Postgres>,
    reference: &PrivacyAccessExportReference,
    invocation: &AccessExportInvocation,
    business_transaction_id: &str,
) -> Result<(), SdkError> {
    let payload = encode_access_export_reference(reference)?;
    let result = postgres_sqlx::query(
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
    .bind(invocation.tenant_id.as_str())
    .bind(ACCESS_EXPORT_REFERENCE_RECORD_TYPE)
    .bind(reference.reference_id().as_str())
    .bind(MODULE_ID)
    .bind(ACCESS_EXPORT_STATE_SCHEMA_ID)
    .bind(ACCESS_EXPORT_STATE_SCHEMA_VERSION)
    .bind(access_export_state_descriptor_hash().as_slice())
    .bind(checked_i64(
        ACCESS_EXPORT_STATE_MAXIMUM_BYTES,
        "access export maximum payload size",
    )?)
    .bind(ACCESS_EXPORT_STATE_RETENTION_POLICY_ID)
    .bind(payload)
    .bind(business_transaction_id)
    .execute(&mut **transaction)
    .await
    .map_err(database_error)?;
    if result.rows_affected() != 1 {
        return Err(access_export_conflict(
            "access export reference appeared concurrently",
        ));
    }
    Ok(())
}

async fn update_reference(
    transaction: &mut Transaction<'_, Postgres>,
    prepared: &PrivacyAccessExportReference,
    completed: &PrivacyAccessExportReference,
    invocation: &AccessExportInvocation,
    business_transaction_id: &str,
) -> Result<(), SdkError> {
    let payload = encode_access_export_reference(completed)?;
    let result = postgres_sqlx::query(
        r#"
        UPDATE crm.records
        SET version = 2, schema_id = $5, schema_version = $6,
            descriptor_hash = $7, data_class = 'confidential',
            payload_encoding = 'json', maximum_payload_size = $8,
            retention_policy_id = $9, payload_bytes = $10,
            last_business_transaction_id = $11, updated_at = clock_timestamp()
        WHERE tenant_id = $1 AND owner_module_id = $2
          AND record_type = $3 AND record_id = $4
          AND version = 1 AND deleted_at IS NULL
        "#,
    )
    .bind(invocation.tenant_id.as_str())
    .bind(MODULE_ID)
    .bind(ACCESS_EXPORT_REFERENCE_RECORD_TYPE)
    .bind(prepared.reference_id().as_str())
    .bind(ACCESS_EXPORT_STATE_SCHEMA_ID)
    .bind(ACCESS_EXPORT_STATE_SCHEMA_VERSION)
    .bind(access_export_state_descriptor_hash().as_slice())
    .bind(checked_i64(
        ACCESS_EXPORT_STATE_MAXIMUM_BYTES,
        "access export maximum payload size",
    )?)
    .bind(ACCESS_EXPORT_STATE_RETENTION_POLICY_ID)
    .bind(payload)
    .bind(business_transaction_id)
    .execute(&mut **transaction)
    .await
    .map_err(database_error)?;
    if result.rows_affected() != 1 {
        return Err(access_export_conflict(
            "prepared access export reference changed before completion",
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn insert_access_export_transaction_evidence(
    transaction: &mut Transaction<'_, Postgres>,
    invocation: &AccessExportInvocation,
    phase: &str,
    event_type: &str,
    reference: &PrivacyAccessExportReference,
    aggregate_version: i64,
    occurred_at_unix_nanos: i64,
    business_transaction_id: &str,
) -> Result<(), SdkError> {
    if aggregate_version <= 0 {
        return Err(access_export_evidence_invalid(
            "access export aggregate version must be positive",
        ));
    }
    let payload = encode_access_export_reference(reference)?;
    let request_hash = access_export_request_hash(
        invocation,
        phase,
        reference,
        business_transaction_id,
        &payload,
    );
    let suffix = &hex(&discovery_sha256(business_transaction_id.as_bytes()))[..24];
    let event_id = format!("privacy-access-export-event-{suffix}");
    let audit_id = format!("privacy-access-export-audit-{suffix}");
    let idempotency_scope = format!("{ACCESS_EXPORT_IDEMPOTENCY_SCOPE_PREFIX}.{phase}");

    postgres_sqlx::query(
        r#"
        INSERT INTO crm.idempotency_records (
          tenant_id, idempotency_scope, idempotency_key, request_hash,
          status, business_transaction_id, expires_at
        ) VALUES ($1,$2,$3,$4,'completed',$5,clock_timestamp() + INTERVAL '24 hours')
        "#,
    )
    .bind(invocation.tenant_id.as_str())
    .bind(&idempotency_scope)
    .bind(business_transaction_id)
    .bind(request_hash.as_slice())
    .bind(business_transaction_id)
    .execute(&mut **transaction)
    .await
    .map_err(database_error)?;

    postgres_sqlx::query(
        r#"
        INSERT INTO crm.outbox_events (
          tenant_id, event_id, business_transaction_id,
          aggregate_type, aggregate_id, aggregate_version, event_sequence,
          event_type, deduplication_key, schema_id, schema_version, descriptor_hash,
          data_class, payload_encoding, maximum_payload_size, retention_policy_id,
          payload_bytes, occurred_at
        ) VALUES (
          $1,$2,$3,$4,$5,$6,$6,$7,$8,$9,$10,$11,
          'confidential','json',$12,$13,$14,
          TIMESTAMPTZ 'epoch' + ($15::bigint / 1000) * INTERVAL '1 microsecond'
        )
        "#,
    )
    .bind(invocation.tenant_id.as_str())
    .bind(&event_id)
    .bind(business_transaction_id)
    .bind(ACCESS_EXPORT_REFERENCE_RECORD_TYPE)
    .bind(reference.reference_id().as_str())
    .bind(aggregate_version)
    .bind(event_type)
    .bind(&event_id)
    .bind(ACCESS_EXPORT_EVENT_SCHEMA_ID)
    .bind(ACCESS_EXPORT_EVENT_SCHEMA_VERSION)
    .bind(discovery_sha256(ACCESS_EXPORT_EVENT_DESCRIPTOR).as_slice())
    .bind(checked_i64(
        ACCESS_EXPORT_STATE_MAXIMUM_BYTES,
        "access export event maximum payload size",
    )?)
    .bind(ACCESS_EXPORT_STATE_RETENTION_POLICY_ID)
    .bind(payload.as_slice())
    .bind(occurred_at_unix_nanos)
    .execute(&mut **transaction)
    .await
    .map_err(database_error)?;

    let _audit_lock =
        postgres_sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, $2))")
            .bind(invocation.tenant_id.as_str())
            .bind(AUDIT_LOCK_NAMESPACE)
            .fetch_one(&mut **transaction)
            .await
            .map_err(database_error)?;
    let head = postgres_sqlx::query(
        "SELECT next_sequence, last_hash FROM crm.audit_heads WHERE tenant_id = $1",
    )
    .bind(invocation.tenant_id.as_str())
    .fetch_optional(&mut **transaction)
    .await
    .map_err(database_error)?;
    let (sequence, previous_hash) = match head {
        Some(row) => {
            let sequence: i64 = row.try_get("next_sequence").map_err(database_error)?;
            if sequence <= 0 {
                return Err(access_export_evidence_invalid(
                    "tenant audit next sequence must be positive",
                ));
            }
            let previous_hash = row
                .try_get::<Vec<u8>, _>("last_hash")
                .map_err(database_error)?
                .try_into()
                .map_err(|_| {
                    access_export_evidence_invalid("tenant audit hash must contain 32 bytes")
                })?;
            (sequence, previous_hash)
        }
        None => (1, [0; 32]),
    };
    let occurred_at = (occurred_at_unix_nanos / 1_000) * 1_000;
    let audit_hash = access_export_transaction_audit_hash(
        invocation,
        sequence,
        previous_hash,
        &audit_id,
        business_transaction_id,
        &payload,
        occurred_at,
    );
    postgres_sqlx::query(
        r#"
        INSERT INTO crm.audit_records (
          tenant_id, audit_sequence, audit_record_id, business_transaction_id,
          actor_id, capability_id, capability_version, canonicalization_profile,
          previous_hash, record_hash, canonical_envelope, occurred_at
        ) VALUES (
          $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,
          TIMESTAMPTZ 'epoch' + ($12::bigint / 1000) * INTERVAL '1 microsecond'
        )
        "#,
    )
    .bind(invocation.tenant_id.as_str())
    .bind(sequence)
    .bind(&audit_id)
    .bind(business_transaction_id)
    .bind(invocation.actor_id.as_str())
    .bind(invocation.initiating_capability_id.as_str())
    .bind(invocation.initiating_capability_version.as_str())
    .bind(AUDIT_CANONICALIZATION_PROFILE)
    .bind(previous_hash.as_slice())
    .bind(audit_hash.as_slice())
    .bind(payload.as_slice())
    .bind(occurred_at)
    .execute(&mut **transaction)
    .await
    .map_err(database_error)?;

    postgres_sqlx::query(
        r#"
        INSERT INTO crm.business_transactions (
          tenant_id, business_transaction_id, actor_id, request_id,
          correlation_id, trace_id, capability_id, capability_version,
          expected_outbox_events, expected_audit_records, expected_idempotency_records
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,1,1,1)
        "#,
    )
    .bind(invocation.tenant_id.as_str())
    .bind(business_transaction_id)
    .bind(invocation.actor_id.as_str())
    .bind(invocation.request_id.as_str())
    .bind(invocation.correlation_id.as_str())
    .bind(invocation.trace_id.as_str())
    .bind(invocation.initiating_capability_id.as_str())
    .bind(invocation.initiating_capability_version.as_str())
    .execute(&mut **transaction)
    .await
    .map_err(database_error)?;
    Ok(())
}

fn access_export_transaction_id(
    invocation: &AccessExportInvocation,
    phase: &str,
    reference: &PrivacyAccessExportReference,
) -> String {
    let mut bytes = Vec::new();
    for field in [
        ACCESS_EXPORT_TRANSACTION_DOMAIN,
        invocation.tenant_id.as_str().as_bytes(),
        invocation.privacy_case_id.as_str().as_bytes(),
        invocation.action_plan_id.as_str().as_bytes(),
        phase.as_bytes(),
        reference.reference_id().as_str().as_bytes(),
        reference.digest().as_slice(),
    ] {
        append_digest_field(&mut bytes, field);
    }
    let digest = discovery_sha256(&bytes);
    format!("privacy-access-export-{phase}-{}", hex(&digest[..12]))
}

fn access_export_request_hash(
    invocation: &AccessExportInvocation,
    phase: &str,
    reference: &PrivacyAccessExportReference,
    business_transaction_id: &str,
    payload: &[u8],
) -> [u8; 32] {
    let mut bytes = Vec::new();
    for field in [
        ACCESS_EXPORT_REQUEST_HASH_DOMAIN,
        invocation.tenant_id.as_str().as_bytes(),
        invocation.privacy_case_id.as_str().as_bytes(),
        invocation.action_plan_id.as_str().as_bytes(),
        phase.as_bytes(),
        reference.reference_id().as_str().as_bytes(),
        reference.digest().as_slice(),
        business_transaction_id.as_bytes(),
        payload,
    ] {
        append_digest_field(&mut bytes, field);
    }
    discovery_sha256(&bytes)
}

fn access_export_transaction_audit_hash(
    invocation: &AccessExportInvocation,
    sequence: i64,
    previous_hash: [u8; 32],
    audit_id: &str,
    business_transaction_id: &str,
    canonical_envelope: &[u8],
    occurred_at_unix_nanos: i64,
) -> [u8; 32] {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"crm.audit.record.sha256/v1");
    append_digest_field(&mut bytes, invocation.tenant_id.as_str().as_bytes());
    bytes.extend_from_slice(&sequence.to_be_bytes());
    for field in [
        audit_id.as_bytes(),
        business_transaction_id.as_bytes(),
        invocation.actor_id.as_str().as_bytes(),
        invocation.initiating_capability_id.as_str().as_bytes(),
        invocation.initiating_capability_version.as_str().as_bytes(),
        AUDIT_CANONICALIZATION_PROFILE.as_bytes(),
    ] {
        append_digest_field(&mut bytes, field);
    }
    bytes.extend_from_slice(&previous_hash);
    append_digest_field(&mut bytes, canonical_envelope);
    bytes.extend_from_slice(&occurred_at_unix_nanos.to_be_bytes());
    discovery_sha256(&bytes)
}

fn decode_record_snapshot(
    record_type: &str,
    record_id: RecordId,
    row: postgres_sqlx::postgres::PgRow,
) -> Result<RecordSnapshot, SdkError> {
    let version = row.try_get("version").map_err(database_error)?;
    let data_class: String = row.try_get("data_class").map_err(database_error)?;
    let encoding: String = row.try_get("payload_encoding").map_err(database_error)?;
    if data_class != "confidential" || encoding != "json" {
        return Err(access_export_evidence_invalid(
            "record data class or encoding differs from its governed contract",
        ));
    }
    let descriptor: Vec<u8> = row.try_get("descriptor_hash").map_err(database_error)?;
    let descriptor_hash: [u8; 32] = descriptor.try_into().map_err(|_| {
        access_export_evidence_invalid("record descriptor hash must contain 32 bytes")
    })?;
    let maximum: i64 = row
        .try_get("maximum_payload_size")
        .map_err(database_error)?;
    Ok(RecordSnapshot {
        reference: RecordRef {
            record_type: RecordType::try_new(record_type)
                .map_err(access_export_evidence_invalid)?,
            record_id,
        },
        version,
        payload: TypedPayload {
            owner: ModuleId::try_new(
                row.try_get::<String, _>("owner_module_id")
                    .map_err(database_error)?,
            )
            .map_err(access_export_evidence_invalid)?,
            schema_id: SchemaId::try_new(
                row.try_get::<String, _>("schema_id")
                    .map_err(database_error)?,
            )
            .map_err(access_export_evidence_invalid)?,
            schema_version: SchemaVersion::try_new(
                row.try_get::<String, _>("schema_version")
                    .map_err(database_error)?,
            )
            .map_err(access_export_evidence_invalid)?,
            descriptor_hash,
            data_class: DataClass::Confidential,
            encoding: PayloadEncoding::Json,
            maximum_size_bytes: u64::try_from(maximum)
                .map_err(|_| access_export_evidence_invalid("record maximum size is negative"))?,
            retention_policy_id: RetentionPolicyId::try_new(
                row.try_get::<String, _>("retention_policy_id")
                    .map_err(database_error)?,
            )
            .map_err(access_export_evidence_invalid)?,
            bytes: row.try_get("payload_bytes").map_err(database_error)?,
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
    if snapshot.payload.owner.as_str() != MODULE_ID
        || snapshot.payload.schema_id.as_str() != schema_id
        || snapshot.payload.schema_version.as_str() != schema_version
        || snapshot.payload.descriptor_hash != descriptor_hash
        || snapshot.payload.maximum_size_bytes != maximum_size_bytes
        || snapshot.payload.retention_policy_id.as_str() != retention_policy_id
        || snapshot.payload.validate().is_err()
    {
        return Err(access_export_evidence_invalid(
            "access export evidence envelope differs from its governed contract",
        ));
    }
    Ok(())
}

fn digest_column(row: &postgres_sqlx::postgres::PgRow, column: &str) -> Result<[u8; 32], SdkError> {
    row.try_get::<Vec<u8>, _>(column)
        .map_err(database_error)?
        .try_into()
        .map_err(|_| access_export_evidence_invalid(format!("{column} must contain 32 bytes")))
}

fn append_digest_field(bytes: &mut Vec<u8>, field: &[u8]) {
    bytes.extend_from_slice(&(field.len() as u64).to_be_bytes());
    bytes.extend_from_slice(field);
}

fn checked_i64(value: u64, label: &str) -> Result<i64, SdkError> {
    i64::try_from(value).map_err(|_| access_export_evidence_invalid(format!("{label} exceeds i64")))
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

const RECORD_SELECT_FOR_SHARE_SQL: &str = r#"
SELECT version, owner_module_id, schema_id, schema_version,
       descriptor_hash, data_class, payload_encoding,
       maximum_payload_size, retention_policy_id, payload_bytes
FROM crm.records
WHERE tenant_id = $1 AND owner_module_id = $2
  AND record_type = $3 AND record_id = $4 AND deleted_at IS NULL
FOR SHARE
"#;

const RECORD_SELECT_FOR_UPDATE_SQL: &str = r#"
SELECT version, owner_module_id, schema_id, schema_version,
       descriptor_hash, data_class, payload_encoding,
       maximum_payload_size, retention_policy_id, payload_bytes
FROM crm.records
WHERE tenant_id = $1 AND owner_module_id = $2
  AND record_type = $3 AND record_id = $4 AND deleted_at IS NULL
FOR UPDATE
"#;

fn database_error(reference: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_ACCESS_EXPORT_STORE_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "Customer Privacy access export storage is unavailable.",
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

fn access_export_conflict(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_ACCESS_EXPORT_CONFLICT",
        ErrorCategory::Conflict,
        false,
        "Customer Privacy access export conflicts with immutable evidence.",
    )
    .with_internal_reference(reference.into())
}

fn access_export_evidence_invalid(reference: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_ACCESS_EXPORT_EVIDENCE_INVALID",
        ErrorCategory::Internal,
        false,
        "Customer Privacy access export evidence is invalid.",
    )
    .with_internal_reference(reference.to_string())
}
