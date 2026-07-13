use crm_capability_adapters::CapabilityCatalog;
use crm_capability_runtime::{
    CapabilityDefinition, CapabilityExecutionResult, CapabilityRequest,
    TransactionalCapabilityExecutor,
};
use crm_core_data::{
    AggregateTarget, CapabilityBatchExecutionPlan, PostgresMetadataCapabilityExecutor,
    PostgresTransactionalAggregateExecutor, TransactionalAggregatePlanner,
};
use crm_metadata_api_adapter::{
    METADATA_MUTATION_CAPABILITY_IDS, METADATA_QUERY_CAPABILITY_IDS,
    metadata_mutation_capability_definitions, metadata_query_capability_definitions,
};
use crm_metadata_query_adapter::MetadataQueryAdapter;
use crm_module_sdk::{ErrorCategory, PortFuture, RecordSnapshot, SdkError};
use crm_parties_capability_adapter::{
    PARTY_MUTATION_CAPABILITY_IDS, PartyCapabilityPlanner,
    capability_definitions as party_capability_definitions,
};
use crm_parties_query_adapter::{
    PARTY_QUERY_CAPABILITY_IDS, PartyQueryAdapter,
    query_capability_definitions as party_query_capability_definitions,
};
use crm_query_runtime::{
    QueryExecutionResult, QueryExecutor, QueryRequest, QuerySemanticValidator,
};
use crm_sales_activities_capability_composition::{
    ProductionQueryRouter, SalesActivitiesCapabilityPlannerRouter,
    capability_definitions as sales_activities_capability_definitions,
    query_capability_definitions as production_query_capability_definitions,
};
use std::fmt;
use std::sync::Arc;

pub fn application_mutation_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    let mut definitions = sales_activities_capability_definitions()?;
    definitions.extend(party_capability_definitions()?);
    definitions.extend(metadata_mutation_capability_definitions()?);
    Ok(definitions)
}

pub fn application_query_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    let mut definitions = production_query_capability_definitions()?;
    definitions.extend(party_query_capability_definitions()?);
    definitions.extend(metadata_query_capability_definitions()?);
    Ok(definitions)
}

pub fn application_capability_catalog() -> Result<CapabilityCatalog, SdkError> {
    CapabilityCatalog::new(application_mutation_definitions()?).map_err(catalog_error)
}

pub fn application_query_capability_catalog() -> Result<CapabilityCatalog, SdkError> {
    CapabilityCatalog::new(application_query_definitions()?).map_err(catalog_error)
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ApplicationAggregatePlannerRouter;

impl TransactionalAggregatePlanner for ApplicationAggregatePlannerRouter {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        if PARTY_MUTATION_CAPABILITY_IDS.contains(&definition.capability_id.as_str()) {
            PartyCapabilityPlanner.target(definition, request)
        } else {
            SalesActivitiesCapabilityPlannerRouter.target(definition, request)
        }
    }

    fn plan(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
        current: Option<&RecordSnapshot>,
    ) -> Result<CapabilityBatchExecutionPlan, SdkError> {
        if PARTY_MUTATION_CAPABILITY_IDS.contains(&definition.capability_id.as_str()) {
            PartyCapabilityPlanner.plan(definition, request, current)
        } else {
            SalesActivitiesCapabilityPlannerRouter.plan(definition, request, current)
        }
    }
}

#[derive(Clone)]
pub struct ApplicationCapabilityExecutorRouter {
    aggregate: Arc<PostgresTransactionalAggregateExecutor>,
    metadata: Arc<PostgresMetadataCapabilityExecutor>,
}

impl ApplicationCapabilityExecutorRouter {
    pub fn new(
        aggregate: Arc<PostgresTransactionalAggregateExecutor>,
        metadata: Arc<PostgresMetadataCapabilityExecutor>,
    ) -> Self {
        Self {
            aggregate,
            metadata,
        }
    }
}

impl fmt::Debug for ApplicationCapabilityExecutorRouter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApplicationCapabilityExecutorRouter")
            .field("aggregate", &"PostgresTransactionalAggregateExecutor")
            .field("metadata", &"PostgresMetadataCapabilityExecutor")
            .finish()
    }
}

impl TransactionalCapabilityExecutor for ApplicationCapabilityExecutorRouter {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: CapabilityRequest,
    ) -> PortFuture<'a, Result<CapabilityExecutionResult, SdkError>> {
        if METADATA_MUTATION_CAPABILITY_IDS.contains(&definition.capability_id.as_str()) {
            self.metadata.execute(definition, request)
        } else {
            self.aggregate.execute(definition, request)
        }
    }
}

#[derive(Debug, Clone)]
pub struct ApplicationQueryRouter {
    production: ProductionQueryRouter,
    parties: PartyQueryAdapter,
    metadata: MetadataQueryAdapter,
}

impl ApplicationQueryRouter {
    pub fn new(
        production: ProductionQueryRouter,
        parties: PartyQueryAdapter,
        metadata: MetadataQueryAdapter,
    ) -> Self {
        Self {
            production,
            parties,
            metadata,
        }
    }
}

impl QuerySemanticValidator for ApplicationQueryRouter {
    fn validate<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a QueryRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        if METADATA_QUERY_CAPABILITY_IDS.contains(&definition.capability_id.as_str()) {
            self.metadata.validate(definition, request)
        } else if PARTY_QUERY_CAPABILITY_IDS.contains(&definition.capability_id.as_str()) {
            self.parties.validate(definition, request)
        } else {
            self.production.validate(definition, request)
        }
    }
}

impl QueryExecutor for ApplicationQueryRouter {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: QueryRequest,
    ) -> PortFuture<'a, Result<QueryExecutionResult, SdkError>> {
        if METADATA_QUERY_CAPABILITY_IDS.contains(&definition.capability_id.as_str()) {
            self.metadata.execute(definition, request)
        } else if PARTY_QUERY_CAPABILITY_IDS.contains(&definition.capability_id.as_str()) {
            self.parties.execute(definition, request)
        } else {
            self.production.execute(definition, request)
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
    use crm_sales_activities_capability_composition::PRODUCTION_MUTATION_CAPABILITY_IDS;
    use crm_sales_activities_query_adapter::PRODUCTION_QUERY_CAPABILITY_IDS;
    use crm_search_query_adapter::SEARCH_QUERY_CAPABILITY;

    #[test]
    fn application_catalogs_extend_existing_product_coordinates_without_replacing_them() {
        let mutations = application_mutation_definitions().unwrap();
        assert_eq!(
            mutations.len(),
            PRODUCTION_MUTATION_CAPABILITY_IDS.len()
                + PARTY_MUTATION_CAPABILITY_IDS.len()
                + METADATA_MUTATION_CAPABILITY_IDS.len()
        );
        for coordinate in PRODUCTION_MUTATION_CAPABILITY_IDS {
            assert!(
                mutations
                    .iter()
                    .any(|definition| { definition.capability_id.as_str() == coordinate })
            );
        }
        for coordinate in PARTY_MUTATION_CAPABILITY_IDS {
            assert!(
                mutations
                    .iter()
                    .any(|definition| definition.capability_id.as_str() == coordinate)
            );
        }
        for coordinate in METADATA_MUTATION_CAPABILITY_IDS {
            assert!(
                mutations
                    .iter()
                    .any(|definition| { definition.capability_id.as_str() == coordinate })
            );
        }

        let queries = application_query_definitions().unwrap();
        assert_eq!(
            queries.len(),
            PRODUCTION_QUERY_CAPABILITY_IDS.len()
                + 1
                + PARTY_QUERY_CAPABILITY_IDS.len()
                + METADATA_QUERY_CAPABILITY_IDS.len()
        );
        assert!(
            queries
                .iter()
                .any(|definition| { definition.capability_id.as_str() == SEARCH_QUERY_CAPABILITY })
        );
        for coordinate in PARTY_QUERY_CAPABILITY_IDS {
            assert!(
                queries
                    .iter()
                    .any(|definition| definition.capability_id.as_str() == coordinate)
            );
        }
        for coordinate in METADATA_QUERY_CAPABILITY_IDS {
            assert!(
                queries
                    .iter()
                    .any(|definition| { definition.capability_id.as_str() == coordinate })
            );
        }
    }
}
