#![forbid(unsafe_code)]

pub mod definitions;
pub mod lifecycle;
/// Strict bounded canonical persisted-state codecs for governed enrichment evidence.
#[allow(unused_imports)]
pub mod persistence;

pub use definitions::{
    MappingDraft, MappingNormalization, MappingVersion, MappingVersionId, ProviderProfileDraft,
    ProviderProfileVersion, ProviderProfileVersionId, RawPayloadPolicy, TargetField,
};
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
