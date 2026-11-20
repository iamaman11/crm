use crm_customer_data_operations_execution_composition::{
    PartyExportExecutionSource, PartyExportExecutionWorker, PostgresPartyExportExecutionReader,
    PostgresPartyExportExecutionSink,
};

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn export_execution_production_types_are_thread_safe() {
    assert_send_sync::<PartyExportExecutionWorker>();
    assert_send_sync::<PostgresPartyExportExecutionReader>();
    assert_send_sync::<PostgresPartyExportExecutionSink>();
    assert_send_sync::<Box<dyn PartyExportExecutionSource>>();
}
