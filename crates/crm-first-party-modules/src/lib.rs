#![forbid(unsafe_code)]

//! Mechanically narrow aggregation of proven first-party owner contributions.
//!
//! This crate contains no route catalog and no business dispatch. Exact routes
//! remain defined and built by owner packages; this boundary only combines
//! their contribution entry points for the generic process host.

use crm_application_composition::{ModuleActivationPort, ModuleContributionSet};
use crm_capability_runtime::CapabilityAuthorizer;
use crm_consents_capability_composition::{
    ConsentsProductionDependencies, build_contribution as build_consents_contribution,
};
pub use crm_consents_capability_composition::{
    mutation_capability_definitions as consents_mutation_capability_definitions,
    query_capability_definitions as consents_query_capability_definitions,
};
use crm_contact_points_capability_composition::{
    ContactPointsProductionDependencies, build_contribution as build_contact_points_contribution,
};
pub use crm_contact_points_capability_composition::{
    mutation_capability_definitions as contact_points_mutation_capability_definitions,
    query_capability_definitions as contact_points_query_capability_definitions,
};
use crm_core_data::PostgresDataStore;
pub use crm_customer_360_query_adapter::MODULE_ID as CUSTOMER_360_MODULE_ID;
pub use crm_customer_360_query_adapter::query_capability_definitions as customer_360_query_capability_definitions;
use crm_customer_360_query_adapter::{
    Customer360ProductionDependencies, build_contribution as build_customer_360_contribution,
};
use crm_customer_accounts_query_adapter::{
    CustomerAccountsProductionDependencies,
    build_contribution as build_customer_accounts_contribution,
};
pub use crm_customer_accounts_query_adapter::{
    mutation_capability_definitions as customer_accounts_mutation_capability_definitions,
    query_capability_definitions as customer_accounts_query_capability_definitions,
};
use crm_customer_data_operations_execution_composition::{
    CustomerDataOperationsProductionDependencies,
    build_contribution as build_customer_data_operations_contribution,
};
pub use crm_customer_data_operations_execution_composition::{
    mutation_capability_definitions as customer_data_operations_mutation_capability_definitions,
    query_capability_definitions as customer_data_operations_query_capability_definitions,
};
use crm_customer_enrichment_capability_composition::{
    CustomerEnrichmentProductionDependencies,
    build_contribution as build_customer_enrichment_contribution,
};
pub use crm_customer_enrichment_capability_composition::{
    mutation_capability_definitions as customer_enrichment_mutation_capability_definitions,
    query_capability_definitions as customer_enrichment_query_capability_definitions,
};
use crm_data_quality_source_composition::{
    DataQualityProductionDependencies, build_contribution as build_data_quality_contribution,
};
pub use crm_data_quality_source_composition::{
    mutation_capability_definitions as data_quality_mutation_capability_definitions,
    query_capability_definitions as data_quality_query_capability_definitions,
};
use crm_identity_resolution_capability_composition::{
    IdentityResolutionProductionDependencies,
    build_contribution as build_identity_resolution_contribution,
};
pub use crm_identity_resolution_capability_composition::{
    mutation_capability_definitions as identity_resolution_mutation_capability_definitions,
    query_capability_definitions as identity_resolution_query_capability_definitions,
};
use crm_module_sdk::SdkError;
use crm_party_reference_composition::{
    PartiesProductionDependencies, PostgresPartyReferenceReader,
    build_contribution as build_parties_contribution,
};
pub use crm_party_reference_composition::{
    mutation_capability_definitions as parties_mutation_capability_definitions,
    query_capability_definitions as parties_query_capability_definitions,
};
use crm_party_relationships_capability_composition::{
    PartyRelationshipsProductionDependencies,
    build_contribution as build_party_relationships_contribution,
};
pub use crm_party_relationships_capability_composition::{
    mutation_capability_definitions as party_relationships_mutation_capability_definitions,
    query_capability_definitions as party_relationships_query_capability_definitions,
};
use crm_query_runtime::{QueryAuthorizer, QueryVisibilityAuthorizer};
use crm_sales_activities_capability_composition::{
    SalesActivitiesProductionDependencies,
    build_contribution as build_sales_activities_contribution,
};
pub use crm_sales_activities_capability_composition::{
    mutation_capability_definitions as sales_activities_mutation_capability_definitions,
    production_query_capability_definitions as sales_activities_query_capability_definitions,
};
use std::sync::Arc;

#[derive(Clone)]
pub struct FirstPartyProductionDependencies {
    pub store: PostgresDataStore,
    pub activation: Arc<dyn ModuleActivationPort>,
    pub capability_authorizer: Arc<dyn CapabilityAuthorizer>,
    pub query_authorizer: Arc<dyn QueryAuthorizer>,
    pub visibility_authorizer: Arc<dyn QueryVisibilityAuthorizer>,
    pub cursor_key: [u8; 32],
}

/// Builds the accepted first-party contribution sequence without repeating any
/// module identifier, capability coordinate or owner-specific dispatch rule.
pub fn build_all(
    dependencies: FirstPartyProductionDependencies,
) -> Result<ModuleContributionSet, SdkError> {
    let FirstPartyProductionDependencies {
        store,
        activation,
        capability_authorizer,
        query_authorizer,
        visibility_authorizer,
        cursor_key,
    } = dependencies;
    let parties = Arc::new(PostgresPartyReferenceReader::new(store.clone()));
    let mut contributions = ModuleContributionSet::new();

    contributions.merge(build_sales_activities_contribution(
        SalesActivitiesProductionDependencies {
            store: store.clone(),
            activation: activation.clone(),
            visibility_authorizer: visibility_authorizer.clone(),
            cursor_key,
        },
    )?);

    contributions.merge(build_parties_contribution(PartiesProductionDependencies {
        store: store.clone(),
        activation: activation.clone(),
        visibility_authorizer: visibility_authorizer.clone(),
        cursor_key,
    })?);
    contributions.merge(build_customer_accounts_contribution(
        CustomerAccountsProductionDependencies {
            store: store.clone(),
            parties: parties.clone(),
            activation: activation.clone(),
            visibility_authorizer: visibility_authorizer.clone(),
            cursor_key,
        },
    )?);
    contributions.merge(build_consents_contribution(
        ConsentsProductionDependencies {
            store: store.clone(),
            activation: activation.clone(),
            visibility_authorizer: visibility_authorizer.clone(),
            cursor_key,
        },
    )?);
    contributions.merge(build_contact_points_contribution(
        ContactPointsProductionDependencies {
            store: store.clone(),
            parties: parties.clone(),
            activation: activation.clone(),
            visibility_authorizer: visibility_authorizer.clone(),
            cursor_key,
        },
    )?);
    contributions.merge(build_party_relationships_contribution(
        PartyRelationshipsProductionDependencies {
            store: store.clone(),
            parties: parties.clone(),
            activation: activation.clone(),
            visibility_authorizer: visibility_authorizer.clone(),
            cursor_key,
        },
    )?);
    contributions.merge(build_customer_360_contribution(
        Customer360ProductionDependencies {
            store: store.clone(),
            activation: activation.clone(),
            visibility_authorizer: visibility_authorizer.clone(),
        },
    )?);
    contributions.merge(build_identity_resolution_contribution(
        IdentityResolutionProductionDependencies {
            store: store.clone(),
            activation: activation.clone(),
            visibility_authorizer: visibility_authorizer.clone(),
            cursor_key,
        },
    )?);
    contributions.merge(build_customer_data_operations_contribution(
        CustomerDataOperationsProductionDependencies {
            store: store.clone(),
            activation: activation.clone(),
            capability_authorizer: capability_authorizer.clone(),
            visibility_authorizer: visibility_authorizer.clone(),
            cursor_key,
        },
    )?);
    contributions.merge(build_data_quality_contribution(
        DataQualityProductionDependencies {
            store: store.clone(),
            activation: activation.clone(),
            capability_authorizer,
            query_authorizer: query_authorizer.clone(),
            visibility_authorizer: visibility_authorizer.clone(),
        },
    )?);
    contributions.merge(build_customer_enrichment_contribution(
        CustomerEnrichmentProductionDependencies {
            store,
            activation,
            query_authorizer,
            visibility_authorizer,
            cursor_key,
        },
    )?);

    Ok(contributions)
}
