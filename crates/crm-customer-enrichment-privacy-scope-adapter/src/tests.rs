use crate::contract::{
    CAPABILITY_ID, CAPABILITY_VERSION, DEFAULT_PAGE_SIZE, MAXIMUM_CURSOR_BYTES, MAXIMUM_PAGE_SIZE,
    MAX_PRIVACY_ACTIVE_REDIRECT_EDGES, MAX_PRIVACY_ALIAS_HOPS, MAX_PRIVACY_ALIAS_NODES,
    MAX_PRIVACY_APPLICATION_ATTEMPTS_SCANNED, MAX_PRIVACY_ASSOCIATION_RECORDS_REHYDRATED,
    MAX_PRIVACY_CANONICAL_PARTY_RESOLUTIONS, MAX_PRIVACY_DEFINITION_RECORDS_REHYDRATED,
    MAX_PRIVACY_OWNER_RECORDS_SCANNED, MAX_PRIVACY_PROVIDER_USAGE_ENTRIES_SCANNED,
    MAX_PRIVACY_REQUEST_RECORDS_REHYDRATED, MAX_PRIVACY_REQUEST_RELATIONSHIPS_SCANNED,
    MAX_PRIVACY_RESPONSE_CONFLICTS_SCANNED, MAX_PRIVACY_RESPONSE_RECEIPTS_SCANNED,
    MAX_PRIVACY_REVIEW_DECISIONS_SCANNED, MAX_PRIVACY_SUGGESTIONS_SCANNED,
    PRIVACY_OWNER_SCAN_BATCH_SIZE, PRIVACY_RELATIONSHIP_SCAN_BATCH_SIZE,
    customer_enrichment_privacy_scope_definition,
};
use crate::request::ResourceFamily;
use crm_customer_enrichment_capability_adapter::MODULE_ID;

#[test]
fn definition_is_exact_contract_only_query_coordinate() {
    let definition = customer_enrichment_privacy_scope_definition().unwrap();
    assert_eq!(definition.owner_module_id.as_str(), MODULE_ID);
    assert_eq!(definition.capability_id.as_str(), CAPABILITY_ID);
    assert_eq!(definition.capability_version.as_str(), CAPABILITY_VERSION);
    assert!(!definition.mutation);
    assert!(!definition.requires_idempotency);
    assert!(!definition.requires_approval);
    assert_eq!(definition.authorization_policy_id, CAPABILITY_ID);
}

#[test]
fn frozen_bounds_match_the_entry_packet() {
    assert_eq!(DEFAULT_PAGE_SIZE, 64);
    assert_eq!(MAXIMUM_PAGE_SIZE, 128);
    assert_eq!(MAXIMUM_CURSOR_BYTES, 2_048);
    assert_eq!(MAX_PRIVACY_ALIAS_HOPS, 64);
    assert_eq!(MAX_PRIVACY_ALIAS_NODES, 4_096);
    assert_eq!(MAX_PRIVACY_ACTIVE_REDIRECT_EDGES, 4_095);
    assert_eq!(MAX_PRIVACY_REQUEST_RELATIONSHIPS_SCANNED, 16_384);
    assert_eq!(MAX_PRIVACY_REQUEST_RECORDS_REHYDRATED, 16_384);
    assert_eq!(MAX_PRIVACY_RESPONSE_RECEIPTS_SCANNED, 32_768);
    assert_eq!(MAX_PRIVACY_RESPONSE_CONFLICTS_SCANNED, 16_384);
    assert_eq!(MAX_PRIVACY_SUGGESTIONS_SCANNED, 65_536);
    assert_eq!(MAX_PRIVACY_REVIEW_DECISIONS_SCANNED, 65_536);
    assert_eq!(MAX_PRIVACY_APPLICATION_ATTEMPTS_SCANNED, 65_536);
    assert_eq!(MAX_PRIVACY_PROVIDER_USAGE_ENTRIES_SCANNED, 65_536);
    assert_eq!(MAX_PRIVACY_DEFINITION_RECORDS_REHYDRATED, 8_192);
    assert_eq!(MAX_PRIVACY_ASSOCIATION_RECORDS_REHYDRATED, 131_072);
    assert_eq!(MAX_PRIVACY_CANONICAL_PARTY_RESOLUTIONS, 16_384);
    assert_eq!(MAX_PRIVACY_OWNER_RECORDS_SCANNED, 131_072);
    assert_eq!(PRIVACY_RELATIONSHIP_SCAN_BATCH_SIZE, 512);
    assert_eq!(PRIVACY_OWNER_SCAN_BATCH_SIZE, 512);
}

#[test]
fn seven_family_order_and_tokens_are_stable() {
    let families = [
        ResourceFamily::Request,
        ResourceFamily::ResponseReceipt,
        ResourceFamily::ResponseConflict,
        ResourceFamily::Suggestion,
        ResourceFamily::ReviewDecision,
        ResourceFamily::ApplicationAttempt,
        ResourceFamily::ProviderUsageEntry,
    ];
    assert_eq!(
        families.map(ResourceFamily::token),
        [
            "request",
            "receipt",
            "conflict",
            "suggestion",
            "review",
            "application",
            "usage",
        ]
    );
    assert!(families.windows(2).all(|pair| pair[0] < pair[1]));
}
