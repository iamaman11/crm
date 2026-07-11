use crm_capability_adapters::CapabilityCatalog;
use crm_capability_runtime::CapabilityDefinition;
use crm_module_sdk::{ErrorCategory, SdkError};
use crm_sales_activities_query_adapter::{
    PRODUCTION_QUERY_CAPABILITY_IDS, query_capability_definitions as adapter_query_definitions,
};

pub fn query_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    adapter_query_definitions()
}

pub fn query_capability_catalog() -> Result<CapabilityCatalog, SdkError> {
    CapabilityCatalog::new(query_capability_definitions()?).map_err(|error| {
        SdkError::new(
            "QUERY_CAPABILITY_CATALOG_CONFIGURATION_INVALID",
            ErrorCategory::Internal,
            false,
            "The query capability catalog configuration is invalid.",
        )
        .with_internal_reference(error.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_catalog_contains_exactly_four_read_only_coordinates() {
        let definitions = query_capability_definitions().unwrap();
        assert_eq!(definitions.len(), PRODUCTION_QUERY_CAPABILITY_IDS.len());
        assert_eq!(
            definitions
                .iter()
                .map(|definition| definition.capability_id.as_str())
                .collect::<Vec<_>>(),
            PRODUCTION_QUERY_CAPABILITY_IDS
        );
        assert!(definitions.iter().all(|definition| !definition.mutation));
        assert_eq!(query_capability_catalog().unwrap().len(), 4);
    }
}
