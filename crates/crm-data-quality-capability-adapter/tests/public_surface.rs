use crm_capability_runtime::CapabilityRisk;
use crm_data_quality_capability_adapter::{
    MODULE_ID, PUBLISH_PARTY_RULE_SET_CAPABILITY, capability_definition,
};
use crm_module_sdk::DataClass;

#[test]
fn rule_set_publication_remains_exact_confidential_idempotent_surface() {
    let definition = capability_definition().unwrap();

    assert_eq!(definition.owner_module_id.as_str(), MODULE_ID);
    assert_eq!(
        definition.capability_id.as_str(),
        PUBLISH_PARTY_RULE_SET_CAPABILITY
    );
    assert_eq!(
        definition.authorization_policy_id,
        PUBLISH_PARTY_RULE_SET_CAPABILITY
    );
    assert_eq!(
        definition.input_contract.allowed_data_classes,
        vec![DataClass::Confidential]
    );
    assert_eq!(
        definition
            .output_contract
            .as_ref()
            .unwrap()
            .allowed_data_classes,
        vec![DataClass::Confidential]
    );
    assert_eq!(definition.risk, CapabilityRisk::Medium);
    assert!(definition.mutation);
    assert!(definition.requires_idempotency);
    assert!(!definition.requires_approval);
}
