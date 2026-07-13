#![forbid(unsafe_code)]

use crm_proto_contracts::{
    crm::{
        core::v1::UnixTime,
        customer::v1::{CustomerResourceVersion, PartyRef, PartyRelationshipRef},
        party_relationships::v1::{
            CreatePartyRelationshipRequest, PartyRelationship, PartyRelationshipCreatedEvent,
            PartyRelationshipDirectionality, PartyRelationshipStatus, PartyRelationshipType,
        },
    },
    message_descriptor_hash,
};
use prost::Message;

const PARTY_RELATIONSHIP_REF_SCHEMA: &str = "crm.customer.v1.PartyRelationshipRef";
const CREATE_PARTY_RELATIONSHIP_REQUEST_SCHEMA: &str =
    "crm.party_relationships.v1.CreatePartyRelationshipRequest";
const PARTY_RELATIONSHIP_CREATED_EVENT_SCHEMA: &str =
    "crm.party_relationships.v1.PartyRelationshipCreatedEvent";

#[test]
fn canonical_party_relationship_reference_is_a_distinct_typed_contract() {
    let value = PartyRelationshipRef {
        party_relationship_id: "party-relationship-01J00000000000000000001".to_owned(),
    };
    assert_eq!(
        PartyRelationshipRef::decode(value.encode_to_vec().as_slice()).unwrap(),
        value
    );
}

#[test]
fn create_contract_preserves_typed_party_endpoints_and_relationship_semantics() {
    let request = CreatePartyRelationshipRequest {
        party_relationship_ref: Some(PartyRelationshipRef {
            party_relationship_id: "relationship-employment-ada".to_owned(),
        }),
        from_party_ref: Some(PartyRef {
            party_id: "party-acme".to_owned(),
        }),
        to_party_ref: Some(PartyRef {
            party_id: "party-ada".to_owned(),
        }),
        relationship_type: Some(PartyRelationshipType {
            code: "employment".to_owned(),
            directionality: PartyRelationshipDirectionality::Directional as i32,
            from_role: "employer".to_owned(),
            to_role: "employee".to_owned(),
        }),
        valid_from: Some(UnixTime { unix_nanos: 100 }),
        valid_until: Some(UnixTime { unix_nanos: 1_000 }),
    };

    let decoded =
        CreatePartyRelationshipRequest::decode(request.encode_to_vec().as_slice()).unwrap();
    assert_eq!(decoded, request);
    assert_eq!(decoded.from_party_ref.unwrap().party_id, "party-acme");
    assert_eq!(decoded.to_party_ref.unwrap().party_id, "party-ada");
}

#[test]
fn unknown_future_party_relationship_enums_survive_wire_round_trip() {
    let value = PartyRelationship {
        party_relationship_ref: Some(PartyRelationshipRef {
            party_relationship_id: "relationship-future-enums".to_owned(),
        }),
        from_party_ref: Some(PartyRef {
            party_id: "party-a".to_owned(),
        }),
        to_party_ref: Some(PartyRef {
            party_id: "party-b".to_owned(),
        }),
        relationship_type: Some(PartyRelationshipType {
            code: "future_semantics".to_owned(),
            directionality: 77,
            from_role: "source".to_owned(),
            to_role: "target".to_owned(),
        }),
        status: 88,
        valid_from: None,
        valid_until: None,
        resource_version: None,
    };

    let decoded = PartyRelationship::decode(value.encode_to_vec().as_slice()).unwrap();
    assert_eq!(decoded.relationship_type.unwrap().directionality, 77);
    assert!(PartyRelationshipDirectionality::try_from(77).is_err());
    assert_eq!(decoded.status, 88);
    assert!(PartyRelationshipStatus::try_from(88).is_err());
}

#[test]
fn created_event_round_trip_preserves_references_semantics_and_version_metadata() {
    let event = PartyRelationshipCreatedEvent {
        party_relationship: Some(PartyRelationship {
            party_relationship_ref: Some(PartyRelationshipRef {
                party_relationship_id: "relationship-household-1".to_owned(),
            }),
            from_party_ref: Some(PartyRef {
                party_id: "party-a".to_owned(),
            }),
            to_party_ref: Some(PartyRef {
                party_id: "party-b".to_owned(),
            }),
            relationship_type: Some(PartyRelationshipType {
                code: "household".to_owned(),
                directionality: PartyRelationshipDirectionality::Reciprocal as i32,
                from_role: "household_member".to_owned(),
                to_role: "household_member".to_owned(),
            }),
            status: PartyRelationshipStatus::Active as i32,
            valid_from: Some(UnixTime { unix_nanos: 100 }),
            valid_until: None,
            resource_version: Some(CustomerResourceVersion {
                version: 1,
                created_at: Some(UnixTime { unix_nanos: 100 }),
                updated_at: Some(UnixTime { unix_nanos: 100 }),
            }),
        }),
    };

    let decoded = PartyRelationshipCreatedEvent::decode(event.encode_to_vec().as_slice()).unwrap();
    assert_eq!(decoded, event);
}

#[test]
fn party_relationship_descriptor_identities_are_exact_and_distinct() {
    let reference_hash = message_descriptor_hash(PARTY_RELATIONSHIP_REF_SCHEMA);
    let create_hash = message_descriptor_hash(CREATE_PARTY_RELATIONSHIP_REQUEST_SCHEMA);
    let event_hash = message_descriptor_hash(PARTY_RELATIONSHIP_CREATED_EVENT_SCHEMA);

    assert_eq!(
        reference_hash,
        message_descriptor_hash(PARTY_RELATIONSHIP_REF_SCHEMA)
    );
    assert_eq!(
        create_hash,
        message_descriptor_hash(CREATE_PARTY_RELATIONSHIP_REQUEST_SCHEMA)
    );
    assert_eq!(
        event_hash,
        message_descriptor_hash(PARTY_RELATIONSHIP_CREATED_EVENT_SCHEMA)
    );
    assert_ne!(reference_hash, create_hash);
    assert_ne!(create_hash, event_hash);
    assert_ne!(reference_hash, event_hash);
}
