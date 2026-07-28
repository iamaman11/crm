#![forbid(unsafe_code)]

//! Pre-authorization application validation for Account Party references.

use crm_application_composition::{
    ActivationGatedMutationValidator, ActivationGatedQueryValidator, ModuleActivationPort,
    ModuleContributionSet,
};
use crm_capability_runtime::{
    CapabilityDefinition, CapabilityRequest, CapabilitySemanticValidator,
    TransactionalCapabilityExecutor,
};
use crm_core_data::{PostgresDataStore, PostgresTransactionalAggregateExecutor};
use crm_customer_accounts_capability_adapter::{
    CREATE_CAPABILITY, CustomerAccountCapabilityPlanner, MUTATION_CAPABILITY_IDS,
    UPDATE_CAPABILITY, capability_definitions as adapter_mutation_capability_definitions,
    referenced_party_ids_from_create, referenced_party_ids_from_update,
};
use crm_customer_accounts_query_adapter::{
    AccountQueryAdapter, query_capability_definitions as adapter_query_capability_definitions,
};
use crm_module_sdk::{ErrorCategory, PortFuture, SdkError};
use crm_party_reference_composition::PartyReferenceReader;
use crm_query_runtime::{
    CursorCodec, QueryExecutor, QuerySemanticValidator, QueryVisibilityAuthorizer,
};
use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

#[derive(Clone)]
pub struct AccountPartyReferenceSemanticValidator {
    parties: Arc<dyn PartyReferenceReader>,
}

impl fmt::Debug for AccountPartyReferenceSemanticValidator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AccountPartyReferenceSemanticValidator")
            .field("parties", &"dyn PartyReferenceReader")
            .finish()
    }
}

impl AccountPartyReferenceSemanticValidator {
    pub fn new(parties: Arc<dyn PartyReferenceReader>) -> Self {
        Self { parties }
    }
}

impl CapabilitySemanticValidator for AccountPartyReferenceSemanticValidator {
    fn validate<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a CapabilityRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        let references = match definition.capability_id.as_str() {
            CREATE_CAPABILITY => referenced_party_ids_from_create(request),
            UPDATE_CAPABILITY => referenced_party_ids_from_update(request),
            value if MUTATION_CAPABILITY_IDS.contains(&value) => Err(configuration_error()),
            _ => Err(unsupported_capability()),
        };
        Box::pin(async move {
            let references = references?;
            let unique = references
                .into_iter()
                .map(|reference| reference.as_str().to_owned())
                .collect::<BTreeSet<_>>();
            for party_id in unique {
                if !self
                    .parties
                    .exists(&request.context.execution.tenant_id, &party_id)
                    .await?
                {
                    return Err(reference_unavailable());
                }
            }
            Ok(())
        })
    }
}

fn reference_unavailable() -> SdkError {
    SdkError::new(
        "CUSTOMER_ACCOUNTS_PARTY_REFERENCE_UNAVAILABLE",
        ErrorCategory::InvalidArgument,
        false,
        "One or more referenced Parties are unavailable.",
    )
}

fn configuration_error() -> SdkError {
    SdkError::new(
        "CUSTOMER_ACCOUNTS_REFERENCE_VALIDATION_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Account reference validation configuration is invalid.",
    )
}

fn unsupported_capability() -> SdkError {
    SdkError::new(
        "CUSTOMER_ACCOUNTS_COMPOSITION_CAPABILITY_UNSUPPORTED",
        ErrorCategory::Internal,
        false,
        "The Account mutation capability is not configured for this composition boundary.",
    )
}

#[derive(Clone)]
pub struct CustomerAccountsProductionDependencies {
    pub store: PostgresDataStore,
    pub parties: Arc<dyn PartyReferenceReader>,
    pub activation: Arc<dyn ModuleActivationPort>,
    pub visibility_authorizer: Arc<dyn QueryVisibilityAuthorizer>,
    pub cursor_key: [u8; 32],
}

/// Returns the exact Customer Accounts mutation inventory owned by this
/// production composition package.
pub fn mutation_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    adapter_mutation_capability_definitions()
}

/// Returns the exact Customer Accounts query inventory owned by this production
/// composition package.
pub fn query_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    adapter_query_capability_definitions()
}

/// Builds the complete Customer Accounts mutation/query contribution inside
/// the authoritative owner package rather than the generic process host.
pub fn build_contribution(
    dependencies: CustomerAccountsProductionDependencies,
) -> Result<ModuleContributionSet, SdkError> {
    let CustomerAccountsProductionDependencies {
        store,
        parties,
        activation,
        visibility_authorizer,
        cursor_key,
    } = dependencies;
    let mut contributions = ModuleContributionSet::new();

    let mutation_executor: Arc<dyn TransactionalCapabilityExecutor> =
        Arc::new(PostgresTransactionalAggregateExecutor::new(
            store.clone(),
            Arc::new(CustomerAccountCapabilityPlanner),
        ));
    let mutation_validator: Arc<dyn CapabilitySemanticValidator> =
        Arc::new(ActivationGatedMutationValidator::new(
            activation.clone(),
            Arc::new(AccountPartyReferenceSemanticValidator::new(parties)),
        ));
    contributions
        .add_mutations(
            mutation_capability_definitions()?,
            mutation_validator,
            mutation_executor,
        )
        .map_err(production_composition_error)?;

    let query_adapter = Arc::new(AccountQueryAdapter::new(
        store,
        customer_accounts_cursor(cursor_key)?,
        visibility_authorizer,
    )?);
    let query_validator: Arc<dyn QuerySemanticValidator> = Arc::new(
        ActivationGatedQueryValidator::new(activation, query_adapter.clone()),
    );
    let query_executor: Arc<dyn QueryExecutor> = query_adapter;
    contributions
        .add_queries(
            query_capability_definitions()?,
            query_validator,
            query_executor,
        )
        .map_err(production_composition_error)?;

    Ok(contributions)
}

fn customer_accounts_cursor(key: [u8; 32]) -> Result<CursorCodec, SdkError> {
    CursorCodec::new(key).map_err(|error| {
        SdkError::new(
            "CUSTOMER_ACCOUNTS_CURSOR_CONFIGURATION_INVALID",
            ErrorCategory::Internal,
            false,
            "The Customer Accounts cursor configuration is invalid.",
        )
        .with_internal_reference(error.to_string())
    })
}

fn production_composition_error(error: impl fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_ACCOUNTS_PRODUCTION_COMPOSITION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Customer Accounts production contribution is invalid.",
    )
    .with_internal_reference(error.to_string())
}

pub const CRATE_NAME: &str = "crm-customer-accounts-capability-composition";
