use crate::{
    ExportJobId, PartyExportExclusionReason, PartyExportExecutionOutcome,
    PartyExportExecutionOutcomeKind,
};
use crm_module_sdk::{ErrorCategory, SdkError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const EXPORT_EXECUTION_OUTCOME_STATE_SCHEMA_ID: &str =
    "crm.customer-data-operations.export_execution_outcome.state";
pub const EXPORT_EXECUTION_OUTCOME_STATE_SCHEMA_VERSION: &str = "1.0.0";
pub const EXPORT_EXECUTION_OUTCOME_STATE_MAXIMUM_BYTES: u64 = 16 * 1024;
pub const EXPORT_EXECUTION_OUTCOME_STATE_RETENTION_POLICY_ID: &str =
    "crm.customer_data.export_execution_outcome";

const EXPORT_EXECUTION_OUTCOME_STATE_DESCRIPTOR: &[u8] = b"crm.customer-data-operations.export_execution_outcome.state/v1:outcome_id,export_job_id,manifest_position,outcome_kind,artifact_chunk_index,chunk_sha256,chunk_size_bytes,redacted_fields,occurred_at_unix_nanos,version";
const PERSISTED_STATE_VERSION: u32 = 1;

pub fn export_execution_outcome_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(EXPORT_EXECUTION_OUTCOME_STATE_DESCRIPTOR).into()
}

pub fn encode_export_execution_outcome_state(
    outcome: &PartyExportExecutionOutcome,
) -> Result<Vec<u8>, SdkError> {
    let bytes = serde_json::to_vec(&ExportExecutionOutcomeStateV1::from(outcome)).map_err(|error| {
        persisted_error(format!(
            "export execution outcome serialization failed: {error}"
        ))
    })?;
    validate_size(&bytes)?;
    Ok(bytes)
}

pub fn decode_export_execution_outcome_state(
    bytes: &[u8],
) -> Result<PartyExportExecutionOutcome, SdkError> {
    validate_size(bytes)?;
    let state: ExportExecutionOutcomeStateV1 = serde_json::from_slice(bytes).map_err(|error| {
        persisted_error(format!("export execution outcome JSON is invalid: {error}"))
    })?;
    if state.version != PERSISTED_STATE_VERSION {
        return Err(persisted_error(
            "export execution outcome state version is unsupported".to_owned(),
        ));
    }

    let expected_outcome_id = state.outcome_id.clone();
    let outcome_kind = state.outcome_kind.clone();
    let job_id = ExportJobId::try_new(state.export_job_id.clone())
        .map_err(|error| persisted_domain_error("export job ID", error))?;

    let outcome = match outcome_kind.as_str() {
        "emitted" => {
            let artifact_chunk_index = state.artifact_chunk_index.ok_or_else(|| {
                persisted_error("emitted outcome is missing artifact chunk index".to_owned())
            })?;
            let chunk_sha256 = state.chunk_sha256.clone().ok_or_else(|| {
                persisted_error("emitted outcome is missing chunk SHA-256".to_owned())
            })?;
            let chunk_size_bytes = state.chunk_size_bytes.ok_or_else(|| {
                persisted_error("emitted outcome is missing chunk byte size".to_owned())
            })?;
            PartyExportExecutionOutcome::emitted(
                job_id,
                state.manifest_position,
                artifact_chunk_index,
                chunk_sha256,
                chunk_size_bytes,
                state.redacted_fields,
                state.occurred_at_unix_nanos,
            )
        }
        "excluded_not_visible" => {
            decode_excluded(state, job_id, PartyExportExclusionReason::NotVisible)
        }
        "excluded_version_changed" => {
            decode_excluded(state, job_id, PartyExportExclusionReason::VersionChanged)
        }
        "excluded_unavailable" => {
            decode_excluded(state, job_id, PartyExportExclusionReason::Unavailable)
        }
        _ => {
            return Err(persisted_error(
                "export execution outcome kind is unsupported".to_owned(),
            ));
        }
    }
    .map_err(|error| persisted_domain_error("export execution outcome", error))?;

    if outcome.outcome_id().as_str() != expected_outcome_id {
        return Err(persisted_error(
            "export execution outcome identity is inconsistent".to_owned(),
        ));
    }

    let canonical = encode_export_execution_outcome_state(&outcome)?;
    if canonical != bytes {
        return Err(persisted_error(
            "export execution outcome state is not the strict canonical v1 encoding".to_owned(),
        ));
    }

    Ok(outcome)
}

fn decode_excluded(
    state: ExportExecutionOutcomeStateV1,
    job_id: ExportJobId,
    reason: PartyExportExclusionReason,
) -> Result<PartyExportExecutionOutcome, SdkError> {
    if state.artifact_chunk_index.is_some()
        || state.chunk_sha256.is_some()
        || state.chunk_size_bytes.is_some()
        || state.redacted_fields != 0
    {
        return Err(persisted_error(
            "excluded outcome contains emitted-row fields".to_owned(),
        ));
    }
    PartyExportExecutionOutcome::excluded(
        job_id,
        state.manifest_position,
        reason,
        state.occurred_at_unix_nanos,
    )
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportExecutionOutcomeStateV1 {
    outcome_id: String,
    export_job_id: String,
    manifest_position: u32,
    outcome_kind: String,
    artifact_chunk_index: Option<u32>,
    chunk_sha256: Option<String>,
    chunk_size_bytes: Option<u64>,
    redacted_fields: u32,
    occurred_at_unix_nanos: i64,
    version: u32,
}

impl From<&PartyExportExecutionOutcome> for ExportExecutionOutcomeStateV1 {
    fn from(outcome: &PartyExportExecutionOutcome) -> Self {
        let (outcome_kind, artifact_chunk_index, chunk_sha256, chunk_size_bytes) =
            match outcome.kind() {
                PartyExportExecutionOutcomeKind::Emitted {
                    artifact_chunk_index,
                    chunk_sha256,
                    chunk_size_bytes,
                } => (
                    "emitted".to_owned(),
                    Some(*artifact_chunk_index),
                    Some(chunk_sha256.clone()),
                    Some(*chunk_size_bytes),
                ),
                PartyExportExecutionOutcomeKind::Excluded(
                    PartyExportExclusionReason::NotVisible,
                ) => ("excluded_not_visible".to_owned(), None, None, None),
                PartyExportExecutionOutcomeKind::Excluded(
                    PartyExportExclusionReason::VersionChanged,
                ) => ("excluded_version_changed".to_owned(), None, None, None),
                PartyExportExecutionOutcomeKind::Excluded(
                    PartyExportExclusionReason::Unavailable,
                ) => ("excluded_unavailable".to_owned(), None, None, None),
            };
        Self {
            outcome_id: outcome.outcome_id().as_str().to_owned(),
            export_job_id: outcome.job_id().as_str().to_owned(),
            manifest_position: outcome.manifest_position(),
            outcome_kind,
            artifact_chunk_index,
            chunk_sha256,
            chunk_size_bytes,
            redacted_fields: outcome.redacted_fields(),
            occurred_at_unix_nanos: outcome.occurred_at_unix_nanos(),
            version: PERSISTED_STATE_VERSION,
        }
    }
}

fn validate_size(bytes: &[u8]) -> Result<(), SdkError> {
    if bytes.len() as u64 > EXPORT_EXECUTION_OUTCOME_STATE_MAXIMUM_BYTES {
        return Err(persisted_error(format!(
            "export execution outcome state exceeds {EXPORT_EXECUTION_OUTCOME_STATE_MAXIMUM_BYTES} bytes"
        )));
    }
    Ok(())
}

fn persisted_domain_error(context: &str, error: SdkError) -> SdkError {
    persisted_error(format!("{context}: {}: {}", error.code, error.safe_message))
}

fn persisted_error(detail: String) -> SdkError {
    let _ = detail;
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_EXECUTION_OUTCOME_PERSISTED_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "Stored customer export execution outcome state is invalid.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn emitted() -> PartyExportExecutionOutcome {
        PartyExportExecutionOutcome::emitted(
            ExportJobId::try_new("export-outcome-persistence-job").unwrap(),
            3,
            2,
            "11".repeat(32),
            42,
            1,
            100,
        )
        .unwrap()
    }

    fn excluded() -> PartyExportExecutionOutcome {
        PartyExportExecutionOutcome::excluded(
            ExportJobId::try_new("export-outcome-persistence-job").unwrap(),
            4,
            PartyExportExclusionReason::VersionChanged,
            101,
        )
        .unwrap()
    }

    #[test]
    fn round_trips_canonical_emitted_and_excluded_outcomes() {
        for outcome in [emitted(), excluded()] {
            let encoded = encode_export_execution_outcome_state(&outcome).unwrap();
            let decoded = decode_export_execution_outcome_state(&encoded).unwrap();
            assert_eq!(decoded, outcome);
            assert_eq!(
                encode_export_execution_outcome_state(&decoded).unwrap(),
                encoded
            );
        }
    }

    #[test]
    fn rejects_unknown_fields_tampered_identity_and_noncanonical_exclusion_fields() {
        let bytes = encode_export_execution_outcome_state(&emitted()).unwrap();
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["unknown"] = serde_json::json!(true);
        assert!(
            decode_export_execution_outcome_state(&serde_json::to_vec(&value).unwrap()).is_err()
        );

        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["outcome_id"] = serde_json::json!("cdo-export-outcome-tampered");
        assert!(
            decode_export_execution_outcome_state(&serde_json::to_vec(&value).unwrap()).is_err()
        );

        let bytes = encode_export_execution_outcome_state(&excluded()).unwrap();
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["chunk_size_bytes"] = serde_json::json!(1);
        assert!(
            decode_export_execution_outcome_state(&serde_json::to_vec(&value).unwrap()).is_err()
        );
    }

    #[test]
    fn descriptor_hash_is_stable_and_non_zero() {
        assert_ne!(export_execution_outcome_state_descriptor_hash(), [0; 32]);
    }
}
