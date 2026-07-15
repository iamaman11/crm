use crm_proto_contracts::crm::{customer::v1 as customer, data_quality::v1 as data_quality};
use prost::Message;

#[test]
fn party_rule_set_contract_round_trip_is_typed() {
    let definition = data_quality::PartyRuleSetDefinition {
        evaluator_semantic_version: data_quality::PartyQualityEvaluatorSemanticVersion::V1 as i32,
        rules: vec![data_quality::PartyQualityRule {
            rule_key: "display_name.minimum".to_owned(),
            severity: data_quality::QualitySeverity::Warning as i32,
            evaluator: Some(
                data_quality::party_quality_rule::Evaluator::DisplayNameMinUtf8Bytes(
                    data_quality::PartyDisplayNameMinUtf8BytesEvaluator {
                        minimum_utf8_bytes: 4,
                    },
                ),
            ),
            title: "Display name length".to_owned(),
            remediation_guidance: "Replace the display name with a meaningful customer name."
                .to_owned(),
        }],
    };

    let bytes = definition.encode_to_vec();
    let decoded = data_quality::PartyRuleSetDefinition::decode(bytes.as_slice()).unwrap();
    assert_eq!(decoded, definition);
}

#[test]
fn completeness_profile_contract_binds_exact_rule_set_version() {
    let definition = data_quality::PartyCompletenessProfileDefinition {
        completeness_semantic_version: data_quality::PartyCompletenessSemanticVersion::V1 as i32,
        rule_set_version_ref: Some(data_quality::PartyRuleSetVersionRef {
            rule_set_version_id: "dq-party-rule-set-example".to_owned(),
        }),
        components: vec![data_quality::PartyCompletenessComponent {
            component_key: "name.minimum".to_owned(),
            rule_key: "display_name.minimum".to_owned(),
            weight_basis_points: 10_000,
        }],
    };

    let bytes = definition.encode_to_vec();
    assert_eq!(
        data_quality::PartyCompletenessProfileDefinition::decode(bytes.as_slice()).unwrap(),
        definition
    );
}

#[test]
fn evaluation_request_binds_party_rule_set_and_profile_refs() {
    let request = data_quality::RequestPartyEvaluationRequest {
        evaluation_job_ref: Some(data_quality::PartyEvaluationJobRef {
            evaluation_job_id: "dq-evaluation-job-1".to_owned(),
        }),
        party_ref: Some(customer::PartyRef {
            party_id: "party-1".to_owned(),
        }),
        rule_set_version_ref: Some(data_quality::PartyRuleSetVersionRef {
            rule_set_version_id: "dq-party-rule-set-1".to_owned(),
        }),
        completeness_profile_version_ref: Some(data_quality::PartyCompletenessProfileVersionRef {
            completeness_profile_version_id: "dq-party-completeness-profile-1".to_owned(),
        }),
    };

    let bytes = request.encode_to_vec();
    assert_eq!(
        data_quality::RequestPartyEvaluationRequest::decode(bytes.as_slice()).unwrap(),
        request
    );
}

#[test]
fn stewardship_contract_binds_current_observation_and_expected_version() {
    let request = data_quality::AcknowledgeDataQualityFindingRequest {
        finding_ref: Some(data_quality::DataQualityFindingRef {
            finding_id: "dq-finding-1".to_owned(),
        }),
        expected_current_observation_ref: Some(data_quality::DataQualityFindingObservationRef {
            finding_observation_id: "dq-finding-observation-1".to_owned(),
        }),
        expected_version: 7,
    };

    let bytes = request.encode_to_vec();
    assert_eq!(
        data_quality::AcknowledgeDataQualityFindingRequest::decode(bytes.as_slice()).unwrap(),
        request
    );
}
