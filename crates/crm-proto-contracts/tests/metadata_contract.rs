#![forbid(unsafe_code)]

use crm_proto_contracts::{
    crm::metadata::v1::{
        MetadataDefinitionInput, MetadataKind, PublishMetadataBundleRequest,
        PublishMetadataBundleResponse,
    },
    message_descriptor_hash,
};
use prost::Message;

const PUBLISH_REQUEST_SCHEMA: &str = "crm.metadata.v1.PublishMetadataBundleRequest";
const PUBLISH_RESPONSE_SCHEMA: &str = "crm.metadata.v1.PublishMetadataBundleResponse";

#[test]
fn metadata_publish_contract_round_trips_strict_definition_input() {
    let request = PublishMetadataBundleRequest {
        definitions: vec![MetadataDefinitionInput {
            schema_version: "crm.metadata.definition/v1".to_owned(),
            definition_json: br#"{"kind":"object","definition":{"id":"crm.sales.deal","owner_module_id":"crm.sales","label":"Deal","plural_label":"Deals","description":null,"tags":["sales"]}}"#.to_vec(),
        }],
    };

    let encoded = request.encode_to_vec();
    let decoded = PublishMetadataBundleRequest::decode(encoded.as_slice()).unwrap();

    assert_eq!(decoded, request);
}

#[test]
fn metadata_contract_descriptor_identities_cover_request_and_response() {
    let request_hash = message_descriptor_hash(PUBLISH_REQUEST_SCHEMA);
    let response_hash = message_descriptor_hash(PUBLISH_RESPONSE_SCHEMA);

    assert_eq!(
        request_hash,
        message_descriptor_hash(PUBLISH_REQUEST_SCHEMA)
    );
    assert_ne!(request_hash, response_hash);
}

#[test]
fn metadata_kind_contract_has_an_explicit_unspecified_sentinel() {
    assert_eq!(MetadataKind::Unspecified as i32, 0);
    assert_ne!(MetadataKind::Object as i32, MetadataKind::Field as i32);
}

#[test]
fn publish_response_distinguishes_new_publication_from_idempotent_replay() {
    let fresh = PublishMetadataBundleResponse {
        revision_id: "a".repeat(64),
        newly_published: true,
    };
    let replay = PublishMetadataBundleResponse {
        revision_id: fresh.revision_id.clone(),
        newly_published: false,
    };

    assert_eq!(fresh.revision_id, replay.revision_id);
    assert_ne!(fresh.newly_published, replay.newly_published);
}
