use crm_capability_runtime::{
    CapabilityAuthorizer, CapabilityDefinition, CapabilityExecutionResult, CapabilityRequest,
    TransactionalCapabilityExecutor,
};
use crm_core_data::{PostgresDataStore, PostgresTransactionalAggregateExecutor};
use crm_customer_enrichment::ProviderAdapterRegistryPort;
use crm_customer_enrichment_capability_adapter::{
    CustomerEnrichmentRequestDispatchPlanner, CustomerEnrichmentRequestReferencePlanner,
};
use crm_customer_enrichment_worker_composition::CustomerEnrichmentProviderWorker;
use crm_module_sdk::{ErrorCategory, PortFuture, SdkError};
use std::fmt;
use std::sync::Arc;

pub struct CustomerEnrichmentProviderWorkerDependencies {
    pub store: PostgresDataStore,
    pub registry: Arc<dyn ProviderAdapterRegistryPort>,
    pub authorizer: Arc<dyn CapabilityAuthorizer>,
}

impl fmt::Debug for CustomerEnrichmentProviderWorkerDependencies {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CustomerEnrichmentProviderWorkerDependencies")
            .field("store", &self.store)
            .field("registry", &"dyn ProviderAdapterRegistryPort")
            .field("authorizer", &"dyn CapabilityAuthorizer")
            .finish()
    }
}

pub fn build_customer_enrichment_provider_worker(
    dependencies: CustomerEnrichmentProviderWorkerDependencies,
) -> Result<CustomerEnrichmentProviderWorker, SdkError> {
    let dispatch: Arc<dyn TransactionalCapabilityExecutor> =
        Arc::new(AuthorizedWorkerCapabilityExecutor::new(
            Arc::new(PostgresTransactionalAggregateExecutor::new(
                dependencies.store.clone(),
                Arc::new(CustomerEnrichmentRequestDispatchPlanner),
            )),
            dependencies.authorizer.clone(),
            "CUSTOMER_ENRICHMENT_DISPATCH_PERMISSION_DENIED",
        ));
    let response: Arc<dyn TransactionalCapabilityExecutor> =
        Arc::new(AuthorizedWorkerCapabilityExecutor::new(
            Arc::new(PostgresTransactionalAggregateExecutor::new(
                dependencies.store,
                Arc::new(CustomerEnrichmentRequestReferencePlanner),
            )),
            dependencies.authorizer,
            "CUSTOMER_ENRICHMENT_RESPONSE_PERMISSION_DENIED",
        ));
    CustomerEnrichmentProviderWorker::try_new(dispatch, response, dependencies.registry)
}

#[derive(Clone)]
struct AuthorizedWorkerCapabilityExecutor {
    inner: Arc<dyn TransactionalCapabilityExecutor>,
    authorizer: Arc<dyn CapabilityAuthorizer>,
    denial_code: &'static str,
}

impl AuthorizedWorkerCapabilityExecutor {
    fn new(
        inner: Arc<dyn TransactionalCapabilityExecutor>,
        authorizer: Arc<dyn CapabilityAuthorizer>,
        denial_code: &'static str,
    ) -> Self {
        Self {
            inner,
            authorizer,
            denial_code,
        }
    }
}

impl fmt::Debug for AuthorizedWorkerCapabilityExecutor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthorizedWorkerCapabilityExecutor")
            .field("inner", &"dyn TransactionalCapabilityExecutor")
            .field("authorizer", &"dyn CapabilityAuthorizer")
            .field("denial_code", &self.denial_code)
            .finish()
    }
}

impl TransactionalCapabilityExecutor for AuthorizedWorkerCapabilityExecutor {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: CapabilityRequest,
    ) -> PortFuture<'a, Result<CapabilityExecutionResult, SdkError>> {
        Box::pin(async move {
            let decision = self.authorizer.authorize(definition, &request).await?;
            if !decision.allowed {
                return Err(SdkError::new(
                    self.denial_code,
                    ErrorCategory::Authorization,
                    false,
                    "The provider worker is not authorized to execute the internal capability.",
                )
                .with_internal_reference(format!(
                    "decision_id={};reason_code={};policy_version={}",
                    decision.decision_id, decision.reason_code, decision.policy_version
                )));
            }
            self.inner.execute(definition, request).await
        })
    }
}
