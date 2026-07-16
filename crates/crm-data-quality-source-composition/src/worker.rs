use crate::{
    PartyQualitySource, PartyQualitySourceRequest, PostgresPartyEvaluationMaterializationSink,
    PostgresPartyEvaluationStageSink,
    worker_context::{evaluation_worker_context, worker_actor_id},
};
use crm_capability_plan_support::{PersistedPayloadContract, persisted_json_bytes_with_data_class};
use crm_core_data::{
    PostgresDataStore, RecordGetQuery, RecordListQuery, RecordQueryContinuation, RecordQuerySort,
};
use crm_data_quality::{
    PARTY_EVALUATION_INPUT_RECORD_TYPE, PARTY_EVALUATION_INPUT_STATE_MAXIMUM_BYTES,
    PARTY_EVALUATION_INPUT_STATE_RETENTION_POLICY_ID, PARTY_EVALUATION_INPUT_STATE_SCHEMA_ID,
    PARTY_EVALUATION_INPUT_STATE_SCHEMA_VERSION, PartyEvaluationInputSnapshot,
    PartyEvaluationJobStatus, decode_party_evaluation_input_state,
    party_evaluation_input_state_descriptor_hash,
};
use crm_data_quality_capability_adapter::{
    PARTY_COMPLETENESS_PROFILE_VERSION_RECORD_TYPE, PARTY_EVALUATION_JOB_RECORD_TYPE,
    PARTY_RULE_SET_VERSION_RECORD_TYPE, party_completeness_profile_from_immutable_snapshot,
    party_evaluation_job_from_snapshot, party_rule_set_from_snapshot,
};
use crm_module_sdk::{
    ActorId, Clock, DataClass, ErrorCategory, ModuleId, PortFuture, RecordId, RecordSnapshot,
    RecordType, SdkError, TenantId,
};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

pub const DEFAULT_EVALUATION_STAGE_SCAN_PAGE_SIZE: u32 = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluationStageTenantCycle {
    pub scanned_jobs: u32,
    pub staged_jobs: u32,
    pub materialized_jobs: u32,
    pub deferred_jobs: u32,
    pub has_more: bool,
}

#[derive(Clone)]
pub struct PartyEvaluationStageWorker {
    store: PostgresDataStore,
    source: Arc<dyn PartyQualitySource>,
    stage_sink: Arc<PostgresPartyEvaluationStageSink>,
    materialization_sink: Option<Arc<PostgresPartyEvaluationMaterializationSink>>,
    clock: Arc<dyn Clock>,
    actor_id: ActorId,
    page_size: u32,
    scan_cursors: Arc<Mutex<BTreeMap<TenantId, Option<RecordQueryContinuation>>>>,
}

impl PartyEvaluationStageWorker {
    pub fn new(
        store: PostgresDataStore,
        source: Arc<dyn PartyQualitySource>,
        stage_sink: Arc<PostgresPartyEvaluationStageSink>,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, SdkError> {
        Self::try_with_page_size(
            store,
            source,
            stage_sink,
            clock,
            worker_actor_id()?,
            DEFAULT_EVALUATION_STAGE_SCAN_PAGE_SIZE,
        )
    }

    pub fn new_with_materialization(
        store: PostgresDataStore,
        source: Arc<dyn PartyQualitySource>,
        stage_sink: Arc<PostgresPartyEvaluationStageSink>,
        materialization_sink: Arc<PostgresPartyEvaluationMaterializationSink>,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, SdkError> {
        Self::build(
            store,
            source,
            stage_sink,
            Some(materialization_sink),
            clock,
            worker_actor_id()?,
            DEFAULT_EVALUATION_STAGE_SCAN_PAGE_SIZE,
        )
    }

    pub fn try_with_page_size(
        store: PostgresDataStore,
        source: Arc<dyn PartyQualitySource>,
        stage_sink: Arc<PostgresPartyEvaluationStageSink>,
        clock: Arc<dyn Clock>,
        actor_id: ActorId,
        page_size: u32,
    ) -> Result<Self, SdkError> {
        Self::build(store, source, stage_sink, None, clock, actor_id, page_size)
    }

    fn build(
        store: PostgresDataStore,
        source: Arc<dyn PartyQualitySource>,
        stage_sink: Arc<PostgresPartyEvaluationStageSink>,
        materialization_sink: Option<Arc<PostgresPartyEvaluationMaterializationSink>>,
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
            stage_sink,
            materialization_sink,
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
                    owner_module_id: data_quality_module_id()?,
                    record_type: record_type(PARTY_EVALUATION_JOB_RECORD_TYPE)?,
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

            let scanned_jobs =
                u32::try_from(page.records.len()).map_err(|_| worker_unavailable())?;
            let mut staged_jobs = 0_u32;
            let mut materialized_jobs = 0_u32;
            let mut deferred_jobs = 0_u32;
            for record in page.records {
                let job = party_evaluation_job_from_snapshot(&record)?;
                match job.status() {
                    PartyEvaluationJobStatus::Created => {
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
                        self.stage_sink
                            .stage(&context, &job, record.version, &source)
                            .await?;
                        staged_jobs = staged_jobs.saturating_add(1);
                    }
                    PartyEvaluationJobStatus::Staged if !job.outcomes_materialized() => {
                        let Some(sink) = &self.materialization_sink else {
                            continue;
                        };
                        let input = self.load_evaluation_input(&tenant_id, job.job_id()).await?;
                        let rule_set_snapshot = self
                            .load_owned_record(
                                &tenant_id,
                                PARTY_RULE_SET_VERSION_RECORD_TYPE,
                                job.rule_set_version_id(),
                            )
                            .await?;
                        let rule_set = party_rule_set_from_snapshot(&rule_set_snapshot)?;
                        let profile_snapshot = self
                            .load_owned_record(
                                &tenant_id,
                                PARTY_COMPLETENESS_PROFILE_VERSION_RECORD_TYPE,
                                job.profile_version_id(),
                            )
                            .await?;
                        let profile = party_completeness_profile_from_immutable_snapshot(
                            &profile_snapshot,
                            &rule_set,
                        )?;
                        let now = self.clock.now_unix_nanos();
                        let context = evaluation_worker_context(
                            &tenant_id,
                            &self.actor_id,
                            job.job_id().as_str(),
                            now,
                        )?;
                        sink.materialize(&context, &job, record.version, rule_set, profile, input)
                            .await?;
                        materialized_jobs = materialized_jobs.saturating_add(1);
                    }
                    PartyEvaluationJobStatus::Staged | PartyEvaluationJobStatus::Completed => {}
                }
            }
            Ok(EvaluationStageTenantCycle {
                scanned_jobs,
                staged_jobs,
                materialized_jobs,
                deferred_jobs,
                has_more: next.is_some(),
            })
        })
    }

    async fn load_evaluation_input(
        &self,
        tenant_id: &TenantId,
        job_id: &RecordId,
    ) -> Result<PartyEvaluationInputSnapshot, SdkError> {
        let snapshot = self
            .load_owned_record(
                tenant_id,
                PARTY_EVALUATION_INPUT_RECORD_TYPE,
                job_id.as_str(),
            )
            .await?;
        if snapshot.version != 1 {
            return Err(materialization_state_invalid(
                "evaluation input record is not immutable version one",
            ));
        }
        let bytes = persisted_json_bytes_with_data_class(
            &snapshot,
            PersistedPayloadContract {
                owner: crm_data_quality::MODULE_ID,
                schema_id: PARTY_EVALUATION_INPUT_STATE_SCHEMA_ID,
                schema_version: PARTY_EVALUATION_INPUT_STATE_SCHEMA_VERSION,
                descriptor_hash: party_evaluation_input_state_descriptor_hash(),
                maximum_size_bytes: PARTY_EVALUATION_INPUT_STATE_MAXIMUM_BYTES,
                retention_policy_id: PARTY_EVALUATION_INPUT_STATE_RETENTION_POLICY_ID,
            },
            DataClass::Personal,
        )?;
        let input = decode_party_evaluation_input_state(bytes)?;
        if input.job_id() != job_id
            || snapshot.reference.record_id.as_str() != input.job_id().as_str()
        {
            return Err(materialization_state_invalid(
                "evaluation input identity differs from its record",
            ));
        }
        Ok(input)
    }

    async fn load_owned_record(
        &self,
        tenant_id: &TenantId,
        record_type_value: &str,
        record_id_value: &str,
    ) -> Result<RecordSnapshot, SdkError> {
        self.store
            .get_record_for_query(&RecordGetQuery {
                tenant_id: tenant_id.clone(),
                owner_module_id: data_quality_module_id()?,
                record_type: record_type(record_type_value)?,
                record_id: RecordId::try_new(record_id_value).map_err(config_error)?,
            })
            .await?
            .ok_or_else(|| {
                materialization_state_invalid("required immutable evaluation evidence is missing")
            })
    }
}

impl std::fmt::Debug for PartyEvaluationStageWorker {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PartyEvaluationStageWorker")
            .field("store", &self.store)
            .field("source", &"dyn PartyQualitySource")
            .field("stage_sink", &self.stage_sink)
            .field(
                "materialization_sink",
                &self.materialization_sink.as_ref().map(|_| "configured"),
            )
            .field("actor_id", &self.actor_id)
            .field("page_size", &self.page_size)
            .finish()
    }
}

fn data_quality_module_id() -> Result<ModuleId, SdkError> {
    ModuleId::try_new(crm_data_quality::MODULE_ID).map_err(config_error)
}

fn record_type(value: &str) -> Result<RecordType, SdkError> {
    RecordType::try_new(value).map_err(config_error)
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

fn materialization_state_invalid(reference: &str) -> SdkError {
    SdkError::new(
        "DATA_QUALITY_EVALUATION_MATERIALIZATION_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "The durable Party evaluation evidence is invalid.",
    )
    .with_internal_reference(reference)
}

fn worker_unavailable() -> SdkError {
    SdkError::new(
        "DATA_QUALITY_EVALUATION_WORKER_STATE_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "The Data Quality evaluation worker is temporarily unavailable.",
    )
}
