#![forbid(unsafe_code)]

//! Authoritative customer-privacy case and orchestration owner foundation.
//!
//! This pure module core owns privacy case, restriction, legal-hold and
//! owner-action evidence only. It contains no SQL, transport, scheduler,
//! secret-store or direct cross-owner storage access. Party, Consent,
//! Identity Resolution, Customer Data Operations and all other customer-master
//! values remain authoritative in their existing owner modules.

mod canonical_json;
mod canonicalization;

pub mod domain {
    include!("domain.rs");
    include!("scope.rs");
    include!("query_access.rs");

    pub mod persistence {
        use crate::canonicalization::persisted_state_json as serde_json;
        include!("persistence.rs");
    }
}

pub use domain::persistence::*;
pub use domain::*;

/// Stable crate identity for repository tooling.
pub const CRATE_NAME: &str = "crm-customer-privacy";
/// Immutable governed module identity.
pub const MODULE_ID: &str = "crm.customer-privacy";
/// Canonical private-state encoding profile.
pub const CANONICALIZATION_PROFILE_ID: &str = canonicalization::PROFILE_ID;

/// Authoritative privacy-case record type.
pub const PRIVACY_CASE_RECORD_TYPE: &str = "customer-privacy.case";
/// Authoritative current restriction record type.
pub const RESTRICTION_RECORD_TYPE: &str = "customer-privacy.restriction";
/// Authoritative customer-data legal-hold record type.
pub const LEGAL_HOLD_RECORD_TYPE: &str = "customer-privacy.legal-hold";
/// Immutable complete privacy scope snapshot record type.
pub const SCOPE_SNAPSHOT_RECORD_TYPE: &str = "customer-privacy.scope-snapshot";
/// Immutable receipt for one exact owner scope contribution.
pub const OWNER_SCOPE_CONTRIBUTION_RECEIPT_RECORD_TYPE: &str =
    "customer-privacy.owner-scope-contribution";
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
        assert_eq!(CANONICALIZATION_PROFILE_ID, "crm.cjson/v1");

        let record_types = [
            PRIVACY_CASE_RECORD_TYPE,
            RESTRICTION_RECORD_TYPE,
            LEGAL_HOLD_RECORD_TYPE,
            SCOPE_SNAPSHOT_RECORD_TYPE,
            OWNER_SCOPE_CONTRIBUTION_RECEIPT_RECORD_TYPE,
            ACTION_PLAN_RECORD_TYPE,
            OWNER_ACTION_ATTEMPT_RECORD_TYPE,
            OWNER_ACTION_OUTCOME_RECORD_TYPE,
        ];
        assert_eq!(record_types.len(), 8);
        assert!(
            record_types
                .iter()
                .all(|value| value.starts_with("customer-privacy."))
        );
    }
}
