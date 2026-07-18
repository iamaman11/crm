#![forbid(unsafe_code)]

/// Strict immutable provider-profile and mapping persisted-state codecs.
pub mod definition_persistence;
/// Deterministic pre-I/O provider dispatch and crash-recovery protocol.
pub mod dispatch;
pub mod definitions;
pub mod lifecycle;
/// Strict bounded canonical persisted-state codecs for governed enrichment evidence.
#[allow(dead_code, unused_imports)]
pub mod persistence;
/// Pure-core governed reads, policy, provider dispatch and owner-application boundaries.
pub mod ports;
/// Exact identities for the eight manifest-owned enrichment record types.
pub mod records;
/// Immutable provider metering and quota evidence plus strict persistence.
pub mod provider_usage {
    #[cfg(test)]
    use crate::TargetField;
    include!("provider_usage.rs");
}

pub use definition_persistence::*;
pub use definitions::{
    MappingDraft, MappingNormalization, MappingVersion, MappingVersionId, ProviderProfileDraft,
    ProviderProfileVersion, ProviderProfileVersionId, RawPayloadPolicy, TargetField,
};
pub use dispatch::*;
pub use lifecycle::{
    ApplicationAttempt, ApplicationAttemptId, ApplicationOutcome, ApprovalRequirement,
    EnrichmentRequest, EnrichmentRequestDraft, EnrichmentRequestId, EnrichmentRequestStatus,
    ProviderResponseClass, ProviderResponseReceipt, ProviderResponseReceiptDraft,
    ProviderResponseReceiptId, RecordedApplicationOutcome, ReplayDisposition,
    RequestPolicyEvidence, ReviewDecision, ReviewDecisionId, ReviewDecisionKind, Suggestion,
    SuggestionDraft, SuggestionId, SuggestionLifecycleStatus, TargetSnapshot,
    derive_suggestion_status,
};
pub use persistence::*;
pub use ports::*;
pub use provider_usage::*;
pub use records::*;

/// Stable crate identity for repository tooling.
pub const CRATE_NAME: &str = "crm-customer-enrichment";
/// Immutable governed module identity.
pub const MODULE_ID: &str = "crm.customer-enrichment";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaffold_identity_is_explicit() {
        assert_eq!(CRATE_NAME, "crm-customer-enrichment");
        assert_eq!(MODULE_ID, "crm.customer-enrichment");
    }
}
