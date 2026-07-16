use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRisk};
use crm_module_sdk::{CapabilityId, CapabilityVersion, DataClass, ModuleId, SdkError};

use crate::MODULE_ID;

pub const REMEDIATE_PARTY_DISPLAY_NAME_CAPABILITY: &str =
    "data_quality.party.display_name.remediate";
pub const REMEDIATE_PARTY_DISPLAY_NAME_REQUEST_SCHEMA: &str =
    "crm.data_quality.v1.RemediatePartyDisplayNameRequest";
pub const REMEDIATE_PARTY_DISPLAY_NAME_RESPONSE_SCHEMA: &str =
    "crm.data_quality.v1.RemediatePartyDisplayNameResponse";
pub const PARTY_REMEDIATION_COMPLETED_EVENT_TYPE: &str =
    "data_quality.party.remediation.completed";
pub const PARTY_REMEDIATION_COMPLETED_EVENT_SCHEMA: &str =
    "crm.data_quality.v1.PartyDisplayNameRemediationCompletedEvent";

pub fn remediation_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    Ok(CapabilityDefinition {
        capability_id: support::configured_identifier(CapabilityId::try_new(
            REMEDIATE_PARTY_DISPLAY_NAME_CAPABILITY,
        ))?,
        capability_version: support::configured_identifier(CapabilityVersion::try_new(
            support::CONTRACT_VERSION,
        ))?,
        owner_module_id: support::configured_identifier(ModuleId::try_new(MODULE_ID))?,
        input_contract: support::protobuf_contract(
            MODULE_ID,
            REMEDIATE_PARTY_DISPLAY_NAME_REQUEST_SCHEMA,
            vec![DataClass::Personal],
        )?,
        output_contract: Some(support::protobuf_contract(
            MODULE_ID,
            REMEDIATE_PARTY_DISPLAY_NAME_RESPONSE_SCHEMA,
            vec![DataClass::Personal],
        )?),
        risk: CapabilityRisk::High,
        mutation: true,
        requires_idempotency: true,
        requires_approval: false,
        authorization_policy_id: REMEDIATE_PARTY_DISPLAY_NAME_CAPABILITY.to_owned(),
        rate_limit_policy_id: None,
    })
}
