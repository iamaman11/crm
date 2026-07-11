use crate::{CapabilityBatchExecutionPlan, PostgresDataStore, capability_batch_error_to_sdk};
use crm_capability_runtime::{
    CapabilityDefinition, CapabilityExecutionResult, CapabilityRequest,
    TransactionalCapabilityExecutor,
};
use crm_module_sdk::{PortFuture, RecordRef, RecordSnapshot, SdkError};
use std::fmt;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregatePresence {
    MustBeAbsent,
    MustExist,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggregateTarget {
    pub reference: RecordRef,
    pub presence: AggregatePresence,
}

/// Resolves an aggregate target without I/O, then builds a deterministic batch
/// from the authoritative snapshot loaded and locked by PostgreSQL.
///
/// Implementations must not perform I/O, read clocks or use non-deterministic
/// randomness. All time/identity material must already be present in the
/// validated request and execution context.
pub trait TransactionalAggregatePlanner: Send + Sync {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError>;

    fn plan(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
        current: Option<&RecordSnapshot>,
    ) -> Result<CapabilityBatchExecutionPlan, SdkError>;
}

#[derive(Clone)]
pub struct PostgresTransactionalAggregateExecutor {
    store: PostgresDataStore,
    planner: Arc<dyn TransactionalAggregatePlanner>,
}

impl fmt::Debug for PostgresTransactionalAggregateExecutor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostgresTransactionalAggregateExecutor")
            .field("store", &self.store)
            .field("planner", &"dyn TransactionalAggregatePlanner")
            .finish()
    }
}

impl PostgresTransactionalAggregateExecutor {
    pub fn new(store: PostgresDataStore, planner: Arc<dyn TransactionalAggregatePlanner>) -> Self {
        Self { store, planner }
    }
}

impl TransactionalCapabilityExecutor for PostgresTransactionalAggregateExecutor {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: CapabilityRequest,
    ) -> PortFuture<'a, Result<CapabilityExecutionResult, SdkError>> {
        Box::pin(async move {
            crate::capability_executor::validate_executor_definition(definition)?;

            // Target resolution is deliberately synchronous. PostgreSQL remains
            // the first awaited operation after the gateway's live authorization.
            let target = self.planner.target(definition, &request)?;
            self.store
                .execute_transactional_aggregate(definition, request, target, self.planner.as_ref())
                .await
                .map_err(capability_batch_error_to_sdk)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AuditIntent, BatchMutationPlan, EventEvidence, IdempotencyEvidence, RecordMutation,
    };
    use crm_capability_runtime::{CapabilityRisk, PayloadContract};
    use crm_module_sdk::{
        ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId,
        CorrelationId, DataClass, DomainEvent, EventType, ExecutionContext, IdempotencyKey,
        ModuleExecutionContext, ModuleId, PayloadEncoding, RecordId, RecordType, RequestId,
        RetentionPolicyId, SchemaId, SchemaVersion, TenantId, TraceId, TypedPayload,
    };

    struct Planner;

    impl TransactionalAggregatePlanner for Planner {
        fn target(
            &self,
            _definition: &CapabilityDefinition,
            _request: &CapabilityRequest,
        ) -> Result<AggregateTarget, SdkError> {
            Ok(AggregateTarget {
                reference: reference(),
                presence: AggregatePresence::MustExist,
            })
        }

        fn plan(
            &self,
            _definition: &CapabilityDefinition,
            request: &CapabilityRequest,
            current: Option<&RecordSnapshot>,
        ) -> Result<CapabilityBatchExecutionPlan, SdkError> {
            let current = current.expect("unit fixture requires a snapshot");
            Ok(CapabilityBatchExecutionPlan {
                batch: BatchMutationPlan {
                    context: request.context.clone(),
                    records: vec![RecordMutation::Update {
                        reference: reference(),
                        expected_version: current.version,
                        payload: payload("state", vec![2]),
                    }],
                    relationships: Vec::new(),
                    events: vec![EventEvidence {
                        event_id: "event-1".to_owned(),
                        event: DomainEvent {
                            event_type: EventType::try_new("test.aggregate.updated").unwrap(),
                            aggregate: reference(),
                            expected_aggregate_version: Some(current.version),
                            payload: payload("event", vec![2]),
                            deduplication_key: "event-1".to_owned(),
                        },
                        aggregate_version: current.version + 1,
                        event_sequence: current.version + 1,
                        occurred_at_unix_nanos: 2,
                    }],
                    idempotency: IdempotencyEvidence {
                        scope: "capability:test.aggregate.update:1.0.0".to_owned(),
                        key: request.context.execution.idempotency_key.to_string(),
                        request_hash: request.input_hash,
                        expires_at_unix_nanos: 10,
                    },
                    audits: vec![AuditIntent {
                        audit_record_id: "audit-1".to_owned(),
                        canonicalization_profile: "crm.cjson/v1".to_owned(),
                        canonical_envelope: vec![1],
                        occurred_at_unix_nanos: 2,
                    }],
                },
                output: Some(payload("output", vec![2])),
            })
        }
    }

    #[test]
    fn target_resolution_is_deterministic_and_infrastructure_free() {
        let planner = Planner;
        assert_eq!(
            planner.target(&definition(), &request()).unwrap(),
            AggregateTarget {
                reference: reference(),
                presence: AggregatePresence::MustExist,
            }
        );
    }

    fn definition() -> CapabilityDefinition {
        CapabilityDefinition {
            capability_id: CapabilityId::try_new("test.aggregate.update").unwrap(),
            capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
            owner_module_id: ModuleId::try_new("crm.test").unwrap(),
            input_contract: contract("input"),
            output_contract: Some(contract("output")),
            risk: CapabilityRisk::Low,
            mutation: true,
            requires_idempotency: true,
            requires_approval: false,
            authorization_policy_id: "test.aggregate.update".to_owned(),
            rate_limit_policy_id: None,
        }
    }

    fn request() -> CapabilityRequest {
        CapabilityRequest {
            context: ModuleExecutionContext {
                module_id: ModuleId::try_new("crm.test").unwrap(),
                execution: ExecutionContext {
                    tenant_id: TenantId::try_new("tenant-a").unwrap(),
                    actor_id: ActorId::try_new("actor-a").unwrap(),
                    request_id: RequestId::try_new("request-a").unwrap(),
                    correlation_id: CorrelationId::try_new("correlation-a").unwrap(),
                    causation_id: CausationId::try_new("causation-a").unwrap(),
                    trace_id: TraceId::try_new("trace-a").unwrap(),
                    capability_id: CapabilityId::try_new("test.aggregate.update").unwrap(),
                    capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                    idempotency_key: IdempotencyKey::try_new("idem-a").unwrap(),
                    business_transaction_id: BusinessTransactionId::try_new("tx-a").unwrap(),
                    schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                    request_started_at_unix_nanos: 1,
                },
            },
            input: payload("input", vec![1]),
            input_hash: [7; 32],
            approval: None,
        }
    }

    fn reference() -> RecordRef {
        RecordRef {
            record_type: RecordType::try_new("test.aggregate").unwrap(),
            record_id: RecordId::try_new("aggregate-1").unwrap(),
        }
    }

    fn contract(suffix: &str) -> PayloadContract {
        PayloadContract {
            owner: ModuleId::try_new("crm.test").unwrap(),
            schema_id: SchemaId::try_new(format!("test.aggregate.{suffix}")).unwrap(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
            descriptor_hash: [3; 32],
            allowed_data_classes: vec![DataClass::Internal],
            allowed_encodings: vec![PayloadEncoding::Json],
            maximum_size_bytes: 1024,
        }
    }

    fn payload(suffix: &str, bytes: Vec<u8>) -> TypedPayload {
        TypedPayload {
            owner: ModuleId::try_new("crm.test").unwrap(),
            schema_id: SchemaId::try_new(format!("test.aggregate.{suffix}")).unwrap(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
            descriptor_hash: [3; 32],
            data_class: DataClass::Internal,
            encoding: PayloadEncoding::Json,
            maximum_size_bytes: 1024,
            retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
            bytes,
        }
    }
}
