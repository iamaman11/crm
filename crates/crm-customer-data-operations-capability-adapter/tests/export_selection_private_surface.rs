use crm_customer_data_operations_capability_adapter::{
    INTERNAL_COMMIT_PARTY_EXPORT_SELECTION_PAGE_CAPABILITY,
    INTERNAL_EXPORT_SELECTION_CAPABILITY_IDS, INTERNAL_FINALIZE_PARTY_EXPORT_SELECTION_CAPABILITY,
    MUTATION_CAPABILITY_IDS, capability_definitions, internal_export_selection_capability_definitions,
};

#[test]
fn worker_selection_capabilities_are_versioned_but_absent_from_public_mutation_catalog() {
    let public_definitions = capability_definitions().expect("public capability definitions");
    let public_ids = public_definitions
        .iter()
        .map(|definition| definition.capability_id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(public_ids, MUTATION_CAPABILITY_IDS);
    assert!(!public_ids.contains(&INTERNAL_COMMIT_PARTY_EXPORT_SELECTION_PAGE_CAPABILITY));
    assert!(!public_ids.contains(&INTERNAL_FINALIZE_PARTY_EXPORT_SELECTION_CAPABILITY));

    let internal_definitions =
        internal_export_selection_capability_definitions().expect("private capability definitions");
    assert_eq!(
        internal_definitions
            .iter()
            .map(|definition| definition.capability_id.as_str())
            .collect::<Vec<_>>(),
        INTERNAL_EXPORT_SELECTION_CAPABILITY_IDS
    );
    assert!(internal_definitions.iter().all(|definition| {
        definition.mutation
            && definition.requires_idempotency
            && !definition.requires_approval
    }));
}
