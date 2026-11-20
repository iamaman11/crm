use crm_core_data::MAXIMUM_RELATED_RECORD_QUERY_PAGE_SIZE;
use crm_customer_data_operations_execution_composition::{
    DEFAULT_EXECUTION_READER_PAGE_SIZE, ImportExecutionSnapshotReader,
    PostgresImportExecutionSnapshotReader,
};

const _: () = assert!(DEFAULT_EXECUTION_READER_PAGE_SIZE > 0);
const _: () = assert!(DEFAULT_EXECUTION_READER_PAGE_SIZE <= MAXIMUM_RELATED_RECORD_QUERY_PAGE_SIZE);

#[test]
fn production_reader_implements_the_execution_snapshot_port() {
    fn assert_reader_port<T: ImportExecutionSnapshotReader>() {}

    assert_reader_port::<PostgresImportExecutionSnapshotReader>();
}
