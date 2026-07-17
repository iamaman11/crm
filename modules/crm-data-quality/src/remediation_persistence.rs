use crate::{
    PartyDisplayNameRemediationAttempt,
    remediation::PartyDisplayNameRemediationAttemptRestore,
};
use crm_module_sdk::{ErrorCategory, IdempotencyKey, RecordId, SdkError, TenantId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const REMEDIATION_ATTEMPT_STATE_SCHEMA_ID: &str =
    "crm.data-quality.remediation_attempt.state";
pub const REMEDIATION_ATTEMPT_STATE_SCHEMA_VERSION: &str = "1.0.0";
pub const REMEDIATION_ATTEMPT_STATE_MAXIMUM_BYTES: u64 = 32 * 1024;
pub const REMEDIATION_ATTEMPT_STATE_RETENTION_POLICY_ID: &str =
    "crm.data_quality.remediation_attempts";

const REMEDIATION_ATTEMPT_STATE_DESCRIPTOR: &[u8] = b"crm.data-quality.remediation_attempt.state/v1:tenant_id,attempt_id,caller_idempotency_key,finding_id,expected_finding_version,observation_id,party_id,expected_party_version,requested_display_name,target_idempotency_key,updated_party_version,completed_at_decimal";

pub fn remediation_attempt_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(REMEDIATION_ATTEMPT_STATE_DESCRIPTOR).into()
}

pub fn encode_remediation_attempt_state(
    attempt: &PartyDisplayNameRemediationAttempt,
) -> Result<Vec<u8>, SdkError> {
    let bytes = serde_json::to_vec(&PartyDisplayNameRemediationAttemptStateV1::from(attempt))
        .map_err(|error| persisted_error(format!("remediation serialization failed: {error}")))?;
    validate_size(&bytes)?;
    Ok(bytes)
}

pub fn decode_remediation_attempt_state(
    bytes: &[u8],
) -> Result<PartyDisplayNameRemediationAttempt, SdkError> {
    validate_size(bytes)?;
    let state: PartyDisplayNameRemediationAttemptStateV1 = serde_json::from_slice(bytes)
        .map_err(|error| persisted_error(format!("remediation JSON is invalid: {error}")))?;
    let attempt = state.into_domain()?;
    if encode_remediation_attempt_state(&attempt)? != bytes {
        return Err(persisted_error(
            "persisted remediation attempt is not the strict canonical v1 encoding",
        ));
    }
    Ok(attempt)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartyDisplayNameRemediationAttemptStateV1 {
    tenant_id: String,
    attempt_id: String,
    caller_idempotency_key: String,
    finding_id: String,
    expected_finding_version: i64,
    observation_id: String,
    party_id: String,
    expected_party_version: i64,
    requested_display_name: String,
    target_idempotency_key: String,
    updated_party_version: i64,
    completed_at: String,
}

impl From<&PartyDisplayNameRemediationAttempt> for PartyDisplayNameRemediationAttemptStateV1 {
    fn from(attempt: &PartyDisplayNameRemediationAttempt) -> Self {
        Self {
            tenant_id: attempt.tenant_id().as_str().to_owned(),
            attempt_id: attempt.attempt_id().to_owned(),
            caller_idempotency_key: attempt.caller_idempotency_key().as_str().to_owned(),
            finding_id: attempt.finding_id().to_owned(),
            expected_finding_version: attempt.expected_finding_version(),
            observation_id: attempt.observation_id().to_owned(),
            party_id: attempt.party_id().as_str().to_owned(),
            expected_party_version: attempt.expected_party_version(),
            requested_display_name: attempt.requested_display_name().to_owned(),
            target_idempotency_key: attempt.target_idempotency_key().as_str().to_owned(),
            updated_party_version: attempt.updated_party_version(),
            completed_at: attempt.completed_at().to_string(),
        }
    }
}

impl PartyDisplayNameRemediationAttemptStateV1 {
    fn into_domain(self) -> Result<PartyDisplayNameRemediationAttempt, SdkError> {
        PartyDisplayNameRemediationAttempt::restore(PartyDisplayNameRemediationAttemptRestore {
            tenant_id: TenantId::try_new(self.tenant_id).map_err(identifier_error)?,
            attempt_id: self.attempt_id,
            caller_idempotency_key: IdempotencyKey::try_new(self.caller_idempotency_key)
                .map_err(identifier_error)?,
            finding_id: self.finding_id,
            expected_finding_version: self.expected_finding_version,
            observation_id: self.observation_id,
            party_id: RecordId::try_new(self.party_id).map_err(identifier_error)?,
            expected_party_version: self.expected_party_version,
            requested_display_name: self.requested_display_name,
            target_idempotency_key: IdempotencyKey::try_new(self.target_idempotency_key)
                .map_err(identifier_error)?,
            updated_party_version: self.updated_party_version,
            completed_at: decimal_i64(self.completed_at)?,
        })
        .map_err(|error| persisted_error(format!("remediation domain validation failed: {error}")))
    }
}

fn decimal_i64(value: String) -> Result<i64, SdkError> {
    let parsed = value
        .parse::<i64>()
        .map_err(|error| persisted_error(format!("completed_at decimal is invalid: {error}")))?;
    if parsed.to_string() != value {
        return Err(persisted_error("completed_at decimal is not canonical"));
    }
    Ok(parsed)
}

fn validate_size(bytes: &[u8]) -> Result<(), SdkError> {
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX)
        > REMEDIATION_ATTEMPT_STATE_MAXIMUM_BYTES
    {
        return Err(persisted_error(
            "remediation attempt exceeds its maximum size",
        ));
    }
    Ok(())
}

fn identifier_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    persisted_error(format!("remediation identifier is invalid: {error}"))
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
