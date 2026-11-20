use crm_customer_data_operations_execution_composition::{
    EXPORT_SELECTION_WORKER_ACTOR_ID, IMPORT_EXECUTION_WORKER_ACTOR_ID,
};

#[test]
fn export_selection_worker_uses_a_dedicated_actor_identity() {
    assert!(!EXPORT_SELECTION_WORKER_ACTOR_ID.is_empty());
    assert_ne!(
        EXPORT_SELECTION_WORKER_ACTOR_ID,
        IMPORT_EXECUTION_WORKER_ACTOR_ID
    );
}
