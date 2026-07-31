#![forbid(unsafe_code)]

//! Contract-only authoritative Customer Enrichment privacy-scope contribution.
//!
//! The adapter discovers Party-bound enrichment requests through the existing owner relationship
//! and contributes only minimized references to exact request-descendant evidence. Shared provider
//! and mapping definitions remain strict validation dependencies and are never emitted. No route,
//! worker, application registration, provider transport or Party mutation is introduced here.

mod contract;
mod digest;
mod errors;
mod owner_action;
#[allow(dead_code)]
mod postgres;
mod request;
mod response;

#[cfg(test)]
mod tests;

pub use contract::{
    CAPABILITY_ID, CAPABILITY_VERSION, CONTRACT_SCHEMA_VERSION, DEFAULT_PAGE_SIZE,
    INPUT_MAXIMUM_BYTES, INPUT_RETENTION_POLICY_ID, INPUT_SCHEMA_ID,
    MAX_PRIVACY_ACTIVE_REDIRECT_EDGES, MAX_PRIVACY_ALIAS_HOPS, MAX_PRIVACY_ALIAS_NODES,
    MAX_PRIVACY_APPLICATION_ATTEMPTS_SCANNED, MAX_PRIVACY_ASSOCIATION_RECORDS_REHYDRATED,
    MAX_PRIVACY_CANONICAL_PARTY_RESOLUTIONS, MAX_PRIVACY_DEFINITION_RECORDS_REHYDRATED,
    MAX_PRIVACY_OWNER_RECORDS_SCANNED, MAX_PRIVACY_PROVIDER_USAGE_ENTRIES_SCANNED,
    MAX_PRIVACY_REQUEST_RECORDS_REHYDRATED, MAX_PRIVACY_REQUEST_RELATIONSHIPS_SCANNED,
    MAX_PRIVACY_RESPONSE_CONFLICTS_SCANNED, MAX_PRIVACY_RESPONSE_RECEIPTS_SCANNED,
    MAX_PRIVACY_REVIEW_DECISIONS_SCANNED, MAX_PRIVACY_SUGGESTIONS_SCANNED, MAXIMUM_CURSOR_BYTES,
    MAXIMUM_PAGE_SIZE, OUTPUT_MAXIMUM_BYTES, OUTPUT_RETENTION_POLICY_ID, OUTPUT_SCHEMA_ID,
    PRIVACY_OWNER_SCAN_BATCH_SIZE, PRIVACY_RELATIONSHIP_SCAN_BATCH_SIZE,
    customer_enrichment_privacy_scope_definition,
};
pub use owner_action::{
    CustomerEnrichmentPrivacyActionPlanner, CustomerEnrichmentPrivacyActionPolicy,
    OWNER_ACTION_CAPABILITY_ID, customer_enrichment_privacy_action_definition,
    customer_enrichment_privacy_action_planner,
};
pub use postgres::CustomerEnrichmentPrivacyScopeQueryAdapter;
