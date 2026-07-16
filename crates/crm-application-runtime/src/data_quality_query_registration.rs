use crate::governed_metadata::{
    ApplicationQueryRouter as BaseApplicationQueryRouter,
    application_query_definitions as base_application_query_definitions,
};
use crm_capability_adapters::CapabilityCatalog;
use crm_capability_runtime::CapabilityDefinition;
use crm_data_quality_query_adapter::{
    DataQualityQueryAdapter, GET_PARTY_RULE_SET_CAPABILITY,
    query_capability_definitions as data_quality_query_capability_definitions,
};
use crm_module_sdk::{ErrorCategory, PortFuture, SdkError};
use crm_query_runtime::{
    QueryExecutionResult, QueryExecutor, QueryRequest, QuerySemanticValidator,
};
use std::fmt;

pub fn application_query_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    let mut definitions = base_application_query_definitions()?;
    definitions.extend(data_quality_query_capability_definitions()?);
    Ok(definitions)
}

pub fn application_query_capability_catalog() -> Result<CapabilityCatalog, SdkError> {
    CapabilityCatalog::new(application_query_definitions()?).map_err(catalog_error)
}

#[derive(Debug, Clone)]
pub struct ApplicationQueryRouter {
    base: BaseApplicationQueryRouter,
    data_quality: DataQualityQueryAdapter,
}

impl ApplicationQueryRouter {
    pub fn new(base: BaseApplicationQueryRouter, data_quality: DataQualityQueryAdapter) -> Self {
        Self { base, data_quality }
    }
}

impl QuerySemanticValidator for ApplicationQueryRouter {
    fn validate<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a QueryRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        if definition.capability_id.as_str() == GET_PARTY_RULE_SET_CAPABILITY {
            self.data_quality.validate(definition, request)
        } else {
            self.base.validate(definition, request)
        }
    }
}

impl QueryExecutor for ApplicationQueryRouter {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: QueryRequest,
    ) -> PortFuture<'a, Result<QueryExecutionResult, SdkError>> {
        if definition.capability_id.as_str() == GET_PARTY_RULE_SET_CAPABILITY {
            self.data_quality.execute(definition, request)
        } else {
            self.base.execute(definition, request)
        }
    }
}

fn catalog_error(error: impl fmt::Display) -> SdkError {
    SdkError::new(
        "APPLICATION_QUERY_CATALOG_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The application query capability catalog configuration is invalid.",
    )
    .with_internal_reference(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_capability_runtime::CapabilityRisk;
    use crm_data_quality_capability_adapter::MODULE_ID;
    use crm_module_sdk::DataClass;

    #[test]
    fn application_query_catalog_adds_exact_data_quality_rule_set_coordinate() {
        let definitions = application_query_definitions().unwrap();
        let matches = definitions
            .iter()
            .filter(|definition| {
                definition.capability_id.as_str() == GET_PARTY_RULE_SET_CAPABILITY
            })
            .collect::<Vec<_>>();
        assert_eq!(matches.len(), 1);

        let definition = matches[0];
        assert_eq!(definition.owner_module_id.as_str(), MODULE_ID);
        assert_eq!(definition.risk, CapabilityRisk::Low);
        assert_eq!(
            definition.input_contract.allowed_data_classes,
            vec![DataClass::Confidential]
        );
        assert!(definition.output_contract.is_some());
        assert!(!definition.mutation);
        assert!(!definition.requires_idempotency);
        assert!(!definition.requires_approval);
    }
}
