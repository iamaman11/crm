#![forbid(unsafe_code)]

use crate::{
    CustomerEnrichmentCapabilityExecutor, PostgresCustomerEnrichmentMappingReferenceGuard,
    PostgresCustomerEnrichmentRequestPartyGuard,
};
use crm_application_composition::{
    ActivationGatedMutationValidator, ActivationGatedQueryValidator, ModuleActivationPort,
    ModuleContributionSet, NoopMutationSemanticValidator,
};
use crm_capability_runtime::{
    CapabilityDefinition, CapabilitySemanticValidator, TransactionalCapabilityExecutor,
};
use crm_consents_query_adapter::ConsentQueryAdapter;
use crm_core_data::{PostgresDataStore, PostgresTransactionalAggregateExecutor};
use crm_customer_enrichment_capability_adapter::{
    CustomerEnrichmentProviderProfileCapabilityPlanner,
    CustomerEnrichmentRequestCreateCapabilityPlanner,
    capability_definitions as adapter_mutation_capability_definitions,
};
use crm_customer_enrichment_query_adapter::{
    CustomerEnrichmentQueryAdapter,
    query_capability_definitions as entity_query_capability_definitions,
};
use crm_customer_enrichment_request_list_query_adapter::{
    CustomerEnrichmentRequestListQueryAdapter,
    query_capability_definition as request_list_query_capability_definition,
};
use crm_customer_enrichment_suggestion_query_adapter::{
    CustomerEnrichmentSuggestionQueryAdapter, get_suggestion_capability_definition,
};
use crm_module_sdk::{ErrorCategory, SdkError};
use crm_parties_query_adapter::PartyQueryAdapter;
use crm_query_runtime::{
    CursorCodec, QueryAuthorizer, QueryExecutor, QuerySemanticValidator,
    QueryVisibilityAuthorizer,
};
use std::fmt;
use std::sync::Arc;

#[derive(Clone)]
pub struct CustomerEnrichmentProductionDependencies {
    pub store: PostgresDataStore,
    pub activation: Arc<dyn ModuleActivationPort>,
    pub query_authorizer: Arc<dyn QueryAuthorizer>,
    pub visibility_authorizer: Arc<dyn QueryVisibilityAuthorizer>,
    pub cursor_key: [u8; 32],
}

pub fn mutation_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    adapter_mutation_capability_definitions()
}

pub fn query_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    let mut definitions = entity_query_capability_definitions()?;
    definitions.push(request_list_query_capability_definition()?);
    definitions.push(get_suggestion_capability_definition()?);
    Ok(definitions)
}

pub fn build_contribution(
    dependencies: CustomerEnrichmentProductionDependencies,
) -> Result<ModuleContributionSet, SdkError> {
    let CustomerEnrichmentProductionDependencies {
        store,
        activation,
        query_authorizer,
        visibility_authorizer,
        cursor_key,
    } = dependencies;
    let mut contributions = ModuleContributionSet::new();

    let fallback: Arc<dyn TransactionalCapabilityExecutor> =
        Arc::new(PostgresTransactionalAggregateExecutor::guarded(
            store.clone(),
            Arc::new(CustomerEnrichmentProviderProfileCapabilityPlanner),
            Arc::new(PostgresCustomerEnrichmentMappingReferenceGuard),
        ));
    let request_create: Arc<dyn TransactionalCapabilityExecutor> =
        Arc::new(PostgresTransactionalAggregateExecutor::guarded(
            store.clone(),
            Arc::new(CustomerEnrichmentRequestCreateCapabilityPlanner),
            Arc::new(PostgresCustomerEnrichmentRequestPartyGuard),
        ));
    let party_queries = Arc::new(PartyQueryAdapter::new(
        store.clone(),
        cursor(cursor_key)?,
        visibility_authorizer.clone(),
    )?);
    let consent_queries = Arc::new(ConsentQueryAdapter::new(
        store.clone(),
        cursor(cursor_key)?,
        visibility_authorizer.clone(),
    )?);
    let mutation_executor: Arc<dyn TransactionalCapabilityExecutor> = Arc::new(
        CustomerEnrichmentCapabilityExecutor::new(
            store.clone(),
            fallback,
            request_create,
            party_queries,
            consent_queries,
            query_authorizer,
        ),
    );
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

    let entity_queries = Arc::new(CustomerEnrichmentQueryAdapter::new(
        store.clone(),
        visibility_authorizer.clone(),
    ));
    add_queries(
        &mut contributions,
        entity_query_capability_definitions()?,
        entity_queries,
        activation.clone(),
    )?;

    let request_list_queries = Arc::new(CustomerEnrichmentRequestListQueryAdapter::new(
        store.clone(),
        cursor(cursor_key)?,
        visibility_authorizer.clone(),
    ));
    add_queries(
        &mut contributions,
        vec![request_list_query_capability_definition()?],
        request_list_queries,
        activation.clone(),
    )?;

    let suggestion_queries = Arc::new(CustomerEnrichmentSuggestionQueryAdapter::new_get_only(
        store,
        visibility_authorizer,
    ));
    add_queries(
        &mut contributions,
        vec![get_suggestion_capability_definition()?],
        suggestion_queries,
        activation,
    )?;

    Ok(contributions)
}

fn add_queries<T>(
    contributions: &mut ModuleContributionSet,
    definitions: Vec<CapabilityDefinition>,
    adapter: Arc<T>,
    activation: Arc<dyn ModuleActivationPort>,
) -> Result<(), SdkError>
where
    T: QuerySemanticValidator + QueryExecutor + 'static,
{
    let validator: Arc<dyn QuerySemanticValidator> = Arc::new(
        ActivationGatedQueryValidator::new(activation, adapter.clone()),
    );
    let executor: Arc<dyn QueryExecutor> = adapter;
    contributions
        .add_queries(definitions, validator, executor)
        .map(|_| ())
        .map_err(production_composition_error)
}

fn cursor(key: [u8; 32]) -> Result<CursorCodec, SdkError> {
    CursorCodec::new(key).map_err(|error| {
        SdkError::new(
            "CUSTOMER_ENRICHMENT_CURSOR_CONFIGURATION_INVALID",
            ErrorCategory::Internal,
            false,
            "The Customer Enrichment cursor configuration is invalid.",
        )
        .with_internal_reference(error.to_string())
    })
}

fn production_composition_error(error: impl fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PRODUCTION_COMPOSITION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Customer Enrichment production contribution is invalid.",
    )
    .with_internal_reference(error.to_string())
}
