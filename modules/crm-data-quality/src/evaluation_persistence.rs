use crate::{
    EvaluatedPartyKind, PartyEvaluationInputSnapshot, PartyEvaluationJob, PartyEvaluationJobStatus,
};
use crm_module_sdk::{ErrorCategory, RecordId, SdkError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PARTY_EVALUATION_JOB_STATE_SCHEMA_ID: &str =
    "crm.data-quality.party_evaluation_job.state";
pub const PARTY_EVALUATION_JOB_STATE_SCHEMA_VERSION: &str = "1.0.0";
pub const PARTY_EVALUATION_JOB_STATE_MAXIMUM_BYTES: u64 = 32 * 1024;
pub const PARTY_EVALUATION_JOB_STATE_RETENTION_POLICY_ID: &str =
    "crm.data_quality.evaluation";

pub const PARTY_EVALUATION_INPUT_STATE_SCHEMA_ID: &str =
    "crm.data-quality.party_evaluation_input.state";
pub const PARTY_EVALUATION_INPUT_STATE_SCHEMA_VERSION: &str = "1.0.0";
pub const PARTY_EVALUATION_INPUT_STATE_MAXIMUM_BYTES: u64 = 16 * 1024;
pub const PARTY_EVALUATION_INPUT_STATE_RETENTION_POLICY_ID: &str =
    "crm.data_quality.evaluation_input";

const PARTY_EVALUATION_JOB_STATE_DESCRIPTOR: &[u8] = b"crm.data-quality.party_evaluation_job.state/v1:job_id,party_id,rule_set_version_id,profile_version_id,status,party_resource_version,evaluated_rules,failed_rules,created_at,updated_at";
const PARTY_EVALUATION_INPUT_STATE_DESCRIPTOR: &[u8] = b"crm.data-quality.party_evaluation_input.state/v1:job_id,party_id,kind,display_name,party_resource_version,captured_at";

pub fn party_evaluation_job_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(PARTY_EVALUATION_JOB_STATE_DESCRIPTOR).into()
}

pub fn party_evaluation_input_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(PARTY_EVALUATION_INPUT_STATE_DESCRIPTOR).into()
}

pub fn encode_party_evaluation_job_state(
    job: &PartyEvaluationJob,
) -> Result<Vec<u8>, SdkError> {
    let bytes = serde_json::to_vec(&PartyEvaluationJobStateV1::from(job))
        .map_err(|error| persisted_error(format!("evaluation job serialization failed: {error}")))?;
    validate_size(
        &bytes,
        PARTY_EVALUATION_JOB_STATE_MAXIMUM_BYTES,
        "Party evaluation job",
    )?;
    Ok(bytes)
}

pub fn decode_party_evaluation_job_state(bytes: &[u8]) -> Result<PartyEvaluationJob, SdkError> {
    validate_size(
        bytes,
        PARTY_EVALUATION_JOB_STATE_MAXIMUM_BYTES,
        "Party evaluation job",
    )?;
    let state: PartyEvaluationJobStateV1 = serde_json::from_slice(bytes)
        .map_err(|error| persisted_error(format!("evaluation job JSON is invalid: {error}")))?;
    let job = state.into_domain()?;
    if encode_party_evaluation_job_state(&job)? != bytes {
        return Err(persisted_error(
            "persisted evaluation job is not the strict canonical v1 encoding",
        ));
    }
    Ok(job)
}

pub fn encode_party_evaluation_input_state(
    input: &PartyEvaluationInputSnapshot,
) -> Result<Vec<u8>, SdkError> {
    let bytes = serde_json::to_vec(&PartyEvaluationInputStateV1::from(input)).map_err(|error| {
        persisted_error(format!("evaluation input serialization failed: {error}"))
    })?;
    validate_size(
        &bytes,
        PARTY_EVALUATION_INPUT_STATE_MAXIMUM_BYTES,
        "Party evaluation input",
    )?;
    Ok(bytes)
}

pub fn decode_party_evaluation_input_state(
    bytes: &[u8],
) -> Result<PartyEvaluationInputSnapshot, SdkError> {
    validate_size(
        bytes,
        PARTY_EVALUATION_INPUT_STATE_MAXIMUM_BYTES,
        "Party evaluation input",
    )?;
    let state: PartyEvaluationInputStateV1 = serde_json::from_slice(bytes)
        .map_err(|error| persisted_error(format!("evaluation input JSON is invalid: {error}")))?;
    let input = state.into_domain()?;
    if encode_party_evaluation_input_state(&input)? != bytes {
        return Err(persisted_error(
            "persisted evaluation input is not the strict canonical v1 encoding",
        ));
    }
    Ok(input)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartyEvaluationJobStateV1 {
    job_id: String,
    party_id: String,
    rule_set_version_id: String,
    profile_version_id: String,
    status: PartyEvaluationJobStatusState,
    party_resource_version: Option<i64>,
    evaluated_rules: u32,
    failed_rules: u32,
    created_at: i64,
    updated_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PartyEvaluationJobStatusState {
    Created,
    Staged,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartyEvaluationInputStateV1 {
    job_id: String,
    party_id: String,
    kind: EvaluatedPartyKind,
    display_name: String,
    party_resource_version: i64,
    captured_at: i64,
}

impl From<&PartyEvaluationJob> for PartyEvaluationJobStateV1 {
    fn from(job: &PartyEvaluationJob) -> Self {
        Self {
            job_id: job.job_id().as_str().to_owned(),
            party_id: job.party_id().as_str().to_owned(),
            rule_set_version_id: job.rule_set_version_id().to_owned(),
            profile_version_id: job.profile_version_id().to_owned(),
            status: job.status().into(),
            party_resource_version: job.party_resource_version(),
            evaluated_rules: job.evaluated_rules(),
            failed_rules: job.failed_rules(),
            created_at: job.created_at(),
            updated_at: job.updated_at(),
        }
    }
}

impl PartyEvaluationJobStateV1 {
    fn into_domain(self) -> Result<PartyEvaluationJob, SdkError> {
        PartyEvaluationJob::restore(
            record_id(self.job_id, "evaluation job")?,
            record_id(self.party_id, "Party")?,
            self.rule_set_version_id,
            self.profile_version_id,
            self.status.into(),
            self.party_resource_version,
            self.evaluated_rules,
            self.failed_rules,
            self.created_at,
            self.updated_at,
        )
        .map_err(|error| persisted_domain_error("Party evaluation job", error))
    }
}

impl From<PartyEvaluationJobStatus> for PartyEvaluationJobStatusState {
    fn from(value: PartyEvaluationJobStatus) -> Self {
        match value {
            PartyEvaluationJobStatus::Created => Self::Created,
            PartyEvaluationJobStatus::Staged => Self::Staged,
            PartyEvaluationJobStatus::Completed => Self::Completed,
        }
    }
}

impl From<PartyEvaluationJobStatusState> for PartyEvaluationJobStatus {
    fn from(value: PartyEvaluationJobStatusState) -> Self {
        match value {
            PartyEvaluationJobStatusState::Created => Self::Created,
            PartyEvaluationJobStatusState::Staged => Self::Staged,
            PartyEvaluationJobStatusState::Completed => Self::Completed,
        }
    }
}

impl From<&PartyEvaluationInputSnapshot> for PartyEvaluationInputStateV1 {
    fn from(input: &PartyEvaluationInputSnapshot) -> Self {
        Self {
            job_id: input.job_id().as_str().to_owned(),
            party_id: input.party_id().as_str().to_owned(),
            kind: input.kind(),
            display_name: input.display_name().to_owned(),
            party_resource_version: input.party_resource_version(),
            captured_at: input.captured_at(),
        }
    }
}

impl PartyEvaluationInputStateV1 {
    fn into_domain(self) -> Result<PartyEvaluationInputSnapshot, SdkError> {
        PartyEvaluationInputSnapshot::restore(
            record_id(self.job_id, "evaluation job")?,
            record_id(self.party_id, "Party")?,
            self.kind,
            self.display_name,
            self.party_resource_version,
            self.captured_at,
        )
        .map_err(|error| persisted_domain_error("Party evaluation input", error))
    }
}

fn record_id(value: String, label: &str) -> Result<RecordId, SdkError> {
    RecordId::try_new(value)
        .map_err(|error| persisted_error(format!("persisted {label} identity is invalid: {error}")))
}

fn validate_size(bytes: &[u8], maximum_bytes: u64, label: &str) -> Result<(), SdkError> {
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum_bytes {
        return Err(persisted_error(format!(
            "{label} state exceeds the maximum of {maximum_bytes} bytes"
        )));
    }
    Ok(())
}

fn persisted_domain_error(label: &str, error: SdkError) -> SdkError {
    persisted_error(format!(
        "{label} failed strict persisted-state validation: {}: {}",
        error.code, error.safe_message
    ))
}

fn persisted_error(message: impl Into<String>) -> SdkError {
    SdkError::new(
        "DATA_QUALITY_PERSISTED_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "The persisted Data Quality state is invalid.",
    )
    .with_internal_reference(message)
}
