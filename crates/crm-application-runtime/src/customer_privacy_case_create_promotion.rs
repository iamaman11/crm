use crate::customer_enrichment_reject_promotion as base_runtime;
use crate::native_composition::ProductionCompositionDependencies;
use crm_application_composition::{
    ActivationGatedMutationValidator, ApplicationComposition, ModuleContributionSet,
    NoopMutationSemanticValidator,
};
use crm_capability_runtime::{CapabilityDefinition, CapabilitySemanticValidator};
use crm_customer_privacy_capability_adapter::capability_definitions as customer_privacy_create_definitions;
use crm_customer_privacy_capability_composition::{
    postgres_case_create_executor, postgres_case_submit_executor,
};
use crm_customer_privacy_submit_capability_adapter::capability_definitions as customer_privacy_submit_definitions;
use crm_module_sdk::{ErrorCategory, ModuleId, SdkError};
use std::sync::Arc;

pub use base_runtime::{PRODUCTION_REVIEW_POLICY_VERSION, application_query_definitions};

/// Returns the accepted public mutation inventory plus exactly two Customer
/// Privacy production coordinates: case creation and the optimistic
/// `Draft -> Submitted` lifecycle transition. Candidate process features never
/// change this inventory API.
pub fn application_mutation_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    let mut definitions = base_runtime::application_mutation_definitions()?;
    definitions.extend(customer_privacy_create_definitions()?);
    definitions.extend(customer_privacy_submit_definitions()?);
    Ok(definitions)
}

/// Default builds assemble only the accepted production composition. The explicit
/// candidate feature changes process assembly solely for the real generic-ingress
/// acceptance binary; it does not add an environment switch or a new endpoint.
#[cfg(not(feature = "customer-privacy-subject-verify-candidate"))]
pub fn build_production_composition(
    dependencies: ProductionCompositionDependencies,
) -> Result<ApplicationComposition, SdkError> {
    build_accepted_production_composition(dependencies)
}

#[cfg(feature = "customer-privacy-subject-verify-candidate")]
pub fn build_production_composition(
    dependencies: ProductionCompositionDependencies,
) -> Result<ApplicationComposition, SdkError> {
    crate::customer_privacy_subject_verify_candidate::build_candidate_process_composition(
        dependencies,
    )
}

/// Extends the existing production composition without adding a capability-
/// specific HTTP/gRPC switch. The generic application ingress still owns exact
/// version resolution, validation, live authorization and dispatch. Creation
/// keeps its optional predecessor guard, while submission uses the shared
/// single-aggregate optimistic executor directly.
pub(crate) fn build_accepted_production_composition(
    dependencies: ProductionCompositionDependencies,
) -> Result<ApplicationComposition, SdkError> {
    let create_executor = postgres_case_create_executor(dependencies.store.clone());
    let submit_executor = postgres_case_submit_executor(dependencies.store.clone());

    let base_dependencies = ProductionCompositionDependencies {
        store: dependencies.store,
        activation: dependencies.activation.clone(),
        capability_authorizer: dependencies.capability_authorizer,
        query_authorizer: dependencies.query_authorizer,
        visibility_authorizer: dependencies.visibility_authorizer,
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

    let create_validator: Arc<dyn CapabilitySemanticValidator> =
        Arc::new(ActivationGatedMutationValidator::new(
            dependencies.activation.clone(),
            Arc::new(NoopMutationSemanticValidator),
        ));
    contributions
        .add_mutations(
            customer_privacy_create_definitions()?,
            create_validator,
            create_executor,
        )
        .map_err(composition_error)?;

    let submit_validator: Arc<dyn CapabilitySemanticValidator> =
        Arc::new(ActivationGatedMutationValidator::new(
            dependencies.activation,
            Arc::new(NoopMutationSemanticValidator),
        ));
    contributions
        .add_mutations(
            customer_privacy_submit_definitions()?,
            submit_validator,
            submit_executor,
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
