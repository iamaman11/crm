use crate::errors::configured;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRisk, PayloadContract};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ModuleId, PayloadEncoding, SchemaId, SchemaVersion,
    SdkError,
};
use crm_parties::MODULE_ID;
use crm_proto_contracts::message_descriptor_hash;

pub const CAPABILITY_ID: &str = "parties.privacy.scope.contribute";
pub const CAPABILITY_VERSION: &str = "1.0.0";
pub const INPUT_SCHEMA_ID: &str = "crm.customer_privacy.v1.PartiesPrivacyScopeContributionRequest";
pub const OUTPUT_SCHEMA_ID: &str =
    "crm.customer_privacy.v1.PartiesPrivacyScopeContributionResponse";
pub const CONTRACT_SCHEMA_VERSION: &str = "1.0.0";
pub const INPUT_MAXIMUM_BYTES: u64 = 16 * 1024;
pub const OUTPUT_MAXIMUM_BYTES: u64 = 64 * 1024;
pub const INPUT_RETENTION_POLICY_ID: &str = "crm.parties.privacy.scope.request";
pub const OUTPUT_RETENTION_POLICY_ID: &str = "crm.parties.privacy.scope.response";
pub const DEFAULT_PAGE_SIZE: u32 = 64;
pub const MAXIMUM_PAGE_SIZE: u32 = 128;

pub fn parties_privacy_scope_definition() -> Result<CapabilityDefinition, SdkError> {
    Ok(CapabilityDefinition {
        capability_id: capability_id()?,
        capability_version: capability_version()?,
        owner_module_id: module_id()?,
        input_contract: payload_contract(INPUT_SCHEMA_ID, INPUT_SCHEMA_ID, INPUT_MAXIMUM_BYTES)?,
        output_contract: Some(payload_contract(
            OUTPUT_SCHEMA_ID,
            OUTPUT_SCHEMA_ID,
            OUTPUT_MAXIMUM_BYTES,
        )?),
        risk: CapabilityRisk::Medium,
        mutation: false,
        requires_idempotency: false,
        requires_approval: false,
        authorization_policy_id: CAPABILITY_ID.to_owned(),
        rate_limit_policy_id: None,
    })
}

pub(crate) fn validate_definition(definition: &CapabilityDefinition) -> Result<(), SdkError> {
    let expected = parties_privacy_scope_definition()?;
    if definition != &expected {
        return Err(crate::errors::invalid_contract(
            "PARTIES_PRIVACY_SCOPE_DEFINITION_MISMATCH",
            "The Parties privacy scope definition is invalid.",
        ));
    }
    Ok(())
}

pub(crate) fn module_id() -> Result<ModuleId, SdkError> {
    configured(ModuleId::try_new(MODULE_ID))
}

pub(crate) fn capability_id() -> Result<CapabilityId, SdkError> {
    configured(CapabilityId::try_new(CAPABILITY_ID))
}

pub(crate) fn capability_version() -> Result<CapabilityVersion, SdkError> {
    configured(CapabilityVersion::try_new(CAPABILITY_VERSION))
}

pub(crate) fn schema_id(value: &str) -> Result<SchemaId, SdkError> {
    configured(SchemaId::try_new(value))
}

pub(crate) fn schema_version(value: &str) -> Result<SchemaVersion, SdkError> {
    configured(SchemaVersion::try_new(value))
}

pub(crate) fn input_descriptor_hash() -> [u8; 32] {
    message_descriptor_hash(INPUT_SCHEMA_ID)
}

pub(crate) fn output_descriptor_hash() -> [u8; 32] {
    message_descriptor_hash(OUTPUT_SCHEMA_ID)
}

fn payload_contract(
    schema: &str,
    message: &str,
    maximum_size_bytes: u64,
) -> Result<PayloadContract, SdkError> {
    Ok(PayloadContract {
        owner: module_id()?,
        schema_id: schema_id(schema)?,
        schema_version: schema_version(CONTRACT_SCHEMA_VERSION)?,
        descriptor_hash: message_descriptor_hash(message),
        allowed_data_classes: vec![DataClass::Confidential],
        allowed_encodings: vec![PayloadEncoding::Protobuf],
        maximum_size_bytes,
    })
}
