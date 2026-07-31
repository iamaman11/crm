#![forbid(unsafe_code)]

//! Authoritative non-runtime Consents privacy-scope contribution adapter.
//!
//! The adapter validates the exact current owner contract, proves canonical
//! Party lineage, reads authoritative Party-to-Consent relationships and strict
//! Consent records inside one tenant-bound repeatable READ ONLY PostgreSQL
//! snapshot, and returns reference-only immutable evidence.

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
    CAPABILITY_ID, CAPABILITY_VERSION, CONTRACT_SCHEMA_VERSION, DEFAULT_PAGE_SIZE,
    INPUT_MAXIMUM_BYTES, INPUT_RETENTION_POLICY_ID, INPUT_SCHEMA_ID, MAXIMUM_CURSOR_BYTES,
    MAXIMUM_PAGE_SIZE, OUTPUT_MAXIMUM_BYTES, OUTPUT_RETENTION_POLICY_ID, OUTPUT_SCHEMA_ID,
    consents_privacy_scope_definition,
};
pub use owner_action::{
    OWNER_ACTION_CAPABILITY_ID, ConsentsPrivacyActionPlanner, ConsentsPrivacyActionPolicy,
    consents_privacy_action_definition, consents_privacy_action_planner,
};
pub use postgres::ConsentsPrivacyScopeQueryAdapter;
