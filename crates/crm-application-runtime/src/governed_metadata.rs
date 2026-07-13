use crm_capability_adapters::CapabilityCatalog;
use crm_capability_runtime::{
    CapabilityDefinition, CapabilityExecutionResult, CapabilityRequest,
    TransactionalCapabilityExecutor,
};
use crm_contact_points_capability_adapter::{
    ContactPointCapabilityPlanner,
    MUTATION_CAPABILITY_IDS as CONTACT_POINT_MUTATION_CAPABILITY_IDS,
    capability_definitions as contact_point_capability_definitions,
    referenced_party_id_from_create,
};
use crm_contact_points_query_adapter::{
    ContactPointQueryAdapter, QUERY_CAPABILITY_IDS as CONTACT_POINT_QUERY_CAPABILITY_IDS,
    query_capability_definitions as contact_point_query_capability_definitions,
};
use crm_core_data::{
    AggregateTarget, CapabilityBatchExecutionPlan, PostgresDataStore,
    PostgresMetadataCapabilityExecutor, PostgresTransactionalAggregateExecutor, RecordGetQuery,
    TransactionalAggregatePlanner,
};
use crm_customer_accounts_capability_adapter::{
    CustomerAccountCapabilityPlanner, MUTATION_CAPABILITY_IDS as ACCOUNT_MUTATION_CAPABILITY_IDS,
    capability_definitions as account_capability_definitions, referenced_party_ids_from_create,
    referenced_party_ids_from_update,
};
use crm_customer_accounts_query_adapter::{
    AccountQueryAdapter, QUERY_CAPABILITY_IDS as ACCOUNT_QUERY_CAPABILITY_IDS,
    query_capability_definitions as account_query_capability_definitions,
};
use crm_metadata_api_adapter::{
    METADATA_MUTATION_CAPABILITY_IDS, METADATA_QUERY_CAPABILITY_IDS,
    metadata_mutation_capability_definitions, metadata_query_capability_definitions,
};
use crm_metadata_query_adapter::MetadataQueryAdapter;
use crm_module_sdk::{
    ErrorCategory, ModuleId, PortFuture, RecordId, RecordSnapshot, RecordType, SdkError,
};
use crm_parties_capability_adapter::{
    MODULE_ID as PARTIES_MODULE_ID, PARTY_MUTATION_CAPABILITY_IDS, PartyCapabilityPlanner,
    RECORD_TYPE as PARTY_RECORD_TYPE, capability_definitions as party_capability_definitions,
};
use crm_parties_query_adapter::{
    PARTY_QUERY_CAPABILITY_IDS, PartyQueryAdapter,
    query_capability_definitions as party_query_capability_definitions,
};
use crm_party_relationships_capability_adapter::{
    MUTATION_CAPABILITY_IDS as PARTY_RELATIONSHIP_MUTATION_CAPABILITY_IDS,
    PartyRelationshipCapabilityPlanner,
    capability_definitions as party_relationship_capability_definitions,
    referenced_party_ids_from_create as referenced_relationship_party_ids_from_create,
};
use crm_party_relationships_query_adapter::{
    PartyRelationshipQueryAdapter,
    QUERY_CAPABILITY_IDS as PARTY_RELATIONSHIP_QUERY_CAPABILITY_IDS,
    query_capability_definitions as party_relationship_query_capability_definitions,
};
use crm_query_runtime::{
    QueryExecutionResult, QueryExecutor, QueryRequest, QuerySemanticValidator,
};
use crm_sales_activities_capability_composition::{
    ProductionQueryRouter, SalesActivitiesCapabilityPlannerRouter,
    capability_definitions as sales_activities_capability_definitions,
    query_capability_definitions as production_query_capability_definitions,
};
use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

pub fn application_mutation_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    let mut definitions = sales_activities_capability_definitions()?;
    definitions.extend(party_capability_definitions()?);
    definitions.extend(account_capability_definitions()?);
    definitions.extend(contact_point_capability_definitions()?);
    definitions.extend(party_relationship_capability_definitions()?);
    definitions.extend(metadata_mutation_capability_definitions()?);
    Ok(definitions)
}

pub fn application_query_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    let mut definitions = production_query_capability_definitions()?;
    definitions.extend(party_query_capability_definitions()?);
    definitions.extend(account_query_capability_definitions()?);
    definitions.extend(contact_point_query_capability_definitions()?);
    definitions.extend(party_relationship_query_capability_definitions()?);
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
        } else if ACCOUNT_MUTATION_CAPABILITY_IDS.contains(&definition.capability_id.as_str()) {
            CustomerAccountCapabilityPlanner.target(definition, request)
        } else if CONTACT_POINT_MUTATION_CAPABILITY_IDS.contains(&definition.capability_id.as_str())
        {
            ContactPointCapabilityPlanner.target(definition, request)
        } else if PARTY_RELATIONSHIP_MUTATION_CAPABILITY_IDS
            .contains(&definition.capability_id.as_str())
        {
            PartyRelationshipCapabilityPlanner.target(definition, request)
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
        } else if ACCOUNT_MUTATION_CAPABILITY_IDS.contains(&definition.capability_id.as_str()) {
            CustomerAccountCapabilityPlanner.plan(definition, request, current)
        } else if CONTACT_POINT_MUTATION_CAPABILITY_IDS.contains(&definition.capability_id.as_str())
        {
            ContactPointCapabilityPlanner.plan(definition, request, current)
        } else if PARTY_RELATIONSHIP_MUTATION_CAPABILITY_IDS
            .contains(&definition.capability_id.as_str())
        {
            PartyRelationshipCapabilityPlanner.plan(definition, request, current)
        } else {
            SalesActivitiesCapabilityPlannerRouter.plan(definition, request, current)
        }
    }
}

#[derive(Clone)]
pub struct ApplicationCapabilityExecutorRouter {
    store: PostgresDataStore,
    aggregate: Arc<PostgresTransactionalAggregateExecutor>,
    metadata: Arc<PostgresMetadataCapabilityExecutor>,
}

impl ApplicationCapabilityExecutorRouter {
    pub fn new(
        store: PostgresDataStore,
        aggregate: Arc<PostgresTransactionalAggregateExecutor>,
        metadata: Arc<PostgresMetadataCapabilityExecutor>,
    ) -> Self {
        Self {
            store,
            aggregate,
            metadata,
        }
    }
}

impl fmt::Debug for ApplicationCapabilityExecutorRouter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApplicationCapabilityExecutorRouter")
            .field("store", &self.store)
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
        } else if ACCOUNT_MUTATION_CAPABILITY_IDS.contains(&definition.capability_id.as_str()) {
            Box::pin(async move {
                validate_account_party_references(&self.store, definition, &request).await?;
                self.aggregate.execute(definition, request).await
            })
        } else if CONTACT_POINT_MUTATION_CAPABILITY_IDS.contains(&definition.capability_id.as_str())
        {
            Box::pin(async move {
                validate_contact_point_party_reference(&self.store, definition, &request).await?;
                self.aggregate.execute(definition, request).await
            })
        } else if PARTY_RELATIONSHIP_MUTATION_CAPABILITY_IDS
            .contains(&definition.capability_id.as_str())
        {
            Box::pin(async move {
                validate_party_relationship_party_references(&self.store, definition, &request)
                    .await?;
                self.aggregate.execute(definition, request).await
            })
        } else {
            self.aggregate.execute(definition, request)
        }
    }
}

async fn validate_account_party_references(
    store: &PostgresDataStore,
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<(), SdkError> {
    let references = match definition.capability_id.as_str() {
        "accounts.account.create" => referenced_party_ids_from_create(request)?,
        "accounts.account.update" => referenced_party_ids_from_update(request)?,
        _ => return Err(account_reference_configuration_error()),
    };
    let unique_party_ids = references
        .into_iter()
        .map(|reference| reference.as_str().to_owned())
        .collect::<BTreeSet<_>>();
    let owner_module_id = ModuleId::try_new(PARTIES_MODULE_ID).map_err(catalog_error)?;
    let record_type = RecordType::try_new(PARTY_RECORD_TYPE).map_err(catalog_error)?;

    for party_id in unique_party_ids {
        if !party_reference_exists(store, request, &owner_module_id, &record_type, party_id).await?
        {
            return Err(account_party_reference_unavailable());
        }
    }
    Ok(())
}

async fn validate_contact_point_party_reference(
    store: &PostgresDataStore,
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<(), SdkError> {
    if definition.capability_id.as_str() != "contact-points.contact-point.create" {
        return Ok(());
    }
    let reference = referenced_party_id_from_create(request)?;
    let owner_module_id = ModuleId::try_new(PARTIES_MODULE_ID).map_err(catalog_error)?;
    let record_type = RecordType::try_new(PARTY_RECORD_TYPE).map_err(catalog_error)?;
    if party_reference_exists(
        store,
        request,
        &owner_module_id,
        &record_type,
        reference.as_str().to_owned(),
    )
    .await?
    {
        Ok(())
    } else {
        Err(contact_point_party_reference_unavailable())
    }
}

async fn validate_party_relationship_party_references(
    store: &PostgresDataStore,
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<(), SdkError> {
    if definition.capability_id.as_str() != "party-relationships.party-relationship.create" {
        return Ok(());
    }
    let unique_party_ids = referenced_relationship_party_ids_from_create(request)?
        .into_iter()
        .map(|reference| reference.as_str().to_owned())
        .collect::<BTreeSet<_>>();
    let owner_module_id = ModuleId::try_new(PARTIES_MODULE_ID).map_err(catalog_error)?;
    let record_type = RecordType::try_new(PARTY_RECORD_TYPE).map_err(catalog_error)?;
    for party_id in unique_party_ids {
        if !party_reference_exists(store, request, &owner_module_id, &record_type, party_id).await?
        {
            return Err(party_relationship_party_reference_unavailable());
        }
    }
    Ok(())
}

async fn party_reference_exists(
    store: &PostgresDataStore,
    request: &CapabilityRequest,
    owner_module_id: &ModuleId,
    record_type: &RecordType,
    party_id: String,
) -> Result<bool, SdkError> {
    let record_id = RecordId::try_new(party_id).map_err(catalog_error)?;
    Ok(store
        .get_record_for_query(&RecordGetQuery {
            tenant_id: request.context.execution.tenant_id.clone(),
            owner_module_id: owner_module_id.clone(),
            record_type: record_type.clone(),
            record_id,
        })
        .await?
        .is_some())
}

fn account_party_reference_unavailable() -> SdkError {
    SdkError::new(
        "CUSTOMER_ACCOUNTS_PARTY_REFERENCE_UNAVAILABLE",
        ErrorCategory::InvalidArgument,
        false,
        "One or more referenced Parties are unavailable.",
    )
}

fn contact_point_party_reference_unavailable() -> SdkError {
    SdkError::new(
        "CONTACT_POINTS_PARTY_REFERENCE_UNAVAILABLE",
        ErrorCategory::InvalidArgument,
        false,
        "The referenced Party is unavailable.",
    )
}

fn party_relationship_party_reference_unavailable() -> SdkError {
    SdkError::new(
        "PARTY_RELATIONSHIPS_PARTY_REFERENCE_UNAVAILABLE",
        ErrorCategory::InvalidArgument,
        false,
        "One or more referenced Parties are unavailable.",
    )
}

fn account_reference_configuration_error() -> SdkError {
    SdkError::new(
        "CUSTOMER_ACCOUNTS_REFERENCE_VALIDATION_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Account reference validation configuration is invalid.",
    )
}

#[derive(Debug, Clone)]
pub struct ApplicationQueryRouter {
    production: ProductionQueryRouter,
    parties: PartyQueryAdapter,
    accounts: AccountQueryAdapter,
    contact_points: ContactPointQueryAdapter,
    party_relationships: PartyRelationshipQueryAdapter,
    metadata: MetadataQueryAdapter,
}

impl ApplicationQueryRouter {
    pub fn new(
        production: ProductionQueryRouter,
        parties: PartyQueryAdapter,
        accounts: AccountQueryAdapter,
        contact_points: ContactPointQueryAdapter,
        party_relationships: PartyRelationshipQueryAdapter,
        metadata: MetadataQueryAdapter,
    ) -> Self {
        Self {
            production,
            parties,
            accounts,
            contact_points,
            party_relationships,
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
        } else if ACCOUNT_QUERY_CAPABILITY_IDS.contains(&definition.capability_id.as_str()) {
            self.accounts.validate(definition, request)
        } else if CONTACT_POINT_QUERY_CAPABILITY_IDS.contains(&definition.capability_id.as_str()) {
            self.contact_points.validate(definition, request)
        } else if PARTY_RELATIONSHIP_QUERY_CAPABILITY_IDS
            .contains(&definition.capability_id.as_str())
        {
            self.party_relationships.validate(definition, request)
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
        } else if ACCOUNT_QUERY_CAPABILITY_IDS.contains(&definition.capability_id.as_str()) {
            self.accounts.execute(definition, request)
        } else if CONTACT_POINT_QUERY_CAPABILITY_IDS.contains(&definition.capability_id.as_str()) {
            self.contact_points.execute(definition, request)
        } else if PARTY_RELATIONSHIP_QUERY_CAPABILITY_IDS
            .contains(&definition.capability_id.as_str())
        {
            self.party_relationships.execute(definition, request)
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
                + ACCOUNT_MUTATION_CAPABILITY_IDS.len()
                + CONTACT_POINT_MUTATION_CAPABILITY_IDS.len()
                + PARTY_RELATIONSHIP_MUTATION_CAPABILITY_IDS.len()
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
        for coordinate in ACCOUNT_MUTATION_CAPABILITY_IDS {
            assert!(
                mutations
                    .iter()
                    .any(|definition| definition.capability_id.as_str() == coordinate)
            );
        }
        for coordinate in CONTACT_POINT_MUTATION_CAPABILITY_IDS {
            assert!(
                mutations
                    .iter()
                    .any(|definition| definition.capability_id.as_str() == coordinate)
            );
        }
        for coordinate in PARTY_RELATIONSHIP_MUTATION_CAPABILITY_IDS {
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
                + ACCOUNT_QUERY_CAPABILITY_IDS.len()
                + CONTACT_POINT_QUERY_CAPABILITY_IDS.len()
                + PARTY_RELATIONSHIP_QUERY_CAPABILITY_IDS.len()
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
        for coordinate in ACCOUNT_QUERY_CAPABILITY_IDS {
            assert!(
                queries
                    .iter()
                    .any(|definition| definition.capability_id.as_str() == coordinate)
            );
        }
        for coordinate in CONTACT_POINT_QUERY_CAPABILITY_IDS {
            assert!(
                queries
                    .iter()
                    .any(|definition| definition.capability_id.as_str() == coordinate)
            );
        }
        for coordinate in PARTY_RELATIONSHIP_QUERY_CAPABILITY_IDS {
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
