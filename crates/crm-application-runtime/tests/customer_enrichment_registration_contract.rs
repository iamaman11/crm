use crm_application_runtime::{
    application_mutation_definitions, application_query_definitions, declared_business_module_ids,
};
use crm_customer_enrichment_capability_adapter::{
    CANCEL_ENRICHMENT_REQUEST_CAPABILITY, CREATE_ENRICHMENT_REQUEST_CAPABILITY, MODULE_ID,
    PUBLISH_MAPPING_CAPABILITY, PUBLISH_PROVIDER_PROFILE_CAPABILITY,
};
use crm_customer_enrichment_review_adapter::{
    ACCEPT_SUGGESTION_CAPABILITY, REJECT_SUGGESTION_CAPABILITY,
};
use crm_customer_enrichment_visibility::QUERY_VISIBILITY_CAPABILITY_IDS;
use std::collections::BTreeSet;

#[test]
fn definition_publications_request_lifecycle_and_reviews_are_the_composed_enrichment_mutations() {
    let enrichment_definitions = application_mutation_definitions()
        .unwrap()
        .into_iter()
        .filter(|definition| definition.owner_module_id.as_str() == MODULE_ID)
        .collect::<Vec<_>>();

    assert_eq!(enrichment_definitions.len(), 6);
    assert_eq!(
        enrichment_definitions
            .iter()
            .map(|definition| definition.capability_id.as_str())
            .collect::<BTreeSet<_>>(),
        [
            PUBLISH_PROVIDER_PROFILE_CAPABILITY,
            PUBLISH_MAPPING_CAPABILITY,
            CREATE_ENRICHMENT_REQUEST_CAPABILITY,
            CANCEL_ENRICHMENT_REQUEST_CAPABILITY,
            REJECT_SUGGESTION_CAPABILITY,
            ACCEPT_SUGGESTION_CAPABILITY,
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
fn production_queries_have_exact_module_owned_visibility_parity() {
    let enrichment_definitions = application_query_definitions()
        .unwrap()
        .into_iter()
        .filter(|definition| definition.owner_module_id.as_str() == MODULE_ID)
        .collect::<Vec<_>>();
    let production_ids = enrichment_definitions
        .iter()
        .map(|definition| definition.capability_id.as_str())
        .collect::<BTreeSet<_>>();
    let visibility_ids = QUERY_VISIBILITY_CAPABILITY_IDS
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();

    assert_eq!(enrichment_definitions.len(), 6);
    assert_eq!(QUERY_VISIBILITY_CAPABILITY_IDS.len(), 6);
    assert_eq!(production_ids, visibility_ids);
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
