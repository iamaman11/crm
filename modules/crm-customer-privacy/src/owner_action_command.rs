use crate::PrivacyOwnerActionAttempt;
use crate::canonicalization::persisted_state_json as canonical_json;
use crm_module_sdk::{IdempotencyKey, ModuleId, RecordId, SdkError, TenantId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const OWNER_ACTION_COMMAND_SCHEMA_ID: &str = "crm.customer-privacy.owner_action.command";
pub const OWNER_ACTION_COMMAND_SCHEMA_VERSION: &str = "1.0.0";
pub const OWNER_ACTION_COMMAND_MAXIMUM_BYTES: u64 = 32 * 1024;
pub const OWNER_ACTION_COMMAND_RETENTION_POLICY_ID: &str =
    "crm.customer_privacy.owner_action_command";

const COMMAND_DESCRIPTOR: &[u8] = b"crm.customer-privacy.owner_action.command/v1:tenant_id,privacy_case_id,action_plan_id,action_plan_digest,retention_decision_id,retention_decision_digest,attempt_id,attempt_digest,item_sequence,attempt_generation,item_digest,owner_module_id,owner_capability_id,owner_capability_version,target_idempotency_key,resource_type,resource_id,resource_version,action_code,planned_at_unix_nanos";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyOwnerActionCommand {
    tenant_id: TenantId,
    privacy_case_id: RecordId,
    action_plan_id: RecordId,
    action_plan_digest: [u8; 32],
    retention_decision_id: RecordId,
    retention_decision_digest: [u8; 32],
    attempt_id: RecordId,
    attempt_digest: [u8; 32],
    item_sequence: u32,
    attempt_generation: u32,
    item_digest: [u8; 32],
    owner_module_id: ModuleId,
    owner_capability_id: String,
    owner_capability_version: String,
    target_idempotency_key: IdempotencyKey,
    resource_type: String,
    resource_id: RecordId,
    resource_version: u64,
    action_code: String,
    planned_at_unix_nanos: i64,
}

impl PrivacyOwnerActionCommand {
    pub fn from_attempt(attempt: &PrivacyOwnerActionAttempt) -> Result<Self, SdkError> {
        let command = Self {
            tenant_id: attempt.tenant_id().clone(),
            privacy_case_id: attempt.privacy_case_id().clone(),
            action_plan_id: attempt.action_plan_id().clone(),
            action_plan_digest: *attempt.action_plan_digest(),
            retention_decision_id: attempt.retention_decision_id().clone(),
            retention_decision_digest: *attempt.retention_decision_digest(),
            attempt_id: attempt.attempt_id().clone(),
            attempt_digest: *attempt.digest(),
            item_sequence: attempt.item_sequence(),
            attempt_generation: attempt.attempt_generation(),
            item_digest: *attempt.item_digest(),
            owner_module_id: attempt.owner_module_id().clone(),
            owner_capability_id: attempt.owner_capability_id().to_owned(),
            owner_capability_version: attempt.owner_capability_version().to_owned(),
            target_idempotency_key: attempt.target_idempotency_key().clone(),
            resource_type: attempt.resource_type().to_owned(),
            resource_id: attempt.resource_id().clone(),
            resource_version: attempt.resource_version(),
            action_code: attempt.action_code().to_owned(),
            planned_at_unix_nanos: attempt.planned_at_unix_nanos(),
        };
        command.validate()?;
        Ok(command)
    }

    pub fn validate(&self) -> Result<(), SdkError> {
        if self.owner_capability_id.is_empty()
            || self.owner_capability_version != OWNER_ACTION_COMMAND_SCHEMA_VERSION
            || self.resource_type.is_empty()
            || self.resource_version == 0
            || self.item_sequence == 0
            || self.planned_at_unix_nanos <= 0
            || !matches!(
                self.action_code.as_str(),
                "delete" | "anonymize" | "crypto_shred"
            )
        {
            return Err(command_invalid("owner action command fields are invalid"));
        }
        if self.action_plan_digest == [0; 32]
            || self.retention_decision_digest == [0; 32]
            || self.attempt_digest == [0; 32]
            || self.item_digest == [0; 32]
        {
            return Err(command_invalid("owner action command digest is missing"));
        }
        Ok(())
    }

    pub fn tenant_id(&self) -> &TenantId {
        &self.tenant_id
    }
    pub fn privacy_case_id(&self) -> &RecordId {
        &self.privacy_case_id
    }
    pub fn action_plan_id(&self) -> &RecordId {
        &self.action_plan_id
    }
    pub const fn action_plan_digest(&self) -> &[u8; 32] {
        &self.action_plan_digest
    }
    pub fn retention_decision_id(&self) -> &RecordId {
        &self.retention_decision_id
    }
    pub const fn retention_decision_digest(&self) -> &[u8; 32] {
        &self.retention_decision_digest
    }
    pub fn attempt_id(&self) -> &RecordId {
        &self.attempt_id
    }
    pub const fn attempt_digest(&self) -> &[u8; 32] {
        &self.attempt_digest
    }
    pub const fn item_sequence(&self) -> u32 {
        self.item_sequence
    }
    pub const fn attempt_generation(&self) -> u32 {
        self.attempt_generation
    }
    pub const fn item_digest(&self) -> &[u8; 32] {
        &self.item_digest
    }
    pub fn owner_module_id(&self) -> &ModuleId {
        &self.owner_module_id
    }
    pub fn owner_capability_id(&self) -> &str {
        &self.owner_capability_id
    }
    pub fn owner_capability_version(&self) -> &str {
        &self.owner_capability_version
    }
    pub fn target_idempotency_key(&self) -> &IdempotencyKey {
        &self.target_idempotency_key
    }
    pub fn resource_type(&self) -> &str {
        &self.resource_type
    }
    pub fn resource_id(&self) -> &RecordId {
        &self.resource_id
    }
    pub const fn resource_version(&self) -> u64 {
        self.resource_version
    }
    pub fn action_code(&self) -> &str {
        &self.action_code
    }
    pub const fn planned_at_unix_nanos(&self) -> i64 {
        self.planned_at_unix_nanos
    }
}

pub fn owner_action_command_descriptor_hash() -> [u8; 32] {
    Sha256::digest(COMMAND_DESCRIPTOR).into()
}

pub fn encode_owner_action_command(
    command: &PrivacyOwnerActionCommand,
) -> Result<Vec<u8>, SdkError> {
    command.validate()?;
    let bytes = canonical_json::to_vec(&CommandStateV1::from(command)).map_err(command_invalid)?;
    validate_size(&bytes)?;
    Ok(bytes)
}

pub fn decode_owner_action_command(bytes: &[u8]) -> Result<PrivacyOwnerActionCommand, SdkError> {
    validate_size(bytes)?;
    let state: CommandStateV1 = canonical_json::from_slice(bytes).map_err(command_invalid)?;
    let command = state.into_domain()?;
    if encode_owner_action_command(&command)? != bytes {
        return Err(command_invalid(
            "owner action command is not the strict canonical v1 encoding",
        ));
    }
    Ok(command)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandStateV1 {
    tenant_id: String,
    privacy_case_id: String,
    action_plan_id: String,
    action_plan_digest: [u8; 32],
    retention_decision_id: String,
    retention_decision_digest: [u8; 32],
    attempt_id: String,
    attempt_digest: [u8; 32],
    item_sequence: u32,
    attempt_generation: u32,
    item_digest: [u8; 32],
    owner_module_id: String,
    owner_capability_id: String,
    owner_capability_version: String,
    target_idempotency_key: String,
    resource_type: String,
    resource_id: String,
    resource_version: u64,
    action_code: String,
    planned_at_unix_nanos: i64,
}

impl From<&PrivacyOwnerActionCommand> for CommandStateV1 {
    fn from(command: &PrivacyOwnerActionCommand) -> Self {
        Self {
            tenant_id: command.tenant_id.to_string(),
            privacy_case_id: command.privacy_case_id.to_string(),
            action_plan_id: command.action_plan_id.to_string(),
            action_plan_digest: command.action_plan_digest,
            retention_decision_id: command.retention_decision_id.to_string(),
            retention_decision_digest: command.retention_decision_digest,
            attempt_id: command.attempt_id.to_string(),
            attempt_digest: command.attempt_digest,
            item_sequence: command.item_sequence,
            attempt_generation: command.attempt_generation,
            item_digest: command.item_digest,
            owner_module_id: command.owner_module_id.to_string(),
            owner_capability_id: command.owner_capability_id.clone(),
            owner_capability_version: command.owner_capability_version.clone(),
            target_idempotency_key: command.target_idempotency_key.to_string(),
            resource_type: command.resource_type.clone(),
            resource_id: command.resource_id.to_string(),
            resource_version: command.resource_version,
            action_code: command.action_code.clone(),
            planned_at_unix_nanos: command.planned_at_unix_nanos,
        }
    }
}

impl CommandStateV1 {
    fn into_domain(self) -> Result<PrivacyOwnerActionCommand, SdkError> {
        let command = PrivacyOwnerActionCommand {
            tenant_id: TenantId::try_new(self.tenant_id).map_err(command_invalid)?,
            privacy_case_id: RecordId::try_new(self.privacy_case_id).map_err(command_invalid)?,
            action_plan_id: RecordId::try_new(self.action_plan_id).map_err(command_invalid)?,
            action_plan_digest: self.action_plan_digest,
            retention_decision_id: RecordId::try_new(self.retention_decision_id)
                .map_err(command_invalid)?,
            retention_decision_digest: self.retention_decision_digest,
            attempt_id: RecordId::try_new(self.attempt_id).map_err(command_invalid)?,
            attempt_digest: self.attempt_digest,
            item_sequence: self.item_sequence,
            attempt_generation: self.attempt_generation,
            item_digest: self.item_digest,
            owner_module_id: ModuleId::try_new(self.owner_module_id).map_err(command_invalid)?,
            owner_capability_id: self.owner_capability_id,
            owner_capability_version: self.owner_capability_version,
            target_idempotency_key: IdempotencyKey::try_new(self.target_idempotency_key)
                .map_err(command_invalid)?,
            resource_type: self.resource_type,
            resource_id: RecordId::try_new(self.resource_id).map_err(command_invalid)?,
            resource_version: self.resource_version,
            action_code: self.action_code,
            planned_at_unix_nanos: self.planned_at_unix_nanos,
        };
        command.validate()?;
        Ok(command)
    }
}

fn validate_size(bytes: &[u8]) -> Result<(), SdkError> {
    if bytes.is_empty()
        || u64::try_from(bytes.len()).unwrap_or(u64::MAX) > OWNER_ACTION_COMMAND_MAXIMUM_BYTES
    {
        return Err(command_invalid("owner action command size is invalid"));
    }
    Ok(())
}

fn command_invalid(reference: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_OWNER_ACTION_COMMAND_INVALID",
        crm_module_sdk::ErrorCategory::InvalidArgument,
        false,
        "The owner action command is invalid.",
    )
    .with_internal_reference(reference.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_descriptor_is_stable_and_nonzero() {
        assert_eq!(
            owner_action_command_descriptor_hash(),
            owner_action_command_descriptor_hash()
        );
        assert_ne!(owner_action_command_descriptor_hash(), [0; 32]);
    }
}
