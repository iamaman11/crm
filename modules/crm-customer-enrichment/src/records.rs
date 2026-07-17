/// Immutable published provider-profile version record type.
pub const PROVIDER_PROFILE_VERSION_RECORD_TYPE: &str =
    "customer_enrichment.provider_profile_version";
/// Immutable published mapping version record type.
pub const MAPPING_VERSION_RECORD_TYPE: &str = "customer_enrichment.mapping_version";
/// Governed enrichment request record type.
pub const ENRICHMENT_REQUEST_RECORD_TYPE: &str = "customer_enrichment.request";
/// Immutable provider response receipt record type.
pub const PROVIDER_RESPONSE_RECEIPT_RECORD_TYPE: &str =
    "customer_enrichment.provider_response_receipt";
/// Immutable enrichment suggestion record type.
pub const SUGGESTION_RECORD_TYPE: &str = "customer_enrichment.suggestion";
/// Immutable suggestion review-decision record type.
pub const REVIEW_DECISION_RECORD_TYPE: &str = "customer_enrichment.review_decision";
/// Immutable owner-capability application-attempt record type.
pub const APPLICATION_ATTEMPT_RECORD_TYPE: &str = "customer_enrichment.application_attempt";
/// Immutable provider metering/quota evidence record type.
pub const PROVIDER_USAGE_ENTRY_RECORD_TYPE: &str = "customer_enrichment.provider_usage_entry";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owned_record_types_are_exact_and_unique() {
        let record_types = [
            PROVIDER_PROFILE_VERSION_RECORD_TYPE,
            MAPPING_VERSION_RECORD_TYPE,
            ENRICHMENT_REQUEST_RECORD_TYPE,
            PROVIDER_RESPONSE_RECEIPT_RECORD_TYPE,
            SUGGESTION_RECORD_TYPE,
            REVIEW_DECISION_RECORD_TYPE,
            APPLICATION_ATTEMPT_RECORD_TYPE,
            PROVIDER_USAGE_ENTRY_RECORD_TYPE,
        ];
        let unique = record_types.into_iter().collect::<std::collections::BTreeSet<_>>();
        assert_eq!(unique.len(), 8);
        assert!(
            unique
                .iter()
                .all(|record_type| record_type.starts_with("customer_enrichment."))
        );
    }
}
