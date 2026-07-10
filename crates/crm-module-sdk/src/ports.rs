use crate::types::{
    CapabilityId, CapabilityVersion, EventType, FileId, ModuleExecutionContext, RecordId,
    RecordType, RelationshipType, ResourceRef, SdkError, StateKey, TypedPayload, WorkflowId,
    WorkflowRunId,
};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;

pub type PortFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
pub type PortResult<T> = Result<T, SdkError>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityInvocation {
    pub capability_id: CapabilityId,
    pub capability_version: CapabilityVersion,
    pub input: TypedPayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityOutcome {
    pub output: Option<TypedPayload>,
    pub affected_resources: Vec<ResourceRef>,
}

pub trait CapabilityClient: Send + Sync {
    fn invoke<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        request: CapabilityInvocation,
    ) -> PortFuture<'a, PortResult<CapabilityOutcome>>;
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordRef {
    pub record_type: RecordType,
    pub record_id: RecordId,
}

impl From<&RecordRef> for ResourceRef {
    fn from(value: &RecordRef) -> Self {
        Self {
            resource_type: value.record_type.to_string(),
            resource_id: value.record_id.to_string(),
            version: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordSnapshot {
    pub reference: RecordRef,
    pub version: i64,
    pub payload: TypedPayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateRecordRequest {
    pub record_type: RecordType,
    pub requested_record_id: Option<RecordId>,
    pub payload: TypedPayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateRecordRequest {
    pub reference: RecordRef,
    pub expected_version: i64,
    pub payload: TypedPayload,
}

pub trait RecordClient: Send + Sync {
    fn create<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        request: CreateRecordRequest,
    ) -> PortFuture<'a, PortResult<RecordSnapshot>>;

    fn update<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        request: UpdateRecordRequest,
    ) -> PortFuture<'a, PortResult<RecordSnapshot>>;

    fn get<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        reference: RecordRef,
    ) -> PortFuture<'a, PortResult<Option<RecordSnapshot>>>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelationshipRef {
    pub relationship_type: RelationshipType,
    pub source: RecordRef,
    pub target: RecordRef,
}

pub trait RelationshipClient: Send + Sync {
    fn link<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        relationship: RelationshipRef,
    ) -> PortFuture<'a, PortResult<()>>;

    fn unlink<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        relationship: RelationshipRef,
    ) -> PortFuture<'a, PortResult<()>>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainEvent {
    pub event_type: EventType,
    pub aggregate: RecordRef,
    pub expected_aggregate_version: Option<i64>,
    pub deduplication_key: String,
    pub payload: TypedPayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublishedEvent {
    pub event_id: String,
    pub aggregate_version: i64,
}

pub trait EventPublisher: Send + Sync {
    fn publish<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        event: DomainEvent,
    ) -> PortFuture<'a, PortResult<PublishedEvent>>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleStateEntry {
    pub key: StateKey,
    pub version: i64,
    pub value: TypedPayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PutModuleStateRequest {
    pub key: StateKey,
    pub expected_version: Option<i64>,
    pub value: TypedPayload,
}

pub trait ModuleStateStore: Send + Sync {
    fn get<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        key: StateKey,
    ) -> PortFuture<'a, PortResult<Option<ModuleStateEntry>>>;

    fn put<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        request: PutModuleStateRequest,
    ) -> PortFuture<'a, PortResult<ModuleStateEntry>>;

    fn delete<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        key: StateKey,
        expected_version: Option<i64>,
    ) -> PortFuture<'a, PortResult<()>>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartWorkflowRequest {
    pub workflow_id: WorkflowId,
    pub workflow_version: String,
    pub input: TypedPayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowSignal {
    pub workflow_run_id: WorkflowRunId,
    pub signal_type: String,
    pub payload: TypedPayload,
}

pub trait WorkflowClient: Send + Sync {
    fn start<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        request: StartWorkflowRequest,
    ) -> PortFuture<'a, PortResult<WorkflowRunId>>;

    fn signal<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        signal: WorkflowSignal,
    ) -> PortFuture<'a, PortResult<()>>;

    fn cancel<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        workflow_run_id: WorkflowRunId,
        reason_code: String,
    ) -> PortFuture<'a, PortResult<()>>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateFileIntentRequest {
    pub file_name: String,
    pub media_type: String,
    pub expected_size_bytes: u64,
    pub content_sha256: [u8; 32],
    pub classification: crate::types::DataClass,
    pub retention_policy_id: crate::types::RetentionPolicyId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileIntent {
    pub file_id: FileId,
    pub upload_token: String,
    pub expires_at_unix_nanos: i64,
}

pub trait FileClient: Send + Sync {
    fn create_upload_intent<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        request: CreateFileIntentRequest,
    ) -> PortFuture<'a, PortResult<FileIntent>>;

    fn attach_to_record<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        file_id: FileId,
        record: RecordRef,
    ) -> PortFuture<'a, PortResult<()>>;
}

pub trait Clock: Send + Sync {
    fn now_unix_nanos(&self) -> i64;
}

pub trait RandomSource: Send + Sync {
    fn fill_bytes(&self, destination: &mut [u8]) -> PortResult<()>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TelemetryLevel {
    Debug,
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TelemetryEvent {
    pub level: TelemetryLevel,
    pub event_name: String,
    pub attributes: Vec<(String, String)>,
}

pub trait ObservabilityContext: Send + Sync {
    fn emit<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        event: TelemetryEvent,
    ) -> PortFuture<'a, PortResult<()>>;
}
