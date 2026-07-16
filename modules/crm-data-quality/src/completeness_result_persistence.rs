use crate::{
    ComponentKey, PartyCompletenessResult, RuleKey,
    completeness_result::{PartyCompletenessResultRestore, restore_component},
};
use crm_module_sdk::{ErrorCategory, RecordId, SdkError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PARTY_COMPLETENESS_RESULT_STATE_SCHEMA_ID: &str =
    "crm.data-quality.party_completeness_result.state";
pub const PARTY_COMPLETENESS_RESULT_STATE_SCHEMA_VERSION: &str = "1.0.0";
pub const PARTY_COMPLETENESS_RESULT_STATE_MAXIMUM_BYTES: u64 = 64 * 1024;
pub const PARTY_COMPLETENESS_RESULT_STATE_RETENTION_POLICY_ID: &str =
    "crm.data_quality.evaluation";

const PARTY_COMPLETENESS_RESULT_STATE_DESCRIPTOR: &[u8] = b"crm.data-quality.party_completeness_result.state/v1:result_id,job_id,party_id,party_resource_version,profile_version_id,score_basis_points,components,computed_at_decimal";

pub fn party_completeness_result_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(PARTY_COMPLETENESS_RESULT_STATE_DESCRIPTOR).into()
}

pub fn encode_party_completeness_result_state(
    result: &PartyCompletenessResult,
) -> Result<Vec<u8>, SdkError> {
    let bytes = serde_json::to_vec(&PartyCompletenessResultStateV1::from(result)).map_err(|error| {
        persisted_error(format!("completeness result serialization failed: {error}"))
    })?;
    validate_size(&bytes)?;
    Ok(bytes)
}

pub fn decode_party_completeness_result_state(
    bytes: &[u8],
) -> Result<PartyCompletenessResult, SdkError> {
    validate_size(bytes)?;
    let state: PartyCompletenessResultStateV1 = serde_json::from_slice(bytes).map_err(|error| {
        persisted_error(format!("completeness result JSON is invalid: {error}"))
    })?;
    let result = state.into_domain()?;
    if encode_party_completeness_result_state(&result)? != bytes {
        return Err(persisted_error(
            "persisted completeness result is not the strict canonical v1 encoding",
        ));
    }
    Ok(result)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartyCompletenessResultStateV1 {
    result_id: String,
    job_id: String,
    party_id: String,
    party_resource_version: i64,
    profile_version_id: String,
    score_basis_points: u32,
    components: Vec<PartyCompletenessComponentResultStateV1>,
    computed_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartyCompletenessComponentResultStateV1 {
    component_key: String,
    rule_key: String,
    rule_outcome_id: String,
    awarded_basis_points: u32,
}

impl From<&PartyCompletenessResult> for PartyCompletenessResultStateV1 {
    fn from(result: &PartyCompletenessResult) -> Self {
        Self {
            result_id: result.result_id().to_owned(),
            job_id: result.job_id().as_str().to_owned(),
            party_id: result.party_id().as_str().to_owned(),
            party_resource_version: result.party_resource_version(),
            profile_version_id: result.profile_version_id().to_owned(),
            score_basis_points: result.score_basis_points(),
            components: result
                .components()
                .iter()
                .map(|component| PartyCompletenessComponentResultStateV1 {
                    component_key: component.component_key().as_str().to_owned(),
                    rule_key: component.rule_key().as_str().to_owned(),
                    rule_outcome_id: component.rule_outcome_id().to_owned(),
                    awarded_basis_points: component.awarded_basis_points(),
                })
                .collect(),
            computed_at: result.computed_at().to_string(),
        }
    }
}

impl PartyCompletenessResultStateV1 {
    fn into_domain(self) -> Result<PartyCompletenessResult, SdkError> {
        let components = self
            .components
            .into_iter()
            .map(|component| {
                Ok(restore_component(
                    ComponentKey::try_new(component.component_key)
                        .map_err(persisted_domain_error)?,
                    RuleKey::try_new(component.rule_key).map_err(persisted_domain_error)?,
                    component.rule_outcome_id,
                    component.awarded_basis_points,
                ))
            })
            .collect::<Result<Vec<_>, SdkError>>()?;
        PartyCompletenessResult::restore(PartyCompletenessResultRestore {
            result_id: self.result_id,
            job_id: record_id(self.job_id, "evaluation job")?,
            party_id: record_id(self.party_id, "Party")?,
            party_resource_version: self.party_resource_version,
            profile_version_id: self.profile_version_id,
            score_basis_points: self.score_basis_points,
            components,
            computed_at: decimal_i64(self.computed_at, "computed_at")?,
        })
        .map_err(persisted_domain_error)
    }
}

fn record_id(value: String, label: &str) -> Result<RecordId, SdkError> {
    RecordId::try_new(value)
        .map_err(|error| persisted_error(format!("persisted {label} identity is invalid: {error}")))
}

fn decimal_i64(value: String, field: &str) -> Result<i64, SdkError> {
    let parsed = value.parse::<i64>().map_err(|error| {
        persisted_error(format!("persisted {field} decimal is invalid: {error}"))
    })?;
    if parsed.to_string() != value {
        return Err(persisted_error(format!(
            "persisted {field} decimal is not canonical"
        )));
    }
    Ok(parsed)
}

fn validate_size(bytes: &[u8]) -> Result<(), SdkError> {
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX)
        > PARTY_COMPLETENESS_RESULT_STATE_MAXIMUM_BYTES
    {
        return Err(persisted_error(
            "completeness result state exceeds its maximum size",
        ));
    }
    Ok(())
}

fn persisted_domain_error(error: SdkError) -> SdkError {
    persisted_error(format!(
        "completeness result failed strict persisted-state validation: {}: {}",
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
