#![forbid(unsafe_code)]

//! Authoritative customer-privacy case and orchestration owner foundation.
//!
//! This pure module core owns privacy case, restriction, legal-hold and
//! owner-action evidence only. It contains no SQL, transport, scheduler,
//! secret-store or direct cross-owner storage access. Party, Consent,
//! Identity Resolution, Customer Data Operations and all other customer-master
//! values remain authoritative in their existing owner modules.

pub mod domain;

pub use domain::*;

/// Stable crate identity for repository tooling.
pub const CRATE_NAME: &str = "crm-customer-privacy";
/// Immutable governed module identity.
pub const MODULE_ID: &str = "crm.customer-privacy";

/// Authoritative privacy-case record type.
pub const PRIVACY_CASE_RECORD_TYPE: &str = "customer-privacy.case";
/// Authoritative current restriction record type.
pub const RESTRICTION_RECORD_TYPE: &str = "customer-privacy.restriction";
/// Authoritative customer-data legal-hold record type.
pub const LEGAL_HOLD_RECORD_TYPE: &str = "customer-privacy.legal-hold";
/// Immutable owner-aware privacy action plan record type.
pub const ACTION_PLAN_RECORD_TYPE: &str = "customer-privacy.action-plan";
/// Deterministic owner action attempt record type.
pub const OWNER_ACTION_ATTEMPT_RECORD_TYPE: &str = "customer-privacy.owner-action-attempt";
/// Append-once owner action outcome record type.
pub const OWNER_ACTION_OUTCOME_RECORD_TYPE: &str = "customer-privacy.owner-action-outcome";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn foundation_identity_and_owned_record_types_are_explicit() {
        assert_eq!(CRATE_NAME, "crm-customer-privacy");
        assert_eq!(MODULE_ID, "crm.customer-privacy");

        let record_types = [
            PRIVACY_CASE_RECORD_TYPE,
            RESTRICTION_RECORD_TYPE,
            LEGAL_HOLD_RECORD_TYPE,
            ACTION_PLAN_RECORD_TYPE,
            OWNER_ACTION_ATTEMPT_RECORD_TYPE,
            OWNER_ACTION_OUTCOME_RECORD_TYPE,
        ];
        assert_eq!(record_types.len(), 6);
        assert!(
            record_types
                .iter()
                .all(|value| value.starts_with("customer-privacy."))
        );
    }
}
