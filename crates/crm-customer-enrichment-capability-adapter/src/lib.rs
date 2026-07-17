#![forbid(unsafe_code)]

//! Governed mutation adapter foundation for `crm.customer-enrichment`.
//!
//! This first native slice publishes immutable provider-profile versions through the shared
//! transactional record/idempotency/outbox/audit runtime. Provider network I/O, credentials,
//! Party reads and owner mutation remain outside this crate.

mod provider_profile_planner;

pub use provider_profile_planner::{
    CustomerEnrichmentProviderProfileCapabilityPlanner, provider_profile_from_definition,
    provider_profile_persisted_contract, provider_profile_persisted_payload,
    provider_profile_record_ref, provider_profile_to_wire,
};

use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRisk};
use crm_module_sdk::{CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, SdkError};

pub const MODULE_ID: &str = crm_customer_enrichment::MODULE_ID;
pub const PROVIDER_PROFILE_VERSION_RECORD_TYPE: &str =
    crm_customer_enrichment::PROVIDER_PROFILE_VERSION_RECORD_TYPE;

pub const PUBLISH_PROVIDER_PROFILE_CAPABILITY: &str =
    "customer_enrichment.provider_profile.publish";
pub const PUBLISH_PROVIDER_PROFILE_REQUEST_SCHEMA: &str =
    "crm.customer_enrichment.v1.PublishProviderProfileVersionRequest";
pub const PUBLISH_PROVIDER_PROFILE_RESPONSE_SCHEMA: &str =
    "crm.customer_enrichment.v1.PublishProviderProfileVersionResponse";
pub const PROVIDER_PROFILE_PUBLISHED_EVENT_TYPE: &str =
    "customer_enrichment.provider_profile.published";
pub const PROVIDER_PROFILE_PUBLISHED_EVENT_SCHEMA: &str =
    "crm.customer_enrichment.v1.ProviderProfileVersionPublishedEvent";

pub const IMPLEMENTED_MUTATION_CAPABILITY_IDS: &[&str] = &[PUBLISH_PROVIDER_PROFILE_CAPABILITY];

pub fn capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    Ok(vec![provider_profile_capability_definition()?])
}

pub fn capability_definition() -> Result<CapabilityDefinition, SdkError> {
    provider_profile_capability_definition()
}

pub fn provider_profile_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    Ok(CapabilityDefinition {
        capability_id: configured(CapabilityId::try_new(PUBLISH_PROVIDER_PROFILE_CAPABILITY))?,
        capability_version: configured(CapabilityVersion::try_new(support::CONTRACT_VERSION))?,
        owner_module_id: configured(ModuleId::try_new(MODULE_ID))?,
        input_contract: support::protobuf_contract(
            MODULE_ID,
            PUBLISH_PROVIDER_PROFILE_REQUEST_SCHEMA,
            vec![DataClass::Confidential],
        )?,
        output_contract: Some(support::protobuf_contract(
            MODULE_ID,
            PUBLISH_PROVIDER_PROFILE_RESPONSE_SCHEMA,
            vec![DataClass::Confidential],
        )?),
        risk: CapabilityRisk::Medium,
        mutation: true,
        requires_idempotency: true,
        requires_approval: false,
        authorization_policy_id: PUBLISH_PROVIDER_PROFILE_CAPABILITY.to_owned(),
        rate_limit_policy_id: None,
    })
}

fn configured<T>(value: Result<T, crm_module_sdk::IdentifierError>) -> Result<T, SdkError> {
    value.map_err(|error| configuration_error().with_internal_reference(error.to_string()))
}

fn configuration_error() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Customer Enrichment capability configuration is invalid.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn implemented_mutation_catalog_is_exact() {
        let definitions = capability_definitions().unwrap();
        assert_eq!(definitions.len(), 1);
        let definition = &definitions[0];
        assert_eq!(definition.owner_module_id.as_str(), MODULE_ID);
        assert_eq!(
            definition.capability_id.as_str(),
            PUBLISH_PROVIDER_PROFILE_CAPABILITY
        );
        assert_eq!(definition.capability_version.as_str(), "1.0.0");
        assert!(definition.mutation);
        assert!(definition.requires_idempotency);
        assert!(!definition.requires_approval);
        assert_eq!(definition.risk, CapabilityRisk::Medium);
        assert_eq!(
            IMPLEMENTED_MUTATION_CAPABILITY_IDS,
            &[PUBLISH_PROVIDER_PROFILE_CAPABILITY]
        );
    }
}
