#![forbid(unsafe_code)]

//! Mechanically narrow aggregation of proven first-party owner contributions.
//!
//! This crate contains no route catalog and no business dispatch. Exact routes
//! remain defined and built by owner packages; this boundary only combines
//! their contribution entry points for the generic process host.

use crm_application_composition::{ModuleActivationPort, ModuleContributionSet};
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
use crm_customer_accounts_capability_composition::{
    CustomerAccountsProductionDependencies,
    build_contribution as build_customer_accounts_contribution,
};
pub use crm_customer_accounts_capability_composition::{
    mutation_capability_definitions as customer_accounts_mutation_capability_definitions,
    query_capability_definitions as customer_accounts_query_capability_definitions,
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
use crm_query_runtime::QueryVisibilityAuthorizer;
use std::sync::Arc;

#[derive(Clone)]
pub struct FirstPartyProductionDependencies {
    pub store: PostgresDataStore,
    pub activation: Arc<dyn ModuleActivationPort>,
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
        visibility_authorizer,
        cursor_key,
    } = dependencies;
    let parties = Arc::new(PostgresPartyReferenceReader::new(store.clone()));
    let mut contributions = ModuleContributionSet::new();

    contributions.merge(build_parties_contribution(PartiesProductionDependencies {
        store: store.clone(),
        activation: activation.clone(),
        visibility_authorizer: visibility_authorizer.clone(),
        cursor_key,
    })?)?;
    contributions.merge(build_customer_accounts_contribution(
        CustomerAccountsProductionDependencies {
            store: store.clone(),
            parties: parties.clone(),
            activation: activation.clone(),
            visibility_authorizer: visibility_authorizer.clone(),
            cursor_key,
        },
    )?)?;
    contributions.merge(build_consents_contribution(
        ConsentsProductionDependencies {
            store: store.clone(),
            activation: activation.clone(),
            visibility_authorizer: visibility_authorizer.clone(),
            cursor_key,
        },
    )?)?;
    contributions.merge(build_contact_points_contribution(
        ContactPointsProductionDependencies {
            store: store.clone(),
            parties: parties.clone(),
            activation: activation.clone(),
            visibility_authorizer: visibility_authorizer.clone(),
            cursor_key,
        },
    )?)?;
    contributions.merge(build_party_relationships_contribution(
        PartyRelationshipsProductionDependencies {
            store,
            parties,
            activation,
            visibility_authorizer,
            cursor_key,
        },
    )?)?;

    Ok(contributions)
}

pub const CRATE_NAME: &str = "crm-first-party-modules";
