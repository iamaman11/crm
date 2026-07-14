use crm_application_runtime::application_mutation_definitions;
use crm_customer_data_operations_source_composition::{
    CREATE_JOB_FROM_SOURCE_CAPABILITY, VALIDATE_SOURCE_BATCH_CAPABILITY,
};

#[test]
fn production_catalog_exposes_only_artifact_backed_import_create_and_validation() {
    let definitions = application_mutation_definitions().unwrap();
    let ids = definitions
        .iter()
        .map(|definition| definition.capability_id.as_str())
        .collect::<Vec<_>>();

    assert!(ids.contains(&CREATE_JOB_FROM_SOURCE_CAPABILITY));
    assert!(ids.contains(&VALIDATE_SOURCE_BATCH_CAPABILITY));
    assert!(!ids.contains(&"customer_data.import.party.create"));
    assert!(!ids.contains(&"customer_data.import.party.rows.validate"));
}
