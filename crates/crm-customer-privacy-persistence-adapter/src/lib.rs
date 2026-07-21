#![forbid(unsafe_code)]

//! Governed persisted-payload and snapshot boundary for Customer Privacy.

use crm_capability_plan_support::{self as support, PersistedPayloadContract};
use crm_customer_privacy::{
    CustomerDataLegalHold, LEGAL_HOLD_RECORD_TYPE, LEGAL_HOLD_STATE_MAXIMUM_BYTES,
    LEGAL_HOLD_STATE_RETENTION_POLICY_ID, LEGAL_HOLD_STATE_SCHEMA_ID,
    LEGAL_HOLD_STATE_SCHEMA_VERSION, MODULE_ID, PRIVACY_CASE_RECORD_TYPE,
    PRIVACY_CASE_STATE_MAXIMUM_BYTES, PRIVACY_CASE_STATE_RETENTION_POLICY_ID,
    PRIVACY_CASE_STATE_SCHEMA_ID, PRIVACY_CASE_STATE_SCHEMA_VERSION, PrivacyCase,
    PROCESSING_RESTRICTION_STATE_MAXIMUM_BYTES,
    PROCESSING_RESTRICTION_STATE_RETENTION_POLICY_ID, PROCESSING_RESTRICTION_STATE_SCHEMA_ID,
    PROCESSING_RESTRICTION_STATE_SCHEMA_VERSION, ProcessingRestriction, RESTRICTION_RECORD_TYPE,
    decode_legal_hold_state, decode_privacy_case_state, decode_processing_restriction_state,
    encode_legal_hold_state, encode_privacy_case_state, encode_processing_restriction_state,
    legal_hold_state_descriptor_hash, privacy_case_state_descriptor_hash,
    processing_restriction_state_descriptor_hash,
};
use crm_module_sdk::{DataClass, ErrorCategory, RecordRef, RecordSnapshot, SdkError, TypedPayload};

pub fn privacy_case_record_ref(case: &PrivacyCase) -> Result<RecordRef, SdkError> {
    support::record_ref(
        PRIVACY_CASE_RECORD_TYPE,
        case.case_id().as_str(),
        "customer_privacy.privacy_case_ref.privacy_case_id",
    )
}

pub fn processing_restriction_record_ref(
    restriction: &ProcessingRestriction,
) -> Result<RecordRef, SdkError> {
    support::record_ref(
        RESTRICTION_RECORD_TYPE,
        restriction.restriction_id().as_str(),
        "customer_privacy.processing_restriction_ref.processing_restriction_id",
    )
}

pub fn legal_hold_record_ref(hold: &CustomerDataLegalHold) -> Result<RecordRef, SdkError> {
    support::record_ref(
        LEGAL_HOLD_RECORD_TYPE,
        hold.hold_id().as_str(),
        "customer_privacy.customer_data_legal_hold_ref.customer_data_legal_hold_id",
    )
}

pub fn privacy_case_persisted_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: PRIVACY_CASE_STATE_SCHEMA_ID,
        schema_version: PRIVACY_CASE_STATE_SCHEMA_VERSION,
        descriptor_hash: privacy_case_state_descriptor_hash(),
        maximum_size_bytes: PRIVACY_CASE_STATE_MAXIMUM_BYTES,
        retention_policy_id: PRIVACY_CASE_STATE_RETENTION_POLICY_ID,
    }
}

pub fn processing_restriction_persisted_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: PROCESSING_RESTRICTION_STATE_SCHEMA_ID,
        schema_version: PROCESSING_RESTRICTION_STATE_SCHEMA_VERSION,
        descriptor_hash: processing_restriction_state_descriptor_hash(),
        maximum_size_bytes: PROCESSING_RESTRICTION_STATE_MAXIMUM_BYTES,
        retention_policy_id: PROCESSING_RESTRICTION_STATE_RETENTION_POLICY_ID,
    }
}

pub fn legal_hold_persisted_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: LEGAL_HOLD_STATE_SCHEMA_ID,
        schema_version: LEGAL_HOLD_STATE_SCHEMA_VERSION,
        descriptor_hash: legal_hold_state_descriptor_hash(),
        maximum_size_bytes: LEGAL_HOLD_STATE_MAXIMUM_BYTES,
        retention_policy_id: LEGAL_HOLD_STATE_RETENTION_POLICY_ID,
    }
}

pub fn privacy_case_persisted_payload(case: &PrivacyCase) -> Result<TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        privacy_case_persisted_contract(),
        DataClass::Personal,
        encode_privacy_case_state(case)?,
    )
}

pub fn processing_restriction_persisted_payload(
    restriction: &ProcessingRestriction,
) -> Result<TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        processing_restriction_persisted_contract(),
        DataClass::Personal,
        encode_processing_restriction_state(restriction)?,
    )
}

pub fn legal_hold_persisted_payload(
    hold: &CustomerDataLegalHold,
) -> Result<TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        legal_hold_persisted_contract(),
        DataClass::Personal,
        encode_legal_hold_state(hold)?,
    )
}

pub fn privacy_case_from_snapshot(snapshot: &RecordSnapshot) -> Result<PrivacyCase, SdkError> {
    ensure_record_type(snapshot, PRIVACY_CASE_RECORD_TYPE, "privacy case")?;
    let bytes = support::persisted_json_bytes_with_data_class(
        snapshot,
        privacy_case_persisted_contract(),
        DataClass::Personal,
    )?;
    let case = decode_privacy_case_state(bytes)?;
    ensure_snapshot_identity_and_version(
        snapshot,
        case.case_id().as_str(),
        case.version(),
        "privacy case",
    )?;
    Ok(case)
}

pub fn processing_restriction_from_snapshot(
    snapshot: &RecordSnapshot,
) -> Result<ProcessingRestriction, SdkError> {
    ensure_record_type(
        snapshot,
        RESTRICTION_RECORD_TYPE,
        "processing restriction",
    )?;
    let bytes = support::persisted_json_bytes_with_data_class(
        snapshot,
        processing_restriction_persisted_contract(),
        DataClass::Personal,
    )?;
    let restriction = decode_processing_restriction_state(bytes)?;
    ensure_snapshot_identity_and_version(
        snapshot,
        restriction.restriction_id().as_str(),
        restriction.version(),
        "processing restriction",
    )?;
    Ok(restriction)
}

pub fn legal_hold_from_snapshot(
    snapshot: &RecordSnapshot,
) -> Result<CustomerDataLegalHold, SdkError> {
    ensure_record_type(snapshot, LEGAL_HOLD_RECORD_TYPE, "customer-data legal hold")?;
    let bytes = support::persisted_json_bytes_with_data_class(
        snapshot,
        legal_hold_persisted_contract(),
        DataClass::Personal,
    )?;
    let hold = decode_legal_hold_state(bytes)?;
    ensure_snapshot_identity_and_version(
        snapshot,
        hold.hold_id().as_str(),
        hold.version(),
        "customer-data legal hold",
    )?;
    Ok(hold)
}

fn ensure_record_type(
    snapshot: &RecordSnapshot,
    expected: &str,
    aggregate: &str,
) -> Result<(), SdkError> {
    if snapshot.reference.record_type.as_str() != expected {
        return Err(adapter_error(format!(
            "{aggregate} record type differs from its governed contract"
        )));
    }
    Ok(())
}

fn ensure_snapshot_identity_and_version(
    snapshot: &RecordSnapshot,
    aggregate_id: &str,
    aggregate_version: u64,
    aggregate: &str,
) -> Result<(), SdkError> {
    let expected_version = i64::try_from(aggregate_version).map_err(|_| {
        adapter_error(format!(
            "{aggregate} aggregate version exceeds the record envelope range"
        ))
    })?;
    if snapshot.reference.record_id.as_str() != aggregate_id {
        return Err(adapter_error(format!(
            "{aggregate} identity differs from its record envelope"
        )));
    }
    if snapshot.version != expected_version {
        return Err(adapter_error(format!(
            "{aggregate} version differs from its record envelope"
        )));
    }
    Ok(())
}

fn adapter_error(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_PERSISTENCE_ADAPTER_INVALID",
        ErrorCategory::Internal,
        false,
        "The Customer Privacy state could not be loaded safely.",
    )
    .with_internal_reference(reference)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_customer_privacy::{
        LegalHoldScope, PrivacyCaseKind, RestrictionScope,
    };
    use crm_module_sdk::{ActorId, RecordId, RecordType, SchemaId, SchemaVersion, TenantId};

    fn record_id(value: &str) -> RecordId {
        RecordId::try_new(value).unwrap()
    }

    fn tenant_id() -> TenantId {
        TenantId::try_new("tenant-a").unwrap()
    }

    fn actor_id(value: &str) -> ActorId {
        ActorId::try_new(value).unwrap()
    }

    fn policy_version() -> SchemaVersion {
        SchemaVersion::try_new("privacy-policy/1").unwrap()
    }

    fn privacy_case() -> PrivacyCase {
        PrivacyCase::new(
            record_id("privacy-case-1"),
            tenant_id(),
            PrivacyCaseKind::Erasure,
            policy_version(),
            10,
            None,
        )
        .unwrap()
    }

    fn restriction() -> ProcessingRestriction {
        ProcessingRestriction::place(
            record_id("restriction-1"),
            tenant_id(),
            record_id("party-1"),
            RestrictionScope::ProcessingAndCommunication,
            policy_version(),
            actor_id("privacy-officer"),
            20,
            20,
            None,
        )
        .unwrap()
    }

    fn legal_hold() -> CustomerDataLegalHold {
        CustomerDataLegalHold::place(
            record_id("legal-hold-1"),
            tenant_id(),
            record_id("party-1"),
            LegalHoldScope::AllCustomerData,
            record_id("authority-1"),
            "LITIGATION_HOLD",
            policy_version(),
            actor_id("legal-officer"),
            30,
            None,
        )
        .unwrap()
    }

    #[test]
    fn privacy_case_payload_and_snapshot_round_trip_are_exact() {
        let case = privacy_case();
        let payload = privacy_case_persisted_payload(&case).unwrap();
        assert_eq!(payload.owner.as_str(), MODULE_ID);
        assert_eq!(payload.schema_id.as_str(), PRIVACY_CASE_STATE_SCHEMA_ID);
        assert_eq!(payload.data_class, DataClass::Personal);

        let snapshot = RecordSnapshot {
            reference: privacy_case_record_ref(&case).unwrap(),
            version: 1,
            payload,
        };
        assert_eq!(privacy_case_from_snapshot(&snapshot).unwrap(), case);
    }

    #[test]
    fn restriction_and_legal_hold_snapshots_round_trip() {
        let restriction = restriction();
        let restriction_snapshot = RecordSnapshot {
            reference: processing_restriction_record_ref(&restriction).unwrap(),
            version: 1,
            payload: processing_restriction_persisted_payload(&restriction).unwrap(),
        };
        assert_eq!(
            processing_restriction_from_snapshot(&restriction_snapshot).unwrap(),
            restriction
        );

        let hold = legal_hold();
        let hold_snapshot = RecordSnapshot {
            reference: legal_hold_record_ref(&hold).unwrap(),
            version: 1,
            payload: legal_hold_persisted_payload(&hold).unwrap(),
        };
        assert_eq!(legal_hold_from_snapshot(&hold_snapshot).unwrap(), hold);
    }

    #[test]
    fn rehydration_rejects_record_identity_version_and_contract_drift() {
        let case = privacy_case();
        let payload = privacy_case_persisted_payload(&case).unwrap();

        let wrong_identity = RecordSnapshot {
            reference: RecordRef {
                record_type: RecordType::try_new(PRIVACY_CASE_RECORD_TYPE).unwrap(),
                record_id: record_id("privacy-case-other"),
            },
            version: 1,
            payload: payload.clone(),
        };
        assert!(privacy_case_from_snapshot(&wrong_identity).is_err());

        let wrong_version = RecordSnapshot {
            reference: privacy_case_record_ref(&case).unwrap(),
            version: 2,
            payload: payload.clone(),
        };
        assert!(privacy_case_from_snapshot(&wrong_version).is_err());

        let mut wrong_contract = payload;
        wrong_contract.schema_id = SchemaId::try_new("crm.customer-privacy.case.state.v2").unwrap();
        let wrong_contract = RecordSnapshot {
            reference: privacy_case_record_ref(&case).unwrap(),
            version: 1,
            payload: wrong_contract,
        };
        assert!(privacy_case_from_snapshot(&wrong_contract).is_err());
    }
}
