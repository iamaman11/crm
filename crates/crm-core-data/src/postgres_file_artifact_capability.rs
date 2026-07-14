use crate::audit::materialize_audit_chain;
use crate::capability_executor::capability_idempotency_scope;
use crate::postgres_batch::{
    bind_execution_context, capability_idempotency, complete_capability_idempotency,
    insert_audit_record, insert_completion_marker, insert_idempotency_claim,
    load_capability_replay,
};
use crate::postgres_file_artifact_evidence::insert_file_artifact_outbox_event;
use crate::{AuditIntent, BatchError, BatchMutationPlan, EventEvidence, PostgresDataStore};
use crm_capability_runtime::{CapabilityDefinition, CapabilityExecutionResult, CapabilityRequest};
use crm_core_files::{
    AppendImmutableFileChunk, CreateImmutableFileArtifact, FileArtifactMetadata, FileArtifactStatus,
};
use crm_module_sdk::{
    DataClass, ErrorCategory, FileId, ModuleExecutionContext, ModuleId, ResourceRef,
    RetentionPolicyId, SdkError, TypedPayload,
};
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Row, Transaction};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileArtifactCapabilityMutation {
    Create(CreateImmutableFileArtifact),
    AppendChunk(AppendImmutableFileChunk),
    Finalize { file_id: FileId },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileArtifactCapabilityMutationResult {
    pub metadata: FileArtifactMetadata,
    pub changed: bool,
    pub chunk_replayed: bool,
}

#[derive(Debug, Clone)]
pub struct FileArtifactCapabilityEvidence {
    pub output: TypedPayload,
    pub events: Vec<EventEvidence>,
    pub audits: Vec<AuditIntent>,
    pub affected_resources: Vec<ResourceRef>,
}

impl FileArtifactCapabilityEvidence {
    fn validate(&self) -> Result<(), BatchError> {
        self.output.validate().map_err(BatchError::Sdk)?;
        if self.audits.is_empty() {
            return Err(BatchError::InvalidPlan(
                "file artifact capability requires at least one audit record".to_owned(),
            ));
        }
        for event in &self.events {
            event.event.payload.validate().map_err(BatchError::Sdk)?;
            if event.event_id.is_empty()
                || event.aggregate_version <= 0
                || event.event_sequence <= 0
            {
                return Err(BatchError::InvalidPlan(
                    "file artifact event evidence is invalid".to_owned(),
                ));
            }
        }
        for audit in &self.audits {
            audit.validate().map_err(BatchError::InvalidPlan)?;
        }
        Ok(())
    }
}

impl PostgresDataStore {
    pub async fn execute_file_artifact_capability<F>(
        &self,
        definition: &CapabilityDefinition,
        request: CapabilityRequest,
        mutation: FileArtifactCapabilityMutation,
        evidence_builder: F,
    ) -> Result<CapabilityExecutionResult, BatchError>
    where
        F: FnOnce(
            &FileArtifactCapabilityMutationResult,
            &CapabilityRequest,
        ) -> Result<FileArtifactCapabilityEvidence, SdkError>,
    {
        if !definition.mutation || !definition.requires_idempotency {
            return Err(BatchError::InvalidPlan(
                "file artifact capability definition must be an idempotent mutation".to_owned(),
            ));
        }
        request.context.validate().map_err(BatchError::Sdk)?;
        let idempotency =
            capability_idempotency(&request, capability_idempotency_scope(definition))?;
        let mut transaction = self.pool().begin().await?;
        bind_execution_context(&mut transaction, &request.context).await?;

        if let Some(result) =
            load_capability_replay(&mut transaction, &request.context, &idempotency).await?
        {
            transaction.commit().await?;
            return Ok(result);
        }

        insert_idempotency_claim(&mut transaction, &request.context, &idempotency).await?;
        let mutation_result =
            apply_file_artifact_mutation(&mut transaction, &request.context, mutation).await?;
        let evidence = evidence_builder(&mutation_result, &request).map_err(BatchError::Sdk)?;
        evidence.validate()?;
        let result = CapabilityExecutionResult {
            output: Some(evidence.output),
            affected_resources: evidence.affected_resources,
            replayed: false,
        };
        let evidence_plan = BatchMutationPlan {
            context: request.context.clone(),
            records: Vec::new(),
            relationships: Vec::new(),
            events: evidence.events,
            idempotency,
            audits: evidence.audits,
        };

        for event in &evidence_plan.events {
            insert_file_artifact_outbox_event(&mut transaction, &evidence_plan.context, event)
                .await?;
        }
        let materialized = materialize_audit_chain(
            &mut transaction,
            &evidence_plan.context,
            &evidence_plan.audits,
        )
        .await
        .map_err(|error| {
            BatchError::Sdk(
                SdkError::new(
                    "FILE_ARTIFACT_AUDIT_MATERIALIZATION_FAILED",
                    ErrorCategory::Internal,
                    false,
                    "File artifact audit evidence could not be produced.",
                )
                .with_internal_reference(error.to_string()),
            )
        })?;
        for audit in &materialized {
            insert_audit_record(&mut transaction, &evidence_plan.context, audit).await?;
        }
        complete_capability_idempotency(&mut transaction, &evidence_plan, &result).await?;
        insert_completion_marker(&mut transaction, &evidence_plan).await?;
        transaction.commit().await?;
        Ok(result)
    }
}

async fn apply_file_artifact_mutation(
    transaction: &mut Transaction<'_, Postgres>,
    context: &ModuleExecutionContext,
    mutation: FileArtifactCapabilityMutation,
) -> Result<FileArtifactCapabilityMutationResult, BatchError> {
    match mutation {
        FileArtifactCapabilityMutation::Create(command) => {
            command.validate().map_err(BatchError::Sdk)?;
            if context.module_id != command.owner_module_id {
                return Err(BatchError::Sdk(owner_mismatch()));
            }
            let expected_size_bytes = i64::try_from(command.expected_size_bytes).map_err(|_| {
                BatchError::InvalidPlan("artifact size does not fit PostgreSQL bigint".to_owned())
            })?;
            let inserted = sqlx::query(
                r#"
                INSERT INTO crm.file_artifacts (
                  tenant_id,
                  file_id,
                  owner_module_id,
                  media_type,
                  data_class,
                  retention_policy_id,
                  expected_size_bytes,
                  expected_sha256,
                  status
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'uploading')
                ON CONFLICT (tenant_id, file_id) DO NOTHING
                "#,
            )
            .bind(context.execution.tenant_id.as_str())
            .bind(command.file_id.as_str())
            .bind(command.owner_module_id.as_str())
            .bind(&command.media_type)
            .bind(data_class_to_db(command.data_class))
            .bind(command.retention_policy_id.as_str())
            .bind(expected_size_bytes)
            .bind(command.expected_sha256.as_slice())
            .execute(&mut **transaction)
            .await?;
            let metadata = load_metadata_for_update(transaction, context, &command.file_id)
                .await?
                .ok_or_else(|| {
                    BatchError::InvalidStoredValue("created artifact is missing".to_owned())
                })?;
            if metadata.owner_module_id != command.owner_module_id
                || metadata.media_type != command.media_type
                || metadata.data_class != command.data_class
                || metadata.retention_policy_id != command.retention_policy_id
                || metadata.expected_size_bytes != command.expected_size_bytes
                || metadata.expected_sha256 != command.expected_sha256
            {
                return Err(BatchError::Sdk(file_conflict(
                    "FILE_ARTIFACT_CREATE_CONFLICT",
                    "The file artifact ID is already bound to different immutable metadata.",
                )));
            }
            Ok(FileArtifactCapabilityMutationResult {
                metadata,
                changed: inserted.rows_affected() == 1,
                chunk_replayed: false,
            })
        }
        FileArtifactCapabilityMutation::AppendChunk(command) => {
            command.validate().map_err(BatchError::Sdk)?;
            let computed_hash: [u8; 32] = Sha256::digest(&command.bytes).into();
            if computed_hash != command.chunk_sha256 {
                return Err(BatchError::Sdk(SdkError::invalid_argument(
                    "file_artifact.chunk.sha256",
                    "File artifact chunk SHA-256 does not match the supplied bytes",
                )));
            }
            let chunk_index = i64::try_from(command.chunk_index).map_err(|_| {
                BatchError::Sdk(SdkError::invalid_argument(
                    "file_artifact.chunk_index",
                    "Chunk index is too large",
                ))
            })?;
            let chunk_size = i32::try_from(command.bytes.len()).map_err(|_| {
                BatchError::Sdk(SdkError::invalid_argument(
                    "file_artifact.chunk.bytes",
                    "Chunk is too large",
                ))
            })?;
            let metadata = load_metadata_for_update(transaction, context, &command.file_id)
                .await?
                .ok_or_else(|| BatchError::Sdk(file_not_found()))?;
            require_owner(&metadata, context).map_err(BatchError::Sdk)?;
            if metadata.status == FileArtifactStatus::Finalized {
                return Err(BatchError::Sdk(file_conflict(
                    "FILE_ARTIFACT_ALREADY_FINALIZED",
                    "A finalized file artifact is immutable.",
                )));
            }
            if command.chunk_index < metadata.next_chunk_index {
                let existing = sqlx::query(
                    r#"
                    SELECT chunk_sha256, chunk_bytes
                      FROM crm.file_artifact_chunks
                     WHERE tenant_id = $1 AND file_id = $2 AND chunk_index = $3
                    "#,
                )
                .bind(context.execution.tenant_id.as_str())
                .bind(command.file_id.as_str())
                .bind(chunk_index)
                .fetch_optional(&mut **transaction)
                .await?
                .ok_or_else(|| {
                    BatchError::InvalidStoredValue(
                        "previously accepted artifact chunk is missing".to_owned(),
                    )
                })?;
                let stored_hash: Vec<u8> = existing
                    .try_get("chunk_sha256")
                    .map_err(|error| BatchError::InvalidStoredValue(error.to_string()))?;
                let stored_bytes: Vec<u8> = existing
                    .try_get("chunk_bytes")
                    .map_err(|error| BatchError::InvalidStoredValue(error.to_string()))?;
                if stored_hash.as_slice() != command.chunk_sha256 || stored_bytes != command.bytes {
                    return Err(BatchError::Sdk(file_conflict(
                        "FILE_ARTIFACT_CHUNK_REPLAY_CONFLICT",
                        "The chunk index was already used for different bytes.",
                    )));
                }
                return Ok(FileArtifactCapabilityMutationResult {
                    metadata,
                    changed: false,
                    chunk_replayed: true,
                });
            }
            if command.chunk_index != metadata.next_chunk_index {
                return Err(BatchError::Sdk(file_conflict(
                    "FILE_ARTIFACT_CHUNK_OUT_OF_ORDER",
                    "File artifact chunks must be appended in exact sequential order.",
                )));
            }
            let new_received = metadata
                .received_size_bytes
                .checked_add(command.bytes.len() as u64)
                .ok_or_else(|| {
                    BatchError::Sdk(file_conflict(
                        "FILE_ARTIFACT_SIZE_OVERFLOW",
                        "File artifact received size cannot advance further.",
                    ))
                })?;
            if new_received > metadata.expected_size_bytes {
                return Err(BatchError::Sdk(file_conflict(
                    "FILE_ARTIFACT_SIZE_EXCEEDED",
                    "File artifact chunks exceed the declared immutable size.",
                )));
            }
            sqlx::query(
                r#"
                INSERT INTO crm.file_artifact_chunks (
                  tenant_id, file_id, chunk_index, chunk_size_bytes, chunk_sha256, chunk_bytes
                ) VALUES ($1, $2, $3, $4, $5, $6)
                "#,
            )
            .bind(context.execution.tenant_id.as_str())
            .bind(command.file_id.as_str())
            .bind(chunk_index)
            .bind(chunk_size)
            .bind(command.chunk_sha256.as_slice())
            .bind(&command.bytes)
            .execute(&mut **transaction)
            .await?;
            sqlx::query(
                r#"
                UPDATE crm.file_artifacts
                   SET next_chunk_index = next_chunk_index + 1,
                       received_size_bytes = $3
                 WHERE tenant_id = $1 AND file_id = $2
                "#,
            )
            .bind(context.execution.tenant_id.as_str())
            .bind(command.file_id.as_str())
            .bind(i64::try_from(new_received).map_err(|_| {
                BatchError::InvalidPlan("artifact size does not fit PostgreSQL bigint".to_owned())
            })?)
            .execute(&mut **transaction)
            .await?;
            let updated = load_metadata_for_update(transaction, context, &command.file_id)
                .await?
                .ok_or_else(|| {
                    BatchError::InvalidStoredValue("updated artifact is missing".to_owned())
                })?;
            Ok(FileArtifactCapabilityMutationResult {
                metadata: updated,
                changed: true,
                chunk_replayed: false,
            })
        }
        FileArtifactCapabilityMutation::Finalize { file_id } => {
            let metadata = load_metadata_for_update(transaction, context, &file_id)
                .await?
                .ok_or_else(|| BatchError::Sdk(file_not_found()))?;
            require_owner(&metadata, context).map_err(BatchError::Sdk)?;
            if metadata.status == FileArtifactStatus::Finalized {
                return Ok(FileArtifactCapabilityMutationResult {
                    metadata,
                    changed: false,
                    chunk_replayed: false,
                });
            }
            if metadata.received_size_bytes != metadata.expected_size_bytes {
                return Err(BatchError::Sdk(file_conflict(
                    "FILE_ARTIFACT_UPLOAD_INCOMPLETE",
                    "The file artifact cannot be finalized before all declared bytes are uploaded.",
                )));
            }
            verify_chunks(transaction, context, &metadata).await?;
            sqlx::query(
                r#"
                UPDATE crm.file_artifacts
                   SET status = 'finalized', finalized_at = clock_timestamp()
                 WHERE tenant_id = $1 AND file_id = $2 AND status = 'uploading'
                "#,
            )
            .bind(context.execution.tenant_id.as_str())
            .bind(file_id.as_str())
            .execute(&mut **transaction)
            .await?;
            let finalized = load_metadata_for_update(transaction, context, &file_id)
                .await?
                .ok_or_else(|| {
                    BatchError::InvalidStoredValue("finalized artifact is missing".to_owned())
                })?;
            Ok(FileArtifactCapabilityMutationResult {
                metadata: finalized,
                changed: true,
                chunk_replayed: false,
            })
        }
    }
}

async fn load_metadata_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    context: &ModuleExecutionContext,
    file_id: &FileId,
) -> Result<Option<FileArtifactMetadata>, BatchError> {
    let row = sqlx::query(
        r#"
        SELECT file_id, owner_module_id, media_type, data_class, retention_policy_id,
               expected_size_bytes, expected_sha256, status, next_chunk_index, received_size_bytes
          FROM crm.file_artifacts
         WHERE tenant_id = $1 AND file_id = $2
         FOR UPDATE
        "#,
    )
    .bind(context.execution.tenant_id.as_str())
    .bind(file_id.as_str())
    .fetch_optional(&mut **transaction)
    .await?;
    row.map(decode_metadata).transpose()
}

async fn verify_chunks(
    transaction: &mut Transaction<'_, Postgres>,
    context: &ModuleExecutionContext,
    metadata: &FileArtifactMetadata,
) -> Result<(), BatchError> {
    let rows = sqlx::query(
        r#"
        SELECT chunk_index, chunk_size_bytes, chunk_sha256, chunk_bytes
          FROM crm.file_artifact_chunks
         WHERE tenant_id = $1 AND file_id = $2
         ORDER BY chunk_index ASC
        "#,
    )
    .bind(context.execution.tenant_id.as_str())
    .bind(metadata.file_id.as_str())
    .fetch_all(&mut **transaction)
    .await?;
    if rows.len() as u64 != metadata.next_chunk_index {
        return Err(BatchError::InvalidStoredValue(
            "artifact chunk count does not match next chunk index".to_owned(),
        ));
    }
    let capacity = usize::try_from(metadata.expected_size_bytes).map_err(|_| {
        BatchError::InvalidStoredValue("artifact size does not fit memory address space".to_owned())
    })?;
    let mut bytes = Vec::with_capacity(capacity);
    for (expected_index, row) in rows.into_iter().enumerate() {
        let chunk_index: i64 = row
            .try_get("chunk_index")
            .map_err(|error| BatchError::InvalidStoredValue(error.to_string()))?;
        let chunk_size: i32 = row
            .try_get("chunk_size_bytes")
            .map_err(|error| BatchError::InvalidStoredValue(error.to_string()))?;
        let chunk_hash: Vec<u8> = row
            .try_get("chunk_sha256")
            .map_err(|error| BatchError::InvalidStoredValue(error.to_string()))?;
        let chunk_bytes: Vec<u8> = row
            .try_get("chunk_bytes")
            .map_err(|error| BatchError::InvalidStoredValue(error.to_string()))?;
        if chunk_index != expected_index as i64
            || usize::try_from(chunk_size).ok() != Some(chunk_bytes.len())
            || chunk_hash.len() != 32
            || Sha256::digest(&chunk_bytes).as_slice() != chunk_hash.as_slice()
        {
            return Err(BatchError::InvalidStoredValue(
                "artifact chunk integrity validation failed".to_owned(),
            ));
        }
        bytes.extend_from_slice(&chunk_bytes);
    }
    let digest: [u8; 32] = Sha256::digest(&bytes).into();
    if bytes.len() as u64 != metadata.expected_size_bytes || digest != metadata.expected_sha256 {
        return Err(BatchError::Sdk(file_conflict(
            "FILE_ARTIFACT_DIGEST_MISMATCH",
            "File artifact bytes do not match the declared immutable size and SHA-256.",
        )));
    }
    Ok(())
}

fn decode_metadata(row: sqlx::postgres::PgRow) -> Result<FileArtifactMetadata, BatchError> {
    let expected_sha256: Vec<u8> = row
        .try_get("expected_sha256")
        .map_err(|error| BatchError::InvalidStoredValue(error.to_string()))?;
    let expected_sha256: [u8; 32] = expected_sha256.try_into().map_err(|_| {
        BatchError::InvalidStoredValue("artifact SHA-256 must contain exactly 32 bytes".to_owned())
    })?;
    let expected_size_bytes = u64::try_from(
        row.try_get::<i64, _>("expected_size_bytes")
            .map_err(|error| BatchError::InvalidStoredValue(error.to_string()))?,
    )
    .map_err(|_| BatchError::InvalidStoredValue("artifact expected size is negative".to_owned()))?;
    let next_chunk_index = u64::try_from(
        row.try_get::<i64, _>("next_chunk_index")
            .map_err(|error| BatchError::InvalidStoredValue(error.to_string()))?,
    )
    .map_err(|_| BatchError::InvalidStoredValue("artifact chunk index is negative".to_owned()))?;
    let received_size_bytes = u64::try_from(
        row.try_get::<i64, _>("received_size_bytes")
            .map_err(|error| BatchError::InvalidStoredValue(error.to_string()))?,
    )
    .map_err(|_| BatchError::InvalidStoredValue("artifact received size is negative".to_owned()))?;
    let metadata = FileArtifactMetadata {
        file_id: FileId::try_new(
            row.try_get::<String, _>("file_id")
                .map_err(|error| BatchError::InvalidStoredValue(error.to_string()))?,
        )
        .map_err(|error| BatchError::InvalidStoredValue(error.to_string()))?,
        owner_module_id: ModuleId::try_new(
            row.try_get::<String, _>("owner_module_id")
                .map_err(|error| BatchError::InvalidStoredValue(error.to_string()))?,
        )
        .map_err(|error| BatchError::InvalidStoredValue(error.to_string()))?,
        media_type: row
            .try_get("media_type")
            .map_err(|error| BatchError::InvalidStoredValue(error.to_string()))?,
        data_class: data_class_from_db(
            row.try_get::<String, _>("data_class")
                .map_err(|error| BatchError::InvalidStoredValue(error.to_string()))?
                .as_str(),
        )?,
        retention_policy_id: RetentionPolicyId::try_new(
            row.try_get::<String, _>("retention_policy_id")
                .map_err(|error| BatchError::InvalidStoredValue(error.to_string()))?,
        )
        .map_err(|error| BatchError::InvalidStoredValue(error.to_string()))?,
        expected_size_bytes,
        expected_sha256,
        status: match row
            .try_get::<String, _>("status")
            .map_err(|error| BatchError::InvalidStoredValue(error.to_string()))?
            .as_str()
        {
            "uploading" => FileArtifactStatus::Uploading,
            "finalized" => FileArtifactStatus::Finalized,
            _ => {
                return Err(BatchError::InvalidStoredValue(
                    "artifact status is invalid".to_owned(),
                ));
            }
        },
        next_chunk_index,
        received_size_bytes,
    };
    metadata.validate().map_err(BatchError::Sdk)?;
    Ok(metadata)
}

fn require_owner(
    metadata: &FileArtifactMetadata,
    context: &ModuleExecutionContext,
) -> Result<(), SdkError> {
    if metadata.owner_module_id == context.module_id {
        Ok(())
    } else {
        Err(owner_mismatch())
    }
}

fn data_class_to_db(value: DataClass) -> &'static str {
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

fn data_class_from_db(value: &str) -> Result<DataClass, BatchError> {
    match value {
        "public" => Ok(DataClass::Public),
        "internal" => Ok(DataClass::Internal),
        "confidential" => Ok(DataClass::Confidential),
        "restricted" => Ok(DataClass::Restricted),
        "personal" => Ok(DataClass::Personal),
        "sensitive_personal" => Ok(DataClass::SensitivePersonal),
        "biometric" => Ok(DataClass::Biometric),
        "financial" => Ok(DataClass::Financial),
        "credential" => Ok(DataClass::Credential),
        _ => Err(BatchError::InvalidStoredValue(
            "artifact data class is invalid".to_owned(),
        )),
    }
}

fn file_not_found() -> SdkError {
    SdkError::new(
        "FILE_ARTIFACT_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The requested file artifact was not found.",
    )
}

fn owner_mismatch() -> SdkError {
    SdkError::new(
        "FILE_ARTIFACT_OWNER_MISMATCH",
        ErrorCategory::Authorization,
        false,
        "The file artifact is not owned by the executing module.",
    )
}

fn file_conflict(code: &'static str, safe_message: &'static str) -> SdkError {
    SdkError::new(code, ErrorCategory::Conflict, false, safe_message)
}
