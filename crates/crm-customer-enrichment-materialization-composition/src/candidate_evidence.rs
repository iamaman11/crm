use crm_core_files::{FinalizedFileArtifact, ImmutableFileArtifactStore};
use crm_customer_enrichment_capability_adapter::MODULE_ID;
use crm_module_sdk::{
    DataClass, ErrorCategory, FileId, ModuleExecutionContext, PortFuture, SdkError,
};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;
use prost::Message;
use std::fmt;
use std::sync::Arc;

/// Canonical media type for bounded provider suggestion candidates retained as governed evidence.
pub const PROVIDER_SUGGESTION_CANDIDATE_EVIDENCE_MEDIA_TYPE: &str =
    "application/vnd.crm.customer-enrichment.materialization+protobuf";
/// The first production slice permits at most the module-wide bounded suggestion count.
pub const MAXIMUM_PROVIDER_SUGGESTION_CANDIDATES: usize = 32;
/// Canonical candidate evidence is intentionally much smaller than the generic file-artifact limit.
pub const MAXIMUM_PROVIDER_SUGGESTION_EVIDENCE_BYTES: usize = 256 * 1024;

/// Exact governed lookup for one finalized candidate-evidence artifact.
#[derive(Debug, Clone)]
pub struct ProviderSuggestionCandidateEvidenceRequest {
    pub context: ModuleExecutionContext,
    pub file_id: FileId,
    pub expected_enrichment_request_id: String,
    pub expected_provider_response_receipt_id: String,
}

/// Finalized canonical candidate source used by the materialization process.
pub trait ProviderSuggestionCandidateEvidenceSourcePort: Send + Sync {
    fn load<'a>(
        &'a self,
        request: ProviderSuggestionCandidateEvidenceRequest,
    ) -> PortFuture<'a, Result<wire::MaterializeSuggestionsRequest, SdkError>>;
}

/// Governed immutable-file implementation. It never interprets a raw provider payload.
#[derive(Clone)]
pub struct GovernedFileProviderSuggestionCandidateEvidenceSource {
    files: Arc<dyn ImmutableFileArtifactStore>,
}

impl GovernedFileProviderSuggestionCandidateEvidenceSource {
    pub fn new(files: Arc<dyn ImmutableFileArtifactStore>) -> Self {
        Self { files }
    }
}

impl fmt::Debug for GovernedFileProviderSuggestionCandidateEvidenceSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GovernedFileProviderSuggestionCandidateEvidenceSource")
            .field("files", &"dyn ImmutableFileArtifactStore")
            .finish()
    }
}

impl ProviderSuggestionCandidateEvidenceSourcePort
    for GovernedFileProviderSuggestionCandidateEvidenceSource
{
    fn load<'a>(
        &'a self,
        request: ProviderSuggestionCandidateEvidenceRequest,
    ) -> PortFuture<'a, Result<wire::MaterializeSuggestionsRequest, SdkError>> {
        Box::pin(async move {
            let artifact = self
                .files
                .read_finalized(&request.context, &request.file_id)
                .await
                .map_err(|error| evidence_unavailable(error.code))?;
            decode_provider_suggestion_candidate_evidence(
                &artifact,
                &request.expected_enrichment_request_id,
                &request.expected_provider_response_receipt_id,
            )
        })
    }
}

/// Strictly validates finalized metadata and decodes the existing bounded internal materialization
/// contract. The artifact is canonical extracted evidence, never an arbitrary provider body.
pub fn decode_provider_suggestion_candidate_evidence(
    artifact: &FinalizedFileArtifact,
    expected_enrichment_request_id: &str,
    expected_provider_response_receipt_id: &str,
) -> Result<wire::MaterializeSuggestionsRequest, SdkError> {
    artifact
        .validate()
        .map_err(|error| evidence_invalid(error.code))?;
    if artifact.metadata.owner_module_id.as_str() != MODULE_ID
        || artifact.metadata.media_type != PROVIDER_SUGGESTION_CANDIDATE_EVIDENCE_MEDIA_TYPE
        || artifact.metadata.data_class != DataClass::Personal
        || artifact.bytes.is_empty()
        || artifact.bytes.len() > MAXIMUM_PROVIDER_SUGGESTION_EVIDENCE_BYTES
    {
        return Err(evidence_invalid("artifact metadata or size is invalid"));
    }

    let command = wire::MaterializeSuggestionsRequest::decode(artifact.bytes.as_slice())
        .map_err(|_| evidence_invalid("canonical candidate evidence could not be decoded"))?;
    validate_candidate_evidence_lineage(
        &command,
        expected_enrichment_request_id,
        expected_provider_response_receipt_id,
    )?;
    Ok(command)
}

fn validate_candidate_evidence_lineage(
    command: &wire::MaterializeSuggestionsRequest,
    expected_enrichment_request_id: &str,
    expected_provider_response_receipt_id: &str,
) -> Result<(), SdkError> {
    let request_id = command
        .enrichment_request_ref
        .as_ref()
        .map(|reference| reference.enrichment_request_id.as_str());
    let receipt_id = command
        .provider_response_receipt_ref
        .as_ref()
        .map(|reference| reference.provider_response_receipt_id.as_str());
    if expected_enrichment_request_id.is_empty()
        || expected_provider_response_receipt_id.is_empty()
        || request_id != Some(expected_enrichment_request_id)
        || receipt_id != Some(expected_provider_response_receipt_id)
        || command.candidates.len() > MAXIMUM_PROVIDER_SUGGESTION_CANDIDATES
    {
        return Err(evidence_conflict());
    }
    Ok(())
}

fn evidence_unavailable(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_SUGGESTION_EVIDENCE_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "The governed provider suggestion evidence is unavailable.",
    )
    .with_internal_reference(reference.into())
}

fn evidence_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_SUGGESTION_EVIDENCE_INVALID",
        ErrorCategory::Internal,
        false,
        "The governed provider suggestion evidence is invalid.",
    )
    .with_internal_reference(reference.into())
}

fn evidence_conflict() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_SUGGESTION_EVIDENCE_CONFLICT",
        ErrorCategory::Conflict,
        false,
        "The governed provider suggestion evidence does not match the exact response lineage.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_core_files::{FileArtifactMetadata, FileArtifactStatus};
    use crm_module_sdk::{ModuleId, RetentionPolicyId};

    fn artifact(command: &wire::MaterializeSuggestionsRequest) -> FinalizedFileArtifact {
        let bytes = command.encode_to_vec();
        FinalizedFileArtifact {
            metadata: FileArtifactMetadata {
                file_id: FileId::try_new("candidate-evidence-1").unwrap(),
                owner_module_id: ModuleId::try_new(MODULE_ID).unwrap(),
                media_type: PROVIDER_SUGGESTION_CANDIDATE_EVIDENCE_MEDIA_TYPE.to_owned(),
                data_class: DataClass::Personal,
                retention_policy_id: RetentionPolicyId::try_new("enrichment-evidence").unwrap(),
                expected_size_bytes: bytes.len() as u64,
                expected_sha256: [7; 32],
                status: FileArtifactStatus::Finalized,
                next_chunk_index: 1,
                received_size_bytes: bytes.len() as u64,
            },
            bytes,
        }
    }

    fn command() -> wire::MaterializeSuggestionsRequest {
        wire::MaterializeSuggestionsRequest {
            enrichment_request_ref: Some(wire::EnrichmentRequestRef {
                enrichment_request_id: "request-1".to_owned(),
            }),
            provider_response_receipt_ref: Some(wire::ProviderResponseReceiptRef {
                provider_response_receipt_id: "receipt-1".to_owned(),
            }),
            candidates: Vec::new(),
        }
    }

    #[test]
    fn exact_finalized_candidate_evidence_decodes() {
        let command = command();
        let decoded = decode_provider_suggestion_candidate_evidence(
            &artifact(&command),
            "request-1",
            "receipt-1",
        )
        .unwrap();
        assert_eq!(decoded, command);
    }

    #[test]
    fn request_or_receipt_lineage_mismatch_fails_closed() {
        let command = command();
        let error = decode_provider_suggestion_candidate_evidence(
            &artifact(&command),
            "request-other",
            "receipt-1",
        )
        .unwrap_err();
        assert_eq!(
            error.code,
            "CUSTOMER_ENRICHMENT_SUGGESTION_EVIDENCE_CONFLICT"
        );
    }

    #[test]
    fn raw_or_wrongly_classified_artifact_is_rejected() {
        let mut artifact = artifact(&command());
        artifact.metadata.media_type = "application/json".to_owned();
        let error = decode_provider_suggestion_candidate_evidence(
            &artifact,
            "request-1",
            "receipt-1",
        )
        .unwrap_err();
        assert_eq!(
            error.code,
            "CUSTOMER_ENRICHMENT_SUGGESTION_EVIDENCE_INVALID"
        );
    }

    #[test]
    fn candidate_count_is_bounded_before_planner_execution() {
        let mut command = command();
        command.candidates = vec![wire::ProviderSuggestionCandidate::default(); 33];
        let error = decode_provider_suggestion_candidate_evidence(
            &artifact(&command),
            "request-1",
            "receipt-1",
        )
        .unwrap_err();
        assert_eq!(
            error.code,
            "CUSTOMER_ENRICHMENT_SUGGESTION_EVIDENCE_CONFLICT"
        );
    }
}
