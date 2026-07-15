use crate::audit::{AuditMaterializationError, MaterializedAuditRecord, materialize_audit_chain};
use crate::{AuditIntent, DataError, PostgresDataStore};
use crm_module_sdk::ModuleExecutionContext;
use sqlx::{Postgres, Transaction};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditedReadPlan {
    pub context: ModuleExecutionContext,
    pub audit: AuditIntent,
}

impl AuditedReadPlan {
    pub fn validate(&self) -> Result<(), DataError> {
        self.context.validate().map_err(DataError::Sdk)?;
        self.audit.validate().map_err(DataError::InvalidPlan)
    }
}

impl PostgresDataStore {
    /// Atomically appends one tamper-evident audit record for a governed read/disclosure.
    ///
    /// Read operations intentionally produce no outbox event and no idempotency claim. The
    /// matching business-transaction marker still proves that exactly one audit record was
    /// expected and committed for the disclosure before the caller releases protected bytes.
    pub async fn record_audited_read(&self, plan: &AuditedReadPlan) -> Result<(), DataError> {
        plan.validate()?;
        let mut transaction = self.pool().begin().await?;
        bind_execution_context(&mut transaction, &plan.context).await?;
        let materialized = materialize_audit_chain(
            &mut transaction,
            &plan.context,
            std::slice::from_ref(&plan.audit),
        )
        .await
        .map_err(audit_materialization_to_data_error)?;
        insert_audit_record(&mut transaction, &plan.context, &materialized[0]).await?;
        insert_completion_marker(&mut transaction, &plan.context).await?;
        transaction.commit().await?;
        Ok(())
    }
}

async fn bind_execution_context(
    transaction: &mut Transaction<'_, Postgres>,
    context: &ModuleExecutionContext,
) -> Result<(), DataError> {
    sqlx::query(
        r#"
        SELECT
          set_config('app.tenant_id', $1, true),
          set_config('app.actor_id', $2, true),
          set_config('app.request_id', $3, true),
          set_config('app.capability_id', $4, true),
          set_config('app.capability_version', $5, true),
          set_config('app.business_transaction_id', $6, true)
        "#,
    )
    .bind(context.execution.tenant_id.as_str())
    .bind(context.execution.actor_id.as_str())
    .bind(context.execution.request_id.as_str())
    .bind(context.execution.capability_id.as_str())
    .bind(context.execution.capability_version.as_str())
    .bind(context.execution.business_transaction_id.as_str())
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn insert_audit_record(
    transaction: &mut Transaction<'_, Postgres>,
    context: &ModuleExecutionContext,
    audit: &MaterializedAuditRecord,
) -> Result<(), DataError> {
    sqlx::query(
        r#"
        INSERT INTO crm.audit_records (
          tenant_id, audit_sequence, audit_record_id, business_transaction_id,
          actor_id, capability_id, capability_version, canonicalization_profile,
          previous_hash, record_hash, canonical_envelope, occurred_at
        )
        VALUES (
          $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11,
          TIMESTAMPTZ 'epoch' + ($12::bigint / 1000) * INTERVAL '1 microsecond'
        )
        "#,
    )
    .bind(context.execution.tenant_id.as_str())
    .bind(audit.audit_sequence)
    .bind(&audit.audit_record_id)
    .bind(context.execution.business_transaction_id.as_str())
    .bind(context.execution.actor_id.as_str())
    .bind(context.execution.capability_id.as_str())
    .bind(context.execution.capability_version.as_str())
    .bind(&audit.canonicalization_profile)
    .bind(audit.previous_hash.as_slice())
    .bind(audit.record_hash.as_slice())
    .bind(audit.canonical_envelope.as_slice())
    .bind(audit.occurred_at_unix_nanos)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn insert_completion_marker(
    transaction: &mut Transaction<'_, Postgres>,
    context: &ModuleExecutionContext,
) -> Result<(), DataError> {
    sqlx::query(
        r#"
        INSERT INTO crm.business_transactions (
          tenant_id, business_transaction_id, actor_id, request_id,
          correlation_id, trace_id, capability_id, capability_version,
          expected_outbox_events, expected_audit_records, expected_idempotency_records
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 0, 1, 0)
        "#,
    )
    .bind(context.execution.tenant_id.as_str())
    .bind(context.execution.business_transaction_id.as_str())
    .bind(context.execution.actor_id.as_str())
    .bind(context.execution.request_id.as_str())
    .bind(context.execution.correlation_id.as_str())
    .bind(context.execution.trace_id.as_str())
    .bind(context.execution.capability_id.as_str())
    .bind(context.execution.capability_version.as_str())
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

fn audit_materialization_to_data_error(error: AuditMaterializationError) -> DataError {
    match error {
        AuditMaterializationError::Database(error) => DataError::Database(error),
        AuditMaterializationError::InvalidIntent(message) => DataError::InvalidPlan(message),
        AuditMaterializationError::InvalidStoredValue(message) => {
            DataError::InvalidStoredValue(message)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_module_sdk::{
        ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId,
        CorrelationId, ExecutionContext, IdempotencyKey, ModuleId, RequestId, SchemaVersion,
        TenantId, TraceId,
    };

    #[test]
    fn audited_read_plan_requires_valid_context_and_audit_evidence() {
        let plan = AuditedReadPlan {
            context: ModuleExecutionContext {
                module_id: ModuleId::try_new("crm.test").unwrap(),
                execution: ExecutionContext {
                    tenant_id: TenantId::try_new("tenant-a").unwrap(),
                    actor_id: ActorId::try_new("actor-a").unwrap(),
                    request_id: RequestId::try_new("request-a").unwrap(),
                    correlation_id: CorrelationId::try_new("correlation-a").unwrap(),
                    causation_id: CausationId::try_new("causation-a").unwrap(),
                    trace_id: TraceId::try_new("trace-a").unwrap(),
                    capability_id: CapabilityId::try_new("test.read").unwrap(),
                    capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                    idempotency_key: IdempotencyKey::try_new("read-request-a").unwrap(),
                    business_transaction_id: BusinessTransactionId::try_new("read-tx-a").unwrap(),
                    schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                    request_started_at_unix_nanos: 100,
                },
            },
            audit: AuditIntent {
                audit_record_id: "audit-read-a".to_owned(),
                canonicalization_profile: "crm.cjson/v1".to_owned(),
                canonical_envelope: br#"{"operation":"read"}"#.to_vec(),
                occurred_at_unix_nanos: 100,
            },
        };
        assert!(plan.validate().is_ok());
    }
}
