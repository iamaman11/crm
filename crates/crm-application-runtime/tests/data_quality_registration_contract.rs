use crm_application_runtime::{application_capability_catalog, application_mutation_definitions};
use crm_capability_runtime::CapabilityRisk;
use crm_data_quality_capability_adapter::{
    MODULE_ID, PUBLISH_PARTY_COMPLETENESS_PROFILE_CAPABILITY, PUBLISH_PARTY_RULE_SET_CAPABILITY,
};
use crm_module_sdk::{CapabilityId, CapabilityVersion};

#[test]
fn application_runtime_registers_exact_data_quality_definition_publications() {
    let definitions = application_mutation_definitions().unwrap();
    let catalog = application_capability_catalog().unwrap();

    for capability in [
        PUBLISH_PARTY_RULE_SET_CAPABILITY,
        PUBLISH_PARTY_COMPLETENESS_PROFILE_CAPABILITY,
    ] {
        assert_eq!(
            definitions
                .iter()
                .filter(|definition| definition.capability_id.as_str() == capability)
                .count(),
            1
        );

        let capability_id = CapabilityId::try_new(capability).unwrap();
        let capability_version = CapabilityVersion::try_new("1.0.0").unwrap();
        let definition = catalog
            .definition(&capability_id, &capability_version)
            .expect("Data Quality publication capability in production application catalog");

        assert_eq!(definition.capability_id.as_str(), capability);
        assert_eq!(definition.owner_module_id.as_str(), MODULE_ID);
        assert_eq!(definition.risk, CapabilityRisk::Medium);
        assert!(definition.output_contract.is_some());
        assert!(definition.mutation);
        assert!(definition.requires_idempotency);
    }
}
