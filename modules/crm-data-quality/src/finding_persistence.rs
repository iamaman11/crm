use crate::{
    PartyFinding, PartyFindingObservation, PartyFindingStatus, QualitySeverity, RuleKey,
    finding::{PartyFindingObservationRestore, PartyFindingRestore},
};
use crm_module_sdk::{ActorId, ErrorCategory, RecordId, SdkError, TenantId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const FINDING_STATE_SCHEMA_ID: &str = "crm.data-quality.finding.state";
pub const FINDING_STATE_SCHEMA_VERSION: &str = "1.0.0";
pub const FINDING_STATE_MAXIMUM_BYTES: u64 = 32 * 1024;
pub const FINDING_STATE_RETENTION_POLICY_ID: &str = "crm.data_quality.findings";

pub const FINDING_OBSERVATION_STATE_SCHEMA_ID: &str =
    "crm.data-quality.finding_observation.state";
pub const FINDING_OBSERVATION_STATE_SCHEMA_VERSION: &str = "1.0.0";
pub const FINDING_OBSERVATION_STATE_MAXIMUM_BYTES: u64 = 16 * 1024;
pub const FINDING_OBSERVATION_STATE_RETENTION_POLICY_ID: &str =
    "crm.data_quality.findings";

const FINDING_STATE_DESCRIPTOR: &[u8] = b"crm.data-quality.finding.state/v1:tenant_id,finding_id,party_id,rule_set_version_id,rule_key,severity,status,current_observation_id,evaluated_party_resource_version,assigned_actor_id,waiver_reason,remediated_by_rule_outcome_id,created_at_decimal,updated_at_decimal";
const FINDING_OBSERVATION_STATE_DESCRIPTOR: &[u8] = b"crm.data-quality.finding_observation.state/v1:tenant_id,observation_id,finding_id,party_id,party_resource_version,rule_set_version_id,rule_key,reason_code,observed_at_decimal";

pub fn finding_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(FINDING_STATE_DESCRIPTOR).into()
}

pub fn finding_observation_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(FINDING_OBSERVATION_STATE_DESCRIPTOR).into()
}

pub fn encode_finding_state(finding: &PartyFinding) -> Result<Vec<u8>, SdkError> {
    let bytes = serde_json::to_vec(&PartyFindingStateV1::from(finding))
        .map_err(|error| persisted_error(format!("finding serialization failed: {error}")))?;
    validate_size(&bytes, FINDING_STATE_MAXIMUM_BYTES, "finding")?;
    Ok(bytes)
}

pub fn decode_finding_state(bytes: &[u8]) -> Result<PartyFinding, SdkError> {
    validate_size(bytes, FINDING_STATE_MAXIMUM_BYTES, "finding")?;
    let state: PartyFindingStateV1 = serde_json::from_slice(bytes)
        .map_err(|error| persisted_error(format!("finding JSON is invalid: {error}")))?;
    let finding = state.into_domain()?;
    if encode_finding_state(&finding)? != bytes {
        return Err(persisted_error(
            "persisted finding is not the strict canonical v1 encoding",
        ));
    }
    Ok(finding)
}

pub fn encode_finding_observation_state(
    observation: &PartyFindingObservation,
) -> Result<Vec<u8>, SdkError> {
    let bytes = serde_json::to_vec(&PartyFindingObservationStateV1::from(observation)).map_err(
        |error| persisted_error(format!("finding observation serialization failed: {error}")),
    )?;
    validate_size(
        &bytes,
        FINDING_OBSERVATION_STATE_MAXIMUM_BYTES,
        "finding observation",
    )?;
    Ok(bytes)
}

pub fn decode_finding_observation_state(
    bytes: &[u8],
) -> Result<PartyFindingObservation, SdkError> {
    validate_size(
        bytes,
        FINDING_OBSERVATION_STATE_MAXIMUM_BYTES,
        "finding observation",
    )?;
    let state: PartyFindingObservationStateV1 = serde_json::from_slice(bytes).map_err(|error| {
        persisted_error(format!("finding observation JSON is invalid: {error}"))
    })?;
    let observation = state.into_domain()?;
    if encode_finding_observation_state(&observation)? != bytes {
        return Err(persisted_error(
            "persisted finding observation is not the strict canonical v1 encoding",
        ));
    }
    Ok(observation)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartyFindingStateV1 {
    tenant_id: String,
    finding_id: String,
    party_id: String,
    rule_set_version_id: String,
    rule_key: String,
    severity: QualitySeverity,
    status: PartyFindingStatus,
    current_observation_id: String,
    evaluated_party_resource_version: i64,
    assigned_actor_id: Option<String>,
    waiver_reason: Option<String>,
    remediated_by_rule_outcome_id: Option<String>,
    created_at: String,
    updated_at: String,
}

impl From<&PartyFinding> for PartyFindingStateV1 {
    fn from(finding: &PartyFinding) -> Self {
        Self {
            tenant_id: finding.tenant_id().as_str().to_owned(),
            finding_id: finding.finding_id().to_owned(),
            party_id: finding.party_id().as_str().to_owned(),
            rule_set_version_id: finding.rule_set_version_id().to_owned(),
            rule_key: finding.rule_key().as_str().to_owned(),
            severity: finding.severity(),
            status: finding.status(),
            current_observation_id: finding.current_observation_id().to_owned(),
            evaluated_party_resource_version: finding.evaluated_party_resource_version(),
            assigned_actor_id: finding
                .assigned_actor_id()
                .map(|actor_id| actor_id.as_str().to_owned()),
            waiver_reason: finding.waiver_reason().map(str::to_owned),
            remediated_by_rule_outcome_id: finding
                .remediated_by_rule_outcome_id()
                .map(str::to_owned),
            created_at: finding.created_at().to_string(),
            updated_at: finding.updated_at().to_string(),
        }
    }
}

impl PartyFindingStateV1 {
    fn into_domain(self) -> Result<PartyFinding, SdkError> {
        PartyFinding::restore(PartyFindingRestore {
            tenant_id: tenant_id(self.tenant_id)?,
            finding_id: self.finding_id,
            party_id: record_id(self.party_id, "Party")?,
            rule_set_version_id: self.rule_set_version_id,
            rule_key: RuleKey::try_new(self.rule_key).map_err(persisted_domain_error)?,
            severity: self.severity,
            status: self.status,
            current_observation_id: self.current_observation_id,
            evaluated_party_resource_version: self.evaluated_party_resource_version,
            assigned_actor_id: self.assigned_actor_id.map(actor_id).transpose()?,
            waiver_reason: self.waiver_reason,
            remediated_by_rule_outcome_id: self.remediated_by_rule_outcome_id,
            created_at: decimal_i64(self.created_at, "created_at")?,
            updated_at: decimal_i64(self.updated_at, "updated_at")?,
        })
        .map_err(persisted_domain_error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartyFindingObservationStateV1 {
    tenant_id: String,
    observation_id: String,
    finding_id: String,
    party_id: String,
    party_resource_version: i64,
    rule_set_version_id: String,
    rule_key: String,
    reason_code: String,
    observed_at: String,
}

impl From<&PartyFindingObservation> for PartyFindingObservationStateV1 {
    fn from(observation: &PartyFindingObservation) -> Self {
        Self {
            tenant_id: observation.tenant_id().as_str().to_owned(),
            observation_id: observation.observation_id().to_owned(),
            finding_id: observation.finding_id().to_owned(),
            party_id: observation.party_id().as_str().to_owned(),
            party_resource_version: observation.party_resource_version(),
            rule_set_version_id: observation.rule_set_version_id().to_owned(),
            rule_key: observation.rule_key().as_str().to_owned(),
            reason_code: observation.reason_code().to_owned(),
            observed_at: observation.observed_at().to_string(),
        }
    }
}

impl PartyFindingObservationStateV1 {
    fn into_domain(self) -> Result<PartyFindingObservation, SdkError> {
        PartyFindingObservation::restore(PartyFindingObservationRestore {
            tenant_id: tenant_id(self.tenant_id)?,
            observation_id: self.observation_id,
            finding_id: self.finding_id,
            party_id: record_id(self.party_id, "Party")?,
            party_resource_version: self.party_resource_version,
            rule_set_version_id: self.rule_set_version_id,
            rule_key: RuleKey::try_new(self.rule_key).map_err(persisted_domain_error)?,
            reason_code: self.reason_code,
            observed_at: decimal_i64(self.observed_at, "observed_at")?,
        })
        .map_err(persisted_domain_error)
    }
}

fn tenant_id(value: String) -> Result<TenantId, SdkError> {
    TenantId::try_new(value).map_err(|error| {
        persisted_error(format!("persisted tenant identity is invalid: {error}"))
    })
}

fn record_id(value: String, label: &str) -> Result<RecordId, SdkError> {
    RecordId::try_new(value)
        .map_err(|error| persisted_error(format!("persisted {label} identity is invalid: {error}")))
}

fn actor_id(value: String) -> Result<ActorId, SdkError> {
    ActorId::try_new(value).map_err(|error| {
        persisted_error(format!("persisted assigned actor identity is invalid: {error}"))
    })
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

fn validate_size(bytes: &[u8], maximum: u64, label: &str) -> Result<(), SdkError> {
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum {
        return Err(persisted_error(format!(
            "{label} state exceeds its maximum size"
        )));
    }
    Ok(())
}

fn persisted_domain_error(error: SdkError) -> SdkError {
    persisted_error(format!(
        "finding state failed strict domain validation: {}: {}",
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
