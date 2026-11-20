use crate::execution::outcome_from_row;
use crm_core_data::{
    PostgresDataStore,
    postgres_sqlx::{self, Postgres, Row, Transaction},
};
use crm_customer_privacy::{
    ACTION_PLAN_RECORD_TYPE, ACTION_PLAN_STATE_MAXIMUM_BYTES,
    ACTION_PLAN_STATE_RETENTION_POLICY_ID, ACTION_PLAN_STATE_SCHEMA_ID,
    ACTION_PLAN_STATE_SCHEMA_VERSION, DISCOVERY_SCOPE_SNAPSHOT_STATE_MAXIMUM_BYTES,
    DISCOVERY_SCOPE_SNAPSHOT_STATE_RETENTION_POLICY_ID, DISCOVERY_SCOPE_SNAPSHOT_STATE_SCHEMA_ID,
    DISCOVERY_SCOPE_SNAPSHOT_STATE_SCHEMA_VERSION, DiscoveryScopeSnapshot, MODULE_ID,
    PRIVACY_CASE_RECORD_TYPE, PrivacyActionPlan, PrivacyCase, SCOPE_SNAPSHOT_RECORD_TYPE,
    action_plan_state_descriptor_hash, decode_action_plan_state,
    decode_discovery_scope_snapshot_state, discovery_scope_snapshot_state_descriptor_hash,
    discovery_sha256,
};
use crm_customer_privacy_application::{
    PrivacyOwnerOutcomePage, PrivacyOwnerOutcomePosition, PrivacyPlanReadSource,
    PrivacyPlanReplayLink, PrivacyReadAuditRecord, PrivacyReadContext, PrivacyReadPersistencePort,
};
use crm_customer_privacy_persistence_adapter::privacy_case_from_snapshot;
use crm_module_sdk::{
    DataClass, ErrorCategory, ModuleId, PayloadEncoding, PortFuture, RecordId, RecordRef,
    RecordSnapshot, RecordType, RetentionPolicyId, SchemaId, SchemaVersion, SdkError, TenantId,
    TypedPayload,
};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct PostgresPrivacyReadPersistence {
    store: Arc<PostgresDataStore>,
}

impl PostgresPrivacyReadPersistence {
    pub fn new(store: Arc<PostgresDataStore>) -> Self {
        Self { store }
    }

    pub fn store(&self) -> &Arc<PostgresDataStore> {
        &self.store
    }
}

impl PrivacyReadPersistencePort for PostgresPrivacyReadPersistence {
    fn load_plan_source<'a>(
        &'a self,
        context: &'a PrivacyReadContext,
        privacy_case_id: &'a RecordId,
    ) -> PortFuture<'a, Result<Option<PrivacyPlanReadSource>, SdkError>> {
        Box::pin(async move {
            let mut transaction = self.store.pool().begin().await.map_err(database_error)?;
            bind_context(&mut transaction, context).await?;
            let privacy_case =
                match load_case(&mut transaction, &context.tenant_id, privacy_case_id).await? {
                    Some(value) => value,
                    None => {
                        transaction.commit().await.map_err(database_error)?;
                        return Ok(None);
                    }
                };
            let snapshot_id = privacy_case.scope_snapshot_id().ok_or_else(|| {
                evidence_invalid("privacy case has no immutable scope snapshot reference")
            })?;
            let plan_id = privacy_case.action_plan_id().ok_or_else(|| {
                evidence_invalid("privacy case has no immutable action plan reference")
            })?;
            let scope_snapshot = load_snapshot(&mut transaction, &context.tenant_id, snapshot_id)
                .await?
                .ok_or_else(|| evidence_invalid("referenced scope snapshot is missing"))?;
            let action_plan = load_plan(&mut transaction, &context.tenant_id, plan_id)
                .await?
                .ok_or_else(|| evidence_invalid("referenced action plan is missing"))?;
            let replay_link =
                load_replay_link(&mut transaction, &context.tenant_id, privacy_case_id)
                    .await?
                    .ok_or_else(|| evidence_invalid("durable case-plan replay link is missing"))?;
            transaction.commit().await.map_err(database_error)?;
            Ok(Some(PrivacyPlanReadSource {
                privacy_case,
                scope_snapshot,
                action_plan,
                replay_link,
            }))
        })
    }

    fn load_owner_outcomes<'a>(
        &'a self,
        context: &'a PrivacyReadContext,
        privacy_case_id: &'a RecordId,
        action_plan_id: &'a RecordId,
        owner_module_filter: Option<&'a ModuleId>,
        after: Option<&'a PrivacyOwnerOutcomePosition>,
        page_size: u32,
    ) -> PortFuture<'a, Result<PrivacyOwnerOutcomePage, SdkError>> {
        Box::pin(async move {
            let mut transaction = self.store.pool().begin().await.map_err(database_error)?;
            bind_context(&mut transaction, context).await?;
            let after_sequence = after
                .map(|value| i32::try_from(value.item_sequence))
                .transpose()
                .map_err(|_| {
                    evidence_invalid("outcome cursor sequence exceeds PostgreSQL range")
                })?;
            let after_generation = after
                .map(|value| i32::try_from(value.attempt_generation))
                .transpose()
                .map_err(|_| {
                    evidence_invalid("outcome cursor generation exceeds PostgreSQL range")
                })?;
            let limit = page_size
                .checked_add(1)
                .ok_or_else(|| evidence_invalid("outcome page size overflowed"))?;
            let rows = postgres_sqlx::query(
                r#"
                SELECT outcome_id, payload_bytes AS outcome_payload,
                       schema_id AS outcome_schema_id,
                       schema_version AS outcome_schema_version,
                       descriptor_hash AS outcome_descriptor_hash,
                       maximum_payload_size AS outcome_maximum,
                       retention_policy_id AS outcome_retention
                FROM crm.customer_privacy_owner_action_outcomes
                WHERE tenant_id = $1 AND privacy_case_id = $2
                  AND action_plan_id = $3
                  AND ($4::text IS NULL OR owner_module_id = $4)
                  AND (
                    $5::integer IS NULL
                    OR (item_sequence, attempt_generation, outcome_id)
                       > ($5, $6, $7)
                  )
                ORDER BY item_sequence, attempt_generation, outcome_id
                LIMIT $8
                "#,
            )
            .bind(context.tenant_id.as_str())
            .bind(privacy_case_id.as_str())
            .bind(action_plan_id.as_str())
            .bind(owner_module_filter.map(ModuleId::as_str))
            .bind(after_sequence)
            .bind(after_generation)
            .bind(after.map(|value| value.outcome_id.as_str()))
            .bind(i64::from(limit))
            .fetch_all(&mut *transaction)
            .await
            .map_err(database_error)?;
            let page_size_usize = usize::try_from(page_size)
                .map_err(|_| evidence_invalid("outcome page size exceeds usize"))?;
            let has_more = rows.len() > page_size_usize;
            let outcomes = rows
                .into_iter()
                .take(page_size_usize)
                .map(|row| outcome_from_row(&row))
                .collect::<Result<Vec<_>, _>>()?;
            transaction.commit().await.map_err(database_error)?;
            Ok(PrivacyOwnerOutcomePage { outcomes, has_more })
        })
    }

    fn append_read_audit<'a>(
        &'a self,
        record: &'a PrivacyReadAuditRecord,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            let mut transaction = self.store.pool().begin().await.map_err(database_error)?;
            bind_context(&mut transaction, &record.context).await?;
            let audit_digest = read_audit_digest(record);
            let page_size = record
                .page_size
                .map(i32::try_from)
                .transpose()
                .map_err(|_| evidence_invalid("read audit page size exceeds PostgreSQL range"))?;
            postgres_sqlx::query(
                r#"
                INSERT INTO crm.customer_privacy_plan_read_audit (
                  tenant_id, audit_digest, capability_id, privacy_case_id,
                  plan_id, plan_digest, owner_module_filter, page_size,
                  page_digest, terminal_digest, authorization_digest,
                  allowed, result_code, actor_id, request_id, correlation_id,
                  trace_id, occurred_at_unix_nanos
                ) VALUES (
                  $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18
                )
                ON CONFLICT DO NOTHING
                "#,
            )
            .bind(record.context.tenant_id.as_str())
            .bind(audit_digest.as_slice())
            .bind(record.context.capability_id.as_str())
            .bind(record.privacy_case_id.as_str())
            .bind(record.plan_id.as_ref().map(RecordId::as_str))
            .bind(record.plan_digest.as_ref().map(|value| value.as_slice()))
            .bind(record.owner_module_filter.as_ref().map(ModuleId::as_str))
            .bind(page_size)
            .bind(record.page_digest.as_ref().map(|value| value.as_slice()))
            .bind(
                record
                    .terminal_digest
                    .as_ref()
                    .map(|value| value.as_slice()),
            )
            .bind(record.authorization_digest.as_slice())
            .bind(record.allowed)
            .bind(record.result_code)
            .bind(record.context.actor_id.as_str())
            .bind(record.context.request_id.as_str())
            .bind(record.context.correlation_id.as_str())
            .bind(record.context.trace_id.as_str())
            .bind(record.context.request_started_at_unix_nanos)
            .execute(&mut *transaction)
            .await
            .map_err(database_error)?;
            transaction.commit().await.map_err(database_error)?;
            Ok(())
        })
    }
}

async fn load_case(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: &TenantId,
    case_id: &RecordId,
) -> Result<Option<PrivacyCase>, SdkError> {
    select_record(transaction, tenant_id, PRIVACY_CASE_RECORD_TYPE, case_id)
        .await?
        .map(|row| {
            let snapshot = decode_record_snapshot(PRIVACY_CASE_RECORD_TYPE, case_id.clone(), row)?;
            privacy_case_from_snapshot(&snapshot).map_err(evidence_invalid)
        })
        .transpose()
}

async fn load_snapshot(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: &TenantId,
    snapshot_id: &RecordId,
) -> Result<Option<DiscoveryScopeSnapshot>, SdkError> {
    select_record(
        transaction,
        tenant_id,
        SCOPE_SNAPSHOT_RECORD_TYPE,
        snapshot_id,
    )
    .await?
    .map(|row| decode_snapshot_row(tenant_id, snapshot_id, row))
    .transpose()
}

async fn load_plan(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: &TenantId,
    plan_id: &RecordId,
) -> Result<Option<PrivacyActionPlan>, SdkError> {
    select_record(transaction, tenant_id, ACTION_PLAN_RECORD_TYPE, plan_id)
        .await?
        .map(|row| decode_plan_row(tenant_id, plan_id, row))
        .transpose()
}

async fn select_record(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: &TenantId,
    record_type: &str,
    record_id: &RecordId,
) -> Result<Option<postgres_sqlx::postgres::PgRow>, SdkError> {
    postgres_sqlx::query(
        r#"
        SELECT version, owner_module_id, schema_id, schema_version, descriptor_hash,
               data_class, payload_encoding, maximum_payload_size, retention_policy_id,
               payload_bytes
        FROM crm.records
        WHERE tenant_id = $1 AND owner_module_id = $2
          AND record_type = $3 AND record_id = $4 AND deleted_at IS NULL
        FOR SHARE
        "#,
    )
    .bind(tenant_id.as_str())
    .bind(MODULE_ID)
    .bind(record_type)
    .bind(record_id.as_str())
    .fetch_optional(&mut **transaction)
    .await
    .map_err(database_error)
}

async fn load_replay_link(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: &TenantId,
    privacy_case_id: &RecordId,
) -> Result<Option<PrivacyPlanReplayLink>, SdkError> {
    postgres_sqlx::query(
        r#"
        SELECT source_case_version, resulting_case_version, scope_snapshot_id,
               plan_id, plan_digest, approval_required,
               (extract(epoch FROM planned_at) * 1000000000)::bigint AS planned_at_unix_nanos
        FROM crm.customer_privacy_action_plans
        WHERE tenant_id = $1 AND privacy_case_id = $2
        FOR SHARE
        "#,
    )
    .bind(tenant_id.as_str())
    .bind(privacy_case_id.as_str())
    .fetch_optional(&mut **transaction)
    .await
    .map_err(database_error)?
    .map(|row| {
        Ok(PrivacyPlanReplayLink {
            source_case_version: positive_u64(&row, "source_case_version")?,
            resulting_case_version: positive_u64(&row, "resulting_case_version")?,
            scope_snapshot_id: RecordId::try_new(
                row.try_get::<String, _>("scope_snapshot_id")
                    .map_err(database_error)?,
            )
            .map_err(evidence_invalid)?,
            plan_id: RecordId::try_new(
                row.try_get::<String, _>("plan_id")
                    .map_err(database_error)?,
            )
            .map_err(evidence_invalid)?,
            plan_digest: digest_column(&row, "plan_digest")?,
            approval_required: row.try_get("approval_required").map_err(database_error)?,
            planned_at_unix_nanos: row
                .try_get("planned_at_unix_nanos")
                .map_err(database_error)?,
        })
    })
    .transpose()
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
    let value =
        decode_discovery_scope_snapshot_state(&snapshot.payload.bytes).map_err(evidence_invalid)?;
    if value.snapshot_id() != snapshot_id || value.lineage().tenant_id() != tenant_id {
        return Err(evidence_invalid(
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
    let value = decode_action_plan_state(&snapshot.payload.bytes).map_err(evidence_invalid)?;
    if value.plan_id() != plan_id || value.lineage().tenant_id() != tenant_id {
        return Err(evidence_invalid(
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
    let version: i64 = row.try_get("version").map_err(evidence_invalid)?;
    let owner: String = row.try_get("owner_module_id").map_err(evidence_invalid)?;
    let schema_id: String = row.try_get("schema_id").map_err(evidence_invalid)?;
    let schema_version: String = row.try_get("schema_version").map_err(evidence_invalid)?;
    let descriptor_hash: Vec<u8> = row.try_get("descriptor_hash").map_err(evidence_invalid)?;
    let data_class: String = row.try_get("data_class").map_err(evidence_invalid)?;
    let encoding: String = row.try_get("payload_encoding").map_err(evidence_invalid)?;
    let maximum: i64 = row
        .try_get("maximum_payload_size")
        .map_err(evidence_invalid)?;
    let retention: String = row
        .try_get("retention_policy_id")
        .map_err(evidence_invalid)?;
    let bytes: Vec<u8> = row.try_get("payload_bytes").map_err(evidence_invalid)?;
    if data_class != "confidential" || encoding != "json" {
        return Err(evidence_invalid(
            "read source data class or encoding differs from its contract",
        ));
    }
    Ok(RecordSnapshot {
        reference: RecordRef {
            record_type: RecordType::try_new(record_type).map_err(evidence_invalid)?,
            record_id,
        },
        version,
        payload: TypedPayload {
            owner: ModuleId::try_new(owner).map_err(evidence_invalid)?,
            schema_id: SchemaId::try_new(schema_id).map_err(evidence_invalid)?,
            schema_version: SchemaVersion::try_new(schema_version).map_err(evidence_invalid)?,
            descriptor_hash: descriptor_hash.try_into().map_err(|_| {
                evidence_invalid("read source descriptor hash must contain exactly 32 bytes")
            })?,
            data_class: DataClass::Confidential,
            encoding: PayloadEncoding::Json,
            maximum_size_bytes: u64::try_from(maximum)
                .map_err(|_| evidence_invalid("read source maximum size is negative"))?,
            retention_policy_id: RetentionPolicyId::try_new(retention).map_err(evidence_invalid)?,
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
        || snapshot.payload.validate().is_err()
    {
        return Err(evidence_invalid("read evidence record envelope drifted"));
    }
    Ok(())
}

async fn bind_context(
    transaction: &mut Transaction<'_, Postgres>,
    context: &PrivacyReadContext,
) -> Result<(), SdkError> {
    postgres_sqlx::query(
        r#"
        SELECT set_config('app.tenant_id', $1, true),
               set_config('app.actor_id', $2, true),
               set_config('app.request_id', $3, true),
               set_config('app.capability_id', $4, true),
               set_config('app.capability_version', $5, true),
               set_config('app.business_transaction_id', $6, true)
        "#,
    )
    .bind(context.tenant_id.as_str())
    .bind(context.actor_id.as_str())
    .bind(context.request_id.as_str())
    .bind(context.capability_id.as_str())
    .bind(context.capability_version.as_str())
    .bind(read_transaction_id(context))
    .execute(&mut **transaction)
    .await
    .map_err(database_error)?;
    Ok(())
}

fn read_transaction_id(context: &PrivacyReadContext) -> String {
    let digest = discovery_sha256(context.request_id.as_str().as_bytes());
    format!("privacy-read-{}", hex(&digest[..12]))
}

fn read_audit_digest(record: &PrivacyReadAuditRecord) -> [u8; 32] {
    let page_size = record.page_size.map(|value| value.to_string());
    let occurred_at = record.context.request_started_at_unix_nanos.to_string();
    let decision = if record.allowed {
        b"allow".as_slice()
    } else {
        b"deny".as_slice()
    };
    let mut bytes = Vec::new();
    for value in [
        b"crm.customer-privacy.plan-read-audit/v1".as_slice(),
        record.context.tenant_id.as_str().as_bytes(),
        record.context.capability_id.as_str().as_bytes(),
        record.privacy_case_id.as_str().as_bytes(),
        record
            .plan_id
            .as_ref()
            .map(RecordId::as_str)
            .unwrap_or("")
            .as_bytes(),
        record
            .owner_module_filter
            .as_ref()
            .map(ModuleId::as_str)
            .unwrap_or("")
            .as_bytes(),
        page_size.as_deref().unwrap_or("").as_bytes(),
        record.authorization_digest.as_slice(),
        decision,
        record.result_code.as_bytes(),
        record.context.actor_id.as_str().as_bytes(),
        record.context.request_id.as_str().as_bytes(),
        occurred_at.as_bytes(),
    ] {
        append_field(&mut bytes, value);
    }
    for digest in [
        record.plan_digest.as_ref(),
        record.page_digest.as_ref(),
        record.terminal_digest.as_ref(),
    ] {
        append_field(
            &mut bytes,
            digest.map(|value| value.as_slice()).unwrap_or(&[]),
        );
    }
    discovery_sha256(&bytes)
}

fn append_field(output: &mut Vec<u8>, value: &[u8]) {
    output.extend_from_slice(&(value.len() as u64).to_be_bytes());
    output.extend_from_slice(value);
}

fn positive_u64(row: &postgres_sqlx::postgres::PgRow, column: &str) -> Result<u64, SdkError> {
    let value: i64 = row.try_get(column).map_err(database_error)?;
    u64::try_from(value).map_err(|_| evidence_invalid(format!("{column} is negative")))
}

fn digest_column(row: &postgres_sqlx::postgres::PgRow, column: &str) -> Result<[u8; 32], SdkError> {
    let value: Vec<u8> = row.try_get(column).map_err(database_error)?;
    value
        .try_into()
        .map_err(|_| evidence_invalid(format!("{column} must contain exactly 32 bytes")))
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
        "CUSTOMER_PRIVACY_READ_STORE_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "Customer Privacy read storage is unavailable.",
    )
    .with_internal_reference(reference.to_string())
}

fn evidence_invalid(reference: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_READ_EVIDENCE_INVALID",
        ErrorCategory::Internal,
        false,
        "Customer Privacy read evidence is invalid.",
    )
    .with_internal_reference(reference.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_transaction_and_audit_identity_are_deterministic() {
        let context = PrivacyReadContext {
            tenant_id: TenantId::try_new("tenant-a").unwrap(),
            actor_id: crm_module_sdk::ActorId::try_new("actor-a").unwrap(),
            request_id: crm_module_sdk::RequestId::try_new("request-a").unwrap(),
            correlation_id: crm_module_sdk::CorrelationId::try_new("correlation-a").unwrap(),
            trace_id: crm_module_sdk::TraceId::try_new("trace-a").unwrap(),
            capability_id: crm_module_sdk::CapabilityId::try_new("customer_privacy.case.plan.get")
                .unwrap(),
            capability_version: crm_module_sdk::CapabilityVersion::try_new("1.0.0").unwrap(),
            request_started_at_unix_nanos: 1_000,
        };
        let record = PrivacyReadAuditRecord {
            context: context.clone(),
            privacy_case_id: RecordId::try_new("case-a").unwrap(),
            plan_id: None,
            plan_digest: None,
            owner_module_filter: None,
            page_size: None,
            page_digest: None,
            terminal_digest: None,
            authorization_digest: [7; 32],
            allowed: false,
            result_code: "source_not_found",
        };
        assert_eq!(read_transaction_id(&context), read_transaction_id(&context));
        assert_eq!(read_audit_digest(&record), read_audit_digest(&record));
    }
}
