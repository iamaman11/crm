#![forbid(unsafe_code)]

//! Governed mutation adapter for the authoritative Data Quality owner domain.
//!
//! Immutable definitions and durable Party evaluation jobs remain owned by
//! `crm.data-quality`. Governed Party source reads are supplied separately by
//! application composition and never by direct cross-owner storage access.

mod completeness_profile_planner;
mod evaluation_job_planner;
mod evaluation_materialization_planner;
mod evaluation_stage_planner;
mod finding_stewardship_planner;
mod finding_wire;
mod remediation_planner;
mod remediation_registration;
mod rule_set_planner;

pub use completeness_profile_planner::{
    CompletenessProfileReferenceScope, DataQualityCompletenessProfileCapabilityPlanner,
    completeness_profile_reference_scope_from_request,
    completeness_profile_rule_set_version_id_from_snapshot,
    party_completeness_profile_from_definition, party_completeness_profile_persisted_contract,
    party_completeness_profile_persisted_payload, party_completeness_profile_to_wire,
};
pub use evaluation_job_planner::{
    DataQualityEvaluationJobCapabilityPlanner, EvaluationReferenceScope,
    evaluation_reference_scope_from_request, party_evaluation_job_from_snapshot,
    party_evaluation_job_persisted_contract, party_evaluation_job_persisted_payload,
    party_evaluation_job_to_wire,
};
pub use evaluation_materialization_planner::{
    DataQualityEvaluationMaterializationPlanner, ExistingPartyFinding,
    ExistingPartyFindingObservation, party_completeness_result_persisted_contract,
    party_completeness_result_persisted_payload, party_completeness_result_record_ref,
    party_finding_observation_persisted_contract, party_finding_observation_persisted_payload,
    party_finding_observation_record_ref, party_finding_persisted_contract,
    party_finding_persisted_payload, party_finding_record_ref,
    party_rule_outcome_persisted_contract, party_rule_outcome_persisted_payload,
    party_rule_outcome_record_ref,
};
pub use evaluation_stage_planner::{
    DataQualityEvaluationStagePlanner, party_evaluation_input_persisted_contract,
    party_evaluation_input_persisted_payload, party_evaluation_input_record_ref,
};
pub use finding_stewardship_planner::{
    DataQualityFindingStewardshipPlanner, party_finding_from_snapshot,
};
pub use finding_wire::party_finding_to_wire;
pub use remediation_planner::{
    DataQualityRemediationCompletionPlanner, remediation_attempt_persisted_contract,
    remediation_attempt_persisted_payload, remediation_attempt_record_ref,
    remediation_attempt_to_wire,
};
pub use remediation_registration::*;
pub use rule_set_planner::{
    DataQualityRuleSetCapabilityPlanner, party_rule_set_from_definition,
    party_rule_set_persisted_contract, party_rule_set_persisted_payload, party_rule_set_to_wire,
};

use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRisk};
use crm_data_quality::{PartyCompletenessProfileVersion, PartyRuleSetVersion};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, RecordSnapshot, SdkError,
};

pub const MODULE_ID: &str = crm_data_quality::MODULE_ID;
pub const PARTY_RULE_SET_VERSION_RECORD_TYPE: &str =
    crm_data_quality::PARTY_RULE_SET_VERSION_RECORD_TYPE;
pub const PARTY_COMPLETENESS_PROFILE_VERSION_RECORD_TYPE: &str =
    crm_data_quality::PARTY_COMPLETENESS_PROFILE_VERSION_RECORD_TYPE;
pub const PARTY_EVALUATION_JOB_RECORD_TYPE: &str =
    crm_data_quality::PARTY_EVALUATION_JOB_RECORD_TYPE;

pub const PUBLISH_PARTY_RULE_SET_CAPABILITY: &str = "data_quality.party.rule_set.publish";
pub const PUBLISH_PARTY_RULE_SET_REQUEST_SCHEMA: &str =
    "crm.data_quality.v1.PublishPartyRuleSetVersionRequest";
pub const PUBLISH_PARTY_RULE_SET_RESPONSE_SCHEMA: &str =
    "crm.data_quality.v1.PublishPartyRuleSetVersionResponse";
pub const PARTY_RULE_SET_PUBLISHED_EVENT_TYPE: &str = "data_quality.party.rule_set.published";
pub const PARTY_RULE_SET_PUBLISHED_EVENT_SCHEMA: &str =
    "crm.data_quality.v1.PartyRuleSetVersionPublishedEvent";

pub const PUBLISH_PARTY_COMPLETENESS_PROFILE_CAPABILITY: &str =
    "data_quality.party.completeness_profile.publish";
pub const PUBLISH_PARTY_COMPLETENESS_PROFILE_REQUEST_SCHEMA: &str =
    "crm.data_quality.v1.PublishPartyCompletenessProfileVersionRequest";
pub const PUBLISH_PARTY_COMPLETENESS_PROFILE_RESPONSE_SCHEMA: &str =
    "crm.data_quality.v1.PublishPartyCompletenessProfileVersionResponse";
pub const PARTY_COMPLETENESS_PROFILE_PUBLISHED_EVENT_TYPE: &str =
    "data_quality.party.completeness_profile.published";
pub const PARTY_COMPLETENESS_PROFILE_PUBLISHED_EVENT_SCHEMA: &str =
    "crm.data_quality.v1.PartyCompletenessProfileVersionPublishedEvent";

pub const REQUEST_PARTY_EVALUATION_CAPABILITY: &str = "data_quality.party.evaluation.request";
pub const REQUEST_PARTY_EVALUATION_REQUEST_SCHEMA: &str =
    "crm.data_quality.v1.RequestPartyEvaluationRequest";
pub const REQUEST_PARTY_EVALUATION_RESPONSE_SCHEMA: &str =
    "crm.data_quality.v1.RequestPartyEvaluationResponse";
pub const PARTY_EVALUATION_REQUESTED_EVENT_TYPE: &str = "data_quality.party.evaluation.requested";
pub const PARTY_EVALUATION_REQUESTED_EVENT_SCHEMA: &str =
    "crm.data_quality.v1.PartyEvaluationRequestedEvent";

pub const ASSIGN_FINDING_CAPABILITY: &str = "data_quality.finding.assign";
pub const ASSIGN_FINDING_REQUEST_SCHEMA: &str =
    "crm.data_quality.v1.AssignDataQualityFindingRequest";
pub const ASSIGN_FINDING_RESPONSE_SCHEMA: &str =
    "crm.data_quality.v1.AssignDataQualityFindingResponse";
pub const ACKNOWLEDGE_FINDING_CAPABILITY: &str = "data_quality.finding.acknowledge";
pub const ACKNOWLEDGE_FINDING_REQUEST_SCHEMA: &str =
    "crm.data_quality.v1.AcknowledgeDataQualityFindingRequest";
pub const ACKNOWLEDGE_FINDING_RESPONSE_SCHEMA: &str =
    "crm.data_quality.v1.AcknowledgeDataQualityFindingResponse";
pub const WAIVE_FINDING_CAPABILITY: &str = "data_quality.finding.waive";
pub const WAIVE_FINDING_REQUEST_SCHEMA: &str =
    "crm.data_quality.v1.WaiveDataQualityFindingRequest";
pub const WAIVE_FINDING_RESPONSE_SCHEMA: &str =
    "crm.data_quality.v1.WaiveDataQualityFindingResponse";
pub const FINDING_STATUS_CHANGED_EVENT_TYPE: &str = "data_quality.finding.status_changed";
pub const FINDING_STATUS_CHANGED_EVENT_SCHEMA: &str =
    "crm.data_quality.v1.DataQualityFindingStatusChangedEvent";
pub const FINDING_ASSIGNMENT_CHANGED_EVENT_TYPE: &str =
    "data_quality.finding.assignment_changed";
pub const FINDING_ASSIGNMENT_CHANGED_EVENT_SCHEMA: &str =
    "crm.data_quality.v1.DataQualityFindingAssignmentChangedEvent";

pub const STAGE_PARTY_EVALUATION_INPUT_CAPABILITY: &str =
    "data_quality.party.evaluation.internal.stage";
pub const STAGE_PARTY_EVALUATION_INPUT_REQUEST_SCHEMA: &str =
    "crm.data_quality.v1.StagePartyEvaluationInputRequest";
pub const STAGE_PARTY_EVALUATION_INPUT_RESPONSE_SCHEMA: &str =
    "crm.data_quality.v1.StagePartyEvaluationInputResponse";
pub const PARTY_EVALUATION_STAGED_EVENT_TYPE: &str = "data_quality.party.evaluation.staged";
pub const PARTY_EVALUATION_STAGED_EVENT_SCHEMA: &str =
    "crm.data_quality.v1.PartyEvaluationStagedEvent";

pub const MATERIALIZE_PARTY_EVALUATION_CAPABILITY: &str =
    "data_quality.party.evaluation.internal.materialize";
pub const MATERIALIZE_PARTY_EVALUATION_REQUEST_SCHEMA: &str =
    "crm.data_quality.v1.MaterializePartyEvaluationRequest";
pub const MATERIALIZE_PARTY_EVALUATION_RESPONSE_SCHEMA: &str =
    "crm.data_quality.v1.MaterializePartyEvaluationResponse";
pub const PARTY_EVALUATION_MATERIALIZED_EVENT_TYPE: &str =
    "data_quality.party.evaluation.materialized";
pub const PARTY_EVALUATION_MATERIALIZED_EVENT_SCHEMA: &str =
    "crm.data_quality.v1.PartyEvaluationMaterializedEvent";

pub const MUTATION_CAPABILITY_IDS: &[&str] = &[
    PUBLISH_PARTY_RULE_SET_CAPABILITY,
    PUBLISH_PARTY_COMPLETENESS_PROFILE_CAPABILITY,
    REQUEST_PARTY_EVALUATION_CAPABILITY,
    ASSIGN_FINDING_CAPABILITY,
    ACKNOWLEDGE_FINDING_CAPABILITY,
    WAIVE_FINDING_CAPABILITY,
    REMEDIATE_PARTY_DISPLAY_NAME_CAPABILITY,
];

pub fn party_rule_set_from_snapshot(
    snapshot: &RecordSnapshot,
) -> Result<PartyRuleSetVersion, SdkError> {
    ensure_immutable_version(snapshot, "Party rule-set version")?;
    rule_set_planner::party_rule_set_from_snapshot(snapshot)
}

pub fn party_completeness_profile_from_immutable_snapshot(
    snapshot: &RecordSnapshot,
    rule_set: &PartyRuleSetVersion,
) -> Result<PartyCompletenessProfileVersion, SdkError> {
    ensure_immutable_version(snapshot, "Party completeness-profile version")?;
    completeness_profile_planner::party_completeness_profile_from_snapshot(snapshot, rule_set)
}

pub fn capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    Ok(vec![
        rule_set_capability_definition()?,
        completeness_profile_capability_definition()?,
        evaluation_request_capability_definition()?,
        finding_assign_capability_definition()?,
        finding_acknowledge_capability_definition()?,
        finding_waive_capability_definition()?,
        remediation_capability_definition()?,
    ])
}

pub fn capability_definition() -> Result<CapabilityDefinition, SdkError> {
    rule_set_capability_definition()
}

pub fn rule_set_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    mutation_definition(
        PUBLISH_PARTY_RULE_SET_CAPABILITY,
        PUBLISH_PARTY_RULE_SET_REQUEST_SCHEMA,
        PUBLISH_PARTY_RULE_SET_RESPONSE_SCHEMA,
        DataClass::Confidential,
    )
}

pub fn completeness_profile_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    mutation_definition(
        PUBLISH_PARTY_COMPLETENESS_PROFILE_CAPABILITY,
        PUBLISH_PARTY_COMPLETENESS_PROFILE_REQUEST_SCHEMA,
        PUBLISH_PARTY_COMPLETENESS_PROFILE_RESPONSE_SCHEMA,
        DataClass::Confidential,
    )
}

pub fn evaluation_request_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    mutation_definition(
        REQUEST_PARTY_EVALUATION_CAPABILITY,
        REQUEST_PARTY_EVALUATION_REQUEST_SCHEMA,
        REQUEST_PARTY_EVALUATION_RESPONSE_SCHEMA,
        DataClass::Personal,
    )
}

pub fn finding_assign_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    mutation_definition(
        ASSIGN_FINDING_CAPABILITY,
        ASSIGN_FINDING_REQUEST_SCHEMA,
        ASSIGN_FINDING_RESPONSE_SCHEMA,
        DataClass::Personal,
    )
}

pub fn finding_acknowledge_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    mutation_definition(
        ACKNOWLEDGE_FINDING_CAPABILITY,
        ACKNOWLEDGE_FINDING_REQUEST_SCHEMA,
        ACKNOWLEDGE_FINDING_RESPONSE_SCHEMA,
        DataClass::Personal,
    )
}

pub fn finding_waive_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    mutation_definition(
        WAIVE_FINDING_CAPABILITY,
        WAIVE_FINDING_REQUEST_SCHEMA,
        WAIVE_FINDING_RESPONSE_SCHEMA,
        DataClass::Personal,
    )
}

pub fn evaluation_stage_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    mutation_definition(
        STAGE_PARTY_EVALUATION_INPUT_CAPABILITY,
        STAGE_PARTY_EVALUATION_INPUT_REQUEST_SCHEMA,
        STAGE_PARTY_EVALUATION_INPUT_RESPONSE_SCHEMA,
        DataClass::Personal,
    )
}

pub fn evaluation_materialization_capability_definition() -> Result<CapabilityDefinition, SdkError>
{
    mutation_definition(
        MATERIALIZE_PARTY_EVALUATION_CAPABILITY,
        MATERIALIZE_PARTY_EVALUATION_REQUEST_SCHEMA,
        MATERIALIZE_PARTY_EVALUATION_RESPONSE_SCHEMA,
        DataClass::Personal,
    )
}

fn mutation_definition(
    capability_id: &'static str,
    input_schema: &'static str,
    output_schema: &'static str,
    data_class: DataClass,
) -> Result<CapabilityDefinition, SdkError> {
    Ok(CapabilityDefinition {
        capability_id: configured(CapabilityId::try_new(capability_id))?,
        capability_version: configured(CapabilityVersion::try_new(support::CONTRACT_VERSION))?,
        owner_module_id: configured(ModuleId::try_new(MODULE_ID))?,
        input_contract: support::protobuf_contract(MODULE_ID, input_schema, vec![data_class])?,
        output_contract: Some(support::protobuf_contract(
            MODULE_ID,
            output_schema,
            vec![data_class],
        )?),
        risk: CapabilityRisk::Medium,
        mutation: true,
        requires_idempotency: true,
        requires_approval: false,
        authorization_policy_id: capability_id.to_owned(),
        rate_limit_policy_id: None,
    })
}

fn ensure_immutable_version(snapshot: &RecordSnapshot, label: &str) -> Result<(), SdkError> {
    if snapshot.version != 1 {
        return Err(SdkError::new(
            "DATA_QUALITY_PERSISTED_STATE_INVALID",
            ErrorCategory::Internal,
            false,
            "The persisted Data Quality state is invalid.",
        )
        .with_internal_reference(format!(
            "immutable {label} record has unexpected aggregate version {}",
            snapshot.version
        )));
    }
    Ok(())
}

fn configured<T>(value: Result<T, crm_module_sdk::IdentifierError>) -> Result<T, SdkError> {
    value.map_err(|error| configuration_error().with_internal_reference(error.to_string()))
}

fn configuration_error() -> SdkError {
    SdkError::new(
        "DATA_QUALITY_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Data Quality capability configuration is invalid.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutation_catalog_has_exact_coordinates_and_data_classification() {
        let definitions = capability_definitions().unwrap();
        assert_eq!(definitions.len(), MUTATION_CAPABILITY_IDS.len());
        for (definition, expected_capability) in definitions.iter().zip(MUTATION_CAPABILITY_IDS) {
            assert_eq!(definition.capability_id.as_str(), *expected_capability);
            assert_eq!(definition.owner_module_id.as_str(), MODULE_ID);
            assert_eq!(definition.capability_version.as_str(), support::CONTRACT_VERSION);
            let expected_class = if [
                PUBLISH_PARTY_RULE_SET_CAPABILITY,
                PUBLISH_PARTY_COMPLETENESS_PROFILE_CAPABILITY,
            ]
            .contains(expected_capability)
            {
                DataClass::Confidential
            } else {
                DataClass::Personal
            };
            assert_eq!(definition.input_contract.allowed_data_classes, vec![expected_class]);
            let expected_risk = if *expected_capability == REMEDIATE_PARTY_DISPLAY_NAME_CAPABILITY {
                CapabilityRisk::High
            } else {
                CapabilityRisk::Medium
            };
            assert_eq!(definition.risk, expected_risk);
            assert!(definition.mutation);
            assert!(definition.requires_idempotency);
            assert!(!definition.requires_approval);
        }
    }

    #[test]
    fn internal_evaluation_definitions_are_personal_and_not_publicly_catalogued() {
        for definition in [
            evaluation_stage_capability_definition().unwrap(),
            evaluation_materialization_capability_definition().unwrap(),
        ] {
            assert_eq!(definition.input_contract.allowed_data_classes, vec![DataClass::Personal]);
            assert!(!MUTATION_CAPABILITY_IDS.contains(&definition.capability_id.as_str()));
        }
    }
}
