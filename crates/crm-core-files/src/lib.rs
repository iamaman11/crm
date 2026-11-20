#![forbid(unsafe_code)]

//! Governed, tenant-aware immutable file artifact contracts.
//!
//! Business modules receive this typed port rather than raw PostgreSQL or object-storage clients.
//! Uploads are bounded and chunked. Bytes become readable only after exact size and SHA-256
//! finalization, after which the artifact is immutable.

use crm_module_sdk::{
    DataClass, ErrorCategory, FileId, ModuleExecutionContext, ModuleId, PortFuture,
    RetentionPolicyId, SdkError,
};

pub const MAXIMUM_FILE_ARTIFACT_BYTES: u64 = 64 * 1024 * 1024;
pub const MAXIMUM_FILE_ARTIFACT_CHUNK_BYTES: usize = 512 * 1024;
pub const MAXIMUM_MEDIA_TYPE_BYTES: usize = 255;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileArtifactStatus {
    Uploading,
    Finalized,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileArtifactMetadata {
    pub file_id: FileId,
    pub owner_module_id: ModuleId,
    pub media_type: String,
    pub data_class: DataClass,
    pub retention_policy_id: RetentionPolicyId,
    pub expected_size_bytes: u64,
    pub expected_sha256: [u8; 32],
    pub status: FileArtifactStatus,
    pub next_chunk_index: u64,
    pub received_size_bytes: u64,
}

impl FileArtifactMetadata {
    pub fn validate(&self) -> Result<(), SdkError> {
        validate_media_type(&self.media_type)?;
        validate_expected_size_and_hash(self.expected_size_bytes, &self.expected_sha256)?;
        if self.received_size_bytes > self.expected_size_bytes {
            return Err(file_error(
                "FILE_ARTIFACT_RECEIVED_SIZE_INVALID",
                ErrorCategory::Internal,
                false,
                "Stored file artifact metadata is inconsistent.",
            ));
        }
        if self.status == FileArtifactStatus::Finalized
            && self.received_size_bytes != self.expected_size_bytes
        {
            return Err(file_error(
                "FILE_ARTIFACT_FINALIZED_SIZE_INVALID",
                ErrorCategory::Internal,
                false,
                "Stored file artifact metadata is inconsistent.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateImmutableFileArtifact {
    pub file_id: FileId,
    pub owner_module_id: ModuleId,
    pub media_type: String,
    pub data_class: DataClass,
    pub retention_policy_id: RetentionPolicyId,
    pub expected_size_bytes: u64,
    pub expected_sha256: [u8; 32],
}

impl CreateImmutableFileArtifact {
    pub fn validate(&self) -> Result<(), SdkError> {
        validate_media_type(&self.media_type)?;
        validate_expected_size_and_hash(self.expected_size_bytes, &self.expected_sha256)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppendImmutableFileChunk {
    pub file_id: FileId,
    pub chunk_index: u64,
    pub chunk_sha256: [u8; 32],
    pub bytes: Vec<u8>,
}

impl AppendImmutableFileChunk {
    pub fn validate(&self) -> Result<(), SdkError> {
        if self.bytes.is_empty() || self.bytes.len() > MAXIMUM_FILE_ARTIFACT_CHUNK_BYTES {
            return Err(SdkError::invalid_argument(
                "file_artifact.chunk.bytes",
                format!(
                    "File artifact chunk must contain between 1 and {MAXIMUM_FILE_ARTIFACT_CHUNK_BYTES} bytes"
                ),
            ));
        }
        if self.chunk_sha256.iter().all(|byte| *byte == 0) {
            return Err(SdkError::invalid_argument(
                "file_artifact.chunk.sha256",
                "File artifact chunk SHA-256 must not be all zeroes",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileArtifactAppendResult {
    pub metadata: FileArtifactMetadata,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalizedFileArtifact {
    pub metadata: FileArtifactMetadata,
    pub bytes: Vec<u8>,
}

impl FinalizedFileArtifact {
    pub fn validate(&self) -> Result<(), SdkError> {
        self.metadata.validate()?;
        if self.metadata.status != FileArtifactStatus::Finalized
            || self.bytes.len() as u64 != self.metadata.expected_size_bytes
        {
            return Err(file_error(
                "FILE_ARTIFACT_FINALIZED_BYTES_INVALID",
                ErrorCategory::Internal,
                false,
                "Stored finalized file artifact bytes are inconsistent.",
            ));
        }
        Ok(())
    }
}

pub trait ImmutableFileArtifactStore: Send + Sync {
    fn create<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        command: CreateImmutableFileArtifact,
    ) -> PortFuture<'a, Result<FileArtifactMetadata, SdkError>>;

    fn append_chunk<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        command: AppendImmutableFileChunk,
    ) -> PortFuture<'a, Result<FileArtifactAppendResult, SdkError>>;

    fn finalize<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        file_id: &'a FileId,
    ) -> PortFuture<'a, Result<FileArtifactMetadata, SdkError>>;

    fn read_finalized<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        file_id: &'a FileId,
    ) -> PortFuture<'a, Result<FinalizedFileArtifact, SdkError>>;
}

fn validate_media_type(value: &str) -> Result<(), SdkError> {
    if value.is_empty()
        || value.len() > MAXIMUM_MEDIA_TYPE_BYTES
        || value.chars().any(char::is_control)
        || !value.contains('/')
    {
        return Err(SdkError::invalid_argument(
            "file_artifact.media_type",
            "File artifact media type is invalid",
        ));
    }
    Ok(())
}

fn validate_expected_size_and_hash(size: u64, sha256: &[u8; 32]) -> Result<(), SdkError> {
    if size > MAXIMUM_FILE_ARTIFACT_BYTES {
        return Err(SdkError::invalid_argument(
            "file_artifact.expected_size_bytes",
            format!("File artifact must not exceed {MAXIMUM_FILE_ARTIFACT_BYTES} bytes"),
        ));
    }
    if sha256.iter().all(|byte| *byte == 0) {
        return Err(SdkError::invalid_argument(
            "file_artifact.expected_sha256",
            "File artifact SHA-256 must not be all zeroes",
        ));
    }
    Ok(())
}

fn file_error(
    code: &'static str,
    category: ErrorCategory,
    retryable: bool,
    safe_message: &'static str,
) -> SdkError {
    SdkError::new(code, category, retryable, safe_message)
}

/// Architecture marker for `crm-core-files`.
pub const CRATE_NAME: &str = "crm-core-files";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_oversized_artifacts_and_chunks() {
        let artifact_error =
            validate_expected_size_and_hash(MAXIMUM_FILE_ARTIFACT_BYTES + 1, &[1; 32]).unwrap_err();
        assert_eq!(artifact_error.code, "SDK_INVALID_ARGUMENT");

        let chunk = AppendImmutableFileChunk {
            file_id: FileId::try_new("file-1").unwrap(),
            chunk_index: 0,
            chunk_sha256: [1; 32],
            bytes: vec![0; MAXIMUM_FILE_ARTIFACT_CHUNK_BYTES + 1],
        };
        assert!(chunk.validate().is_err());

        let empty = AppendImmutableFileChunk {
            file_id: FileId::try_new("file-2").unwrap(),
            chunk_index: 0,
            chunk_sha256: [1; 32],
            bytes: Vec::new(),
        };
        assert!(empty.validate().is_err());
    }
}
