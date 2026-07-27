#![forbid(unsafe_code)]

//! Owner-owned production contribution for `crm.customer-privacy`.
//!
//! This package is the only supported process-composition entry point for
//! Customer Privacy. It preserves the accepted four mutations and two queries;
//! discovery remains unregistered.

use crm_application_composition::{
    ActivationGatedMutationValidator, ActivationGatedQueryValidator, ModuleActivationPort,
    ModuleContributionSet, NoopMutationSemanticValidator,
};
use crm_capability_runtime::CapabilitySemanticValidator;
use crm_core_data::PostgresDataStore;
pub use crm_customer_privacy_application::{
    mutation_capability_definitions, query_capability_definitions,
};
use crm_customer_privacy_postgres::{
    postgres_case_cancel_executor, postgres_case_create_executor,
    postgres_case_subject_verify_executor, postgres_case_submit_executor,
};
use crm_customer_privacy_query_adapter::CustomerPrivacyQueryAdapter;
use crm_module_sdk::{ErrorCategory, SdkError};
use crm_query_runtime::{
    CursorCodec, QueryExecutor, QuerySemanticValidator, QueryVisibilityAuthorizer,
};
use std::sync::Arc;

#[derive(Clone)]
pub struct CustomerPrivacyProductionDependencies {
    pub store: PostgresDataStore,
    pub activation: Arc<dyn ModuleActivationPort>,
    pub visibility_authorizer: Arc<dyn QueryVisibilityAuthorizer>,
    pub cursor_key: [u8; 32],
}

pub fn build_contribution(
    dependencies: CustomerPrivacyProductionDependencies,
) -> Result<ModuleContributionSet, SdkError> {
    let mutations = mutation_capability_definitions()?;
    if mutations.len() != 4 {
        return Err(configuration_error(
            "Customer Privacy production inventory must contain exactly four mutations",
        ));
    }
    let queries = query_capability_definitions()?;
    if queries.len() != 2 {
        return Err(configuration_error(
            "Customer Privacy production inventory must contain exactly two queries",
        ));
    }

    let mut contributions = ModuleContributionSet::new();
    let executors = [
        postgres_case_create_executor(dependencies.store.clone()),
        postgres_case_submit_executor(dependencies.store.clone()),
        postgres_case_subject_verify_executor(dependencies.store.clone()),
        postgres_case_cancel_executor(dependencies.store.clone()),
    ];
    for (definition, executor) in mutations.into_iter().zip(executors) {
        let validator: Arc<dyn CapabilitySemanticValidator> =
            Arc::new(ActivationGatedMutationValidator::new(
                dependencies.activation.clone(),
                Arc::new(NoopMutationSemanticValidator),
            ));
        contributions
            .add_mutations([definition], validator, executor)
            .map_err(composition_error)?;
    }

    let query_adapter = Arc::new(CustomerPrivacyQueryAdapter::new_with_cursor(
        dependencies.store,
        cursor(dependencies.cursor_key)?,
        dependencies.visibility_authorizer,
    ));
    let query_validator: Arc<dyn QuerySemanticValidator> = Arc::new(
        ActivationGatedQueryValidator::new(dependencies.activation, query_adapter.clone()),
    );
    let query_executor: Arc<dyn QueryExecutor> = query_adapter;
    contributions
        .add_queries(queries, query_validator, query_executor)
        .map_err(composition_error)?;

    Ok(contributions)
}

fn cursor(key: [u8; 32]) -> Result<CursorCodec, SdkError> {
    CursorCodec::new(key).map_err(|error| configuration_error(error.to_string()))
}

fn composition_error(error: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_PRODUCTION_COMPOSITION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Customer Privacy production contribution is invalid.",
    )
    .with_internal_reference(error.to_string())
}

fn configuration_error(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_PRODUCTION_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Customer Privacy production package is misconfigured.",
    )
    .with_internal_reference(reference)
}
