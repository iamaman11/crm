use crm_core_data::MAXIMUM_RELATED_RECORD_QUERY_PAGE_SIZE;
use crm_customer_data_operations_execution_composition::{
    DEFAULT_EXECUTION_READER_PAGE_SIZE, ImportExecutionSnapshotReader,
    PostgresImportExecutionSnapshotReader,
};

#[test]
fn production_reader_implements_the_execution_snapshot_port_with_a_bounded_default_page() {
    fn assert_reader_port<T: ImportExecutionSnapshotReader>() {}

    assert_reader_port::<PostgresImportExecutionSnapshotReader>();
    assert!(DEFAULT_EXECUTION_READER_PAGE_SIZE > 0);
    assert!(DEFAULT_EXECUTION_READER_PAGE_SIZE <= MAXIMUM_RELATED_RECORD_QUERY_PAGE_SIZE);
}
