#![forbid(unsafe_code)]

//! Contract-only authoritative Party Relationships privacy-scope contribution.
//!
//! The adapter enumerates Party Relationship records whose authoritative persisted
//! Party reference matches one canonical Party inside a tenant-bound repeatable
//! read-only PostgreSQL snapshot. It reuses only the proven common owner-scope
//! protocol support; Party Relationship scans, Party matching, pagination,
//! classification, retention and errors remain owner-specific.

mod contract;
mod digest;
mod errors;
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
pub use postgres::PartyRelationshipsPrivacyScopeQueryAdapter;
