use crm_capability_adapters::CapabilityCatalog;
use crm_capability_runtime::CapabilityDefinition;
use crm_module_sdk::{ErrorCategory, SdkError};
use crm_sales_activities_query_adapter::query_capability_definitions as owner_query_definitions;
use crm_search_query_adapter::search_query_capability_definition;

pub fn query_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    let mut definitions = owner_query_definitions()?;
    definitions.push(search_query_capability_definition()?);
    Ok(definitions)
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
    use crm_sales_activities_query_adapter::PRODUCTION_QUERY_CAPABILITY_IDS;
    use crm_search_query_adapter::SEARCH_QUERY_CAPABILITY;

    #[test]
    fn query_catalog_contains_owner_queries_and_governed_search() {
        let definitions = query_capability_definitions().unwrap();
        assert_eq!(definitions.len(), PRODUCTION_QUERY_CAPABILITY_IDS.len() + 1);
        assert_eq!(
            definitions
                .iter()
                .take(PRODUCTION_QUERY_CAPABILITY_IDS.len())
                .map(|definition| definition.capability_id.as_str())
                .collect::<Vec<_>>(),
            PRODUCTION_QUERY_CAPABILITY_IDS
        );
        assert_eq!(
            definitions.last().unwrap().capability_id.as_str(),
            SEARCH_QUERY_CAPABILITY
        );
        assert!(definitions.iter().all(|definition| !definition.mutation));
        assert_eq!(query_capability_catalog().unwrap().len(), 5);
    }
}
