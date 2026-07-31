#![forbid(unsafe_code)]

//! Contract-only authoritative Identity Resolution privacy-scope contribution.
//!
//! The adapter enumerates duplicate-candidate cases and reversible merge operations
//! relevant to one accepted canonical Party. Reverse alias discovery, merge topology,
//! persistence rehydration, heterogeneous pagination, classification, retention and
//! errors remain Identity Resolution-owned. No runtime route, application registration
//! or worker is introduced by this package; query execution remains tenant-bound and
//! read-only.

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
    INPUT_RETENTION_POLICY_ID, INPUT_SCHEMA_ID, MAX_PRIVACY_ACTIVE_REDIRECT_EDGES,
    MAX_PRIVACY_ALIAS_HOPS, MAX_PRIVACY_ALIAS_NODES, MAX_PRIVACY_CANDIDATE_RECORDS_REHYDRATED,
    MAX_PRIVACY_MERGE_RECORDS_REHYDRATED, MAX_PRIVACY_OWNER_RECORDS_SCANNED,
    MAX_PRIVACY_RELATIONSHIP_CANDIDATES, OUTPUT_MAXIMUM_BYTES, OUTPUT_RETENTION_POLICY_ID,
    OUTPUT_SCHEMA_ID, identity_resolution_privacy_scope_definition,
};
pub use owner_action::{
    OWNER_ACTION_CAPABILITY_ID, IdentityResolutionPrivacyActionPlanner,
    IdentityResolutionPrivacyActionPolicy, identity_resolution_privacy_action_definition,
    identity_resolution_privacy_action_planner,
};
pub use postgres::IdentityResolutionPrivacyScopeQueryAdapter;
