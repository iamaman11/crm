#![forbid(unsafe_code)]

use crm_application_composition::{
    ActivationGatedMutationValidator, ActivationGatedQueryValidator, ModuleActivationPort,
    ModuleContributionSet, NoopMutationSemanticValidator,
};
use crm_capability_runtime::{
    CapabilityAuthorizer, CapabilityDefinition, CapabilitySemanticValidator,
    TransactionalCapabilityExecutor,
};
use crm_core_data::{
    PostgresDataStore, PostgresImmutableFileArtifactStore, PostgresTransactionalAggregateExecutor,
};
use crm_customer_data_operations_capability_adapter::{
    CREATE_PARTY_IMPORT_JOB_CAPABILITY, CustomerDataOperationsCapabilityPlanner,
    VALIDATE_PARTY_IMPORT_ROWS_CAPABILITY,
    capability_definitions as adapter_mutation_capability_definitions,
};
use crm_customer_data_operations_query_adapter::{
    CustomerDataOperationsQueryAdapter,
    query_capability_definitions as adapter_query_capability_definitions,
};
use crm_customer_data_operations_source_composition::{
    CustomerDataOperationsSourceExecutor,
    source_capability_definitions as source_mutation_capability_definitions,
};
use crm_module_sdk::{ErrorCategory, SdkError};
use crm_query_runtime::{
    CursorCodec, QueryExecutor, QuerySemanticValidator, QueryVisibilityAuthorizer,
};
use std::fmt;
use std::sync::Arc;

#[derive(Clone)]
pub struct CustomerDataOperationsProductionDependencies {
    pub store: PostgresDataStore,
    pub activation: Arc<dyn ModuleActivationPort>,
    pub capability_authorizer: Arc<dyn CapabilityAuthorizer>,
    pub visibility_authorizer: Arc<dyn QueryVisibilityAuthorizer>,
    pub cursor_key: [u8; 32],
}

pub fn mutation_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    let mut definitions = ordinary_mutation_capability_definitions()?;
    definitions.extend(source_mutation_capability_definitions()?);
    Ok(definitions)
}

pub fn query_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    adapter_query_capability_definitions()
}

pub fn build_contribution(
    dependencies: CustomerDataOperationsProductionDependencies,
) -> Result<ModuleContributionSet, SdkError> {
    let CustomerDataOperationsProductionDependencies {
        store,
        activation,
        capability_authorizer,
        visibility_authorizer,
        cursor_key,
    } = dependencies;
    let mut contributions = ModuleContributionSet::new();

    let ordinary_executor: Arc<dyn TransactionalCapabilityExecutor> =
        Arc::new(PostgresTransactionalAggregateExecutor::new(
            store.clone(),
            Arc::new(CustomerDataOperationsCapabilityPlanner),
        ));
    let ordinary_validator: Arc<dyn CapabilitySemanticValidator> =
        Arc::new(ActivationGatedMutationValidator::new(
            activation.clone(),
            Arc::new(NoopMutationSemanticValidator),
        ));
    contributions
        .add_mutations(
            ordinary_mutation_capability_definitions()?,
            ordinary_validator,
            ordinary_executor,
        )
        .map_err(production_composition_error)?;

    let source_executor: Arc<dyn TransactionalCapabilityExecutor> =
        Arc::new(CustomerDataOperationsSourceExecutor::new(
            store.clone(),
            Arc::new(PostgresImmutableFileArtifactStore::new(store.clone())),
            capability_authorizer,
        ));
    let source_validator: Arc<dyn CapabilitySemanticValidator> =
        Arc::new(ActivationGatedMutationValidator::new(
            activation.clone(),
            Arc::new(NoopMutationSemanticValidator),
        ));
    contributions
        .add_mutations(
            source_mutation_capability_definitions()?,
            source_validator,
            source_executor,
        )
        .map_err(production_composition_error)?;

    let queries = Arc::new(CustomerDataOperationsQueryAdapter::new(
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

fn ordinary_mutation_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    Ok(adapter_mutation_capability_definitions()?
        .into_iter()
        .filter(|definition| {
            !matches!(
                definition.capability_id.as_str(),
                CREATE_PARTY_IMPORT_JOB_CAPABILITY | VALIDATE_PARTY_IMPORT_ROWS_CAPABILITY
            )
        })
        .collect())
}

fn cursor(key: [u8; 32]) -> Result<CursorCodec, SdkError> {
    CursorCodec::new(key).map_err(|error| {
        SdkError::new(
            "CUSTOMER_DATA_OPERATIONS_CURSOR_CONFIGURATION_INVALID",
            ErrorCategory::Internal,
            false,
            "The Customer Data Operations cursor configuration is invalid.",
        )
        .with_internal_reference(error.to_string())
    })
}

fn production_composition_error(error: impl fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_OPERATIONS_PRODUCTION_COMPOSITION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Customer Data Operations production contribution is invalid.",
    )
    .with_internal_reference(error.to_string())
}
