use crate::GovernedCustomerEnrichmentProviderSource;
use crm_core_data::PostgresDataStore;
use crm_customer_enrichment_provider_process_composition::{
    CustomerEnrichmentProviderProcessWorker, PROVIDER_PROCESS_WORKER_ACTOR_ID,
    ProviderDispatchExecutorPort,
};
use crm_module_sdk::{ActorId, ErrorCategory, SdkError};
use crm_parties_query_adapter::PartyQueryAdapter;
use crm_query_runtime::{CursorCodec, QueryAuthorizer, QueryVisibilityAuthorizer};
use std::fmt;
use std::sync::Arc;

pub struct CustomerEnrichmentProviderProcessDependencies {
    pub store: PostgresDataStore,
    pub executor: Arc<dyn ProviderDispatchExecutorPort>,
    pub query_authorizer: Arc<dyn QueryAuthorizer>,
    pub visibility_authorizer: Arc<dyn QueryVisibilityAuthorizer>,
    pub cursor_key: [u8; 32],
}

impl fmt::Debug for CustomerEnrichmentProviderProcessDependencies {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CustomerEnrichmentProviderProcessDependencies")
            .field("store", &self.store)
            .field("executor", &"dyn ProviderDispatchExecutorPort")
            .field("query_authorizer", &"dyn QueryAuthorizer")
            .field("visibility_authorizer", &"dyn QueryVisibilityAuthorizer")
            .finish_non_exhaustive()
    }
}

pub fn build_customer_enrichment_provider_process(
    dependencies: CustomerEnrichmentProviderProcessDependencies,
) -> Result<Arc<CustomerEnrichmentProviderProcessWorker>, SdkError> {
    let party_queries = Arc::new(PartyQueryAdapter::new(
        dependencies.store.clone(),
        CursorCodec::new(dependencies.cursor_key).map_err(configuration_error)?,
        dependencies.visibility_authorizer,
    )?);
    let source = Arc::new(GovernedCustomerEnrichmentProviderSource::new(
        dependencies.store.clone(),
        party_queries,
        dependencies.query_authorizer,
    ));
    let actor_id =
        ActorId::try_new(PROVIDER_PROCESS_WORKER_ACTOR_ID).map_err(configuration_error)?;
    Ok(Arc::new(CustomerEnrichmentProviderProcessWorker::new(
        dependencies.store,
        source,
        dependencies.executor,
        actor_id,
    )?))
}

fn configuration_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_PROCESS_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Customer Enrichment provider process is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_worker_actor_is_valid() {
        assert!(ActorId::try_new(PROVIDER_PROCESS_WORKER_ACTOR_ID).is_ok());
    }

    #[test]
    fn dependencies_are_thread_safe() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<CustomerEnrichmentProviderProcessDependencies>();
    }
}
