#![forbid(unsafe_code)]

use crate::{DataQualityAggregatePlanner, DataQualityCapabilityExecutor};
use crm_application_composition::{
    ActivationGatedMutationValidator, ActivationGatedQueryValidator, ModuleActivationPort,
    ModuleContributionSet, NoopMutationSemanticValidator,
};
use crm_capability_runtime::{
    CapabilityAuthorizer, CapabilityDefinition, CapabilitySemanticValidator,
    TransactionalCapabilityExecutor,
};
use crm_core_data::{PostgresDataStore, PostgresTransactionalAggregateExecutor};
use crm_data_quality_capability_adapter::capability_definitions as adapter_mutation_capability_definitions;
use crm_data_quality_query_adapter::{
    DataQualityQueryAdapter, query_capability_definitions as adapter_query_capability_definitions,
};
use crm_module_sdk::{ErrorCategory, SdkError};
use crm_parties_capability_adapter::PartyCapabilityPlanner;
use crm_query_runtime::{
    QueryAuthorizer, QueryExecutor, QuerySemanticValidator, QueryVisibilityAuthorizer,
};
use std::fmt;
use std::sync::Arc;

#[derive(Clone)]
pub struct DataQualityProductionDependencies {
    pub store: PostgresDataStore,
    pub activation: Arc<dyn ModuleActivationPort>,
    pub capability_authorizer: Arc<dyn CapabilityAuthorizer>,
    pub query_authorizer: Arc<dyn QueryAuthorizer>,
    pub visibility_authorizer: Arc<dyn QueryVisibilityAuthorizer>,
}

pub fn mutation_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    adapter_mutation_capability_definitions()
}

pub fn query_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    adapter_query_capability_definitions()
}

pub fn build_contribution(
    dependencies: DataQualityProductionDependencies,
) -> Result<ModuleContributionSet, SdkError> {
    let DataQualityProductionDependencies {
        store,
        activation,
        capability_authorizer,
        query_authorizer,
        visibility_authorizer,
    } = dependencies;
    let mut contributions = ModuleContributionSet::new();

    let fallback: Arc<dyn TransactionalCapabilityExecutor> =
        Arc::new(PostgresTransactionalAggregateExecutor::new(
            store.clone(),
            Arc::new(DataQualityAggregatePlanner),
        ));
    let party_update_executor: Arc<dyn TransactionalCapabilityExecutor> =
        Arc::new(PostgresTransactionalAggregateExecutor::new(
            store.clone(),
            Arc::new(PartyCapabilityPlanner),
        ));
    let mutation_executor: Arc<dyn TransactionalCapabilityExecutor> =
        Arc::new(DataQualityCapabilityExecutor::new(
            store.clone(),
            fallback,
            party_update_executor,
            activation.clone(),
            capability_authorizer,
            query_authorizer,
        ));
    let mutation_validator: Arc<dyn CapabilitySemanticValidator> =
        Arc::new(ActivationGatedMutationValidator::new(
            activation.clone(),
            Arc::new(NoopMutationSemanticValidator),
        ));
    contributions
        .add_mutations(
            mutation_capability_definitions()?,
            mutation_validator,
            mutation_executor,
        )
        .map_err(production_composition_error)?;

    let queries = Arc::new(DataQualityQueryAdapter::new(store, visibility_authorizer));
    let query_validator: Arc<dyn QuerySemanticValidator> = Arc::new(
        ActivationGatedQueryValidator::new(activation, queries.clone()),
    );
    let query_executor: Arc<dyn QueryExecutor> = queries;
    contributions
        .add_queries(
            query_capability_definitions()?,
            query_validator,
            query_executor,
        )
        .map_err(production_composition_error)?;

    Ok(contributions)
}

fn production_composition_error(error: impl fmt::Display) -> SdkError {
    SdkError::new(
        "DATA_QUALITY_PRODUCTION_COMPOSITION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Data Quality production contribution is invalid.",
    )
    .with_internal_reference(error.to_string())
}
