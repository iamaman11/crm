use crate::{
    PartyQualitySource, PartyQualitySourceRequest, PostgresPartyEvaluationStageSink,
    worker_context::{evaluation_worker_context, worker_actor_id},
};
use crm_core_data::{
    PostgresDataStore, RecordListQuery, RecordQueryContinuation, RecordQuerySort,
};
use crm_data_quality::PartyEvaluationJobStatus;
use crm_data_quality_capability_adapter::{
    PARTY_EVALUATION_JOB_RECORD_TYPE, party_evaluation_job_from_snapshot,
};
use crm_module_sdk::{
    ActorId, Clock, ErrorCategory, ModuleId, PortFuture, RecordType, SdkError, TenantId,
};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

pub const DEFAULT_EVALUATION_STAGE_SCAN_PAGE_SIZE: u32 = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluationStageTenantCycle {
    pub scanned_jobs: u32,
    pub staged_jobs: u32,
    pub deferred_jobs: u32,
    pub has_more: bool,
}

#[derive(Clone)]
pub struct PartyEvaluationStageWorker {
    store: PostgresDataStore,
    source: Arc<dyn PartyQualitySource>,
    sink: Arc<PostgresPartyEvaluationStageSink>,
    clock: Arc<dyn Clock>,
    actor_id: ActorId,
    page_size: u32,
    scan_cursors: Arc<Mutex<BTreeMap<TenantId, Option<RecordQueryContinuation>>>>,
}

impl PartyEvaluationStageWorker {
    pub fn new(
        store: PostgresDataStore,
        source: Arc<dyn PartyQualitySource>,
        sink: Arc<PostgresPartyEvaluationStageSink>,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, SdkError> {
        Self::try_with_page_size(
            store,
            source,
            sink,
            clock,
            worker_actor_id()?,
            DEFAULT_EVALUATION_STAGE_SCAN_PAGE_SIZE,
        )
    }

    pub fn try_with_page_size(
        store: PostgresDataStore,
        source: Arc<dyn PartyQualitySource>,
        sink: Arc<PostgresPartyEvaluationStageSink>,
        clock: Arc<dyn Clock>,
        actor_id: ActorId,
        page_size: u32,
    ) -> Result<Self, SdkError> {
        if page_size == 0 || page_size > crm_core_data::MAXIMUM_RECORD_QUERY_PAGE_SIZE {
            return Err(SdkError::invalid_argument(
                "data_quality.evaluation_worker.page_size",
                "Evaluation worker page size is invalid",
            ));
        }
        Ok(Self {
            store,
            source,
            sink,
            clock,
            actor_id,
            page_size,
            scan_cursors: Arc::new(Mutex::new(BTreeMap::new())),
        })
    }

    pub fn run_tenant_cycle<'a>(
        &'a self,
        tenant_id: TenantId,
    ) -> PortFuture<'a, Result<EvaluationStageTenantCycle, SdkError>> {
        Box::pin(async move {
            let after = self
                .scan_cursors
                .lock()
                .map_err(|_| worker_unavailable())?
                .get(&tenant_id)
                .cloned()
                .flatten();
            let page = self
                .store
                .list_records_for_query(&RecordListQuery {
                    tenant_id: tenant_id.clone(),
                    owner_module_id: ModuleId::try_new(crm_data_quality::MODULE_ID)
                        .map_err(config_error)?,
                    record_type: RecordType::try_new(PARTY_EVALUATION_JOB_RECORD_TYPE)
                        .map_err(config_error)?,
                    page_size: self.page_size,
                    sort: RecordQuerySort::CreatedAtAscending,
                    after,
                })
                .await?;
            let next = page.next.clone();
            self.scan_cursors
                .lock()
                .map_err(|_| worker_unavailable())?
                .insert(tenant_id.clone(), next.clone());

            let scanned_jobs = u32::try_from(page.records.len()).map_err(|_| worker_unavailable())?;
            let mut staged_jobs = 0_u32;
            let mut deferred_jobs = 0_u32;
            for record in page.records {
                let job = party_evaluation_job_from_snapshot(&record)?;
                if job.status() != PartyEvaluationJobStatus::Created {
                    continue;
                }
                let now = self.clock.now_unix_nanos();
                let source = match self
                    .source
                    .get(PartyQualitySourceRequest {
                        tenant_id: &tenant_id,
                        actor_id: &self.actor_id,
                        request_identity: job.job_id().as_str(),
                        party_id: job.party_id(),
                        request_started_at_unix_nanos: now,
                    })
                    .await
                {
                    Ok(source) => source,
                    Err(error) if error.category == ErrorCategory::NotFound => {
                        deferred_jobs = deferred_jobs.saturating_add(1);
                        continue;
                    }
                    Err(error) => return Err(error),
                };
                let context = evaluation_worker_context(
                    &tenant_id,
                    &self.actor_id,
                    job.job_id().as_str(),
                    now,
                )?;
                self.sink.stage(&context, &job, record.version, &source).await?;
                staged_jobs = staged_jobs.saturating_add(1);
            }
            Ok(EvaluationStageTenantCycle {
                scanned_jobs,
                staged_jobs,
                deferred_jobs,
                has_more: next.is_some(),
            })
        })
    }
}

impl std::fmt::Debug for PartyEvaluationStageWorker {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PartyEvaluationStageWorker")
            .field("store", &self.store)
            .field("source", &"dyn PartyQualitySource")
            .field("sink", &self.sink)
            .field("actor_id", &self.actor_id)
            .field("page_size", &self.page_size)
            .finish()
    }
}

fn config_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    SdkError::new(
        "DATA_QUALITY_EVALUATION_WORKER_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Data Quality evaluation worker is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}

fn worker_unavailable() -> SdkError {
    SdkError::new(
        "DATA_QUALITY_EVALUATION_WORKER_STATE_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "The Data Quality evaluation worker is temporarily unavailable.",
    )
}
