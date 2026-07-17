use crm_application_runtime::{application_mutation_definitions, declared_business_module_ids};
use crm_customer_enrichment_capability_adapter::{MODULE_ID, PUBLISH_PROVIDER_PROFILE_CAPABILITY};

#[test]
fn provider_profile_publication_is_the_only_composed_enrichment_route() {
    let enrichment_definitions = application_mutation_definitions()
        .unwrap()
        .into_iter()
        .filter(|definition| definition.owner_module_id.as_str() == MODULE_ID)
        .collect::<Vec<_>>();

    assert_eq!(enrichment_definitions.len(), 1);
    let definition = &enrichment_definitions[0];
    assert_eq!(
        definition.capability_id.as_str(),
        PUBLISH_PROVIDER_PROFILE_CAPABILITY
    );
    assert_eq!(definition.capability_version.as_str(), "1.0.0");
    assert!(definition.mutation);
    assert!(definition.requires_idempotency);
    assert!(!definition.requires_approval);
}

#[test]
fn customer_enrichment_is_a_declared_business_module() {
    assert!(declared_business_module_ids().contains(MODULE_ID));
}
