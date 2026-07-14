use crate::{ImportExecutionSnapshotReader, PartyImportExecutionCoordinator};
use crm_core_data::{PostgresDataStore, RecordListQuery, RecordQueryContinuation, RecordQuerySort};
use crm_customer_data_operations::ImportJobStatus;
use crm_customer_data_operations_capability_adapter::{
    IMPORT_JOB_RECORD_TYPE, MODULE_ID as CUSTOMER_DATA_OPERATIONS_MODULE_ID,
    import_job_from_snapshot,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, Clock,
    CorrelationId, ExecutionContext, IdempotencyKey, ModuleExecutionContext, ModuleId, PortFuture,
    RecordType, RequestId, SchemaVersion, SdkError, TenantId, TraceId,
};
use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex};

pub const DEFAULT_IMPORT_EXECUTION_SCAN_PAGE_SIZE: u32 = 100;
pub const IMPORT_EXECUTION_WORKER_ACTOR_ID: &str = "crm-api-import-execution-worker";
pub const IMPORT_EXECUTION_WORKER_CAPABILITY_ID: &str =
    "customer_data.import.party.internal.execute_cycle";
pub const IMPORT_EXECUTION_WORKER_CAPABILITY_VERSION: &str = "1.0.0";

const _: () = assert!(DEFAULT_IMPORT_EXECUTION_SCAN_PAGE_SIZE > 0);
const _: () =
    assert!(DEFAULT_IMPORT_EXECUTION_SCAN_PAGE_SIZE <= crm_core_data::MAXIMUM_RECORD_QUERY_PAGE_SIZE);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportExecutionTenantCycle {
    pub scanned_jobs: u32,
    pub executed_jobs: u32,
    pub has_more: bool,
}

#[derive(Clone)]
pub struct PartyImportExecutionWorker {
    store: PostgresDataStore,
    reader: Arc<dyn ImportExecutionSnapshotReader>,
    coordinator: Arc<PartyImportExecutionCoordinator>,
    clock: Arc<dyn Clock>,
    actor_id: ActorId,
    page_size: u32,
    scan_cursors: Arc<Mutex<BTreeMap<TenantId, Option<RecordQueryContinuation>>>>,
}

impl fmt::Debug for PartyImportExecutionWorker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PartyImportExecutionWorker")
            .field("store", &self.store)
            .field("reader", &"dyn ImportExecutionSnapshotReader")
            .field("coordinator", &self.coordinator)
            .field("clock", &"dyn Clock")
            .field("actor_id", &self.actor_id)
            .field("page_size", &self.page_size)
            .finish()
    }
}

impl PartyImportExecutionWorker {
    pub fn new(
        store: PostgresDataStore,
        reader: Arc<dyn ImportExecutionSnapshotReader>,
        coordinator: Arc<PartyImportExecutionCoordinator>,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, SdkError> {
        Self::try_with_page_size(
            store,
            reader,
            coordinator,
            clock,
            ActorId::try_new(IMPORT_EXECUTION_WORKER_ACTOR_ID).map_err(configuration_error)?,
            DEFAULT_IMPORT_EXECUTION_SCAN_PAGE_SIZE,
        )
    }

    pub fn try_with_page_size(
        store: PostgresDataStore,
        reader: Arc<dyn ImportExecutionSnapshotReader>,
        coordinator: Arc<PartyImportExecutionCoordinator>,
        clock: Arc<dyn Clock>,
        actor_id: ActorId,
        page_size: u32,
    ) -> Result<Self, SdkError> {
        if page_size == 0 || page_size > crm_core_data::MAXIMUM_RECORD_QUERY_PAGE_SIZE {
            return Err(SdkError::invalid_argument(
                "customer_data.import.execution_worker.page_size",
                "Import execution worker page size is invalid",
            ));
        }
        Ok(Self {
            store,
            reader,
            coordinator,
            clock,
            actor_id,
            page_size,
            scan_cursors: Arc::new(Mutex::new(BTreeMap::new())),
        })
    }

    pub fn actor_id(&self) -> &ActorId {
        &self.actor_id
    }

    pub fn run_tenant_cycle<'a>(
        &'a self,
        tenant_id: TenantId,
    ) -> PortFuture<'a, Result<ImportExecutionTenantCycle, SdkError>> {
        Box::pin(async move {
            let after = self
                .scan_cursors
                .lock()
                .map_err(|_| worker_state_unavailable())?
                .get(&tenant_id)
                .cloned()
                .flatten();
            let page = self
                .store
                .list_records_for_query(&RecordListQuery {
                    tenant_id: tenant_id.clone(),
                    owner_module_id: ModuleId::try_new(CUSTOMER_DATA_OPERATIONS_MODULE_ID)
                        .map_err(configuration_error)?,
                    record_type: RecordType::try_new(IMPORT_JOB_RECORD_TYPE)
                        .map_err(configuration_error)?,
                    page_size: self.page_size,
                    sort: RecordQuerySort::CreatedAtAscending,
                    after,
                })
                .await?;

            let next_cursor = page.next.clone();
            self.scan_cursors
                .lock()
                .map_err(|_| worker_state_unavailable())?
                .insert(tenant_id.clone(), next_cursor.clone());

            let scanned_jobs =
                u32::try_from(page.records.len()).map_err(|_| worker_state_unavailable())?;
            let mut executed_jobs = 0_u32;
            for record in page.records {
                let job = import_job_from_snapshot(&record)?;
                if job.status() != ImportJobStatus::Executing {
                    continue;
                }
                let snapshot = self.reader.load(&tenant_id, job.job_id()).await?;
                let context = worker_context(
                    &tenant_id,
                    &self.actor_id,
                    job.job_id().as_str(),
                    self.clock.now_unix_nanos(),
                )?;
                self.coordinator.execute_next(&context, &snapshot).await?;
                executed_jobs = executed_jobs.saturating_add(1);
            }

            Ok(ImportExecutionTenantCycle {
                scanned_jobs,
                executed_jobs,
                has_more: next_cursor.is_some(),
            })
        })
    }
}

fn worker_context(
    tenant_id: &TenantId,
    actor_id: &ActorId,
    job_id: &str,
    now_unix_nanos: i64,
) -> Result<ModuleExecutionContext, SdkError> {
    Ok(ModuleExecutionContext {
        module_id: ModuleId::try_new(CUSTOMER_DATA_OPERATIONS_MODULE_ID)
            .map_err(configuration_error)?,
        execution: ExecutionContext {
            tenant_id: tenant_id.clone(),
            actor_id: actor_id.clone(),
            request_id: RequestId::try_new(job_id).map_err(configuration_error)?,
            correlation_id: CorrelationId::try_new(job_id).map_err(configuration_error)?,
            causation_id: CausationId::try_new(job_id).map_err(configuration_error)?,
            trace_id: TraceId::try_new(job_id).map_err(configuration_error)?,
            capability_id: CapabilityId::try_new(IMPORT_EXECUTION_WORKER_CAPABILITY_ID)
                .map_err(configuration_error)?,
            capability_version: CapabilityVersion::try_new(
                IMPORT_EXECUTION_WORKER_CAPABILITY_VERSION,
            )
            .map_err(configuration_error)?,
            idempotency_key: IdempotencyKey::try_new(job_id).map_err(configuration_error)?,
            business_transaction_id: BusinessTransactionId::try_new(job_id)
                .map_err(configuration_error)?,
            schema_version: SchemaVersion::try_new(IMPORT_EXECUTION_WORKER_CAPABILITY_VERSION)
                .map_err(configuration_error)?,
            request_started_at_unix_nanos: now_unix_nanos,
        },
    })
}

fn configuration_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_IMPORT_EXECUTION_WORKER_CONFIGURATION_INVALID",
        crm_module_sdk::ErrorCategory::Internal,
        false,
        "The customer-data import execution worker is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}

fn worker_state_unavailable() -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_IMPORT_EXECUTION_WORKER_STATE_UNAVAILABLE",
        crm_module_sdk::ErrorCategory::Unavailable,
        true,
        "The customer-data import execution worker is temporarily unavailable.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_context_uses_a_private_worker_identity_and_internal_cycle_coordinate() {
        let tenant_id = TenantId::try_new("tenant-worker-test").unwrap();
        let actor_id = ActorId::try_new(IMPORT_EXECUTION_WORKER_ACTOR_ID).unwrap();
        let context = worker_context(&tenant_id, &actor_id, "import-job-worker-test", 123).unwrap();

        assert_eq!(context.execution.tenant_id, tenant_id);
        assert_eq!(context.execution.actor_id, actor_id);
        assert_eq!(
            context.execution.capability_id.as_str(),
            IMPORT_EXECUTION_WORKER_CAPABILITY_ID
        );
        assert_eq!(
            context.execution.capability_version.as_str(),
            IMPORT_EXECUTION_WORKER_CAPABILITY_VERSION
        );
        assert_eq!(context.execution.request_started_at_unix_nanos, 123);
    }
}
