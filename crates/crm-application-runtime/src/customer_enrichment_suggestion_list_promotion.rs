use crate::native_composition::{self, ProductionCompositionDependencies};
use crm_application_composition::{
    ActivationGatedQueryValidator, ApplicationComposition, ModuleContributionSet,
};
use crm_capability_runtime::{CapabilityDefinition, TransactionalCapabilityExecutor};
use crm_customer_enrichment_suggestion_query_adapter::{
    CustomerEnrichmentSuggestionQueryAdapter, list_suggestions_by_party_capability_definition,
};
use crm_module_sdk::{ErrorCategory, ModuleId, SdkError};
use crm_query_runtime::{CursorCodec, QueryExecutor, QuerySemanticValidator};
use std::sync::Arc;

/// Returns the exact public query inventory after promoting the permission-aware
/// Customer Enrichment suggestion list surface.
pub fn application_query_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    let mut definitions = native_composition::application_query_definitions()?;
    definitions.push(list_suggestions_by_party_capability_definition()?);
    Ok(definitions)
}

/// Extends the canonical production composition without changing any existing
/// route ownership or executor. Existing routes are re-contributed through the
/// canonical routers, then the activation-gated suggestion-list route is added
/// with the process cursor key and live visibility authorizer.
pub fn build_production_composition(
    dependencies: ProductionCompositionDependencies,
) -> Result<ApplicationComposition, SdkError> {
    let base_dependencies = ProductionCompositionDependencies {
        store: dependencies.store.clone(),
        activation: dependencies.activation.clone(),
        capability_authorizer: dependencies.capability_authorizer.clone(),
        query_authorizer: dependencies.query_authorizer.clone(),
        visibility_authorizer: dependencies.visibility_authorizer.clone(),
        cursor_key: dependencies.cursor_key,
    };
    let base = native_composition::build_production_composition(base_dependencies)?;
    let mut contributions = ModuleContributionSet::new();

    let mutation_executor: Arc<dyn TransactionalCapabilityExecutor> = base.mutation_executor();
    contributions
        .add_mutations(
            base.mutation_definitions().iter().cloned(),
            base.mutation_validator(),
            mutation_executor,
        )
        .map_err(composition_error)?;
    contributions
        .add_queries(
            base.query_definitions().iter().cloned(),
            base.query_validator(),
            base.query_executor(),
        )
        .map_err(composition_error)?;

    let adapter = Arc::new(CustomerEnrichmentSuggestionQueryAdapter::new(
        dependencies.store,
        cursor(dependencies.cursor_key)?,
        dependencies.visibility_authorizer,
    ));
    let validator: Arc<dyn QuerySemanticValidator> = Arc::new(ActivationGatedQueryValidator::new(
        dependencies.activation,
        adapter.clone(),
    ));
    let executor: Arc<dyn QueryExecutor> = adapter;
    contributions
        .add_queries(
            [list_suggestions_by_party_capability_definition()?],
            validator,
            executor,
        )
        .map_err(composition_error)?;

    for module_id in base.module_ids() {
        contributions
            .add_empty_module(ModuleId::try_new(module_id.clone()).map_err(configuration_error)?)
            .map_err(composition_error)?;
    }
    contributions.build().map_err(composition_error)
}

fn cursor(key: [u8; 32]) -> Result<CursorCodec, SdkError> {
    CursorCodec::new(key).map_err(|error| {
        SdkError::new(
            "APPLICATION_CURSOR_CONFIGURATION_INVALID",
            ErrorCategory::Internal,
            false,
            "The application cursor configuration is invalid.",
        )
        .with_internal_reference(error.to_string())
    })
}

fn composition_error(error: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "APPLICATION_COMPOSITION_INVALID",
        ErrorCategory::Internal,
        false,
        "The production application composition is invalid.",
    )
    .with_internal_reference(error.to_string())
}

fn configuration_error(error: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "APPLICATION_COMPOSITION_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The production application composition configuration is invalid.",
    )
    .with_internal_reference(error.to_string())
}
