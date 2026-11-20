use crm_capability_plan_support::{self as support, PersistedPayloadContract};
use crm_customer_privacy::{
    MODULE_ID, PrivacyRetentionDecisionSet, RETENTION_DECISION_RECORD_TYPE,
    RETENTION_DECISION_STATE_MAXIMUM_BYTES, RETENTION_DECISION_STATE_RETENTION_POLICY_ID,
    RETENTION_DECISION_STATE_SCHEMA_ID, RETENTION_DECISION_STATE_SCHEMA_VERSION,
    decode_retention_decision_state, encode_retention_decision_state,
    retention_decision_state_descriptor_hash,
};
use crm_module_sdk::{DataClass, RecordRef, RecordSnapshot, SdkError, TypedPayload};

pub fn retention_decision_record_ref(
    decision: &PrivacyRetentionDecisionSet,
) -> Result<RecordRef, SdkError> {
    support::record_ref(
        RETENTION_DECISION_RECORD_TYPE,
        decision.decision_id().as_str(),
        "customer_privacy.retention_decision_ref.retention_decision_id",
    )
}

pub fn retention_decision_persisted_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: RETENTION_DECISION_STATE_SCHEMA_ID,
        schema_version: RETENTION_DECISION_STATE_SCHEMA_VERSION,
        descriptor_hash: retention_decision_state_descriptor_hash(),
        maximum_size_bytes: RETENTION_DECISION_STATE_MAXIMUM_BYTES,
        retention_policy_id: RETENTION_DECISION_STATE_RETENTION_POLICY_ID,
    }
}

pub fn retention_decision_persisted_payload(
    decision: &PrivacyRetentionDecisionSet,
) -> Result<TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        retention_decision_persisted_contract(),
        DataClass::Personal,
        encode_retention_decision_state(decision)?,
    )
}

pub fn retention_decision_from_snapshot(
    snapshot: &RecordSnapshot,
) -> Result<PrivacyRetentionDecisionSet, SdkError> {
    if snapshot.reference.record_type.as_str() != RETENTION_DECISION_RECORD_TYPE {
        return Err(retention_adapter_error(
            "retention-decision record type differs from its governed contract",
        ));
    }
    let bytes = support::persisted_json_bytes_with_data_class(
        snapshot,
        retention_decision_persisted_contract(),
        DataClass::Personal,
    )?;
    let decision = decode_retention_decision_state(bytes)?;
    if snapshot.reference.record_id.as_str() != decision.decision_id().as_str()
        || snapshot.version != 1
    {
        return Err(retention_adapter_error(
            "retention-decision identity or version differs from its record envelope",
        ));
    }
    Ok(decision)
}

fn retention_adapter_error(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_RETENTION_PERSISTENCE_INVALID",
        crm_module_sdk::ErrorCategory::Internal,
        false,
        "The Customer Privacy retention decision could not be loaded safely.",
    )
    .with_internal_reference(reference)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retention_record_contract_is_personal_and_immutable_v1() {
        let contract = retention_decision_persisted_contract();
        assert_eq!(
            RETENTION_DECISION_RECORD_TYPE,
            "customer-privacy.retention-decision"
        );
        assert_eq!(contract.owner, MODULE_ID);
        assert_eq!(contract.schema_id, RETENTION_DECISION_STATE_SCHEMA_ID);
        assert_eq!(
            contract.schema_version,
            RETENTION_DECISION_STATE_SCHEMA_VERSION
        );
        assert_eq!(
            contract.retention_policy_id,
            RETENTION_DECISION_STATE_RETENTION_POLICY_ID
        );
        assert_eq!(contract.maximum_size_bytes, 2 * 1024 * 1024);
    }
}
