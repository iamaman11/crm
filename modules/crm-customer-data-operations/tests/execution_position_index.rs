use crm_customer_data_operations::{
    ExecutionPositionIndex, ExecutionRowReference, ImportRowId, ImportRowStatus,
};

fn row(position: u32, status: ImportRowStatus) -> ExecutionRowReference {
    ExecutionRowReference::new(
        ImportRowId::try_new(format!("import-row-{position}")).unwrap(),
        position,
        status,
    )
}

#[test]
fn public_execution_index_orders_by_source_position_not_relationship_page_order() {
    let index = ExecutionPositionIndex::build(
        3,
        [
            row(3, ImportRowStatus::Valid),
            row(1, ImportRowStatus::Invalid),
            row(2, ImportRowStatus::FailedRetryable),
        ],
    )
    .unwrap();

    assert_eq!(
        index.next_after_checkpoint(0).unwrap().unwrap().row_position(),
        1
    );
    assert_eq!(
        index.next_after_checkpoint(1).unwrap().unwrap().row_position(),
        2
    );
    assert_eq!(
        index.next_after_checkpoint(2).unwrap().unwrap().row_position(),
        3
    );
    assert!(index.next_after_checkpoint(3).unwrap().is_none());
}

#[test]
fn public_execution_index_rejects_incomplete_authoritative_row_sets() {
    let error = ExecutionPositionIndex::build(
        3,
        [
            row(1, ImportRowStatus::Valid),
            row(3, ImportRowStatus::Valid),
        ],
    )
    .unwrap_err();

    assert_eq!(
        error.code,
        "CUSTOMER_DATA_IMPORT_EXECUTION_ROW_SET_INCOMPLETE"
    );
}
