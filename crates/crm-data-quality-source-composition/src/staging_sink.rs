use crm_capability_runtime::CapabilityAuthorizer;
use crm_core_data::PostgresDataStore;
use crm_data_quality::PartyEvaluationJob;
use crm_module_sdk::{ModuleExecutionContext, PortFuture, SdkError};
use std::sync::Arc;

use crate::{
    PartyQualitySourceSnapshot, staging_execute::execute_stage,
    staging_request::prepare_stage_request,
};

#[derive(Clone)]
pub struct PostgresPartyEvaluationStageSink {
    store: PostgresDataStore,
    authorizer: Arc<dyn CapabilityAuthorizer>,
}

impl PostgresPartyEvaluationStageSink {
    pub fn new(store: PostgresDataStore, authorizer: Arc<dyn CapabilityAuthorizer>) -> Self {
        Self { store, authorizer }
    }

    pub fn stage<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        job: &'a PartyEvaluationJob,
        expected_job_version: i64,
        source: &'a PartyQualitySourceSnapshot,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            let prepared =
                prepare_stage_request(context, job, expected_job_version, source)?;
            execute_stage(&self.store, self.authorizer.as_ref(), prepared).await
        })
    }
}

impl std::fmt::Debug for PostgresPartyEvaluationStageSink {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PostgresPartyEvaluationStageSink")
            .field("store", &self.store)
            .field("authorizer", &"dyn CapabilityAuthorizer")
            .finish()
    }
}
