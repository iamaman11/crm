use super::data_quality_evaluation_fixture::{
    PARTY_CREATE, PUBLISH_PROFILE, PUBLISH_RULE_SET, REQUEST_EVALUATION,
};
use super::data_quality_evaluation_gateway::{mutate, mutation_definition, payload};
use crm_application_runtime::gateway_v1::application_gateway_service_client::ApplicationGatewayServiceClient;
use crm_proto_contracts::crm::{
    customer::v1 as customer, data_quality::v1 as data_quality, parties::v1 as parties,
};
use prost::Message;

pub async fn publish_rule_set(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: data_quality::PartyRuleSetDefinition,
    key: &str,
) -> data_quality::PartyRuleSetVersion {
    let capability = mutation_definition(PUBLISH_RULE_SET);
    let response = mutate(
        client,
        &capability,
        payload(
            &capability,
            data_quality::PublishPartyRuleSetVersionRequest {
                definition: Some(definition),
            },
        ),
        key,
    )
    .await
    .expect("publish evaluation rule set");
    data_quality::PublishPartyRuleSetVersionResponse::decode(
        response.output.expect("rule-set output").payload.as_slice(),
    )
    .expect("decode rule-set response")
    .rule_set_version
    .expect("published rule-set version")
}

pub async fn publish_profile(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: data_quality::PartyCompletenessProfileDefinition,
    key: &str,
) -> data_quality::PartyCompletenessProfileVersion {
    let capability = mutation_definition(PUBLISH_PROFILE);
    let response = mutate(
        client,
        &capability,
        payload(
            &capability,
            data_quality::PublishPartyCompletenessProfileVersionRequest {
                definition: Some(definition),
            },
        ),
        key,
    )
    .await
    .expect("publish evaluation completeness profile");
    data_quality::PublishPartyCompletenessProfileVersionResponse::decode(
        response.output.expect("profile output").payload.as_slice(),
    )
    .expect("decode profile response")
    .completeness_profile_version
    .expect("published completeness-profile version")
}

pub async fn create_party(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    party_id: &str,
    key: &str,
) -> parties::Party {
    create_party_with_display_name(client, party_id, "Ada Lovelace", key).await
}

pub async fn create_party_with_display_name(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    party_id: &str,
    display_name: &str,
    key: &str,
) -> parties::Party {
    let capability = mutation_definition(PARTY_CREATE);
    let response = mutate(
        client,
        &capability,
        payload(
            &capability,
            parties::CreatePartyRequest {
                party_ref: Some(customer::PartyRef {
                    party_id: party_id.to_owned(),
                }),
                kind: parties::PartyKind::Person as i32,
                display_name: display_name.to_owned(),
            },
        ),
        key,
    )
    .await
    .expect("create Party for evaluation staging");
    parties::CreatePartyResponse::decode(response.output.expect("Party output").payload.as_slice())
        .expect("decode Party response")
        .party
        .expect("created Party")
}

pub async fn request_evaluation(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    job_id: &str,
    party_id: &str,
    rule_set_version_id: &str,
    profile_version_id: &str,
    key: &str,
) -> data_quality::PartyEvaluationJob {
    let capability = mutation_definition(REQUEST_EVALUATION);
    let response = mutate(
        client,
        &capability,
        payload(
            &capability,
            data_quality::RequestPartyEvaluationRequest {
                evaluation_job_ref: Some(data_quality::PartyEvaluationJobRef {
                    evaluation_job_id: job_id.to_owned(),
                }),
                party_ref: Some(customer::PartyRef {
                    party_id: party_id.to_owned(),
                }),
                rule_set_version_ref: Some(data_quality::PartyRuleSetVersionRef {
                    rule_set_version_id: rule_set_version_id.to_owned(),
                }),
                completeness_profile_version_ref: Some(
                    data_quality::PartyCompletenessProfileVersionRef {
                        completeness_profile_version_id: profile_version_id.to_owned(),
                    },
                ),
            },
        ),
        key,
    )
    .await
    .expect("request Party evaluation");
    data_quality::RequestPartyEvaluationResponse::decode(
        response
            .output
            .expect("evaluation output")
            .payload
            .as_slice(),
    )
    .expect("decode evaluation response")
    .evaluation_job
    .expect("created evaluation job")
}

pub fn rule_set_version_id(value: &data_quality::PartyRuleSetVersion) -> String {
    value
        .rule_set_version_ref
        .as_ref()
        .expect("rule-set version ref")
        .rule_set_version_id
        .clone()
}

pub fn profile_version_id(value: &data_quality::PartyCompletenessProfileVersion) -> String {
    value
        .completeness_profile_version_ref
        .as_ref()
        .expect("profile version ref")
        .completeness_profile_version_id
        .clone()
}
