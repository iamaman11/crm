use crm_activities_capability_adapter::MODULE_ID as ACTIVITIES_MODULE_ID;
use crm_sales_activities_capability_composition::{
    PRODUCTION_MUTATION_CAPABILITY_IDS, capability_definitions,
};
use crm_sales_capability_adapter::MODULE_ID as SALES_MODULE_ID;
use std::collections::BTreeSet;

#[test]
fn production_catalog_coordinates_are_unique_and_partitioned_by_owner() {
    let definitions = capability_definitions().expect("valid production capability definitions");
    let ids = definitions
        .iter()
        .map(|definition| definition.capability_id.as_str())
        .collect::<BTreeSet<_>>();

    assert_eq!(definitions.len(), PRODUCTION_MUTATION_CAPABILITY_IDS.len());
    assert_eq!(ids.len(), PRODUCTION_MUTATION_CAPABILITY_IDS.len());
    assert_eq!(
        definitions
            .iter()
            .filter(|definition| definition.owner_module_id.as_str() == SALES_MODULE_ID)
            .count(),
        3
    );
    assert_eq!(
        definitions
            .iter()
            .filter(|definition| definition.owner_module_id.as_str() == ACTIVITIES_MODULE_ID)
            .count(),
        4
    );
    assert!(definitions.iter().all(|definition| definition.mutation));
    assert!(
        definitions
            .iter()
            .all(|definition| definition.requires_idempotency)
    );
}
