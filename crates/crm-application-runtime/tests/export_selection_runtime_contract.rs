use crm_application_runtime::{GovernedPartyExportSelectionSource, ApplicationComponents};
use crm_customer_data_operations_execution_composition::PartyExportSelectionWorker;

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn production_export_selection_runtime_types_are_send_sync() {
    assert_send_sync::<GovernedPartyExportSelectionSource>();
    assert_send_sync::<PartyExportSelectionWorker>();
    assert_send_sync::<ApplicationComponents>();
}
