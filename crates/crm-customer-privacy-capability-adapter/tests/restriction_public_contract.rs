use crm_customer_privacy_capability_adapter::{
    PLACE_PROCESSING_RESTRICTION_CAPABILITY, deterministic_processing_restriction_id,
    place_processing_restriction_capability_definition,
};

#[test]
fn restriction_place_definition_is_exact_and_fail_closed() {
    let definition = place_processing_restriction_capability_definition()
        .expect("restriction placement definition must be configured");

    assert_eq!(
        definition.capability_id.as_str(),
        PLACE_PROCESSING_RESTRICTION_CAPABILITY
    );
    assert_eq!(definition.capability_version.as_str(), "1.0.0");
    assert_eq!(definition.owner_module_id.as_str(), "crm.customer-privacy");
    assert!(definition.mutation);
    assert!(definition.requires_idempotency);
    assert!(!definition.requires_approval);
    assert_eq!(
        definition.authorization_policy_id,
        PLACE_PROCESSING_RESTRICTION_CAPABILITY
    );
}

#[test]
fn restriction_identity_is_stable_and_tenant_bound() {
    let first = deterministic_processing_restriction_id("tenant-a", "same-key")
        .expect("first restriction identity");
    let replay = deterministic_processing_restriction_id("tenant-a", "same-key")
        .expect("replayed restriction identity");
    let other_tenant = deterministic_processing_restriction_id("tenant-b", "same-key")
        .expect("other tenant restriction identity");
    let other_key = deterministic_processing_restriction_id("tenant-a", "other-key")
        .expect("other idempotency key restriction identity");

    assert_eq!(first, replay);
    assert_ne!(first, other_tenant);
    assert_ne!(first, other_key);
    assert!(first.as_str().starts_with("processing-restriction-"));
}
