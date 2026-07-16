use crate::{
    PartyQualityRule, PartyRuleOutcome, QualitySeverity, RuleKey, derived_identity::derived_id,
};
use crm_module_sdk::{ActorId, ErrorCategory, RecordId, SdkError, TenantId};
use serde::{Deserialize, Serialize};

const FINDING_ID_DOMAIN: &[u8] = b"crm.data-quality.finding/v1";
const FINDING_OBSERVATION_ID_DOMAIN: &[u8] = b"crm.data-quality.finding-observation/v1";
const TARGET_OWNER_MODULE_ID: &[u8] = b"crm.parties";
const TARGET_RESOURCE_TYPE: &[u8] = b"parties.party";
const MAX_REASON_CODE_BYTES: usize = 120;
const MAX_WAIVER_REASON_BYTES: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartyFindingStatus {
    Open,
    Acknowledged,
    Waived,
    Remediated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyFindingObservation {
    tenant_id: TenantId,
    observation_id: String,
    finding_id: String,
    party_id: RecordId,
    party_resource_version: i64,
    rule_set_version_id: String,
    rule_key: RuleKey,
    reason_code: String,
    observed_at: i64,
}

impl PartyFindingObservation {
    pub fn observe_failure(
        tenant_id: TenantId,
        rule: &PartyQualityRule,
        outcome: &PartyRuleOutcome,
    ) -> Result<Self, SdkError> {
        if outcome.passed()
            || rule.rule_key() != outcome.rule_key()
            || outcome.party_resource_version() <= 0
            || outcome.evaluated_at() < 0
        {
            return Err(invalid_observation(
                "only a matching failed rule outcome can create an observation",
            ));
        }
        let reason_code = canonical_reason_code(outcome.reason_code())?;
        let finding_id = finding_id(
            &tenant_id,
            outcome.party_id(),
            outcome.rule_set_version_id(),
            outcome.rule_key(),
        );
        let observation_id = observation_id(&finding_id, outcome.party_resource_version());
        Ok(Self {
            tenant_id,
            observation_id,
            finding_id,
            party_id: outcome.party_id().clone(),
            party_resource_version: outcome.party_resource_version(),
            rule_set_version_id: outcome.rule_set_version_id().to_owned(),
            rule_key: outcome.rule_key().clone(),
            reason_code,
            observed_at: outcome.evaluated_at(),
        })
    }

    pub(crate) fn restore(state: PartyFindingObservationRestore) -> Result<Self, SdkError> {
        if state.party_resource_version <= 0 || state.observed_at < 0 {
            return Err(invalid_observation(
                "persisted observation version or timestamp is invalid",
            ));
        }
        let reason_code = canonical_reason_code(&state.reason_code)?;
        if reason_code == "DATA_QUALITY_RULE_PASSED" {
            return Err(invalid_observation(
                "a passing reason code cannot create a finding observation",
            ));
        }
        let expected_finding_id = finding_id(
            &state.tenant_id,
            &state.party_id,
            &state.rule_set_version_id,
            &state.rule_key,
        );
        let expected_observation_id =
            observation_id(&expected_finding_id, state.party_resource_version);
        if state.finding_id != expected_finding_id
            || state.observation_id != expected_observation_id
        {
            return Err(invalid_observation(
                "persisted finding observation identity is invalid",
            ));
        }
        Ok(Self {
            tenant_id: state.tenant_id,
            observation_id: state.observation_id,
            finding_id: state.finding_id,
            party_id: state.party_id,
            party_resource_version: state.party_resource_version,
            rule_set_version_id: state.rule_set_version_id,
            rule_key: state.rule_key,
            reason_code,
            observed_at: state.observed_at,
        })
    }

    pub fn tenant_id(&self) -> &TenantId {
        &self.tenant_id
    }

    pub fn observation_id(&self) -> &str {
        &self.observation_id
    }

    pub fn finding_id(&self) -> &str {
        &self.finding_id
    }

    pub fn party_id(&self) -> &RecordId {
        &self.party_id
    }

    pub const fn party_resource_version(&self) -> i64 {
        self.party_resource_version
    }

    pub fn rule_set_version_id(&self) -> &str {
        &self.rule_set_version_id
    }

    pub fn rule_key(&self) -> &RuleKey {
        &self.rule_key
    }

    pub fn reason_code(&self) -> &str {
        &self.reason_code
    }

    pub const fn observed_at(&self) -> i64 {
        self.observed_at
    }
}

pub(crate) struct PartyFindingObservationRestore {
    pub tenant_id: TenantId,
    pub observation_id: String,
    pub finding_id: String,
    pub party_id: RecordId,
    pub party_resource_version: i64,
    pub rule_set_version_id: String,
    pub rule_key: RuleKey,
    pub reason_code: String,
    pub observed_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyFinding {
    tenant_id: TenantId,
    finding_id: String,
    party_id: RecordId,
    rule_set_version_id: String,
    rule_key: RuleKey,
    severity: QualitySeverity,
    status: PartyFindingStatus,
    current_observation_id: String,
    evaluated_party_resource_version: i64,
    assigned_actor_id: Option<ActorId>,
    waiver_reason: Option<String>,
    remediated_by_rule_outcome_id: Option<String>,
    created_at: i64,
    updated_at: i64,
}

impl PartyFinding {
    pub fn open(
        rule: &PartyQualityRule,
        observation: &PartyFindingObservation,
    ) -> Result<Self, SdkError> {
        if rule.rule_key() != observation.rule_key() {
            return Err(invalid_finding(
                "finding rule differs from its first observation",
            ));
        }
        Ok(Self {
            tenant_id: observation.tenant_id().clone(),
            finding_id: observation.finding_id().to_owned(),
            party_id: observation.party_id().clone(),
            rule_set_version_id: observation.rule_set_version_id().to_owned(),
            rule_key: observation.rule_key().clone(),
            severity: rule.severity(),
            status: PartyFindingStatus::Open,
            current_observation_id: observation.observation_id().to_owned(),
            evaluated_party_resource_version: observation.party_resource_version(),
            assigned_actor_id: None,
            waiver_reason: None,
            remediated_by_rule_outcome_id: None,
            created_at: observation.observed_at(),
            updated_at: observation.observed_at(),
        })
    }

    pub fn apply_failed_observation(
        &self,
        observation: &PartyFindingObservation,
    ) -> Result<Self, SdkError> {
        self.ensure_observation_scope(observation)?;
        match observation
            .party_resource_version()
            .cmp(&self.evaluated_party_resource_version)
        {
            std::cmp::Ordering::Less => Err(invalid_finding(
                "an older failed observation cannot regress finding state",
            )),
            std::cmp::Ordering::Equal => {
                if self.status == PartyFindingStatus::Remediated
                    || self.current_observation_id != observation.observation_id()
                {
                    return Err(invalid_finding(
                        "equal-version failed evidence conflicts with finding state",
                    ));
                }
                Ok(self.clone())
            }
            std::cmp::Ordering::Greater => {
                let mut updated = self.clone();
                updated.status = PartyFindingStatus::Open;
                updated.current_observation_id = observation.observation_id().to_owned();
                updated.evaluated_party_resource_version = observation.party_resource_version();
                updated.waiver_reason = None;
                updated.remediated_by_rule_outcome_id = None;
                updated.updated_at = observation.observed_at();
                Ok(updated)
            }
        }
    }

    pub fn apply_passing_outcome(&self, outcome: &PartyRuleOutcome) -> Result<Self, SdkError> {
        self.ensure_outcome_scope(outcome)?;
        if !outcome.passed() {
            return Err(invalid_finding(
                "only a passing outcome can remediate a finding",
            ));
        }
        match outcome
            .party_resource_version()
            .cmp(&self.evaluated_party_resource_version)
        {
            std::cmp::Ordering::Less => Err(invalid_finding(
                "an older passing outcome cannot regress finding state",
            )),
            std::cmp::Ordering::Equal => {
                if self.status == PartyFindingStatus::Remediated
                    && self.remediated_by_rule_outcome_id.as_deref()
                        == Some(outcome.outcome_id())
                {
                    Ok(self.clone())
                } else {
                    Err(invalid_finding(
                        "equal-version passing evidence conflicts with finding state",
                    ))
                }
            }
            std::cmp::Ordering::Greater => {
                let mut updated = self.clone();
                updated.status = PartyFindingStatus::Remediated;
                updated.evaluated_party_resource_version = outcome.party_resource_version();
                updated.waiver_reason = None;
                updated.remediated_by_rule_outcome_id = Some(outcome.outcome_id().to_owned());
                updated.updated_at = outcome.evaluated_at();
                Ok(updated)
            }
        }
    }

    pub(crate) fn restore(state: PartyFindingRestore) -> Result<Self, SdkError> {
        if state.rule_set_version_id.is_empty()
            || state.current_observation_id.is_empty()
            || state.evaluated_party_resource_version <= 0
            || state.created_at < 0
            || state.updated_at < state.created_at
        {
            return Err(invalid_finding(
                "persisted finding invariants are invalid",
            ));
        }
        let expected_finding_id = finding_id(
            &state.tenant_id,
            &state.party_id,
            &state.rule_set_version_id,
            &state.rule_key,
        );
        if state.finding_id != expected_finding_id {
            return Err(invalid_finding("persisted finding identity is invalid"));
        }
        let waiver_reason = state
            .waiver_reason
            .map(canonical_waiver_reason)
            .transpose()?;
        let lifecycle_valid = match state.status {
            PartyFindingStatus::Open | PartyFindingStatus::Acknowledged => {
                waiver_reason.is_none() && state.remediated_by_rule_outcome_id.is_none()
            }
            PartyFindingStatus::Waived => {
                waiver_reason.is_some() && state.remediated_by_rule_outcome_id.is_none()
            }
            PartyFindingStatus::Remediated => {
                waiver_reason.is_none()
                    && state
                        .remediated_by_rule_outcome_id
                        .as_ref()
                        .is_some_and(|value| !value.is_empty())
            }
        };
        if !lifecycle_valid {
            return Err(invalid_finding(
                "persisted finding lifecycle evidence is invalid",
            ));
        }
        Ok(Self {
            tenant_id: state.tenant_id,
            finding_id: state.finding_id,
            party_id: state.party_id,
            rule_set_version_id: state.rule_set_version_id,
            rule_key: state.rule_key,
            severity: state.severity,
            status: state.status,
            current_observation_id: state.current_observation_id,
            evaluated_party_resource_version: state.evaluated_party_resource_version,
            assigned_actor_id: state.assigned_actor_id,
            waiver_reason,
            remediated_by_rule_outcome_id: state.remediated_by_rule_outcome_id,
            created_at: state.created_at,
            updated_at: state.updated_at,
        })
    }

    pub fn tenant_id(&self) -> &TenantId {
        &self.tenant_id
    }

    pub fn finding_id(&self) -> &str {
        &self.finding_id
    }

    pub fn party_id(&self) -> &RecordId {
        &self.party_id
    }

    pub fn rule_set_version_id(&self) -> &str {
        &self.rule_set_version_id
    }

    pub fn rule_key(&self) -> &RuleKey {
        &self.rule_key
    }

    pub const fn severity(&self) -> QualitySeverity {
        self.severity
    }

    pub const fn status(&self) -> PartyFindingStatus {
        self.status
    }

    pub fn current_observation_id(&self) -> &str {
        &self.current_observation_id
    }

    pub const fn evaluated_party_resource_version(&self) -> i64 {
        self.evaluated_party_resource_version
    }

    pub fn assigned_actor_id(&self) -> Option<&ActorId> {
        self.assigned_actor_id.as_ref()
    }

    pub fn waiver_reason(&self) -> Option<&str> {
        self.waiver_reason.as_deref()
    }

    pub fn remediated_by_rule_outcome_id(&self) -> Option<&str> {
        self.remediated_by_rule_outcome_id.as_deref()
    }

    pub const fn created_at(&self) -> i64 {
        self.created_at
    }

    pub const fn updated_at(&self) -> i64 {
        self.updated_at
    }

    fn ensure_observation_scope(
        &self,
        observation: &PartyFindingObservation,
    ) -> Result<(), SdkError> {
        if observation.tenant_id() != &self.tenant_id
            || observation.finding_id() != self.finding_id
            || observation.party_id() != &self.party_id
            || observation.rule_set_version_id() != self.rule_set_version_id
            || observation.rule_key() != &self.rule_key
        {
            return Err(invalid_finding(
                "failed observation does not belong to this finding",
            ));
        }
        Ok(())
    }

    fn ensure_outcome_scope(&self, outcome: &PartyRuleOutcome) -> Result<(), SdkError> {
        if outcome.party_id() != &self.party_id
            || outcome.rule_set_version_id() != self.rule_set_version_id
            || outcome.rule_key() != &self.rule_key
        {
            return Err(invalid_finding(
                "rule outcome does not belong to this finding",
            ));
        }
        Ok(())
    }
}

pub(crate) struct PartyFindingRestore {
    pub tenant_id: TenantId,
    pub finding_id: String,
    pub party_id: RecordId,
    pub rule_set_version_id: String,
    pub rule_key: RuleKey,
    pub severity: QualitySeverity,
    pub status: PartyFindingStatus,
    pub current_observation_id: String,
    pub evaluated_party_resource_version: i64,
    pub assigned_actor_id: Option<ActorId>,
    pub waiver_reason: Option<String>,
    pub remediated_by_rule_outcome_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

fn finding_id(
    tenant_id: &TenantId,
    party_id: &RecordId,
    rule_set_version_id: &str,
    rule_key: &RuleKey,
) -> String {
    derived_id(
        "dq-finding",
        FINDING_ID_DOMAIN,
        &[
            tenant_id.as_str().as_bytes(),
            TARGET_OWNER_MODULE_ID,
            TARGET_RESOURCE_TYPE,
            party_id.as_str().as_bytes(),
            rule_set_version_id.as_bytes(),
            rule_key.as_str().as_bytes(),
        ],
    )
}

fn observation_id(finding_id: &str, party_resource_version: i64) -> String {
    let version = party_resource_version.to_string();
    derived_id(
        "dq-finding-observation",
        FINDING_OBSERVATION_ID_DOMAIN,
        &[finding_id.as_bytes(), version.as_bytes()],
    )
}

fn canonical_reason_code(value: &str) -> Result<String, SdkError> {
    if value.is_empty()
        || value.len() > MAX_REASON_CODE_BYTES
        || !value.is_ascii()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(invalid_observation(
            "finding observation reason code is invalid",
        ));
    }
    Ok(value.to_owned())
}

fn canonical_waiver_reason(value: String) -> Result<String, SdkError> {
    if value.is_empty()
        || value.len() > MAX_WAIVER_REASON_BYTES
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(invalid_finding("finding waiver reason is invalid"));
    }
    Ok(value)
}

fn invalid_observation(reference: &str) -> SdkError {
    SdkError::new(
        "DATA_QUALITY_FINDING_OBSERVATION_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "The Party finding observation is invalid.",
    )
    .with_internal_reference(reference)
}

fn invalid_finding(reference: &str) -> SdkError {
    SdkError::new(
        "DATA_QUALITY_FINDING_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "The Party data-quality finding is invalid.",
    )
    .with_internal_reference(reference)
}
