use crate::customer_enrichment_reject_promotion as base_runtime;
use crate::native_composition::ProductionCompositionDependencies;
use crm_application_composition::{ApplicationComposition, ModuleContributionSet};
use crm_capability_runtime::CapabilityDefinition;
use crm_first_party_modules::{
    CustomerPrivacyFirstPartyDependencies,
    build_customer_privacy as build_customer_privacy_contribution,
    customer_privacy_mutation_capability_definitions,
    customer_privacy_query_capability_definitions,
};
use crm_module_sdk::{ErrorCategory, ModuleId, SdkError};

pub use base_runtime::PRODUCTION_REVIEW_POLICY_VERSION;

/// Returns the accepted public mutation inventory plus the exact Customer
/// Privacy inventory contributed by the owner application package.
pub fn application_mutation_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    let mut definitions = base_runtime::application_mutation_definitions()?;
    definitions.extend(customer_privacy_mutation_capability_definitions()?);
    Ok(definitions)
}

/// Returns the accepted query inventory plus the exact Customer Privacy
/// inventory contributed by the owner application package.
pub fn application_query_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    let mut definitions = base_runtime::application_query_definitions()?;
    definitions.extend(customer_privacy_query_capability_definitions()?);
    Ok(definitions)
}

/// Extends the existing production composition through the first-party bundle,
/// which delegates to the stable owner-owned Customer Privacy entry point. No
/// concrete Customer Privacy command, query or PostgreSQL implementation is
/// imported by the generic process host.
pub fn build_production_composition(
    dependencies: ProductionCompositionDependencies,
) -> Result<ApplicationComposition, SdkError> {
    let base_dependencies = ProductionCompositionDependencies {
        store: dependencies.store.clone(),
        activation: dependencies.activation.clone(),
        capability_authorizer: dependencies.capability_authorizer,
        query_authorizer: dependencies.query_authorizer,
        visibility_authorizer: dependencies.visibility_authorizer.clone(),
        cursor_key: dependencies.cursor_key,
    };
    let base = base_runtime::build_production_composition(base_dependencies)?;
    let mut contributions = ModuleContributionSet::new();
    contributions
        .add_mutations(
            base.mutation_definitions().iter().cloned(),
            base.mutation_validator(),
            base.mutation_executor(),
        )
        .map_err(composition_error)?;
    contributions
        .add_queries(
            base.query_definitions().iter().cloned(),
            base.query_validator(),
            base.query_executor(),
        )
        .map_err(composition_error)?;

    contributions.merge(build_customer_privacy_contribution(
        CustomerPrivacyFirstPartyDependencies {
            store: dependencies.store,
            activation: dependencies.activation,
            visibility_authorizer: dependencies.visibility_authorizer,
            cursor_key: dependencies.cursor_key,
        },
    )?);

    for module_id in base.module_ids() {
        contributions
            .add_empty_module(ModuleId::try_new(module_id.clone()).map_err(configuration_error)?)
            .map_err(composition_error)?;
    }
    contributions.build().map_err(composition_error)
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
