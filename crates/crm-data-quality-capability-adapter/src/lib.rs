#![forbid(unsafe_code)]

//! Governed mutation adapter for the authoritative Data Quality owner domain.
//!
//! The first production slices publish immutable content-addressed Party
//! rule-set and completeness-profile versions. Evaluation and Party remediation
//! remain separate later composition layers.

mod completeness_profile_planner;
mod rule_set_planner;

pub use completeness_profile_planner::{
    CompletenessProfileReferenceScope, DataQualityCompletenessProfileCapabilityPlanner,
    completeness_profile_reference_scope_from_request,
    completeness_profile_rule_set_version_id_from_snapshot,
    party_completeness_profile_from_definition, party_completeness_profile_persisted_contract,
    party_completeness_profile_persisted_payload, party_completeness_profile_to_wire,
};
pub use rule_set_planner::{
    DataQualityRuleSetCapabilityPlanner, party_rule_set_from_definition,
    party_rule_set_persisted_contract, party_rule_set_persisted_payload, party_rule_set_to_wire,
};

use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRisk};
use crm_data_quality::{PartyCompletenessProfileVersion, PartyRuleSetVersion};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, RecordSnapshot, SdkError,
};

pub const MODULE_ID: &str = crm_data_quality::MODULE_ID;
pub const PARTY_RULE_SET_VERSION_RECORD_TYPE: &str =
    crm_data_quality::PARTY_RULE_SET_VERSION_RECORD_TYPE;
pub const PARTY_COMPLETENESS_PROFILE_VERSION_RECORD_TYPE: &str =
    crm_data_quality::PARTY_COMPLETENESS_PROFILE_VERSION_RECORD_TYPE;
pub const PUBLISH_PARTY_RULE_SET_CAPABILITY: &str = "data_quality.party.rule_set.publish";
pub const PUBLISH_PARTY_RULE_SET_REQUEST_SCHEMA: &str =
    "crm.data_quality.v1.PublishPartyRuleSetVersionRequest";
pub const PUBLISH_PARTY_RULE_SET_RESPONSE_SCHEMA: &str =
    "crm.data_quality.v1.PublishPartyRuleSetVersionResponse";
pub const PARTY_RULE_SET_PUBLISHED_EVENT_TYPE: &str = "data_quality.party.rule_set.published";
pub const PARTY_RULE_SET_PUBLISHED_EVENT_SCHEMA: &str =
    "crm.data_quality.v1.PartyRuleSetVersionPublishedEvent";

pub const PUBLISH_PARTY_COMPLETENESS_PROFILE_CAPABILITY: &str =
    "data_quality.party.completeness_profile.publish";
pub const PUBLISH_PARTY_COMPLETENESS_PROFILE_REQUEST_SCHEMA: &str =
    "crm.data_quality.v1.PublishPartyCompletenessProfileVersionRequest";
pub const PUBLISH_PARTY_COMPLETENESS_PROFILE_RESPONSE_SCHEMA: &str =
    "crm.data_quality.v1.PublishPartyCompletenessProfileVersionResponse";
pub const PARTY_COMPLETENESS_PROFILE_PUBLISHED_EVENT_TYPE: &str =
    "data_quality.party.completeness_profile.published";
pub const PARTY_COMPLETENESS_PROFILE_PUBLISHED_EVENT_SCHEMA: &str =
    "crm.data_quality.v1.PartyCompletenessProfileVersionPublishedEvent";

pub const MUTATION_CAPABILITY_IDS: &[&str] = &[
    PUBLISH_PARTY_RULE_SET_CAPABILITY,
    PUBLISH_PARTY_COMPLETENESS_PROFILE_CAPABILITY,
];

pub fn party_rule_set_from_snapshot(
    snapshot: &RecordSnapshot,
) -> Result<PartyRuleSetVersion, SdkError> {
    ensure_immutable_version(snapshot, "Party rule-set version")?;
    rule_set_planner::party_rule_set_from_snapshot(snapshot)
}

pub fn party_completeness_profile_from_immutable_snapshot(
    snapshot: &RecordSnapshot,
    rule_set: &PartyRuleSetVersion,
) -> Result<PartyCompletenessProfileVersion, SdkError> {
    ensure_immutable_version(snapshot, "Party completeness-profile version")?;
    completeness_profile_planner::party_completeness_profile_from_snapshot(snapshot, rule_set)
}

pub fn capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    Ok(vec![
        rule_set_capability_definition()?,
        completeness_profile_capability_definition()?,
    ])
}

pub fn capability_definition() -> Result<CapabilityDefinition, SdkError> {
    rule_set_capability_definition()
}

pub fn rule_set_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    mutation_definition(
        PUBLISH_PARTY_RULE_SET_CAPABILITY,
        PUBLISH_PARTY_RULE_SET_REQUEST_SCHEMA,
        PUBLISH_PARTY_RULE_SET_RESPONSE_SCHEMA,
    )
}

pub fn completeness_profile_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    mutation_definition(
        PUBLISH_PARTY_COMPLETENESS_PROFILE_CAPABILITY,
        PUBLISH_PARTY_COMPLETENESS_PROFILE_REQUEST_SCHEMA,
        PUBLISH_PARTY_COMPLETENESS_PROFILE_RESPONSE_SCHEMA,
    )
}

fn mutation_definition(
    capability_id: &'static str,
    input_schema: &'static str,
    output_schema: &'static str,
) -> Result<CapabilityDefinition, SdkError> {
    Ok(CapabilityDefinition {
        capability_id: configured(CapabilityId::try_new(capability_id))?,
        capability_version: configured(CapabilityVersion::try_new(support::CONTRACT_VERSION))?,
        owner_module_id: configured(ModuleId::try_new(MODULE_ID))?,
        input_contract: support::protobuf_contract(
            MODULE_ID,
            input_schema,
            vec![DataClass::Confidential],
        )?,
        output_contract: Some(support::protobuf_contract(
            MODULE_ID,
            output_schema,
            vec![DataClass::Confidential],
        )?),
        risk: CapabilityRisk::Medium,
        mutation: true,
        requires_idempotency: true,
        requires_approval: false,
        authorization_policy_id: capability_id.to_owned(),
        rate_limit_policy_id: None,
    })
}

fn ensure_immutable_version(snapshot: &RecordSnapshot, label: &str) -> Result<(), SdkError> {
    if snapshot.version != 1 {
        return Err(SdkError::new(
            "DATA_QUALITY_PERSISTED_STATE_INVALID",
            ErrorCategory::Internal,
            false,
            "The persisted Data Quality state is invalid.",
        )
        .with_internal_reference(format!(
            "immutable {label} record has unexpected aggregate version {}",
            snapshot.version
        )));
    }
    Ok(())
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
    fn immutable_definition_publications_are_exact_confidential_idempotent_mutations() {
        let definitions = capability_definitions().unwrap();
        assert_eq!(definitions.len(), 2);
        for (definition, expected_capability) in definitions.iter().zip(MUTATION_CAPABILITY_IDS) {
            assert_eq!(definition.capability_id.as_str(), *expected_capability);
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
}
