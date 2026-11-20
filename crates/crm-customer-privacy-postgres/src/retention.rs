use crm_core_data::{
    PostgresDataStore,
    postgres_sqlx::{self, Postgres, Row, Transaction},
};
use crm_customer_privacy::{
    ACTION_PLAN_RECORD_TYPE, ACTION_PLAN_STATE_MAXIMUM_BYTES,
    ACTION_PLAN_STATE_RETENTION_POLICY_ID, ACTION_PLAN_STATE_SCHEMA_ID,
    ACTION_PLAN_STATE_SCHEMA_VERSION, CustomerDataLegalHold, LEGAL_HOLD_RECORD_TYPE, MODULE_ID,
    PRIVACY_CASE_RECORD_TYPE, PrivacyActionPlan, PrivacyCase, PrivacyCaseStatus,
    PrivacyRetentionDecisionSet, RETENTION_DECISION_MAXIMUM_HOLDS, RETENTION_DECISION_RECORD_TYPE,
    RETENTION_EVALUATE_COORDINATE, action_plan_state_descriptor_hash, decode_action_plan_state,
    discovery_sha256,
};
use crm_customer_privacy_application::{
    RETENTION_APPROVAL_TRIGGER_CAPABILITY, RETENTION_LEGAL_HOLD_TRIGGER_CAPABILITY,
    RETENTION_TRIGGER_CAPABILITY_VERSION, RetentionEvaluationCommit, RetentionEvaluationInvocation,
    RetentionEvaluationPersistencePort,
};
use crm_customer_privacy_persistence_adapter::{
    legal_hold_from_snapshot, privacy_case_from_snapshot, retention_decision_from_snapshot,
    retention_decision_persisted_payload,
};
use crm_module_sdk::{
    DataClass, ErrorCategory, ModuleId, PayloadEncoding, PortFuture, RecordId, RecordRef,
    RecordSnapshot, RecordType, RetentionPolicyId, SchemaId, SchemaVersion, SdkError, TenantId,
    TypedPayload,
};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct PostgresRetentionEvaluationPersistence {
    store: Arc<PostgresDataStore>,
}

impl PostgresRetentionEvaluationPersistence {
    pub fn new(store: Arc<PostgresDataStore>) -> Self {
        Self { store }
    }

    pub fn store(&self) -> &Arc<PostgresDataStore> {
        &self.store
    }
}

impl RetentionEvaluationPersistencePort for PostgresRetentionEvaluationPersistence {
    fn evaluate_and_persist<'a>(
        &'a self,
        invocation: &'a RetentionEvaluationInvocation,
    ) -> PortFuture<'a, Result<RetentionEvaluationCommit, SdkError>> {
        Box::pin(async move {
            let mut transaction = self.store.pool().begin().await.map_err(database_error)?;
            bind_invocation(&mut transaction, invocation).await?;

            let initial_case = load_case(
                &mut transaction,
                &invocation.tenant_id,
                &invocation.privacy_case_id,
                RowLock::Share,
            )
            .await?
            .ok_or_else(case_not_found)?;
            let canonical_party_id = initial_case
                .subject_binding()
                .map(|binding| binding.canonical_party_id.clone())
                .ok_or_else(|| {
                    retention_conflict("privacy case has no verified canonical Party")
                })?;

            lock_customer_subject(&mut transaction, &invocation.tenant_id, &canonical_party_id)
                .await?;

            let privacy_case = load_case(
                &mut transaction,
                &invocation.tenant_id,
                &invocation.privacy_case_id,
                RowLock::Update,
            )
            .await?
            .ok_or_else(case_not_found)?;
            if privacy_case != initial_case {
                return Err(retention_conflict(
                    "privacy case changed while acquiring the shared subject lock",
                ));
            }
            validate_case(&privacy_case, invocation, &canonical_party_id)?;

            let plan = load_plan(
                &mut transaction,
                &invocation.tenant_id,
                &invocation.action_plan_id,
                RowLock::Share,
            )
            .await?
            .ok_or_else(|| retention_conflict("immutable action plan is unavailable"))?;
            validate_plan(&privacy_case, &plan, invocation, &canonical_party_id)?;

            let holds =
                load_legal_holds(&mut transaction, &invocation.tenant_id, &canonical_party_id)
                    .await?;
            let decision = PrivacyRetentionDecisionSet::build(
                &plan,
                &holds,
                invocation.evaluated_at_unix_nanos,
            )
            .map_err(domain_error)?;

            let transaction_root_inserted =
                insert_transaction_root(&mut transaction, invocation).await?;
            let inserted = insert_decision(&mut transaction, invocation, &decision).await?;
            if inserted {
                if !transaction_root_inserted {
                    return Err(retention_evidence_invalid(
                        "retention transaction root existed without its decision record",
                    ));
                }
                insert_transaction_evidence(&mut transaction, invocation, &decision).await?;
            } else if transaction_root_inserted {
                delete_unused_transaction_root(&mut transaction, invocation).await?;
            }
            let stored = load_decision(
                &mut transaction,
                &invocation.tenant_id,
                decision.decision_id(),
                RowLock::Share,
            )
            .await?
            .ok_or_else(|| {
                retention_evidence_invalid("retention decision disappeared after insert")
            })?;
            if stored != decision {
                return Err(retention_conflict(
                    "retention-decision replay conflicts with deterministic content",
                ));
            }

            transaction.commit().await.map_err(database_error)?;
            Ok(RetentionEvaluationCommit {
                decision,
                replayed: !inserted,
            })
        })
    }
}

#[derive(Debug, Clone, Copy)]
enum RowLock {
    Share,
    Update,
}

async fn bind_invocation(
    transaction: &mut Transaction<'_, Postgres>,
    invocation: &RetentionEvaluationInvocation,
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
    .bind(invocation.tenant_id.as_str())
    .bind(invocation.actor_id.as_str())
    .bind(invocation.request_id.as_str())
    .bind(invocation.initiating_capability_id.as_str())
    .bind(invocation.initiating_capability_version.as_str())
    .bind(transaction_id(invocation))
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
    lock: RowLock,
) -> Result<Option<PrivacyCase>, SdkError> {
    let row = match lock {
        RowLock::Share => postgres_sqlx::query(
            r#"
        SELECT version, owner_module_id, schema_id, schema_version,
               descriptor_hash, data_class, payload_encoding,
               maximum_payload_size, retention_policy_id, payload_bytes
        FROM crm.records
        WHERE tenant_id = $1 AND owner_module_id = $2
          AND record_type = $3 AND record_id = $4 AND deleted_at IS NULL
        FOR SHARE
        "#,
        ),
        RowLock::Update => postgres_sqlx::query(
            r#"
        SELECT version, owner_module_id, schema_id, schema_version,
               descriptor_hash, data_class, payload_encoding,
               maximum_payload_size, retention_policy_id, payload_bytes
        FROM crm.records
        WHERE tenant_id = $1 AND owner_module_id = $2
          AND record_type = $3 AND record_id = $4 AND deleted_at IS NULL
        FOR UPDATE
        "#,
        ),
    }
    .bind(tenant_id.as_str())
    .bind(MODULE_ID)
    .bind(PRIVACY_CASE_RECORD_TYPE)
    .bind(case_id.as_str())
    .fetch_optional(&mut **transaction)
    .await
    .map_err(database_error)?;
    row.map(|row| {
        privacy_case_from_snapshot(&decode_snapshot(
            PRIVACY_CASE_RECORD_TYPE,
            case_id.clone(),
            DataClass::Confidential,
            row,
        )?)
    })
    .transpose()
}

async fn load_plan(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: &TenantId,
    plan_id: &RecordId,
    lock: RowLock,
) -> Result<Option<PrivacyActionPlan>, SdkError> {
    let row = match lock {
        RowLock::Share => postgres_sqlx::query(
            r#"
        SELECT version, owner_module_id, schema_id, schema_version,
               descriptor_hash, data_class, payload_encoding,
               maximum_payload_size, retention_policy_id, payload_bytes
        FROM crm.records
        WHERE tenant_id = $1 AND owner_module_id = $2
          AND record_type = $3 AND record_id = $4 AND deleted_at IS NULL
        FOR SHARE
        "#,
        ),
        RowLock::Update => postgres_sqlx::query(
            r#"
        SELECT version, owner_module_id, schema_id, schema_version,
               descriptor_hash, data_class, payload_encoding,
               maximum_payload_size, retention_policy_id, payload_bytes
        FROM crm.records
        WHERE tenant_id = $1 AND owner_module_id = $2
          AND record_type = $3 AND record_id = $4 AND deleted_at IS NULL
        FOR UPDATE
        "#,
        ),
    }
    .bind(tenant_id.as_str())
    .bind(MODULE_ID)
    .bind(ACTION_PLAN_RECORD_TYPE)
    .bind(plan_id.as_str())
    .fetch_optional(&mut **transaction)
    .await
    .map_err(database_error)?;
    row.map(|row| {
        let snapshot = decode_snapshot(
            ACTION_PLAN_RECORD_TYPE,
            plan_id.clone(),
            DataClass::Confidential,
            row,
        )?;
        if snapshot.version != 1
            || snapshot.payload.owner.as_str() != MODULE_ID
            || snapshot.payload.schema_id.as_str() != ACTION_PLAN_STATE_SCHEMA_ID
            || snapshot.payload.schema_version.as_str() != ACTION_PLAN_STATE_SCHEMA_VERSION
            || snapshot.payload.descriptor_hash != action_plan_state_descriptor_hash()
            || snapshot.payload.maximum_size_bytes != ACTION_PLAN_STATE_MAXIMUM_BYTES
            || snapshot.payload.retention_policy_id.as_str()
                != ACTION_PLAN_STATE_RETENTION_POLICY_ID
        {
            return Err(retention_evidence_invalid(
                "action-plan record envelope drifted",
            ));
        }
        let plan = decode_action_plan_state(&snapshot.payload.bytes)?;
        if plan.plan_id() != plan_id || plan.lineage().tenant_id() != tenant_id {
            return Err(retention_evidence_invalid(
                "action-plan identity differs from its record envelope",
            ));
        }
        Ok(plan)
    })
    .transpose()
}

async fn load_legal_holds(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: &TenantId,
    canonical_party_id: &RecordId,
) -> Result<Vec<CustomerDataLegalHold>, SdkError> {
    let limit = i64::try_from(RETENTION_DECISION_MAXIMUM_HOLDS + 1)
        .map_err(|_| retention_evidence_invalid("legal-hold bound exceeds PostgreSQL range"))?;
    let rows = postgres_sqlx::query(
        r#"
        SELECT record_id, version, owner_module_id, schema_id, schema_version,
               descriptor_hash, data_class, payload_encoding,
               maximum_payload_size, retention_policy_id, payload_bytes
        FROM crm.records
        WHERE tenant_id = $1 AND owner_module_id = $2
          AND record_type = $3 AND deleted_at IS NULL
          AND crm.customer_privacy_legal_hold_canonical_party_id(payload_bytes) = $4
        ORDER BY record_id ASC
        LIMIT $5
        FOR SHARE
        "#,
    )
    .bind(tenant_id.as_str())
    .bind(MODULE_ID)
    .bind(LEGAL_HOLD_RECORD_TYPE)
    .bind(canonical_party_id.as_str())
    .bind(limit)
    .fetch_all(&mut **transaction)
    .await
    .map_err(database_error)?;
    if rows.len() > RETENTION_DECISION_MAXIMUM_HOLDS {
        return Err(retention_evidence_invalid(
            "legal-hold inventory exceeds the governed bound",
        ));
    }

    rows.into_iter()
        .map(|row| {
            let record_id = row
                .try_get::<String, _>("record_id")
                .map_err(database_error)?;
            let snapshot = decode_snapshot(
                LEGAL_HOLD_RECORD_TYPE,
                RecordId::try_new(record_id).map_err(retention_evidence_invalid)?,
                DataClass::Personal,
                row,
            )?;
            let hold = legal_hold_from_snapshot(&snapshot).map_err(|_| {
                retention_evidence_invalid("legal-hold state failed strict rehydration")
            })?;
            if hold.tenant_id() != tenant_id || hold.canonical_party_id() != canonical_party_id {
                return Err(retention_evidence_invalid(
                    "legal-hold state differs from the tenant or canonical Party lookup key",
                ));
            }
            Ok(hold)
        })
        .collect()
}

const RETENTION_EVENT_TYPE: &str = "customer_privacy.retention.evaluated";
const AUDIT_CANONICALIZATION_PROFILE: &str = "crm.cjson/v1";
const AUDIT_LOCK_NAMESPACE: i64 = 0x4352_4d41_5544_4954;

async fn require_registered_initiating_capability(
    transaction: &mut Transaction<'_, Postgres>,
    invocation: &RetentionEvaluationInvocation,
) -> Result<(), SdkError> {
    if invocation.initiating_capability_version.as_str() != RETENTION_TRIGGER_CAPABILITY_VERSION
        || !matches!(
            invocation.initiating_capability_id.as_str(),
            RETENTION_APPROVAL_TRIGGER_CAPABILITY | RETENTION_LEGAL_HOLD_TRIGGER_CAPABILITY
        )
    {
        return Err(retention_evidence_invalid(
            "retention evaluation initiating capability is outside the governed step-six set",
        ));
    }
    let owner = postgres_sqlx::query(
        r#"
        SELECT owner_module_id
        FROM crm.capability_registry
        WHERE capability_id = $1 AND capability_version = $2
        "#,
    )
    .bind(invocation.initiating_capability_id.as_str())
    .bind(invocation.initiating_capability_version.as_str())
    .fetch_optional(&mut **transaction)
    .await
    .map_err(database_error)?
    .map(|row| row.try_get::<String, _>("owner_module_id"))
    .transpose()
    .map_err(database_error)?;
    if owner.as_deref() != Some(MODULE_ID) {
        return Err(retention_evidence_invalid(
            "retention initiating capability is absent or owned by another module",
        ));
    }
    Ok(())
}

async fn insert_transaction_root(
    transaction: &mut Transaction<'_, Postgres>,
    invocation: &RetentionEvaluationInvocation,
) -> Result<bool, SdkError> {
    require_registered_initiating_capability(transaction, invocation).await?;
    let result = postgres_sqlx::query(
        r#"
        INSERT INTO crm.business_transactions (
          tenant_id, business_transaction_id, actor_id, request_id,
          correlation_id, trace_id, capability_id, capability_version,
          expected_outbox_events, expected_audit_records, expected_idempotency_records
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,1,1,1)
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(invocation.tenant_id.as_str())
    .bind(transaction_id(invocation))
    .bind(invocation.actor_id.as_str())
    .bind(invocation.request_id.as_str())
    .bind(invocation.correlation_id.as_str())
    .bind(invocation.trace_id.as_str())
    .bind(invocation.initiating_capability_id.as_str())
    .bind(invocation.initiating_capability_version.as_str())
    .execute(&mut **transaction)
    .await
    .map_err(database_error)?;
    Ok(result.rows_affected() == 1)
}

async fn delete_unused_transaction_root(
    transaction: &mut Transaction<'_, Postgres>,
    invocation: &RetentionEvaluationInvocation,
) -> Result<(), SdkError> {
    let result = postgres_sqlx::query(
        "DELETE FROM crm.business_transactions WHERE tenant_id = $1 AND business_transaction_id = $2",
    )
    .bind(invocation.tenant_id.as_str())
    .bind(transaction_id(invocation))
    .execute(&mut **transaction)
    .await
    .map_err(database_error)?;
    if result.rows_affected() != 1 {
        return Err(retention_evidence_invalid(
            "unused retention transaction root could not be removed",
        ));
    }
    Ok(())
}

async fn insert_transaction_evidence(
    transaction: &mut Transaction<'_, Postgres>,
    invocation: &RetentionEvaluationInvocation,
    decision: &PrivacyRetentionDecisionSet,
) -> Result<(), SdkError> {
    let payload = retention_decision_persisted_payload(decision)?;
    let maximum = i64::try_from(payload.maximum_size_bytes).map_err(|_| {
        retention_evidence_invalid("decision evidence size exceeds PostgreSQL range")
    })?;
    let transaction_id = transaction_id(invocation);
    let suffix = &hex(&discovery_sha256(transaction_id.as_bytes()))[..24];
    let event_id = format!("privacy-retention-event-{suffix}");
    let audit_id = format!("privacy-retention-audit-{suffix}");
    let request_hash = retention_request_hash(invocation, decision)?;

    postgres_sqlx::query(
        r#"
        INSERT INTO crm.idempotency_records (
          tenant_id, idempotency_scope, idempotency_key, request_hash,
          status, business_transaction_id, expires_at
        ) VALUES ($1,$2,$3,$4,'completed',$5,clock_timestamp() + INTERVAL '24 hours')
        "#,
    )
    .bind(invocation.tenant_id.as_str())
    .bind(RETENTION_EVALUATE_COORDINATE)
    .bind(invocation.request_id.as_str())
    .bind(request_hash.as_slice())
    .bind(&transaction_id)
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
          $1,$2,$3,$4,$5,1,1,$6,$7,$8,$9,$10,
          'personal','json',$11,$12,$13,
          TIMESTAMPTZ 'epoch' + ($14::bigint / 1000) * INTERVAL '1 microsecond'
        )
        "#,
    )
    .bind(invocation.tenant_id.as_str())
    .bind(&event_id)
    .bind(&transaction_id)
    .bind(RETENTION_DECISION_RECORD_TYPE)
    .bind(decision.decision_id().as_str())
    .bind(RETENTION_EVENT_TYPE)
    .bind(&event_id)
    .bind(payload.schema_id.as_str())
    .bind(payload.schema_version.as_str())
    .bind(payload.descriptor_hash.as_slice())
    .bind(maximum)
    .bind(payload.retention_policy_id.as_str())
    .bind(payload.bytes.as_slice())
    .bind(invocation.evaluated_at_unix_nanos)
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
            let sequence = row
                .try_get::<i64, _>("next_sequence")
                .map_err(database_error)?;
            if sequence <= 0 {
                return Err(retention_evidence_invalid(
                    "tenant audit next sequence must be positive",
                ));
            }
            let previous_hash = row
                .try_get::<Vec<u8>, _>("last_hash")
                .map_err(database_error)?
                .try_into()
                .map_err(|_| {
                    retention_evidence_invalid("tenant audit hash must contain 32 bytes")
                })?;
            (sequence, previous_hash)
        }
        None => (1, [0; 32]),
    };
    let occurred_at = (invocation.evaluated_at_unix_nanos / 1_000) * 1_000;
    let audit_hash = retention_audit_hash(
        invocation,
        sequence,
        previous_hash,
        &audit_id,
        &transaction_id,
        &payload.bytes,
        occurred_at,
    )?;
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
    .bind(&transaction_id)
    .bind(invocation.actor_id.as_str())
    .bind(invocation.initiating_capability_id.as_str())
    .bind(invocation.initiating_capability_version.as_str())
    .bind(AUDIT_CANONICALIZATION_PROFILE)
    .bind(previous_hash.as_slice())
    .bind(audit_hash.as_slice())
    .bind(payload.bytes.as_slice())
    .bind(occurred_at)
    .execute(&mut **transaction)
    .await
    .map_err(database_error)?;
    Ok(())
}

fn retention_request_hash(
    invocation: &RetentionEvaluationInvocation,
    decision: &PrivacyRetentionDecisionSet,
) -> Result<[u8; 32], SdkError> {
    let mut bytes = Vec::new();
    for value in [
        b"crm.customer-privacy.retention.request/v1".as_slice(),
        invocation.tenant_id.as_str().as_bytes(),
        invocation.privacy_case_id.as_str().as_bytes(),
        invocation.action_plan_id.as_str().as_bytes(),
        invocation.actor_id.as_str().as_bytes(),
        invocation.request_id.as_str().as_bytes(),
        invocation.correlation_id.as_str().as_bytes(),
        invocation.trace_id.as_str().as_bytes(),
        invocation.initiating_capability_id.as_str().as_bytes(),
        invocation.initiating_capability_version.as_str().as_bytes(),
        &invocation.evaluated_at_unix_nanos.to_be_bytes(),
        decision.digest().as_slice(),
    ] {
        append_hash_field(&mut bytes, value)?;
    }
    Ok(discovery_sha256(&bytes))
}

#[allow(clippy::too_many_arguments)]
fn retention_audit_hash(
    invocation: &RetentionEvaluationInvocation,
    sequence: i64,
    previous_hash: [u8; 32],
    audit_id: &str,
    transaction_id: &str,
    canonical_envelope: &[u8],
    occurred_at: i64,
) -> Result<[u8; 32], SdkError> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"crm.audit.record.sha256/v1");
    append_hash_field(&mut bytes, invocation.tenant_id.as_str().as_bytes())?;
    bytes.extend_from_slice(&sequence.to_be_bytes());
    for value in [
        audit_id.as_bytes(),
        transaction_id.as_bytes(),
        invocation.actor_id.as_str().as_bytes(),
        invocation.initiating_capability_id.as_str().as_bytes(),
        invocation.initiating_capability_version.as_str().as_bytes(),
        AUDIT_CANONICALIZATION_PROFILE.as_bytes(),
    ] {
        append_hash_field(&mut bytes, value)?;
    }
    bytes.extend_from_slice(&previous_hash);
    append_hash_field(&mut bytes, canonical_envelope)?;
    bytes.extend_from_slice(&occurred_at.to_be_bytes());
    Ok(discovery_sha256(&bytes))
}

fn append_hash_field(output: &mut Vec<u8>, value: &[u8]) -> Result<(), SdkError> {
    let length = u64::try_from(value.len())
        .map_err(|_| retention_evidence_invalid("evidence hash field exceeds u64"))?;
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(value);
    Ok(())
}

async fn insert_decision(
    transaction: &mut Transaction<'_, Postgres>,
    invocation: &RetentionEvaluationInvocation,
    decision: &PrivacyRetentionDecisionSet,
) -> Result<bool, SdkError> {
    let payload = retention_decision_persisted_payload(decision)?;
    let maximum = i64::try_from(payload.maximum_size_bytes).map_err(|_| {
        retention_evidence_invalid("decision maximum size exceeds PostgreSQL range")
    })?;
    let result = postgres_sqlx::query(
        r#"
        INSERT INTO crm.records (
          tenant_id, record_type, record_id, version, owner_module_id,
          schema_id, schema_version, descriptor_hash, data_class,
          payload_encoding, maximum_payload_size, retention_policy_id,
          payload_bytes, last_business_transaction_id
        ) VALUES ($1,$2,$3,1,$4,$5,$6,$7,'personal','json',$8,$9,$10,$11)
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(invocation.tenant_id.as_str())
    .bind(RETENTION_DECISION_RECORD_TYPE)
    .bind(decision.decision_id().as_str())
    .bind(payload.owner.as_str())
    .bind(payload.schema_id.as_str())
    .bind(payload.schema_version.as_str())
    .bind(payload.descriptor_hash.as_slice())
    .bind(maximum)
    .bind(payload.retention_policy_id.as_str())
    .bind(payload.bytes)
    .bind(transaction_id(invocation))
    .execute(&mut **transaction)
    .await
    .map_err(database_error)?;
    Ok(result.rows_affected() == 1)
}

async fn load_decision(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: &TenantId,
    decision_id: &RecordId,
    lock: RowLock,
) -> Result<Option<PrivacyRetentionDecisionSet>, SdkError> {
    let row = match lock {
        RowLock::Share => postgres_sqlx::query(
            r#"
        SELECT version, owner_module_id, schema_id, schema_version,
               descriptor_hash, data_class, payload_encoding,
               maximum_payload_size, retention_policy_id, payload_bytes
        FROM crm.records
        WHERE tenant_id = $1 AND owner_module_id = $2
          AND record_type = $3 AND record_id = $4 AND deleted_at IS NULL
        FOR SHARE
        "#,
        ),
        RowLock::Update => postgres_sqlx::query(
            r#"
        SELECT version, owner_module_id, schema_id, schema_version,
               descriptor_hash, data_class, payload_encoding,
               maximum_payload_size, retention_policy_id, payload_bytes
        FROM crm.records
        WHERE tenant_id = $1 AND owner_module_id = $2
          AND record_type = $3 AND record_id = $4 AND deleted_at IS NULL
        FOR UPDATE
        "#,
        ),
    }
    .bind(tenant_id.as_str())
    .bind(MODULE_ID)
    .bind(RETENTION_DECISION_RECORD_TYPE)
    .bind(decision_id.as_str())
    .fetch_optional(&mut **transaction)
    .await
    .map_err(database_error)?;
    row.map(|row| {
        retention_decision_from_snapshot(&decode_snapshot(
            RETENTION_DECISION_RECORD_TYPE,
            decision_id.clone(),
            DataClass::Personal,
            row,
        )?)
    })
    .transpose()
}

fn decode_snapshot(
    record_type: &str,
    record_id: RecordId,
    expected_data_class: DataClass,
    row: postgres_sqlx::postgres::PgRow,
) -> Result<RecordSnapshot, SdkError> {
    let version = row.try_get("version").map_err(database_error)?;
    let owner = row
        .try_get::<String, _>("owner_module_id")
        .map_err(database_error)?;
    let schema_id = row
        .try_get::<String, _>("schema_id")
        .map_err(database_error)?;
    let schema_version = row
        .try_get::<String, _>("schema_version")
        .map_err(database_error)?;
    let descriptor_hash = row
        .try_get::<Vec<u8>, _>("descriptor_hash")
        .map_err(database_error)?
        .try_into()
        .map_err(|_| retention_evidence_invalid("descriptor hash must contain exactly 32 bytes"))?;
    let data_class = row
        .try_get::<String, _>("data_class")
        .map_err(database_error)?;
    let encoding = row
        .try_get::<String, _>("payload_encoding")
        .map_err(database_error)?;
    let maximum = row
        .try_get::<i64, _>("maximum_payload_size")
        .map_err(database_error)?;
    let retention = row
        .try_get::<String, _>("retention_policy_id")
        .map_err(database_error)?;
    let bytes = row
        .try_get::<Vec<u8>, _>("payload_bytes")
        .map_err(database_error)?;
    if data_class != data_class_label(expected_data_class) || encoding != "json" {
        return Err(retention_evidence_invalid(
            "record data class or encoding differs from its contract",
        ));
    }
    Ok(RecordSnapshot {
        reference: RecordRef {
            record_type: RecordType::try_new(record_type).map_err(retention_evidence_invalid)?,
            record_id,
        },
        version,
        payload: TypedPayload {
            owner: ModuleId::try_new(owner).map_err(retention_evidence_invalid)?,
            schema_id: SchemaId::try_new(schema_id).map_err(retention_evidence_invalid)?,
            schema_version: SchemaVersion::try_new(schema_version)
                .map_err(retention_evidence_invalid)?,
            descriptor_hash,
            data_class: expected_data_class,
            encoding: PayloadEncoding::Json,
            maximum_size_bytes: u64::try_from(maximum)
                .map_err(|_| retention_evidence_invalid("maximum payload size is negative"))?,
            retention_policy_id: RetentionPolicyId::try_new(retention)
                .map_err(retention_evidence_invalid)?,
            bytes,
        },
    })
}

fn validate_case(
    privacy_case: &PrivacyCase,
    invocation: &RetentionEvaluationInvocation,
    canonical_party_id: &RecordId,
) -> Result<(), SdkError> {
    if privacy_case.status() != PrivacyCaseStatus::Planned
        || privacy_case.action_plan_id() != Some(&invocation.action_plan_id)
        || privacy_case
            .subject_binding()
            .map(|binding| &binding.canonical_party_id)
            != Some(canonical_party_id)
    {
        return Err(retention_conflict(
            "privacy case is not an exact planned case for the requested subject and plan",
        ));
    }
    Ok(())
}

fn validate_plan(
    privacy_case: &PrivacyCase,
    plan: &PrivacyActionPlan,
    invocation: &RetentionEvaluationInvocation,
    canonical_party_id: &RecordId,
) -> Result<(), SdkError> {
    let lineage = plan.lineage();
    if lineage.tenant_id() != &invocation.tenant_id
        || lineage.privacy_case_id() != &invocation.privacy_case_id
        || lineage.canonical_party_id() != canonical_party_id
        || plan.plan_id() != &invocation.action_plan_id
        || (lineage.approval_required() && privacy_case.approval().is_none())
    {
        return Err(retention_conflict(
            "action plan lineage or approval evidence differs from the planned case",
        ));
    }
    Ok(())
}

fn transaction_id(invocation: &RetentionEvaluationInvocation) -> String {
    let mut bytes = Vec::new();
    for value in [
        invocation.initiating_capability_id.as_str().as_bytes(),
        invocation.initiating_capability_version.as_str().as_bytes(),
        invocation.request_id.as_str().as_bytes(),
    ] {
        bytes.extend_from_slice(&(value.len() as u64).to_be_bytes());
        bytes.extend_from_slice(value);
    }
    let digest = discovery_sha256(&bytes);
    format!("privacy-retention-{}", hex(&digest[..12]))
}

fn data_class_label(value: DataClass) -> &'static str {
    match value {
        DataClass::Public => "public",
        DataClass::Internal => "internal",
        DataClass::Confidential => "confidential",
        DataClass::Restricted => "restricted",
        DataClass::Personal => "personal",
        DataClass::SensitivePersonal => "sensitive_personal",
        DataClass::Biometric => "biometric",
        DataClass::Financial => "financial",
        DataClass::Credential => "credential",
    }
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
        "CUSTOMER_PRIVACY_RETENTION_STORE_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "Customer Privacy retention storage is unavailable.",
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

fn retention_conflict(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_RETENTION_CONFLICT",
        ErrorCategory::Conflict,
        false,
        "Customer Privacy retention adjudication conflicts with current authoritative state.",
    )
    .with_internal_reference(reference)
}

fn retention_evidence_invalid(reference: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_RETENTION_EVIDENCE_INVALID",
        ErrorCategory::Internal,
        false,
        "Customer Privacy retention evidence is invalid.",
    )
    .with_internal_reference(reference.to_string())
}

fn domain_error(error: impl std::fmt::Display) -> SdkError {
    retention_conflict(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transaction_identity_is_deterministic_and_coordinate_bound() {
        let invocation = RetentionEvaluationInvocation {
            tenant_id: TenantId::try_new("tenant-a").unwrap(),
            privacy_case_id: RecordId::try_new("case-a").unwrap(),
            action_plan_id: RecordId::try_new("plan-a").unwrap(),
            actor_id: crm_module_sdk::ActorId::try_new("privacy-worker").unwrap(),
            request_id: crm_module_sdk::RequestId::try_new("request-a").unwrap(),
            correlation_id: crm_module_sdk::CorrelationId::try_new("correlation-a").unwrap(),
            trace_id: crm_module_sdk::TraceId::try_new("trace-a").unwrap(),
            initiating_capability_id: crm_module_sdk::CapabilityId::try_new(
                RETENTION_APPROVAL_TRIGGER_CAPABILITY,
            )
            .unwrap(),
            initiating_capability_version: crm_module_sdk::CapabilityVersion::try_new(
                RETENTION_TRIGGER_CAPABILITY_VERSION,
            )
            .unwrap(),
            request_started_at_unix_nanos: 1_000,
            evaluated_at_unix_nanos: 2_000,
            trusted_internal: true,
        };
        assert_eq!(transaction_id(&invocation), transaction_id(&invocation));
        assert!(transaction_id(&invocation).starts_with("privacy-retention-"));
    }
}
