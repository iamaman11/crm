#![forbid(unsafe_code)]

//! Contract-only authoritative Party Relationships privacy-scope contribution.
//!
//! The adapter enumerates authoritative Party Relationship records when either
//! rehydrated `from_party_ref` or `to_party_ref` matches one canonical Party inside
//! a tenant-bound repeatable read-only PostgreSQL snapshot. It reuses only the
//! proven common owner-scope protocol support; two-endpoint matching, relationship
//! semantics, scans, pagination, classification, retention and errors remain
//! owner-specific.

mod contract;
mod digest;
mod errors;
mod owner_action;
mod postgres;
mod request;
mod response;

#[cfg(test)]
mod tests;

pub use contract::{
    CAPABILITY_ID, CAPABILITY_VERSION, CONTRACT_SCHEMA_VERSION, INPUT_MAXIMUM_BYTES,
    INPUT_RETENTION_POLICY_ID, INPUT_SCHEMA_ID, OUTPUT_MAXIMUM_BYTES, OUTPUT_RETENTION_POLICY_ID,
    OUTPUT_SCHEMA_ID, party_relationships_privacy_scope_definition,
};
pub use owner_action::{
    OWNER_ACTION_CAPABILITY_ID, PartyRelationshipsPrivacyActionPlanner,
    PartyRelationshipsPrivacyActionPolicy, party_relationships_privacy_action_definition,
    party_relationships_privacy_action_planner,
};
pub use postgres::PartyRelationshipsPrivacyScopeQueryAdapter;
