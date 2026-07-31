use crm_capability_runtime::{
    CapabilityDefinition, CapabilityRequest, CapabilityRisk, PayloadContract,
};
use crm_core_data::{
    AuditIntent, EventEvidence, PrivacyOwnerActionPlan, PrivacyOwnerActionPlanner,
    PrivacyOwnerActionTarget, PrivacyOwnerRecordAction,
};
use crm_customer_privacy::{
    MODULE_ID as CUSTOMER_PRIVACY_MODULE_ID, OWNER_ACTION_COMMAND_MAXIMUM_BYTES,
    OWNER_ACTION_COMMAND_RETENTION_POLICY_ID, OWNER_ACTION_COMMAND_SCHEMA_ID,
    OWNER_ACTION_COMMAND_SCHEMA_VERSION, PrivacyOwnerActionCommand, decode_owner_action_command,
    owner_action_command_descriptor_hash,
};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, DomainEvent, ErrorCategory, EventType, ModuleId,
    PayloadEncoding, RecordRef, RecordSnapshot, RecordType, RetentionPolicyId, SchemaId,
    SchemaVersion, SdkError, TypedPayload,
};
use sha2::{Digest, Sha256};

const OWNER_ACTION_EVENT_SCHEMA_ID: &str = "crm.customer-privacy.owner_action.event";
const OWNER_ACTION_EVENT_SCHEMA_VERSION: &str = "1.0.0";
const OWNER_ACTION_EVENT_DESCRIPTOR: &[u8] =
    b"crm.customer-privacy.owner_action.event/v1:canonical_owner_action_command";
const OWNER_ACTION_AUDIT_PROFILE: &str = "crm.cjson/v1";
const OWNER_ACTION_VERSION: &str = "1.0.0";

pub trait OwnerPrivacyActionPolicy: Send + Sync {
    fn owner_module_id(&self) -> &'static str;
    fn capability_id(&self) -> &'static str;
    fn supports_resource_type(&self, resource_type: &str) -> bool;

    fn anonymize(
        &self,
        command: &PrivacyOwnerActionCommand,
        current: &RecordSnapshot,
    ) -> Result<TypedPayload, SdkError>;

    fn deletion_tombstone(
        &self,
        command: &PrivacyOwnerActionCommand,
        current: &RecordSnapshot,
    ) -> Result<TypedPayload, SdkError>;
}

#[derive(Debug, Clone, Copy)]
pub struct OwnerPrivacyActionPlanner<P> {
    policy: P,
}

impl<P> OwnerPrivacyActionPlanner<P> {
    pub const fn new(policy: P) -> Self {
        Self { policy }
    }
}

impl<P> PrivacyOwnerActionPlanner for OwnerPrivacyActionPlanner<P>
where
    P: OwnerPrivacyActionPolicy,
{
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<PrivacyOwnerActionTarget, SdkError> {
        let command = validate_request(&self.policy, definition, request)?;
        let expected_version = i64::try_from(command.resource_version())
            .map_err(|_| action_invalid("resource version exceeds i64"))?;
        Ok(PrivacyOwnerActionTarget {
            reference: RecordRef {
                record_type: RecordType::try_new(command.resource_type())
                    .map_err(action_invalid)?,
                record_id: command.resource_id().clone(),
            },
            expected_version,
        })
    }

    fn plan(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
        current: &RecordSnapshot,
    ) -> Result<PrivacyOwnerActionPlan, SdkError> {
        let command = validate_request(&self.policy, definition, request)?;
        validate_locked_snapshot(&command, current)?;

        let (action, payload) = match command.action_code() {
            "anonymize" => (
                PrivacyOwnerRecordAction::Update,
                self.policy.anonymize(&command, current)?,
            ),
            "delete" => (
                PrivacyOwnerRecordAction::Delete,
                self.policy.deletion_tombstone(&command, current)?,
            ),
            "crypto_shred" => return Err(crypto_shred_unavailable()),
            _ => return Err(action_unsupported()),
        };
        validate_owner_payload(definition, &payload)?;

        Ok(PrivacyOwnerActionPlan {
            action,
            payload,
            event: owner_event(definition, request, &command, current)?,
            audit: owner_audit(request, &command)?,
            output: None,
        })
    }
}

pub fn owner_action_definition(
    owner_module_id: &str,
    capability_id: &str,
) -> Result<CapabilityDefinition, SdkError> {
    if !capability_id.ends_with(".privacy.action.apply") {
        return Err(action_configuration_invalid(
            "owner action capability coordinate is not canonical",
        ));
    }
    Ok(CapabilityDefinition {
        capability_id: CapabilityId::try_new(capability_id)
            .map_err(action_configuration_invalid)?,
        capability_version: CapabilityVersion::try_new(OWNER_ACTION_VERSION)
            .map_err(action_configuration_invalid)?,
        owner_module_id: ModuleId::try_new(owner_module_id)
            .map_err(action_configuration_invalid)?,
        input_contract: PayloadContract {
            owner: ModuleId::try_new(CUSTOMER_PRIVACY_MODULE_ID)
                .map_err(action_configuration_invalid)?,
            schema_id: SchemaId::try_new(OWNER_ACTION_COMMAND_SCHEMA_ID)
                .map_err(action_configuration_invalid)?,
            schema_version: SchemaVersion::try_new(OWNER_ACTION_COMMAND_SCHEMA_VERSION)
                .map_err(action_configuration_invalid)?,
            descriptor_hash: owner_action_command_descriptor_hash(),
            allowed_data_classes: vec![DataClass::Restricted],
            allowed_encodings: vec![PayloadEncoding::Json],
            maximum_size_bytes: OWNER_ACTION_COMMAND_MAXIMUM_BYTES,
        },
        output_contract: None,
        risk: CapabilityRisk::Critical,
        mutation: true,
        requires_idempotency: true,
        requires_approval: false,
        authorization_policy_id: capability_id.to_owned(),
        rate_limit_policy_id: None,
    })
}

pub fn owner_action_input_payload(bytes: Vec<u8>) -> Result<TypedPayload, SdkError> {
    let payload = TypedPayload {
        owner: ModuleId::try_new(CUSTOMER_PRIVACY_MODULE_ID)
            .map_err(action_configuration_invalid)?,
        schema_id: SchemaId::try_new(OWNER_ACTION_COMMAND_SCHEMA_ID)
            .map_err(action_configuration_invalid)?,
        schema_version: SchemaVersion::try_new(OWNER_ACTION_COMMAND_SCHEMA_VERSION)
            .map_err(action_configuration_invalid)?,
        descriptor_hash: owner_action_command_descriptor_hash(),
        data_class: DataClass::Restricted,
        encoding: PayloadEncoding::Json,
        maximum_size_bytes: OWNER_ACTION_COMMAND_MAXIMUM_BYTES,
        retention_policy_id: RetentionPolicyId::try_new(OWNER_ACTION_COMMAND_RETENTION_POLICY_ID)
            .map_err(action_configuration_invalid)?,
        bytes,
    };
    payload.validate()?;
    Ok(payload)
}

pub fn unsupported_owner_action(
    owner_module_id: &str,
    resource_type: &str,
    action: &str,
) -> SdkError {
    action_unsupported().with_internal_reference(format!(
        "owner={owner_module_id};resource_type={resource_type};action={action}"
    ))
}

fn validate_request<P: OwnerPrivacyActionPolicy>(
    policy: &P,
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<PrivacyOwnerActionCommand, SdkError> {
    request.context.validate()?;
    request.input.validate()?;
    let input_hash: [u8; 32] = Sha256::digest(&request.input.bytes).into();
    if definition.owner_module_id.as_str() != policy.owner_module_id()
        || definition.capability_id.as_str() != policy.capability_id()
        || definition.capability_version.as_str() != OWNER_ACTION_VERSION
        || request.context.module_id != definition.owner_module_id
        || request.context.execution.capability_id != definition.capability_id
        || request.context.execution.capability_version != definition.capability_version
        || !definition.input_contract.matches(&request.input)
        || request.input_hash != input_hash
    {
        return Err(action_invalid(
            "definition, execution context or typed input binding is invalid",
        ));
    }

    let command = decode_owner_action_command(&request.input.bytes)?;
    if command.tenant_id() != &request.context.execution.tenant_id
        || command.owner_module_id() != &definition.owner_module_id
        || command.owner_capability_id() != definition.capability_id.as_str()
        || command.owner_capability_version() != definition.capability_version.as_str()
        || command.target_idempotency_key() != &request.context.execution.idempotency_key
        || !policy.supports_resource_type(command.resource_type())
    {
        return Err(action_invalid(
            "canonical command is cross-tenant, cross-owner, over-bound or unsupported",
        ));
    }
    Ok(command)
}

fn validate_locked_snapshot(
    command: &PrivacyOwnerActionCommand,
    current: &RecordSnapshot,
) -> Result<(), SdkError> {
    let expected_version = i64::try_from(command.resource_version())
        .map_err(|_| action_invalid("resource version exceeds i64"))?;
    if current.reference.record_type.as_str() != command.resource_type()
        || current.reference.record_id != *command.resource_id()
        || current.version != expected_version
    {
        return Err(action_stale());
    }
    Ok(())
}

fn validate_owner_payload(
    definition: &CapabilityDefinition,
    payload: &TypedPayload,
) -> Result<(), SdkError> {
    payload.validate()?;
    if payload.owner != definition.owner_module_id || payload.bytes.is_empty() {
        return Err(action_invalid(
            "owner action payload is empty or crosses the authoritative owner boundary",
        ));
    }
    Ok(())
}

fn owner_event(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    command: &PrivacyOwnerActionCommand,
    current: &RecordSnapshot,
) -> Result<EventEvidence, SdkError> {
    let next_version = current
        .version
        .checked_add(1)
        .ok_or_else(|| action_invalid("owner record version overflowed"))?;
    let event_payload = TypedPayload {
        owner: definition.owner_module_id.clone(),
        schema_id: SchemaId::try_new(OWNER_ACTION_EVENT_SCHEMA_ID)
            .map_err(action_configuration_invalid)?,
        schema_version: SchemaVersion::try_new(OWNER_ACTION_EVENT_SCHEMA_VERSION)
            .map_err(action_configuration_invalid)?,
        descriptor_hash: Sha256::digest(OWNER_ACTION_EVENT_DESCRIPTOR).into(),
        data_class: DataClass::Restricted,
        encoding: PayloadEncoding::Json,
        maximum_size_bytes: OWNER_ACTION_COMMAND_MAXIMUM_BYTES,
        retention_policy_id: RetentionPolicyId::try_new(OWNER_ACTION_COMMAND_RETENTION_POLICY_ID)
            .map_err(action_configuration_invalid)?,
        bytes: request.input.bytes.clone(),
    };
    event_payload.validate()?;

    Ok(EventEvidence {
        event_id: deterministic_id("event", command),
        event: DomainEvent {
            event_type: EventType::try_new(format!("{}.completed", definition.capability_id))
                .map_err(action_configuration_invalid)?,
            aggregate: current.reference.clone(),
            expected_aggregate_version: Some(current.version),
            deduplication_key: command.target_idempotency_key().to_string(),
            payload: event_payload,
        },
        aggregate_version: next_version,
        event_sequence: next_version,
        occurred_at_unix_nanos: command.planned_at_unix_nanos(),
    })
}

fn owner_audit(
    request: &CapabilityRequest,
    command: &PrivacyOwnerActionCommand,
) -> Result<AuditIntent, SdkError> {
    Ok(AuditIntent {
        audit_record_id: deterministic_id("audit", command),
        canonicalization_profile: OWNER_ACTION_AUDIT_PROFILE.to_owned(),
        canonical_envelope: request.input.bytes.clone(),
        occurred_at_unix_nanos: command.planned_at_unix_nanos(),
    })
}

fn deterministic_id(prefix: &str, command: &PrivacyOwnerActionCommand) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"crm.customer-privacy.owner-action-evidence/v1");
    append_frame(&mut hasher, prefix.as_bytes());
    append_frame(&mut hasher, command.tenant_id().as_str().as_bytes());
    append_frame(&mut hasher, command.attempt_id().as_str().as_bytes());
    append_frame(&mut hasher, command.attempt_digest());
    append_frame(&mut hasher, command.owner_module_id().as_str().as_bytes());
    append_frame(&mut hasher, command.resource_type().as_bytes());
    append_frame(&mut hasher, command.resource_id().as_str().as_bytes());
    format!("privacy-owner-{prefix}-{}", hex(&hasher.finalize()))
}

fn append_frame(hasher: &mut Sha256, value: &[u8]) {
    hasher.update(u64::try_from(value.len()).unwrap_or(u64::MAX).to_be_bytes());
    hasher.update(value);
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

fn action_configuration_invalid(reference: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "PRIVACY_OWNER_ACTION_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The owner privacy action configuration is invalid.",
    )
    .with_internal_reference(reference.to_string())
}

fn action_invalid(reference: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "PRIVACY_OWNER_ACTION_COMMAND_REJECTED",
        ErrorCategory::InvalidArgument,
        false,
        "The owner privacy action command was rejected.",
    )
    .with_internal_reference(reference.to_string())
}

fn action_stale() -> SdkError {
    SdkError::new(
        "PRIVACY_OWNER_ACTION_STALE",
        ErrorCategory::Conflict,
        false,
        "The owner resource changed after privacy planning.",
    )
}

fn action_unsupported() -> SdkError {
    SdkError::new(
        "PRIVACY_OWNER_ACTION_UNSUPPORTED",
        ErrorCategory::InvalidArgument,
        false,
        "The requested owner privacy action is not supported.",
    )
}

fn crypto_shred_unavailable() -> SdkError {
    SdkError::new(
        "PRIVACY_OWNER_CRYPTO_SHRED_UNAVAILABLE",
        ErrorCategory::Unavailable,
        false,
        "The required cryptographic destruction boundary is unavailable.",
    )
}
