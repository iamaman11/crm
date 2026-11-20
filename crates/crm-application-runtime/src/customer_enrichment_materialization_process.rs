use crm_capability_runtime::{CapabilityAuthorizer, CapabilityExecutionResult, CapabilityRequest};
use crm_core_data::{PostgresDataStore, PostgresImmutableFileArtifactStore};
use crm_customer_enrichment_materialization_adapter::suggestion_materialization_capability_definition;
use crm_customer_enrichment_materialization_composition::{
    CustomerEnrichmentMaterializationProcessWorker,
    GovernedFileProviderSuggestionCandidateEvidenceSource, MATERIALIZATION_PROCESS_WORKER_ACTOR_ID,
    PostgresCustomerEnrichmentSuggestionMaterializationWorker,
    SuggestionMaterializationExecutorPort,
};
use crm_module_sdk::{ActorId, ErrorCategory, PortFuture, SdkError};
use std::fmt;
use std::sync::Arc;

pub struct CustomerEnrichmentMaterializationProcessDependencies {
    pub store: PostgresDataStore,
    pub authorizer: Arc<dyn CapabilityAuthorizer>,
}

impl fmt::Debug for CustomerEnrichmentMaterializationProcessDependencies {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CustomerEnrichmentMaterializationProcessDependencies")
            .field("store", &self.store)
            .field("authorizer", &"dyn CapabilityAuthorizer")
            .finish()
    }
}

pub fn build_customer_enrichment_materialization_process(
    dependencies: CustomerEnrichmentMaterializationProcessDependencies,
) -> Result<CustomerEnrichmentMaterializationProcessWorker, SdkError> {
    let evidence = Arc::new(GovernedFileProviderSuggestionCandidateEvidenceSource::new(
        Arc::new(PostgresImmutableFileArtifactStore::new(
            dependencies.store.clone(),
        )),
    ));
    let executor: Arc<dyn SuggestionMaterializationExecutorPort> =
        Arc::new(AuthorizedSuggestionMaterializationExecutor::new(
            Arc::new(
                PostgresCustomerEnrichmentSuggestionMaterializationWorker::new(
                    dependencies.store.clone(),
                ),
            ),
            dependencies.authorizer,
        ));
    let actor_id = ActorId::try_new(MATERIALIZATION_PROCESS_WORKER_ACTOR_ID)
        .map_err(materialization_configuration_invalid)?;
    CustomerEnrichmentMaterializationProcessWorker::new(
        dependencies.store,
        evidence,
        executor,
        actor_id,
    )
}

#[derive(Clone)]
struct AuthorizedSuggestionMaterializationExecutor {
    inner: Arc<dyn SuggestionMaterializationExecutorPort>,
    authorizer: Arc<dyn CapabilityAuthorizer>,
}

impl AuthorizedSuggestionMaterializationExecutor {
    fn new(
        inner: Arc<dyn SuggestionMaterializationExecutorPort>,
        authorizer: Arc<dyn CapabilityAuthorizer>,
    ) -> Self {
        Self { inner, authorizer }
    }
}

impl fmt::Debug for AuthorizedSuggestionMaterializationExecutor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthorizedSuggestionMaterializationExecutor")
            .field("inner", &"dyn SuggestionMaterializationExecutorPort")
            .field("authorizer", &"dyn CapabilityAuthorizer")
            .finish()
    }
}

impl SuggestionMaterializationExecutorPort for AuthorizedSuggestionMaterializationExecutor {
    fn execute<'a>(
        &'a self,
        request: CapabilityRequest,
    ) -> PortFuture<'a, Result<CapabilityExecutionResult, SdkError>> {
        Box::pin(async move {
            let definition = suggestion_materialization_capability_definition()?;
            let decision = self.authorizer.authorize(&definition, &request).await?;
            if !decision.allowed {
                return Err(SdkError::new(
                    "CUSTOMER_ENRICHMENT_MATERIALIZATION_PERMISSION_DENIED",
                    ErrorCategory::Authorization,
                    false,
                    "The materialization worker is not authorized to execute the internal capability.",
                )
                .with_internal_reference(format!(
                    "decision_id={};reason_code={};policy_version={}",
                    decision.decision_id, decision.reason_code, decision.policy_version
                )));
            }
            self.inner.execute(request).await
        })
    }
}

fn materialization_configuration_invalid(error: impl fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_MATERIALIZATION_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The materialization process is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}
