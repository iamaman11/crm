use crate::canonicalization::persisted_state_json as execution_state_json;
use crm_module_sdk::{ErrorCategory, IdempotencyKey, SdkError};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

pub const OWNER_ACTION_DISPATCH_COORDINATE: &str =
    "customer_privacy.owner_action.dispatch@1.0.0";
pub const OWNER_OUTCOME_RECORD_COORDINATE: &str =
    "customer_privacy.owner_outcome.record@1.0.0";
pub const OWNER_ACTION_ATTEMPT_STATE_SCHEMA_ID: &str =
    "crm.customer-privacy.owner_action_attempt.state";
pub const OWNER_ACTION_ATTEMPT_STATE_SCHEMA_VERSION: &str = "1.0.0";
pub const OWNER_ACTION_ATTEMPT_STATE_MAXIMUM_BYTES: u64 = 32 * 1024;
pub const OWNER_ACTION_ATTEMPT_STATE_RETENTION_POLICY_ID: &str =
    "crm.customer_privacy.owner_action_attempt";
pub const OWNER_ACTION_OUTCOME_STATE_SCHEMA_ID: &str =
    "crm.customer-privacy.owner_action_outcome.state";
pub const OWNER_ACTION_OUTCOME_STATE_SCHEMA_VERSION: &str = "1.0.0";
pub const OWNER_ACTION_OUTCOME_STATE_MAXIMUM_BYTES: u64 = 32 * 1024;
pub const OWNER_ACTION_OUTCOME_STATE_RETENTION_POLICY_ID: &str =
    "crm.customer_privacy.owner_action_outcome";

const ATTEMPT_DESCRIPTOR: &[u8] = b"crm.customer-privacy.owner_action_attempt.state/v1:attempt_id,tenant_id,privacy_case_id,action_plan_id,action_plan_digest,retention_decision_id,retention_decision_digest,item_sequence,attempt_generation,item_digest,owner_module_id,owner_capability_id,owner_capability_version,resource_type,resource_id,resource_version,action_code,decision_reason,target_idempotency_key,planned_at_unix_nanos,attempt_digest";
const OUTCOME_DESCRIPTOR: &[u8] = b"crm.customer-privacy.owner_action_outcome.state/v1:outcome_id,tenant_id,privacy_case_id,action_plan_id,retention_decision_id,item_sequence,attempt_generation,attempt_id,attempt_digest,owner_module_id,action_code,status,safe_failure_code,recorded_at_unix_nanos,outcome_digest";
const ATTEMPT_ID_PREFIX: &str = "privacy-owner-attempt-";
const OUTCOME_ID_PREFIX: &str = "privacy-owner-outcome-";
const IDEMPOTENCY_PREFIX: &str = "privacy-owner-action-";
const MAX_SAFE_CODE_BYTES: usize = 96;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyOwnerOutcomeStatus {
    Succeeded,
    Retained,
    BlockedByHold,
    BlockedByRetention,
    FailedRetryable,
    FailedTerminal,
}

impl PrivacyOwnerOutcomeStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Retained => "retained",
            Self::BlockedByHold => "blocked_by_hold",
            Self::BlockedByRetention => "blocked_by_retention",
            Self::FailedRetryable => "failed_retryable",
            Self::FailedTerminal => "failed_terminal",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyOwnerActionAttempt {
    attempt_id: RecordId,
    tenant_id: TenantId,
    privacy_case_id: RecordId,
    action_plan_id: RecordId,
    action_plan_digest: [u8; 32],
    retention_decision_id: RecordId,
    retention_decision_digest: [u8; 32],
    item_sequence: u32,
    attempt_generation: u32,
    item_digest: [u8; 32],
    owner_module_id: ModuleId,
    owner_capability_id: String,
    owner_capability_version: String,
    resource_type: String,
    resource_id: RecordId,
    resource_version: u64,
    action_code: String,
    decision_reason: String,
    target_idempotency_key: IdempotencyKey,
    planned_at_unix_nanos: i64,
    digest: [u8; 32],
}

#[allow(clippy::too_many_arguments)]
impl PrivacyOwnerActionAttempt {
    pub fn build(
        tenant_id: TenantId,
        privacy_case_id: RecordId,
        action_plan_id: RecordId,
        action_plan_digest: [u8; 32],
        retention_decision_id: RecordId,
        retention_decision_digest: [u8; 32],
        item: &PrivacyRetentionDecisionItem,
        attempt_generation: u32,
        planned_at_unix_nanos: i64,
    ) -> Result<Self, SdkError> {
        if planned_at_unix_nanos <= 0 || attempt_generation > 100 {
            return Err(execution_invalid("attempt time must be positive"));
        }
        let owner_capability_id = owner_action_capability(item.owner_module_id())?.to_owned();
        let owner_capability_version = "1.0.0".to_owned();
        let action_code = item.final_action().label().to_owned();
        let decision_reason = item.reason().label().to_owned();
        let digest = attempt_digest(
            &tenant_id,
            &privacy_case_id,
            &action_plan_id,
            &action_plan_digest,
            &retention_decision_id,
            &retention_decision_digest,
            item,
            attempt_generation,
            &owner_capability_id,
            &owner_capability_version,
            planned_at_unix_nanos,
        );
        let attempt_id = RecordId::try_new(format!("{ATTEMPT_ID_PREFIX}{}", hex(&digest)))
            .map_err(execution_invalid)?;
        let key_digest = target_key_digest(
            &tenant_id,
            &privacy_case_id,
            &action_plan_id,
            &retention_decision_id,
            item,
            &owner_capability_id,
            &owner_capability_version,
        );
        let target_idempotency_key = IdempotencyKey::try_new(format!(
            "{IDEMPOTENCY_PREFIX}{}",
            hex(&key_digest)
        ))
        .map_err(execution_invalid)?;
        Ok(Self {
            attempt_id,
            tenant_id,
            privacy_case_id,
            action_plan_id,
            action_plan_digest,
            retention_decision_id,
            retention_decision_digest,
            item_sequence: item.sequence(),
            attempt_generation,
            item_digest: *item.digest(),
            owner_module_id: item.owner_module_id().clone(),
            owner_capability_id,
            owner_capability_version,
            resource_type: item.resource_type().to_owned(),
            resource_id: item.resource_id().clone(),
            resource_version: item.resource_version(),
            action_code,
            decision_reason,
            target_idempotency_key,
            planned_at_unix_nanos,
            digest,
        })
    }

    pub fn attempt_id(&self) -> &RecordId { &self.attempt_id }
    pub fn tenant_id(&self) -> &TenantId { &self.tenant_id }
    pub fn privacy_case_id(&self) -> &RecordId { &self.privacy_case_id }
    pub fn action_plan_id(&self) -> &RecordId { &self.action_plan_id }
    pub const fn action_plan_digest(&self) -> &[u8; 32] { &self.action_plan_digest }
    pub fn retention_decision_id(&self) -> &RecordId { &self.retention_decision_id }
    pub const fn retention_decision_digest(&self) -> &[u8; 32] { &self.retention_decision_digest }
    pub const fn item_sequence(&self) -> u32 { self.item_sequence }
    pub const fn attempt_generation(&self) -> u32 { self.attempt_generation }
    pub const fn item_digest(&self) -> &[u8; 32] { &self.item_digest }
    pub fn owner_module_id(&self) -> &ModuleId { &self.owner_module_id }
    pub fn owner_capability_id(&self) -> &str { &self.owner_capability_id }
    pub fn owner_capability_version(&self) -> &str { &self.owner_capability_version }
    pub fn resource_type(&self) -> &str { &self.resource_type }
    pub fn resource_id(&self) -> &RecordId { &self.resource_id }
    pub const fn resource_version(&self) -> u64 { self.resource_version }
    pub fn action_code(&self) -> &str { &self.action_code }
    pub fn decision_reason(&self) -> &str { &self.decision_reason }
    pub fn target_idempotency_key(&self) -> &IdempotencyKey { &self.target_idempotency_key }
    pub const fn planned_at_unix_nanos(&self) -> i64 { self.planned_at_unix_nanos }
    pub const fn digest(&self) -> &[u8; 32] { &self.digest }

    pub fn coordinator_outcome_status(&self) -> Option<PrivacyOwnerOutcomeStatus> {
        match (self.action_code.as_str(), self.decision_reason.as_str()) {
            ("retain", "active_legal_hold") => Some(PrivacyOwnerOutcomeStatus::BlockedByHold),
            ("retain", "mandatory_retention") => {
                Some(PrivacyOwnerOutcomeStatus::BlockedByRetention)
            }
            ("retain", _) => Some(PrivacyOwnerOutcomeStatus::Retained),
            ("no_op_already_compliant", _) => Some(PrivacyOwnerOutcomeStatus::Succeeded),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyOwnerActionOutcome {
    outcome_id: RecordId,
    tenant_id: TenantId,
    privacy_case_id: RecordId,
    action_plan_id: RecordId,
    retention_decision_id: RecordId,
    item_sequence: u32,
    attempt_generation: u32,
    attempt_id: RecordId,
    attempt_digest: [u8; 32],
    owner_module_id: ModuleId,
    action_code: String,
    status: PrivacyOwnerOutcomeStatus,
    safe_failure_code: Option<String>,
    recorded_at_unix_nanos: i64,
    digest: [u8; 32],
}

impl PrivacyOwnerActionOutcome {
    pub fn record(
        attempt: &PrivacyOwnerActionAttempt,
        status: PrivacyOwnerOutcomeStatus,
        safe_failure_code: Option<String>,
        recorded_at_unix_nanos: i64,
    ) -> Result<Self, SdkError> {
        if recorded_at_unix_nanos < attempt.planned_at_unix_nanos() {
            return Err(execution_invalid("outcome predates its deterministic attempt"));
        }
        validate_safe_failure(status, safe_failure_code.as_deref())?;
        let outcome_id_digest = outcome_id_digest(attempt.attempt_id());
        let outcome_id = RecordId::try_new(format!("{OUTCOME_ID_PREFIX}{}", hex(&outcome_id_digest)))
            .map_err(execution_invalid)?;
        let digest = outcome_digest(
            attempt,
            status,
            safe_failure_code.as_deref(),
            recorded_at_unix_nanos,
        );
        Ok(Self {
            outcome_id,
            tenant_id: attempt.tenant_id.clone(),
            privacy_case_id: attempt.privacy_case_id.clone(),
            action_plan_id: attempt.action_plan_id.clone(),
            retention_decision_id: attempt.retention_decision_id.clone(),
            item_sequence: attempt.item_sequence,
            attempt_generation: attempt.attempt_generation,
            attempt_id: attempt.attempt_id.clone(),
            attempt_digest: attempt.digest,
            owner_module_id: attempt.owner_module_id.clone(),
            action_code: attempt.action_code.clone(),
            status,
            safe_failure_code,
            recorded_at_unix_nanos,
            digest,
        })
    }

    pub fn outcome_id(&self) -> &RecordId { &self.outcome_id }
    pub fn tenant_id(&self) -> &TenantId { &self.tenant_id }
    pub fn privacy_case_id(&self) -> &RecordId { &self.privacy_case_id }
    pub fn action_plan_id(&self) -> &RecordId { &self.action_plan_id }
    pub fn retention_decision_id(&self) -> &RecordId { &self.retention_decision_id }
    pub const fn item_sequence(&self) -> u32 { self.item_sequence }
    pub const fn attempt_generation(&self) -> u32 { self.attempt_generation }
    pub fn attempt_id(&self) -> &RecordId { &self.attempt_id }
    pub const fn attempt_digest(&self) -> &[u8; 32] { &self.attempt_digest }
    pub fn owner_module_id(&self) -> &ModuleId { &self.owner_module_id }
    pub fn action_code(&self) -> &str { &self.action_code }
    pub const fn status(&self) -> PrivacyOwnerOutcomeStatus { self.status }
    pub fn safe_failure_code(&self) -> Option<&str> { self.safe_failure_code.as_deref() }
    pub const fn recorded_at_unix_nanos(&self) -> i64 { self.recorded_at_unix_nanos }
    pub const fn digest(&self) -> &[u8; 32] { &self.digest }
}

pub fn owner_action_attempt_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(ATTEMPT_DESCRIPTOR).into()
}

pub fn owner_action_outcome_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(OUTCOME_DESCRIPTOR).into()
}

pub fn encode_owner_action_attempt_state(
    attempt: &PrivacyOwnerActionAttempt,
) -> Result<Vec<u8>, SdkError> {
    let bytes = execution_state_json::to_vec(&AttemptStateV1::from(attempt))
        .map_err(execution_invalid)?;
    validate_execution_state_size(&bytes, OWNER_ACTION_ATTEMPT_STATE_MAXIMUM_BYTES, "attempt")?;
    Ok(bytes)
}

pub fn decode_owner_action_attempt_state(
    bytes: &[u8],
) -> Result<PrivacyOwnerActionAttempt, SdkError> {
    validate_execution_state_size(bytes, OWNER_ACTION_ATTEMPT_STATE_MAXIMUM_BYTES, "attempt")?;
    let state: AttemptStateV1 =
        execution_state_json::from_slice(bytes).map_err(execution_invalid)?;
    let attempt = state.into_domain()?;
    if encode_owner_action_attempt_state(&attempt)? != bytes {
        return Err(execution_invalid(
            "persisted attempt is not the strict canonical v1 encoding",
        ));
    }
    Ok(attempt)
}

pub fn encode_owner_action_outcome_state(
    outcome: &PrivacyOwnerActionOutcome,
) -> Result<Vec<u8>, SdkError> {
    let bytes = execution_state_json::to_vec(&OutcomeStateV1::from(outcome))
        .map_err(execution_invalid)?;
    validate_execution_state_size(&bytes, OWNER_ACTION_OUTCOME_STATE_MAXIMUM_BYTES, "outcome")?;
    Ok(bytes)
}

pub fn decode_owner_action_outcome_state(
    bytes: &[u8],
) -> Result<PrivacyOwnerActionOutcome, SdkError> {
    validate_execution_state_size(bytes, OWNER_ACTION_OUTCOME_STATE_MAXIMUM_BYTES, "outcome")?;
    let state: OutcomeStateV1 =
        execution_state_json::from_slice(bytes).map_err(execution_invalid)?;
    let outcome = state.into_domain()?;
    if encode_owner_action_outcome_state(&outcome)? != bytes {
        return Err(execution_invalid(
            "persisted outcome is not the strict canonical v1 encoding",
        ));
    }
    Ok(outcome)
}

fn owner_action_capability(owner: &ModuleId) -> Result<&'static str, SdkError> {
    match owner.as_str() {
        "crm.parties" => Ok("parties.privacy.action.apply"),
        "crm.customer-accounts" => Ok("customer_accounts.privacy.action.apply"),
        "crm.contact-points" => Ok("contact_points.privacy.action.apply"),
        "crm.party-relationships" => Ok("party_relationships.privacy.action.apply"),
        "crm.consents" => Ok("consents.privacy.action.apply"),
        "crm.identity-resolution" => Ok("identity_resolution.privacy.action.apply"),
        "crm.customer-data-operations" => Ok("customer_data.privacy.action.apply"),
        "crm.data-quality" => Ok("data_quality.privacy.action.apply"),
        "crm.customer-enrichment" => Ok("customer_enrichment.privacy.action.apply"),
        _ => Err(execution_invalid("owner module has no frozen privacy-action coordinate")),
    }
}

#[allow(clippy::too_many_arguments)]
fn attempt_digest(
    tenant_id: &TenantId,
    privacy_case_id: &RecordId,
    action_plan_id: &RecordId,
    action_plan_digest: &[u8; 32],
    retention_decision_id: &RecordId,
    retention_decision_digest: &[u8; 32],
    item: &PrivacyRetentionDecisionItem,
    attempt_generation: u32,
    capability_id: &str,
    capability_version: &str,
    planned_at_unix_nanos: i64,
) -> [u8; 32] {
    let sequence = item.sequence().to_be_bytes();
    let generation = attempt_generation.to_be_bytes();
    let resource_version = item.resource_version().to_be_bytes();
    let planned = planned_at_unix_nanos.to_be_bytes();
    framed_digest(&[
        b"crm.customer-privacy.owner-action-attempt/v1",
        tenant_id.as_str().as_bytes(),
        privacy_case_id.as_str().as_bytes(),
        action_plan_id.as_str().as_bytes(),
        action_plan_digest,
        retention_decision_id.as_str().as_bytes(),
        retention_decision_digest,
        &sequence,
        &generation,
        item.digest(),
        item.owner_module_id().as_str().as_bytes(),
        capability_id.as_bytes(),
        capability_version.as_bytes(),
        item.resource_type().as_bytes(),
        item.resource_id().as_str().as_bytes(),
        &resource_version,
        item.final_action().label().as_bytes(),
        item.reason().label().as_bytes(),
        &planned,
    ])
}

fn target_key_digest(
    tenant_id: &TenantId,
    privacy_case_id: &RecordId,
    action_plan_id: &RecordId,
    retention_decision_id: &RecordId,
    item: &PrivacyRetentionDecisionItem,
    capability_id: &str,
    capability_version: &str,
) -> [u8; 32] {
    let sequence = item.sequence().to_be_bytes();
    framed_digest(&[
        b"crm.customer-privacy.owner-target-idempotency/v1",
        tenant_id.as_str().as_bytes(),
        privacy_case_id.as_str().as_bytes(),
        action_plan_id.as_str().as_bytes(),
        retention_decision_id.as_str().as_bytes(),
        &sequence,
        item.digest(),
        item.owner_module_id().as_str().as_bytes(),
        capability_id.as_bytes(),
        capability_version.as_bytes(),
    ])
}

fn rehydrated_attempt_digest(attempt: &PrivacyOwnerActionAttempt) -> [u8; 32] {
    let sequence = attempt.item_sequence.to_be_bytes();
    let generation = attempt.attempt_generation.to_be_bytes();
    let resource_version = attempt.resource_version.to_be_bytes();
    let planned = attempt.planned_at_unix_nanos.to_be_bytes();
    framed_digest(&[
        b"crm.customer-privacy.owner-action-attempt/v1",
        attempt.tenant_id.as_str().as_bytes(),
        attempt.privacy_case_id.as_str().as_bytes(),
        attempt.action_plan_id.as_str().as_bytes(),
        &attempt.action_plan_digest,
        attempt.retention_decision_id.as_str().as_bytes(),
        &attempt.retention_decision_digest,
        &sequence,
        &generation,
        &attempt.item_digest,
        attempt.owner_module_id.as_str().as_bytes(),
        attempt.owner_capability_id.as_bytes(),
        attempt.owner_capability_version.as_bytes(),
        attempt.resource_type.as_bytes(),
        attempt.resource_id.as_str().as_bytes(),
        &resource_version,
        attempt.action_code.as_bytes(),
        attempt.decision_reason.as_bytes(),
        &planned,
    ])
}

fn rehydrated_target_key_digest(attempt: &PrivacyOwnerActionAttempt) -> [u8; 32] {
    let sequence = attempt.item_sequence.to_be_bytes();
    framed_digest(&[
        b"crm.customer-privacy.owner-target-idempotency/v1",
        attempt.tenant_id.as_str().as_bytes(),
        attempt.privacy_case_id.as_str().as_bytes(),
        attempt.action_plan_id.as_str().as_bytes(),
        attempt.retention_decision_id.as_str().as_bytes(),
        &sequence,
        &attempt.item_digest,
        attempt.owner_module_id.as_str().as_bytes(),
        attempt.owner_capability_id.as_bytes(),
        attempt.owner_capability_version.as_bytes(),
    ])
}

fn outcome_id_digest(attempt_id: &RecordId) -> [u8; 32] {
    framed_digest(&[b"crm.customer-privacy.owner-action-outcome-id/v1", attempt_id.as_str().as_bytes()])
}

fn outcome_digest(
    attempt: &PrivacyOwnerActionAttempt,
    status: PrivacyOwnerOutcomeStatus,
    safe_failure_code: Option<&str>,
    recorded_at_unix_nanos: i64,
) -> [u8; 32] {
    let recorded = recorded_at_unix_nanos.to_be_bytes();
    framed_digest(&[
        b"crm.customer-privacy.owner-action-outcome/v1",
        attempt.digest(),
        attempt.attempt_id().as_str().as_bytes(),
        status.label().as_bytes(),
        safe_failure_code.unwrap_or("").as_bytes(),
        &recorded,
    ])
}

fn rehydrated_outcome_digest(outcome: &PrivacyOwnerActionOutcome) -> [u8; 32] {
    let recorded = outcome.recorded_at_unix_nanos.to_be_bytes();
    framed_digest(&[
        b"crm.customer-privacy.owner-action-outcome/v1",
        &outcome.attempt_digest,
        outcome.attempt_id.as_str().as_bytes(),
        outcome.status.label().as_bytes(),
        outcome.safe_failure_code.as_deref().unwrap_or("").as_bytes(),
        &recorded,
    ])
}

fn framed_digest(fields: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    for field in fields {
        hasher.update((field.len() as u64).to_be_bytes());
        hasher.update(field);
    }
    hasher.finalize().into()
}

fn validate_safe_failure(
    status: PrivacyOwnerOutcomeStatus,
    safe_failure_code: Option<&str>,
) -> Result<(), SdkError> {
    let requires = matches!(status, PrivacyOwnerOutcomeStatus::FailedRetryable | PrivacyOwnerOutcomeStatus::FailedTerminal);
    if requires != safe_failure_code.is_some() {
        return Err(execution_invalid("failure status and safe failure code do not match"));
    }
    if let Some(code) = safe_failure_code
        && (code.is_empty()
            || code.len() > MAX_SAFE_CODE_BYTES
            || !code
                .bytes()
                .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_'))
    {
        return Err(execution_invalid("safe failure code is not canonical"));
    }
    Ok(())
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AttemptStateV1 {
    attempt_id: String,
    tenant_id: String,
    privacy_case_id: String,
    action_plan_id: String,
    action_plan_digest: String,
    retention_decision_id: String,
    retention_decision_digest: String,
    item_sequence: u32,
    attempt_generation: u32,
    item_digest: String,
    owner_module_id: String,
    owner_capability_id: String,
    owner_capability_version: String,
    resource_type: String,
    resource_id: String,
    resource_version: String,
    action_code: String,
    decision_reason: String,
    target_idempotency_key: String,
    planned_at_unix_nanos: String,
    attempt_digest: String,
}

impl From<&PrivacyOwnerActionAttempt> for AttemptStateV1 {
    fn from(value: &PrivacyOwnerActionAttempt) -> Self {
        Self {
            attempt_id: value.attempt_id.as_str().to_owned(),
            tenant_id: value.tenant_id.as_str().to_owned(),
            privacy_case_id: value.privacy_case_id.as_str().to_owned(),
            action_plan_id: value.action_plan_id.as_str().to_owned(),
            action_plan_digest: hex(&value.action_plan_digest),
            retention_decision_id: value.retention_decision_id.as_str().to_owned(),
            retention_decision_digest: hex(&value.retention_decision_digest),
            item_sequence: value.item_sequence,
            attempt_generation: value.attempt_generation,
            item_digest: hex(&value.item_digest),
            owner_module_id: value.owner_module_id.as_str().to_owned(),
            owner_capability_id: value.owner_capability_id.clone(),
            owner_capability_version: value.owner_capability_version.clone(),
            resource_type: value.resource_type.clone(),
            resource_id: value.resource_id.as_str().to_owned(),
            resource_version: value.resource_version.to_string(),
            action_code: value.action_code.clone(),
            decision_reason: value.decision_reason.clone(),
            target_idempotency_key: value.target_idempotency_key.as_str().to_owned(),
            planned_at_unix_nanos: value.planned_at_unix_nanos.to_string(),
            attempt_digest: hex(&value.digest),
        }
    }
}

impl AttemptStateV1 {
    fn into_domain(self) -> Result<PrivacyOwnerActionAttempt, SdkError> {
        let attempt = PrivacyOwnerActionAttempt {
            attempt_id: RecordId::try_new(self.attempt_id).map_err(execution_invalid)?,
            tenant_id: TenantId::try_new(self.tenant_id).map_err(execution_invalid)?,
            privacy_case_id: RecordId::try_new(self.privacy_case_id).map_err(execution_invalid)?,
            action_plan_id: RecordId::try_new(self.action_plan_id).map_err(execution_invalid)?,
            action_plan_digest: decode_digest(&self.action_plan_digest)?,
            retention_decision_id: RecordId::try_new(self.retention_decision_id).map_err(execution_invalid)?,
            retention_decision_digest: decode_digest(&self.retention_decision_digest)?,
            item_sequence: self.item_sequence,
            attempt_generation: self.attempt_generation,
            item_digest: decode_digest(&self.item_digest)?,
            owner_module_id: ModuleId::try_new(self.owner_module_id).map_err(execution_invalid)?,
            owner_capability_id: self.owner_capability_id,
            owner_capability_version: self.owner_capability_version,
            resource_type: self.resource_type,
            resource_id: RecordId::try_new(self.resource_id).map_err(execution_invalid)?,
            resource_version: self.resource_version.parse().map_err(execution_invalid)?,
            action_code: self.action_code,
            decision_reason: self.decision_reason,
            target_idempotency_key: IdempotencyKey::try_new(self.target_idempotency_key).map_err(execution_invalid)?,
            planned_at_unix_nanos: self.planned_at_unix_nanos.parse().map_err(execution_invalid)?,
            digest: decode_digest(&self.attempt_digest)?,
        };
        validate_attempt_rehydration(&attempt)?;
        Ok(attempt)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OutcomeStateV1 {
    outcome_id: String,
    tenant_id: String,
    privacy_case_id: String,
    action_plan_id: String,
    retention_decision_id: String,
    item_sequence: u32,
    attempt_generation: u32,
    attempt_id: String,
    attempt_digest: String,
    owner_module_id: String,
    action_code: String,
    status: PrivacyOwnerOutcomeStatus,
    safe_failure_code: Option<String>,
    recorded_at_unix_nanos: String,
    outcome_digest: String,
}

impl From<&PrivacyOwnerActionOutcome> for OutcomeStateV1 {
    fn from(value: &PrivacyOwnerActionOutcome) -> Self {
        Self {
            outcome_id: value.outcome_id.as_str().to_owned(),
            tenant_id: value.tenant_id.as_str().to_owned(),
            privacy_case_id: value.privacy_case_id.as_str().to_owned(),
            action_plan_id: value.action_plan_id.as_str().to_owned(),
            retention_decision_id: value.retention_decision_id.as_str().to_owned(),
            item_sequence: value.item_sequence,
            attempt_generation: value.attempt_generation,
            attempt_id: value.attempt_id.as_str().to_owned(),
            attempt_digest: hex(&value.attempt_digest),
            owner_module_id: value.owner_module_id.as_str().to_owned(),
            action_code: value.action_code.clone(),
            status: value.status,
            safe_failure_code: value.safe_failure_code.clone(),
            recorded_at_unix_nanos: value.recorded_at_unix_nanos.to_string(),
            outcome_digest: hex(&value.digest),
        }
    }
}

impl OutcomeStateV1 {
    fn into_domain(self) -> Result<PrivacyOwnerActionOutcome, SdkError> {
        let outcome = PrivacyOwnerActionOutcome {
            outcome_id: RecordId::try_new(self.outcome_id).map_err(execution_invalid)?,
            tenant_id: TenantId::try_new(self.tenant_id).map_err(execution_invalid)?,
            privacy_case_id: RecordId::try_new(self.privacy_case_id).map_err(execution_invalid)?,
            action_plan_id: RecordId::try_new(self.action_plan_id).map_err(execution_invalid)?,
            retention_decision_id: RecordId::try_new(self.retention_decision_id).map_err(execution_invalid)?,
            item_sequence: self.item_sequence,
            attempt_generation: self.attempt_generation,
            attempt_id: RecordId::try_new(self.attempt_id).map_err(execution_invalid)?,
            attempt_digest: decode_digest(&self.attempt_digest)?,
            owner_module_id: ModuleId::try_new(self.owner_module_id).map_err(execution_invalid)?,
            action_code: self.action_code,
            status: self.status,
            safe_failure_code: self.safe_failure_code,
            recorded_at_unix_nanos: self.recorded_at_unix_nanos.parse().map_err(execution_invalid)?,
            digest: decode_digest(&self.outcome_digest)?,
        };
        validate_safe_failure(outcome.status, outcome.safe_failure_code.as_deref())?;
        let expected_id = outcome_id_digest(&outcome.attempt_id);
        if outcome.outcome_id.as_str() != format!("{OUTCOME_ID_PREFIX}{}", hex(&expected_id)) {
            return Err(execution_invalid("outcome id differs from its deterministic attempt identity"));
        }
        if outcome.item_sequence == 0
            || outcome.attempt_generation > 100
            || outcome.recorded_at_unix_nanos <= 0
            || outcome.digest != rehydrated_outcome_digest(&outcome)
        {
            return Err(execution_invalid(
                "outcome sequence, timestamp or digest is invalid",
            ));
        }
        Ok(outcome)
    }
}

fn validate_attempt_rehydration(attempt: &PrivacyOwnerActionAttempt) -> Result<(), SdkError> {
    if attempt.item_sequence == 0 || attempt.attempt_generation > 100 || attempt.resource_version == 0 || attempt.planned_at_unix_nanos <= 0 {
        return Err(execution_invalid("attempt sequence, version or timestamp is invalid"));
    }
    if owner_action_capability(&attempt.owner_module_id)? != attempt.owner_capability_id
        || attempt.owner_capability_version != "1.0.0"
    {
        return Err(execution_invalid("attempt owner coordinate is not frozen"));
    }
    let expected_key = rehydrated_target_key_digest(attempt);
    let expected_digest = rehydrated_attempt_digest(attempt);
    if attempt.digest != expected_digest
        || attempt.target_idempotency_key.as_str()
            != format!("{IDEMPOTENCY_PREFIX}{}", hex(&expected_key))
        || attempt.attempt_id.as_str()
            != format!("{ATTEMPT_ID_PREFIX}{}", hex(&expected_digest))
    {
        return Err(execution_invalid("attempt deterministic identities do not match"));
    }
    Ok(())
}

fn validate_execution_state_size(bytes: &[u8], maximum: u64, label: &str) -> Result<(), SdkError> {
    if bytes.is_empty() || u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum {
        return Err(execution_invalid(format!(
            "{label} state is empty or exceeds the governed maximum",
        )));
    }
    Ok(())
}

fn decode_digest(value: &str) -> Result<[u8; 32], SdkError> {
    if value.len() != 64 { return Err(execution_invalid("digest must contain 64 lowercase hex characters")); }
    let mut output = [0u8; 32];
    for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
        let text = std::str::from_utf8(chunk).map_err(execution_invalid)?;
        output[index] = u8::from_str_radix(text, 16).map_err(execution_invalid)?;
    }
    if hex(&output) != value { return Err(execution_invalid("digest hex encoding is not canonical")); }
    Ok(output)
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn execution_invalid(reference: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_OWNER_EXECUTION_EVIDENCE_INVALID",
        ErrorCategory::Internal,
        false,
        "Customer Privacy owner-execution evidence is invalid.",
    )
    .with_internal_reference(reference.to_string())
}

#[cfg(test)]
mod execution_tests {
    use super::*;

    fn decision_item(
        owner_module_id: &str,
        final_action: PlannedPrivacyAction,
        reason: RetentionDecisionReason,
    ) -> PrivacyRetentionDecisionItem {
        let mut item = PrivacyRetentionDecisionItem {
            sequence: 1,
            owner_module_id: ModuleId::try_new(owner_module_id).unwrap(),
            resource_type: "party.profile".to_owned(),
            resource_id: RecordId::try_new("party-a").unwrap(),
            resource_version: 7,
            data_class: DataClass::Personal,
            evidence_class: EvidenceClass::DestroyableSubjectData,
            retention_policy_id: RetentionPolicyId::try_new("privacy-policy").unwrap(),
            approved_action: final_action,
            final_action,
            reason,
            legal_hold: None,
            digest: [0; 32],
        };
        item.digest = retention_item_digest(&item);
        item
    }

    fn attempt(
        generation: u32,
        planned_at_unix_nanos: i64,
    ) -> PrivacyOwnerActionAttempt {
        PrivacyOwnerActionAttempt::build(
            TenantId::try_new("tenant-a").unwrap(),
            RecordId::try_new("privacy-case-a").unwrap(),
            RecordId::try_new("action-plan-a").unwrap(),
            [11; 32],
            RecordId::try_new("retention-decision-a").unwrap(),
            [17; 32],
            &decision_item(
                "crm.parties",
                PlannedPrivacyAction::Delete,
                RetentionDecisionReason::ApprovedPrivacyAction,
            ),
            generation,
            planned_at_unix_nanos,
        )
        .unwrap()
    }

    #[test]
    fn descriptors_and_coordinates_are_stable() {
        assert_eq!(
            OWNER_ACTION_DISPATCH_COORDINATE,
            "customer_privacy.owner_action.dispatch@1.0.0"
        );
        assert_eq!(
            OWNER_OUTCOME_RECORD_COORDINATE,
            "customer_privacy.owner_outcome.record@1.0.0"
        );
        assert_ne!(owner_action_attempt_state_descriptor_hash(), [0; 32]);
        assert_ne!(owner_action_outcome_state_descriptor_hash(), [0; 32]);
    }

    #[test]
    fn attempt_identity_is_deterministic_and_target_key_survives_retry_generation() {
        let first = attempt(0, 100);
        let exact_replay = attempt(0, 100);
        let retry = attempt(1, 200);

        assert_eq!(first, exact_replay);
        assert_ne!(first.attempt_id(), retry.attempt_id());
        assert_ne!(first.digest(), retry.digest());
        assert_eq!(first.target_idempotency_key(), retry.target_idempotency_key());
        assert_eq!(first.owner_capability_id(), "parties.privacy.action.apply");
        assert_eq!(first.owner_capability_version(), "1.0.0");
    }

    #[test]
    fn attempt_and_outcome_round_trip_strict_canonical_state() {
        let attempt = attempt(0, 100);
        let attempt_bytes = encode_owner_action_attempt_state(&attempt).unwrap();
        assert_eq!(
            decode_owner_action_attempt_state(&attempt_bytes).unwrap(),
            attempt
        );

        let outcome = PrivacyOwnerActionOutcome::record(
            &attempt,
            PrivacyOwnerOutcomeStatus::Succeeded,
            None,
            101,
        )
        .unwrap();
        let outcome_bytes = encode_owner_action_outcome_state(&outcome).unwrap();
        assert_eq!(
            decode_owner_action_outcome_state(&outcome_bytes).unwrap(),
            outcome
        );
    }

    #[test]
    fn tampered_or_unknown_execution_evidence_is_rejected() {
        let attempt = attempt(0, 100);
        let bytes = encode_owner_action_attempt_state(&attempt).unwrap();
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["attempt_digest"] = serde_json::Value::String("00".repeat(32));
        let tampered = serde_json::to_vec(&value).unwrap();
        assert!(decode_owner_action_attempt_state(&tampered).is_err());

        let mut unknown: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        unknown["owner_private_payload"] = serde_json::json!({"secret": true});
        let unknown = serde_json::to_vec(&unknown).unwrap();
        assert!(decode_owner_action_attempt_state(&unknown).is_err());
    }

    #[test]
    fn execution_evidence_bounds_and_failure_codes_fail_closed() {
        let over_bound = vec![b'x'; OWNER_ACTION_ATTEMPT_STATE_MAXIMUM_BYTES as usize + 1];
        assert!(decode_owner_action_attempt_state(&over_bound).is_err());

        let attempt = attempt(0, 100);
        assert!(
            PrivacyOwnerActionOutcome::record(
                &attempt,
                PrivacyOwnerOutcomeStatus::Succeeded,
                Some("UNEXPECTED".to_owned()),
                101,
            )
            .is_err()
        );
        assert!(
            PrivacyOwnerActionOutcome::record(
                &attempt,
                PrivacyOwnerOutcomeStatus::FailedRetryable,
                None,
                101,
            )
            .is_err()
        );
        assert!(
            PrivacyOwnerActionOutcome::record(
                &attempt,
                PrivacyOwnerOutcomeStatus::FailedRetryable,
                Some("not_canonical".to_owned()),
                101,
            )
            .is_err()
        );
        assert!(
            PrivacyOwnerActionOutcome::record(
                &attempt,
                PrivacyOwnerOutcomeStatus::FailedRetryable,
                Some("OWNER_TEMPORARILY_UNAVAILABLE".to_owned()),
                101,
            )
            .is_ok()
        );
    }

    #[test]
    fn owner_coordinates_and_coordinator_only_outcomes_are_frozen() {
        let unsupported = PrivacyOwnerActionAttempt::build(
            TenantId::try_new("tenant-a").unwrap(),
            RecordId::try_new("privacy-case-a").unwrap(),
            RecordId::try_new("action-plan-a").unwrap(),
            [11; 32],
            RecordId::try_new("retention-decision-a").unwrap(),
            [17; 32],
            &decision_item(
                "crm.unknown-owner",
                PlannedPrivacyAction::Delete,
                RetentionDecisionReason::ApprovedPrivacyAction,
            ),
            0,
            100,
        );
        assert!(unsupported.is_err());

        let retained = PrivacyOwnerActionAttempt::build(
            TenantId::try_new("tenant-a").unwrap(),
            RecordId::try_new("privacy-case-a").unwrap(),
            RecordId::try_new("action-plan-a").unwrap(),
            [11; 32],
            RecordId::try_new("retention-decision-a").unwrap(),
            [17; 32],
            &decision_item(
                "crm.parties",
                PlannedPrivacyAction::Retain,
                RetentionDecisionReason::MandatoryRetention,
            ),
            0,
            100,
        )
        .unwrap();
        assert_eq!(
            retained.coordinator_outcome_status(),
            Some(PrivacyOwnerOutcomeStatus::BlockedByRetention)
        );
    }
}
