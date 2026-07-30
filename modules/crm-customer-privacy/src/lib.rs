#![forbid(unsafe_code)]

//! Authoritative customer-privacy case and orchestration owner foundation.
//!
//! This pure module core owns privacy case, restriction, legal-hold and
//! orchestration evidence only. It contains no SQL, transport, scheduler,
//! secret-store or direct cross-owner storage access. Party, Consent,
//! Identity Resolution, Customer Data Operations and all other customer-master
//! values remain authoritative in their existing owner modules.

mod canonical_json;
mod canonicalization;

pub mod domain {
    include!("domain.rs");

    pub mod scope {
        include!("scope.rs");
        include!("scope_discovery.rs");
        include!("scope_planning.rs");
    }
    pub use scope::*;

    include!("query_access.rs");
    include!("retention.rs");
    include!("execution.rs");

    pub mod access_export {
        use super::*;

        impl serde::Serialize for PrivacyCaseKind {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                serializer.serialize_str(match self {
                    Self::Access => "access",
                    Self::PortabilityExport => "portability_export",
                    Self::RestrictProcessing => "restrict_processing",
                    Self::Erasure => "erasure",
                })
            }
        }

        impl<'de> serde::Deserialize<'de> for PrivacyCaseKind {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let value = <String as serde::Deserialize>::deserialize(deserializer)?;
                match value.as_str() {
                    "access" => Ok(Self::Access),
                    "portability_export" => Ok(Self::PortabilityExport),
                    "restrict_processing" => Ok(Self::RestrictProcessing),
                    "erasure" => Ok(Self::Erasure),
                    _ => Err(serde::de::Error::custom(
                        "privacy case kind is not a canonical v1 value",
                    )),
                }
            }
        }

        include!("access_export.rs");
    }
    pub use access_export::*;

    impl CustomerDataLegalHold {
        pub const fn effective_from_unix_nanos(&self) -> i64 {
            self.effective_from_unix_nanos
        }

        pub const fn effective_until_unix_nanos(&self) -> Option<i64> {
            self.effective_until_unix_nanos
        }
    }

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
/// Immutable legal-hold and mandatory-retention adjudication record type.
pub const RETENTION_DECISION_RECORD_TYPE: &str = "customer-privacy.retention-decision";
/// Deterministic owner action attempt record type.
pub const OWNER_ACTION_ATTEMPT_RECORD_TYPE: &str = "customer-privacy.owner-action-attempt";
/// Append-once owner action outcome record type.
pub const OWNER_ACTION_OUTCOME_RECORD_TYPE: &str = "customer-privacy.owner-action-outcome";
/// Customer Privacy-owned immutable manifest and stable Customer Data Operations export references.
pub const ACCESS_EXPORT_REFERENCE_RECORD_TYPE: &str = "customer-privacy.access-export-reference";

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
            RETENTION_DECISION_RECORD_TYPE,
            OWNER_ACTION_ATTEMPT_RECORD_TYPE,
            OWNER_ACTION_OUTCOME_RECORD_TYPE,
            ACCESS_EXPORT_REFERENCE_RECORD_TYPE,
        ];
        assert_eq!(record_types.len(), 10);
        assert!(
            record_types
                .iter()
                .all(|value| value.starts_with("customer-privacy."))
        );
    }
}
