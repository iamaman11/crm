use crm_capability_runtime::CapabilityRisk;
use crm_data_quality_query_adapter::{
    GET_PARTY_COMPLETENESS_PROFILE_CAPABILITY, GET_PARTY_EVALUATION_JOB_CAPABILITY,
    GET_PARTY_RULE_SET_CAPABILITY, query_capability_definitions,
};
use crm_module_sdk::DataClass;

#[test]
fn data_quality_gets_remain_exact_classified_read_only_surfaces() {
    let definitions = query_capability_definitions().unwrap();
    assert_eq!(definitions.len(), 3);

    for (capability, expected_class) in [
        (GET_PARTY_RULE_SET_CAPABILITY, DataClass::Confidential),
        (
            GET_PARTY_COMPLETENESS_PROFILE_CAPABILITY,
            DataClass::Confidential,
        ),
        (GET_PARTY_EVALUATION_JOB_CAPABILITY, DataClass::Personal),
    ] {
        let definition = definitions
            .iter()
            .find(|definition| definition.capability_id.as_str() == capability)
            .expect("registered Data Quality query capability");
        assert_eq!(
            definition.input_contract.allowed_data_classes,
            vec![expected_class]
        );
        assert_eq!(
            definition
                .output_contract
                .as_ref()
                .expect("Data Quality query output contract")
                .allowed_data_classes,
            vec![expected_class]
        );
        assert_eq!(definition.risk, CapabilityRisk::Low);
        assert!(!definition.mutation);
        assert!(!definition.requires_idempotency);
        assert!(!definition.requires_approval);
    }
}
