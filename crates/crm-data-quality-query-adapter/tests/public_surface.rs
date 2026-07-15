use crm_capability_runtime::CapabilityRisk;
use crm_data_quality_query_adapter::{GET_PARTY_RULE_SET_CAPABILITY, query_capability_definition};
use crm_module_sdk::DataClass;

#[test]
fn rule_set_get_remains_exact_confidential_read_only_surface() {
    let definition = query_capability_definition().unwrap();

    assert_eq!(
        definition.capability_id.as_str(),
        GET_PARTY_RULE_SET_CAPABILITY
    );
    assert_eq!(
        definition.input_contract.allowed_data_classes,
        vec![DataClass::Confidential]
    );
    assert_eq!(definition.risk, CapabilityRisk::Low);
    assert!(!definition.mutation);
    assert!(!definition.requires_idempotency);
    assert!(!definition.requires_approval);
}
