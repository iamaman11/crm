use crm_core_data::{
    AuditIntent, BatchError, BatchMutationPlan, EventEvidence, FaultInjection, IdempotencyEvidence,
    PostgresDataStore, RecordMutation, RelationshipMutation,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, CorrelationId,
    DataClass, DomainEvent, EventType, ExecutionContext, IdempotencyKey, ModuleExecutionContext,
    ModuleId, PayloadEncoding, RecordId, RecordRef, RecordType, RelationshipRef, RelationshipType,
    RequestId, RetentionPolicyId, SchemaId, SchemaVersion, TenantId, TraceId, TypedPayload,
};
use sqlx::{Postgres, Row, Transaction};

include!("postgres_advanced/support.rs");
include!("postgres_advanced/plans.rs");
include!("postgres_advanced/database.rs");
include!("postgres_advanced/scenario.rs");
