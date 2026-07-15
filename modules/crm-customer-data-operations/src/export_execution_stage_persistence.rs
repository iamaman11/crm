use crate::{
    ExportJobId, PartyExportExclusionReason, PartyExportExecutionStage,
    PartyExportExecutionStageKind,
};
use crm_module_sdk::{ErrorCategory, SdkError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const EXPORT_EXECUTION_STAGE_STATE_SCHEMA_ID: &str =
    "crm.customer-data-operations.export_execution_stage.state";
pub const EXPORT_EXECUTION_STAGE_STATE_SCHEMA_VERSION: &str = "1.0.0";
pub const EXPORT_EXECUTION_STAGE_STATE_MAXIMUM_BYTES: u64 = 600 * 1024;
pub const EXPORT_EXECUTION_STAGE_STATE_RETENTION_POLICY_ID: &str =
    "crm.customer_data.export_execution_stage";

const DESCRIPTOR: &[u8] = b"crm.customer-data-operations.export_execution_stage.state/v1:stage_id,export_job_id,manifest_position,stage_kind,row_utf8,row_sha256,redacted_fields,occurred_at_unix_nanos,version";
const VERSION: u32 = 1;

pub fn export_execution_stage_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(DESCRIPTOR).into()
}

pub fn encode_export_execution_stage_state(
    stage: &PartyExportExecutionStage,
) -> Result<Vec<u8>, SdkError> {
    let bytes = serde_json::to_vec(&StageStateV1::from(stage))
        .map_err(|error| persisted_error(error.to_string()))?;
    validate_size(&bytes)?;
    Ok(bytes)
}

pub fn decode_export_execution_stage_state(
    bytes: &[u8],
) -> Result<PartyExportExecutionStage, SdkError> {
    validate_size(bytes)?;
    let state: StageStateV1 =
        serde_json::from_slice(bytes).map_err(|error| persisted_error(error.to_string()))?;
    if state.version != VERSION {
        return Err(persisted_error("unsupported execution stage state version"));
    }
    let expected_stage_id = state.stage_id.clone();
    let job_id = ExportJobId::try_new(state.export_job_id.clone())
        .map_err(|error| persisted_domain_error("export job ID", error))?;
    let stage = match state.stage_kind.as_str() {
        "emitted" => {
            let row_utf8 = state
                .row_utf8
                .clone()
                .ok_or_else(|| persisted_error("emitted stage is missing row bytes"))?;
            let row_sha256 = state
                .row_sha256
                .clone()
                .ok_or_else(|| persisted_error("emitted stage is missing row SHA-256"))?;
            let stage = PartyExportExecutionStage::emitted(
                job_id,
                state.manifest_position,
                row_utf8,
                state.redacted_fields,
                state.occurred_at_unix_nanos,
            )
            .map_err(|error| persisted_domain_error("emitted execution stage", error))?;
            match stage.kind() {
                PartyExportExecutionStageKind::Emitted {
                    row_sha256: canonical,
                    ..
                } if canonical == &row_sha256 => stage,
                _ => return Err(persisted_error("execution stage row hash is inconsistent")),
            }
        }
        "excluded_not_visible" => {
            decode_excluded(state, job_id, PartyExportExclusionReason::NotVisible)?
        }
        "excluded_version_changed" => {
            decode_excluded(state, job_id, PartyExportExclusionReason::VersionChanged)?
        }
        "excluded_unavailable" => {
            decode_excluded(state, job_id, PartyExportExclusionReason::Unavailable)?
        }
        _ => return Err(persisted_error("unsupported execution stage kind")),
    };
    if stage.stage_id().as_str() != expected_stage_id {
        return Err(persisted_error("execution stage identity is inconsistent"));
    }
    if encode_export_execution_stage_state(&stage)? != bytes {
        return Err(persisted_error(
            "execution stage state is not the strict canonical v1 encoding",
        ));
    }
    Ok(stage)
}

fn decode_excluded(
    state: StageStateV1,
    job_id: ExportJobId,
    reason: PartyExportExclusionReason,
) -> Result<PartyExportExecutionStage, SdkError> {
    if state.row_utf8.is_some() || state.row_sha256.is_some() || state.redacted_fields != 0 {
        return Err(persisted_error(
            "excluded execution stage contains emitted-row fields",
        ));
    }
    PartyExportExecutionStage::excluded(
        job_id,
        state.manifest_position,
        reason,
        state.occurred_at_unix_nanos,
    )
    .map_err(|error| persisted_domain_error("excluded execution stage", error))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StageStateV1 {
    stage_id: String,
    export_job_id: String,
    manifest_position: u32,
    stage_kind: String,
    row_utf8: Option<String>,
    row_sha256: Option<String>,
    redacted_fields: u32,
    occurred_at_unix_nanos: i64,
    version: u32,
}

impl From<&PartyExportExecutionStage> for StageStateV1 {
    fn from(stage: &PartyExportExecutionStage) -> Self {
        let (stage_kind, row_utf8, row_sha256, redacted_fields) = match stage.kind() {
            PartyExportExecutionStageKind::Emitted {
                row_utf8,
                row_sha256,
                redacted_fields,
            } => (
                "emitted".to_owned(),
                Some(row_utf8.clone()),
                Some(row_sha256.clone()),
                *redacted_fields,
            ),
            PartyExportExecutionStageKind::Excluded(PartyExportExclusionReason::NotVisible) => {
                ("excluded_not_visible".to_owned(), None, None, 0)
            }
            PartyExportExecutionStageKind::Excluded(PartyExportExclusionReason::VersionChanged) => {
                ("excluded_version_changed".to_owned(), None, None, 0)
            }
            PartyExportExecutionStageKind::Excluded(PartyExportExclusionReason::Unavailable) => {
                ("excluded_unavailable".to_owned(), None, None, 0)
            }
        };
        Self {
            stage_id: stage.stage_id().as_str().to_owned(),
            export_job_id: stage.job_id().as_str().to_owned(),
            manifest_position: stage.manifest_position(),
            stage_kind,
            row_utf8,
            row_sha256,
            redacted_fields,
            occurred_at_unix_nanos: stage.occurred_at_unix_nanos(),
            version: VERSION,
        }
    }
}

fn validate_size(bytes: &[u8]) -> Result<(), SdkError> {
    if bytes.len() as u64 > EXPORT_EXECUTION_STAGE_STATE_MAXIMUM_BYTES {
        return Err(persisted_error(
            "execution stage state exceeds maximum size",
        ));
    }
    Ok(())
}

fn persisted_domain_error(context: &str, error: SdkError) -> SdkError {
    persisted_error(format!("{context}: {}: {}", error.code, error.safe_message))
}

fn persisted_error(detail: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_EXECUTION_STAGE_PERSISTED_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "Stored customer export execution staging evidence is invalid.",
    )
    .with_internal_reference(detail.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_emitted_and_excluded_stage_evidence() {
        let job_id = ExportJobId::try_new("stage-persistence-job").unwrap();
        let stages = [
            PartyExportExecutionStage::emitted(
                job_id.clone(),
                1,
                "party-1,person,Alice\n".to_owned(),
                1,
                100,
            )
            .unwrap(),
            PartyExportExecutionStage::excluded(
                job_id,
                2,
                PartyExportExclusionReason::VersionChanged,
                101,
            )
            .unwrap(),
        ];
        for stage in stages {
            let bytes = encode_export_execution_stage_state(&stage).unwrap();
            let decoded = decode_export_execution_stage_state(&bytes).unwrap();
            assert_eq!(decoded, stage);
            assert_eq!(
                encode_export_execution_stage_state(&decoded).unwrap(),
                bytes
            );
        }
    }

    #[test]
    fn rejects_tampered_row_hash() {
        let stage = PartyExportExecutionStage::emitted(
            ExportJobId::try_new("stage-tamper-job").unwrap(),
            1,
            "party-1\n".to_owned(),
            0,
            100,
        )
        .unwrap();
        let bytes = encode_export_execution_stage_state(&stage).unwrap();
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["row_sha256"] = serde_json::json!("11".repeat(32));
        assert!(decode_export_execution_stage_state(&serde_json::to_vec(&value).unwrap()).is_err());
    }
}
