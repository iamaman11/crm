use crm_capability_ingress::semantic_input_hash;
use crm_capability_plan_support::{self as support, PersistedPayloadContract};
use crm_capability_runtime::{CapabilityExecutionResult, CapabilityRequest};
use crm_core_data::{
    AuditIntent, IdempotencyEvidence, PostgresDataStore, PostgresImmutableFileArtifactStore,
    RecordCreatePlan,
};
use crm_core_events::{
    EventHistoryRequest, ProjectionDocumentWrite, ProjectionEventApplication, ProjectionStore,
};
use crm_core_files::{
    AppendImmutableFileChunk, CreateImmutableFileArtifact, ImmutableFileArtifactStore,
};
use crm_customer_enrichment::{
    EnrichmentRequest, EnrichmentRequestDraft, LIFECYCLE_STATE_RETENTION_POLICY_ID,
    LIFECYCLE_STATE_SCHEMA_VERSION, MappingDraft, MappingNormalization, MappingVersion,
    PROVIDER_PROCESS_OUTCOME_RESOURCE_TYPE, PROVIDER_PROCESS_PROJECTION_ID,
    PROVIDER_RESPONSE_RECEIPT_RECORD_TYPE, PROVIDER_RESPONSE_RECEIPT_STATE_MAXIMUM_BYTES,
    PROVIDER_RESPONSE_RECEIPT_STATE_SCHEMA_ID, ProviderProcessCanonicalOutcome,
    ProviderProfileDraft, ProviderProfileVersion, ProviderResponseClass, ProviderResponseReceipt,
    ProviderResponseReceiptDraft, RawPayloadPolicy, RequestPolicyEvidence, TargetField,
    TargetSnapshot, encode_provider_response_receipt_state,
    provider_response_receipt_state_descriptor_hash,
};
use crm_customer_enrichment_capability_adapter::{
    ENRICHMENT_REQUEST_CREATED_EVENT_SCHEMA, ENRICHMENT_REQUEST_CREATED_EVENT_TYPE,
    MAPPING_PUBLISHED_EVENT_SCHEMA, MAPPING_PUBLISHED_EVENT_TYPE, MODULE_ID,
    PROVIDER_PROFILE_PUBLISHED_EVENT_SCHEMA, PROVIDER_PROFILE_PUBLISHED_EVENT_TYPE,
    enrichment_request_persisted_payload, enrichment_request_record_ref,
    enrichment_request_to_wire, mapping_persisted_payload, mapping_record_ref, mapping_to_wire,
    provider_profile_persisted_payload, provider_profile_record_ref, provider_profile_to_wire,
};
use crm_customer_enrichment_materialization_composition::{
    CustomerEnrichmentMaterializationProcessWorker,
    GovernedFileProviderSuggestionCandidateEvidenceSource, MATERIALIZATION_PROCESS_PROJECTION_ID,
    PROVIDER_RESPONSE_RECORDED_EVENT_SCHEMA, PROVIDER_RESPONSE_RECORDED_EVENT_TYPE,
    PROVIDER_SUGGESTION_CANDIDATE_EVIDENCE_MEDIA_TYPE,
    PostgresCustomerEnrichmentSuggestionMaterializationWorker,
    ProviderSuggestionCandidateEvidenceRequest, ProviderSuggestionCandidateEvidenceSourcePort,
    SuggestionMaterializationExecutorPort,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, CorrelationId,
    DataClass, DomainEvent, EventType, ExecutionContext, FileId, IdempotencyKey,
    ModuleExecutionContext, ModuleId, PortFuture, RecordRef, RequestId, RetentionPolicyId,
    SchemaVersion, SdkError, TenantId, TraceId, TypedPayload,
};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

const TENANT_ID: &str = "tenant-a";
const ACTOR_ID: &str = "actor-a";
const FILE_ID: &str = "materialization-malformed-candidate-evidence-1";
const SEED_CAPABILITY: &str = "customer_enrichment.materialization.seed";
