use crate::ImportExecutionSnapshot;
use crm_capability_plan_support as support;
use crm_core_data::{
    MAXIMUM_RELATED_RECORD_QUERY_PAGE_SIZE, PostgresDataStore, RecordGetQuery,
    RelatedRecordListQuery,
};
use crm_customer_data_operations::{ImportJobId, decode_import_row_state};
use crm_customer_data_operations_capability_adapter::{
    IMPORT_JOB_RECORD_TYPE, IMPORT_JOB_ROW_RELATIONSHIP_TYPE, IMPORT_ROW_RECORD_TYPE, MODULE_ID,
    import_job_from_snapshot, import_row_persisted_contract,
};
use crm_module_sdk::{
    DataClass, ErrorCategory, ModuleId, PortFuture, RecordId, RecordRef, RecordType,
    RelationshipType, SdkError, TenantId,
};

pub const DEFAULT_EXECUTION_READER_PAGE_SIZE: u32 = 1_000;

pub trait ImportExecutionSnapshotReader: Send + Sync {
    fn load<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        job_id: &'a ImportJobId,
    ) -> PortFuture<'a, Result<ImportExecutionSnapshot, SdkError>>;
}

/// Reads one authoritative executing import snapshot through tenant-scoped core-data query ports.
///
/// The reader never assumes relationship pagination order is source order. It loads every
/// authoritative job-to-row target under tenant RLS, strictly rehydrates import-owned row state,
/// and delegates duplicate/missing/out-of-range position rejection to `ExecutionPositionIndex`
/// before any target Party capability can be invoked.
#[derive(Debug, Clone)]
pub struct PostgresImportExecutionSnapshotReader {
    store: PostgresDataStore,
    page_size: u32,
}

impl PostgresImportExecutionSnapshotReader {
    pub fn new(store: PostgresDataStore) -> Self {
        Self {
            store,
            page_size: DEFAULT_EXECUTION_READER_PAGE_SIZE,
        }
    }

    pub fn try_with_page_size(store: PostgresDataStore, page_size: u32) -> Result<Self, SdkError> {
        validate_page_size(page_size)?;
        Ok(Self { store, page_size })
    }
}

impl ImportExecutionSnapshotReader for PostgresImportExecutionSnapshotReader {
    fn load<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        job_id: &'a ImportJobId,
    ) -> PortFuture<'a, Result<ImportExecutionSnapshot, SdkError>> {
        Box::pin(async move {
            let owner_module_id = module_id()?;
            let job_record_type = job_record_type()?;
            let row_record_type = row_record_type()?;
            let job_record_id = RecordId::try_new(job_id.as_str()).map_err(configuration_error)?;
            let job_snapshot = self
                .store
                .get_record_for_query(&RecordGetQuery {
                    tenant_id: tenant_id.clone(),
                    owner_module_id: owner_module_id.clone(),
                    record_type: job_record_type.clone(),
                    record_id: job_record_id,
                })
                .await?
                .ok_or_else(job_not_found)?;
            let job = import_job_from_snapshot(&job_snapshot)?;
            let job_ref = RecordRef {
                record_type: job_record_type,
                record_id: job_snapshot.reference.record_id.clone(),
            };

            let mut rows = Vec::with_capacity(job.total_rows() as usize);
            let mut after_record_id = None;
            loop {
                let page = self
                    .store
                    .list_related_records_for_query(&RelatedRecordListQuery {
                        tenant_id: tenant_id.clone(),
                        relationship_owner_module_id: owner_module_id.clone(),
                        relationship_type: RelationshipType::try_new(
                            IMPORT_JOB_ROW_RELATIONSHIP_TYPE,
                        )
                        .map_err(configuration_error)?,
                        source: job_ref.clone(),
                        target_owner_module_id: owner_module_id.clone(),
                        target_record_type: row_record_type.clone(),
                        page_size: self.page_size,
                        after_record_id: after_record_id.clone(),
                    })
                    .await?;
                for snapshot in page.records {
                    let row =
                        decode_import_row_state(support::persisted_json_bytes_with_data_class(
                            &snapshot,
                            import_row_persisted_contract(),
                            DataClass::Personal,
                        )?)?;
                    if row.row_id().as_str() != snapshot.reference.record_id.as_str()
                        || row.version() != snapshot.version
                    {
                        return Err(reader_error(
                            "CUSTOMER_DATA_IMPORT_EXECUTION_ROW_IDENTITY_INVALID",
                            ErrorCategory::Unavailable,
                            true,
                            "Stored customer-data import execution state is temporarily unavailable.",
                        ));
                    }
                    rows.push(row);
                    if rows.len() > job.total_rows() as usize {
                        return Err(reader_error(
                            "CUSTOMER_DATA_IMPORT_EXECUTION_ROW_SET_OVERSIZED",
                            ErrorCategory::Unavailable,
                            true,
                            "Stored customer-data import execution state is temporarily unavailable.",
                        ));
                    }
                }
                match page.next_record_id {
                    Some(next) => after_record_id = Some(next),
                    None => break,
                }
            }

            ImportExecutionSnapshot::try_new(job, rows)
        })
    }
}

fn validate_page_size(page_size: u32) -> Result<(), SdkError> {
    if page_size == 0 || page_size > MAXIMUM_RELATED_RECORD_QUERY_PAGE_SIZE {
        Err(reader_error(
            "CUSTOMER_DATA_IMPORT_EXECUTION_READER_PAGE_SIZE_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "The import execution reader page size is invalid.",
        ))
    } else {
        Ok(())
    }
}

fn module_id() -> Result<ModuleId, SdkError> {
    ModuleId::try_new(MODULE_ID).map_err(configuration_error)
}

fn job_record_type() -> Result<RecordType, SdkError> {
    RecordType::try_new(IMPORT_JOB_RECORD_TYPE).map_err(configuration_error)
}

fn row_record_type() -> Result<RecordType, SdkError> {
    RecordType::try_new(IMPORT_ROW_RECORD_TYPE).map_err(configuration_error)
}

fn job_not_found() -> SdkError {
    reader_error(
        "CUSTOMER_DATA_IMPORT_EXECUTION_JOB_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The requested import job was not found.",
    )
}

fn configuration_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    reader_error(
        "CUSTOMER_DATA_IMPORT_EXECUTION_READER_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The import execution reader is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}

fn reader_error(
    code: &'static str,
    category: ErrorCategory,
    retryable: bool,
    safe_message: &'static str,
) -> SdkError {
    SdkError::new(code, category, retryable, safe_message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_zero_and_oversized_page_sizes_before_database_access() {
        let invalid_zero = validate_page_size(0).unwrap_err();
        assert_eq!(
            invalid_zero.code,
            "CUSTOMER_DATA_IMPORT_EXECUTION_READER_PAGE_SIZE_INVALID"
        );

        let invalid_large =
            validate_page_size(MAXIMUM_RELATED_RECORD_QUERY_PAGE_SIZE + 1).unwrap_err();
        assert_eq!(
            invalid_large.code,
            "CUSTOMER_DATA_IMPORT_EXECUTION_READER_PAGE_SIZE_INVALID"
        );
    }

    #[test]
    fn accepts_the_default_bounded_page_size() {
        validate_page_size(DEFAULT_EXECUTION_READER_PAGE_SIZE).unwrap();
    }
}
