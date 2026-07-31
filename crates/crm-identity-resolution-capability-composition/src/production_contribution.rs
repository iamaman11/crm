#![forbid(unsafe_code)]

use crate::{
    IdentityResolutionCapabilityExecutor, IdentityResolutionCapabilitySemanticValidator,
    PostgresIdentityResolutionReferenceReader,
};
use crm_application_composition::{
    ActivationGatedMutationValidator, ActivationGatedQueryValidator, ModuleActivationPort,
    ModuleContributionSet,
};
use crm_capability_runtime::{
    CapabilityDefinition, CapabilitySemanticValidator, TransactionalCapabilityExecutor,
};
use crm_core_data::{PostgresDataStore, PostgresTransactionalAggregateExecutor};
use crm_identity_resolution_capability_adapter::{
    CANDIDATE_MUTATION_CAPABILITY_IDS, IdentityResolutionCapabilityPlanner,
    MERGE_MUTATION_CAPABILITY_IDS,
    capability_definitions as adapter_mutation_capability_definitions,
};
use crm_identity_resolution_merge_composition::{
    MergeLineageCapabilityExecutor, MergeLineageCapabilitySemanticValidator,
    PostgresMergeLineageReferenceReader,
};
use crm_identity_resolution_merge_query_adapter::{
    IdentityResolutionMergeQueryAdapter,
    query_capability_definitions as merge_query_capability_definitions,
};
use crm_identity_resolution_query_adapter::{
    IdentityResolutionQueryAdapter,
    query_capability_definitions as candidate_query_capability_definitions,
};
use crm_module_sdk::{ErrorCategory, SdkError};
use crm_query_runtime::{
    CursorCodec, QueryExecutor, QuerySemanticValidator, QueryVisibilityAuthorizer,
};
use std::fmt;
use std::sync::Arc;

#[derive(Clone)]
pub struct IdentityResolutionProductionDependencies {
    pub store: PostgresDataStore,
    pub activation: Arc<dyn ModuleActivationPort>,
    pub visibility_authorizer: Arc<dyn QueryVisibilityAuthorizer>,
    pub cursor_key: [u8; 32],
}

pub fn mutation_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    adapter_mutation_capability_definitions()
}

pub fn query_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    let mut definitions = candidate_query_capability_definitions()?;
    definitions.extend(merge_query_capability_definitions()?);
    Ok(definitions)
}

pub fn build_contribution(
    dependencies: IdentityResolutionProductionDependencies,
) -> Result<ModuleContributionSet, SdkError> {
    let IdentityResolutionProductionDependencies {
        store,
        activation,
        visibility_authorizer,
        cursor_key,
    } = dependencies;
    let mut contributions = ModuleContributionSet::new();
    let identity_aggregate: Arc<dyn TransactionalCapabilityExecutor> =
        Arc::new(PostgresTransactionalAggregateExecutor::new(
            store.clone(),
            Arc::new(IdentityResolutionCapabilityPlanner),
        ));
    let definitions = mutation_capability_definitions()?;

    let candidate_validator: Arc<dyn CapabilitySemanticValidator> =
        Arc::new(ActivationGatedMutationValidator::new(
            activation.clone(),
            Arc::new(IdentityResolutionCapabilitySemanticValidator::new(
                Arc::new(PostgresIdentityResolutionReferenceReader::new(
                    store.clone(),
                )),
            )),
        ));
    contributions
        .add_mutations(
            select_definitions(&definitions, CANDIDATE_MUTATION_CAPABILITY_IDS),
            candidate_validator,
            Arc::new(IdentityResolutionCapabilityExecutor::new(
                identity_aggregate.clone(),
            )),
        )
        .map_err(production_composition_error)?;

    let merge_validator: Arc<dyn CapabilitySemanticValidator> =
        Arc::new(ActivationGatedMutationValidator::new(
            activation.clone(),
            Arc::new(MergeLineageCapabilitySemanticValidator::new(Arc::new(
                PostgresMergeLineageReferenceReader::new(store.clone()),
            ))),
        ));
    contributions
        .add_mutations(
            select_definitions(&definitions, MERGE_MUTATION_CAPABILITY_IDS),
            merge_validator,
            Arc::new(MergeLineageCapabilityExecutor::new(identity_aggregate)),
        )
        .map_err(production_composition_error)?;

    let candidate_queries = Arc::new(IdentityResolutionQueryAdapter::new(
        store.clone(),
        cursor(cursor_key)?,
        visibility_authorizer.clone(),
    )?);
    let candidate_query_validator: Arc<dyn QuerySemanticValidator> = Arc::new(
        ActivationGatedQueryValidator::new(activation.clone(), candidate_queries.clone()),
    );
    let candidate_query_executor: Arc<dyn QueryExecutor> = candidate_queries;
    contributions
        .add_queries(
            candidate_query_capability_definitions()?,
            candidate_query_validator,
            candidate_query_executor,
        )
        .map_err(production_composition_error)?;

    let merge_queries = Arc::new(IdentityResolutionMergeQueryAdapter::new(
        store,
        cursor(cursor_key)?,
        visibility_authorizer,
    )?);
    let merge_query_validator: Arc<dyn QuerySemanticValidator> = Arc::new(
        ActivationGatedQueryValidator::new(activation, merge_queries.clone()),
    );
    let merge_query_executor: Arc<dyn QueryExecutor> = merge_queries;
    contributions
        .add_queries(
            merge_query_capability_definitions()?,
            merge_query_validator,
            merge_query_executor,
        )
        .map_err(production_composition_error)?;

    Ok(contributions)
}

fn select_definitions(
    definitions: &[CapabilityDefinition],
    capability_ids: &[&str],
) -> Vec<CapabilityDefinition> {
    definitions
        .iter()
        .filter(|definition| capability_ids.contains(&definition.capability_id.as_str()))
        .cloned()
        .collect()
}

fn cursor(key: [u8; 32]) -> Result<CursorCodec, SdkError> {
    CursorCodec::new(key).map_err(|error| {
        SdkError::new(
            "IDENTITY_RESOLUTION_CURSOR_CONFIGURATION_INVALID",
            ErrorCategory::Internal,
            false,
            "The Identity Resolution cursor configuration is invalid.",
        )
        .with_internal_reference(error.to_string())
    })
}

fn production_composition_error(error: impl fmt::Display) -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_PRODUCTION_COMPOSITION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Identity Resolution production contribution is invalid.",
    )
    .with_internal_reference(error.to_string())
}
