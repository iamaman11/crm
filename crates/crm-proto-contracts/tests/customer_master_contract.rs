#![forbid(unsafe_code)]

use crm_proto_contracts::{
    crm::{
        accounts::v1::{
            Account, AccountCreatedEvent, AccountPartyAssociation, AccountPartyRole, AccountStatus,
            CreateAccountRequest,
        },
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
const CREATE_ACCOUNT_REQUEST_SCHEMA: &str = "crm.accounts.v1.CreateAccountRequest";
const ACCOUNT_CREATED_EVENT_SCHEMA: &str = "crm.accounts.v1.AccountCreatedEvent";

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
fn account_contract_preserves_typed_party_associations_without_copying_party_identity() {
    let request = CreateAccountRequest {
        account_ref: Some(AccountRef {
            account_id: "account-northwind".to_owned(),
        }),
        name: "Northwind Customer Group".to_owned(),
        party_associations: vec![
            AccountPartyAssociation {
                party_ref: Some(PartyRef {
                    party_id: "party-northwind-org".to_owned(),
                }),
                role: AccountPartyRole::Primary as i32,
            },
            AccountPartyAssociation {
                party_ref: Some(PartyRef {
                    party_id: "party-ada-buyer".to_owned(),
                }),
                role: AccountPartyRole::Member as i32,
            },
        ],
    };

    let decoded = CreateAccountRequest::decode(request.encode_to_vec().as_slice()).unwrap();

    assert_eq!(decoded, request);
    assert_eq!(decoded.party_associations.len(), 2);
    assert_eq!(
        decoded.party_associations[0]
            .party_ref
            .as_ref()
            .unwrap()
            .party_id,
        "party-northwind-org"
    );
}

#[test]
fn unknown_future_account_enums_survive_wire_round_trip() {
    let account = Account {
        account_ref: Some(AccountRef {
            account_id: "account-future-enums".to_owned(),
        }),
        name: "Future Account".to_owned(),
        status: 77,
        party_associations: vec![AccountPartyAssociation {
            party_ref: Some(PartyRef {
                party_id: "party-future-role".to_owned(),
            }),
            role: 88,
        }],
        resource_version: None,
    };

    let decoded = Account::decode(account.encode_to_vec().as_slice()).unwrap();

    assert_eq!(decoded.status, 77);
    assert!(AccountStatus::try_from(decoded.status).is_err());
    assert_eq!(decoded.party_associations[0].role, 88);
    assert!(AccountPartyRole::try_from(decoded.party_associations[0].role).is_err());
}

#[test]
fn account_event_round_trip_preserves_references_status_and_version_metadata() {
    let event = AccountCreatedEvent {
        account: Some(Account {
            account_ref: Some(AccountRef {
                account_id: "account-01J000000000000000000001".to_owned(),
            }),
            name: "Northwind Customer Group".to_owned(),
            status: AccountStatus::Active as i32,
            party_associations: vec![AccountPartyAssociation {
                party_ref: Some(PartyRef {
                    party_id: "party-01J000000000000000000001".to_owned(),
                }),
                role: AccountPartyRole::Primary as i32,
            }],
            resource_version: Some(CustomerResourceVersion {
                version: 1,
                created_at: Some(UnixTime { unix_nanos: 200 }),
                updated_at: Some(UnixTime { unix_nanos: 200 }),
            }),
        }),
    };

    let decoded = AccountCreatedEvent::decode(event.encode_to_vec().as_slice()).unwrap();

    assert_eq!(decoded, event);
}

#[test]
fn customer_contract_descriptor_identities_are_exact_and_distinct() {
    let party_create_hash = message_descriptor_hash(CREATE_PARTY_REQUEST_SCHEMA);
    let party_event_hash = message_descriptor_hash(PARTY_CREATED_EVENT_SCHEMA);
    let reference_hash = message_descriptor_hash(PARTY_REF_SCHEMA);
    let account_create_hash = message_descriptor_hash(CREATE_ACCOUNT_REQUEST_SCHEMA);
    let account_event_hash = message_descriptor_hash(ACCOUNT_CREATED_EVENT_SCHEMA);

    assert_eq!(
        party_create_hash,
        message_descriptor_hash(CREATE_PARTY_REQUEST_SCHEMA)
    );
    assert_eq!(
        account_create_hash,
        message_descriptor_hash(CREATE_ACCOUNT_REQUEST_SCHEMA)
    );
    assert_ne!(party_create_hash, party_event_hash);
    assert_ne!(party_create_hash, reference_hash);
    assert_ne!(party_event_hash, reference_hash);
    assert_ne!(account_create_hash, account_event_hash);
    assert_ne!(account_create_hash, reference_hash);
    assert_ne!(party_create_hash, account_create_hash);
}
