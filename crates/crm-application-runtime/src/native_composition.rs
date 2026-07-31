use crm_application_composition::{
    ApplicationComposition, ModuleActivationPort, ModuleContributionSet,
    NoopMutationSemanticValidator,
};
use crm_capability_adapters::CapabilityCatalog;
use crm_capability_runtime::{CapabilityAuthorizer, CapabilityDefinition};
use crm_core_data::{
    PostgresDataStore, PostgresMetadataCapabilityExecutor, PostgresMetadataQueryStore,
};
use crm_first_party_modules::{
    FirstPartyProductionDependencies, build_all as build_first_party_modules,
    consents_mutation_capability_definitions as consent_capability_definitions,
    consents_query_capability_definitions as consent_query_capability_definitions,
    contact_points_mutation_capability_definitions as contact_point_capability_definitions,
    contact_points_query_capability_definitions as contact_point_query_capability_definitions,
    customer_360_query_capability_definitions,
    customer_accounts_mutation_capability_definitions as account_capability_definitions,
    customer_accounts_query_capability_definitions as account_query_capability_definitions,
    customer_data_operations_mutation_capability_definitions as customer_data_operations_capability_definitions,
    customer_data_operations_query_capability_definitions,
    customer_enrichment_mutation_capability_definitions as customer_enrichment_capability_definitions,
    customer_enrichment_query_capability_definitions,
    data_quality_mutation_capability_definitions as data_quality_capability_definitions,
    data_quality_query_capability_definitions,
    identity_resolution_mutation_capability_definitions as identity_resolution_capability_definitions,
    identity_resolution_query_capability_definitions,
    parties_mutation_capability_definitions as party_capability_definitions,
    parties_query_capability_definitions as party_query_capability_definitions,
    party_relationships_mutation_capability_definitions as party_relationship_capability_definitions,
    party_relationships_query_capability_definitions as party_relationship_query_capability_definitions,
    sales_activities_mutation_capability_definitions as sales_activities_capability_definitions,
    sales_activities_query_capability_definitions,
};
use crm_global_search_composition::GLOBAL_SEARCH_INDEX_ID;
use crm_metadata_api_adapter::{
    metadata_mutation_capability_definitions, metadata_query_capability_definitions,
};
use crm_metadata_query_adapter::MetadataQueryAdapter;
use crm_module_sdk::{ErrorCategory, ModuleId, PortFuture, SdkError, TenantId};
use crm_query_runtime::{CursorCodec, QueryAuthorizer, QueryVisibilityAuthorizer};
use crm_sales_activities_link::MODULE_ID as LINK_MODULE_ID;
use crm_search_query_adapter::{SearchQueryAdapter, search_query_capability_definition};
use crm_search_runtime::SearchIndexId;
use std::collections::BTreeSet;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct PostgresModuleActivation {
    store: PostgresDataStore,
}

impl PostgresModuleActivation {
    pub fn new(store: PostgresDataStore) -> Self {
        Self { store }
    }
}

impl ModuleActivationPort for PostgresModuleActivation {
    fn is_active<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        module_id: &'a ModuleId,
    ) -> PortFuture<'a, Result<bool, SdkError>> {
        Box::pin(async move { self.store.is_module_active(tenant_id, module_id).await })
    }
}

pub struct ProductionCompositionDependencies {
    pub store: PostgresDataStore,
    pub activation: Arc<dyn ModuleActivationPort>,
    pub capability_authorizer: Arc<dyn CapabilityAuthorizer>,
    pub query_authorizer: Arc<dyn QueryAuthorizer>,
    pub visibility_authorizer: Arc<dyn QueryVisibilityAuthorizer>,
    pub cursor_key: [u8; 32],
}

/// Returns the exact public mutation inventory assembled from module-owned
/// definition factories. This compatibility API is intentionally data-only:
/// production dispatch is owned exclusively by `ApplicationComposition`.
pub fn application_mutation_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    let mut definitions = sales_activities_capability_definitions()?;
    definitions.extend(party_capability_definitions()?);
    definitions.extend(account_capability_definitions()?);
    definitions.extend(contact_point_capability_definitions()?);
    definitions.extend(party_relationship_capability_definitions()?);
    definitions.extend(consent_capability_definitions()?);
    definitions.extend(identity_resolution_capability_definitions()?);
    definitions.extend(customer_data_operations_capability_definitions()?);
    definitions.extend(data_quality_capability_definitions()?);
    definitions.extend(customer_enrichment_capability_definitions()?);
    definitions.extend(metadata_mutation_capability_definitions()?);
    Ok(definitions)
}

/// Returns the exact public query inventory assembled from module-owned
/// definition factories. It exists for tests, bootstrap grants and parity
/// checks; it is not a router and performs no runtime dispatch.
pub fn application_capability_catalog() -> Result<CapabilityCatalog, SdkError> {
    CapabilityCatalog::new(application_mutation_definitions()?).map_err(configuration_error)
}

pub fn application_query_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    let mut definitions = sales_activities_query_capability_definitions()?;
    definitions.extend(party_query_capability_definitions()?);
    definitions.extend(account_query_capability_definitions()?);
    definitions.extend(contact_point_query_capability_definitions()?);
    definitions.extend(party_relationship_query_capability_definitions()?);
    definitions.extend(customer_360_query_capability_definitions()?);
    definitions.extend(consent_query_capability_definitions()?);
    definitions.extend(identity_resolution_query_capability_definitions()?);
    definitions.extend(customer_data_operations_query_capability_definitions()?);
    definitions.extend(customer_enrichment_query_capability_definitions()?);
    definitions.extend(data_quality_query_capability_definitions()?);
    definitions.push(search_query_capability_definition()?);
    definitions.extend(metadata_query_capability_definitions()?);
    Ok(definitions)
}

pub fn build_production_composition(
    dependencies: ProductionCompositionDependencies,
) -> Result<ApplicationComposition, SdkError> {
    let ProductionCompositionDependencies {
        store,
        activation,
        capability_authorizer,
        query_authorizer,
        visibility_authorizer,
        cursor_key,
    } = dependencies;
    let mut contributions = ModuleContributionSet::new();

    contributions.merge(build_first_party_modules(
        FirstPartyProductionDependencies {
            store: store.clone(),
            activation: activation.clone(),
            capability_authorizer: capability_authorizer.clone(),
            query_authorizer: query_authorizer.clone(),
            visibility_authorizer: visibility_authorizer.clone(),
            cursor_key,
        },
    )?);

    contributions
        .add_mutations(
            metadata_mutation_capability_definitions()?,
            Arc::new(NoopMutationSemanticValidator),
            Arc::new(PostgresMetadataCapabilityExecutor::new(store.clone())),
        )
        .map_err(composition_error)?;

    let search_queries = Arc::new(SearchQueryAdapter::new(
        SearchIndexId::try_new(GLOBAL_SEARCH_INDEX_ID).map_err(configuration_error)?,
        Arc::new(store.clone()),
        visibility_authorizer,
        cursor(cursor_key)?,
    )?);
    contributions
        .add_queries(
            [search_query_capability_definition()?],
            search_queries.clone(),
            search_queries,
        )
        .map_err(composition_error)?;

    let metadata_queries = Arc::new(MetadataQueryAdapter::new(Arc::new(
        PostgresMetadataQueryStore::new(store),
    )));
    contributions
        .add_queries(
            metadata_query_capability_definitions()?,
            metadata_queries.clone(),
            metadata_queries,
        )
        .map_err(composition_error)?;

    contributions
        .add_empty_module(ModuleId::try_new(LINK_MODULE_ID).map_err(configuration_error)?)
        .map_err(composition_error)?;
    contributions.build().map_err(composition_error)
}

fn cursor(key: [u8; 32]) -> Result<CursorCodec, SdkError> {
    CursorCodec::new(key).map_err(|error| {
        SdkError::new(
            "APPLICATION_CURSOR_CONFIGURATION_INVALID",
            ErrorCategory::Internal,
            false,
            "The application cursor configuration is invalid.",
        )
        .with_internal_reference(error.to_string())
    })
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

pub fn declared_business_module_ids() -> BTreeSet<String> {
    [
        "crm.activities",
        "crm.consents",
        "crm.contact-points",
        "crm.customer-accounts",
        "crm.customer-data-operations",
        "crm.customer-enrichment",
        "crm.customer360",
        "crm.data-quality",
        "crm.identity-resolution",
        "crm.parties",
        "crm.party-relationships",
        "crm.sales",
        LINK_MODULE_ID,
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}
