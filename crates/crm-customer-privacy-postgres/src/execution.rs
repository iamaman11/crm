use crm_core_data::{
    PostgresDataStore,
    postgres_sqlx::{self, Postgres, Row, Transaction},
};
use crm_customer_privacy::{
    ACTION_PLAN_RECORD_TYPE, ACTION_PLAN_STATE_MAXIMUM_BYTES,
    ACTION_PLAN_STATE_RETENTION_POLICY_ID, ACTION_PLAN_STATE_SCHEMA_ID,
    ACTION_PLAN_STATE_SCHEMA_VERSION, MODULE_ID, OWNER_ACTION_ATTEMPT_STATE_MAXIMUM_BYTES,
    OWNER_ACTION_ATTEMPT_STATE_RETENTION_POLICY_ID, OWNER_ACTION_ATTEMPT_STATE_SCHEMA_ID,
    OWNER_ACTION_ATTEMPT_STATE_SCHEMA_VERSION, OWNER_ACTION_OUTCOME_STATE_MAXIMUM_BYTES,
    OWNER_ACTION_OUTCOME_STATE_RETENTION_POLICY_ID, OWNER_ACTION_OUTCOME_STATE_SCHEMA_ID,
    OWNER_ACTION_OUTCOME_STATE_SCHEMA_VERSION, PRIVACY_CASE_RECORD_TYPE, PrivacyActionPlan,
    PrivacyCase, PrivacyCaseStatus, PrivacyOwnerActionAttempt, PrivacyOwnerActionOutcome,
    PrivacyOwnerOutcomeStatus, PrivacyRetentionDecisionSet, RETENTION_DECISION_RECORD_TYPE,
    ResumeStage, action_plan_state_descriptor_hash, decode_action_plan_state,
    decode_owner_action_attempt_state, decode_owner_action_outcome_state, discovery_sha256,
    encode_owner_action_attempt_state, encode_owner_action_outcome_state,
    owner_action_attempt_state_descriptor_hash, owner_action_outcome_state_descriptor_hash,
};
use crm_customer_privacy_application::{
    CheckpointAdvance, ExecutionPreparation, OwnerExecutionInvocation,
    OwnerExecutionPersistencePort,
};
use crm_customer_privacy_persistence_adapter::{
    privacy_case_from_snapshot, privacy_case_persisted_payload, retention_decision_from_snapshot,
};
use crm_module_sdk::{
    DataClass, ErrorCategory, ModuleId, PayloadEncoding, PortFuture, RecordId, RecordRef,
    RecordSnapshot, RecordType, RetentionPolicyId, SchemaId, SchemaVersion, SdkError, TenantId,
    TypedPayload,
};
use std::sync::Arc;

const MAXIMUM_ITEMS: usize = 16_384;
const EXECUTION_CASE_EVENT_TYPE: &str = "customer_privacy.owner_execution.case_transitioned";
const EXECUTION_CASE_IDEMPOTENCY_SCOPE: &str = "customer_privacy.owner_execution.case-transition";
const AUDIT_CANONICALIZATION_PROFILE: &str = "crm.cjson/v1";
const AUDIT_LOCK_NAMESPACE: i64 = 0x4352_4d41_5544_4954;

#[derive(Debug, Clone)]
pub struct PostgresOwnerExecutionPersistence {
    store: Arc<PostgresDataStore>,
}

impl PostgresOwnerExecutionPersistence {
    pub fn new(store: Arc<PostgresDataStore>) -> Self {
        Self { store }
    }

    pub fn store(&self) -> &Arc<PostgresDataStore> {
        &self.store
    }
}

impl OwnerExecutionPersistencePort for PostgresOwnerExecutionPersistence {
    fn prepare_next<'a>(
        &'a self,
        invocation: &'a OwnerExecutionInvocation,
    ) -> PortFuture<'a, Result<ExecutionPreparation, SdkError>> {
        Box::pin(async move {
            let mut transaction = self.store.pool().begin().await.map_err(database_error)?;
            bind_invocation(&mut transaction, invocation).await?;
            let mut source = load_execution_source(&mut transaction, invocation).await?;
            let mut checkpoint = match load_checkpoint(
                &mut transaction,
                &invocation.tenant_id,
                &invocation.privacy_case_id,
            )
            .await?
            {
                Some(checkpoint) => {
                    validate_checkpoint(&checkpoint, &source, invocation)?;
                    checkpoint
                }
                None => initialize_checkpoint(&mut transaction, invocation, &mut source).await?,
            };

            normalize_checkpoint(&mut transaction, invocation, &mut source, &mut checkpoint)
                .await?;
            if checkpoint.complete() {
                transaction.commit().await.map_err(database_error)?;
                return Ok(ExecutionPreparation::Complete {
                    total_items: checkpoint.total_items,
                    durable_outcomes: checkpoint.total_items,
                });
            }

            let sequence = checkpoint.next_sequence;
            let item = source
                .decision
                .items()
                .get(sequence.saturating_sub(1) as usize)
                .ok_or_else(|| evidence_invalid("checkpoint references a missing decision item"))?;
            let latest =
                load_latest_attempt_with_outcome(&mut transaction, invocation, sequence).await?;
            let (attempt, replayed) = match latest {
                None => (
                    PrivacyOwnerActionAttempt::build(
                        invocation.tenant_id.clone(),
                        invocation.privacy_case_id.clone(),
                        invocation.action_plan_id.clone(),
                        *source.plan.digest(),
                        invocation.retention_decision_id.clone(),
                        *source.decision.digest(),
                        item,
                        0,
                        invocation.planned_at_unix_nanos,
                    )?,
                    false,
                ),
                Some((attempt, None)) => (attempt, true),
                Some((attempt, Some(outcome)))
                    if outcome.status() == PrivacyOwnerOutcomeStatus::FailedRetryable =>
                {
                    let generation = attempt
                        .attempt_generation()
                        .checked_add(1)
                        .ok_or_else(|| evidence_invalid("attempt generation overflowed"))?;
                    if invocation.planned_at_unix_nanos <= attempt.planned_at_unix_nanos() {
                        return Err(execution_conflict(
                            "retry attempt time must advance the prior attempt time",
                        ));
                    }
                    (
                        PrivacyOwnerActionAttempt::build(
                            invocation.tenant_id.clone(),
                            invocation.privacy_case_id.clone(),
                            invocation.action_plan_id.clone(),
                            *source.plan.digest(),
                            invocation.retention_decision_id.clone(),
                            *source.decision.digest(),
                            item,
                            generation,
                            invocation.planned_at_unix_nanos,
                        )?,
                        false,
                    )
                }
                Some((_attempt, Some(_outcome))) => {
                    return Err(evidence_invalid(
                        "checkpoint did not advance across a final durable outcome",
                    ));
                }
            };
            let inserted = insert_attempt(&mut transaction, invocation, &attempt).await?;
            let stored = load_attempt_by_id(
                &mut transaction,
                &invocation.tenant_id,
                attempt.attempt_id(),
            )
            .await?
            .ok_or_else(|| evidence_invalid("attempt disappeared after persistence"))?;
            if stored != attempt {
                return Err(execution_conflict(
                    "attempt replay conflicts with deterministic evidence",
                ));
            }
            if inserted {
                append_execution_audit(
                    &mut transaction,
                    invocation,
                    "attempt_prepared",
                    Some(&attempt),
                    None,
                    Some(checkpoint.next_sequence),
                    attempt.planned_at_unix_nanos(),
                )
                .await?;
            }
            transaction.commit().await.map_err(database_error)?;
            Ok(ExecutionPreparation::Ready {
                attempt: Box::new(attempt),
                attempt_replayed: replayed || !inserted,
            })
        })
    }

    fn record_outcome<'a>(
        &'a self,
        invocation: &'a OwnerExecutionInvocation,
        attempt: &'a PrivacyOwnerActionAttempt,
        outcome: &'a PrivacyOwnerActionOutcome,
    ) -> PortFuture<'a, Result<bool, SdkError>> {
        Box::pin(async move {
            let mut transaction = self.store.pool().begin().await.map_err(database_error)?;
            bind_invocation(&mut transaction, invocation).await?;
            validate_attempt_invocation(attempt, invocation)?;
            validate_outcome_attempt(outcome, attempt)?;
            let stored_attempt = load_attempt_by_id(
                &mut transaction,
                &invocation.tenant_id,
                attempt.attempt_id(),
            )
            .await?
            .ok_or_else(|| evidence_invalid("outcome references an unavailable attempt"))?;
            if &stored_attempt != attempt {
                return Err(execution_conflict(
                    "outcome attempt differs from durable attempt evidence",
                ));
            }
            let inserted = insert_outcome(&mut transaction, outcome).await?;
            let stored = load_outcome_by_id(
                &mut transaction,
                &invocation.tenant_id,
                outcome.outcome_id(),
            )
            .await?
            .ok_or_else(|| evidence_invalid("outcome disappeared after persistence"))?;
            if stored != *outcome {
                return Err(execution_conflict(
                    "outcome replay conflicts with append-once evidence",
                ));
            }
            if inserted {
                append_execution_audit(
                    &mut transaction,
                    invocation,
                    "outcome_recorded",
                    Some(attempt),
                    Some(outcome),
                    None,
                    outcome.recorded_at_unix_nanos(),
                )
                .await?;
            }
            transaction.commit().await.map_err(database_error)?;
            Ok(inserted)
        })
    }

    fn advance_checkpoint<'a>(
        &'a self,
        invocation: &'a OwnerExecutionInvocation,
    ) -> PortFuture<'a, Result<CheckpointAdvance, SdkError>> {
        Box::pin(async move {
            let mut transaction = self.store.pool().begin().await.map_err(database_error)?;
            bind_invocation(&mut transaction, invocation).await?;
            let mut source = load_execution_source(&mut transaction, invocation).await?;
            let mut checkpoint = load_checkpoint(
                &mut transaction,
                &invocation.tenant_id,
                &invocation.privacy_case_id,
            )
            .await?
            .ok_or_else(|| evidence_invalid("execution checkpoint is unavailable"))?;
            validate_checkpoint(&checkpoint, &source, invocation)?;
            normalize_checkpoint(&mut transaction, invocation, &mut source, &mut checkpoint)
                .await?;
            transaction.commit().await.map_err(database_error)?;
            Ok(CheckpointAdvance {
                next_sequence: checkpoint.next_sequence,
                total_items: checkpoint.total_items,
                complete: checkpoint.complete(),
            })
        })
    }
}

#[derive(Debug, Clone)]
struct ExecutionSource {
    privacy_case: PrivacyCase,
    plan: PrivacyActionPlan,
    decision: PrivacyRetentionDecisionSet,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExecutionCheckpoint {
    source_case_version: u64,
    executing_case_version: u64,
    converging_case_version: Option<u64>,
    action_plan_id: RecordId,
    action_plan_digest: [u8; 32],
    retention_decision_id: RecordId,
    retention_decision_digest: [u8; 32],
    total_items: u32,
    next_sequence: u32,
    started_at_unix_nanos: i64,
    completed_at_unix_nanos: Option<i64>,
}

impl ExecutionCheckpoint {
    fn complete(&self) -> bool {
        self.next_sequence == self.total_items.saturating_add(1)
            && self.completed_at_unix_nanos.is_some()
            && self.converging_case_version.is_some()
    }
}

async fn bind_invocation(
    transaction: &mut Transaction<'_, Postgres>,
    invocation: &OwnerExecutionInvocation,
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

async fn load_execution_source(
    transaction: &mut Transaction<'_, Postgres>,
    invocation: &OwnerExecutionInvocation,
) -> Result<ExecutionSource, SdkError> {
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
        .ok_or_else(|| execution_conflict("privacy case has no verified canonical Party"))?;
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
        return Err(execution_conflict(
            "privacy case changed while acquiring the shared subject lock",
        ));
    }
    let plan = load_plan(
        transaction,
        &invocation.tenant_id,
        &invocation.action_plan_id,
    )
    .await?
    .ok_or_else(|| evidence_invalid("immutable action plan is unavailable"))?;
    let decision = load_decision(
        transaction,
        &invocation.tenant_id,
        &invocation.retention_decision_id,
    )
    .await?
    .ok_or_else(|| evidence_invalid("retention decision is unavailable"))?;
    validate_execution_source(&privacy_case, &plan, &decision, invocation)?;
    Ok(ExecutionSource {
        privacy_case,
        plan,
        decision,
    })
}

fn validate_execution_source(
    privacy_case: &PrivacyCase,
    plan: &PrivacyActionPlan,
    decision: &PrivacyRetentionDecisionSet,
    invocation: &OwnerExecutionInvocation,
) -> Result<(), SdkError> {
    if privacy_case.tenant_id() != &invocation.tenant_id
        || privacy_case.case_id() != &invocation.privacy_case_id
        || privacy_case.action_plan_id() != Some(&invocation.action_plan_id)
        || plan.lineage().tenant_id() != &invocation.tenant_id
        || plan.lineage().privacy_case_id() != &invocation.privacy_case_id
        || plan.plan_id() != &invocation.action_plan_id
        || decision.tenant_id() != &invocation.tenant_id
        || decision.privacy_case_id() != &invocation.privacy_case_id
        || decision.action_plan_id() != &invocation.action_plan_id
        || decision.action_plan_digest() != plan.digest()
        || decision.decision_id() != &invocation.retention_decision_id
        || decision.items().len() != plan.items().len()
        || decision.items().len() > MAXIMUM_ITEMS
    {
        return Err(evidence_invalid(
            "case, action plan and retention decision lineage do not match",
        ));
    }
    if !matches!(
        privacy_case.status(),
        PrivacyCaseStatus::Planned
            | PrivacyCaseStatus::Executing
            | PrivacyCaseStatus::Converging
            | PrivacyCaseStatus::FailedRetryable(ResumeStage::Executing)
    ) {
        return Err(execution_conflict(
            "privacy case is not eligible for owner execution",
        ));
    }
    for (plan_item, decision_item) in plan.items().iter().zip(decision.items()) {
        if plan_item.sequence() != decision_item.sequence()
            || plan_item.owner_module_id() != decision_item.owner_module_id()
            || plan_item.resource_type() != decision_item.resource_type()
            || plan_item.resource_id() != decision_item.resource_id()
            || plan_item.resource_version() != decision_item.resource_version()
            || plan_item.data_class() != decision_item.data_class()
            || plan_item.evidence_class() != decision_item.evidence_class()
            || plan_item.retention_policy_id() != decision_item.retention_policy_id()
            || plan_item.action() != decision_item.approved_action()
        {
            return Err(evidence_invalid(
                "retention decision item differs from the immutable action plan",
            ));
        }
    }
    Ok(())
}

async fn initialize_checkpoint(
    transaction: &mut Transaction<'_, Postgres>,
    invocation: &OwnerExecutionInvocation,
    source: &mut ExecutionSource,
) -> Result<ExecutionCheckpoint, SdkError> {
    if source.privacy_case.status() != PrivacyCaseStatus::Planned {
        return Err(execution_conflict(
            "new execution requires the privacy case to be planned",
        ));
    }
    let source_case_version = source.privacy_case.version();
    source
        .privacy_case
        .begin_execution(source_case_version, invocation.planned_at_unix_nanos)
        .map_err(domain_error)?;
    let executing_case_version = source.privacy_case.version();
    let total_items = u32::try_from(source.decision.items().len())
        .map_err(|_| evidence_invalid("owner execution item count exceeds u32"))?;
    let mut checkpoint = ExecutionCheckpoint {
        source_case_version,
        executing_case_version,
        converging_case_version: None,
        action_plan_id: source.plan.plan_id().clone(),
        action_plan_digest: *source.plan.digest(),
        retention_decision_id: source.decision.decision_id().clone(),
        retention_decision_digest: *source.decision.digest(),
        total_items,
        next_sequence: 1,
        started_at_unix_nanos: invocation.planned_at_unix_nanos,
        completed_at_unix_nanos: None,
    };
    if total_items == 0 {
        source
            .privacy_case
            .begin_convergence(executing_case_version, invocation.planned_at_unix_nanos)
            .map_err(domain_error)?;
        checkpoint.converging_case_version = Some(source.privacy_case.version());
        checkpoint.completed_at_unix_nanos = Some(invocation.planned_at_unix_nanos);
    }
    update_case_record(
        transaction,
        invocation,
        source_case_version,
        &source.privacy_case,
    )
    .await?;
    insert_checkpoint(transaction, invocation, &checkpoint).await?;
    append_execution_audit(
        transaction,
        invocation,
        "execution_started",
        None,
        None,
        Some(checkpoint.next_sequence),
        invocation.planned_at_unix_nanos,
    )
    .await?;
    if checkpoint.complete() {
        append_execution_audit(
            transaction,
            invocation,
            "execution_complete",
            None,
            None,
            Some(checkpoint.next_sequence),
            invocation.planned_at_unix_nanos,
        )
        .await?;
    }
    Ok(checkpoint)
}

async fn normalize_checkpoint(
    transaction: &mut Transaction<'_, Postgres>,
    invocation: &OwnerExecutionInvocation,
    source: &mut ExecutionSource,
    checkpoint: &mut ExecutionCheckpoint,
) -> Result<(), SdkError> {
    if checkpoint.complete() {
        if source.privacy_case.status() != PrivacyCaseStatus::Converging
            || Some(source.privacy_case.version()) != checkpoint.converging_case_version
        {
            return Err(evidence_invalid(
                "completed checkpoint differs from the privacy case convergence state",
            ));
        }
        return Ok(());
    }
    let starting_sequence = checkpoint.next_sequence;
    while checkpoint.next_sequence <= checkpoint.total_items {
        let latest =
            load_latest_attempt_with_outcome(transaction, invocation, checkpoint.next_sequence)
                .await?;
        let Some((_attempt, Some(outcome))) = latest else {
            break;
        };
        if outcome.status() == PrivacyOwnerOutcomeStatus::FailedRetryable {
            break;
        }
        checkpoint.next_sequence = checkpoint
            .next_sequence
            .checked_add(1)
            .ok_or_else(|| evidence_invalid("checkpoint sequence overflowed"))?;
    }
    if checkpoint.next_sequence != starting_sequence {
        if checkpoint.next_sequence == checkpoint.total_items.saturating_add(1) {
            let expected = source.privacy_case.version();
            if !matches!(
                source.privacy_case.status(),
                PrivacyCaseStatus::Executing
                    | PrivacyCaseStatus::FailedRetryable(ResumeStage::Executing)
            ) {
                return Err(evidence_invalid(
                    "execution completed while the privacy case was not executing",
                ));
            }
            if source.privacy_case.status()
                == PrivacyCaseStatus::FailedRetryable(ResumeStage::Executing)
            {
                source
                    .privacy_case
                    .resume(expected, invocation.planned_at_unix_nanos)
                    .map_err(domain_error)?;
            }
            let executing_version = source.privacy_case.version();
            source
                .privacy_case
                .begin_convergence(executing_version, invocation.planned_at_unix_nanos)
                .map_err(domain_error)?;
            update_case_record(transaction, invocation, expected, &source.privacy_case).await?;
            checkpoint.converging_case_version = Some(source.privacy_case.version());
            checkpoint.completed_at_unix_nanos = Some(invocation.planned_at_unix_nanos);
        }
        update_checkpoint(transaction, invocation, checkpoint).await?;
        append_execution_audit(
            transaction,
            invocation,
            "checkpoint_advanced",
            None,
            None,
            Some(checkpoint.next_sequence),
            invocation.planned_at_unix_nanos,
        )
        .await?;
        if checkpoint.complete() {
            append_execution_audit(
                transaction,
                invocation,
                "execution_complete",
                None,
                None,
                Some(checkpoint.next_sequence),
                invocation.planned_at_unix_nanos,
            )
            .await?;
        }
    }
    Ok(())
}

fn validate_checkpoint(
    checkpoint: &ExecutionCheckpoint,
    source: &ExecutionSource,
    invocation: &OwnerExecutionInvocation,
) -> Result<(), SdkError> {
    if checkpoint.action_plan_id != invocation.action_plan_id
        || checkpoint.action_plan_digest != *source.plan.digest()
        || checkpoint.retention_decision_id != invocation.retention_decision_id
        || checkpoint.retention_decision_digest != *source.decision.digest()
        || checkpoint.total_items as usize != source.decision.items().len()
        || checkpoint.next_sequence == 0
        || checkpoint.next_sequence > checkpoint.total_items.saturating_add(1)
        || checkpoint.executing_case_version != checkpoint.source_case_version.saturating_add(1)
    {
        return Err(evidence_invalid(
            "owner execution checkpoint lineage or progress is invalid",
        ));
    }
    if checkpoint.complete() {
        if source.privacy_case.status() != PrivacyCaseStatus::Converging
            || checkpoint.converging_case_version != Some(source.privacy_case.version())
        {
            return Err(evidence_invalid(
                "completed checkpoint does not match the converging case",
            ));
        }
    } else if !matches!(
        source.privacy_case.status(),
        PrivacyCaseStatus::Executing | PrivacyCaseStatus::FailedRetryable(ResumeStage::Executing)
    ) || source.privacy_case.version() < checkpoint.executing_case_version
    {
        return Err(evidence_invalid(
            "active checkpoint does not match the executing privacy case",
        ));
    }
    Ok(())
}

async fn insert_checkpoint(
    transaction: &mut Transaction<'_, Postgres>,
    invocation: &OwnerExecutionInvocation,
    checkpoint: &ExecutionCheckpoint,
) -> Result<(), SdkError> {
    let result = postgres_sqlx::query(
        r#"
        INSERT INTO crm.customer_privacy_owner_execution_checkpoints (
          tenant_id, privacy_case_id, source_case_version, executing_case_version,
          converging_case_version, action_plan_id, action_plan_digest,
          retention_decision_id, retention_decision_digest, total_items,
          next_sequence, started_at_unix_nanos, completed_at_unix_nanos
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(invocation.tenant_id.as_str())
    .bind(invocation.privacy_case_id.as_str())
    .bind(checked_i64(
        checkpoint.source_case_version,
        "source case version",
    )?)
    .bind(checked_i64(
        checkpoint.executing_case_version,
        "executing case version",
    )?)
    .bind(
        checkpoint
            .converging_case_version
            .map(|value| checked_i64(value, "converging case version"))
            .transpose()?,
    )
    .bind(checkpoint.action_plan_id.as_str())
    .bind(checkpoint.action_plan_digest.as_slice())
    .bind(checkpoint.retention_decision_id.as_str())
    .bind(checkpoint.retention_decision_digest.as_slice())
    .bind(i32::try_from(checkpoint.total_items).map_err(evidence_invalid)?)
    .bind(i32::try_from(checkpoint.next_sequence).map_err(evidence_invalid)?)
    .bind(checkpoint.started_at_unix_nanos)
    .bind(checkpoint.completed_at_unix_nanos)
    .execute(&mut **transaction)
    .await
    .map_err(database_error)?;
    if result.rows_affected() != 1 {
        return Err(execution_conflict(
            "execution checkpoint already exists with unknown evidence",
        ));
    }
    Ok(())
}

async fn update_checkpoint(
    transaction: &mut Transaction<'_, Postgres>,
    invocation: &OwnerExecutionInvocation,
    checkpoint: &ExecutionCheckpoint,
) -> Result<(), SdkError> {
    let result = postgres_sqlx::query(
        r#"
        UPDATE crm.customer_privacy_owner_execution_checkpoints
        SET next_sequence = $3,
            converging_case_version = $4,
            completed_at_unix_nanos = $5
        WHERE tenant_id = $1 AND privacy_case_id = $2
          AND next_sequence <= $3
        "#,
    )
    .bind(invocation.tenant_id.as_str())
    .bind(invocation.privacy_case_id.as_str())
    .bind(i32::try_from(checkpoint.next_sequence).map_err(evidence_invalid)?)
    .bind(
        checkpoint
            .converging_case_version
            .map(|value| checked_i64(value, "converging case version"))
            .transpose()?,
    )
    .bind(checkpoint.completed_at_unix_nanos)
    .execute(&mut **transaction)
    .await
    .map_err(database_error)?;
    if result.rows_affected() != 1 {
        return Err(execution_conflict(
            "execution checkpoint changed before advancement",
        ));
    }
    Ok(())
}

async fn load_checkpoint(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: &TenantId,
    privacy_case_id: &RecordId,
) -> Result<Option<ExecutionCheckpoint>, SdkError> {
    postgres_sqlx::query(
        r#"
        SELECT source_case_version, executing_case_version, converging_case_version,
               action_plan_id, action_plan_digest, retention_decision_id,
               retention_decision_digest, total_items, next_sequence,
               started_at_unix_nanos, completed_at_unix_nanos
        FROM crm.customer_privacy_owner_execution_checkpoints
        WHERE tenant_id = $1 AND privacy_case_id = $2
        FOR UPDATE
        "#,
    )
    .bind(tenant_id.as_str())
    .bind(privacy_case_id.as_str())
    .fetch_optional(&mut **transaction)
    .await
    .map_err(database_error)?
    .map(checkpoint_from_row)
    .transpose()
}

fn checkpoint_from_row(
    row: postgres_sqlx::postgres::PgRow,
) -> Result<ExecutionCheckpoint, SdkError> {
    Ok(ExecutionCheckpoint {
        source_case_version: positive_u64(&row, "source_case_version")?,
        executing_case_version: positive_u64(&row, "executing_case_version")?,
        converging_case_version: optional_positive_u64(&row, "converging_case_version")?,
        action_plan_id: identifier(row.try_get("action_plan_id").map_err(database_error)?)?,
        action_plan_digest: digest(&row, "action_plan_digest")?,
        retention_decision_id: identifier(
            row.try_get("retention_decision_id")
                .map_err(database_error)?,
        )?,
        retention_decision_digest: digest(&row, "retention_decision_digest")?,
        total_items: bounded_u32(&row, "total_items", 0, 16_384)?,
        next_sequence: bounded_u32(&row, "next_sequence", 1, 16_385)?,
        started_at_unix_nanos: positive_i64(&row, "started_at_unix_nanos")?,
        completed_at_unix_nanos: row
            .try_get("completed_at_unix_nanos")
            .map_err(database_error)?,
    })
}

async fn insert_attempt(
    transaction: &mut Transaction<'_, Postgres>,
    invocation: &OwnerExecutionInvocation,
    attempt: &PrivacyOwnerActionAttempt,
) -> Result<bool, SdkError> {
    validate_attempt_invocation(attempt, invocation)?;
    let payload = encode_owner_action_attempt_state(attempt)?;
    let result = postgres_sqlx::query(
        r#"
        INSERT INTO crm.customer_privacy_owner_action_attempts (
          tenant_id, privacy_case_id, action_plan_id, action_plan_digest,
          retention_decision_id, retention_decision_digest, item_sequence,
          attempt_generation, attempt_id, attempt_digest, item_digest,
          owner_module_id, owner_capability_id, owner_capability_version,
          target_idempotency_key, resource_type, resource_id, resource_version,
          action_code, decision_reason, schema_id, schema_version,
          descriptor_hash, maximum_payload_size, retention_policy_id,
          payload_bytes, planned_at_unix_nanos
        ) VALUES (
          $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,
          $19,$20,$21,$22,$23,$24,$25,$26,$27
        ) ON CONFLICT DO NOTHING
        "#,
    )
    .bind(attempt.tenant_id().as_str())
    .bind(attempt.privacy_case_id().as_str())
    .bind(attempt.action_plan_id().as_str())
    .bind(attempt.action_plan_digest().as_slice())
    .bind(attempt.retention_decision_id().as_str())
    .bind(attempt.retention_decision_digest().as_slice())
    .bind(i32::try_from(attempt.item_sequence()).map_err(evidence_invalid)?)
    .bind(i32::try_from(attempt.attempt_generation()).map_err(evidence_invalid)?)
    .bind(attempt.attempt_id().as_str())
    .bind(attempt.digest().as_slice())
    .bind(attempt.item_digest().as_slice())
    .bind(attempt.owner_module_id().as_str())
    .bind(attempt.owner_capability_id())
    .bind(attempt.owner_capability_version())
    .bind(attempt.target_idempotency_key().as_str())
    .bind(attempt.resource_type())
    .bind(attempt.resource_id().as_str())
    .bind(checked_i64(attempt.resource_version(), "resource version")?)
    .bind(attempt.action_code())
    .bind(attempt.decision_reason())
    .bind(OWNER_ACTION_ATTEMPT_STATE_SCHEMA_ID)
    .bind(OWNER_ACTION_ATTEMPT_STATE_SCHEMA_VERSION)
    .bind(owner_action_attempt_state_descriptor_hash().as_slice())
    .bind(checked_i64(
        OWNER_ACTION_ATTEMPT_STATE_MAXIMUM_BYTES,
        "attempt maximum payload size",
    )?)
    .bind(OWNER_ACTION_ATTEMPT_STATE_RETENTION_POLICY_ID)
    .bind(payload)
    .bind(attempt.planned_at_unix_nanos())
    .execute(&mut **transaction)
    .await
    .map_err(database_error)?;
    Ok(result.rows_affected() == 1)
}

async fn insert_outcome(
    transaction: &mut Transaction<'_, Postgres>,
    outcome: &PrivacyOwnerActionOutcome,
) -> Result<bool, SdkError> {
    let payload = encode_owner_action_outcome_state(outcome)?;
    let result = postgres_sqlx::query(
        r#"
        INSERT INTO crm.customer_privacy_owner_action_outcomes (
          tenant_id, privacy_case_id, action_plan_id, retention_decision_id,
          item_sequence, attempt_generation, outcome_id, outcome_digest,
          attempt_id, attempt_digest, owner_module_id, action_code, status,
          safe_failure_code, schema_id, schema_version, descriptor_hash,
          maximum_payload_size, retention_policy_id, payload_bytes,
          recorded_at_unix_nanos
        ) VALUES (
          $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,
          $19,$20,$21
        ) ON CONFLICT DO NOTHING
        "#,
    )
    .bind(outcome.tenant_id().as_str())
    .bind(outcome.privacy_case_id().as_str())
    .bind(outcome.action_plan_id().as_str())
    .bind(outcome.retention_decision_id().as_str())
    .bind(i32::try_from(outcome.item_sequence()).map_err(evidence_invalid)?)
    .bind(i32::try_from(outcome.attempt_generation()).map_err(evidence_invalid)?)
    .bind(outcome.outcome_id().as_str())
    .bind(outcome.digest().as_slice())
    .bind(outcome.attempt_id().as_str())
    .bind(outcome.attempt_digest().as_slice())
    .bind(outcome.owner_module_id().as_str())
    .bind(outcome.action_code())
    .bind(outcome.status().label())
    .bind(outcome.safe_failure_code())
    .bind(OWNER_ACTION_OUTCOME_STATE_SCHEMA_ID)
    .bind(OWNER_ACTION_OUTCOME_STATE_SCHEMA_VERSION)
    .bind(owner_action_outcome_state_descriptor_hash().as_slice())
    .bind(checked_i64(
        OWNER_ACTION_OUTCOME_STATE_MAXIMUM_BYTES,
        "outcome maximum payload size",
    )?)
    .bind(OWNER_ACTION_OUTCOME_STATE_RETENTION_POLICY_ID)
    .bind(payload)
    .bind(outcome.recorded_at_unix_nanos())
    .execute(&mut **transaction)
    .await
    .map_err(database_error)?;
    Ok(result.rows_affected() == 1)
}

async fn load_latest_attempt_with_outcome(
    transaction: &mut Transaction<'_, Postgres>,
    invocation: &OwnerExecutionInvocation,
    sequence: u32,
) -> Result<Option<(PrivacyOwnerActionAttempt, Option<PrivacyOwnerActionOutcome>)>, SdkError> {
    let row = postgres_sqlx::query(
        r#"
        SELECT a.attempt_id, a.payload_bytes AS attempt_payload,
               a.schema_id AS attempt_schema_id,
               a.schema_version AS attempt_schema_version,
               a.descriptor_hash AS attempt_descriptor_hash,
               a.maximum_payload_size AS attempt_maximum,
               a.retention_policy_id AS attempt_retention,
               o.outcome_id, o.payload_bytes AS outcome_payload,
               o.schema_id AS outcome_schema_id,
               o.schema_version AS outcome_schema_version,
               o.descriptor_hash AS outcome_descriptor_hash,
               o.maximum_payload_size AS outcome_maximum,
               o.retention_policy_id AS outcome_retention
        FROM crm.customer_privacy_owner_action_attempts a
        LEFT JOIN crm.customer_privacy_owner_action_outcomes o
          ON o.tenant_id = a.tenant_id
         AND o.privacy_case_id = a.privacy_case_id
         AND o.item_sequence = a.item_sequence
         AND o.attempt_generation = a.attempt_generation
        WHERE a.tenant_id = $1 AND a.privacy_case_id = $2
          AND a.item_sequence = $3
        ORDER BY a.attempt_generation DESC
        LIMIT 1
        FOR UPDATE OF a
        "#,
    )
    .bind(invocation.tenant_id.as_str())
    .bind(invocation.privacy_case_id.as_str())
    .bind(i32::try_from(sequence).map_err(evidence_invalid)?)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(database_error)?;
    row.map(|row| {
        let attempt = attempt_from_row(&row)?;
        let outcome = row
            .try_get::<Option<String>, _>("outcome_id")
            .map_err(database_error)?
            .map(|_| outcome_from_row(&row))
            .transpose()?;
        if let Some(outcome) = &outcome {
            validate_outcome_attempt(outcome, &attempt)?;
        }
        Ok((attempt, outcome))
    })
    .transpose()
}

async fn load_attempt_by_id(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: &TenantId,
    attempt_id: &RecordId,
) -> Result<Option<PrivacyOwnerActionAttempt>, SdkError> {
    postgres_sqlx::query(
        r#"
        SELECT attempt_id, payload_bytes AS attempt_payload,
               schema_id AS attempt_schema_id,
               schema_version AS attempt_schema_version,
               descriptor_hash AS attempt_descriptor_hash,
               maximum_payload_size AS attempt_maximum,
               retention_policy_id AS attempt_retention
        FROM crm.customer_privacy_owner_action_attempts
        WHERE tenant_id = $1 AND attempt_id = $2
        FOR SHARE
        "#,
    )
    .bind(tenant_id.as_str())
    .bind(attempt_id.as_str())
    .fetch_optional(&mut **transaction)
    .await
    .map_err(database_error)?
    .map(|row| attempt_from_row(&row))
    .transpose()
}

async fn load_outcome_by_id(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: &TenantId,
    outcome_id: &RecordId,
) -> Result<Option<PrivacyOwnerActionOutcome>, SdkError> {
    postgres_sqlx::query(
        r#"
        SELECT outcome_id, payload_bytes AS outcome_payload,
               schema_id AS outcome_schema_id,
               schema_version AS outcome_schema_version,
               descriptor_hash AS outcome_descriptor_hash,
               maximum_payload_size AS outcome_maximum,
               retention_policy_id AS outcome_retention
        FROM crm.customer_privacy_owner_action_outcomes
        WHERE tenant_id = $1 AND outcome_id = $2
        FOR SHARE
        "#,
    )
    .bind(tenant_id.as_str())
    .bind(outcome_id.as_str())
    .fetch_optional(&mut **transaction)
    .await
    .map_err(database_error)?
    .map(|row| outcome_from_row(&row))
    .transpose()
}

fn attempt_from_row(
    row: &postgres_sqlx::postgres::PgRow,
) -> Result<PrivacyOwnerActionAttempt, SdkError> {
    validate_execution_envelope(
        row,
        "attempt_schema_id",
        "attempt_schema_version",
        "attempt_descriptor_hash",
        "attempt_maximum",
        "attempt_retention",
        OWNER_ACTION_ATTEMPT_STATE_SCHEMA_ID,
        OWNER_ACTION_ATTEMPT_STATE_SCHEMA_VERSION,
        owner_action_attempt_state_descriptor_hash(),
        OWNER_ACTION_ATTEMPT_STATE_MAXIMUM_BYTES,
        OWNER_ACTION_ATTEMPT_STATE_RETENTION_POLICY_ID,
    )?;
    let bytes: Vec<u8> = row.try_get("attempt_payload").map_err(database_error)?;
    let attempt = decode_owner_action_attempt_state(&bytes)?;
    let envelope_id: String = row.try_get("attempt_id").map_err(database_error)?;
    if attempt.attempt_id().as_str() != envelope_id {
        return Err(evidence_invalid(
            "attempt identity differs from its persistence envelope",
        ));
    }
    Ok(attempt)
}

pub(crate) fn outcome_from_row(
    row: &postgres_sqlx::postgres::PgRow,
) -> Result<PrivacyOwnerActionOutcome, SdkError> {
    validate_execution_envelope(
        row,
        "outcome_schema_id",
        "outcome_schema_version",
        "outcome_descriptor_hash",
        "outcome_maximum",
        "outcome_retention",
        OWNER_ACTION_OUTCOME_STATE_SCHEMA_ID,
        OWNER_ACTION_OUTCOME_STATE_SCHEMA_VERSION,
        owner_action_outcome_state_descriptor_hash(),
        OWNER_ACTION_OUTCOME_STATE_MAXIMUM_BYTES,
        OWNER_ACTION_OUTCOME_STATE_RETENTION_POLICY_ID,
    )?;
    let bytes: Vec<u8> = row.try_get("outcome_payload").map_err(database_error)?;
    let outcome = decode_owner_action_outcome_state(&bytes)?;
    let envelope_id: String = row.try_get("outcome_id").map_err(database_error)?;
    if outcome.outcome_id().as_str() != envelope_id {
        return Err(evidence_invalid(
            "outcome identity differs from its persistence envelope",
        ));
    }
    Ok(outcome)
}

#[allow(clippy::too_many_arguments)]
fn validate_execution_envelope(
    row: &postgres_sqlx::postgres::PgRow,
    schema_id_column: &str,
    schema_version_column: &str,
    descriptor_column: &str,
    maximum_column: &str,
    retention_column: &str,
    schema_id: &str,
    schema_version: &str,
    descriptor_hash: [u8; 32],
    maximum: u64,
    retention_policy: &str,
) -> Result<(), SdkError> {
    let actual_descriptor: Vec<u8> = row.try_get(descriptor_column).map_err(database_error)?;
    if row
        .try_get::<String, _>(schema_id_column)
        .map_err(database_error)?
        != schema_id
        || row
            .try_get::<String, _>(schema_version_column)
            .map_err(database_error)?
            != schema_version
        || actual_descriptor.as_slice() != descriptor_hash
        || row
            .try_get::<i64, _>(maximum_column)
            .map_err(database_error)?
            != checked_i64(maximum, "execution envelope maximum")?
        || row
            .try_get::<String, _>(retention_column)
            .map_err(database_error)?
            != retention_policy
    {
        return Err(evidence_invalid(
            "owner execution persistence envelope differs from its governed contract",
        ));
    }
    Ok(())
}

fn validate_attempt_invocation(
    attempt: &PrivacyOwnerActionAttempt,
    invocation: &OwnerExecutionInvocation,
) -> Result<(), SdkError> {
    if attempt.tenant_id() != &invocation.tenant_id
        || attempt.privacy_case_id() != &invocation.privacy_case_id
        || attempt.action_plan_id() != &invocation.action_plan_id
        || attempt.retention_decision_id() != &invocation.retention_decision_id
    {
        return Err(evidence_invalid(
            "prepared attempt differs from its execution invocation",
        ));
    }
    Ok(())
}

fn validate_outcome_attempt(
    outcome: &PrivacyOwnerActionOutcome,
    attempt: &PrivacyOwnerActionAttempt,
) -> Result<(), SdkError> {
    if outcome.tenant_id() != attempt.tenant_id()
        || outcome.privacy_case_id() != attempt.privacy_case_id()
        || outcome.action_plan_id() != attempt.action_plan_id()
        || outcome.retention_decision_id() != attempt.retention_decision_id()
        || outcome.item_sequence() != attempt.item_sequence()
        || outcome.attempt_generation() != attempt.attempt_generation()
        || outcome.attempt_id() != attempt.attempt_id()
        || outcome.attempt_digest() != attempt.digest()
        || outcome.owner_module_id() != attempt.owner_module_id()
        || outcome.action_code() != attempt.action_code()
        || outcome.recorded_at_unix_nanos() < attempt.planned_at_unix_nanos()
    {
        return Err(evidence_invalid(
            "owner outcome differs from its immutable attempt lineage",
        ));
    }
    Ok(())
}

async fn append_execution_audit(
    transaction: &mut Transaction<'_, Postgres>,
    invocation: &OwnerExecutionInvocation,
    event_type: &str,
    attempt: Option<&PrivacyOwnerActionAttempt>,
    outcome: Option<&PrivacyOwnerActionOutcome>,
    next_sequence: Option<u32>,
    occurred_at_unix_nanos: i64,
) -> Result<(), SdkError> {
    let digest = execution_audit_digest(
        invocation,
        event_type,
        attempt,
        outcome,
        next_sequence,
        occurred_at_unix_nanos,
    );
    postgres_sqlx::query(
        r#"
        INSERT INTO crm.customer_privacy_owner_execution_audit (
          tenant_id, audit_digest, event_type, privacy_case_id,
          item_sequence, attempt_generation, attempt_id, outcome_id,
          next_sequence, actor_id, request_id, correlation_id, trace_id,
          occurred_at_unix_nanos
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(invocation.tenant_id.as_str())
    .bind(digest.as_slice())
    .bind(event_type)
    .bind(invocation.privacy_case_id.as_str())
    .bind(
        attempt
            .map(PrivacyOwnerActionAttempt::item_sequence)
            .map(i32::try_from)
            .transpose()
            .map_err(evidence_invalid)?,
    )
    .bind(
        attempt
            .map(PrivacyOwnerActionAttempt::attempt_generation)
            .map(i32::try_from)
            .transpose()
            .map_err(evidence_invalid)?,
    )
    .bind(attempt.map(|value| value.attempt_id().as_str()))
    .bind(outcome.map(|value| value.outcome_id().as_str()))
    .bind(
        next_sequence
            .map(i32::try_from)
            .transpose()
            .map_err(evidence_invalid)?,
    )
    .bind(invocation.actor_id.as_str())
    .bind(invocation.request_id.as_str())
    .bind(invocation.correlation_id.as_str())
    .bind(invocation.trace_id.as_str())
    .bind(occurred_at_unix_nanos)
    .execute(&mut **transaction)
    .await
    .map_err(database_error)?;
    Ok(())
}

fn execution_audit_digest(
    invocation: &OwnerExecutionInvocation,
    event_type: &str,
    attempt: Option<&PrivacyOwnerActionAttempt>,
    outcome: Option<&PrivacyOwnerActionOutcome>,
    next_sequence: Option<u32>,
    occurred_at_unix_nanos: i64,
) -> [u8; 32] {
    let mut bytes = Vec::new();
    for field in [
        b"crm.customer-privacy.owner-execution-audit/v1".as_slice(),
        invocation.tenant_id.as_str().as_bytes(),
        invocation.privacy_case_id.as_str().as_bytes(),
        invocation.action_plan_id.as_str().as_bytes(),
        invocation.retention_decision_id.as_str().as_bytes(),
        event_type.as_bytes(),
        invocation.actor_id.as_str().as_bytes(),
        invocation.request_id.as_str().as_bytes(),
        invocation.correlation_id.as_str().as_bytes(),
        invocation.trace_id.as_str().as_bytes(),
    ] {
        append_digest_field(&mut bytes, field);
    }
    if let Some(attempt) = attempt {
        append_digest_field(&mut bytes, attempt.digest());
    }
    if let Some(outcome) = outcome {
        append_digest_field(&mut bytes, outcome.digest());
    }
    append_digest_field(&mut bytes, &next_sequence.unwrap_or_default().to_be_bytes());
    append_digest_field(&mut bytes, &occurred_at_unix_nanos.to_be_bytes());
    discovery_sha256(&bytes)
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
            let snapshot = decode_record_snapshot(
                ACTION_PLAN_RECORD_TYPE,
                plan_id.clone(),
                DataClass::Confidential,
                row,
            )?;
            validate_record_contract(
                &snapshot,
                ACTION_PLAN_STATE_SCHEMA_ID,
                ACTION_PLAN_STATE_SCHEMA_VERSION,
                action_plan_state_descriptor_hash(),
                ACTION_PLAN_STATE_MAXIMUM_BYTES,
                ACTION_PLAN_STATE_RETENTION_POLICY_ID,
            )?;
            let plan = decode_action_plan_state(&snapshot.payload.bytes)?;
            if plan.plan_id() != plan_id || plan.lineage().tenant_id() != tenant_id {
                return Err(evidence_invalid(
                    "action plan differs from its persistence envelope",
                ));
            }
            Ok(plan)
        })
        .transpose()
}

async fn load_decision(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: &TenantId,
    decision_id: &RecordId,
) -> Result<Option<PrivacyRetentionDecisionSet>, SdkError> {
    postgres_sqlx::query(RECORD_SELECT_FOR_SHARE_SQL)
        .bind(tenant_id.as_str())
        .bind(MODULE_ID)
        .bind(RETENTION_DECISION_RECORD_TYPE)
        .bind(decision_id.as_str())
        .fetch_optional(&mut **transaction)
        .await
        .map_err(database_error)?
        .map(|row| {
            retention_decision_from_snapshot(&decode_record_snapshot(
                RETENTION_DECISION_RECORD_TYPE,
                decision_id.clone(),
                DataClass::Personal,
                row,
            )?)
        })
        .transpose()
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

async fn update_case_record(
    transaction: &mut Transaction<'_, Postgres>,
    invocation: &OwnerExecutionInvocation,
    expected_version: u64,
    privacy_case: &PrivacyCase,
) -> Result<(), SdkError> {
    let business_transaction_id =
        case_transition_transaction_id(invocation, privacy_case.version());
    bind_business_transaction(transaction, &business_transaction_id).await?;
    let payload = privacy_case_persisted_payload(privacy_case)?;
    let result = postgres_sqlx::query(
        r#"
        UPDATE crm.records
        SET version = $5,
            schema_id = $6,
            schema_version = $7,
            descriptor_hash = $8,
            data_class = 'confidential',
            payload_encoding = 'json',
            maximum_payload_size = $9,
            retention_policy_id = $10,
            payload_bytes = $11,
            last_business_transaction_id = $12,
            updated_at = clock_timestamp()
        WHERE tenant_id = $1 AND owner_module_id = $2
          AND record_type = $3 AND record_id = $4
          AND version = $13 AND deleted_at IS NULL
        "#,
    )
    .bind(privacy_case.tenant_id().as_str())
    .bind(MODULE_ID)
    .bind(PRIVACY_CASE_RECORD_TYPE)
    .bind(privacy_case.case_id().as_str())
    .bind(checked_i64(
        privacy_case.version(),
        "resulting case version",
    )?)
    .bind(payload.schema_id.as_str())
    .bind(payload.schema_version.as_str())
    .bind(payload.descriptor_hash.as_slice())
    .bind(checked_i64(
        payload.maximum_size_bytes,
        "case maximum payload size",
    )?)
    .bind(payload.retention_policy_id.as_str())
    .bind(payload.bytes.as_slice())
    .bind(&business_transaction_id)
    .bind(checked_i64(expected_version, "expected case version")?)
    .execute(&mut **transaction)
    .await
    .map_err(database_error)?;
    if result.rows_affected() != 1 {
        return Err(execution_conflict(
            "privacy case changed before owner execution could commit",
        ));
    }
    insert_case_transition_evidence(
        transaction,
        invocation,
        privacy_case,
        &payload,
        &business_transaction_id,
    )
    .await?;
    Ok(())
}

fn decode_record_snapshot(
    record_type: &str,
    record_id: RecordId,
    expected_data_class: DataClass,
    row: postgres_sqlx::postgres::PgRow,
) -> Result<RecordSnapshot, SdkError> {
    let version = row.try_get("version").map_err(database_error)?;
    let data_class: String = row.try_get("data_class").map_err(database_error)?;
    let encoding: String = row.try_get("payload_encoding").map_err(database_error)?;
    if data_class != data_class_label(expected_data_class) || encoding != "json" {
        return Err(evidence_invalid(
            "record data class or encoding differs from its governed contract",
        ));
    }
    let descriptor: Vec<u8> = row.try_get("descriptor_hash").map_err(database_error)?;
    let descriptor_hash: [u8; 32] = descriptor
        .try_into()
        .map_err(|_| evidence_invalid("record descriptor hash must contain 32 bytes"))?;
    let maximum: i64 = row
        .try_get("maximum_payload_size")
        .map_err(database_error)?;
    Ok(RecordSnapshot {
        reference: RecordRef {
            record_type: RecordType::try_new(record_type).map_err(evidence_invalid)?,
            record_id,
        },
        version,
        payload: TypedPayload {
            owner: ModuleId::try_new(
                row.try_get::<String, _>("owner_module_id")
                    .map_err(database_error)?,
            )
            .map_err(evidence_invalid)?,
            schema_id: SchemaId::try_new(
                row.try_get::<String, _>("schema_id")
                    .map_err(database_error)?,
            )
            .map_err(evidence_invalid)?,
            schema_version: SchemaVersion::try_new(
                row.try_get::<String, _>("schema_version")
                    .map_err(database_error)?,
            )
            .map_err(evidence_invalid)?,
            descriptor_hash,
            data_class: expected_data_class,
            encoding: PayloadEncoding::Json,
            maximum_size_bytes: u64::try_from(maximum)
                .map_err(|_| evidence_invalid("record maximum size is negative"))?,
            retention_policy_id: RetentionPolicyId::try_new(
                row.try_get::<String, _>("retention_policy_id")
                    .map_err(database_error)?,
            )
            .map_err(evidence_invalid)?,
            bytes: row.try_get("payload_bytes").map_err(database_error)?,
        },
    })
}

fn validate_record_contract(
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
        return Err(evidence_invalid(
            "record persistence envelope differs from its governed contract",
        ));
    }
    Ok(())
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

fn positive_u64(row: &postgres_sqlx::postgres::PgRow, column: &str) -> Result<u64, SdkError> {
    let value: i64 = row.try_get(column).map_err(database_error)?;
    if value <= 0 {
        return Err(evidence_invalid(format!("{column} is not positive")));
    }
    u64::try_from(value).map_err(|_| evidence_invalid(format!("{column} exceeds u64")))
}

fn optional_positive_u64(
    row: &postgres_sqlx::postgres::PgRow,
    column: &str,
) -> Result<Option<u64>, SdkError> {
    row.try_get::<Option<i64>, _>(column)
        .map_err(database_error)?
        .map(|value| {
            if value <= 0 {
                return Err(evidence_invalid(format!("{column} is not positive")));
            }
            u64::try_from(value).map_err(|_| evidence_invalid(format!("{column} exceeds u64")))
        })
        .transpose()
}

fn bounded_u32(
    row: &postgres_sqlx::postgres::PgRow,
    column: &str,
    minimum: u32,
    maximum: u32,
) -> Result<u32, SdkError> {
    let value: i32 = row.try_get(column).map_err(database_error)?;
    let value =
        u32::try_from(value).map_err(|_| evidence_invalid(format!("{column} is negative")))?;
    if value < minimum || value > maximum {
        return Err(evidence_invalid(format!("{column} is out of bounds")));
    }
    Ok(value)
}

fn positive_i64(row: &postgres_sqlx::postgres::PgRow, column: &str) -> Result<i64, SdkError> {
    let value: i64 = row.try_get(column).map_err(database_error)?;
    if value <= 0 {
        return Err(evidence_invalid(format!("{column} is not positive")));
    }
    Ok(value)
}

fn digest(row: &postgres_sqlx::postgres::PgRow, column: &str) -> Result<[u8; 32], SdkError> {
    row.try_get::<Vec<u8>, _>(column)
        .map_err(database_error)?
        .try_into()
        .map_err(|_| evidence_invalid(format!("{column} must contain 32 bytes")))
}

fn identifier(value: String) -> Result<RecordId, SdkError> {
    RecordId::try_new(value).map_err(evidence_invalid)
}

fn checked_i64(value: u64, label: &str) -> Result<i64, SdkError> {
    i64::try_from(value).map_err(|_| evidence_invalid(format!("{label} exceeds i64")))
}

fn append_digest_field(output: &mut Vec<u8>, value: &[u8]) {
    output.extend_from_slice(&(value.len() as u64).to_be_bytes());
    output.extend_from_slice(value);
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

async fn insert_case_transition_evidence(
    transaction: &mut Transaction<'_, Postgres>,
    invocation: &OwnerExecutionInvocation,
    privacy_case: &PrivacyCase,
    payload: &TypedPayload,
    business_transaction_id: &str,
) -> Result<(), SdkError> {
    let resulting_version = checked_i64(privacy_case.version(), "case transition version")?;
    let maximum = checked_i64(
        payload.maximum_size_bytes,
        "case transition maximum payload size",
    )?;
    let suffix = &hex(&discovery_sha256(business_transaction_id.as_bytes()))[..24];
    let event_id = format!("privacy-owner-execution-event-{suffix}");
    let audit_id = format!("privacy-owner-execution-audit-{suffix}");
    let request_hash =
        case_transition_request_hash(invocation, privacy_case, payload, business_transaction_id);

    postgres_sqlx::query(
        r#"
        INSERT INTO crm.idempotency_records (
          tenant_id, idempotency_scope, idempotency_key, request_hash,
          status, business_transaction_id, expires_at
        ) VALUES ($1,$2,$3,$4,'completed',$5,clock_timestamp() + INTERVAL '24 hours')
        "#,
    )
    .bind(invocation.tenant_id.as_str())
    .bind(EXECUTION_CASE_IDEMPOTENCY_SCOPE)
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
          $12,'json',$13,$14,$15,
          TIMESTAMPTZ 'epoch' + ($16::bigint / 1000) * INTERVAL '1 microsecond'
        )
        "#,
    )
    .bind(invocation.tenant_id.as_str())
    .bind(&event_id)
    .bind(business_transaction_id)
    .bind(PRIVACY_CASE_RECORD_TYPE)
    .bind(privacy_case.case_id().as_str())
    .bind(resulting_version)
    .bind(EXECUTION_CASE_EVENT_TYPE)
    .bind(&event_id)
    .bind(payload.schema_id.as_str())
    .bind(payload.schema_version.as_str())
    .bind(payload.descriptor_hash.as_slice())
    .bind(data_class_label(payload.data_class))
    .bind(maximum)
    .bind(payload.retention_policy_id.as_str())
    .bind(payload.bytes.as_slice())
    .bind(invocation.planned_at_unix_nanos)
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
                return Err(evidence_invalid(
                    "tenant audit next sequence must be positive",
                ));
            }
            let previous_hash = row
                .try_get::<Vec<u8>, _>("last_hash")
                .map_err(database_error)?
                .try_into()
                .map_err(|_| evidence_invalid("tenant audit hash must contain 32 bytes"))?;
            (sequence, previous_hash)
        }
        None => (1, [0; 32]),
    };
    let occurred_at = (invocation.planned_at_unix_nanos / 1_000) * 1_000;
    let audit_hash = case_transition_audit_hash(
        invocation,
        sequence,
        previous_hash,
        &audit_id,
        business_transaction_id,
        payload.bytes.as_slice(),
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
    .bind(payload.bytes.as_slice())
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

fn case_transition_request_hash(
    invocation: &OwnerExecutionInvocation,
    privacy_case: &PrivacyCase,
    payload: &TypedPayload,
    business_transaction_id: &str,
) -> [u8; 32] {
    let resulting_version = privacy_case.version().to_be_bytes();
    let mut bytes = Vec::new();
    for value in [
        b"crm.customer-privacy.owner-execution.case-transition-request/v1".as_slice(),
        invocation.tenant_id.as_str().as_bytes(),
        invocation.privacy_case_id.as_str().as_bytes(),
        invocation.action_plan_id.as_str().as_bytes(),
        invocation.retention_decision_id.as_str().as_bytes(),
        invocation.actor_id.as_str().as_bytes(),
        invocation.request_id.as_str().as_bytes(),
        invocation.correlation_id.as_str().as_bytes(),
        invocation.trace_id.as_str().as_bytes(),
        invocation.initiating_capability_id.as_str().as_bytes(),
        invocation.initiating_capability_version.as_str().as_bytes(),
        business_transaction_id.as_bytes(),
        resulting_version.as_slice(),
        payload.bytes.as_slice(),
    ] {
        append_digest_field(&mut bytes, value);
    }
    discovery_sha256(&bytes)
}

fn case_transition_audit_hash(
    invocation: &OwnerExecutionInvocation,
    sequence: i64,
    previous_hash: [u8; 32],
    audit_id: &str,
    business_transaction_id: &str,
    canonical_envelope: &[u8],
    occurred_at: i64,
) -> [u8; 32] {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"crm.audit.record.sha256/v1");
    append_digest_field(&mut bytes, invocation.tenant_id.as_str().as_bytes());
    bytes.extend_from_slice(&sequence.to_be_bytes());
    for value in [
        audit_id.as_bytes(),
        business_transaction_id.as_bytes(),
        invocation.actor_id.as_str().as_bytes(),
        invocation.initiating_capability_id.as_str().as_bytes(),
        invocation.initiating_capability_version.as_str().as_bytes(),
        AUDIT_CANONICALIZATION_PROFILE.as_bytes(),
    ] {
        append_digest_field(&mut bytes, value);
    }
    bytes.extend_from_slice(&previous_hash);
    append_digest_field(&mut bytes, canonical_envelope);
    bytes.extend_from_slice(&occurred_at.to_be_bytes());
    discovery_sha256(&bytes)
}

fn case_transition_transaction_id(
    invocation: &OwnerExecutionInvocation,
    resulting_case_version: u64,
) -> String {
    let resulting_case_version = resulting_case_version.to_be_bytes();
    let mut bytes = Vec::new();
    for value in [
        b"crm.customer-privacy.owner-execution.case-transition/v1".as_slice(),
        invocation.tenant_id.as_str().as_bytes(),
        invocation.privacy_case_id.as_str().as_bytes(),
        invocation.initiating_capability_id.as_str().as_bytes(),
        invocation.initiating_capability_version.as_str().as_bytes(),
        invocation.request_id.as_str().as_bytes(),
        resulting_case_version.as_slice(),
    ] {
        append_digest_field(&mut bytes, value);
    }
    let digest = discovery_sha256(&bytes);
    format!("privacy-owner-transition-{}", hex(&digest[..12]))
}

fn transaction_id(invocation: &OwnerExecutionInvocation) -> String {
    let mut bytes = Vec::new();
    for value in [
        invocation.initiating_capability_id.as_str().as_bytes(),
        invocation.initiating_capability_version.as_str().as_bytes(),
        invocation.request_id.as_str().as_bytes(),
    ] {
        append_digest_field(&mut bytes, value);
    }
    let digest = discovery_sha256(&bytes);
    format!("privacy-owner-execution-{}", hex(&digest[..12]))
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

fn case_not_found() -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_CASE_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The requested privacy case was not found.",
    )
}

fn domain_error(error: impl std::fmt::Display) -> SdkError {
    execution_conflict(error.to_string())
}

fn execution_conflict(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_OWNER_EXECUTION_CONFLICT",
        ErrorCategory::Conflict,
        false,
        "Customer Privacy owner execution conflicts with durable evidence.",
    )
    .with_internal_reference(reference)
}

fn evidence_invalid(reference: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_OWNER_EXECUTION_EVIDENCE_INVALID",
        ErrorCategory::Internal,
        false,
        "Customer Privacy owner-execution evidence is invalid.",
    )
    .with_internal_reference(reference.to_string())
}

fn database_error(error: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_OWNER_EXECUTION_STORE_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "Customer Privacy owner-execution storage is unavailable.",
    )
    .with_internal_reference(error.to_string())
}
