use crm_application_runtime::{application_capability_catalog, application_mutation_definitions};
use crm_capability_runtime::CapabilityRisk;
use crm_data_quality_capability_adapter::{MODULE_ID, PUBLISH_PARTY_RULE_SET_CAPABILITY};
use crm_module_sdk::{CapabilityId, CapabilityVersion};

#[test]
fn application_runtime_registers_exactly_one_party_rule_set_publication_coordinate() {
    let definitions = application_mutation_definitions().unwrap();
    assert_eq!(
        definitions
            .iter()
            .filter(|definition| {
                definition.capability_id.as_str() == PUBLISH_PARTY_RULE_SET_CAPABILITY
            })
            .count(),
        1
    );

    let capability_id = CapabilityId::try_new(PUBLISH_PARTY_RULE_SET_CAPABILITY).unwrap();
    let capability_version = CapabilityVersion::try_new("1.0.0").unwrap();
    let definition = application_capability_catalog()
        .unwrap()
        .definition(&capability_id, &capability_version)
        .expect("Data Quality publication capability in production application catalog");

    assert_eq!(
        definition.capability_id.as_str(),
        PUBLISH_PARTY_RULE_SET_CAPABILITY
    );
    assert_eq!(definition.owner_module_id.as_str(), MODULE_ID);
    assert_eq!(definition.risk, CapabilityRisk::Medium);
    assert!(definition.output_contract.is_some());
    assert!(definition.mutation);
    assert!(definition.requires_idempotency);
}
