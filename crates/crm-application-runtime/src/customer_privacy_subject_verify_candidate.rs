use crate::customer_privacy_case_create_promotion as production;
use crate::native_composition::ProductionCompositionDependencies;
use crm_application_composition::{
    ActivationGatedMutationValidator, ApplicationComposition, ModuleContributionSet,
    NoopMutationSemanticValidator,
};
use crm_capability_runtime::CapabilitySemanticValidator;
use crm_customer_privacy_capability_composition::postgres_case_subject_verify_executor;
use crm_customer_privacy_subject_capability_adapter::capability_definitions;
use crm_module_sdk::{ErrorCategory, ModuleId, SdkError};
use std::sync::Arc;

/// Builds the real process composition used only by the compile-time candidate
/// acceptance feature. The public production definition and composition APIs remain
/// unchanged, so default builds and production inventory parity stay at exactly two
/// Customer Privacy runtime mutations.
pub(crate) fn build_candidate_process_composition(
    dependencies: ProductionCompositionDependencies,
) -> Result<ApplicationComposition, SdkError> {
    let subject_executor =
        postgres_case_subject_verify_executor(dependencies.store.clone());
    let base_dependencies = ProductionCompositionDependencies {
        store: dependencies.store,
        activation: dependencies.activation.clone(),
        capability_authorizer: dependencies.capability_authorizer,
        query_authorizer: dependencies.query_authorizer,
        visibility_authorizer: dependencies.visibility_authorizer,
        cursor_key: dependencies.cursor_key,
    };
    let base = production::build_production_composition(base_dependencies)?;
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

    let subject_validator: Arc<dyn CapabilitySemanticValidator> =
        Arc::new(ActivationGatedMutationValidator::new(
            dependencies.activation,
            Arc::new(NoopMutationSemanticValidator),
        ));
    contributions
        .add_mutations(
            capability_definitions()?,
            subject_validator,
            subject_executor,
        )
        .map_err(composition_error)?;

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
        "The candidate application composition is invalid.",
    )
    .with_internal_reference(error.to_string())
}

fn configuration_error(error: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "APPLICATION_COMPOSITION_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The candidate application composition configuration is invalid.",
    )
    .with_internal_reference(error.to_string())
}
