use crm_capability_plan_support::{self as support, PersistedPayloadContract};
use crm_capability_runtime::CapabilityDefinition;
use crm_customer_enrichment::{
    APPLICATION_ATTEMPT_RECORD_TYPE, ENRICHMENT_REQUEST_RECORD_TYPE,
    LIFECYCLE_STATE_RETENTION_POLICY_ID, LIFECYCLE_STATE_SCHEMA_VERSION,
    PROVIDER_RESPONSE_CONFLICT_RECORD_TYPE, PROVIDER_RESPONSE_RECEIPT_RECORD_TYPE,
    PROVIDER_RESPONSE_RECEIPT_STATE_MAXIMUM_BYTES, PROVIDER_RESPONSE_RECEIPT_STATE_SCHEMA_ID,
    PROVIDER_USAGE_ENTRY_RECORD_TYPE, PROVIDER_USAGE_ENTRY_STATE_MAXIMUM_BYTES,
    PROVIDER_USAGE_ENTRY_STATE_RETENTION_POLICY_ID, PROVIDER_USAGE_ENTRY_STATE_SCHEMA_ID,
    PROVIDER_USAGE_ENTRY_STATE_SCHEMA_VERSION, REVIEW_DECISION_RECORD_TYPE, SUGGESTION_RECORD_TYPE,
    decode_provider_response_conflict_state, decode_provider_response_receipt_state,
    decode_provider_usage_entry_state, provider_response_receipt_state_descriptor_hash,
    provider_usage_entry_state_descriptor_hash,
};
use crm_customer_enrichment_application_adapter::application_attempt_from_snapshot;
use crm_customer_enrichment_capability_adapter::{
    MODULE_ID, enrichment_request_from_snapshot,
};
use crm_customer_enrichment_provider_process_composition::provider_response_conflict_persisted_contract;
use crm_customer_enrichment_review_adapter::{
    review_decision_from_snapshot, suggestion_from_snapshot,
};
use crm_customer_privacy_owner_scope_support::{
    OwnerPrivacyActionPlanner, OwnerPrivacyActionPolicy, PrivacyOwnerActionCommand,
    owner_action_definition, unsupported_owner_action,
};
use crm_module_sdk::{DataClass, ErrorCategory, RecordSnapshot, SdkError, TypedPayload};

pub const OWNER_ACTION_CAPABILITY_ID: &str = "customer_enrichment.privacy.action.apply";

pub type CustomerEnrichmentPrivacyActionPlanner =
    OwnerPrivacyActionPlanner<CustomerEnrichmentPrivacyActionPolicy>;

#[derive(Debug, Default, Clone, Copy)]
pub struct CustomerEnrichmentPrivacyActionPolicy;

pub fn customer_enrichment_privacy_action_definition() -> Result<CapabilityDefinition, SdkError> {
    owner_action_definition(MODULE_ID, OWNER_ACTION_CAPABILITY_ID)
}

pub const fn customer_enrichment_privacy_action_planner() -> CustomerEnrichmentPrivacyActionPlanner
{
    OwnerPrivacyActionPlanner::new(CustomerEnrichmentPrivacyActionPolicy)
}

impl OwnerPrivacyActionPolicy for CustomerEnrichmentPrivacyActionPolicy {
    fn owner_module_id(&self) -> &'static str {
        MODULE_ID
    }

    fn capability_id(&self) -> &'static str {
        OWNER_ACTION_CAPABILITY_ID
    }

    fn supports_resource_type(&self, resource_type: &str) -> bool {
        matches!(
            resource_type,
            ENRICHMENT_REQUEST_RECORD_TYPE
                | PROVIDER_RESPONSE_RECEIPT_RECORD_TYPE
                | PROVIDER_RESPONSE_CONFLICT_RECORD_TYPE
                | SUGGESTION_RECORD_TYPE
                | REVIEW_DECISION_RECORD_TYPE
                | APPLICATION_ATTEMPT_RECORD_TYPE
                | PROVIDER_USAGE_ENTRY_RECORD_TYPE
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
        ENRICHMENT_REQUEST_RECORD_TYPE => {
            let request = enrichment_request_from_snapshot(current)?;
            if request.tenant_id() != command.tenant_id() {
                return Err(stored_state_invalid(
                    "enrichment request tenant differs from the owner action command",
                ));
            }
            Ok(())
        }
        PROVIDER_RESPONSE_RECEIPT_RECORD_TYPE => validate_response_receipt(current),
        PROVIDER_RESPONSE_CONFLICT_RECORD_TYPE => validate_response_conflict(command, current),
        SUGGESTION_RECORD_TYPE => suggestion_from_snapshot(current).map(|_| ()),
        REVIEW_DECISION_RECORD_TYPE => review_decision_from_snapshot(current).map(|_| ()),
        APPLICATION_ATTEMPT_RECORD_TYPE => {
            let attempt = application_attempt_from_snapshot(current)?;
            if attempt.tenant_id() != command.tenant_id() {
                return Err(stored_state_invalid(
                    "application attempt tenant differs from the owner action command",
                ));
            }
            Ok(())
        }
        PROVIDER_USAGE_ENTRY_RECORD_TYPE => validate_provider_usage(current),
        _ => Err(stored_state_invalid(
            "unsupported Customer Enrichment resource type reached the owner policy",
        )),
    }
}

fn validate_response_receipt(current: &RecordSnapshot) -> Result<(), SdkError> {
    if current.reference.record_type.as_str() != PROVIDER_RESPONSE_RECEIPT_RECORD_TYPE
        || current.version != 1
    {
        return Err(stored_state_invalid(
            "provider response receipt type or immutable version is invalid",
        ));
    }
    let bytes = support::persisted_json_bytes_with_data_class(
        current,
        response_receipt_contract(),
        DataClass::Personal,
    )?;
    let receipt = decode_provider_response_receipt_state(bytes)?;
    if current.reference.record_id.as_str() != receipt.receipt_id().as_str() {
        return Err(stored_state_invalid(
            "provider response receipt identity differs from canonical state",
        ));
    }
    Ok(())
}

fn validate_response_conflict(
    command: &PrivacyOwnerActionCommand,
    current: &RecordSnapshot,
) -> Result<(), SdkError> {
    if current.reference.record_type.as_str() != PROVIDER_RESPONSE_CONFLICT_RECORD_TYPE
        || !(1..=2).contains(&current.version)
    {
        return Err(stored_state_invalid(
            "provider response conflict type or lifecycle version is invalid",
        ));
    }
    let bytes = support::persisted_json_bytes_with_data_class(
        current,
        provider_response_conflict_persisted_contract(),
        DataClass::Confidential,
    )?;
    let conflict = decode_provider_response_conflict_state(bytes)?;
    if current.reference.record_id.as_str() != conflict.conflict_id().as_str()
        || conflict.tenant_id() != command.tenant_id()
        || (current.version == 1) != conflict.resolution().is_none()
    {
        return Err(stored_state_invalid(
            "provider response conflict identity, tenant or lifecycle is inconsistent",
        ));
    }
    Ok(())
}

fn validate_provider_usage(current: &RecordSnapshot) -> Result<(), SdkError> {
    if current.reference.record_type.as_str() != PROVIDER_USAGE_ENTRY_RECORD_TYPE
        || current.version != 1
    {
        return Err(stored_state_invalid(
            "provider usage entry type or immutable version is invalid",
        ));
    }
    let bytes = support::persisted_json_bytes_with_data_class(
        current,
        provider_usage_contract(),
        DataClass::Personal,
    )?;
    let usage = decode_provider_usage_entry_state(bytes)?;
    if current.reference.record_id.as_str() != usage.usage_entry_id().as_str() {
        return Err(stored_state_invalid(
            "provider usage identity differs from canonical state",
        ));
    }
    Ok(())
}

fn response_receipt_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: PROVIDER_RESPONSE_RECEIPT_STATE_SCHEMA_ID,
        schema_version: LIFECYCLE_STATE_SCHEMA_VERSION,
        descriptor_hash: provider_response_receipt_state_descriptor_hash(),
        maximum_size_bytes: PROVIDER_RESPONSE_RECEIPT_STATE_MAXIMUM_BYTES,
        retention_policy_id: LIFECYCLE_STATE_RETENTION_POLICY_ID,
    }
}

fn provider_usage_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: PROVIDER_USAGE_ENTRY_STATE_SCHEMA_ID,
        schema_version: PROVIDER_USAGE_ENTRY_STATE_SCHEMA_VERSION,
        descriptor_hash: provider_usage_entry_state_descriptor_hash(),
        maximum_size_bytes: PROVIDER_USAGE_ENTRY_STATE_MAXIMUM_BYTES,
        retention_policy_id: PROVIDER_USAGE_ENTRY_STATE_RETENTION_POLICY_ID,
    }
}

fn unsupported(command: &PrivacyOwnerActionCommand) -> SdkError {
    unsupported_owner_action(MODULE_ID, command.resource_type(), command.action_code())
}

fn stored_state_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PRIVACY_STORED_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "Stored Customer Enrichment state is invalid.",
    )
    .with_internal_reference(reference)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publishes_the_frozen_owner_action_coordinate() {
        let definition = customer_enrichment_privacy_action_definition().unwrap();
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
        let policy = CustomerEnrichmentPrivacyActionPolicy;
        for resource_type in [
            ENRICHMENT_REQUEST_RECORD_TYPE,
            PROVIDER_RESPONSE_RECEIPT_RECORD_TYPE,
            PROVIDER_RESPONSE_CONFLICT_RECORD_TYPE,
            SUGGESTION_RECORD_TYPE,
            REVIEW_DECISION_RECORD_TYPE,
            APPLICATION_ATTEMPT_RECORD_TYPE,
            PROVIDER_USAGE_ENTRY_RECORD_TYPE,
        ] {
            assert!(policy.supports_resource_type(resource_type));
        }
        assert!(!policy.supports_resource_type(
            "customer_enrichment.provider_profile_version"
        ));
    }
}
