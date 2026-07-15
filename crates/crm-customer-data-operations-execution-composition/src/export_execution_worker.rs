use crate::{
    PartyExportExecutionSource, PartyExportExecutionSourceKind, PartyExportExecutionSourceRequest,
    PartyExportExecutionSourceResult, PostgresPartyExportExecutionReader,
    PostgresPartyExportExecutionSink, PostgresPartyExportSelectionReader,
};
use crm_core_data::{PostgresDataStore, RecordListQuery, RecordQueryContinuation, RecordQuerySort};
use crm_core_files::{
    AppendImmutableFileChunk, CreateImmutableFileArtifact, FileArtifactStatus,
    ImmutableFileArtifactStore,
};
use crm_customer_data_operations::{
    ExportJobId, PARTY_EXPORT_CSV_MEDIA_TYPE, PartyExportArtifactEvidence,
    PartyExportExclusionReason, PartyExportExecutionOutcomeKind, PartyExportExecutionStage,
    PartyExportExecutionStageKind, PartyExportField, PartyExportJob, PartyExportJobStatus,
    PartyExportSelectionItem, bounded_party_export_selection_manifest_sha256,
    canonical_party_export_csv_record, reconcile_durable_party_export_outcomes,
};
use crm_customer_data_operations_capability_adapter::{
    EXPORT_JOB_RECORD_TYPE, MODULE_ID as CUSTOMER_DATA_OPERATIONS_MODULE_ID,
    export_job_from_snapshot,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, Clock,
    CorrelationId, DataClass, ExecutionContext, FileId, IdempotencyKey, ModuleExecutionContext,
    ModuleId, PortFuture, RecordId, RecordType, RequestId, RetentionPolicyId, SchemaVersion,
    SdkError, TenantId, TraceId,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex};

pub const EXPORT_EXECUTION_WORKER_ACTOR_ID: &str = "crm-api-export-execution-worker";
pub const EXPORT_EXECUTION_WORKER_CAPABILITY_ID: &str =
    "customer_data.export.party.internal.execution_cycle";
pub const EXPORT_EXECUTION_WORKER_CAPABILITY_VERSION: &str = "1.0.0";
pub const DEFAULT_EXPORT_EXECUTION_SCAN_PAGE_SIZE: u32 = 100;

const ARTIFACT_ID_DOMAIN: &[u8] = b"crm.customer-data-operations.party-export-artifact/v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportExecutionTenantCycle {
    pub scanned_jobs: u32,
    pub staged_jobs: u32,
    pub checkpointed_jobs: u32,
    pub completed_jobs: u32,
    pub has_more: bool,
}

#[derive(Clone)]
pub struct PartyExportExecutionWorker {
    store: PostgresDataStore,
    selection_reader: Arc<PostgresPartyExportSelectionReader>,
    execution_reader: Arc<PostgresPartyExportExecutionReader>,
    sink: Arc<PostgresPartyExportExecutionSink>,
    source: Arc<dyn PartyExportExecutionSource>,
    file_store: Arc<dyn ImmutableFileArtifactStore>,
    clock: Arc<dyn Clock>,
    actor_id: ActorId,
    page_size: u32,
    scan_cursors: Arc<Mutex<BTreeMap<TenantId, Option<RecordQueryContinuation>>>>,
}

impl fmt::Debug for PartyExportExecutionWorker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PartyExportExecutionWorker")
            .field("store", &self.store)
            .field("selection_reader", &self.selection_reader)
            .field("execution_reader", &self.execution_reader)
            .field("sink", &self.sink)
            .field("source", &"dyn PartyExportExecutionSource")
            .field("file_store", &"dyn ImmutableFileArtifactStore")
            .field("clock", &"dyn Clock")
            .field("actor_id", &self.actor_id)
            .field("page_size", &self.page_size)
            .finish()
    }
}

impl PartyExportExecutionWorker {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        store: PostgresDataStore,
        selection_reader: Arc<PostgresPartyExportSelectionReader>,
        execution_reader: Arc<PostgresPartyExportExecutionReader>,
        sink: Arc<PostgresPartyExportExecutionSink>,
        source: Arc<dyn PartyExportExecutionSource>,
        file_store: Arc<dyn ImmutableFileArtifactStore>,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, SdkError> {
        Ok(Self {
            store,
            selection_reader,
            execution_reader,
            sink,
            source,
            file_store,
            clock,
            actor_id: ActorId::try_new(EXPORT_EXECUTION_WORKER_ACTOR_ID)
                .map_err(configuration_error)?,
            page_size: DEFAULT_EXPORT_EXECUTION_SCAN_PAGE_SIZE,
            scan_cursors: Arc::new(Mutex::new(BTreeMap::new())),
        })
    }

    pub fn actor_id(&self) -> &ActorId {
        &self.actor_id
    }

    pub fn run_tenant_cycle<'a>(
        &'a self,
        tenant_id: TenantId,
    ) -> PortFuture<'a, Result<ExportExecutionTenantCycle, SdkError>> {
        Box::pin(async move {
            let after = self
                .scan_cursors
                .lock()
                .map_err(|_| worker_state_unavailable())?
                .get(&tenant_id)
                .cloned()
                .flatten();
            let page = self
                .store
                .list_records_for_query(&RecordListQuery {
                    tenant_id: tenant_id.clone(),
                    owner_module_id: module_id()?,
                    record_type: RecordType::try_new(EXPORT_JOB_RECORD_TYPE)
                        .map_err(configuration_error)?,
                    page_size: self.page_size,
                    sort: RecordQuerySort::CreatedAtAscending,
                    after,
                })
                .await?;
            let next_cursor = page.next.clone();
            self.scan_cursors
                .lock()
                .map_err(|_| worker_state_unavailable())?
                .insert(tenant_id.clone(), next_cursor.clone());

            let scanned_jobs =
                u32::try_from(page.records.len()).map_err(|_| worker_state_unavailable())?;
            let mut staged_jobs = 0_u32;
            let mut checkpointed_jobs = 0_u32;
            let mut completed_jobs = 0_u32;

            'jobs: for record in page.records {
                let job = export_job_from_snapshot(&record)?;
                if job.status() != PartyExportJobStatus::Executing {
                    continue;
                }
                let selection = job.selection().ok_or_else(worker_state_invalid)?;
                let selection_evidence = self
                    .selection_reader
                    .load_evidence(&tenant_id, &job)
                    .await?;
                let selected_from_progress = selection_evidence
                    .progress
                    .next_manifest_position()
                    .checked_sub(1)
                    .ok_or_else(worker_state_invalid)?;
                let terminal = (selection_evidence.progress.source_exhausted()
                    && selection_evidence.progress.continuation().is_none())
                    || selected_from_progress == selection_evidence.progress.maximum_resources();
                if selected_from_progress != selection.selected_resources() || !terminal {
                    return Err(worker_state_invalid());
                }
                let manifest = self
                    .selection_reader
                    .load_manifest(&tenant_id, &job, &selection_evidence.progress)
                    .await?;
                if manifest.len()
                    != usize::try_from(selection.selected_resources())
                        .map_err(|_| worker_state_invalid())?
                    || bounded_party_export_selection_manifest_sha256(
                        &selection_evidence.boundary,
                        &manifest,
                    )? != selection.manifest_sha256()
                {
                    return Err(worker_state_invalid());
                }

                let mut stages = Vec::with_capacity(manifest.len());
                for item in &manifest {
                    if let Some(stage) = self
                        .execution_reader
                        .load_stage(&tenant_id, job.job_id(), item.manifest_position())
                        .await?
                    {
                        if stage.job_id() != job.job_id()
                            || stage.manifest_position() != item.manifest_position()
                        {
                            return Err(worker_state_invalid());
                        }
                        stages.push(stage);
                        continue;
                    }

                    let now = self.clock.now_unix_nanos();
                    let party_id = RecordId::try_new(item.party_id().as_str())
                        .map_err(configuration_error)?;
                    let source = self
                        .source
                        .get(PartyExportExecutionSourceRequest {
                            tenant_id: &tenant_id,
                            actor_id: &self.actor_id,
                            job_id: job.job_id().as_str(),
                            party_id: &party_id,
                            expected_resource_version: item.party_resource_version(),
                            request_started_at_unix_nanos: now,
                        })
                        .await?;
                    let stage = stage_from_source(&job, item, source, now)?;
                    let context = worker_context(
                        &tenant_id,
                        &self.actor_id,
                        job.job_id(),
                        "stage",
                        item.manifest_position(),
                        now,
                    )?;
                    self.sink.stage(&context, &stage).await?;
                    staged_jobs = staged_jobs.saturating_add(1);
                    continue 'jobs;
                }

                let blueprint = artifact_blueprint(&job, &stages)?;
                let artifact_context = worker_context(
                    &tenant_id,
                    &self.actor_id,
                    job.job_id(),
                    "artifact",
                    job.checkpoint_manifest_position(),
                    self.clock.now_unix_nanos(),
                )?;
                let metadata = self
                    .file_store
                    .create(
                        &artifact_context,
                        CreateImmutableFileArtifact {
                            file_id: blueprint.file_id.clone(),
                            owner_module_id: module_id()?,
                            media_type: PARTY_EXPORT_CSV_MEDIA_TYPE.to_owned(),
                            data_class: DataClass::Personal,
                            retention_policy_id: RetentionPolicyId::try_new(
                                job.specification().profile().retention_policy_id(),
                            )
                            .map_err(configuration_error)?,
                            expected_size_bytes: blueprint.size_bytes,
                            expected_sha256: blueprint.sha256,
                        },
                    )
                    .await?;
                if metadata.status == FileArtifactStatus::Uploading {
                    self.file_store
                        .append_chunk(
                            &artifact_context,
                            AppendImmutableFileChunk {
                                file_id: blueprint.file_id.clone(),
                                chunk_index: 0,
                                chunk_sha256: sha256(&blueprint.header),
                                bytes: blueprint.header.clone(),
                            },
                        )
                        .await?;
                }

                if job.checkpoint_manifest_position() < selection.selected_resources() {
                    if metadata.status == FileArtifactStatus::Finalized {
                        return Err(worker_state_invalid());
                    }
                    let position = job
                        .checkpoint_manifest_position()
                        .checked_add(1)
                        .ok_or_else(worker_state_invalid)?;
                    let stage = stages
                        .get(usize::try_from(position - 1).map_err(|_| worker_state_invalid())?)
                        .ok_or_else(worker_state_invalid)?;
                    let outcome_context = worker_context(
                        &tenant_id,
                        &self.actor_id,
                        job.job_id(),
                        "outcome",
                        position,
                        self.clock.now_unix_nanos(),
                    )?;
                    match stage.kind() {
                        PartyExportExecutionStageKind::Excluded(reason) => {
                            self.sink
                                .commit_excluded(&outcome_context, &job, position, *reason)
                                .await?;
                        }
                        PartyExportExecutionStageKind::Emitted {
                            row_utf8,
                            row_sha256,
                            redacted_fields,
                        } => {
                            let prior = self
                                .execution_reader
                                .load_outcomes(
                                    &tenant_id,
                                    job.job_id(),
                                    job.checkpoint_manifest_position(),
                                )
                                .await?;
                            let emitted_before = prior
                                .iter()
                                .filter(|outcome| {
                                    matches!(
                                        outcome.kind(),
                                        PartyExportExecutionOutcomeKind::Emitted { .. }
                                    )
                                })
                                .count();
                            let chunk_index = u32::try_from(emitted_before + 1)
                                .map_err(|_| worker_state_invalid())?;
                            let chunk_sha256 = decode_sha256(row_sha256)?;
                            self.file_store
                                .append_chunk(
                                    &artifact_context,
                                    AppendImmutableFileChunk {
                                        file_id: blueprint.file_id.clone(),
                                        chunk_index: u64::from(chunk_index),
                                        chunk_sha256,
                                        bytes: row_utf8.as_bytes().to_vec(),
                                    },
                                )
                                .await?;
                            let chunk_size_bytes =
                                u64::try_from(row_utf8.len()).map_err(|_| worker_state_invalid())?;
                            self.sink
                                .commit_emitted(
                                    &outcome_context,
                                    &job,
                                    position,
                                    chunk_index,
                                    chunk_sha256,
                                    chunk_size_bytes,
                                    *redacted_fields,
                                )
                                .await?;
                        }
                    }
                    checkpointed_jobs = checkpointed_jobs.saturating_add(1);
                    continue;
                }

                let finalized = match metadata.status {
                    FileArtifactStatus::Uploading => {
                        self.file_store
                            .finalize(&artifact_context, &blueprint.file_id)
                            .await?
                    }
                    FileArtifactStatus::Finalized => metadata,
                };
                let outcomes = self
                    .execution_reader
                    .load_outcomes(&tenant_id, job.job_id(), selection.selected_resources())
                    .await?;
                let reconciliation = reconcile_durable_party_export_outcomes(
                    job.job_id(),
                    selection.selected_resources(),
                    &outcomes,
                )?;
                let artifact = PartyExportArtifactEvidence::try_new(
                    finalized.file_id.clone(),
                    hex(finalized.expected_sha256),
                    finalized.expected_size_bytes,
                    finalized.retention_policy_id.as_str(),
                )?;
                let completion_context = worker_context(
                    &tenant_id,
                    &self.actor_id,
                    job.job_id(),
                    "complete",
                    selection.selected_resources(),
                    self.clock.now_unix_nanos(),
                )?;
                self.sink
                    .complete(&completion_context, &job, &artifact, &reconciliation)
                    .await?;
                completed_jobs = completed_jobs.saturating_add(1);
            }

            Ok(ExportExecutionTenantCycle {
                scanned_jobs,
                staged_jobs,
                checkpointed_jobs,
                completed_jobs,
                has_more: next_cursor.is_some(),
            })
        })
    }
}

struct ArtifactBlueprint {
    file_id: FileId,
    header: Vec<u8>,
    size_bytes: u64,
    sha256: [u8; 32],
}

fn artifact_blueprint(
    job: &PartyExportJob,
    stages: &[PartyExportExecutionStage],
) -> Result<ArtifactBlueprint, SdkError> {
    let header_cells = job
        .specification()
        .profile()
        .fields()
        .iter()
        .map(|field| field.canonical_name().to_owned())
        .collect::<Vec<_>>();
    let header = canonical_party_export_csv_record(&header_cells)?;
    let mut hasher = Sha256::new();
    hasher.update(&header);
    let mut size_bytes = u64::try_from(header.len()).map_err(|_| worker_state_invalid())?;
    for stage in stages {
        if let PartyExportExecutionStageKind::Emitted { row_utf8, .. } = stage.kind() {
            hasher.update(row_utf8.as_bytes());
            size_bytes = size_bytes
                .checked_add(u64::try_from(row_utf8.len()).map_err(|_| worker_state_invalid())?)
                .ok_or_else(worker_state_invalid)?;
        }
    }
    let sha256: [u8; 32] = hasher.finalize().into();

    let mut id_hasher = Sha256::new();
    id_hasher.update(ARTIFACT_ID_DOMAIN);
    hash_part(&mut id_hasher, job.job_id().as_str().as_bytes());
    hash_part(
        &mut id_hasher,
        job.specification().version_id().as_str().as_bytes(),
    );
    hash_part(
        &mut id_hasher,
        job.selection()
            .ok_or_else(worker_state_invalid)?
            .manifest_sha256()
            .as_bytes(),
    );
    let file_id = FileId::try_new(format!(
        "cdo-export-artifact-{}",
        hex(id_hasher.finalize().into())
    ))
    .map_err(configuration_error)?;

    Ok(ArtifactBlueprint {
        file_id,
        header,
        size_bytes,
        sha256,
    })
}

fn stage_from_source(
    job: &PartyExportJob,
    item: &PartyExportSelectionItem,
    source: PartyExportExecutionSourceResult,
    occurred_at_unix_nanos: i64,
) -> Result<PartyExportExecutionStage, SdkError> {
    match source {
        PartyExportExecutionSourceResult::NotVisible => PartyExportExecutionStage::excluded(
            job.job_id().clone(),
            item.manifest_position(),
            PartyExportExclusionReason::NotVisible,
            occurred_at_unix_nanos,
        ),
        PartyExportExecutionSourceResult::VersionChanged => PartyExportExecutionStage::excluded(
            job.job_id().clone(),
            item.manifest_position(),
            PartyExportExclusionReason::VersionChanged,
            occurred_at_unix_nanos,
        ),
        PartyExportExecutionSourceResult::Unavailable => PartyExportExecutionStage::excluded(
            job.job_id().clone(),
            item.manifest_position(),
            PartyExportExclusionReason::Unavailable,
            occurred_at_unix_nanos,
        ),
        PartyExportExecutionSourceResult::Visible {
            party_id,
            kind,
            display_name,
            resource_version,
        } => {
            if party_id.as_str() != item.party_id().as_str()
                || resource_version != item.party_resource_version()
            {
                return Err(worker_state_invalid());
            }
            let mut redacted_fields = 0_u32;
            let mut cells = Vec::with_capacity(job.specification().profile().fields().len());
            for field in job.specification().profile().fields() {
                let value = match field {
                    PartyExportField::PartyId => party_id.as_str().to_owned(),
                    PartyExportField::Kind => match kind {
                        Some(PartyExportExecutionSourceKind::Person) => "person".to_owned(),
                        Some(PartyExportExecutionSourceKind::Organization) => {
                            "organization".to_owned()
                        }
                        None => {
                            redacted_fields = redacted_fields
                                .checked_add(1)
                                .ok_or_else(worker_state_invalid)?;
                            String::new()
                        }
                    },
                    PartyExportField::DisplayName => match &display_name {
                        Some(value) => value.clone(),
                        None => {
                            redacted_fields = redacted_fields
                                .checked_add(1)
                                .ok_or_else(worker_state_invalid)?;
                            String::new()
                        }
                    },
                    PartyExportField::ResourceVersion => resource_version.to_string(),
                };
                cells.push(value);
            }
            let row = canonical_party_export_csv_record(&cells)?;
            PartyExportExecutionStage::emitted(
                job.job_id().clone(),
                item.manifest_position(),
                String::from_utf8(row).map_err(|_| worker_state_invalid())?,
                redacted_fields,
                occurred_at_unix_nanos,
            )
        }
    }
}

fn worker_context(
    tenant_id: &TenantId,
    actor_id: &ActorId,
    job_id: &ExportJobId,
    phase: &str,
    position: u32,
    now_unix_nanos: i64,
) -> Result<ModuleExecutionContext, SdkError> {
    let identity = cycle_identity(job_id, phase, position);
    Ok(ModuleExecutionContext {
        module_id: module_id()?,
        execution: ExecutionContext {
            tenant_id: tenant_id.clone(),
            actor_id: actor_id.clone(),
            request_id: RequestId::try_new(identity.clone()).map_err(configuration_error)?,
            correlation_id: CorrelationId::try_new(job_id.as_str()).map_err(configuration_error)?,
            causation_id: CausationId::try_new(identity.clone()).map_err(configuration_error)?,
            trace_id: TraceId::try_new(job_id.as_str()).map_err(configuration_error)?,
            capability_id: CapabilityId::try_new(EXPORT_EXECUTION_WORKER_CAPABILITY_ID)
                .map_err(configuration_error)?,
            capability_version: CapabilityVersion::try_new(
                EXPORT_EXECUTION_WORKER_CAPABILITY_VERSION,
            )
            .map_err(configuration_error)?,
            idempotency_key: IdempotencyKey::try_new(identity.clone())
                .map_err(configuration_error)?,
            business_transaction_id: BusinessTransactionId::try_new(identity)
                .map_err(configuration_error)?,
            schema_version: SchemaVersion::try_new(EXPORT_EXECUTION_WORKER_CAPABILITY_VERSION)
                .map_err(configuration_error)?,
            request_started_at_unix_nanos: now_unix_nanos,
        },
    })
}

fn cycle_identity(job_id: &ExportJobId, phase: &str, position: u32) -> String {
    let mut hasher = Sha256::new();
    hash_part(&mut hasher, job_id.as_str().as_bytes());
    hash_part(&mut hasher, phase.as_bytes());
    hash_part(&mut hasher, &position.to_be_bytes());
    format!("cdo-export-cycle-{}", hex(hasher.finalize().into()))
}

fn module_id() -> Result<ModuleId, SdkError> {
    ModuleId::try_new(CUSTOMER_DATA_OPERATIONS_MODULE_ID).map_err(configuration_error)
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn decode_sha256(value: &str) -> Result<[u8; 32], SdkError> {
    if value.len() != 64 {
        return Err(worker_state_invalid());
    }
    let mut output = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let pair = std::str::from_utf8(pair).map_err(|_| worker_state_invalid())?;
        output[index] = u8::from_str_radix(pair, 16).map_err(|_| worker_state_invalid())?;
    }
    Ok(output)
}

fn hash_part(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}

fn hex(bytes: [u8; 32]) -> String {
    let mut value = String::with_capacity(64);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut value, "{byte:02x}").expect("writing to String cannot fail");
    }
    value
}

fn configuration_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_EXECUTION_WORKER_CONFIGURATION_INVALID",
        crm_module_sdk::ErrorCategory::Internal,
        false,
        "The customer export execution worker is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}

fn worker_state_invalid() -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_EXECUTION_WORKER_STATE_INVALID",
        crm_module_sdk::ErrorCategory::Internal,
        false,
        "The customer export execution worker encountered inconsistent state.",
    )
}

fn worker_state_unavailable() -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_EXECUTION_WORKER_STATE_UNAVAILABLE",
        crm_module_sdk::ErrorCategory::Unavailable,
        true,
        "The customer export execution worker is temporarily unavailable.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_identity_is_stable_for_same_job_phase_and_position() {
        let job_id = ExportJobId::try_new("execution-worker-identity-job").unwrap();
        assert_eq!(
            cycle_identity(&job_id, "stage", 1),
            cycle_identity(&job_id, "stage", 1)
        );
        assert_ne!(
            cycle_identity(&job_id, "stage", 1),
            cycle_identity(&job_id, "outcome", 1)
        );
    }
}
