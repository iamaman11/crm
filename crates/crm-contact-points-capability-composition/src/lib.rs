#![forbid(unsafe_code)]

//! Pre-authorization application validation for Contact Point Party references.

use crm_application_composition::{
    ActivationGatedMutationValidator, ActivationGatedQueryValidator, ModuleActivationPort,
    ModuleContributionSet,
};
use crm_capability_runtime::{
    CapabilityDefinition, CapabilityRequest, CapabilitySemanticValidator,
    TransactionalCapabilityExecutor,
};
use crm_contact_points_capability_adapter::{
    CREATE_CAPABILITY, ContactPointCapabilityPlanner, MUTATION_CAPABILITY_IDS,
    capability_definitions as adapter_mutation_capability_definitions,
    referenced_party_id_from_create,
};
use crm_contact_points_query_adapter::{
    ContactPointQueryAdapter, query_capability_definitions as adapter_query_capability_definitions,
};
use crm_core_data::{PostgresDataStore, PostgresTransactionalAggregateExecutor};
use crm_module_sdk::{ErrorCategory, PortFuture, SdkError};
use crm_party_reference_composition::PartyReferenceReader;
use crm_query_runtime::{
    CursorCodec, QueryExecutor, QuerySemanticValidator, QueryVisibilityAuthorizer,
};
use std::fmt;
use std::sync::Arc;

#[derive(Clone)]
pub struct ContactPointPartyReferenceSemanticValidator {
    parties: Arc<dyn PartyReferenceReader>,
}

impl fmt::Debug for ContactPointPartyReferenceSemanticValidator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ContactPointPartyReferenceSemanticValidator")
            .field("parties", &"dyn PartyReferenceReader")
            .finish()
    }
}

impl ContactPointPartyReferenceSemanticValidator {
    pub fn new(parties: Arc<dyn PartyReferenceReader>) -> Self {
        Self { parties }
    }
}

impl CapabilitySemanticValidator for ContactPointPartyReferenceSemanticValidator {
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
        let reference = referenced_party_id_from_create(request);
        Box::pin(async move {
            let reference = reference?;
            if self
                .parties
                .exists(&request.context.execution.tenant_id, reference.as_str())
                .await?
            {
                Ok(())
            } else {
                Err(reference_unavailable())
            }
        })
    }
}

#[derive(Clone)]
pub struct ContactPointsProductionDependencies {
    pub store: PostgresDataStore,
    pub parties: Arc<dyn PartyReferenceReader>,
    pub activation: Arc<dyn ModuleActivationPort>,
    pub visibility_authorizer: Arc<dyn QueryVisibilityAuthorizer>,
    pub cursor_key: [u8; 32],
}

/// Returns the exact Contact Points mutation inventory owned by this production
/// composition package.
pub fn mutation_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    adapter_mutation_capability_definitions()
}

/// Returns the exact Contact Points query inventory owned by this production
/// composition package.
pub fn query_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    adapter_query_capability_definitions()
}

/// Builds the complete Contact Points mutation/query contribution inside the
/// owner composition package while preserving Party reference validation.
pub fn build_contribution(
    dependencies: ContactPointsProductionDependencies,
) -> Result<ModuleContributionSet, SdkError> {
    let ContactPointsProductionDependencies {
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
            Arc::new(ContactPointCapabilityPlanner),
        ));
    let mutation_validator: Arc<dyn CapabilitySemanticValidator> =
        Arc::new(ActivationGatedMutationValidator::new(
            activation.clone(),
            Arc::new(ContactPointPartyReferenceSemanticValidator::new(parties)),
        ));
    contributions
        .add_mutations(
            mutation_capability_definitions()?,
            mutation_validator,
            mutation_executor,
        )
        .map_err(production_composition_error)?;

    let query_adapter = Arc::new(ContactPointQueryAdapter::new(
        store,
        contact_points_cursor(cursor_key)?,
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

fn contact_points_cursor(key: [u8; 32]) -> Result<CursorCodec, SdkError> {
    CursorCodec::new(key).map_err(|error| {
        SdkError::new(
            "CONTACT_POINTS_CURSOR_CONFIGURATION_INVALID",
            ErrorCategory::Internal,
            false,
            "The Contact Points cursor configuration is invalid.",
        )
        .with_internal_reference(error.to_string())
    })
}

fn production_composition_error(error: impl fmt::Display) -> SdkError {
    SdkError::new(
        "CONTACT_POINTS_PRODUCTION_COMPOSITION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Contact Points production contribution is invalid.",
    )
    .with_internal_reference(error.to_string())
}

fn reference_unavailable() -> SdkError {
    SdkError::new(
        "CONTACT_POINTS_PARTY_REFERENCE_UNAVAILABLE",
        ErrorCategory::InvalidArgument,
        false,
        "The referenced Party is unavailable.",
    )
}

fn unsupported_capability() -> SdkError {
    SdkError::new(
        "CONTACT_POINTS_COMPOSITION_CAPABILITY_UNSUPPORTED",
        ErrorCategory::Internal,
        false,
        "The Contact Point mutation capability is not configured for this composition boundary.",
    )
}

pub const CRATE_NAME: &str = "crm-contact-points-capability-composition";
