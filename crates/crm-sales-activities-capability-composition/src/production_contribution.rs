#![forbid(unsafe_code)]

use crate::{SalesActivitiesCapabilityPlannerRouter, capability_definitions};
use crm_application_composition::{
    ActivationGatedMutationValidator, ActivationGatedQueryValidator, ModuleActivationPort,
    ModuleContributionSet, NoopMutationSemanticValidator,
};
use crm_capability_runtime::{
    CapabilityDefinition, CapabilitySemanticValidator, TransactionalCapabilityExecutor,
};
use crm_core_data::{PostgresDataStore, PostgresTransactionalAggregateExecutor};
use crm_module_sdk::{ErrorCategory, SdkError};
use crm_query_runtime::{
    CursorCodec, QueryExecutor, QuerySemanticValidator, QueryVisibilityAuthorizer,
};
use crm_sales_activities_query_adapter::{
    SalesActivitiesQueryAdapter,
    query_capability_definitions as adapter_query_capability_definitions,
};
use std::fmt;
use std::sync::Arc;

#[derive(Clone)]
pub struct SalesActivitiesProductionDependencies {
    pub store: PostgresDataStore,
    pub activation: Arc<dyn ModuleActivationPort>,
    pub visibility_authorizer: Arc<dyn QueryVisibilityAuthorizer>,
    pub cursor_key: [u8; 32],
}

pub fn mutation_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    capability_definitions()
}

pub fn query_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    adapter_query_capability_definitions()
}

pub fn build_contribution(
    dependencies: SalesActivitiesProductionDependencies,
) -> Result<ModuleContributionSet, SdkError> {
    let SalesActivitiesProductionDependencies {
        store,
        activation,
        visibility_authorizer,
        cursor_key,
    } = dependencies;
    let mut contributions = ModuleContributionSet::new();

    let mutation_executor: Arc<dyn TransactionalCapabilityExecutor> =
        Arc::new(PostgresTransactionalAggregateExecutor::new(
            store.clone(),
            Arc::new(SalesActivitiesCapabilityPlannerRouter),
        ));
    let mutation_validator: Arc<dyn CapabilitySemanticValidator> = Arc::new(
        ActivationGatedMutationValidator::new(
            activation.clone(),
            Arc::new(NoopMutationSemanticValidator),
        ),
    );
    contributions
        .add_mutations(
            mutation_capability_definitions()?,
            mutation_validator,
            mutation_executor,
        )
        .map_err(production_composition_error)?;

    let queries = Arc::new(SalesActivitiesQueryAdapter::new(
        store,
        cursor(cursor_key)?,
        visibility_authorizer,
    )?);
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

fn cursor(key: [u8; 32]) -> Result<CursorCodec, SdkError> {
    CursorCodec::new(key).map_err(|error| {
        SdkError::new(
            "SALES_ACTIVITIES_CURSOR_CONFIGURATION_INVALID",
            ErrorCategory::Internal,
            false,
            "The Sales and Activities cursor configuration is invalid.",
        )
        .with_internal_reference(error.to_string())
    })
}

fn production_composition_error(error: impl fmt::Display) -> SdkError {
    SdkError::new(
        "SALES_ACTIVITIES_PRODUCTION_COMPOSITION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Sales and Activities production contribution is invalid.",
    )
    .with_internal_reference(error.to_string())
}
