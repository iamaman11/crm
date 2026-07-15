use crate::{PostgresPartyExportSelectionReader, PostgresPartyExportSelectionSink};
use crm_core_data::{PostgresDataStore, RecordListQuery, RecordQueryContinuation, RecordQuerySort};
use crm_customer_data_operations::{PartyExportJobStatus, PartyExportKindFilter};
use crm_customer_data_operations_capability_adapter::{
    EXPORT_JOB_RECORD_TYPE, MODULE_ID as CUSTOMER_DATA_OPERATIONS_MODULE_ID,
    export_job_from_snapshot,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, Clock,
    CorrelationId, ExecutionContext, IdempotencyKey, ModuleExecutionContext, ModuleId, PortFuture,
    RecordId, RecordType, RequestId, SchemaVersion, SdkError, TenantId, TraceId,
};
use crm_proto_contracts::crm::{
    customer::v1 as customer, customer_data_operations::v1 as export_wire,
};
use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex};

pub const DEFAULT_EXPORT_SELECTION_SCAN_PAGE_SIZE: u32 = 100;
pub const MAXIMUM_EXPORT_SELECTION_SOURCE_PAGE_SIZE: u32 = 1_000;
pub const EXPORT_SELECTION_WORKER_ACTOR_ID: &str = "crm-api-export-selection-worker";
pub const EXPORT_SELECTION_WORKER_CAPABILITY_ID: &str =
    "customer_data.export.party.internal.selection_cycle";
pub const EXPORT_SELECTION_WORKER_CAPABILITY_VERSION: &str = "1.0.0";

const _: () = assert!(DEFAULT_EXPORT_SELECTION_SCAN_PAGE_SIZE > 0);
const _: () = assert!(
    DEFAULT_EXPORT_SELECTION_SCAN_PAGE_SIZE <= crm_core_data::MAXIMUM_RECORD_QUERY_PAGE_SIZE
);
const _: () = assert!(MAXIMUM_EXPORT_SELECTION_SOURCE_PAGE_SIZE > 0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartyExportSelectionSourceKind {
    Person,
    Organization,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyExportSelectionSourceContinuation {
    pub sort_value: String,
    pub record_id: RecordId,
}

#[derive(Debug, Clone)]
pub struct PartyExportSelectionSourceRequest<'a> {
    pub tenant_id: &'a TenantId,
    pub actor_id: &'a ActorId,
    pub job_id: &'a str,
    pub selection_cutoff_unix_nanos: i64,
    pub kind: Option<PartyExportSelectionSourceKind>,
    pub page_size: u32,
    pub after: Option<PartyExportSelectionSourceContinuation>,
    pub request_started_at_unix_nanos: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyExportSelectionSourceCandidate {
    pub party_id: RecordId,
    pub resource_version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyExportSelectionSourcePage {
    pub candidates: Vec<PartyExportSelectionSourceCandidate>,
    pub next: Option<PartyExportSelectionSourceContinuation>,
}

/// Worker-private governed Party selection source.
///
/// The execution-composition crate intentionally knows nothing about the concrete Party query
/// adapter. Production runtime composition supplies an implementation that re-enters the Party-owned
/// governed query path with tenant/RLS isolation and live visibility checks.
pub trait PartyExportSelectionSource: Send + Sync {
    fn list_page<'a>(
        &'a self,
        request: PartyExportSelectionSourceRequest<'a>,
    ) -> PortFuture<'a, Result<PartyExportSelectionSourcePage, SdkError>>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportSelectionTenantCycle {
    pub scanned_jobs: u32,
    pub progressed_jobs: u32,
    pub finalized_jobs: u32,
    pub has_more: bool,
}

#[derive(Clone)]
pub struct PartyExportSelectionWorker {
    store: PostgresDataStore,
    reader: Arc<PostgresPartyExportSelectionReader>,
    sink: Arc<PostgresPartyExportSelectionSink>,
    source: Arc<dyn PartyExportSelectionSource>,
    clock: Arc<dyn Clock>,
    actor_id: ActorId,
    page_size: u32,
    scan_cursors: Arc<Mutex<BTreeMap<TenantId, Option<RecordQueryContinuation>>>>,
}

impl fmt::Debug for PartyExportSelectionWorker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PartyExportSelectionWorker")
            .field("store", &self.store)
            .field("reader", &self.reader)
            .field("sink", &self.sink)
            .field("source", &"dyn PartyExportSelectionSource")
            .field("clock", &"dyn Clock")
            .field("actor_id", &self.actor_id)
            .field("page_size", &self.page_size)
            .finish()
    }
}

impl PartyExportSelectionWorker {
    pub fn new(
        store: PostgresDataStore,
        reader: Arc<PostgresPartyExportSelectionReader>,
        sink: Arc<PostgresPartyExportSelectionSink>,
        source: Arc<dyn PartyExportSelectionSource>,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, SdkError> {
        Self::try_with_page_size(
            store,
            reader,
            sink,
            source,
            clock,
            ActorId::try_new(EXPORT_SELECTION_WORKER_ACTOR_ID).map_err(configuration_error)?,
            DEFAULT_EXPORT_SELECTION_SCAN_PAGE_SIZE,
        )
    }

    pub fn try_with_page_size(
        store: PostgresDataStore,
        reader: Arc<PostgresPartyExportSelectionReader>,
        sink: Arc<PostgresPartyExportSelectionSink>,
        source: Arc<dyn PartyExportSelectionSource>,
        clock: Arc<dyn Clock>,
        actor_id: ActorId,
        page_size: u32,
    ) -> Result<Self, SdkError> {
        if page_size == 0 || page_size > crm_core_data::MAXIMUM_RECORD_QUERY_PAGE_SIZE {
            return Err(SdkError::invalid_argument(
                "customer_data.export.selection_worker.page_size",
                "Export selection worker page size is invalid",
            ));
        }
        Ok(Self {
            store,
            reader,
            sink,
            source,
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
    ) -> PortFuture<'a, Result<ExportSelectionTenantCycle, SdkError>> {
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
                    record_type: RecordType::try_new(EXPORT_JOB_RECORD_TYPE)
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
            let mut progressed_jobs = 0_u32;
            let mut finalized_jobs = 0_u32;
            for record in page.records {
                let job = export_job_from_snapshot(&record)?;
                if job.status() != PartyExportJobStatus::Selecting {
                    continue;
                }

                let evidence = self.reader.load_evidence(&tenant_id, &job).await?;
                let selected_resources = evidence
                    .progress
                    .next_manifest_position()
                    .checked_sub(1)
                    .ok_or_else(worker_state_unavailable)?;
                let terminal = evidence.progress.source_exhausted()
                    || selected_resources == evidence.progress.maximum_resources();
                let now_unix_nanos = self.clock.now_unix_nanos();
                let context = worker_context(
                    &tenant_id,
                    &self.actor_id,
                    job.job_id().as_str(),
                    now_unix_nanos,
                )?;

                if terminal {
                    let summary = self
                        .reader
                        .prove_finalization(&tenant_id, &job, &evidence)
                        .await?;
                    self.sink
                        .finalize(&context, &job, &evidence.progress, &summary)
                        .await?;
                    finalized_jobs = finalized_jobs.saturating_add(1);
                    continue;
                }

                let remaining = evidence
                    .progress
                    .maximum_resources()
                    .checked_sub(selected_resources)
                    .ok_or_else(worker_state_unavailable)?;
                let selection_page_size = remaining.min(MAXIMUM_EXPORT_SELECTION_SOURCE_PAGE_SIZE);
                if selection_page_size == 0 {
                    return Err(worker_state_unavailable());
                }

                let kind = job.specification().scope().kind_filter().map(|kind| match kind {
                    PartyExportKindFilter::Person => PartyExportSelectionSourceKind::Person,
                    PartyExportKindFilter::Organization => {
                        PartyExportSelectionSourceKind::Organization
                    }
                });
                let after = evidence.progress.continuation().map(|continuation| {
                    PartyExportSelectionSourceContinuation {
                        sort_value: continuation.sort_value().to_owned(),
                        record_id: continuation.record_id().clone(),
                    }
                });
                let result = self
                    .source
                    .list_page(PartyExportSelectionSourceRequest {
                        tenant_id: &tenant_id,
                        actor_id: &self.actor_id,
                        job_id: job.job_id().as_str(),
                        selection_cutoff_unix_nanos: evidence
                            .boundary
                            .selection_cutoff_unix_nanos(),
                        kind,
                        page_size: selection_page_size,
                        after,
                        request_started_at_unix_nanos: now_unix_nanos,
                    })
                    .await?;
                let candidates = result
                    .candidates
                    .into_iter()
                    .map(|candidate| export_wire::PartyExportSelectionCandidate {
                        party_ref: Some(customer::PartyRef {
                            party_id: candidate.party_id.as_str().to_owned(),
                        }),
                        resource_version: Some(customer::CustomerResourceVersion {
                            version: candidate.resource_version,
                            created_at: None,
                            updated_at: None,
                        }),
                    })
                    .collect();
                let source_after =
                    result
                        .next
                        .map(|continuation| export_wire::PartyExportSourceContinuation {
                            sort_value: continuation.sort_value,
                            record_id: continuation.record_id.as_str().to_owned(),
                        });
                self.sink
                    .commit_page(
                        &context,
                        job.job_id(),
                        &evidence.progress,
                        candidates,
                        source_after,
                    )
                    .await?;
                progressed_jobs = progressed_jobs.saturating_add(1);
            }

            Ok(ExportSelectionTenantCycle {
                scanned_jobs,
                progressed_jobs,
                finalized_jobs,
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
            capability_id: CapabilityId::try_new(EXPORT_SELECTION_WORKER_CAPABILITY_ID)
                .map_err(configuration_error)?,
            capability_version: CapabilityVersion::try_new(
                EXPORT_SELECTION_WORKER_CAPABILITY_VERSION,
            )
            .map_err(configuration_error)?,
            idempotency_key: IdempotencyKey::try_new(job_id).map_err(configuration_error)?,
            business_transaction_id: BusinessTransactionId::try_new(job_id)
                .map_err(configuration_error)?,
            schema_version: SchemaVersion::try_new(EXPORT_SELECTION_WORKER_CAPABILITY_VERSION)
                .map_err(configuration_error)?,
            request_started_at_unix_nanos: now_unix_nanos,
        },
    })
}

fn configuration_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_SELECTION_WORKER_CONFIGURATION_INVALID",
        crm_module_sdk::ErrorCategory::Internal,
        false,
        "The customer export selection worker is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}

fn worker_state_unavailable() -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_SELECTION_WORKER_STATE_UNAVAILABLE",
        crm_module_sdk::ErrorCategory::Unavailable,
        true,
        "The customer export selection worker is temporarily unavailable.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_context_uses_private_export_selection_identity() {
        let tenant_id = TenantId::try_new("tenant-export-selection-worker-test").unwrap();
        let actor_id = ActorId::try_new(EXPORT_SELECTION_WORKER_ACTOR_ID).unwrap();
        let context =
            worker_context(&tenant_id, &actor_id, "export-selection-job-test", 123).unwrap();
        assert_eq!(context.execution.tenant_id, tenant_id);
        assert_eq!(context.execution.actor_id, actor_id);
        assert_eq!(
            context.execution.capability_id.as_str(),
            EXPORT_SELECTION_WORKER_CAPABILITY_ID
        );
        assert_eq!(context.execution.request_started_at_unix_nanos, 123);
    }

    #[test]
    fn source_page_limit_is_bounded_by_core_query_capacity() {
        assert!(MAXIMUM_EXPORT_SELECTION_SOURCE_PAGE_SIZE > 0);
        assert!(
            MAXIMUM_EXPORT_SELECTION_SOURCE_PAGE_SIZE
                <= crm_core_data::MAXIMUM_RECORD_QUERY_PAGE_SIZE
        );
    }
}
