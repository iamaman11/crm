#![forbid(unsafe_code)]

//! Contract-only authoritative Customer Accounts privacy-scope contribution.
//!
//! The adapter enumerates Account-owned records associated with one canonical
//! Party inside a tenant-bound repeatable read-only PostgreSQL snapshot. It
//! reuses only the proven common owner-scope protocol support; Account scans,
//! embedded association semantics, pagination, classification and errors remain
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
    OUTPUT_SCHEMA_ID, customer_accounts_privacy_scope_definition,
};
pub use owner_action::{
    OWNER_ACTION_CAPABILITY_ID, CustomerAccountsPrivacyActionPlanner,
    CustomerAccountsPrivacyActionPolicy, customer_accounts_privacy_action_definition,
    customer_accounts_privacy_action_planner,
};
pub use postgres::CustomerAccountsPrivacyScopeQueryAdapter;
