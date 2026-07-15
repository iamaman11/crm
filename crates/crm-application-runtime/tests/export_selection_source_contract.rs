#![forbid(unsafe_code)]

#[path = "../src/export_selection_source.rs"]
mod export_selection_source;

use crm_customer_data_operations_execution_composition::{
    PartyExportSelectionSource, PartyExportSelectionSourceContinuation,
    PartyExportSelectionSourceKind,
};
use crm_module_sdk::RecordId;
use export_selection_source::GovernedPartyExportSelectionSource;

fn assert_source_contract<T: PartyExportSelectionSource>() {}

#[test]
fn governed_runtime_source_implements_private_export_selection_port() {
    assert_source_contract::<GovernedPartyExportSelectionSource>();
}

#[test]
fn continuation_and_kind_contracts_remain_bounded_and_explicit() {
    let continuation = PartyExportSelectionSourceContinuation {
        sort_value: "100".to_owned(),
        record_id: RecordId::try_new("party-export-source-contract").unwrap(),
    };
    assert_eq!(continuation.sort_value, "100");
    assert_ne!(
        PartyExportSelectionSourceKind::Person,
        PartyExportSelectionSourceKind::Organization
    );
}
