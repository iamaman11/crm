use crm_customer_data_operations_source_composition::{
    APPEND_SOURCE_CHUNK_CAPABILITY, CREATE_JOB_FROM_SOURCE_CAPABILITY,
    CREATE_SOURCE_ARTIFACT_CAPABILITY, FINALIZE_SOURCE_ARTIFACT_CAPABILITY,
    SOURCE_MUTATION_CAPABILITY_IDS, VALIDATE_SOURCE_BATCH_CAPABILITY,
    source_capability_definitions,
};
use crm_module_sdk::DataClass;

#[test]
fn production_source_capabilities_are_artifact_backed_and_idempotent() {
    let definitions = source_capability_definitions().unwrap();
    assert_eq!(definitions.len(), 5);
    assert_eq!(SOURCE_MUTATION_CAPABILITY_IDS.len(), 5);
    assert_eq!(
        SOURCE_MUTATION_CAPABILITY_IDS,
        [
            CREATE_SOURCE_ARTIFACT_CAPABILITY,
            APPEND_SOURCE_CHUNK_CAPABILITY,
            FINALIZE_SOURCE_ARTIFACT_CAPABILITY,
            CREATE_JOB_FROM_SOURCE_CAPABILITY,
            VALIDATE_SOURCE_BATCH_CAPABILITY,
        ]
    );
    for definition in definitions {
        assert!(definition.mutation);
        assert!(definition.requires_idempotency);
        assert_eq!(
            definition.input_contract.allowed_data_classes,
            vec![DataClass::Personal]
        );
        assert_eq!(
            definition.output_contract.unwrap().allowed_data_classes,
            vec![DataClass::Personal]
        );
    }
}

#[test]
fn legacy_preparsed_coordinates_are_not_part_of_the_source_capability_surface() {
    assert!(!SOURCE_MUTATION_CAPABILITY_IDS.contains(&"customer_data.import.party.create"));
    assert!(!SOURCE_MUTATION_CAPABILITY_IDS.contains(&"customer_data.import.party.rows.validate"));
}
