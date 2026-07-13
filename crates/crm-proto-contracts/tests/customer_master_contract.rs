#![forbid(unsafe_code)]

use crm_proto_contracts::{
    crm::{
        core::v1::UnixTime,
        customer::v1::{AccountRef, ContactPointRef, CustomerResourceVersion, PartyRef},
        parties::v1::{CreatePartyRequest, Party, PartyCreatedEvent, PartyKind},
    },
    message_descriptor_hash,
};
use prost::Message;

const CREATE_PARTY_REQUEST_SCHEMA: &str = "crm.parties.v1.CreatePartyRequest";
const PARTY_CREATED_EVENT_SCHEMA: &str = "crm.parties.v1.PartyCreatedEvent";
const PARTY_REF_SCHEMA: &str = "crm.customer.v1.PartyRef";

#[test]
fn canonical_customer_references_are_distinct_typed_contracts() {
    let party = PartyRef {
        party_id: "party-01J00000000000000000000000".to_owned(),
    };
    let account = AccountRef {
        account_id: "account-01J000000000000000000000".to_owned(),
    };
    let contact_point = ContactPointRef {
        contact_point_id: "contact-01J00000000000000000000".to_owned(),
    };

    assert_eq!(
        PartyRef::decode(party.encode_to_vec().as_slice()).unwrap(),
        party
    );
    assert_eq!(
        AccountRef::decode(account.encode_to_vec().as_slice()).unwrap(),
        account
    );
    assert_eq!(
        ContactPointRef::decode(contact_point.encode_to_vec().as_slice()).unwrap(),
        contact_point
    );
}

#[test]
fn party_kind_has_an_explicit_unspecified_sentinel() {
    assert_eq!(PartyKind::Unspecified as i32, 0);
    assert_ne!(PartyKind::Person as i32, PartyKind::Organization as i32);
}

#[test]
fn unknown_future_party_kind_survives_wire_round_trip() {
    let request = CreatePartyRequest {
        party_ref: Some(PartyRef {
            party_id: "party-future-kind".to_owned(),
        }),
        kind: 77,
        display_name: "Future party kind".to_owned(),
    };

    let decoded = CreatePartyRequest::decode(request.encode_to_vec().as_slice()).unwrap();

    assert_eq!(decoded.kind, 77);
    assert!(PartyKind::try_from(decoded.kind).is_err());
}

#[test]
fn party_event_round_trip_preserves_identity_and_version_metadata() {
    let event = PartyCreatedEvent {
        party: Some(Party {
            party_ref: Some(PartyRef {
                party_id: "party-01J00000000000000000000001".to_owned(),
            }),
            kind: PartyKind::Organization as i32,
            display_name: "Northwind Holdings".to_owned(),
            resource_version: Some(CustomerResourceVersion {
                version: 1,
                created_at: Some(UnixTime { unix_nanos: 100 }),
                updated_at: Some(UnixTime { unix_nanos: 100 }),
            }),
        }),
    };

    let decoded = PartyCreatedEvent::decode(event.encode_to_vec().as_slice()).unwrap();

    assert_eq!(decoded, event);
}

#[test]
fn customer_contract_descriptor_identities_are_exact_and_distinct() {
    let create_hash = message_descriptor_hash(CREATE_PARTY_REQUEST_SCHEMA);
    let event_hash = message_descriptor_hash(PARTY_CREATED_EVENT_SCHEMA);
    let reference_hash = message_descriptor_hash(PARTY_REF_SCHEMA);

    assert_eq!(
        create_hash,
        message_descriptor_hash(CREATE_PARTY_REQUEST_SCHEMA)
    );
    assert_ne!(create_hash, event_hash);
    assert_ne!(create_hash, reference_hash);
    assert_ne!(event_hash, reference_hash);
}
