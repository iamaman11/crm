use crate::customer_enrichment_reject_promotion as base_runtime;
use crate::native_composition::ProductionCompositionDependencies;
use crm_application_composition::{
    ActivationGatedMutationValidator, ApplicationComposition, ModuleContributionSet,
};
use crm_capability_runtime::{
    CapabilityDefinition, CapabilitySemanticValidator, TransactionalCapabilityExecutor,
};
use crm_contact_points_capability_adapter::{
    CREATE_CAPABILITY as CONTACT_POINT_CREATE_CAPABILITY, ContactPointCapabilityPlanner,
    capability_definition as contact_point_capability_definition,
};
use crm_contact_points_capability_composition::{
    ContactPointCreateCustomerSubjectGuard, ContactPointPartyReferenceSemanticValidator,
};
use crm_core_data::{PostgresTransactionalAggregateExecutor, TransactionalAggregatePlanner};
use crm_customer_privacy_production::{
    CustomerPrivacyProductionDependencies, PostgresCustomerPrivacySubjectPolicy,
    build_contribution_with_complete_control_lifecycle as build_customer_privacy_contribution,
    control_query_capability_definitions,
    mutation_capability_definitions_with_complete_control_lifecycle,
};
use crm_module_sdk::{ErrorCategory, ModuleId, SdkError};
use crm_party_reference_composition::PostgresPartyReferenceReader;
use std::sync::Arc;

pub use base_runtime::PRODUCTION_REVIEW_POLICY_VERSION;

/// Returns the accepted public mutation inventory plus the exact frozen
/// Customer Privacy nine-mutation owner inventory.
pub fn application_mutation_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    let mut definitions = base_runtime::application_mutation_definitions()?;
    definitions.extend(mutation_capability_definitions_with_complete_control_lifecycle()?);
    Ok(definitions)
}

/// Returns the accepted query inventory plus the exact frozen Customer Privacy
/// seven-query owner inventory.
pub fn application_query_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    let mut definitions = base_runtime::application_query_definitions()?;
    definitions.extend(crm_customer_privacy_production::query_capability_definitions()?);
    definitions.extend(control_query_capability_definitions()?);
    Ok(definitions)
}

/// Extends the accepted production composition through owner-owned boundaries:
/// Customer Privacy contributes the complete restriction and legal-hold
/// placement, release and permission-aware read lifecycle, while Contact Points
/// replaces only `contact-point.create` with a final transaction guard. Generic
/// dispatch and every unrelated owner route remain unchanged.
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
    let contact_create_count = base
        .mutation_definitions()
        .iter()
        .filter(|definition| definition.capability_id.as_str() == CONTACT_POINT_CREATE_CAPABILITY)
        .count();
    if contact_create_count != 1 {
        return Err(configuration_error(
            "the accepted base composition must contain exactly one Contact Point create route",
        ));
    }

    let mut contributions = ModuleContributionSet::new();
    contributions
        .add_mutations(
            base.mutation_definitions()
                .iter()
                .filter(|definition| {
                    definition.capability_id.as_str() != CONTACT_POINT_CREATE_CAPABILITY
                })
                .cloned(),
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

    let contact_create_definition =
        contact_point_capability_definition(CONTACT_POINT_CREATE_CAPABILITY)?;
    let contact_semantic_validator: Arc<dyn CapabilitySemanticValidator> =
        Arc::new(ContactPointPartyReferenceSemanticValidator::new(Arc::new(
            PostgresPartyReferenceReader::new(dependencies.store.clone()),
        )));
    let contact_validator: Arc<dyn CapabilitySemanticValidator> =
        Arc::new(ActivationGatedMutationValidator::new(
            dependencies.activation.clone(),
            contact_semantic_validator,
        ));
    let contact_planner: Arc<dyn TransactionalAggregatePlanner> =
        Arc::new(ContactPointCapabilityPlanner);
    let contact_executor: Arc<dyn TransactionalCapabilityExecutor> =
        Arc::new(PostgresTransactionalAggregateExecutor::guarded(
            dependencies.store.clone(),
            contact_planner,
            Arc::new(ContactPointCreateCustomerSubjectGuard::new(Arc::new(
                PostgresCustomerPrivacySubjectPolicy,
            ))),
        ));
    contributions
        .add_mutations(
            [contact_create_definition],
            contact_validator,
            contact_executor,
        )
        .map_err(composition_error)?;

    contributions.merge(build_customer_privacy_contribution(
        CustomerPrivacyProductionDependencies {
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
