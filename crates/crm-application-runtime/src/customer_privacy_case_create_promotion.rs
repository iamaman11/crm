use crate::customer_enrichment_reject_promotion as base_runtime;
use crate::native_composition::ProductionCompositionDependencies;
use crm_application_composition::{
    ActivationGatedMutationValidator, ActivationGatedQueryValidator, ApplicationComposition,
    ModuleContributionSet, NoopMutationSemanticValidator,
};
use crm_capability_runtime::{CapabilityDefinition, CapabilitySemanticValidator};
use crm_customer_privacy_capability_adapter::capability_definitions as customer_privacy_create_definitions;
use crm_customer_privacy_capability_composition::{
    postgres_case_create_executor, postgres_case_subject_verify_executor,
    postgres_case_submit_executor,
};
use crm_customer_privacy_query_adapter::{
    CustomerPrivacyQueryAdapter, query_capability_definitions as customer_privacy_query_definitions,
};
use crm_customer_privacy_subject_capability_adapter::capability_definitions as customer_privacy_subject_definitions;
use crm_customer_privacy_submit_capability_adapter::capability_definitions as customer_privacy_submit_definitions;
use crm_module_sdk::{ErrorCategory, ModuleId, SdkError};
use crm_query_runtime::{QueryExecutor, QuerySemanticValidator};
use std::sync::Arc;

pub use base_runtime::PRODUCTION_REVIEW_POLICY_VERSION;

/// Returns the accepted public mutation inventory plus exactly three Customer
/// Privacy production coordinates: case creation, submission and authoritative
/// subject verification.
pub fn application_mutation_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    let mut definitions = base_runtime::application_mutation_definitions()?;
    definitions.extend(customer_privacy_create_definitions()?);
    definitions.extend(customer_privacy_submit_definitions()?);
    definitions.extend(customer_privacy_subject_definitions()?);
    Ok(definitions)
}

/// Returns the accepted query inventory plus exactly one permission-aware
/// Customer Privacy coordinate: `customer_privacy.case.get@1.0.0`.
pub fn application_query_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    let mut definitions = base_runtime::application_query_definitions()?;
    definitions.extend(customer_privacy_query_definitions()?);
    Ok(definitions)
}

/// Extends the existing production composition without adding a capability-
/// specific HTTP/gRPC switch. The generic application ingress owns exact version
/// resolution, validation, live authorization and dispatch. Subject verification
/// uses the accepted transaction-scoped Party, canonical-lineage, topology-
/// generation and shared-subject-lock guard. Case reads use strict persisted-state
/// rehydration plus live case/subject resource visibility and field redaction.
pub fn build_production_composition(
    dependencies: ProductionCompositionDependencies,
) -> Result<ApplicationComposition, SdkError> {
    let create_executor = postgres_case_create_executor(dependencies.store.clone());
    let submit_executor = postgres_case_submit_executor(dependencies.store.clone());
    let subject_executor = postgres_case_subject_verify_executor(dependencies.store.clone());
    let query_adapter = Arc::new(CustomerPrivacyQueryAdapter::new(
        dependencies.store.clone(),
        dependencies.visibility_authorizer.clone(),
    ));

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
            dependencies.activation.clone(),
            Arc::new(NoopMutationSemanticValidator),
        ));
    contributions
        .add_mutations(
            customer_privacy_submit_definitions()?,
            submit_validator,
            submit_executor,
        )
        .map_err(composition_error)?;

    let subject_validator: Arc<dyn CapabilitySemanticValidator> =
        Arc::new(ActivationGatedMutationValidator::new(
            dependencies.activation.clone(),
            Arc::new(NoopMutationSemanticValidator),
        ));
    contributions
        .add_mutations(
            customer_privacy_subject_definitions()?,
            subject_validator,
            subject_executor,
        )
        .map_err(composition_error)?;

    let query_validator: Arc<dyn QuerySemanticValidator> = Arc::new(
        ActivationGatedQueryValidator::new(dependencies.activation, query_adapter.clone()),
    );
    let query_executor: Arc<dyn QueryExecutor> = query_adapter;
    contributions
        .add_queries(
            customer_privacy_query_definitions()?,
            query_validator,
            query_executor,
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
