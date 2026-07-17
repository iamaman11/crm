use crate::{PartyRuleOutcome, RuleKey, rule_outcome::PartyRuleOutcomeRestore};
use crm_module_sdk::{ErrorCategory, RecordId, SdkError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const RULE_OUTCOME_STATE_SCHEMA_ID: &str = "crm.data-quality.rule_outcome.state";
pub const RULE_OUTCOME_STATE_SCHEMA_VERSION: &str = "1.0.0";
pub const RULE_OUTCOME_STATE_MAXIMUM_BYTES: u64 = 16 * 1024;
pub const RULE_OUTCOME_STATE_RETENTION_POLICY_ID: &str = "crm.data_quality.evaluation";

const RULE_OUTCOME_STATE_DESCRIPTOR: &[u8] = b"crm.data-quality.rule_outcome.state/v1:outcome_id,job_id,party_id,party_resource_version,rule_set_version_id,rule_key,passed,reason_code,evaluated_at_decimal";

pub fn rule_outcome_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(RULE_OUTCOME_STATE_DESCRIPTOR).into()
}

pub fn encode_rule_outcome_state(outcome: &PartyRuleOutcome) -> Result<Vec<u8>, SdkError> {
    let bytes = serde_json::to_vec(&PartyRuleOutcomeStateV1::from(outcome))
        .map_err(|error| persisted_error(format!("rule outcome serialization failed: {error}")))?;
    validate_size(&bytes)?;
    Ok(bytes)
}

pub fn decode_rule_outcome_state(bytes: &[u8]) -> Result<PartyRuleOutcome, SdkError> {
    validate_size(bytes)?;
    let state: PartyRuleOutcomeStateV1 = serde_json::from_slice(bytes)
        .map_err(|error| persisted_error(format!("rule outcome JSON is invalid: {error}")))?;
    let outcome = state.into_domain()?;
    if encode_rule_outcome_state(&outcome)? != bytes {
        return Err(persisted_error(
            "persisted rule outcome is not the strict canonical v1 encoding",
        ));
    }
    Ok(outcome)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartyRuleOutcomeStateV1 {
    outcome_id: String,
    job_id: String,
    party_id: String,
    party_resource_version: i64,
    rule_set_version_id: String,
    rule_key: String,
    passed: bool,
    reason_code: String,
    evaluated_at: String,
}

impl From<&PartyRuleOutcome> for PartyRuleOutcomeStateV1 {
    fn from(outcome: &PartyRuleOutcome) -> Self {
        Self {
            outcome_id: outcome.outcome_id().to_owned(),
            job_id: outcome.job_id().as_str().to_owned(),
            party_id: outcome.party_id().as_str().to_owned(),
            party_resource_version: outcome.party_resource_version(),
            rule_set_version_id: outcome.rule_set_version_id().to_owned(),
            rule_key: outcome.rule_key().as_str().to_owned(),
            passed: outcome.passed(),
            reason_code: outcome.reason_code().to_owned(),
            evaluated_at: outcome.evaluated_at().to_string(),
        }
    }
}

impl PartyRuleOutcomeStateV1 {
    fn into_domain(self) -> Result<PartyRuleOutcome, SdkError> {
        PartyRuleOutcome::restore(PartyRuleOutcomeRestore {
            outcome_id: self.outcome_id,
            job_id: record_id(self.job_id, "evaluation job")?,
            party_id: record_id(self.party_id, "Party")?,
            party_resource_version: self.party_resource_version,
            rule_set_version_id: self.rule_set_version_id,
            rule_key: RuleKey::try_new(self.rule_key).map_err(persisted_domain_error)?,
            passed: self.passed,
            reason_code: self.reason_code,
            evaluated_at: decimal_i64(self.evaluated_at, "evaluated_at")?,
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
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > RULE_OUTCOME_STATE_MAXIMUM_BYTES {
        return Err(persisted_error("rule outcome state exceeds its maximum size"));
    }
    Ok(())
}

fn persisted_domain_error(error: SdkError) -> SdkError {
    persisted_error(format!(
        "rule outcome failed strict persisted-state validation: {}: {}",
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
