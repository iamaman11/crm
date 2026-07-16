use crm_proto_contracts::crm::{data_quality::v1 as data_quality, parties::v1 as parties};
use crm_proto_contracts::message_descriptor_hash;
use prost::Message;

#[test]
fn remediation_request_binds_exact_finding_observation_and_party_versions() {
    let request = data_quality::RemediatePartyDisplayNameRequest {
        finding_ref: Some(data_quality::DataQualityFindingRef {
            finding_id: "dq-finding-1".to_owned(),
        }),
        expected_finding_version: 4,
        expected_current_observation_ref: Some(
            data_quality::DataQualityFindingObservationRef {
                finding_observation_id: "dq-observation-1".to_owned(),
            },
        ),
        expected_party_resource_version: 7,
        display_name: "Ada Lovelace".to_owned(),
    };
    let bytes = request.encode_to_vec();
    assert_eq!(
        data_quality::RemediatePartyDisplayNameRequest::decode(bytes.as_slice()).unwrap(),
        request
    );
}

#[test]
fn remediation_response_carries_owner_party_and_durable_attempt_evidence() {
    let response = data_quality::RemediatePartyDisplayNameResponse {
        remediation_attempt: Some(data_quality::PartyDisplayNameRemediationAttempt {
            remediation_attempt_ref: Some(data_quality::DataQualityRemediationAttemptRef {
                remediation_attempt_id: "dq-remediation-attempt-1".to_owned(),
            }),
            finding_ref: Some(data_quality::DataQualityFindingRef {
                finding_id: "dq-finding-1".to_owned(),
            }),
            finding_observation_ref: Some(data_quality::DataQualityFindingObservationRef {
                finding_observation_id: "dq-observation-1".to_owned(),
            }),
            party_ref: None,
            expected_party_resource_version: None,
            requested_display_name: "Ada Lovelace".to_owned(),
            target_idempotency_key: "dq-remediation-target-1".to_owned(),
            updated_party_resource_version: None,
            completed_at: None,
            resource_version: None,
        }),
        party: Some(parties::Party {
            party_ref: None,
            kind: parties::PartyKind::Person as i32,
            display_name: "Ada Lovelace".to_owned(),
            resource_version: None,
        }),
    };
    let bytes = response.encode_to_vec();
    assert_eq!(
        data_quality::RemediatePartyDisplayNameResponse::decode(bytes.as_slice()).unwrap(),
        response
    );
}

#[test]
fn remediation_descriptor_identity_is_stable_and_distinct() {
    let request =
        message_descriptor_hash("crm.data_quality.v1.RemediatePartyDisplayNameRequest");
    let attempt =
        message_descriptor_hash("crm.data_quality.v1.PartyDisplayNameRemediationAttempt");
    assert_eq!(
        request,
        message_descriptor_hash("crm.data_quality.v1.RemediatePartyDisplayNameRequest")
    );
    assert_ne!(request, attempt);
}
