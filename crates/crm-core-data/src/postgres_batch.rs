use crate::aggregate_executor::{
    AggregatePresence, AggregateTarget, TransactionalAggregatePlanner,
};
use crate::audit::{
    AuditIntent, AuditMaterializationError, MaterializedAuditRecord, materialize_audit_chain,
};
use crate::capability_executor::{
    CapabilityBatchExecutionPlan, affected_resources, capability_idempotency_scope,
    validate_transactional_aggregate_execution_plan,
};
use crate::postgres::{FaultInjection, IdempotencyEvidence, PostgresDataStore};
use crm_capability_runtime::{CapabilityDefinition, CapabilityExecutionResult, CapabilityRequest};
use crm_module_sdk::{
    DataClass, DomainEvent, ErrorCategory, ModuleExecutionContext, ModuleId, PayloadEncoding,
    RecordRef, RecordSnapshot, RelationshipRef, RetentionPolicyId, SchemaId, SchemaVersion,
    SdkError, TypedPayload,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Row, Transaction};
use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

include!("postgres_batch/model.rs");
include!("postgres_batch/composition.rs");
include!("postgres_batch/executor.rs");
include!("postgres_batch/composition_executor.rs");
include!("postgres_batch/records.rs");
include!("postgres_batch/evidence.rs");
include!("postgres_batch/helpers.rs");
include!("postgres_batch/tests.rs");
