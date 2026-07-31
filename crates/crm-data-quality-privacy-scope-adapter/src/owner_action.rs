use crm_capability_plan_support::PersistedPayloadContract;
use crm_capability_runtime::CapabilityDefinition;
use crm_customer_privacy_owner_scope_support::{
    OwnerPrivacyActionPlanner, OwnerPrivacyActionPolicy, PrivacyOwnerActionCommand,
    owner_action_definition, unsupported_owner_action,
};
use crm_data_quality::{
    FINDING_OBSERVATION_RECORD_TYPE, FINDING_RECORD_TYPE, PARTY_COMPLETENESS_RESULT_RECORD_TYPE,
    PARTY_EVALUATION_INPUT_RECORD_TYPE, PARTY_EVALUATION_JOB_RECORD_TYPE,
    REMEDIATION_ATTEMPT_RECORD_TYPE, RULE_OUTCOME_RECORD_TYPE, decode_finding_observation_state,
    decode_finding_state, decode_party_completeness_result_state,
    decode_party_evaluation_input_state, decode_party_evaluation_job_state,
    decode_remediation_attempt_state, decode_rule_outcome_state,
};
use crm_data_quality_capability_adapter::{
    MODULE_ID, party_completeness_result_persisted_contract,
    party_evaluation_input_persisted_contract, party_evaluation_job_persisted_contract,
    party_finding_observation_persisted_contract, party_finding_persisted_contract,
    party_rule_outcome_persisted_contract, remediation_attempt_persisted_contract,
};
use crm_module_sdk::{
    DataClass, ErrorCategory, PayloadEncoding, RecordSnapshot, SdkError, TypedPayload,
};

pub const OWNER_ACTION_CAPABILITY_ID: &str = "data_quality.privacy.action.apply";

pub type DataQualityPrivacyActionPlanner =
    OwnerPrivacyActionPlanner<DataQualityPrivacyActionPolicy>;

#[derive(Debug, Default, Clone, Copy)]
pub struct DataQualityPrivacyActionPolicy;

pub fn data_quality_privacy_action_definition() -> Result<CapabilityDefinition, SdkError> {
    owner_action_definition(MODULE_ID, OWNER_ACTION_CAPABILITY_ID)
}

pub const fn data_quality_privacy_action_planner() -> DataQualityPrivacyActionPlanner {
    OwnerPrivacyActionPlanner::new(DataQualityPrivacyActionPolicy)
}

impl OwnerPrivacyActionPolicy for DataQualityPrivacyActionPolicy {
    fn owner_module_id(&self) -> &'static str {
        MODULE_ID
    }

    fn capability_id(&self) -> &'static str {
        OWNER_ACTION_CAPABILITY_ID
    }

    fn supports_resource_type(&self, resource_type: &str) -> bool {
        matches!(
            resource_type,
            PARTY_EVALUATION_JOB_RECORD_TYPE
                | PARTY_EVALUATION_INPUT_RECORD_TYPE
                | RULE_OUTCOME_RECORD_TYPE
                | FINDING_RECORD_TYPE
                | FINDING_OBSERVATION_RECORD_TYPE
                | PARTY_COMPLETENESS_RESULT_RECORD_TYPE
                | REMEDIATION_ATTEMPT_RECORD_TYPE
        )
    }

    fn anonymize(
        &self,
        command: &PrivacyOwnerActionCommand,
        current: &RecordSnapshot,
    ) -> Result<TypedPayload, SdkError> {
        validate_resource(command, current)?;
        Err(unsupported(command))
    }

    fn deletion_tombstone(
        &self,
        command: &PrivacyOwnerActionCommand,
        current: &RecordSnapshot,
    ) -> Result<TypedPayload, SdkError> {
        validate_resource(command, current)?;
        Err(unsupported(command))
    }
}

fn validate_resource(
    command: &PrivacyOwnerActionCommand,
    current: &RecordSnapshot,
) -> Result<(), SdkError> {
    match command.resource_type() {
        PARTY_EVALUATION_JOB_RECORD_TYPE => {
            validate_payload(current, party_evaluation_job_persisted_contract())?;
            let value = decode_party_evaluation_job_state(&current.payload.bytes)?;
            validate_identity(
                current,
                PARTY_EVALUATION_JOB_RECORD_TYPE,
                value.job_id().as_str(),
            )
        }
        PARTY_EVALUATION_INPUT_RECORD_TYPE => {
            validate_payload(current, party_evaluation_input_persisted_contract())?;
            let value = decode_party_evaluation_input_state(&current.payload.bytes)?;
            validate_identity(
                current,
                PARTY_EVALUATION_INPUT_RECORD_TYPE,
                value.job_id().as_str(),
            )
        }
        RULE_OUTCOME_RECORD_TYPE => {
            validate_payload(current, party_rule_outcome_persisted_contract())?;
            let value = decode_rule_outcome_state(&current.payload.bytes)?;
            validate_identity(current, RULE_OUTCOME_RECORD_TYPE, value.outcome_id())
        }
        FINDING_RECORD_TYPE => {
            validate_payload(current, party_finding_persisted_contract())?;
            let value = decode_finding_state(&current.payload.bytes)?;
            if value.tenant_id() != command.tenant_id() {
                return Err(stored_state_invalid(
                    "finding tenant differs from the command",
                ));
            }
            validate_identity(current, FINDING_RECORD_TYPE, value.finding_id())
        }
        FINDING_OBSERVATION_RECORD_TYPE => {
            validate_payload(current, party_finding_observation_persisted_contract())?;
            let value = decode_finding_observation_state(&current.payload.bytes)?;
            if value.tenant_id() != command.tenant_id() {
                return Err(stored_state_invalid(
                    "finding observation tenant differs from the command",
                ));
            }
            validate_identity(
                current,
                FINDING_OBSERVATION_RECORD_TYPE,
                value.observation_id(),
            )
        }
        PARTY_COMPLETENESS_RESULT_RECORD_TYPE => {
            validate_payload(current, party_completeness_result_persisted_contract())?;
            let value = decode_party_completeness_result_state(&current.payload.bytes)?;
            validate_identity(
                current,
                PARTY_COMPLETENESS_RESULT_RECORD_TYPE,
                value.result_id(),
            )
        }
        REMEDIATION_ATTEMPT_RECORD_TYPE => {
            validate_payload(current, remediation_attempt_persisted_contract())?;
            let value = decode_remediation_attempt_state(&current.payload.bytes)?;
            if value.tenant_id() != command.tenant_id() {
                return Err(stored_state_invalid(
                    "remediation attempt tenant differs from the command",
                ));
            }
            validate_identity(current, REMEDIATION_ATTEMPT_RECORD_TYPE, value.attempt_id())
        }
        _ => Err(stored_state_invalid(
            "unsupported Data Quality resource type reached the owner policy",
        )),
    }
}

fn validate_payload(
    current: &RecordSnapshot,
    contract: PersistedPayloadContract<'_>,
) -> Result<(), SdkError> {
    let payload = &current.payload;
    if payload.owner.as_str() != contract.owner
        || payload.schema_id.as_str() != contract.schema_id
        || payload.schema_version.as_str() != contract.schema_version
        || payload.descriptor_hash != contract.descriptor_hash
        || payload.data_class != DataClass::Personal
        || payload.encoding != PayloadEncoding::Json
        || payload.maximum_size_bytes != contract.maximum_size_bytes
        || payload.retention_policy_id.as_str() != contract.retention_policy_id
        || payload.validate().is_err()
    {
        return Err(stored_state_invalid(
            "typed payload does not match the authoritative persisted contract",
        ));
    }
    Ok(())
}

fn validate_identity(
    current: &RecordSnapshot,
    resource_type: &str,
    resource_id: &str,
) -> Result<(), SdkError> {
    if current.reference.record_type.as_str() != resource_type
        || current.reference.record_id.as_str() != resource_id
    {
        return Err(stored_state_invalid(
            "decoded identity does not match the locked owner record",
        ));
    }
    Ok(())
}

fn unsupported(command: &PrivacyOwnerActionCommand) -> SdkError {
    unsupported_owner_action(MODULE_ID, command.resource_type(), command.action_code())
}

fn stored_state_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "DATA_QUALITY_PRIVACY_STORED_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "Stored Data Quality state is invalid.",
    )
    .with_internal_reference(reference)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publishes_the_frozen_owner_action_coordinate() {
        let definition = data_quality_privacy_action_definition().unwrap();
        assert_eq!(definition.owner_module_id.as_str(), MODULE_ID);
        assert_eq!(
            definition.capability_id.as_str(),
            OWNER_ACTION_CAPABILITY_ID
        );
        assert!(definition.mutation);
        assert!(definition.requires_idempotency);
    }

    #[test]
    fn supports_exactly_the_seven_discovered_resource_families() {
        let policy = DataQualityPrivacyActionPolicy;
        for resource_type in [
            PARTY_EVALUATION_JOB_RECORD_TYPE,
            PARTY_EVALUATION_INPUT_RECORD_TYPE,
            RULE_OUTCOME_RECORD_TYPE,
            FINDING_RECORD_TYPE,
            FINDING_OBSERVATION_RECORD_TYPE,
            PARTY_COMPLETENESS_RESULT_RECORD_TYPE,
            REMEDIATION_ATTEMPT_RECORD_TYPE,
        ] {
            assert!(policy.supports_resource_type(resource_type));
        }
        assert!(!policy.supports_resource_type("data_quality.rule_set_version"));
    }
}
