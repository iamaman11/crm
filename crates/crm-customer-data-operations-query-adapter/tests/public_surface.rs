use crm_customer_data_operations_query_adapter::{
    GET_IMPORT_JOB_CAPABILITY, LIST_IMPORT_JOBS_CAPABILITY, LIST_IMPORT_ROWS_CAPABILITY,
    query_capability_definitions,
};
use crm_module_sdk::DataClass;

#[test]
fn publishes_exact_permission_aware_import_query_surface() {
    let definitions = query_capability_definitions().expect("query definitions must be valid");
    let capability_ids = definitions
        .iter()
        .map(|definition| definition.capability_id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        capability_ids,
        vec![
            GET_IMPORT_JOB_CAPABILITY,
            LIST_IMPORT_JOBS_CAPABILITY,
            LIST_IMPORT_ROWS_CAPABILITY,
        ]
    );
    assert!(definitions.iter().all(|definition| !definition.mutation));
    assert!(
        definitions
            .iter()
            .all(|definition| !definition.requires_idempotency)
    );
    assert!(definitions.iter().all(|definition| {
        definition.input_contract.allowed_data_classes == vec![DataClass::Personal]
    }));
}
