use crm_proto_contracts::crm::{customer::v1 as customer, data_quality::v1 as data_quality};
use crm_proto_contracts::message_descriptor_hash;
use prost::Message;

#[test]
fn finding_list_contract_binds_filters_page_size_and_signed_cursor() {
    let request = data_quality::ListDataQualityFindingsByPartyRequest {
        party_ref: Some(customer::PartyRef { party_id: "party-1".to_owned() }),
        status: Some(data_quality::DataQualityFindingStatus::Open as i32),
        severity: Some(data_quality::QualitySeverity::Warning as i32),
        page_size: 25,
        cursor: "signed-cursor".to_owned(),
    };
    let bytes = request.encode_to_vec();
    assert_eq!(data_quality::ListDataQualityFindingsByPartyRequest::decode(bytes.as_slice()).unwrap(), request);
}

#[test]
fn completeness_result_contract_preserves_exact_component_lineage() {
    let result = data_quality::PartyCompletenessResult {
        completeness_result_ref: Some(data_quality::PartyCompletenessResultRef { completeness_result_id: "dq-completeness-result-1".to_owned() }),
        evaluation_job_ref: Some(data_quality::PartyEvaluationJobRef { evaluation_job_id: "dq-evaluation-job-1".to_owned() }),
        party_ref: Some(customer::PartyRef { party_id: "party-1".to_owned() }),
        evaluated_party_resource_version: None,
        completeness_profile_version_ref: Some(data_quality::PartyCompletenessProfileVersionRef { completeness_profile_version_id: "dq-profile-1".to_owned() }),
        score_basis_points: 4_000,
        components: vec![data_quality::PartyCompletenessComponentResult {
            component_key: "name.placeholder".to_owned(),
            rule_key: "display_name.placeholder".to_owned(),
            rule_outcome_ref: Some(data_quality::PartyRuleOutcomeRef { rule_outcome_id: "dq-rule-outcome-1".to_owned() }),
            awarded_basis_points: 0,
        }],
        computed_at: None,
        resource_version: None,
    };
    let bytes = result.encode_to_vec();
    assert_eq!(data_quality::PartyCompletenessResult::decode(bytes.as_slice()).unwrap(), result);
}

#[test]
fn read_descriptor_identities_are_stable_and_distinct() {
    let finding = message_descriptor_hash("crm.data_quality.v1.GetDataQualityFindingRequest");
    let completeness = message_descriptor_hash("crm.data_quality.v1.PartyCompletenessResult");
    assert_eq!(finding, message_descriptor_hash("crm.data_quality.v1.GetDataQualityFindingRequest"));
    assert_ne!(finding, completeness);
}
