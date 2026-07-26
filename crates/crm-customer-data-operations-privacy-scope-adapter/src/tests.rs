use super::*;
use crm_capability_runtime::CapabilityRisk;
use crm_module_sdk::{DataClass, PayloadEncoding};

#[test]
fn publishes_exact_contract_only_customer_data_coordinate() {
    let definition = customer_data_privacy_scope_definition().unwrap();
    assert_eq!(definition.capability_id.as_str(), CAPABILITY_ID);
    assert_eq!(definition.capability_version.as_str(), CAPABILITY_VERSION);
    assert_eq!(
        definition.owner_module_id.as_str(),
        crm_customer_data_operations::MODULE_ID
    );
    assert_eq!(definition.risk, CapabilityRisk::Medium);
    assert!(!definition.mutation);
    assert!(!definition.requires_idempotency);
    assert!(!definition.requires_approval);
    assert_eq!(definition.authorization_policy_id, CAPABILITY_ID);
    assert_eq!(
        definition.input_contract.allowed_data_classes,
        vec![DataClass::Confidential]
    );
    assert_eq!(
        definition.input_contract.allowed_encodings,
        vec![PayloadEncoding::Protobuf]
    );
    let output = definition.output_contract.unwrap();
    assert_eq!(output.schema_id.as_str(), OUTPUT_SCHEMA_ID);
    assert_eq!(output.maximum_size_bytes, OUTPUT_MAXIMUM_BYTES);
}

#[test]
fn freezes_owner_scan_and_rehydration_bounds() {
    assert_eq!(DEFAULT_PAGE_SIZE, 64);
    assert_eq!(MAXIMUM_PAGE_SIZE, 128);
    assert_eq!(MAXIMUM_CURSOR_BYTES, 2_048);
    assert_eq!(MAX_PRIVACY_IMPORT_ROWS_SCANNED, 16_384);
    assert_eq!(MAX_PRIVACY_EXPORT_SELECTION_ITEMS_SCANNED, 16_384);
    assert_eq!(MAX_PRIVACY_ASSOCIATED_EXPORT_RECORDS_REHYDRATED, 32_768);
    assert_eq!(MAX_PRIVACY_CANONICAL_PARTY_RESOLUTIONS, 32_768);
    assert_eq!(MAX_PRIVACY_OWNER_RECORDS_SCANNED, 65_536);
}

#[test]
fn definition_validation_rejects_any_mutated_contract() {
    let mut definition = customer_data_privacy_scope_definition().unwrap();
    definition.mutation = true;
    let error = contract::validate_definition(&definition).unwrap_err();
    assert_eq!(
        error.code,
        "CUSTOMER_DATA_PRIVACY_SCOPE_DEFINITION_MISMATCH"
    );
}

#[test]
fn owner_specific_errors_are_stable_and_safe() {
    assert_eq!(
        errors::import_row_state_invalid("private import row").code,
        "CUSTOMER_DATA_PRIVACY_SCOPE_IMPORT_ROW_STATE_INVALID"
    );
    assert_eq!(
        errors::export_selection_state_invalid("private selection").code,
        "CUSTOMER_DATA_PRIVACY_SCOPE_EXPORT_SELECTION_STATE_INVALID"
    );
    assert_eq!(
        errors::export_stage_state_invalid("private stage").code,
        "CUSTOMER_DATA_PRIVACY_SCOPE_EXPORT_STAGE_STATE_INVALID"
    );
    assert_eq!(
        errors::export_outcome_state_invalid("private outcome").code,
        "CUSTOMER_DATA_PRIVACY_SCOPE_EXPORT_OUTCOME_STATE_INVALID"
    );
    assert_eq!(
        errors::scan_limit_exceeded("private row count").safe_message,
        "The Customer Data Operations privacy scope is temporarily unavailable."
    );
}
