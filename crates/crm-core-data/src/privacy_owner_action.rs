use crate::audit::{AuditMaterializationError, materialize_audit_chain};
use crate::capability_executor::{
    capability_batch_error_to_sdk, validate_executor_definition,
    validate_transactional_aggregate_execution_plan,
};
use crate::postgres_batch::{
    bind_execution_context, capability_idempotency, complete_capability_idempotency,
    insert_audit_record, insert_completion_marker, insert_idempotency_claim,
    load_capability_replay, load_record_for_update,
};
use crate::{
    AuditIntent, BatchError, BatchMutationPlan, CapabilityBatchExecutionPlan, EventEvidence,
    PostgresDataStore, RecordMutation,
};
use crm_capability_runtime::{
    CapabilityDefinition, CapabilityExecutionResult, CapabilityRequest,
    TransactionalCapabilityExecutor,
};
use crm_module_sdk::{
    DataClass, ErrorCategory, PayloadEncoding, PortFuture, RecordRef, RecordSnapshot, ResourceRef,
    SdkError, TypedPayload,
};
use sqlx::{Postgres, Row, Transaction};
use std::fmt;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivacyOwnerRecordAction {
    Update,
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyOwnerActionTarget {
    pub reference: RecordRef,
    pub expected_version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyOwnerActionPlan {
    pub action: PrivacyOwnerRecordAction,
    pub payload: TypedPayload,
    pub event: EventEvidence,
    pub audit: AuditIntent,
    pub output: Option<TypedPayload>,
}

/// Pure owner-local planning boundary for privacy actions.
///
/// Implementations must strictly rehydrate the locked authoritative snapshot and
/// must not perform I/O, read clocks, branch on another owner, or invent KMS/HSM
/// evidence. All identities and timestamps must come from the validated request.
pub trait PrivacyOwnerActionPlanner: Send + Sync {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<PrivacyOwnerActionTarget, SdkError>;

    fn plan(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
        current: &RecordSnapshot,
    ) -> Result<PrivacyOwnerActionPlan, SdkError>;
}

#[derive(Clone)]
pub struct PostgresPrivacyOwnerActionExecutor {
    store: PostgresDataStore,
    planner: Arc<dyn PrivacyOwnerActionPlanner>,
}

impl fmt::Debug for PostgresPrivacyOwnerActionExecutor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostgresPrivacyOwnerActionExecutor")
            .field("store", &self.store)
            .field("planner", &"dyn PrivacyOwnerActionPlanner")
            .finish()
    }
}

impl PostgresPrivacyOwnerActionExecutor {
    pub fn new(store: PostgresDataStore, planner: Arc<dyn PrivacyOwnerActionPlanner>) -> Self {
        Self { store, planner }
    }
}

impl TransactionalCapabilityExecutor for PostgresPrivacyOwnerActionExecutor {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: CapabilityRequest,
    ) -> PortFuture<'a, Result<CapabilityExecutionResult, SdkError>> {
        Box::pin(async move {
            validate_executor_definition(definition)?;
            validate_request_identity(definition, &request)?;
            let target = self.planner.target(definition, &request)?;
            if target.expected_version <= 0 {
                return Err(invalid_plan(
                    "expected owner record version must be positive",
                ));
            }

            let idempotency =
                capability_idempotency(&request, crate::capability_idempotency_scope(definition))
                    .map_err(capability_batch_error_to_sdk)?;
            let mut transaction = self
                .store
                .pool()
                .begin()
                .await
                .map_err(BatchError::Database)
                .map_err(capability_batch_error_to_sdk)?;
            bind_execution_context(&mut transaction, &request.context)
                .await
                .map_err(capability_batch_error_to_sdk)?;

            if let Some(result) =
                load_capability_replay(&mut transaction, &request.context, &idempotency)
                    .await
                    .map_err(capability_batch_error_to_sdk)?
            {
                transaction
                    .commit()
                    .await
                    .map_err(BatchError::Database)
                    .map_err(capability_batch_error_to_sdk)?;
                return Ok(result);
            }

            insert_idempotency_claim(&mut transaction, &request.context, &idempotency)
                .await
                .map_err(capability_batch_error_to_sdk)?;
            let current =
                load_record_for_update(&mut transaction, &request.context, &target.reference)
                    .await
                    .map_err(capability_batch_error_to_sdk)?
                    .ok_or_else(owner_record_not_found)?;
            if current.version != target.expected_version {
                return Err(owner_record_stale());
            }

            let plan = self.planner.plan(definition, &request, &current)?;
            let evidence_plan = evidence_plan(&request, &target, &plan, idempotency)?;
            validate_transactional_aggregate_execution_plan(definition, &request, &evidence_plan)?;
            validate_owner_plan(&target, &current, &plan)?;

            let new_version = apply_owner_record_action(
                &mut transaction,
                &request,
                &target,
                plan.action,
                &plan.payload,
            )
            .await
            .map_err(capability_batch_error_to_sdk)?;
            insert_owner_outbox_event(&mut transaction, &request, &plan.event)
                .await
                .map_err(capability_batch_error_to_sdk)?;
            let materialized = materialize_audit_chain(
                &mut transaction,
                &request.context,
                std::slice::from_ref(&plan.audit),
            )
            .await
            .map_err(audit_materialization_to_batch_error)
            .map_err(capability_batch_error_to_sdk)?;
            insert_audit_record(&mut transaction, &request.context, &materialized[0])
                .await
                .map_err(capability_batch_error_to_sdk)?;

            let result = CapabilityExecutionResult {
                output: plan.output,
                affected_resources: vec![ResourceRef {
                    resource_type: target.reference.record_type.to_string(),
                    resource_id: target.reference.record_id.to_string(),
                    version: Some(new_version),
                }],
                replayed: false,
            };
            complete_capability_idempotency(&mut transaction, &evidence_plan.batch, &result)
                .await
                .map_err(capability_batch_error_to_sdk)?;
            insert_completion_marker(&mut transaction, &evidence_plan.batch)
                .await
                .map_err(capability_batch_error_to_sdk)?;
            transaction
                .commit()
                .await
                .map_err(BatchError::Database)
                .map_err(capability_batch_error_to_sdk)?;
            Ok(result)
        })
    }
}

fn validate_request_identity(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<(), SdkError> {
    if definition.owner_module_id != request.context.module_id
        || definition.capability_id != request.context.execution.capability_id
        || definition.capability_version != request.context.execution.capability_version
    {
        return Err(invalid_plan(
            "definition, executing owner and request coordinate do not agree",
        ));
    }
    Ok(())
}

fn evidence_plan(
    request: &CapabilityRequest,
    target: &PrivacyOwnerActionTarget,
    plan: &PrivacyOwnerActionPlan,
    idempotency: crate::IdempotencyEvidence,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    Ok(CapabilityBatchExecutionPlan {
        batch: BatchMutationPlan {
            context: request.context.clone(),
            records: vec![RecordMutation::Update {
                reference: target.reference.clone(),
                expected_version: target.expected_version,
                payload: plan.payload.clone(),
            }],
            relationships: Vec::new(),
            events: vec![plan.event.clone()],
            idempotency,
            audits: vec![plan.audit.clone()],
        },
        output: plan.output.clone(),
    })
}

fn validate_owner_plan(
    target: &PrivacyOwnerActionTarget,
    current: &RecordSnapshot,
    plan: &PrivacyOwnerActionPlan,
) -> Result<(), SdkError> {
    let next_version = current
        .version
        .checked_add(1)
        .ok_or_else(|| invalid_plan("owner record version overflowed"))?;
    if plan.event.event.aggregate != target.reference
        || plan.event.event.expected_aggregate_version != Some(current.version)
        || plan.event.aggregate_version != next_version
        || plan.event.event_sequence != next_version
    {
        return Err(invalid_plan(
            "owner event does not bind the locked resource and next version",
        ));
    }
    Ok(())
}

async fn apply_owner_record_action(
    transaction: &mut Transaction<'_, Postgres>,
    request: &CapabilityRequest,
    target: &PrivacyOwnerActionTarget,
    action: PrivacyOwnerRecordAction,
    payload: &TypedPayload,
) -> Result<i64, BatchError> {
    let maximum_size = i64::try_from(payload.maximum_size_bytes)
        .map_err(|_| BatchError::InvalidPlan("owner payload size exceeds i64".to_owned()))?;
    let delete = matches!(action, PrivacyOwnerRecordAction::Delete);
    let row = sqlx::query(
        r#"
        UPDATE crm.records
           SET version = version + 1,
               schema_id = $4,
               schema_version = $5,
               descriptor_hash = $6,
               data_class = $7,
               payload_encoding = $8,
               maximum_payload_size = $9,
               retention_policy_id = $10,
               payload_bytes = $11,
               last_business_transaction_id = $12,
               updated_at = clock_timestamp(),
               deleted_at = CASE WHEN $15::boolean THEN clock_timestamp() ELSE NULL END
         WHERE tenant_id = $1
           AND record_type = $2
           AND record_id = $3
           AND owner_module_id = $13
           AND version = $14
           AND deleted_at IS NULL
        RETURNING version
        "#,
    )
    .bind(request.context.execution.tenant_id.as_str())
    .bind(target.reference.record_type.as_str())
    .bind(target.reference.record_id.as_str())
    .bind(payload.schema_id.as_str())
    .bind(payload.schema_version.as_str())
    .bind(payload.descriptor_hash.as_slice())
    .bind(data_class_name(payload.data_class))
    .bind(payload_encoding_name(payload.encoding))
    .bind(maximum_size)
    .bind(payload.retention_policy_id.as_str())
    .bind(payload.bytes.as_slice())
    .bind(request.context.execution.business_transaction_id.as_str())
    .bind(request.context.module_id.as_str())
    .bind(target.expected_version)
    .bind(delete)
    .fetch_optional(&mut **transaction)
    .await?;
    row.map(|row| row.try_get("version"))
        .transpose()?
        .ok_or_else(|| {
            BatchError::Conflict(
                "owner record disappeared, changed version or crossed an ownership boundary"
                    .to_owned(),
            )
        })
}

async fn insert_owner_outbox_event(
    transaction: &mut Transaction<'_, Postgres>,
    request: &CapabilityRequest,
    evidence: &EventEvidence,
) -> Result<(), BatchError> {
    let maximum_size = i64::try_from(evidence.event.payload.maximum_size_bytes)
        .map_err(|_| BatchError::InvalidPlan("owner event payload size exceeds i64".to_owned()))?;
    sqlx::query(
        r#"
        INSERT INTO crm.outbox_events (
          tenant_id, event_id, business_transaction_id,
          aggregate_type, aggregate_id, aggregate_version, event_sequence,
          event_type, deduplication_key, schema_id, schema_version, descriptor_hash,
          data_class, payload_encoding, maximum_payload_size, retention_policy_id,
          payload_bytes, occurred_at
        )
        VALUES (
          $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17,
          TIMESTAMPTZ 'epoch' + ($18::bigint / 1000) * INTERVAL '1 microsecond'
        )
        "#,
    )
    .bind(request.context.execution.tenant_id.as_str())
    .bind(&evidence.event_id)
    .bind(request.context.execution.business_transaction_id.as_str())
    .bind(evidence.event.aggregate.record_type.as_str())
    .bind(evidence.event.aggregate.record_id.as_str())
    .bind(evidence.aggregate_version)
    .bind(evidence.event_sequence)
    .bind(evidence.event.event_type.as_str())
    .bind(&evidence.event.deduplication_key)
    .bind(evidence.event.payload.schema_id.as_str())
    .bind(evidence.event.payload.schema_version.as_str())
    .bind(evidence.event.payload.descriptor_hash.as_slice())
    .bind(data_class_name(evidence.event.payload.data_class))
    .bind(payload_encoding_name(evidence.event.payload.encoding))
    .bind(maximum_size)
    .bind(evidence.event.payload.retention_policy_id.as_str())
    .bind(evidence.event.payload.bytes.as_slice())
    .bind(evidence.occurred_at_unix_nanos)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

const fn data_class_name(value: DataClass) -> &'static str {
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

const fn payload_encoding_name(value: PayloadEncoding) -> &'static str {
    match value {
        PayloadEncoding::Protobuf => "protobuf",
        PayloadEncoding::Json => "json",
        PayloadEncoding::Utf8Text => "utf8_text",
        PayloadEncoding::Binary => "binary",
    }
}

fn audit_materialization_to_batch_error(error: AuditMaterializationError) -> BatchError {
    match error {
        AuditMaterializationError::Database(error) => BatchError::Database(error),
        AuditMaterializationError::InvalidIntent(message) => BatchError::InvalidPlan(message),
        AuditMaterializationError::InvalidStoredValue(message) => {
            BatchError::InvalidStoredValue(message)
        }
    }
}

fn owner_record_not_found() -> SdkError {
    SdkError::new(
        "PRIVACY_OWNER_RECORD_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The owner resource was not found.",
    )
}

fn owner_record_stale() -> SdkError {
    SdkError::new(
        "PRIVACY_OWNER_RECORD_STALE",
        ErrorCategory::Conflict,
        false,
        "The owner resource changed after privacy planning.",
    )
}

fn invalid_plan(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "PRIVACY_OWNER_ACTION_PLAN_INVALID",
        ErrorCategory::Internal,
        false,
        "The owner privacy action could not be planned safely.",
    )
    .with_internal_reference(reference)
}
