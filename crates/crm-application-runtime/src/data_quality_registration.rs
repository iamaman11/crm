use crate::governed_metadata::{
    ApplicationAggregatePlannerRouter as BaseApplicationAggregatePlannerRouter,
    application_capability_catalog as base_application_capability_catalog,
    application_mutation_definitions as base_application_mutation_definitions,
};
use crm_capability_adapters::CapabilityCatalog;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest};
use crm_core_data::{AggregateTarget, CapabilityBatchExecutionPlan, TransactionalAggregatePlanner};
use crm_data_quality_capability_adapter::{
    DataQualityRuleSetCapabilityPlanner, PUBLISH_PARTY_RULE_SET_CAPABILITY,
    capability_definitions as data_quality_capability_definitions,
};
use crm_module_sdk::{ErrorCategory, RecordSnapshot, SdkError};
use std::fmt;

pub fn application_mutation_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    let mut definitions = base_application_mutation_definitions()?;
    definitions.extend(data_quality_capability_definitions()?);
    Ok(definitions)
}

pub fn application_capability_catalog() -> Result<CapabilityCatalog, SdkError> {
    base_application_capability_catalog()?;
    CapabilityCatalog::new(application_mutation_definitions()?).map_err(catalog_error)
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ApplicationAggregatePlannerRouter;

impl TransactionalAggregatePlanner for ApplicationAggregatePlannerRouter {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        if definition.capability_id.as_str() == PUBLISH_PARTY_RULE_SET_CAPABILITY {
            DataQualityRuleSetCapabilityPlanner.target(definition, request)
        } else {
            BaseApplicationAggregatePlannerRouter.target(definition, request)
        }
    }

    fn plan(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
        current: Option<&RecordSnapshot>,
    ) -> Result<CapabilityBatchExecutionPlan, SdkError> {
        if definition.capability_id.as_str() == PUBLISH_PARTY_RULE_SET_CAPABILITY {
            DataQualityRuleSetCapabilityPlanner.plan(definition, request, current)
        } else {
            BaseApplicationAggregatePlannerRouter.plan(definition, request, current)
        }
    }
}

fn catalog_error(error: impl fmt::Display) -> SdkError {
    SdkError::new(
        "APPLICATION_CAPABILITY_CATALOG_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The application capability catalog configuration is invalid.",
    )
    .with_internal_reference(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_data_quality_capability_adapter::PUBLISH_PARTY_COMPLETENESS_PROFILE_CAPABILITY;
    use crm_module_sdk::{CapabilityId, CapabilityVersion};

    #[test]
    fn application_mutation_catalog_adds_exact_data_quality_definition_coordinates() {
        let definitions = application_mutation_definitions().unwrap();
        for capability in [
            PUBLISH_PARTY_RULE_SET_CAPABILITY,
            PUBLISH_PARTY_COMPLETENESS_PROFILE_CAPABILITY,
        ] {
            assert_eq!(
                definitions
                    .iter()
                    .filter(|definition| definition.capability_id.as_str() == capability)
                    .count(),
                1
            );
            let capability_id = CapabilityId::try_new(capability).unwrap();
            let capability_version = CapabilityVersion::try_new("1.0.0").unwrap();
            assert_eq!(
                application_capability_catalog()
                    .unwrap()
                    .definition(&capability_id, &capability_version)
                    .unwrap()
                    .capability_id
                    .as_str(),
                capability
            );
        }
    }
}
