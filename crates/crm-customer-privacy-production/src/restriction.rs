use crate::legacy::{
    CustomerPrivacyProduction, CustomerPrivacyProductionDependencies, build_contribution,
    build_internal_discovery, build_internal_planning,
};
use crm_application_composition::{
    ActivationGatedMutationValidator, ModuleContributionSet, NoopMutationSemanticValidator,
};
use crm_capability_runtime::CapabilitySemanticValidator;
use crm_customer_privacy_application::{
    mutation_capability_definitions_with_restrictions,
    place_processing_restriction_capability_definition,
};
use crm_customer_privacy_postgres::postgres_restriction_place_executor;
use crm_module_sdk::{ErrorCategory, SdkError};
use std::sync::Arc;

/// Builds the complete step-four production package while retaining the accepted
/// five-mutation legacy contribution as its stable base.
pub fn build_production_with_restrictions(
    dependencies: CustomerPrivacyProductionDependencies,
) -> Result<CustomerPrivacyProduction, SdkError> {
    let contribution = build_contribution_with_restrictions(dependencies.clone())?;
    let (discovery, snapshot_reader) = build_internal_discovery(&dependencies)?;
    let planning = build_internal_planning(&dependencies);
    Ok(CustomerPrivacyProduction {
        contribution,
        discovery,
        snapshot_reader,
        planning,
    })
}

pub fn build_contribution_with_restrictions(
    dependencies: CustomerPrivacyProductionDependencies,
) -> Result<ModuleContributionSet, SdkError> {
    let inventory = mutation_capability_definitions_with_restrictions()?;
    if inventory.len() != 6 {
        return Err(composition_error(
            "step-four Customer Privacy inventory must contain exactly six mutations",
        ));
    }
    let restriction_definition = place_processing_restriction_capability_definition()?;
    if inventory.get(1).map(|definition| &definition.capability_id)
        != Some(&restriction_definition.capability_id)
    {
        return Err(composition_error(
            "restriction.place must remain the second owner mutation in the exact inventory",
        ));
    }

    let mut contribution = build_contribution(dependencies.clone())?;
    let validator: Arc<dyn CapabilitySemanticValidator> =
        Arc::new(ActivationGatedMutationValidator::new(
            dependencies.activation,
            Arc::new(NoopMutationSemanticValidator),
        ));
    contribution
        .add_mutations(
            [restriction_definition],
            validator,
            postgres_restriction_place_executor(dependencies.store),
        )
        .map_err(composition_error)?;
    Ok(contribution)
}

fn composition_error(error: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_RESTRICTION_PRODUCTION_COMPOSITION_INVALID",
        ErrorCategory::Internal,
        false,
        "The processing restriction production contribution is invalid.",
    )
    .with_internal_reference(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_step_four_inventory_is_six() {
        let inventory = mutation_capability_definitions_with_restrictions().unwrap();
        assert_eq!(inventory.len(), 6);
        assert_eq!(
            inventory[1].capability_id.as_str(),
            "customer_privacy.restriction.place"
        );
    }
}
