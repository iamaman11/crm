#![forbid(unsafe_code)]

//! Authoritative owner of typed, time-bounded Party-to-Party relationship state.
//! Party identity remains owned by `crm.parties`; Account membership, generic
//! platform relationships, Customer 360 projections, consent, and Sales roles
//! remain outside this owner module.
//!
//! Cross-owner Party-reference integrity is composed by the application layer.
//! Persisted aggregate state is an internal deterministic versioned contract;
//! public Protobuf contracts remain an external additive boundary. Hierarchy
//! traversal indexes, trees, caches, and other read models are rebuildable
//! projections and never become a second authoritative relationship store.

pub mod domain;
pub mod persistence;

pub use domain::*;
pub use persistence::*;

/// Stable crate identity for repository tooling.
pub const CRATE_NAME: &str = "crm-party-relationships";
/// Immutable governed module identity.
pub const MODULE_ID: &str = "crm.party-relationships";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_identity_is_explicit() {
        assert_eq!(CRATE_NAME, "crm-party-relationships");
        assert_eq!(MODULE_ID, "crm.party-relationships");
    }
}
