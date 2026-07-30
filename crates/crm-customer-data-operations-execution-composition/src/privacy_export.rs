use crm_capability_plan_support::{
    PersistedPayloadContract, persisted_json_payload_with_data_class,
};
use crm_core_data::{
    AuditIntent, BatchError, BatchMutationPlan, EventEvidence, IdempotencyEvidence,
    PostgresDataStore, RecordGetQuery, RecordMutation,
};
use crm_core_files::{
    AppendImmutableFileChunk, CreateImmutableFileArtifact, FileArtifactMetadata,
    FileArtifactStatus, ImmutableFileArtifactStore, MAXIMUM_FILE_ARTIFACT_CHUNK_BYTES,
};
use crm_customer_data_operations_capability_adapter::MODULE_ID;
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, CorrelationId,
    DataClass, DomainEvent, ErrorCategory, EventType, ExecutionContext, FileId, IdempotencyKey,
    ModuleExecutionContext, ModuleId, PayloadEncoding, RecordId, RecordRef, RecordType, RequestId,
    RetentionPolicyId, SchemaId, SchemaVersion, SdkError, TenantId, TraceId, TypedPayload,
};
use sha2::{Digest, Sha256};
use std::sync::Arc;

pub const PRIVACY_EXPORT_REQUEST_CAPABILITY: &str = "customer_data.export.privacy.request";
pub const PRIVACY_EXPORT_REQUEST_VERSION: &str = "1.0.0";
pub const PRIVACY_EXPORT_JOB_RECORD_TYPE: &str = "customer_data.privacy_export_job";
pub const PRIVACY_EXPORT_JOB_STATE_SCHEMA_ID: &str =
    "crm.customer-data-operations.privacy_export_job.state";
pub const PRIVACY_EXPORT_JOB_STATE_SCHEMA_VERSION: &str = "1.0.0";
pub const PRIVACY_EXPORT_JOB_STATE_MAXIMUM_BYTES: u64 = 16 * 1024;
pub const PRIVACY_EXPORT_RETENTION_POLICY_ID: &str = "customer_privacy_access_export";
pub const PRIVACY_EXPORT_MEDIA_TYPE: &str =
    "application/vnd.crm.customer-privacy-access-export+json;version=1";

const PRIVACY_EXPORT_JOB_DESCRIPTOR: &[u8] = b"crm.customer-data-operations.privacy_export_job.state/v1:status,tenant_id,privacy_case_id,export_job_id,target_idempotency_key,manifest_id,manifest_digest,file_id,content_sha256,size_bytes,retention_policy_id,prepared_at_unix_nanos";
const PRIVACY_EXPORT_EVENT_SCHEMA_ID: &str =
    "crm.customer-data-operations.privacy_export_job.event";
const PRIVACY_EXPORT_EVENT_DESCRIPTOR: &[u8] =
    b"crm.customer-data-operations.privacy_export_job.event/v1:operation,job_state_sha256";
const PRIVACY_EXPORT_PREPARED_EVENT_TYPE: &str =
    "customer_data.export.privacy.internal.job_prepared";
const PRIVACY_EXPORT_COMPLETED_EVENT_TYPE: &str =
    "customer_data.export.privacy.internal.job_completed";
const FILE_ID_PREFIX: &str = "privacy-export-artifact-";
const IDEMPOTENCY_TTL_NANOS: i64 = 86_400_000_000_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyManifestExportRequest {
    pub tenant_id: TenantId,
    pub privacy_case_id: RecordId,
    pub export_job_id: RecordId,
    pub target_idempotency_key: String,
    pub manifest_id: RecordId,
    pub manifest_digest: [u8; 32],
    pub manifest_bytes: Vec<u8>,
    pub actor_id: ActorId,
    pub correlation_id: CorrelationId,
    pub trace_id: TraceId,
    pub prepared_at_unix_nanos: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyManifestExportResult {
    pub export_job_id: RecordId,
    pub file_id: FileId,
    pub media_type: String,
    pub content_sha256: [u8; 32],
    pub size_bytes: u64,
    pub retention_policy_id: RetentionPolicyId,
    pub completed_at_unix_nanos: i64,
    pub replayed: bool,
}

#[derive(Clone)]
pub struct PrivacyManifestExportPublisher {
    store: PostgresDataStore,
    file_store: Arc<dyn ImmutableFileArtifactStore>,
}

impl std::fmt::Debug for PrivacyManifestExportPublisher {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PrivacyManifestExportPublisher")
            .field("store", &self.store)
            .field("file_store", &"dyn ImmutableFileArtifactStore")
            .finish()
    }
}

impl PrivacyManifestExportPublisher {
    pub fn new(store: PostgresDataStore, file_store: Arc<dyn ImmutableFileArtifactStore>) -> Self {
        Self { store, file_store }
    }

    pub async fn request(
        &self,
        request: PrivacyManifestExportRequest,
    ) -> Result<PrivacyManifestExportResult, SdkError> {
        let blueprint = PrivacyExportBlueprint::build(&request)?;
        let record = self.load_job(&request).await?;
        let mut job_replayed = false;
        match record.as_ref() {
            None => {
                self.persist_job(&request, &blueprint, JobStatus::Prepared, None)
                    .await?;
            }
            Some(record) if record.version == 1 => {
                validate_job_snapshot(record, &blueprint, JobStatus::Prepared)?;
                job_replayed = true;
            }
            Some(record) if record.version == 2 => {
                validate_job_snapshot(record, &blueprint, JobStatus::Completed)?;
                job_replayed = true;
            }
            Some(_) => {
                return Err(job_state_invalid(
                    "privacy export job version is unsupported",
                ));
            }
        }

        let context = execution_context(&request, "artifact")?;
        let initial = self
            .file_store
            .create(
                &context,
                CreateImmutableFileArtifact {
                    file_id: blueprint.file_id.clone(),
                    owner_module_id: module_id()?,
                    media_type: PRIVACY_EXPORT_MEDIA_TYPE.to_owned(),
                    data_class: DataClass::Personal,
                    retention_policy_id: blueprint.retention_policy_id.clone(),
                    expected_size_bytes: blueprint.size_bytes,
                    expected_sha256: blueprint.content_sha256,
                },
            )
            .await?;
        validate_artifact_metadata(&initial, &blueprint)?;
        let artifact_replayed = initial.status == FileArtifactStatus::Finalized;
        let finalized = match initial.status {
            FileArtifactStatus::Finalized => initial,
            FileArtifactStatus::Uploading => {
                let mut metadata = initial;
                let chunks: Vec<&[u8]> = request
                    .manifest_bytes
                    .chunks(MAXIMUM_FILE_ARTIFACT_CHUNK_BYTES)
                    .collect();
                let start = usize::try_from(metadata.next_chunk_index)
                    .map_err(|_| job_state_invalid("artifact chunk index exceeds usize"))?;
                if start > chunks.len()
                    || metadata.received_size_bytes
                        != chunks[..start].iter().try_fold(0_u64, |size, chunk| {
                            size.checked_add(chunk.len() as u64)
                                .ok_or_else(|| job_state_invalid("artifact size overflowed"))
                        })?
                {
                    return Err(job_state_invalid(
                        "artifact upload checkpoint differs from deterministic chunks",
                    ));
                }
                for (index, chunk) in chunks.into_iter().enumerate().skip(start) {
                    let append = self
                        .file_store
                        .append_chunk(
                            &context,
                            AppendImmutableFileChunk {
                                file_id: blueprint.file_id.clone(),
                                chunk_index: u64::try_from(index)
                                    .map_err(|_| job_state_invalid("chunk index exceeds u64"))?,
                                chunk_sha256: sha256(chunk),
                                bytes: chunk.to_vec(),
                            },
                        )
                        .await?;
                    metadata = append.metadata;
                    validate_artifact_metadata(&metadata, &blueprint)?;
                }
                self.file_store
                    .finalize(&context, &blueprint.file_id)
                    .await?
            }
        };
        validate_finalized_artifact(&finalized, &blueprint)?;

        let latest = self.load_job(&request).await?.ok_or_else(|| {
            job_state_invalid("privacy export job disappeared after artifact I/O")
        })?;
        let completion_replayed = if latest.version == 2 {
            validate_job_snapshot(&latest, &blueprint, JobStatus::Completed)?;
            true
        } else if latest.version == 1 {
            validate_job_snapshot(&latest, &blueprint, JobStatus::Prepared)?;
            self.persist_job(&request, &blueprint, JobStatus::Completed, Some(1))
                .await?;
            false
        } else {
            return Err(job_state_invalid(
                "privacy export job version is unsupported",
            ));
        };

        Ok(PrivacyManifestExportResult {
            export_job_id: request.export_job_id,
            file_id: blueprint.file_id,
            media_type: PRIVACY_EXPORT_MEDIA_TYPE.to_owned(),
            content_sha256: blueprint.content_sha256,
            size_bytes: blueprint.size_bytes,
            retention_policy_id: blueprint.retention_policy_id,
            completed_at_unix_nanos: request.prepared_at_unix_nanos,
            replayed: job_replayed || artifact_replayed || completion_replayed,
        })
    }

    async fn load_job(
        &self,
        request: &PrivacyManifestExportRequest,
    ) -> Result<Option<crm_module_sdk::RecordSnapshot>, SdkError> {
        self.store
            .get_record_for_query(&RecordGetQuery {
                tenant_id: request.tenant_id.clone(),
                owner_module_id: module_id()?,
                record_type: RecordType::try_new(PRIVACY_EXPORT_JOB_RECORD_TYPE)
                    .map_err(configuration_error)?,
                record_id: request.export_job_id.clone(),
            })
            .await
    }

    async fn persist_job(
        &self,
        request: &PrivacyManifestExportRequest,
        blueprint: &PrivacyExportBlueprint,
        status: JobStatus,
        expected_version: Option<i64>,
    ) -> Result<(), SdkError> {
        let state = job_state_bytes(request, blueprint, status);
        let context = execution_context(request, status.label())?;
        let reference = RecordRef {
            record_type: RecordType::try_new(PRIVACY_EXPORT_JOB_RECORD_TYPE)
                .map_err(configuration_error)?,
            record_id: request.export_job_id.clone(),
        };
        let payload = persisted_json_payload_with_data_class(
            job_contract(),
            DataClass::Personal,
            state.clone(),
        )?;
        let aggregate_version = expected_version.map_or(1, |version| version + 1);
        let mutation = match expected_version {
            None => RecordMutation::Create {
                reference: reference.clone(),
                payload,
            },
            Some(expected_version) => RecordMutation::Update {
                reference: reference.clone(),
                expected_version,
                payload,
            },
        };
        let operation = status.label();
        let evidence_id = stable_id("privacy-export-event", request, operation);
        let event_payload_bytes = event_payload(operation, sha256(&state));
        let event_payload = TypedPayload {
            owner: module_id()?,
            schema_id: SchemaId::try_new(PRIVACY_EXPORT_EVENT_SCHEMA_ID)
                .map_err(configuration_error)?,
            schema_version: SchemaVersion::try_new(PRIVACY_EXPORT_JOB_STATE_SCHEMA_VERSION)
                .map_err(configuration_error)?,
            descriptor_hash: sha256(PRIVACY_EXPORT_EVENT_DESCRIPTOR),
            data_class: DataClass::Personal,
            encoding: PayloadEncoding::Json,
            maximum_size_bytes: 1024,
            retention_policy_id: blueprint.retention_policy_id.clone(),
            bytes: event_payload_bytes,
        };
        event_payload.validate()?;
        let event_type = EventType::try_new(match status {
            JobStatus::Prepared => PRIVACY_EXPORT_PREPARED_EVENT_TYPE,
            JobStatus::Completed => PRIVACY_EXPORT_COMPLETED_EVENT_TYPE,
        })
        .map_err(configuration_error)?;
        let event = EventEvidence {
            event_id: evidence_id.clone(),
            event: DomainEvent {
                event_type,
                aggregate: reference,
                expected_aggregate_version: expected_version,
                deduplication_key: evidence_id,
                payload: event_payload,
            },
            aggregate_version,
            event_sequence: aggregate_version,
            occurred_at_unix_nanos: request.prepared_at_unix_nanos,
        };
        let request_hash = request_hash(operation, &state);
        let audit = AuditIntent {
            audit_record_id: stable_id("privacy-export-audit", request, operation),
            canonicalization_profile: "crm.cjson/v1".to_owned(),
            canonical_envelope: audit_envelope(operation, &request_hash, &state),
            occurred_at_unix_nanos: request.prepared_at_unix_nanos,
        };
        let plan = BatchMutationPlan {
            context: context.clone(),
            records: vec![mutation],
            relationships: Vec::new(),
            events: vec![event],
            idempotency: IdempotencyEvidence {
                scope: format!(
                    "{MODULE_ID}:{PRIVACY_EXPORT_REQUEST_CAPABILITY}@{PRIVACY_EXPORT_REQUEST_VERSION}:{operation}"
                ),
                key: context.execution.idempotency_key.as_str().to_owned(),
                request_hash,
                expires_at_unix_nanos: request
                    .prepared_at_unix_nanos
                    .checked_add(IDEMPOTENCY_TTL_NANOS)
                    .ok_or_else(|| job_state_invalid("idempotency expiry overflowed"))?,
            },
            audits: vec![audit],
        };
        self.store
            .execute_batch(&plan)
            .await
            .map(|_| ())
            .map_err(batch_error_to_sdk)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JobStatus {
    Prepared,
    Completed,
}

impl JobStatus {
    const fn label(self) -> &'static str {
        match self {
            Self::Prepared => "prepared",
            Self::Completed => "completed",
        }
    }
}

#[derive(Debug, Clone)]
struct PrivacyExportBlueprint {
    file_id: FileId,
    content_sha256: [u8; 32],
    size_bytes: u64,
    retention_policy_id: RetentionPolicyId,
}

impl PrivacyExportBlueprint {
    fn build(request: &PrivacyManifestExportRequest) -> Result<Self, SdkError> {
        if request.prepared_at_unix_nanos <= 0
            || request.manifest_digest.iter().all(|byte| *byte == 0)
            || request.manifest_bytes.is_empty()
            || request.manifest_bytes.len() as u64 > 2 * 1024 * 1024
        {
            return Err(SdkError::invalid_argument(
                "customer_data.export.privacy.request",
                "Privacy export manifest evidence is invalid.",
            ));
        }
        IdempotencyKey::try_new(request.target_idempotency_key.clone())
            .map_err(configuration_error)?;
        let file_id = FileId::try_new(format!("{FILE_ID_PREFIX}{}", hex(&request.manifest_digest)))
            .map_err(configuration_error)?;
        Ok(Self {
            file_id,
            content_sha256: sha256(&request.manifest_bytes),
            size_bytes: request.manifest_bytes.len() as u64,
            retention_policy_id: RetentionPolicyId::try_new(PRIVACY_EXPORT_RETENTION_POLICY_ID)
                .map_err(configuration_error)?,
        })
    }
}

fn execution_context(
    request: &PrivacyManifestExportRequest,
    operation: &str,
) -> Result<ModuleExecutionContext, SdkError> {
    let identity = format!("{}-{operation}", request.target_idempotency_key);
    Ok(ModuleExecutionContext {
        module_id: module_id()?,
        execution: ExecutionContext {
            tenant_id: request.tenant_id.clone(),
            actor_id: request.actor_id.clone(),
            request_id: RequestId::try_new(format!(
                "privacy-export-request-{}-{operation}",
                short_hex(&request.manifest_digest)
            ))
            .map_err(configuration_error)?,
            correlation_id: request.correlation_id.clone(),
            causation_id: CausationId::try_new(request.manifest_id.as_str().to_owned())
                .map_err(configuration_error)?,
            trace_id: request.trace_id.clone(),
            capability_id: CapabilityId::try_new(PRIVACY_EXPORT_REQUEST_CAPABILITY)
                .map_err(configuration_error)?,
            capability_version: CapabilityVersion::try_new(PRIVACY_EXPORT_REQUEST_VERSION)
                .map_err(configuration_error)?,
            business_transaction_id: BusinessTransactionId::try_new(identity.clone())
                .map_err(configuration_error)?,
            idempotency_key: IdempotencyKey::try_new(identity).map_err(configuration_error)?,
            schema_version: SchemaVersion::try_new(PRIVACY_EXPORT_REQUEST_VERSION)
                .map_err(configuration_error)?,
            request_started_at_unix_nanos: request.prepared_at_unix_nanos,
        },
    })
}

fn validate_job_snapshot(
    snapshot: &crm_module_sdk::RecordSnapshot,
    blueprint: &PrivacyExportBlueprint,
    status: JobStatus,
) -> Result<(), SdkError> {
    let expected_version = match status {
        JobStatus::Prepared => 1,
        JobStatus::Completed => 2,
    };
    if snapshot.version != expected_version
        || snapshot.payload.owner.as_str() != MODULE_ID
        || snapshot.payload.schema_id.as_str() != PRIVACY_EXPORT_JOB_STATE_SCHEMA_ID
        || snapshot.payload.schema_version.as_str() != PRIVACY_EXPORT_JOB_STATE_SCHEMA_VERSION
        || snapshot.payload.descriptor_hash != sha256(PRIVACY_EXPORT_JOB_DESCRIPTOR)
        || snapshot.payload.data_class != DataClass::Personal
        || snapshot.payload.encoding != PayloadEncoding::Json
        || snapshot.payload.maximum_size_bytes != PRIVACY_EXPORT_JOB_STATE_MAXIMUM_BYTES
        || snapshot.payload.retention_policy_id != blueprint.retention_policy_id
    {
        return Err(job_state_invalid(
            "privacy export job persistence envelope is invalid",
        ));
    }
    Ok(())
}

fn validate_artifact_metadata(
    metadata: &FileArtifactMetadata,
    blueprint: &PrivacyExportBlueprint,
) -> Result<(), SdkError> {
    metadata.validate()?;
    if metadata.file_id != blueprint.file_id
        || metadata.owner_module_id.as_str() != MODULE_ID
        || metadata.media_type != PRIVACY_EXPORT_MEDIA_TYPE
        || metadata.data_class != DataClass::Personal
        || metadata.retention_policy_id != blueprint.retention_policy_id
        || metadata.expected_size_bytes != blueprint.size_bytes
        || metadata.expected_sha256 != blueprint.content_sha256
    {
        return Err(job_state_invalid(
            "privacy export artifact metadata differs from deterministic blueprint",
        ));
    }
    Ok(())
}

fn validate_finalized_artifact(
    metadata: &FileArtifactMetadata,
    blueprint: &PrivacyExportBlueprint,
) -> Result<(), SdkError> {
    validate_artifact_metadata(metadata, blueprint)?;
    if metadata.status != FileArtifactStatus::Finalized
        || metadata.received_size_bytes != blueprint.size_bytes
    {
        return Err(job_state_invalid(
            "privacy export artifact was not finalized exactly",
        ));
    }
    Ok(())
}

fn job_state_bytes(
    request: &PrivacyManifestExportRequest,
    blueprint: &PrivacyExportBlueprint,
    status: JobStatus,
) -> Vec<u8> {
    format!(
        "{{\"content_sha256\":\"{}\",\"export_job_id\":\"{}\",\"file_id\":\"{}\",\"manifest_digest\":\"{}\",\"manifest_id\":\"{}\",\"prepared_at_unix_nanos\":{},\"privacy_case_id\":\"{}\",\"retention_policy_id\":\"{}\",\"size_bytes\":{},\"status\":\"{}\",\"target_idempotency_key\":\"{}\",\"tenant_id\":\"{}\"}}",
        hex(&blueprint.content_sha256),
        request.export_job_id,
        blueprint.file_id,
        hex(&request.manifest_digest),
        request.manifest_id,
        request.prepared_at_unix_nanos,
        request.privacy_case_id,
        blueprint.retention_policy_id,
        blueprint.size_bytes,
        status.label(),
        request.target_idempotency_key,
        request.tenant_id,
    )
    .into_bytes()
}

fn event_payload(operation: &str, state_sha256: [u8; 32]) -> Vec<u8> {
    format!(
        "{{\"job_state_sha256\":\"{}\",\"operation\":\"{}\"}}",
        hex(&state_sha256),
        operation
    )
    .into_bytes()
}

fn audit_envelope(operation: &str, request_hash: &[u8; 32], state: &[u8]) -> Vec<u8> {
    format!(
        "{{\"operation\":\"{}\",\"request_hash\":\"{}\",\"state_hash\":\"{}\"}}",
        operation,
        hex(request_hash),
        hex(&sha256(state))
    )
    .into_bytes()
}

fn job_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: PRIVACY_EXPORT_JOB_STATE_SCHEMA_ID,
        schema_version: PRIVACY_EXPORT_JOB_STATE_SCHEMA_VERSION,
        descriptor_hash: sha256(PRIVACY_EXPORT_JOB_DESCRIPTOR),
        maximum_size_bytes: PRIVACY_EXPORT_JOB_STATE_MAXIMUM_BYTES,
        retention_policy_id: PRIVACY_EXPORT_RETENTION_POLICY_ID,
    }
}

fn request_hash(operation: &str, state: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hash_field(&mut hasher, operation.as_bytes());
    hash_field(&mut hasher, state);
    hasher.finalize().into()
}

fn stable_id(domain: &str, request: &PrivacyManifestExportRequest, operation: &str) -> String {
    let mut hasher = Sha256::new();
    hash_field(&mut hasher, domain.as_bytes());
    hash_field(&mut hasher, request.tenant_id.as_str().as_bytes());
    hash_field(&mut hasher, request.export_job_id.as_str().as_bytes());
    hash_field(&mut hasher, operation.as_bytes());
    format!("{domain}-{}", hex(&hasher.finalize().into()))
}

fn hash_field(hasher: &mut Sha256, field: &[u8]) {
    hasher.update((field.len() as u64).to_be_bytes());
    hasher.update(field);
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn module_id() -> Result<ModuleId, SdkError> {
    ModuleId::try_new(MODULE_ID).map_err(configuration_error)
}

fn short_hex(bytes: &[u8; 32]) -> String {
    hex(&bytes[..12])
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn batch_error_to_sdk(error: BatchError) -> SdkError {
    match error {
        BatchError::Sdk(error) => error,
        BatchError::Conflict(message) | BatchError::InvalidPlan(message) => {
            job_state_invalid(message)
        }
        BatchError::IdempotencyKeyReused => SdkError::new(
            "CUSTOMER_DATA_PRIVACY_EXPORT_IDEMPOTENCY_CONFLICT",
            ErrorCategory::Conflict,
            false,
            "The privacy export idempotency key was reused for different input.",
        ),
        BatchError::IdempotencyInProgress => SdkError::new(
            "CUSTOMER_DATA_PRIVACY_EXPORT_IN_PROGRESS",
            ErrorCategory::Conflict,
            true,
            "The privacy export job is already being persisted.",
        ),
        BatchError::Database(error) => SdkError::new(
            "CUSTOMER_DATA_PRIVACY_EXPORT_STORE_UNAVAILABLE",
            ErrorCategory::Unavailable,
            true,
            "The privacy export job could not be persisted temporarily.",
        )
        .with_internal_reference(error.to_string()),
        BatchError::InvalidStoredValue(message) => SdkError::new(
            "CUSTOMER_DATA_PRIVACY_EXPORT_REPLAY_INVALID",
            ErrorCategory::Unavailable,
            true,
            "Stored privacy export replay evidence is temporarily unavailable.",
        )
        .with_internal_reference(message),
    }
}

fn configuration_error(error: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_PRIVACY_EXPORT_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The privacy export target is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}

fn job_state_invalid(error: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_PRIVACY_EXPORT_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "Customer Data Operations privacy export state is invalid.",
    )
    .with_internal_reference(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frozen_target_coordinate_and_private_job_contract_are_exact() {
        assert_eq!(
            format!("{PRIVACY_EXPORT_REQUEST_CAPABILITY}@{PRIVACY_EXPORT_REQUEST_VERSION}"),
            "customer_data.export.privacy.request@1.0.0"
        );
        assert_eq!(
            PRIVACY_EXPORT_JOB_RECORD_TYPE,
            "customer_data.privacy_export_job"
        );
        assert_ne!(sha256(PRIVACY_EXPORT_JOB_DESCRIPTOR), [0; 32]);
    }
}
