use crate::execution::PostgresOwnerExecutionPersistence;
use crm_core_data::postgres_sqlx::{self, Row};
use crm_customer_privacy::{
    MODULE_ID, PRIVACY_CASE_RECORD_TYPE, RETENTION_DECISION_RECORD_TYPE,
    decode_retention_decision_state,
};
use crm_customer_privacy_application::{
    CheckpointAdvance, ExecutionPreparation, OwnerExecutionInvocation,
    OwnerExecutionPersistencePort, RETENTION_APPROVAL_TRIGGER_CAPABILITY,
    RETENTION_LEGAL_HOLD_TRIGGER_CAPABILITY, RETENTION_TRIGGER_CAPABILITY_VERSION,
};
use crm_module_sdk::{
    ActorId, CapabilityId, CapabilityVersion, CorrelationId, ErrorCategory, PortFuture, RecordId,
    RequestId, SdkError, TenantId, TraceId,
};
use std::sync::Arc;

const MAXIMUM_READY_WORK_ITEMS: u32 = 1_024;

const READY_WORK_SQL: &str = r#"
WITH decision_records AS (
  SELECT d.tenant_id,
         d.record_id AS retention_decision_id,
         d.last_business_transaction_id,
         d.created_at,
         d.payload_bytes AS decision_payload,
         convert_from(d.payload_bytes, 'UTF8')::jsonb AS decision_state
  FROM crm.records d
  WHERE d.tenant_id = $1
    AND d.owner_module_id = $2
    AND d.record_type = $3
    AND d.deleted_at IS NULL
),
incomplete AS (
  SELECT 0::integer AS priority,
         checkpoint.updated_at AS ready_at,
         checkpoint.privacy_case_id,
         checkpoint.action_plan_id,
         checkpoint.retention_decision_id,
         decision.last_business_transaction_id,
         decision.decision_payload,
         decision.decision_state
  FROM crm.customer_privacy_owner_execution_checkpoints checkpoint
  JOIN decision_records decision
    ON decision.tenant_id = checkpoint.tenant_id
   AND decision.retention_decision_id = checkpoint.retention_decision_id
  WHERE checkpoint.tenant_id = $1
    AND checkpoint.completed_at_unix_nanos IS NULL
),
newest_planned AS (
  SELECT DISTINCT ON (decision.decision_state ->> 'privacy_case_id')
         1::integer AS priority,
         decision.created_at AS ready_at,
         decision.decision_state ->> 'privacy_case_id' AS privacy_case_id,
         decision.decision_state ->> 'action_plan_id' AS action_plan_id,
         decision.retention_decision_id,
         decision.last_business_transaction_id,
         decision.decision_payload,
         decision.decision_state
  FROM decision_records decision
  JOIN crm.records privacy_case
    ON privacy_case.tenant_id = decision.tenant_id
   AND privacy_case.owner_module_id = $2
   AND privacy_case.record_type = $4
   AND privacy_case.record_id = decision.decision_state ->> 'privacy_case_id'
   AND privacy_case.deleted_at IS NULL
  LEFT JOIN crm.customer_privacy_owner_execution_checkpoints checkpoint
    ON checkpoint.tenant_id = decision.tenant_id
   AND checkpoint.privacy_case_id = decision.decision_state ->> 'privacy_case_id'
  WHERE checkpoint.privacy_case_id IS NULL
    AND convert_from(privacy_case.payload_bytes, 'UTF8')::jsonb
          -> 'status' ->> 'code' = 'planned'
    AND convert_from(privacy_case.payload_bytes, 'UTF8')::jsonb
          ->> 'action_plan_id' = decision.decision_state ->> 'action_plan_id'
  ORDER BY decision.decision_state ->> 'privacy_case_id',
           (decision.decision_state ->> 'evaluated_at_unix_nanos')::bigint DESC,
           decision.retention_decision_id DESC
),
candidates AS (
  SELECT * FROM incomplete
  UNION ALL
  SELECT * FROM newest_planned
)
SELECT candidate.privacy_case_id,
       candidate.action_plan_id,
       candidate.retention_decision_id,
       candidate.decision_payload,
       source_transaction.actor_id,
       source_transaction.request_id,
       source_transaction.correlation_id,
       source_transaction.trace_id,
       source_transaction.capability_id,
       source_transaction.capability_version
FROM candidates candidate
JOIN crm.business_transactions source_transaction
  ON source_transaction.tenant_id = $1
 AND source_transaction.business_transaction_id = candidate.last_business_transaction_id
WHERE (candidate.decision_state ->> 'evaluated_at_unix_nanos')::bigint > 0
  AND (candidate.decision_state ->> 'evaluated_at_unix_nanos')::bigint <= $5
  AND source_transaction.capability_version = $6
  AND source_transaction.capability_id IN ($7, $8)
ORDER BY candidate.priority ASC,
         candidate.ready_at ASC,
         candidate.privacy_case_id ASC,
         candidate.retention_decision_id ASC
LIMIT $9
"#;

struct ReadyPostgresOwnerExecutionPersistence {
    inner: PostgresOwnerExecutionPersistence,
}

impl From<PostgresOwnerExecutionPersistence> for Arc<dyn OwnerExecutionPersistencePort> {
    fn from(inner: PostgresOwnerExecutionPersistence) -> Self {
        Arc::new(ReadyPostgresOwnerExecutionPersistence { inner })
    }
}

impl OwnerExecutionPersistencePort for ReadyPostgresOwnerExecutionPersistence {
    fn load_ready<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        now_unix_nanos: i64,
        maximum_items: u32,
    ) -> PortFuture<'a, Result<Vec<OwnerExecutionInvocation>, SdkError>> {
        Box::pin(async move {
            if now_unix_nanos <= 0 {
                return Err(ready_work_invalid(
                    "ready-work time must be after the Unix epoch",
                ));
            }
            if maximum_items == 0 || maximum_items > MAXIMUM_READY_WORK_ITEMS {
                return Err(ready_work_invalid(
                    "ready-work limit must be between one and the frozen maximum",
                ));
            }
            let maximum_items = i64::from(maximum_items);
            let mut transaction = self
                .inner
                .store()
                .pool()
                .begin()
                .await
                .map_err(database_error)?;
            postgres_sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
                .bind(tenant_id.as_str())
                .execute(&mut *transaction)
                .await
                .map_err(database_error)?;
            let rows = postgres_sqlx::query(READY_WORK_SQL)
                .bind(tenant_id.as_str())
                .bind(MODULE_ID)
                .bind(RETENTION_DECISION_RECORD_TYPE)
                .bind(PRIVACY_CASE_RECORD_TYPE)
                .bind(now_unix_nanos)
                .bind(RETENTION_TRIGGER_CAPABILITY_VERSION)
                .bind(RETENTION_APPROVAL_TRIGGER_CAPABILITY)
                .bind(RETENTION_LEGAL_HOLD_TRIGGER_CAPABILITY)
                .bind(maximum_items)
                .fetch_all(&mut *transaction)
                .await
                .map_err(database_error)?;
            let work = rows
                .into_iter()
                .map(|row| ready_invocation(tenant_id, now_unix_nanos, row))
                .collect::<Result<Vec<_>, _>>()?;
            transaction.commit().await.map_err(database_error)?;
            Ok(work)
        })
    }

    fn prepare_next<'a>(
        &'a self,
        invocation: &'a OwnerExecutionInvocation,
    ) -> PortFuture<'a, Result<ExecutionPreparation, SdkError>> {
        self.inner.prepare_next(invocation)
    }

    fn record_outcome<'a>(
        &'a self,
        invocation: &'a OwnerExecutionInvocation,
        attempt: &'a crm_customer_privacy::PrivacyOwnerActionAttempt,
        outcome: &'a crm_customer_privacy::PrivacyOwnerActionOutcome,
    ) -> PortFuture<'a, Result<bool, SdkError>> {
        self.inner.record_outcome(invocation, attempt, outcome)
    }

    fn advance_checkpoint<'a>(
        &'a self,
        invocation: &'a OwnerExecutionInvocation,
    ) -> PortFuture<'a, Result<CheckpointAdvance, SdkError>> {
        self.inner.advance_checkpoint(invocation)
    }
}

fn ready_invocation(
    tenant_id: &TenantId,
    now_unix_nanos: i64,
    row: postgres_sqlx::postgres::PgRow,
) -> Result<OwnerExecutionInvocation, SdkError> {
    let decision_bytes = row
        .try_get::<Vec<u8>, _>("decision_payload")
        .map_err(database_error)?;
    let decision =
        decode_retention_decision_state(&decision_bytes).map_err(ready_evidence_invalid)?;
    let privacy_case_id = identifier::<RecordId>(&row, "privacy_case_id")?;
    let action_plan_id = identifier::<RecordId>(&row, "action_plan_id")?;
    let retention_decision_id = identifier::<RecordId>(&row, "retention_decision_id")?;
    if decision.tenant_id() != tenant_id
        || decision.privacy_case_id() != &privacy_case_id
        || decision.action_plan_id() != &action_plan_id
        || decision.decision_id() != &retention_decision_id
        || decision.evaluated_at_unix_nanos() <= 0
        || decision.evaluated_at_unix_nanos() > now_unix_nanos
    {
        return Err(ready_evidence_invalid(
            "ready-work row differs from canonical retention-decision lineage",
        ));
    }
    let capability_id = identifier::<CapabilityId>(&row, "capability_id")?;
    let capability_version = identifier::<CapabilityVersion>(&row, "capability_version")?;
    if capability_version.as_str() != RETENTION_TRIGGER_CAPABILITY_VERSION
        || !matches!(
            capability_id.as_str(),
            RETENTION_APPROVAL_TRIGGER_CAPABILITY | RETENTION_LEGAL_HOLD_TRIGGER_CAPABILITY
        )
    {
        return Err(ready_evidence_invalid(
            "ready-work transaction is outside the governed retention trigger set",
        ));
    }
    Ok(OwnerExecutionInvocation {
        tenant_id: tenant_id.clone(),
        privacy_case_id,
        action_plan_id,
        retention_decision_id,
        actor_id: identifier::<ActorId>(&row, "actor_id")?,
        request_id: identifier::<RequestId>(&row, "request_id")?,
        correlation_id: identifier::<CorrelationId>(&row, "correlation_id")?,
        trace_id: identifier::<TraceId>(&row, "trace_id")?,
        initiating_capability_id: capability_id,
        initiating_capability_version: capability_version,
        request_started_at_unix_nanos: decision.evaluated_at_unix_nanos(),
        planned_at_unix_nanos: now_unix_nanos,
        trusted_internal: true,
    })
}

trait ReadyIdentifier: Sized {
    fn parse(value: String) -> Result<Self, crm_module_sdk::IdentifierError>;
}

macro_rules! ready_identifier {
    ($type:ty) => {
        impl ReadyIdentifier for $type {
            fn parse(value: String) -> Result<Self, crm_module_sdk::IdentifierError> {
                <$type>::try_new(value)
            }
        }
    };
}

ready_identifier!(RecordId);
ready_identifier!(ActorId);
ready_identifier!(RequestId);
ready_identifier!(CorrelationId);
ready_identifier!(TraceId);
ready_identifier!(CapabilityId);
ready_identifier!(CapabilityVersion);

fn identifier<T: ReadyIdentifier>(
    row: &postgres_sqlx::postgres::PgRow,
    column: &str,
) -> Result<T, SdkError> {
    let value = row.try_get::<String, _>(column).map_err(database_error)?;
    T::parse(value).map_err(ready_evidence_invalid)
}

fn ready_work_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_OWNER_EXECUTION_READY_WORK_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "The Customer Privacy owner-execution ready-work request is invalid.",
    )
    .with_internal_reference(reference)
}

fn ready_evidence_invalid(reference: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_OWNER_EXECUTION_READY_EVIDENCE_INVALID",
        ErrorCategory::Internal,
        false,
        "Customer Privacy owner-execution ready-work evidence is invalid.",
    )
    .with_internal_reference(reference.to_string())
}

fn database_error(reference: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_OWNER_EXECUTION_STORE_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "Customer Privacy owner-execution storage is unavailable.",
    )
    .with_internal_reference(reference.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_work_sql_is_tenant_bound_bounded_and_lineage_joined() {
        for marker in [
            "d.tenant_id = $1",
            "checkpoint.tenant_id = $1",
            "source_transaction.tenant_id = $1",
            "source_transaction.business_transaction_id = candidate.last_business_transaction_id",
            "source_transaction.capability_id IN ($7, $8)",
            "LIMIT $9",
        ] {
            assert!(
                READY_WORK_SQL.contains(marker),
                "missing SQL guard: {marker}"
            );
        }
        assert!(READY_WORK_SQL.contains("DISTINCT ON"));
        assert!(READY_WORK_SQL.contains("completed_at_unix_nanos IS NULL"));
    }
}
