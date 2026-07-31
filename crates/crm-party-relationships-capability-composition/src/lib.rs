#![forbid(unsafe_code)]

//! Pre-authorization application validation for Party Relationship endpoints.

use crm_application_composition::{
    ActivationGatedMutationValidator, ActivationGatedQueryValidator, ModuleActivationPort,
    ModuleContributionSet,
};
use crm_capability_runtime::{
    CapabilityDefinition, CapabilityRequest, CapabilitySemanticValidator,
    TransactionalCapabilityExecutor,
};
use crm_core_data::{PostgresDataStore, PostgresTransactionalAggregateExecutor};
use crm_module_sdk::{ErrorCategory, PortFuture, SdkError};
use crm_party_reference_composition::PartyReferenceReader;
use crm_party_relationships_capability_adapter::{
    CREATE_CAPABILITY, MUTATION_CAPABILITY_IDS, PartyRelationshipCapabilityPlanner,
    capability_definitions as adapter_mutation_capability_definitions,
    referenced_party_ids_from_create,
};
use crm_party_relationships_query_adapter::{
    PartyRelationshipQueryAdapter,
    query_capability_definitions as adapter_query_capability_definitions,
};
use crm_query_runtime::{
    CursorCodec, QueryExecutor, QuerySemanticValidator, QueryVisibilityAuthorizer,
};
use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

#[derive(Clone)]
pub struct PartyRelationshipReferenceSemanticValidator {
    parties: Arc<dyn PartyReferenceReader>,
}

impl fmt::Debug for PartyRelationshipReferenceSemanticValidator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PartyRelationshipReferenceSemanticValidator")
            .field("parties", &"dyn PartyReferenceReader")
            .finish()
    }
}

impl PartyRelationshipReferenceSemanticValidator {
    pub fn new(parties: Arc<dyn PartyReferenceReader>) -> Self {
        Self { parties }
    }
}

impl CapabilitySemanticValidator for PartyRelationshipReferenceSemanticValidator {
    fn validate<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a CapabilityRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        if !MUTATION_CAPABILITY_IDS.contains(&definition.capability_id.as_str()) {
            return Box::pin(async { Err(unsupported_capability()) });
        }
        if definition.capability_id.as_str() != CREATE_CAPABILITY {
            return Box::pin(async { Ok(()) });
        }
        let references = referenced_party_ids_from_create(request);
        Box::pin(async move {
            let unique = references?
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

#[derive(Clone)]
pub struct PartyRelationshipsProductionDependencies {
    pub store: PostgresDataStore,
    pub parties: Arc<dyn PartyReferenceReader>,
    pub activation: Arc<dyn ModuleActivationPort>,
    pub visibility_authorizer: Arc<dyn QueryVisibilityAuthorizer>,
    pub cursor_key: [u8; 32],
}

/// Returns the exact Party Relationships mutation inventory owned by this
/// production composition package.
pub fn mutation_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    adapter_mutation_capability_definitions()
}

/// Returns the exact Party Relationships query inventory owned by this
/// production composition package.
pub fn query_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    adapter_query_capability_definitions()
}

/// Builds the complete Party Relationships mutation/query contribution inside
/// the owner package while preserving Party reference validation.
pub fn build_contribution(
    dependencies: PartyRelationshipsProductionDependencies,
) -> Result<ModuleContributionSet, SdkError> {
    let PartyRelationshipsProductionDependencies {
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
            Arc::new(PartyRelationshipCapabilityPlanner),
        ));
    let mutation_validator: Arc<dyn CapabilitySemanticValidator> =
        Arc::new(ActivationGatedMutationValidator::new(
            activation.clone(),
            Arc::new(PartyRelationshipReferenceSemanticValidator::new(parties)),
        ));
    contributions
        .add_mutations(
            mutation_capability_definitions()?,
            mutation_validator,
            mutation_executor,
        )
        .map_err(production_composition_error)?;

    let query_adapter = Arc::new(PartyRelationshipQueryAdapter::new(
        store,
        party_relationships_cursor(cursor_key)?,
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

fn party_relationships_cursor(key: [u8; 32]) -> Result<CursorCodec, SdkError> {
    CursorCodec::new(key).map_err(|error| {
        SdkError::new(
            "PARTY_RELATIONSHIPS_CURSOR_CONFIGURATION_INVALID",
            ErrorCategory::Internal,
            false,
            "The Party Relationships cursor configuration is invalid.",
        )
        .with_internal_reference(error.to_string())
    })
}

fn production_composition_error(error: impl fmt::Display) -> SdkError {
    SdkError::new(
        "PARTY_RELATIONSHIPS_PRODUCTION_COMPOSITION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Party Relationships production contribution is invalid.",
    )
    .with_internal_reference(error.to_string())
}

fn reference_unavailable() -> SdkError {
    SdkError::new(
        "PARTY_RELATIONSHIPS_PARTY_REFERENCE_UNAVAILABLE",
        ErrorCategory::InvalidArgument,
        false,
        "One or more referenced Parties are unavailable.",
    )
}

fn unsupported_capability() -> SdkError {
    SdkError::new(
        "PARTY_RELATIONSHIPS_COMPOSITION_CAPABILITY_UNSUPPORTED",
        ErrorCategory::Internal,
        false,
        "The Party Relationship mutation capability is not configured for this composition boundary.",
    )
}

pub const CRATE_NAME: &str = "crm-party-relationships-capability-composition";
