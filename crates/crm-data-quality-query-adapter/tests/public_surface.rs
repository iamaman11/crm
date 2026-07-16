use crm_capability_runtime::CapabilityRisk;
use crm_data_quality_query_adapter::{
    GET_PARTY_COMPLETENESS_PROFILE_CAPABILITY, GET_PARTY_RULE_SET_CAPABILITY,
    query_capability_definitions,
};
use crm_module_sdk::DataClass;

#[test]
fn immutable_definition_gets_remain_exact_confidential_read_only_surfaces() {
    let definitions = query_capability_definitions().unwrap();
    assert_eq!(definitions.len(), 2);

    for capability in [
        GET_PARTY_RULE_SET_CAPABILITY,
        GET_PARTY_COMPLETENESS_PROFILE_CAPABILITY,
    ] {
        let definition = definitions
            .iter()
            .find(|definition| definition.capability_id.as_str() == capability)
            .expect("registered Data Quality query capability");
        assert_eq!(
            definition.input_contract.allowed_data_classes,
            vec![DataClass::Confidential]
        );
        assert_eq!(definition.risk, CapabilityRisk::Low);
        assert!(!definition.mutation);
        assert!(!definition.requires_idempotency);
        assert!(!definition.requires_approval);
    }
}
