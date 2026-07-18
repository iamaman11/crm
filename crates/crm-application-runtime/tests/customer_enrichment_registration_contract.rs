use crm_application_runtime::{
    application_mutation_definitions, application_query_definitions, declared_business_module_ids,
};
use crm_customer_enrichment_capability_adapter::{
    CREATE_ENRICHMENT_REQUEST_CAPABILITY, MODULE_ID, PUBLISH_MAPPING_CAPABILITY,
    PUBLISH_PROVIDER_PROFILE_CAPABILITY,
};
use crm_customer_enrichment_query_adapter::{
    GET_MAPPING_CAPABILITY, GET_PROVIDER_PROFILE_CAPABILITY,
};
use std::collections::BTreeSet;

#[test]
fn definition_publications_and_request_creation_are_the_composed_enrichment_mutations() {
    let enrichment_definitions = application_mutation_definitions()
        .unwrap()
        .into_iter()
        .filter(|definition| definition.owner_module_id.as_str() == MODULE_ID)
        .collect::<Vec<_>>();

    assert_eq!(enrichment_definitions.len(), 3);
    assert_eq!(
        enrichment_definitions
            .iter()
            .map(|definition| definition.capability_id.as_str())
            .collect::<BTreeSet<_>>(),
        [
            PUBLISH_PROVIDER_PROFILE_CAPABILITY,
            PUBLISH_MAPPING_CAPABILITY,
            CREATE_ENRICHMENT_REQUEST_CAPABILITY,
        ]
        .into_iter()
        .collect()
    );
    for definition in enrichment_definitions {
        assert_eq!(definition.capability_version.as_str(), "1.0.0");
        assert!(definition.mutation);
        assert!(definition.requires_idempotency);
        assert!(!definition.requires_approval);
    }
}

#[test]
fn definition_lookups_are_the_only_composed_enrichment_queries() {
    let enrichment_definitions = application_query_definitions()
        .unwrap()
        .into_iter()
        .filter(|definition| definition.owner_module_id.as_str() == MODULE_ID)
        .collect::<Vec<_>>();

    assert_eq!(enrichment_definitions.len(), 2);
    assert_eq!(
        enrichment_definitions
            .iter()
            .map(|definition| definition.capability_id.as_str())
            .collect::<BTreeSet<_>>(),
        [GET_PROVIDER_PROFILE_CAPABILITY, GET_MAPPING_CAPABILITY]
            .into_iter()
            .collect()
    );
    for definition in enrichment_definitions {
        assert_eq!(definition.capability_version.as_str(), "1.0.0");
        assert!(!definition.mutation);
        assert!(!definition.requires_idempotency);
        assert!(!definition.requires_approval);
    }
}

#[test]
fn customer_enrichment_is_a_declared_business_module() {
    assert!(declared_business_module_ids().contains(MODULE_ID));
}
