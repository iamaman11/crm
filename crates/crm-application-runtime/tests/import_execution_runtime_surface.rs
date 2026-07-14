use crm_application_runtime::application_mutation_definitions;
use crm_customer_data_operations_execution_composition::{
    IMPORT_EXECUTION_WORKER_ACTOR_ID, INTERNAL_OUTCOME_CAPABILITY_IDS,
};

#[test]
fn import_execution_internal_outcomes_are_not_public_application_mutations() {
    let public = application_mutation_definitions().unwrap();
    for internal_capability in INTERNAL_OUTCOME_CAPABILITY_IDS {
        assert!(
            public
                .iter()
                .all(|definition| definition.capability_id.as_str() != internal_capability),
            "internal import outcome capability leaked into the public mutation catalog: {internal_capability}"
        );
    }
}

#[test]
fn import_execution_worker_uses_a_dedicated_non_user_actor_identity() {
    assert_eq!(
        IMPORT_EXECUTION_WORKER_ACTOR_ID,
        "crm-api-import-execution-worker"
    );
}
