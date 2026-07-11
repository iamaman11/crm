use crate::{BatchError, BatchMutationPlan, BatchMutationResult, PostgresDataStore};
use crm_capability_runtime::{
    CapabilityDefinition, CapabilityExecutionResult, CapabilityRequest,
    TransactionalCapabilityExecutor,
};
use crm_module_sdk::{ErrorCategory, PortFuture, ResourceRef, SdkError, TypedPayload};
use std::fmt;
use std::sync::Arc;

const IDEMPOTENCY_SCOPE_PREFIX: &str = "capability";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityBatchExecutionPlan {
    pub batch: BatchMutationPlan,
    pub output: Option<TypedPayload>,
}

/// Pure, deterministic mapping from a validated capability request to one
/// PostgreSQL batch. Implementations must not perform I/O, read clocks or use
/// non-deterministic randomness. This keeps the database batch as the first
/// awaited operation after live authorization.
pub trait CapabilityBatchPlanner: Send + Sync {
    fn plan(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<CapabilityBatchExecutionPlan, SdkError>;
}

pub trait BatchMutationRuntime: Send + Sync {
    fn execute_batch<'a>(
        &'a self,
        plan: &'a BatchMutationPlan,
    ) -> PortFuture<'a, Result<BatchMutationResult, BatchError>>;
}

impl BatchMutationRuntime for PostgresDataStore {
    fn execute_batch<'a>(
        &'a self,
        plan: &'a BatchMutationPlan,
    ) -> PortFuture<'a, Result<BatchMutationResult, BatchError>> {
        Box::pin(async move { PostgresDataStore::execute_batch(self, plan).await })
    }
}

#[derive(Clone)]
pub struct PostgresTransactionalCapabilityExecutor {
    runtime: Arc<dyn BatchMutationRuntime>,
    planner: Arc<dyn CapabilityBatchPlanner>,
}

impl fmt::Debug for PostgresTransactionalCapabilityExecutor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostgresTransactionalCapabilityExecutor")
            .field("runtime", &"dyn BatchMutationRuntime")
            .field("planner", &"dyn CapabilityBatchPlanner")
            .finish()
    }
}

impl PostgresTransactionalCapabilityExecutor {
    pub fn new(store: PostgresDataStore, planner: Arc<dyn CapabilityBatchPlanner>) -> Self {
        Self {
            runtime: Arc::new(store),
            planner,
        }
    }

    #[doc(hidden)]
    pub fn from_runtime(
        runtime: Arc<dyn BatchMutationRuntime>,
        planner: Arc<dyn CapabilityBatchPlanner>,
    ) -> Self {
        Self { runtime, planner }
    }
}

impl TransactionalCapabilityExecutor for PostgresTransactionalCapabilityExecutor {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: CapabilityRequest,
    ) -> PortFuture<'a, Result<CapabilityExecutionResult, SdkError>> {
        Box::pin(async move {
            validate_executor_definition(definition)?;

            // This planner is deliberately synchronous. The existing PostgreSQL
            // batch runtime remains the first awaited operation after the gateway's
            // live authorization decision.
            let execution_plan = self.planner.plan(definition, &request)?;
            validate_execution_plan(definition, &request, &execution_plan)?;

            let batch_result = self
                .runtime
                .execute_batch(&execution_plan.batch)
                .await
                .map_err(capability_batch_error_to_sdk)?;

            Ok(CapabilityExecutionResult {
                output: execution_plan.output,
                affected_resources: affected_resources(&batch_result),
                replayed: batch_result.replayed,
            })
        })
    }
}

pub fn capability_idempotency_scope(definition: &CapabilityDefinition) -> String {
    format!(
        "{IDEMPOTENCY_SCOPE_PREFIX}:{}:{}",
        definition.capability_id, definition.capability_version
    )
}

pub(crate) fn validate_executor_definition(
    definition: &CapabilityDefinition,
) -> Result<(), SdkError> {
    if !definition.mutation || !definition.requires_idempotency {
        return Err(SdkError::new(
            "CAPABILITY_EXECUTOR_DEFINITION_INVALID",
            ErrorCategory::Internal,
            false,
            "The capability execution configuration is invalid.",
        ));
    }
    Ok(())
}

pub(crate) fn validate_execution_plan(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    execution_plan: &CapabilityBatchExecutionPlan,
) -> Result<(), SdkError> {
    if execution_plan.batch.context != request.context {
        return Err(invalid_execution_plan());
    }
    if execution_plan.batch.idempotency.scope != capability_idempotency_scope(definition)
        || execution_plan.batch.idempotency.key
            != request.context.execution.idempotency_key.as_str()
        || execution_plan.batch.idempotency.request_hash != request.input_hash
    {
        return Err(invalid_execution_plan());
    }

    execution_plan
        .batch
        .validate()
        .map_err(capability_batch_error_to_sdk)?;
    validate_planned_output(definition, execution_plan.output.as_ref())?;
    Ok(())
}

fn validate_planned_output(
    definition: &CapabilityDefinition,
    output: Option<&TypedPayload>,
) -> Result<(), SdkError> {
    match (&definition.output_contract, output) {
        (None, None) => Ok(()),
        (Some(contract), Some(payload)) => {
            payload.validate()?;
            if contract.matches(payload) {
                Ok(())
            } else {
                Err(invalid_execution_plan())
            }
        }
        _ => Err(invalid_execution_plan()),
    }
}

fn invalid_execution_plan() -> SdkError {
    SdkError::new(
        "CAPABILITY_EXECUTION_PLAN_INVALID",
        ErrorCategory::Internal,
        false,
        "The capability execution plan is invalid.",
    )
}

pub fn capability_batch_error_to_sdk(error: BatchError) -> SdkError {
    match error {
        BatchError::Sdk(error) => error,
        BatchError::Conflict(_) => SdkError::new(
            "CAPABILITY_CONFLICT",
            ErrorCategory::Conflict,
            false,
            "The requested change conflicts with the current state.",
        ),
        BatchError::IdempotencyKeyReused => SdkError::new(
            "CAPABILITY_IDEMPOTENCY_KEY_REUSED",
            ErrorCategory::Conflict,
            false,
            "The idempotency key was already used for a different request.",
        ),
        BatchError::IdempotencyInProgress => SdkError::new(
            "CAPABILITY_IDEMPOTENCY_IN_PROGRESS",
            ErrorCategory::Conflict,
            true,
            "The same request is already being processed.",
        ),
        BatchError::Database(_) => SdkError::new(
            "CAPABILITY_STORAGE_UNAVAILABLE",
            ErrorCategory::Unavailable,
            true,
            "The capability could not be persisted at this time.",
        ),
        BatchError::InvalidPlan(_) => invalid_execution_plan(),
        BatchError::InvalidStoredValue(_) => SdkError::new(
            "CAPABILITY_IDEMPOTENCY_STATE_INVALID",
            ErrorCategory::Internal,
            false,
            "The stored capability result is invalid.",
        ),
    }
}

pub(crate) fn affected_resources(result: &BatchMutationResult) -> Vec<ResourceRef> {
    let mut resources = Vec::with_capacity(
        result.records.len()
            + result.linked_relationships.len()
            + result.unlinked_relationships.len(),
    );

    resources.extend(result.records.iter().map(|record| ResourceRef {
        resource_type: record.reference.record_type.to_string(),
        resource_id: record.reference.record_id.to_string(),
        version: Some(record.version),
    }));
    resources.extend(
        result
            .linked_relationships
            .iter()
            .map(relationship_resource),
    );
    resources.extend(
        result
            .unlinked_relationships
            .iter()
            .map(relationship_resource),
    );
    resources
}

fn relationship_resource(relationship: &crm_module_sdk::RelationshipRef) -> ResourceRef {
    ResourceRef {
        resource_type: format!("relationship:{}", relationship.relationship_type),
        resource_id: format!(
            "{}:{}/{}:{}",
            relationship.source.record_type,
            relationship.source.record_id,
            relationship.target.record_type,
            relationship.target.record_id
        ),
        version: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AuditIntent, EventEvidence, IdempotencyEvidence, RecordMutation, RelationshipMutation,
    };
    use crm_capability_runtime::{CapabilityRisk, PayloadContract};
    use crm_module_sdk::{
        ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId,
        CorrelationId, DataClass, DomainEvent, EventType, ExecutionContext, IdempotencyKey,
        ModuleExecutionContext, ModuleId, PayloadEncoding, RecordId, RecordRef, RecordSnapshot,
        RecordType, RequestId, RetentionPolicyId, SchemaId, SchemaVersion, TenantId, TraceId,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Clone)]
    enum FakeOutcome {
        Success(BatchMutationResult),
        IdempotencyKeyReused,
    }

    struct FakeRuntime {
        calls: Arc<AtomicUsize>,
        outcome: FakeOutcome,
    }

    impl BatchMutationRuntime for FakeRuntime {
        fn execute_batch<'a>(
            &'a self,
            _plan: &'a BatchMutationPlan,
        ) -> PortFuture<'a, Result<BatchMutationResult, BatchError>> {
            Box::pin(async move {
                self.calls.fetch_add(1, Ordering::SeqCst);
                match &self.outcome {
                    FakeOutcome::Success(result) => Ok(result.clone()),
                    FakeOutcome::IdempotencyKeyReused => Err(BatchError::IdempotencyKeyReused),
                }
            })
        }
    }

    struct FixedPlanner {
        plan: CapabilityBatchExecutionPlan,
    }

    impl CapabilityBatchPlanner for FixedPlanner {
        fn plan(
            &self,
            _definition: &CapabilityDefinition,
            _request: &CapabilityRequest,
        ) -> Result<CapabilityBatchExecutionPlan, SdkError> {
            Ok(self.plan.clone())
        }
    }

    #[tokio::test]
    async fn executes_one_batch_and_propagates_replay_state() {
        let definition = definition();
        let request = request();
        let plan = execution_plan(&definition, &request);
        let calls = Arc::new(AtomicUsize::new(0));
        let runtime = Arc::new(FakeRuntime {
            calls: Arc::clone(&calls),
            outcome: FakeOutcome::Success(BatchMutationResult {
                records: vec![RecordSnapshot {
                    reference: record_ref(),
                    version: 3,
                    payload: payload(),
                }],
                linked_relationships: Vec::new(),
                unlinked_relationships: Vec::new(),
                replayed: true,
            }),
        });
        let executor = PostgresTransactionalCapabilityExecutor::from_runtime(
            runtime,
            Arc::new(FixedPlanner { plan }),
        );

        let result = executor.execute(&definition, request).await.unwrap();

        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(result.replayed);
        assert_eq!(result.affected_resources[0].version, Some(3));
    }

    #[tokio::test]
    async fn rejects_context_mismatch_before_batch_runtime() {
        let definition = definition();
        let request = request();
        let mut plan = execution_plan(&definition, &request);
        plan.batch.context.execution.request_id = RequestId::try_new("different").unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let executor = PostgresTransactionalCapabilityExecutor::from_runtime(
            Arc::new(FakeRuntime {
                calls: Arc::clone(&calls),
                outcome: FakeOutcome::Success(empty_result()),
            }),
            Arc::new(FixedPlanner { plan }),
        );

        let error = executor.execute(&definition, request).await.unwrap_err();

        assert_eq!(error.code, "CAPABILITY_EXECUTION_PLAN_INVALID");
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn maps_idempotency_key_reuse_without_human_text_parsing() {
        let definition = definition();
        let request = request();
        let plan = execution_plan(&definition, &request);
        let executor = PostgresTransactionalCapabilityExecutor::from_runtime(
            Arc::new(FakeRuntime {
                calls: Arc::new(AtomicUsize::new(0)),
                outcome: FakeOutcome::IdempotencyKeyReused,
            }),
            Arc::new(FixedPlanner { plan }),
        );

        let error = executor.execute(&definition, request).await.unwrap_err();

        assert_eq!(error.code, "CAPABILITY_IDEMPOTENCY_KEY_REUSED");
        assert_eq!(error.category, ErrorCategory::Conflict);
        assert!(!error.retryable);
    }

    #[test]
    fn idempotency_scope_is_capability_version_bound() {
        assert_eq!(
            capability_idempotency_scope(&definition()),
            "capability:crm.sales.deal.create:1.0.0"
        );
    }

    fn definition() -> CapabilityDefinition {
        CapabilityDefinition {
            capability_id: CapabilityId::try_new("crm.sales.deal.create").unwrap(),
            capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
            owner_module_id: ModuleId::try_new("crm.sales").unwrap(),
            input_contract: contract(),
            output_contract: Some(contract()),
            risk: CapabilityRisk::Medium,
            mutation: true,
            requires_idempotency: true,
            requires_approval: false,
            authorization_policy_id: "sales.deal.create".to_owned(),
            rate_limit_policy_id: None,
        }
    }

    fn contract() -> PayloadContract {
        PayloadContract {
            owner: ModuleId::try_new("crm.sales").unwrap(),
            schema_id: SchemaId::try_new("crm.sales.deal").unwrap(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
            descriptor_hash: [1; 32],
            allowed_data_classes: vec![DataClass::Internal],
            allowed_encodings: vec![PayloadEncoding::Json],
            maximum_size_bytes: 1024,
        }
    }

    fn request() -> CapabilityRequest {
        CapabilityRequest {
            context: context(),
            input: payload(),
            input_hash: [2; 32],
            approval: None,
        }
    }

    fn context() -> ModuleExecutionContext {
        ModuleExecutionContext {
            module_id: ModuleId::try_new("crm.sales").unwrap(),
            execution: ExecutionContext {
                tenant_id: TenantId::try_new("tenant-1").unwrap(),
                actor_id: ActorId::try_new("actor-1").unwrap(),
                request_id: RequestId::try_new("request-1").unwrap(),
                correlation_id: CorrelationId::try_new("correlation-1").unwrap(),
                causation_id: CausationId::try_new("causation-1").unwrap(),
                trace_id: TraceId::try_new("trace-1").unwrap(),
                capability_id: CapabilityId::try_new("crm.sales.deal.create").unwrap(),
                capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                idempotency_key: IdempotencyKey::try_new("idem-1").unwrap(),
                business_transaction_id: BusinessTransactionId::try_new("txn-1").unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                request_started_at_unix_nanos: 1,
            },
        }
    }

    fn execution_plan(
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> CapabilityBatchExecutionPlan {
        let record = record_ref();
        CapabilityBatchExecutionPlan {
            batch: BatchMutationPlan {
                context: request.context.clone(),
                records: vec![RecordMutation::Create {
                    reference: record.clone(),
                    payload: payload(),
                }],
                relationships: Vec::<RelationshipMutation>::new(),
                events: vec![EventEvidence {
                    event_id: "event-1".to_owned(),
                    event: DomainEvent {
                        event_type: EventType::try_new("crm.sales.deal.created").unwrap(),
                        aggregate: record,
                        expected_aggregate_version: None,
                        deduplication_key: "deal-created-1".to_owned(),
                        payload: payload(),
                    },
                    aggregate_version: 1,
                    event_sequence: 1,
                    occurred_at_unix_nanos: 2,
                }],
                idempotency: IdempotencyEvidence {
                    scope: capability_idempotency_scope(definition),
                    key: request.context.execution.idempotency_key.to_string(),
                    request_hash: request.input_hash,
                    expires_at_unix_nanos: 1_000,
                },
                audits: vec![AuditIntent {
                    audit_record_id: "audit-1".to_owned(),
                    canonicalization_profile: "crm.cjson/v1".to_owned(),
                    canonical_envelope: vec![1],
                    occurred_at_unix_nanos: 2,
                }],
            },
            output: Some(payload()),
        }
    }

    fn record_ref() -> RecordRef {
        RecordRef {
            record_type: RecordType::try_new("deal").unwrap(),
            record_id: RecordId::try_new("deal-1").unwrap(),
        }
    }

    fn payload() -> TypedPayload {
        TypedPayload {
            owner: ModuleId::try_new("crm.sales").unwrap(),
            schema_id: SchemaId::try_new("crm.sales.deal").unwrap(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
            descriptor_hash: [1; 32],
            data_class: DataClass::Internal,
            encoding: PayloadEncoding::Json,
            maximum_size_bytes: 1024,
            retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
            bytes: br#"{"id":"deal-1"}"#.to_vec(),
        }
    }

    fn empty_result() -> BatchMutationResult {
        BatchMutationResult {
            records: Vec::new(),
            linked_relationships: Vec::new(),
            unlinked_relationships: Vec::new(),
            replayed: false,
        }
    }
}
