use crm_customer_privacy_capability_adapter::{
    PLACE_CUSTOMER_DATA_LEGAL_HOLD_CAPABILITY, deterministic_customer_data_legal_hold_id,
    place_customer_data_legal_hold_capability_definition,
};
use crm_module_sdk::DataClass;

#[test]
fn legal_hold_place_definition_is_exact_and_fail_closed() {
    let definition = place_customer_data_legal_hold_capability_definition()
        .expect("legal-hold placement definition must be configured");

    assert_eq!(
        definition.capability_id.as_str(),
        PLACE_CUSTOMER_DATA_LEGAL_HOLD_CAPABILITY
    );
    assert_eq!(definition.capability_version.as_str(), "1.0.0");
    assert_eq!(definition.owner_module_id.as_str(), "crm.customer-privacy");
    assert_eq!(
        definition.input_contract.schema_id.as_str(),
        "crm.customer_privacy.v1.PlaceCustomerDataLegalHoldRequest"
    );
    assert_eq!(
        definition
            .output_contract
            .as_ref()
            .expect("legal-hold placement output contract")
            .schema_id
            .as_str(),
        "crm.customer_privacy.v1.PlaceCustomerDataLegalHoldResponse"
    );
    assert_eq!(
        definition.input_contract.allowed_data_classes,
        vec![DataClass::Personal]
    );
    assert_eq!(
        definition
            .output_contract
            .as_ref()
            .expect("legal-hold placement output contract")
            .allowed_data_classes,
        vec![DataClass::Personal]
    );
    assert_eq!(format!("{:?}", definition.risk), "High");
    assert!(definition.mutation);
    assert!(definition.requires_idempotency);
    assert!(!definition.requires_approval);
    assert_eq!(
        definition.authorization_policy_id,
        PLACE_CUSTOMER_DATA_LEGAL_HOLD_CAPABILITY
    );
}

#[test]
fn legal_hold_identity_is_stable_tenant_and_idempotency_bound() {
    let first = deterministic_customer_data_legal_hold_id("tenant-a", "same-key")
        .expect("first legal-hold identity");
    let replay = deterministic_customer_data_legal_hold_id("tenant-a", "same-key")
        .expect("replayed legal-hold identity");
    let other_tenant = deterministic_customer_data_legal_hold_id("tenant-b", "same-key")
        .expect("other tenant legal-hold identity");
    let other_key = deterministic_customer_data_legal_hold_id("tenant-a", "other-key")
        .expect("other idempotency key legal-hold identity");

    assert_eq!(first, replay);
    assert_ne!(first, other_tenant);
    assert_ne!(first, other_key);
    assert!(first.as_str().starts_with("customer-data-legal-hold-"));
}
