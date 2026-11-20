use crm_capability_runtime::{
    CapabilityDefinition, CapabilityRequest, CapabilityRisk, PayloadContract,
    TransactionalCapabilityExecutor,
};
use crm_core_data::{
    AggregatePresence, AggregateTarget, AuditIntent, BatchError, BatchMutationPlan,
    CapabilityBatchExecutionPlan, EventEvidence, FaultInjection, IdempotencyEvidence,
    PostgresDataStore, PostgresTransactionalAggregateExecutor, RecordMutation,
    RelationshipMutation, TransactionalAggregatePlanner,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, CorrelationId,
    DataClass, DomainEvent, EventType, ExecutionContext, IdempotencyKey, ModuleExecutionContext,
    ModuleId, PayloadEncoding, RecordId, RecordRef, RecordType, RelationshipRef, RelationshipType,
    RequestId, RetentionPolicyId, SchemaId, SchemaVersion, SdkError, TenantId, TraceId,
    TypedPayload,
};
use sqlx::{Postgres, Row, Transaction};
use std::sync::Arc;

include!("postgres_advanced/support.rs");
include!("postgres_advanced/plans.rs");
include!("postgres_advanced/database.rs");
include!("postgres_advanced/scenario.rs");
