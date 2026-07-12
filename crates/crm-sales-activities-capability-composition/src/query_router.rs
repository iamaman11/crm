use crm_capability_runtime::CapabilityDefinition;
use crm_module_sdk::{ErrorCategory, PortFuture, SdkError};
use crm_query_runtime::{
    QueryExecutionResult, QueryExecutor, QueryRequest, QuerySemanticValidator,
};
use crm_sales_activities_query_adapter::{
    PRODUCTION_QUERY_CAPABILITY_IDS, SalesActivitiesQueryAdapter,
};
use crm_search_query_adapter::{SEARCH_QUERY_CAPABILITY, SearchQueryAdapter};

#[derive(Debug, Clone)]
pub struct ProductionQueryRouter {
    sales_activities: SalesActivitiesQueryAdapter,
    search: SearchQueryAdapter,
}

impl ProductionQueryRouter {
    pub fn new(
        sales_activities: SalesActivitiesQueryAdapter,
        search: SearchQueryAdapter,
    ) -> Self {
        Self {
            sales_activities,
            search,
        }
    }
}

impl QuerySemanticValidator for ProductionQueryRouter {
    fn validate<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a QueryRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        match route(definition.capability_id.as_str()) {
            Ok(QueryRoute::SalesActivities) => self.sales_activities.validate(definition, request),
            Ok(QueryRoute::Search) => self.search.validate(definition, request),
            Err(error) => Box::pin(async move { Err(error) }),
        }
    }
}

impl QueryExecutor for ProductionQueryRouter {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: QueryRequest,
    ) -> PortFuture<'a, Result<QueryExecutionResult, SdkError>> {
        match route(definition.capability_id.as_str()) {
            Ok(QueryRoute::SalesActivities) => self.sales_activities.execute(definition, request),
            Ok(QueryRoute::Search) => self.search.execute(definition, request),
            Err(error) => Box::pin(async move { Err(error) }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QueryRoute {
    SalesActivities,
    Search,
}

fn route(capability_id: &str) -> Result<QueryRoute, SdkError> {
    if PRODUCTION_QUERY_CAPABILITY_IDS.contains(&capability_id) {
        return Ok(QueryRoute::SalesActivities);
    }
    if capability_id == SEARCH_QUERY_CAPABILITY {
        return Ok(QueryRoute::Search);
    }
    Err(SdkError::new(
        "PRODUCTION_QUERY_ROUTE_UNSUPPORTED",
        ErrorCategory::Internal,
        false,
        "The production query capability is not configured.",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_existing_owner_queries_and_platform_search_without_overlap() {
        assert_eq!(
            route(PRODUCTION_QUERY_CAPABILITY_IDS[0]).unwrap(),
            QueryRoute::SalesActivities
        );
        assert_eq!(route(SEARCH_QUERY_CAPABILITY).unwrap(), QueryRoute::Search);
        assert_eq!(
            route("unknown.query").unwrap_err().code,
            "PRODUCTION_QUERY_ROUTE_UNSUPPORTED"
        );
    }
}
