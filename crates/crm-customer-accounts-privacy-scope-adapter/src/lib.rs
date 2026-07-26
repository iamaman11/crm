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
mod postgres;
mod request;
mod response;

#[cfg(test)]
mod tests;

pub use contract::{
    CAPABILITY_ID, CAPABILITY_VERSION, customer_accounts_privacy_scope_definition,
};
pub use postgres::CustomerAccountsPrivacyScopeQueryAdapter;
