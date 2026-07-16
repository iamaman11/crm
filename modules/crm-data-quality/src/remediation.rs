use crate::{
    EvaluatedPartyKind, PartyFinding, PartyQualityInput, derived_identity::derived_id,
};
use crm_module_sdk::{ErrorCategory, IdempotencyKey, RecordId, SdkError, TenantId};

const ATTEMPT_ID_DOMAIN: &[u8] = b"crm.data-quality.party-display-name-remediation-attempt/v1";
const TARGET_IDEMPOTENCY_DOMAIN: &[u8] =
    b"crm.data-quality.party-display-name-remediation-target-idempotency/v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyDisplayNameRemediationIdentity {
    attempt_id: String,
    target_idempotency_key: IdempotencyKey,
}

impl PartyDisplayNameRemediationIdentity {
    pub fn derive(
        tenant_id: &TenantId,
        caller_idempotency_key: &IdempotencyKey,
        finding: &PartyFinding,
        expected_finding_version: i64,
        expected_observation_id: &str,
        expected_party_version: i64,
        display_name: &str,
    ) -> Result<Self, SdkError> {
        validate_request(
            finding,
            expected_finding_version,
            expected_observation_id,
            expected_party_version,
            display_name,
        )?;
        let finding_version = expected_finding_version.to_string();
        let party_version = expected_party_version.to_string();
        let attempt_id = derived_id(
            "dq-remediation-attempt",
            ATTEMPT_ID_DOMAIN,
            &[
                tenant_id.as_str().as_bytes(),
                caller_idempotency_key.as_str().as_bytes(),
                finding.finding_id().as_bytes(),
                expected_observation_id.as_bytes(),
                finding_version.as_bytes(),
                party_version.as_bytes(),
                display_name.as_bytes(),
            ],
        );
        let target = derived_id(
            "dq-remediation-target",
            TARGET_IDEMPOTENCY_DOMAIN,
            &[tenant_id.as_str().as_bytes(), attempt_id.as_bytes()],
        );
        Ok(Self {
            attempt_id,
            target_idempotency_key: IdempotencyKey::try_new(target).map_err(configuration_error)?,
        })
    }

    pub fn attempt_id(&self) -> &str {
        &self.attempt_id
    }

    pub fn target_idempotency_key(&self) -> &IdempotencyKey {
        &self.target_idempotency_key
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyDisplayNameRemediationAttempt {
    tenant_id: TenantId,
    attempt_id: String,
    finding_id: String,
    observation_id: String,
    party_id: RecordId,
    expected_party_version: i64,
    requested_display_name: String,
    target_idempotency_key: IdempotencyKey,
    updated_party_version: i64,
    completed_at: i64,
}

impl PartyDisplayNameRemediationAttempt {
    pub fn complete(
        tenant_id: TenantId,
        identity: PartyDisplayNameRemediationIdentity,
        finding: &PartyFinding,
        expected_observation_id: &str,
        expected_party_version: i64,
        display_name: impl Into<String>,
        updated_party_version: i64,
        completed_at: i64,
    ) -> Result<Self, SdkError> {
        let display_name = display_name.into();
        if tenant_id != *finding.tenant_id()
            || expected_observation_id != finding.current_observation_id()
            || expected_party_version != finding.evaluated_party_resource_version()
            || updated_party_version <= expected_party_version
            || completed_at < finding.updated_at()
        {
            return Err(invalid("completed remediation evidence is inconsistent"));
        }
        validate_display_name(&display_name)?;
        Ok(Self {
            tenant_id,
            attempt_id: identity.attempt_id,
            finding_id: finding.finding_id().to_owned(),
            observation_id: expected_observation_id.to_owned(),
            party_id: finding.party_id().clone(),
            expected_party_version,
            requested_display_name: display_name,
            target_idempotency_key: identity.target_idempotency_key,
            updated_party_version,
            completed_at,
        })
    }

    pub(crate) fn restore(state: PartyDisplayNameRemediationAttemptRestore) -> Result<Self, SdkError> {
        validate_display_name(&state.requested_display_name)?;
        if state.attempt_id.is_empty()
            || state.finding_id.is_empty()
            || state.observation_id.is_empty()
            || state.expected_party_version <= 0
            || state.updated_party_version <= state.expected_party_version
            || state.completed_at < 0
        {
            return Err(invalid("persisted remediation attempt invariants are invalid"));
        }
        Ok(Self {
            tenant_id: state.tenant_id,
            attempt_id: state.attempt_id,
            finding_id: state.finding_id,
            observation_id: state.observation_id,
            party_id: state.party_id,
            expected_party_version: state.expected_party_version,
            requested_display_name: state.requested_display_name,
            target_idempotency_key: state.target_idempotency_key,
            updated_party_version: state.updated_party_version,
            completed_at: state.completed_at,
        })
    }

    pub fn tenant_id(&self) -> &TenantId { &self.tenant_id }
    pub fn attempt_id(&self) -> &str { &self.attempt_id }
    pub fn finding_id(&self) -> &str { &self.finding_id }
    pub fn observation_id(&self) -> &str { &self.observation_id }
    pub fn party_id(&self) -> &RecordId { &self.party_id }
    pub const fn expected_party_version(&self) -> i64 { self.expected_party_version }
    pub fn requested_display_name(&self) -> &str { &self.requested_display_name }
    pub fn target_idempotency_key(&self) -> &IdempotencyKey { &self.target_idempotency_key }
    pub const fn updated_party_version(&self) -> i64 { self.updated_party_version }
    pub const fn completed_at(&self) -> i64 { self.completed_at }
}

pub(crate) struct PartyDisplayNameRemediationAttemptRestore {
    pub tenant_id: TenantId,
    pub attempt_id: String,
    pub finding_id: String,
    pub observation_id: String,
    pub party_id: RecordId,
    pub expected_party_version: i64,
    pub requested_display_name: String,
    pub target_idempotency_key: IdempotencyKey,
    pub updated_party_version: i64,
    pub completed_at: i64,
}

fn validate_request(
    finding: &PartyFinding,
    expected_finding_version: i64,
    expected_observation_id: &str,
    expected_party_version: i64,
    display_name: &str,
) -> Result<(), SdkError> {
    if expected_finding_version <= 0
        || expected_observation_id != finding.current_observation_id()
        || expected_party_version != finding.evaluated_party_resource_version()
    {
        return Err(SdkError::new(
            "DATA_QUALITY_REMEDIATION_EVIDENCE_CONFLICT",
            ErrorCategory::Conflict,
            false,
            "The Data Quality finding or Party evidence changed before remediation.",
        ));
    }
    validate_display_name(display_name)
}

fn validate_display_name(value: &str) -> Result<(), SdkError> {
    PartyQualityInput::try_new(EvaluatedPartyKind::Person, value.to_owned())
        .map(|_| ())
        .map_err(|error| invalid(&format!("requested display name is invalid: {error}")))
}

fn configuration_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    SdkError::new(
        "DATA_QUALITY_REMEDIATION_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Data Quality remediation boundary is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}

fn invalid(reference: &str) -> SdkError {
    SdkError::new(
        "DATA_QUALITY_REMEDIATION_ATTEMPT_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "The Party display-name remediation attempt is invalid.",
    )
    .with_internal_reference(reference)
}
