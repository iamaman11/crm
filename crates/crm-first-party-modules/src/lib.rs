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
use crm_core_data::PostgresDataStore;
use crm_customer_accounts_capability_composition::{
    CustomerAccountsProductionDependencies,
    build_contribution as build_customer_accounts_contribution,
};
use crm_customer_privacy_production::{
    CustomerPrivacyProductionDependencies,
    build_contribution as build_customer_privacy_owner_contribution,
};
pub use crm_customer_privacy_production::{
    mutation_capability_definitions as customer_privacy_mutation_capability_definitions,
    query_capability_definitions as customer_privacy_query_capability_definitions,
};
use crm_module_sdk::SdkError;
use crm_party_reference_composition::PartyReferenceReader;
use crm_query_runtime::QueryVisibilityAuthorizer;
use std::sync::Arc;

#[derive(Clone)]
pub struct FirstPartyProductionDependencies {
    pub store: PostgresDataStore,
    pub parties: Arc<dyn PartyReferenceReader>,
    pub activation: Arc<dyn ModuleActivationPort>,
    pub visibility_authorizer: Arc<dyn QueryVisibilityAuthorizer>,
    pub cursor_key: [u8; 32],
}

#[derive(Clone)]
pub struct CustomerPrivacyFirstPartyDependencies {
    pub store: PostgresDataStore,
    pub activation: Arc<dyn ModuleActivationPort>,
    pub visibility_authorizer: Arc<dyn QueryVisibilityAuthorizer>,
    pub cursor_key: [u8; 32],
}

/// Builds all owner contributions that have completed the two-owner proof.
///
/// No module identifiers or route coordinates are repeated here. Owner
/// packages remain the only source of those definitions, and final application
/// assembly retains duplicate, owner and route-kind validation.
pub fn build_all(
    dependencies: FirstPartyProductionDependencies,
) -> Result<ModuleContributionSet, SdkError> {
    let FirstPartyProductionDependencies {
        store,
        parties,
        activation,
        visibility_authorizer,
        cursor_key,
    } = dependencies;
    let mut contributions = ModuleContributionSet::new();

    contributions.merge(build_customer_accounts_contribution(
        CustomerAccountsProductionDependencies {
            store: store.clone(),
            parties,
            activation: activation.clone(),
            visibility_authorizer: visibility_authorizer.clone(),
            cursor_key,
        },
    )?);
    contributions.merge(build_consents_contribution(
        ConsentsProductionDependencies {
            store,
            activation,
            visibility_authorizer,
            cursor_key,
        },
    )?);

    Ok(contributions)
}

/// Routes the already-stable Customer Privacy production contribution through
/// the first-party bundle while preserving its existing late merge point in
/// the generic application runtime.
pub fn build_customer_privacy(
    dependencies: CustomerPrivacyFirstPartyDependencies,
) -> Result<ModuleContributionSet, SdkError> {
    let CustomerPrivacyFirstPartyDependencies {
        store,
        activation,
        visibility_authorizer,
        cursor_key,
    } = dependencies;
    build_customer_privacy_owner_contribution(CustomerPrivacyProductionDependencies {
        store,
        activation,
        visibility_authorizer,
        cursor_key,
    })
}

pub const CRATE_NAME: &str = "crm-first-party-modules";
