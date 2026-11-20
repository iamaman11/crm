use crm_application_runtime::{application_capability_catalog, application_mutation_definitions};
use crm_data_quality_source_composition::mutation_capability_definitions as data_quality_mutation_capability_definitions;
use crm_module_sdk::CapabilityVersion;

const PUBLICATION_CAPABILITIES: [&str; 2] = [
    "data_quality.party.rule_set.publish",
    "data_quality.party.completeness_profile.publish",
];

#[test]
fn application_runtime_registers_exact_data_quality_definition_publications() {
    let definitions = application_mutation_definitions().unwrap();
    let catalog = application_capability_catalog().unwrap();
    let owner_definitions = data_quality_mutation_capability_definitions().unwrap();

    for capability in PUBLICATION_CAPABILITIES {
        let expected = owner_definitions
            .iter()
            .find(|definition| definition.capability_id.as_str() == capability)
            .expect("Data Quality publication capability in owner composition inventory");

        assert_eq!(
            definitions
                .iter()
                .filter(|definition| definition.capability_id == expected.capability_id)
                .count(),
            1
        );

        let capability_version = CapabilityVersion::try_new("1.0.0").unwrap();
        let definition = catalog
            .definition(&expected.capability_id, &capability_version)
            .expect("Data Quality publication capability in production application catalog");

        assert_eq!(definition.owner_module_id, expected.owner_module_id);
        assert_eq!(definition.risk, expected.risk);
        assert_eq!(definition.output_contract, expected.output_contract);
        assert_eq!(definition.mutation, expected.mutation);
        assert_eq!(
            definition.requires_idempotency,
            expected.requires_idempotency
        );
    }
}
