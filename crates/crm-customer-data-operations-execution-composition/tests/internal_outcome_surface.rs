use crm_customer_data_operations_capability_adapter::MUTATION_CAPABILITY_IDS;
use crm_customer_data_operations_execution_composition::{
    INTERNAL_OUTCOME_CAPABILITY_IDS, internal_capability_definitions,
};
use crm_module_sdk::DataClass;

#[test]
fn worker_only_outcome_capabilities_are_typed_idempotent_and_absent_from_public_mutations() {
    let definitions = internal_capability_definitions().unwrap();

    assert_eq!(definitions.len(), INTERNAL_OUTCOME_CAPABILITY_IDS.len());
    for definition in definitions {
        assert!(definition.mutation);
        assert!(definition.requires_idempotency);
        assert!(!definition.requires_approval);
        assert_eq!(
            definition.input_contract.allowed_data_classes,
            vec![DataClass::Personal]
        );
        assert!(
            !MUTATION_CAPABILITY_IDS.contains(&definition.capability_id.as_str()),
            "worker-only outcome coordinate leaked into the public customer-data mutation surface"
        );
    }
}
