use crate::PostgresDataStore;
use crm_core_files::{
    AppendImmutableFileChunk, CreateImmutableFileArtifact, FileArtifactAppendResult,
    FileArtifactMetadata, FileArtifactStatus, FinalizedFileArtifact, ImmutableFileArtifactStore,
};
use crm_module_sdk::{
    DataClass, ErrorCategory, FileId, ModuleExecutionContext, ModuleId, PortFuture,
    RetentionPolicyId, SdkError,
};
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Row, Transaction};

#[derive(Debug, Clone)]
pub struct PostgresImmutableFileArtifactStore {
    store: PostgresDataStore,
}

impl PostgresImmutableFileArtifactStore {
    pub fn new(store: PostgresDataStore) -> Self {
        Self { store }
    }
}

impl ImmutableFileArtifactStore for PostgresImmutableFileArtifactStore {
    fn create<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        command: CreateImmutableFileArtifact,
    ) -> PortFuture<'a, Result<FileArtifactMetadata, SdkError>> {
        Box::pin(async move {
            context.validate()?;
            command.validate()?;
            if context.module_id != command.owner_module_id {
                return Err(owner_mismatch());
            }

            let expected_size_bytes = i64::try_from(command.expected_size_bytes)
                .map_err(|_| invalid_stored_value("artifact size does not fit PostgreSQL bigint"))?;
            let mut transaction = self.store.pool().begin().await.map_err(database_unavailable)?;
            bind_tenant(&mut transaction, context).await?;
            sqlx::query(
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
            .execute(&mut *transaction)
            .await
            .map_err(database_unavailable)?;

            let metadata = load_metadata_for_update(&mut transaction, context, &command.file_id)
                .await?
                .ok_or_else(artifact_not_found)?;
            if metadata.owner_module_id != command.owner_module_id
                || metadata.media_type != command.media_type
                || metadata.data_class != command.data_class
                || metadata.retention_policy_id != command.retention_policy_id
                || metadata.expected_size_bytes != command.expected_size_bytes
                || metadata.expected_sha256 != command.expected_sha256
            {
                return Err(artifact_conflict(
                    "FILE_ARTIFACT_CREATE_CONFLICT",
                    "The file artifact ID is already bound to different immutable metadata.",
                ));
            }
            transaction.commit().await.map_err(database_unavailable)?;
            Ok(metadata)
        })
    }

    fn append_chunk<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        command: AppendImmutableFileChunk,
    ) -> PortFuture<'a, Result<FileArtifactAppendResult, SdkError>> {
        Box::pin(async move {
            context.validate()?;
            command.validate()?;
            let computed_hash: [u8; 32] = Sha256::digest(&command.bytes).into();
            if computed_hash != command.chunk_sha256 {
                return Err(SdkError::invalid_argument(
                    "file_artifact.chunk.sha256",
                    "File artifact chunk SHA-256 does not match the supplied bytes",
                ));
            }
            let chunk_index = i64::try_from(command.chunk_index)
                .map_err(|_| SdkError::invalid_argument("file_artifact.chunk_index", "Chunk index is too large"))?;
            let chunk_size = i32::try_from(command.bytes.len())
                .map_err(|_| SdkError::invalid_argument("file_artifact.chunk.bytes", "Chunk is too large"))?;

            let mut transaction = self.store.pool().begin().await.map_err(database_unavailable)?;
            bind_tenant(&mut transaction, context).await?;
            let metadata = load_metadata_for_update(&mut transaction, context, &command.file_id)
                .await?
                .ok_or_else(artifact_not_found)?;
            require_owner(&metadata, context)?;
            if metadata.status == FileArtifactStatus::Finalized {
                return Err(artifact_conflict(
                    "FILE_ARTIFACT_ALREADY_FINALIZED",
                    "A finalized file artifact is immutable.",
                ));
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
                .fetch_optional(&mut *transaction)
                .await
                .map_err(database_unavailable)?
                .ok_or_else(|| invalid_stored_value("previously accepted artifact chunk is missing"))?;
                let stored_hash: Vec<u8> = existing
                    .try_get("chunk_sha256")
                    .map_err(|error| invalid_stored_value(error.to_string()))?;
                let stored_bytes: Vec<u8> = existing
                    .try_get("chunk_bytes")
                    .map_err(|error| invalid_stored_value(error.to_string()))?;
                if stored_hash.as_slice() != command.chunk_sha256 || stored_bytes != command.bytes {
                    return Err(artifact_conflict(
                        "FILE_ARTIFACT_CHUNK_REPLAY_CONFLICT",
                        "The chunk index was already used for different bytes.",
                    ));
                }
                transaction.commit().await.map_err(database_unavailable)?;
                return Ok(FileArtifactAppendResult {
                    metadata,
                    replayed: true,
                });
            }
            if command.chunk_index != metadata.next_chunk_index {
                return Err(artifact_conflict(
                    "FILE_ARTIFACT_CHUNK_OUT_OF_ORDER",
                    "File artifact chunks must be appended in exact sequential order.",
                ));
            }

            let new_received = metadata
                .received_size_bytes
                .checked_add(command.bytes.len() as u64)
                .ok_or_else(|| artifact_conflict(
                    "FILE_ARTIFACT_SIZE_OVERFLOW",
                    "File artifact received size cannot advance further.",
                ))?;
            if new_received > metadata.expected_size_bytes {
                return Err(artifact_conflict(
                    "FILE_ARTIFACT_SIZE_EXCEEDED",
                    "File artifact chunks exceed the declared immutable size.",
                ));
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
            .execute(&mut *transaction)
            .await
            .map_err(database_unavailable)?;

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
            .bind(i64::try_from(new_received).map_err(|_| invalid_stored_value("artifact size does not fit PostgreSQL bigint"))?)
            .execute(&mut *transaction)
            .await
            .map_err(database_unavailable)?;

            let updated = load_metadata_for_update(&mut transaction, context, &command.file_id)
                .await?
                .ok_or_else(artifact_not_found)?;
            transaction.commit().await.map_err(database_unavailable)?;
            Ok(FileArtifactAppendResult {
                metadata: updated,
                replayed: false,
            })
        })
    }

    fn finalize<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        file_id: &'a FileId,
    ) -> PortFuture<'a, Result<FileArtifactMetadata, SdkError>> {
        Box::pin(async move {
            context.validate()?;
            let mut transaction = self.store.pool().begin().await.map_err(database_unavailable)?;
            bind_tenant(&mut transaction, context).await?;
            let metadata = load_metadata_for_update(&mut transaction, context, file_id)
                .await?
                .ok_or_else(artifact_not_found)?;
            require_owner(&metadata, context)?;
            if metadata.status == FileArtifactStatus::Finalized {
                transaction.commit().await.map_err(database_unavailable)?;
                return Ok(metadata);
            }
            if metadata.received_size_bytes != metadata.expected_size_bytes {
                return Err(artifact_conflict(
                    "FILE_ARTIFACT_UPLOAD_INCOMPLETE",
                    "The file artifact cannot be finalized before all declared bytes are uploaded.",
                ));
            }
            let bytes = load_and_verify_chunks(&mut transaction, context, &metadata).await?;
            if bytes.len() as u64 != metadata.expected_size_bytes {
                return Err(invalid_stored_value("artifact chunk bytes do not match declared size"));
            }

            sqlx::query(
                r#"
                UPDATE crm.file_artifacts
                   SET status = 'finalized', finalized_at = clock_timestamp()
                 WHERE tenant_id = $1 AND file_id = $2 AND status = 'uploading'
                "#,
            )
            .bind(context.execution.tenant_id.as_str())
            .bind(file_id.as_str())
            .execute(&mut *transaction)
            .await
            .map_err(database_unavailable)?;
            let finalized = load_metadata_for_update(&mut transaction, context, file_id)
                .await?
                .ok_or_else(artifact_not_found)?;
            transaction.commit().await.map_err(database_unavailable)?;
            Ok(finalized)
        })
    }

    fn read_finalized<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        file_id: &'a FileId,
    ) -> PortFuture<'a, Result<FinalizedFileArtifact, SdkError>> {
        Box::pin(async move {
            context.validate()?;
            let mut transaction = self.store.pool().begin().await.map_err(database_unavailable)?;
            sqlx::query("SET TRANSACTION READ ONLY")
                .execute(&mut *transaction)
                .await
                .map_err(database_unavailable)?;
            bind_tenant(&mut transaction, context).await?;
            let metadata = load_metadata(&mut transaction, context, file_id)
                .await?
                .ok_or_else(artifact_not_found)?;
            require_owner(&metadata, context)?;
            if metadata.status != FileArtifactStatus::Finalized {
                return Err(artifact_conflict(
                    "FILE_ARTIFACT_NOT_FINALIZED",
                    "File artifact bytes are unavailable until exact finalization succeeds.",
                ));
            }
            let bytes = load_and_verify_chunks(&mut transaction, context, &metadata).await?;
            let artifact = FinalizedFileArtifact { metadata, bytes };
            artifact.validate()?;
            transaction.commit().await.map_err(database_unavailable)?;
            Ok(artifact)
        })
    }
}

async fn bind_tenant(
    transaction: &mut Transaction<'_, Postgres>,
    context: &ModuleExecutionContext,
) -> Result<(), SdkError> {
    sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
        .bind(context.execution.tenant_id.as_str())
        .execute(&mut **transaction)
        .await
        .map_err(database_unavailable)?;
    Ok(())
}

async fn load_metadata_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    context: &ModuleExecutionContext,
    file_id: &FileId,
) -> Result<Option<FileArtifactMetadata>, SdkError> {
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
    .await
    .map_err(database_unavailable)?;
    row.map(decode_metadata).transpose()
}

async fn load_metadata(
    transaction: &mut Transaction<'_, Postgres>,
    context: &ModuleExecutionContext,
    file_id: &FileId,
) -> Result<Option<FileArtifactMetadata>, SdkError> {
    let row = sqlx::query(
        r#"
        SELECT file_id, owner_module_id, media_type, data_class, retention_policy_id,
               expected_size_bytes, expected_sha256, status, next_chunk_index, received_size_bytes
          FROM crm.file_artifacts
         WHERE tenant_id = $1 AND file_id = $2
        "#,
    )
    .bind(context.execution.tenant_id.as_str())
    .bind(file_id.as_str())
    .fetch_optional(&mut **transaction)
    .await
    .map_err(database_unavailable)?;
    row.map(decode_metadata).transpose()
}

async fn load_and_verify_chunks(
    transaction: &mut Transaction<'_, Postgres>,
    context: &ModuleExecutionContext,
    metadata: &FileArtifactMetadata,
) -> Result<Vec<u8>, SdkError> {
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
    .await
    .map_err(database_unavailable)?;

    if rows.len() as u64 != metadata.next_chunk_index {
        return Err(invalid_stored_value("artifact chunk count does not match next chunk index"));
    }
    let capacity = usize::try_from(metadata.expected_size_bytes)
        .map_err(|_| invalid_stored_value("artifact size does not fit memory address space"))?;
    let mut bytes = Vec::with_capacity(capacity);
    for (expected_index, row) in rows.into_iter().enumerate() {
        let chunk_index: i64 = row
            .try_get("chunk_index")
            .map_err(|error| invalid_stored_value(error.to_string()))?;
        if chunk_index != expected_index as i64 {
            return Err(invalid_stored_value("artifact chunks are not contiguous"));
        }
        let chunk_size: i32 = row
            .try_get("chunk_size_bytes")
            .map_err(|error| invalid_stored_value(error.to_string()))?;
        let chunk_hash: Vec<u8> = row
            .try_get("chunk_sha256")
            .map_err(|error| invalid_stored_value(error.to_string()))?;
        let chunk_bytes: Vec<u8> = row
            .try_get("chunk_bytes")
            .map_err(|error| invalid_stored_value(error.to_string()))?;
        if usize::try_from(chunk_size).ok() != Some(chunk_bytes.len())
            || chunk_hash.len() != 32
            || Sha256::digest(&chunk_bytes).as_slice() != chunk_hash.as_slice()
        {
            return Err(invalid_stored_value("artifact chunk integrity validation failed"));
        }
        bytes.extend_from_slice(&chunk_bytes);
    }
    let digest: [u8; 32] = Sha256::digest(&bytes).into();
    if bytes.len() as u64 != metadata.expected_size_bytes || digest != metadata.expected_sha256 {
        return Err(artifact_conflict(
            "FILE_ARTIFACT_DIGEST_MISMATCH",
            "File artifact bytes do not match the declared immutable size and SHA-256.",
        ));
    }
    Ok(bytes)
}

fn decode_metadata(row: sqlx::postgres::PgRow) -> Result<FileArtifactMetadata, SdkError> {
    let expected_sha256: Vec<u8> = row
        .try_get("expected_sha256")
        .map_err(|error| invalid_stored_value(error.to_string()))?;
    let expected_sha256: [u8; 32] = expected_sha256
        .try_into()
        .map_err(|_| invalid_stored_value("artifact SHA-256 must contain exactly 32 bytes"))?;
    let expected_size_bytes = u64::try_from(
        row.try_get::<i64, _>("expected_size_bytes")
            .map_err(|error| invalid_stored_value(error.to_string()))?,
    )
    .map_err(|_| invalid_stored_value("artifact expected size must be non-negative"))?;
    let next_chunk_index = u64::try_from(
        row.try_get::<i64, _>("next_chunk_index")
            .map_err(|error| invalid_stored_value(error.to_string()))?,
    )
    .map_err(|_| invalid_stored_value("artifact next chunk index must be non-negative"))?;
    let received_size_bytes = u64::try_from(
        row.try_get::<i64, _>("received_size_bytes")
            .map_err(|error| invalid_stored_value(error.to_string()))?,
    )
    .map_err(|_| invalid_stored_value("artifact received size must be non-negative"))?;
    let metadata = FileArtifactMetadata {
        file_id: FileId::try_new(
            row.try_get::<String, _>("file_id")
                .map_err(|error| invalid_stored_value(error.to_string()))?,
        )
        .map_err(|error| invalid_stored_value(error.to_string()))?,
        owner_module_id: ModuleId::try_new(
            row.try_get::<String, _>("owner_module_id")
                .map_err(|error| invalid_stored_value(error.to_string()))?,
        )
        .map_err(|error| invalid_stored_value(error.to_string()))?,
        media_type: row
            .try_get("media_type")
            .map_err(|error| invalid_stored_value(error.to_string()))?,
        data_class: data_class_from_db(
            row.try_get::<String, _>("data_class")
                .map_err(|error| invalid_stored_value(error.to_string()))?
                .as_str(),
        )?,
        retention_policy_id: RetentionPolicyId::try_new(
            row.try_get::<String, _>("retention_policy_id")
                .map_err(|error| invalid_stored_value(error.to_string()))?,
        )
        .map_err(|error| invalid_stored_value(error.to_string()))?,
        expected_size_bytes,
        expected_sha256,
        status: match row
            .try_get::<String, _>("status")
            .map_err(|error| invalid_stored_value(error.to_string()))?
            .as_str()
        {
            "uploading" => FileArtifactStatus::Uploading,
            "finalized" => FileArtifactStatus::Finalized,
            _ => return Err(invalid_stored_value("artifact status is invalid")),
        },
        next_chunk_index,
        received_size_bytes,
    };
    metadata.validate()?;
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

fn data_class_from_db(value: &str) -> Result<DataClass, SdkError> {
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
        _ => Err(invalid_stored_value("artifact data class is invalid")),
    }
}

fn artifact_not_found() -> SdkError {
    file_error(
        "FILE_ARTIFACT_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The requested file artifact was not found.",
    )
}

fn owner_mismatch() -> SdkError {
    file_error(
        "FILE_ARTIFACT_OWNER_MISMATCH",
        ErrorCategory::Authorization,
        false,
        "The file artifact is not owned by the executing module.",
    )
}

fn artifact_conflict(code: &'static str, safe_message: &'static str) -> SdkError {
    file_error(code, ErrorCategory::Conflict, false, safe_message)
}

fn database_unavailable(error: sqlx::Error) -> SdkError {
    file_error(
        "FILE_ARTIFACT_DATABASE_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "File artifact storage is temporarily unavailable.",
    )
    .with_internal_reference(error.to_string())
}

fn invalid_stored_value(message: impl Into<String>) -> SdkError {
    file_error(
        "FILE_ARTIFACT_STORED_VALUE_INVALID",
        ErrorCategory::Unavailable,
        true,
        "Stored file artifact state is temporarily unavailable.",
    )
    .with_internal_reference(message.into())
}

fn file_error(
    code: &'static str,
    category: ErrorCategory,
    retryable: bool,
    safe_message: &'static str,
) -> SdkError {
    SdkError::new(code, category, retryable, safe_message)
}
