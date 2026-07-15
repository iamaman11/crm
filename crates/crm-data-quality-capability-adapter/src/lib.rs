#![forbid(unsafe_code)]

//! Governed mutation adapter for the authoritative Data Quality owner domain.
//!
//! The first production slice publishes immutable content-addressed Party
//! rule-set versions. Evaluation, cross-record completeness validation and
//! Party remediation remain separate later composition layers.

mod rule_set_planner;

pub use rule_set_planner::*;

use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRisk};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, SdkError,
};

pub const MODULE_ID: &str = crm_data_quality::MODULE_ID;
pub const PUBLISH_PARTY_RULE_SET_CAPABILITY: &str = "data_quality.party.rule_set.publish";
pub const PUBLISH_PARTY_RULE_SET_REQUEST_SCHEMA: &str =
    "crm.data_quality.v1.PublishPartyRuleSetVersionRequest";
pub const PUBLISH_PARTY_RULE_SET_RESPONSE_SCHEMA: &str =
    "crm.data_quality.v1.PublishPartyRuleSetVersionResponse";
pub const PARTY_RULE_SET_PUBLISHED_EVENT_TYPE: &str = "data_quality.party.rule_set.published";
pub const PARTY_RULE_SET_PUBLISHED_EVENT_SCHEMA: &str =
    "crm.data_quality.v1.PartyRuleSetVersionPublishedEvent";

pub fn capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    Ok(vec![capability_definition()?])
}

pub fn capability_definition() -> Result<CapabilityDefinition, SdkError> {
    Ok(CapabilityDefinition {
        capability_id: configured(CapabilityId::try_new(PUBLISH_PARTY_RULE_SET_CAPABILITY))?,
        capability_version: configured(CapabilityVersion::try_new(support::CONTRACT_VERSION))?,
        owner_module_id: configured(ModuleId::try_new(MODULE_ID))?,
        input_contract: support::protobuf_contract(
            MODULE_ID,
            PUBLISH_PARTY_RULE_SET_REQUEST_SCHEMA,
            vec![DataClass::Confidential],
        )?,
        output_contract: Some(support::protobuf_contract(
            MODULE_ID,
            PUBLISH_PARTY_RULE_SET_RESPONSE_SCHEMA,
            vec![DataClass::Confidential],
        )?),
        risk: CapabilityRisk::Medium,
        mutation: true,
        requires_idempotency: true,
        requires_approval: false,
        authorization_policy_id: PUBLISH_PARTY_RULE_SET_CAPABILITY.to_owned(),
        rate_limit_policy_id: None,
    })
}

fn configured<T>(value: Result<T, crm_module_sdk::IdentifierError>) -> Result<T, SdkError> {
    value.map_err(|error| configuration_error().with_internal_reference(error.to_string()))
}

fn configuration_error() -> SdkError {
    SdkError::new(
        "DATA_QUALITY_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Data Quality capability configuration is invalid.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publishes_exact_rule_set_coordinate_as_confidential_idempotent_mutation() {
        let definition = capability_definition().unwrap();
        assert_eq!(
            definition.capability_id.as_str(),
            PUBLISH_PARTY_RULE_SET_CAPABILITY
        );
        assert_eq!(definition.owner_module_id.as_str(), MODULE_ID);
        assert_eq!(
            definition.capability_version.as_str(),
            support::CONTRACT_VERSION
        );
        assert_eq!(
            definition.input_contract.allowed_data_classes,
            vec![DataClass::Confidential]
        );
        assert_eq!(definition.risk, CapabilityRisk::Medium);
        assert!(definition.mutation);
        assert!(definition.requires_idempotency);
        assert!(!definition.requires_approval);
    }
}
