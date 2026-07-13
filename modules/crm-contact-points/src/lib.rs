#![forbid(unsafe_code)]

//! Authoritative owner of Party-associated Contact Point lifecycle and endpoint
//! verification state. Consent and communication authorization, provider
//! delivery state, SQL, transport contracts, and direct cross-owner storage
//! access remain outside this owner module. Cross-owner integrity is composed
//! by the application layer rather than coupled into the owner aggregate.
//!
//! Persisted aggregate state is an internal deterministic versioned contract;
//! public Protobuf contracts remain an external additive boundary and do not
//! leak transport types into the owner domain.

pub mod domain;
pub mod persistence;

pub use domain::*;
pub use persistence::*;

/// Stable crate identity for repository tooling.
pub const CRATE_NAME: &str = "crm-contact-points";
/// Immutable governed module identity.
pub const MODULE_ID: &str = "crm.contact-points";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_identity_is_explicit() {
        assert_eq!(CRATE_NAME, "crm-contact-points");
        assert_eq!(MODULE_ID, "crm.contact-points");
    }
}
