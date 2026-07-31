#![forbid(unsafe_code)]

use crm_application_composition::{
    ActivationGatedQueryValidator, ModuleActivationPort, ModuleContributionSet,
};
use crm_capability_runtime::CapabilityDefinition;
use crm_core_data::PostgresDataStore;
use crm_customer_360_query_adapter::{
    Customer360QueryAdapter,
    query_capability_definitions as adapter_query_capability_definitions,
};
use crm_module_sdk::{ErrorCategory, SdkError};
use crm_query_runtime::{
    QueryExecutor, QuerySemanticValidator, QueryVisibilityAuthorizer,
};
use std::fmt;
use std::sync::Arc;

#[derive(Clone)]
pub struct Customer360ProductionDependencies {
    pub store: PostgresDataStore,
    pub activation: Arc<dyn ModuleActivationPort>,
    pub visibility_authorizer: Arc<dyn QueryVisibilityAuthorizer>,
}

pub fn query_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    adapter_query_capability_definitions()
}

pub fn build_contribution(
    dependencies: Customer360ProductionDependencies,
) -> Result<ModuleContributionSet, SdkError> {
    let Customer360ProductionDependencies {
        store,
        activation,
        visibility_authorizer,
    } = dependencies;
    let mut contributions = ModuleContributionSet::new();
    let queries = Arc::new(Customer360QueryAdapter::new(store, visibility_authorizer));
    let validator: Arc<dyn QuerySemanticValidator> = Arc::new(
        ActivationGatedQueryValidator::new(activation, queries.clone()),
    );
    let executor: Arc<dyn QueryExecutor> = queries;
    contributions
        .add_queries(query_capability_definitions()?, validator, executor)
        .map_err(production_composition_error)?;
    Ok(contributions)
}

fn production_composition_error(error: impl fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_360_PRODUCTION_COMPOSITION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Customer 360 production contribution is invalid.",
    )
    .with_internal_reference(error.to_string())
}
