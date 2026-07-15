use crate::{ExportJobId, PartyExportSelectionProgress, PartyExportSourceContinuation};
use crm_module_sdk::{ErrorCategory, RecordId, SdkError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const EXPORT_SELECTION_PROGRESS_STATE_SCHEMA_ID: &str =
    "crm.customer-data-operations.export_selection_progress.state";
pub const EXPORT_SELECTION_PROGRESS_STATE_SCHEMA_VERSION: &str = "1.0.0";
pub const EXPORT_SELECTION_PROGRESS_STATE_MAXIMUM_BYTES: u64 = 16 * 1024;
pub const EXPORT_SELECTION_PROGRESS_STATE_RETENTION_POLICY_ID: &str =
    "crm.customer_data.export_selection_progress";

const EXPORT_SELECTION_PROGRESS_STATE_DESCRIPTOR: &[u8] = b"crm.customer-data-operations.export_selection_progress.state/v1:progress_id,export_job_id,next_manifest_position,source_after_sort_value,source_after_record_id,source_exhausted,created_at_unix_nanos,updated_at_unix_nanos,version";

pub fn export_selection_progress_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(EXPORT_SELECTION_PROGRESS_STATE_DESCRIPTOR).into()
}

pub fn encode_export_selection_progress_state(
    progress: &PartyExportSelectionProgress,
) -> Result<Vec<u8>, SdkError> {
    let bytes = serde_json::to_vec(&ExportSelectionProgressStateV1::from(progress)).map_err(
        |error| {
            persisted_error(format!(
                "export selection progress serialization failed: {error}"
            ))
        },
    )?;
    validate_size(&bytes)?;
    Ok(bytes)
}

pub fn decode_export_selection_progress_state(
    bytes: &[u8],
) -> Result<PartyExportSelectionProgress, SdkError> {
    validate_size(bytes)?;
    let state: ExportSelectionProgressStateV1 = serde_json::from_slice(bytes).map_err(|error| {
        persisted_error(format!("export selection progress JSON is invalid: {error}"))
    })?;
    let expected_progress_id = state.progress_id.clone();
    let job_id = ExportJobId::try_new(state.export_job_id)
        .map_err(|error| persisted_domain_error("export job ID", error))?;
    let continuation = match (state.source_after_sort_value, state.source_after_record_id) {
        (Some(sort_value), Some(record_id)) => Some(
            PartyExportSourceContinuation::try_new(
                sort_value,
                RecordId::try_new(record_id)
                    .map_err(|error| persisted_domain_error("source continuation ID", error))?,
            )
            .map_err(|error| persisted_domain_error("source continuation", error))?,
        ),
        (None, None) => None,
        _ => {
            return Err(persisted_error(
                "export selection progress continuation is incomplete".to_owned(),
            ));
        }
    };

    let progress = PartyExportSelectionProgress::rehydrate(
        job_id,
        state.next_manifest_position,
        continuation,
        state.source_exhausted,
        state.created_at_unix_nanos,
        state.updated_at_unix_nanos,
        state.version,
    )
    .map_err(|error| persisted_domain_error("export selection progress", error))?;
    if progress.progress_id().as_str() != expected_progress_id {
        return Err(persisted_error(
            "export selection progress deterministic identity is inconsistent".to_owned(),
        ));
    }

    let canonical = encode_export_selection_progress_state(&progress)?;
    if canonical != bytes {
        return Err(persisted_error(
            "export selection progress state is not the strict canonical v1 encoding".to_owned(),
        ));
    }
    Ok(progress)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportSelectionProgressStateV1 {
    progress_id: String,
    export_job_id: String,
    next_manifest_position: u32,
    source_after_sort_value: Option<String>,
    source_after_record_id: Option<String>,
    source_exhausted: bool,
    created_at_unix_nanos: i64,
    updated_at_unix_nanos: i64,
    version: i64,
}

impl From<&PartyExportSelectionProgress> for ExportSelectionProgressStateV1 {
    fn from(progress: &PartyExportSelectionProgress) -> Self {
        let (source_after_sort_value, source_after_record_id) = progress
            .continuation()
            .map(|continuation| {
                (
                    Some(continuation.sort_value().to_owned()),
                    Some(continuation.record_id().as_str().to_owned()),
                )
            })
            .unwrap_or((None, None));
        Self {
            progress_id: progress.progress_id().as_str().to_owned(),
            export_job_id: progress.job_id().as_str().to_owned(),
            next_manifest_position: progress.next_manifest_position(),
            source_after_sort_value,
            source_after_record_id,
            source_exhausted: progress.source_exhausted(),
            created_at_unix_nanos: progress.created_at_unix_nanos(),
            updated_at_unix_nanos: progress.updated_at_unix_nanos(),
            version: progress.version(),
        }
    }
}

fn validate_size(bytes: &[u8]) -> Result<(), SdkError> {
    if bytes.len() as u64 > EXPORT_SELECTION_PROGRESS_STATE_MAXIMUM_BYTES {
        return Err(persisted_error(format!(
            "export selection progress state exceeds {EXPORT_SELECTION_PROGRESS_STATE_MAXIMUM_BYTES} bytes"
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
        "CUSTOMER_DATA_EXPORT_SELECTION_PROGRESS_PERSISTED_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "Stored customer export selection progress is invalid.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_initial_advanced_and_multi_page_progress() {
        let initial = PartyExportSelectionProgress::create(
            ExportJobId::try_new("selection-progress-persistence-initial").unwrap(),
            10,
        )
        .unwrap();
        let initial_bytes = encode_export_selection_progress_state(&initial).unwrap();
        assert_eq!(
            decode_export_selection_progress_state(&initial_bytes).unwrap(),
            initial
        );

        let mut advanced = PartyExportSelectionProgress::create(
            ExportJobId::try_new("selection-progress-persistence-advanced").unwrap(),
            10,
        )
        .unwrap();
        advanced
            .advance(
                1,
                3,
                Some(
                    PartyExportSourceContinuation::try_new(
                        "2026-07-15T00:00:00Z",
                        RecordId::try_new("party-3").unwrap(),
                    )
                    .unwrap(),
                ),
                20,
            )
            .unwrap();
        advanced
            .advance(
                2,
                2,
                Some(
                    PartyExportSourceContinuation::try_new(
                        "2026-07-15T00:01:00Z",
                        RecordId::try_new("party-5").unwrap(),
                    )
                    .unwrap(),
                ),
                30,
            )
            .unwrap();
        let bytes = encode_export_selection_progress_state(&advanced).unwrap();
        assert_eq!(decode_export_selection_progress_state(&bytes).unwrap(), advanced);
    }

    #[test]
    fn rejects_unknown_fields_tampered_identity_and_incomplete_continuation() {
        let progress = PartyExportSelectionProgress::create(
            ExportJobId::try_new("selection-progress-persistence-tamper").unwrap(),
            10,
        )
        .unwrap();
        let bytes = encode_export_selection_progress_state(&progress).unwrap();

        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["unknown"] = serde_json::json!(true);
        assert!(
            decode_export_selection_progress_state(&serde_json::to_vec(&value).unwrap()).is_err()
        );

        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["progress_id"] = serde_json::json!("cdo-export-selection-progress-tampered");
        assert!(
            decode_export_selection_progress_state(&serde_json::to_vec(&value).unwrap()).is_err()
        );

        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["version"] = serde_json::json!(2);
        value["source_after_sort_value"] = serde_json::json!("2026-07-15T00:00:00Z");
        assert!(
            decode_export_selection_progress_state(&serde_json::to_vec(&value).unwrap()).is_err()
        );
    }

    #[test]
    fn descriptor_hash_is_stable_and_non_zero() {
        assert_ne!(export_selection_progress_state_descriptor_hash(), [0; 32]);
    }
}
