use super::NOW;
use crm_customer_enrichment::Suggestion;
use crm_customer_enrichment_review_adapter::suggestion_to_wire;
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;

pub fn list_request(
    suggestion: &Suggestion,
    page_size: i32,
    cursor: impl Into<String>,
) -> wire::ListSuggestionsByPartyRequest {
    let public = suggestion_to_wire(
        suggestion,
        None,
        u64::try_from(NOW / 1_000_000).expect("non-negative process clock"),
    )
    .expect("convert strict suggestion fixture");
    wire::ListSuggestionsByPartyRequest {
        party_ref: public.target.expect("suggestion target").party_ref,
        status: Some(wire::SuggestionLifecycleStatus::Proposed as i32),
        provider_profile_version_ref: public.provider_profile_version_ref,
        page_size,
        cursor: cursor.into(),
    }
}
