use crate::legacy::{CustomerPrivacyProduction, CustomerPrivacyProductionDependencies};
use crate::restriction::{
    build_contribution_with_restrictions, build_production_with_restrictions,
};
use crm_application_composition::{
    ActivationGatedMutationValidator, ActivationGatedQueryValidator, ModuleContributionSet,
    NoopMutationSemanticValidator,
};
use crm_capability_runtime::CapabilitySemanticValidator;
pub use crm_customer_privacy::{
    ACTION_PLAN_RECORD_TYPE, ACTION_PLAN_STATE_MAXIMUM_BYTES,
    ACTION_PLAN_STATE_RETENTION_POLICY_ID, ACTION_PLAN_STATE_SCHEMA_ID,
    ACTION_PLAN_STATE_SCHEMA_VERSION, ActionPlanningPolicy, ContributionCompletenessProof,
    CustomerDataLegalHold, DiscoveryOwnerScopeContribution, DiscoveryScopeSnapshot, EvidenceClass,
    LegalHoldScope, OwnerScopeContract, OwnerScopeContribution, OwnerScopeRegistry,
    PlannedPrivacyAction, PrivacyActionPlan, PrivacyCase, PrivacyCaseKind,
    PrivacyRetentionDecisionItem, PrivacyRetentionDecisionSet, RetentionDecisionReason,
    ScopeDiscoveryLineage, ScopeResource, SubjectVerificationMethod,
    action_plan_state_descriptor_hash, encode_action_plan_state,
};
pub use {
    crm_customer_privacy_application::{
        PrivacyRetentionEvaluationService, RetentionEvaluationCommit,
        RetentionEvaluationInvocation, RetentionEvaluationPersistencePort,
        mutation_capability_definitions_with_complete_control_lifecycle,
        mutation_capability_definitions_with_restrictions_and_legal_holds,
    },
    crm_customer_privacy_postgres::{
        PostgresRetentionEvaluationPersistence, legal_hold_persisted_payload,
        privacy_case_persisted_payload,
    },
};
use crm_customer_privacy_application::{
    place_customer_data_legal_hold_capability_definition,
    query_capability_definitions_with_complete_control_lifecycle,
    release_customer_data_legal_hold_capability_definition,
    release_processing_restriction_capability_definition,
};
use crm_customer_privacy_postgres::{
    postgres_legal_hold_place_executor, postgres_legal_hold_release_executor,
    postgres_restriction_release_executor,
};
use crm_customer_privacy_query_adapter::CustomerPrivacyControlQueryAdapter;
pub use crm_customer_privacy_query_adapter::control_query_capability_definitions;
use crm_module_sdk::{ErrorCategory, SdkError};
use crm_query_runtime::{CursorCodec, QueryExecutor, QuerySemanticValidator};
use std::sync::Arc;

/// Complete step-six owner package.
///
/// The public contribution adds only legal-hold placement. Retention
/// adjudication remains a trusted-internal service and performs no owner action.
pub struct CustomerPrivacyHoldRetentionProduction {
    pub base: CustomerPrivacyProduction,
    pub retention: PrivacyRetentionEvaluationService,
}

pub fn build_production_with_holds(
    dependencies: CustomerPrivacyProductionDependencies,
) -> Result<CustomerPrivacyHoldRetentionProduction, SdkError> {
    let contribution = build_contribution_with_holds(dependencies.clone())?;
    let mut base = build_production_with_restrictions(dependencies.clone())?;
    base.contribution = contribution;
    let retention = build_internal_retention(&dependencies);
    Ok(CustomerPrivacyHoldRetentionProduction { base, retention })
}

fn build_internal_retention(
    dependencies: &CustomerPrivacyProductionDependencies,
) -> PrivacyRetentionEvaluationService {
    PrivacyRetentionEvaluationService::new(
        dependencies.activation.clone(),
        Arc::new(PostgresRetentionEvaluationPersistence::new(Arc::new(
            dependencies.store.clone(),
        ))),
    )
}

pub fn build_contribution_with_holds(
    dependencies: CustomerPrivacyProductionDependencies,
) -> Result<ModuleContributionSet, SdkError> {
    let inventory = mutation_capability_definitions_with_restrictions_and_legal_holds()?;
    if inventory.len() != 7 {
        return Err(composition_error(
            "step-six Customer Privacy inventory must contain exactly seven mutations",
        ));
    }
    let legal_hold_definition = place_customer_data_legal_hold_capability_definition()?;
    if inventory.get(2).map(|definition| &definition.capability_id)
        != Some(&legal_hold_definition.capability_id)
    {
        return Err(composition_error(
            "legal_hold.place must remain the third owner mutation in the exact inventory",
        ));
    }

    let mut contribution = build_contribution_with_restrictions(dependencies.clone())?;
    let validator: Arc<dyn CapabilitySemanticValidator> =
        Arc::new(ActivationGatedMutationValidator::new(
            dependencies.activation,
            Arc::new(NoopMutationSemanticValidator),
        ));
    contribution
        .add_mutations(
            [legal_hold_definition],
            validator,
            postgres_legal_hold_place_executor(dependencies.store),
        )
        .map_err(composition_error)?;
    Ok(contribution)
}

pub fn build_contribution_with_complete_control_lifecycle(
    dependencies: CustomerPrivacyProductionDependencies,
) -> Result<ModuleContributionSet, SdkError> {
    let mutation_inventory = mutation_capability_definitions_with_complete_control_lifecycle()?;
    if mutation_inventory.len() != 9 {
        return Err(composition_error(
            "complete Customer Privacy inventory must contain exactly nine mutations",
        ));
    }
    let query_inventory = query_capability_definitions_with_complete_control_lifecycle()?;
    if query_inventory.len() != 7 {
        return Err(composition_error(
            "complete Customer Privacy inventory must contain exactly seven queries",
        ));
    }

    let restriction_release = release_processing_restriction_capability_definition()?;
    let legal_hold_release = release_customer_data_legal_hold_capability_definition()?;
    if mutation_inventory
        .get(2)
        .map(|definition| &definition.capability_id)
        != Some(&restriction_release.capability_id)
        || mutation_inventory
            .get(4)
            .map(|definition| &definition.capability_id)
            != Some(&legal_hold_release.capability_id)
    {
        return Err(composition_error(
            "control release mutations must remain at their frozen inventory positions",
        ));
    }

    let control_queries = control_query_capability_definitions()?;
    if control_queries.len() != 3 {
        return Err(composition_error(
            "complete Customer Privacy inventory must add exactly three control queries",
        ));
    }

    let mut contribution = build_contribution_with_holds(dependencies.clone())?;
    add_release_mutation(
        &mut contribution,
        restriction_release,
        dependencies.activation.clone(),
        postgres_restriction_release_executor(dependencies.store.clone()),
    )?;
    add_release_mutation(
        &mut contribution,
        legal_hold_release,
        dependencies.activation.clone(),
        postgres_legal_hold_release_executor(dependencies.store.clone()),
    )?;

    let control_adapter = Arc::new(CustomerPrivacyControlQueryAdapter::new_with_cursor(
        dependencies.store,
        cursor(dependencies.cursor_key)?,
        dependencies.visibility_authorizer,
    ));
    let query_validator: Arc<dyn QuerySemanticValidator> = Arc::new(
        ActivationGatedQueryValidator::new(dependencies.activation, control_adapter.clone()),
    );
    let query_executor: Arc<dyn QueryExecutor> = control_adapter;
    contribution
        .add_queries(control_queries, query_validator, query_executor)
        .map_err(composition_error)?;
    Ok(contribution)
}

fn add_release_mutation(
    contribution: &mut ModuleContributionSet,
    definition: crm_capability_runtime::CapabilityDefinition,
    activation: Arc<dyn crm_application_composition::ModuleActivationPort>,
    executor: Arc<dyn crm_capability_runtime::TransactionalCapabilityExecutor>,
) -> Result<(), SdkError> {
    let validator: Arc<dyn CapabilitySemanticValidator> = Arc::new(
        ActivationGatedMutationValidator::new(activation, Arc::new(NoopMutationSemanticValidator)),
    );
    contribution
        .add_mutations([definition], validator, executor)
        .map_err(composition_error)?;
    Ok(())
}

fn cursor(key: [u8; 32]) -> Result<CursorCodec, SdkError> {
    CursorCodec::new(key).map_err(composition_error)
}

fn composition_error(error: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_HOLD_RETENTION_PRODUCTION_COMPOSITION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Customer Privacy legal-hold and retention production package is invalid.",
    )
    .with_internal_reference(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_step_six_inventory_is_seven() {
        let inventory =
            mutation_capability_definitions_with_restrictions_and_legal_holds().unwrap();
        assert_eq!(inventory.len(), 7);
        assert_eq!(
            inventory[1].capability_id.as_str(),
            "customer_privacy.restriction.place"
        );
        assert_eq!(
            inventory[2].capability_id.as_str(),
            "customer_privacy.legal_hold.place"
        );
    }

    #[test]
    fn complete_control_lifecycle_inventory_is_nine_mutations_and_seven_queries() {
        let mutations = mutation_capability_definitions_with_complete_control_lifecycle().unwrap();
        let queries = query_capability_definitions_with_complete_control_lifecycle().unwrap();
        assert_eq!(mutations.len(), 9);
        assert_eq!(queries.len(), 7);
        assert_eq!(
            mutations[2].capability_id.as_str(),
            "customer_privacy.restriction.release"
        );
        assert_eq!(
            mutations[4].capability_id.as_str(),
            "customer_privacy.legal_hold.release"
        );
    }
}
