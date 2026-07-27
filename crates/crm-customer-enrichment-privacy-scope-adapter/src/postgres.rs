use crate::contract::{
    MAX_PRIVACY_APPLICATION_ATTEMPTS_SCANNED, MAX_PRIVACY_ASSOCIATION_RECORDS_REHYDRATED,
    MAX_PRIVACY_CANONICAL_PARTY_RESOLUTIONS, MAX_PRIVACY_DEFINITION_RECORDS_REHYDRATED,
    MAX_PRIVACY_OWNER_RECORDS_SCANNED, MAX_PRIVACY_PROVIDER_USAGE_ENTRIES_SCANNED,
    MAX_PRIVACY_REQUEST_RECORDS_REHYDRATED, MAX_PRIVACY_REQUEST_RELATIONSHIPS_SCANNED,
    MAX_PRIVACY_RESPONSE_CONFLICTS_SCANNED, MAX_PRIVACY_RESPONSE_RECEIPTS_SCANNED,
    MAX_PRIVACY_REVIEW_DECISIONS_SCANNED, MAX_PRIVACY_SUGGESTIONS_SCANNED,
    PRIVACY_OWNER_SCAN_BATCH_SIZE, PRIVACY_RELATIONSHIP_SCAN_BATCH_SIZE, validate_definition,
};
use crate::errors::{
    association_state_invalid, database_unavailable, limit_exceeded, lineage_invalid,
    map_canonical_party_claim_error, relationship_state_invalid, stored_state_invalid,
    topology_invalid,
};
use crate::request::{
    CursorState, ResourceFamily, ValidatedRequest, validate_request_contract, validate_wire_request,
};
use crate::response::{VerifiedCustomerEnrichmentResource, build_response, typed_output};
use crm_capability_plan_support::PersistedPayloadContract;
use crm_capability_runtime::CapabilityDefinition;
use crm_core_data::{BoundReadTransaction, PostgresDataStore};
use crm_customer_enrichment::{
    APPLICATION_ATTEMPT_RECORD_TYPE, APPLICATION_ATTEMPT_STATE_MAXIMUM_BYTES,
    APPLICATION_ATTEMPT_STATE_SCHEMA_ID, ApplicationAttempt, ENRICHMENT_REQUEST_RECORD_TYPE,
    LIFECYCLE_STATE_RETENTION_POLICY_ID, LIFECYCLE_STATE_SCHEMA_VERSION,
    MAPPING_VERSION_RECORD_TYPE, MappingVersion, PROVIDER_PROFILE_VERSION_RECORD_TYPE,
    PROVIDER_RESPONSE_CONFLICT_RECORD_TYPE, PROVIDER_RESPONSE_CONFLICT_STATE_RETENTION_POLICY_ID,
    PROVIDER_RESPONSE_RECEIPT_RECORD_TYPE, PROVIDER_RESPONSE_RECEIPT_STATE_MAXIMUM_BYTES,
    PROVIDER_RESPONSE_RECEIPT_STATE_SCHEMA_ID, PROVIDER_USAGE_ENTRY_RECORD_TYPE,
    PROVIDER_USAGE_ENTRY_STATE_MAXIMUM_BYTES, PROVIDER_USAGE_ENTRY_STATE_RETENTION_POLICY_ID,
    PROVIDER_USAGE_ENTRY_STATE_SCHEMA_ID, PROVIDER_USAGE_ENTRY_STATE_SCHEMA_VERSION,
    ProviderProfileVersion, ProviderResponseClass, ProviderResponseConflict,
    ProviderResponseReceipt, ProviderUsageEntry, ProviderUsageKind, REVIEW_DECISION_RECORD_TYPE,
    REVIEW_DECISION_STATE_MAXIMUM_BYTES, REVIEW_DECISION_STATE_SCHEMA_ID, ReviewDecision,
    SUGGESTION_RECORD_TYPE, SUGGESTION_STATE_MAXIMUM_BYTES, SUGGESTION_STATE_SCHEMA_ID, Suggestion,
    TargetField, TargetSnapshot, application_attempt_state_descriptor_hash,
    decode_application_attempt_state, decode_provider_response_conflict_state,
    decode_provider_response_receipt_state, decode_provider_usage_entry_state,
    encode_application_attempt_state, encode_provider_response_receipt_state,
    encode_provider_usage_entry_state, provider_response_receipt_state_descriptor_hash,
    provider_usage_entry_state_descriptor_hash,
};
use crm_customer_enrichment_application_adapter::{
    application_attempt_from_snapshot, application_attempt_persisted_contract,
    application_attempt_to_wire,
};
use crm_customer_enrichment_capability_adapter::{
    MODULE_ID, REQUEST_PARTY_RELATIONSHIP_TYPE, REQUEST_PARTY_SOURCE_RECORD_TYPE,
    enrichment_request_from_snapshot, enrichment_request_persisted_contract, mapping_from_snapshot,
    mapping_persisted_contract, provider_profile_from_snapshot,
    provider_profile_persisted_contract,
};
use crm_customer_enrichment_provider_process_composition::provider_response_conflict_persisted_contract;
use crm_customer_enrichment_review_adapter::{
    review_decision_from_snapshot, review_decision_persisted_contract, review_decision_to_wire,
    suggestion_from_snapshot, suggestion_persisted_contract, suggestion_to_wire,
};
use crm_customer_privacy_owner_scope_support::prove_canonical_party_claim;
use crm_identity_resolution::PartyReference;
use crm_identity_resolution_topology_composition::prove_canonical_party_in_transaction;
use crm_module_sdk::{
    DataClass, ErrorCategory, ModuleId, PayloadEncoding, PortFuture, RecordId, RecordRef,
    RecordSnapshot, RecordType, RetentionPolicyId, SchemaId, SchemaVersion, SdkError, TenantId,
    TypedPayload,
};
use crm_query_runtime::{QueryExecutionResult, QueryExecutor, QueryRequest};
use prost::Message;
use serde::Deserialize;
use sqlx::Row;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone)]
pub struct CustomerEnrichmentPrivacyScopeQueryAdapter {
    store: PostgresDataStore,
}

impl std::fmt::Debug for CustomerEnrichmentPrivacyScopeQueryAdapter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CustomerEnrichmentPrivacyScopeQueryAdapter")
            .field("store", &"PostgresDataStore")
            .finish()
    }
}

impl CustomerEnrichmentPrivacyScopeQueryAdapter {
    pub fn new(store: PostgresDataStore) -> Self {
        Self { store }
    }

    async fn execute_query(
        &self,
        definition: &CapabilityDefinition,
        request: QueryRequest,
    ) -> Result<QueryExecutionResult, SdkError> {
        validate_definition(definition)?;
        validate_request_contract(&request)?;
        let validated = validate_wire_request(&request.context, &request.input.bytes)?;

        let mut transaction = self
            .store
            .begin_bound_read_transaction(&request.context.tenant_id)
            .await?;
        prove_canonical_party_claim(
            &mut transaction,
            &request.context.tenant_id,
            &validated.canonical_party_id,
            validated.identity_resolution_generation,
        )
        .await
        .map_err(map_canonical_party_claim_error)?;

        let page =
            read_customer_enrichment_page(&mut transaction, &request.context.tenant_id, &validated)
                .await?;
        let response = build_response(
            &validated,
            &page.resources,
            page.scanned_resource_count,
            page.next_state.as_ref(),
        )?;
        let output = typed_output(response.encode_to_vec())?;
        transaction.commit().await.map_err(database_unavailable)?;
        Ok(QueryExecutionResult { output })
    }
}

impl QueryExecutor for CustomerEnrichmentPrivacyScopeQueryAdapter {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: QueryRequest,
    ) -> PortFuture<'a, Result<QueryExecutionResult, SdkError>> {
        Box::pin(async move { self.execute_query(definition, request).await })
    }
}

struct CustomerEnrichmentPage {
    resources: Vec<VerifiedCustomerEnrichmentResource>,
    scanned_resource_count: u64,
    next_state: Option<CursorState>,
}

struct Versioned<T> {
    record_id: RecordId,
    version: u64,
    value: T,
}

struct ReceiptRecord {
    versioned: Versioned<ProviderResponseReceipt>,
    view: ProviderResponseReceiptView,
}

struct SuggestionRecord {
    versioned: Versioned<Suggestion>,
    view: crm_proto_contracts::crm::customer_enrichment::v1::Suggestion,
}

struct ReviewRecord {
    versioned: Versioned<ReviewDecision>,
    view: crm_proto_contracts::crm::customer_enrichment::v1::ReviewDecision,
}

struct ApplicationRecord {
    versioned: Versioned<ApplicationAttempt>,
    view: crm_proto_contracts::crm::customer_enrichment::v1::ApplicationAttempt,
    tenant_id: String,
}

struct UsageRecord {
    versioned: Versioned<ProviderUsageEntry>,
    view: ProviderUsageEntryView,
}

struct DefinitionCatalog {
    profiles: BTreeMap<String, ProviderProfileVersion>,
    mappings: BTreeMap<String, MappingVersion>,
}

struct DirectRecords {
    requests: BTreeMap<String, Versioned<crm_customer_enrichment::EnrichmentRequest>>,
    receipts: BTreeMap<String, ReceiptRecord>,
    conflicts: BTreeMap<String, Versioned<ProviderResponseConflict>>,
    suggestions: BTreeMap<String, SuggestionRecord>,
    reviews: BTreeMap<String, ReviewRecord>,
    applications: BTreeMap<String, ApplicationRecord>,
    usage_entries: BTreeMap<String, UsageRecord>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderResponseReceiptView {
    receipt_id: String,
    request_id: String,
    provider_profile_version_id: String,
    mapping_version_id: String,
    replay_key: String,
    provider_correlation_id: Option<String>,
    response_class: ProviderResponseClass,
    canonical_response_digest: [u8; 32],
    provider_observed_at_unix_ms: Option<u64>,
    retrieved_at_unix_ms: u64,
    metered_units: u64,
    protected_evidence_reference: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderUsageEntryView {
    usage_entry_id: String,
    request_id: String,
    response_receipt_id: Option<String>,
    provider_profile_version_id: String,
    kind: ProviderUsageKind,
    metered_units: u64,
    quota_bucket: Option<String>,
    quota_remaining: Option<u64>,
    provider_observed_at_unix_ms: Option<u64>,
    recorded_at_unix_ms: u64,
    safe_provider_code: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApplicationTenantView {
    tenant_id: String,
}

struct CanonicalResolutionCache {
    values: BTreeMap<String, bool>,
    examined: usize,
}

impl CanonicalResolutionCache {
    fn new() -> Self {
        Self {
            values: BTreeMap::new(),
            examined: 0,
        }
    }

    async fn resolves_to_subject(
        &mut self,
        transaction: &mut BoundReadTransaction<'_>,
        tenant_id: &TenantId,
        party_id: &str,
        request: &ValidatedRequest,
    ) -> Result<bool, SdkError> {
        if let Some(relevant) = self.values.get(party_id) {
            return Ok(*relevant);
        }
        self.examined = self
            .examined
            .checked_add(1)
            .ok_or_else(|| limit_exceeded("canonical Party resolution counter overflowed"))?;
        if self.examined > MAX_PRIVACY_CANONICAL_PARTY_RESOLUTIONS {
            return Err(limit_exceeded(
                "canonical Party resolution count exceeded the frozen privacy bound",
            ));
        }
        let requested = PartyReference::try_new(party_id).map_err(|error| {
            topology_invalid(format!(
                "persisted request Party reference is invalid: {error}"
            ))
        })?;
        let canonical =
            PartyReference::try_new(request.canonical_party_id.as_str()).map_err(|error| {
                topology_invalid(format!(
                    "accepted canonical Party reference is invalid: {error}"
                ))
            })?;
        let relevant = match prove_canonical_party_in_transaction(
            transaction,
            tenant_id,
            &requested,
            &canonical,
            request.identity_resolution_generation,
        )
        .await
        {
            Ok(_) => true,
            Err(error) if error.code == "IDENTITY_RESOLUTION_CANONICAL_PARTY_MISMATCH" => false,
            Err(error) if error.code == "IDENTITY_RESOLUTION_TOPOLOGY_GENERATION_STALE" => {
                return Err(lineage_invalid(
                    ErrorCategory::Conflict,
                    true,
                    "Identity Resolution topology generation changed during Enrichment scope discovery",
                ));
            }
            Err(error) => {
                return Err(topology_invalid(format!(
                    "{}: {}",
                    error.code, error.safe_message
                )));
            }
        };
        self.values.insert(party_id.to_owned(), relevant);
        Ok(relevant)
    }
}

struct AssociationCounter {
    examined: usize,
}

impl AssociationCounter {
    fn new() -> Self {
        Self { examined: 0 }
    }

    fn charge(&mut self, reference: &'static str) -> Result<(), SdkError> {
        self.examined = self
            .examined
            .checked_add(1)
            .ok_or_else(|| limit_exceeded("association rehydration counter overflowed"))?;
        if self.examined > MAX_PRIVACY_ASSOCIATION_RECORDS_REHYDRATED {
            return Err(limit_exceeded(format!(
                "association rehydration exceeded the frozen privacy bound while resolving {reference}"
            )));
        }
        Ok(())
    }
}

async fn read_customer_enrichment_page(
    transaction: &mut BoundReadTransaction<'_>,
    tenant_id: &TenantId,
    request: &ValidatedRequest,
) -> Result<CustomerEnrichmentPage, SdkError> {
    let relationship_rows = load_request_relationship_rows(transaction, tenant_id.as_str()).await?;
    let profile_rows = load_record_rows(
        transaction,
        tenant_id.as_str(),
        PROVIDER_PROFILE_VERSION_RECORD_TYPE,
        MAX_PRIVACY_DEFINITION_RECORDS_REHYDRATED,
    )
    .await?;
    let mapping_rows = load_record_rows(
        transaction,
        tenant_id.as_str(),
        MAPPING_VERSION_RECORD_TYPE,
        MAX_PRIVACY_DEFINITION_RECORDS_REHYDRATED,
    )
    .await?;
    let definition_count = profile_rows
        .len()
        .checked_add(mapping_rows.len())
        .ok_or_else(|| limit_exceeded("definition rehydration counter overflowed"))?;
    if definition_count > MAX_PRIVACY_DEFINITION_RECORDS_REHYDRATED {
        return Err(limit_exceeded(
            "Customer Enrichment definition rehydration exceeded the frozen privacy bound",
        ));
    }

    let request_rows = load_record_rows(
        transaction,
        tenant_id.as_str(),
        ENRICHMENT_REQUEST_RECORD_TYPE,
        MAX_PRIVACY_REQUEST_RECORDS_REHYDRATED,
    )
    .await?;
    let receipt_rows = load_record_rows(
        transaction,
        tenant_id.as_str(),
        PROVIDER_RESPONSE_RECEIPT_RECORD_TYPE,
        MAX_PRIVACY_RESPONSE_RECEIPTS_SCANNED,
    )
    .await?;
    let conflict_rows = load_record_rows(
        transaction,
        tenant_id.as_str(),
        PROVIDER_RESPONSE_CONFLICT_RECORD_TYPE,
        MAX_PRIVACY_RESPONSE_CONFLICTS_SCANNED,
    )
    .await?;
    let suggestion_rows = load_record_rows(
        transaction,
        tenant_id.as_str(),
        SUGGESTION_RECORD_TYPE,
        MAX_PRIVACY_SUGGESTIONS_SCANNED,
    )
    .await?;
    let review_rows = load_record_rows(
        transaction,
        tenant_id.as_str(),
        REVIEW_DECISION_RECORD_TYPE,
        MAX_PRIVACY_REVIEW_DECISIONS_SCANNED,
    )
    .await?;
    let application_rows = load_record_rows(
        transaction,
        tenant_id.as_str(),
        APPLICATION_ATTEMPT_RECORD_TYPE,
        MAX_PRIVACY_APPLICATION_ATTEMPTS_SCANNED,
    )
    .await?;
    let usage_rows = load_record_rows(
        transaction,
        tenant_id.as_str(),
        PROVIDER_USAGE_ENTRY_RECORD_TYPE,
        MAX_PRIVACY_PROVIDER_USAGE_ENTRIES_SCANNED,
    )
    .await?;

    let owner_scanned = [
        request_rows.len(),
        receipt_rows.len(),
        conflict_rows.len(),
        suggestion_rows.len(),
        review_rows.len(),
        application_rows.len(),
        usage_rows.len(),
    ]
    .into_iter()
    .try_fold(0_usize, |total, count| total.checked_add(count))
    .ok_or_else(|| limit_exceeded("owner record scan count overflowed"))?;
    if owner_scanned > MAX_PRIVACY_OWNER_RECORDS_SCANNED {
        return Err(limit_exceeded(
            "Customer Enrichment owner record scan exceeded the frozen privacy bound",
        ));
    }

    let definitions = strict_definitions(profile_rows, mapping_rows)?;
    let records = DirectRecords {
        requests: strict_requests(request_rows)?,
        receipts: strict_receipts(receipt_rows)?,
        conflicts: strict_conflicts(conflict_rows)?,
        suggestions: strict_suggestions(suggestion_rows)?,
        reviews: strict_reviews(review_rows)?,
        applications: strict_applications(application_rows)?,
        usage_entries: strict_usage_entries(usage_rows)?,
    };

    let mut resolution_cache = CanonicalResolutionCache::new();
    let relationships = validate_relationships(
        transaction,
        tenant_id,
        request,
        &mut resolution_cache,
        relationship_rows,
    )
    .await?;
    let relevant_request_ids =
        validate_associations(tenant_id, &definitions, &records, &relationships)?;

    let mut ordered = Vec::new();
    append_request_resources(&mut ordered, &records.requests, &relevant_request_ids);
    append_receipt_resources(&mut ordered, &records.receipts, &relevant_request_ids);
    append_conflict_resources(&mut ordered, &records.conflicts, &relevant_request_ids);
    append_suggestion_resources(&mut ordered, &records.suggestions, &relevant_request_ids);
    append_review_resources(
        &mut ordered,
        &records.reviews,
        &records.suggestions,
        &relevant_request_ids,
    )?;
    append_application_resources(
        &mut ordered,
        &records.applications,
        &records.suggestions,
        &relevant_request_ids,
    )?;
    append_usage_resources(&mut ordered, &records.usage_entries, &relevant_request_ids);

    let mut matching = ordered
        .into_iter()
        .filter(|resource| resource_after_cursor(resource, &request.cursor_state))
        .take(request.page_size as usize + 1)
        .collect::<Vec<_>>();
    let has_more = matching.len() > request.page_size as usize;
    if has_more {
        matching.pop();
    }
    let next_state = if has_more {
        let last = matching.last().ok_or_else(|| {
            association_state_invalid("Customer Enrichment page continuation has no anchor")
        })?;
        Some(CursorState {
            family: last.family,
            after_record_id: Some(last.record_id.clone()),
        })
    } else {
        None
    };
    let scanned = owner_scanned
        .checked_add(relationships.raw_count)
        .ok_or_else(|| limit_exceeded("combined scan count overflowed"))?;
    Ok(CustomerEnrichmentPage {
        resources: matching,
        scanned_resource_count: u64::try_from(scanned)
            .map_err(|_| limit_exceeded("combined scan count does not fit in u64"))?,
        next_state,
    })
}

struct RelationshipCatalog {
    by_request: BTreeMap<String, String>,
    relevant_requests: BTreeSet<String>,
    raw_count: usize,
}

async fn validate_relationships(
    transaction: &mut BoundReadTransaction<'_>,
    tenant_id: &TenantId,
    request: &ValidatedRequest,
    resolution_cache: &mut CanonicalResolutionCache,
    rows: Vec<RequestRelationshipRow>,
) -> Result<RelationshipCatalog, SdkError> {
    let raw_count = rows.len();
    let mut by_request = BTreeMap::new();
    let mut relevant_requests = BTreeSet::new();
    for row in rows {
        if row.version != 1 || row.attributes_json != "{}" {
            return Err(relationship_state_invalid(
                "request/Party relationship metadata is not the canonical empty v1 link",
            ));
        }
        let source_party_id = RecordId::try_new(row.source_record_id)
            .map_err(|error| relationship_state_invalid(error.to_string()))?;
        let target_request_id = RecordId::try_new(row.target_record_id)
            .map_err(|error| relationship_state_invalid(error.to_string()))?;
        if by_request
            .insert(
                target_request_id.as_str().to_owned(),
                source_party_id.as_str().to_owned(),
            )
            .is_some()
        {
            return Err(relationship_state_invalid(
                "enrichment request has more than one authoritative Party relationship",
            ));
        }
        if resolution_cache
            .resolves_to_subject(transaction, tenant_id, source_party_id.as_str(), request)
            .await?
        {
            relevant_requests.insert(target_request_id.as_str().to_owned());
        }
    }
    Ok(RelationshipCatalog {
        by_request,
        relevant_requests,
        raw_count,
    })
}

fn validate_associations(
    tenant_id: &TenantId,
    definitions: &DefinitionCatalog,
    records: &DirectRecords,
    relationships: &RelationshipCatalog,
) -> Result<BTreeSet<String>, SdkError> {
    let mut counter = AssociationCounter::new();

    for (request_id, request) in &records.requests {
        counter.charge("request/Party relationship")?;
        let relationship_party_id = relationships.by_request.get(request_id).ok_or_else(|| {
            relationship_state_invalid("enrichment request is missing its authoritative Party link")
        })?;
        if request.value.tenant_id() != tenant_id
            || request.value.target().resource_type() != REQUEST_PARTY_SOURCE_RECORD_TYPE
            || request.value.target().target_field != TargetField::PartyDisplayName
            || request.value.target().resource_id != *relationship_party_id
        {
            return Err(relationship_state_invalid(
                "enrichment request target disagrees with its authoritative Party link",
            ));
        }
        counter.charge("request provider profile")?;
        let profile = definitions
            .profiles
            .get(request.value.provider_profile_version_id().as_str())
            .ok_or_else(|| association_state_invalid("request provider profile is missing"))?;
        counter.charge("request mapping")?;
        let mapping = definitions
            .mappings
            .get(request.value.mapping_version_id().as_str())
            .ok_or_else(|| association_state_invalid("request mapping is missing"))?;
        if mapping.provider_profile_version_id() != profile.version_id()
            || mapping.target_field() != request.value.target().target_field
            || !profile
                .supported_target_fields()
                .contains(&request.value.target().target_field)
        {
            return Err(association_state_invalid(
                "request provider-profile and mapping bindings are inconsistent",
            ));
        }
    }
    for request_id in relationships.by_request.keys() {
        if !records.requests.contains_key(request_id) {
            return Err(relationship_state_invalid(
                "request/Party relationship targets a missing enrichment request",
            ));
        }
    }

    let mut receipt_by_request = BTreeMap::<String, String>::new();
    for (receipt_id, receipt) in &records.receipts {
        counter.charge("receipt parent request")?;
        let parent = records
            .requests
            .get(&receipt.view.request_id)
            .ok_or_else(|| {
                association_state_invalid("provider response receipt references a missing request")
            })?;
        if receipt.view.receipt_id != *receipt_id
            || receipt.view.provider_profile_version_id
                != parent.value.provider_profile_version_id().as_str()
            || receipt.view.mapping_version_id != parent.value.mapping_version_id().as_str()
            || parent
                .value
                .response_receipt_id()
                .is_none_or(|value| value.as_str() != receipt_id)
        {
            return Err(association_state_invalid(
                "provider response receipt disagrees with its exact request lineage",
            ));
        }
        if receipt_by_request
            .insert(receipt.view.request_id.clone(), receipt_id.clone())
            .is_some()
        {
            return Err(association_state_invalid(
                "enrichment request has more than one authoritative response receipt",
            ));
        }
    }

    for conflict in records.conflicts.values() {
        counter.charge("conflict parent request")?;
        let request = records
            .requests
            .get(conflict.value.request_id().as_str())
            .ok_or_else(|| {
                association_state_invalid("provider response conflict references a missing request")
            })?;
        counter.charge("conflict first receipt")?;
        let receipt = records
            .receipts
            .get(conflict.value.first_receipt_id().as_str())
            .ok_or_else(|| {
                association_state_invalid("provider response conflict first receipt is missing")
            })?;
        if conflict.value.tenant_id() != tenant_id
            || receipt.view.request_id != request.value.request_id().as_str()
            || conflict.value.retry_generation() > request.value.retry_generation()
            || conflict.value.detected_at_unix_ms() < receipt.view.retrieved_at_unix_ms
        {
            return Err(association_state_invalid(
                "provider response conflict disagrees with request/receipt lineage",
            ));
        }
    }

    for suggestion in records.suggestions.values() {
        let request_id = suggestion_request_id(&suggestion.view)?;
        let receipt_id = suggestion_receipt_id(&suggestion.view)?;
        counter.charge("suggestion parent request")?;
        let request = records.requests.get(request_id).ok_or_else(|| {
            association_state_invalid("suggestion references a missing enrichment request")
        })?;
        counter.charge("suggestion response receipt")?;
        let receipt = records.receipts.get(receipt_id).ok_or_else(|| {
            association_state_invalid("suggestion references a missing response receipt")
        })?;
        let target = suggestion.view.target.as_ref().ok_or_else(|| {
            association_state_invalid("suggestion target is absent from strict wire view")
        })?;
        let party_id = target
            .party_ref
            .as_ref()
            .ok_or_else(|| association_state_invalid("suggestion Party target is missing"))?
            .party_id
            .as_str();
        let profile_id = suggestion
            .view
            .provider_profile_version_ref
            .as_ref()
            .ok_or_else(|| association_state_invalid("suggestion provider profile is missing"))?
            .provider_profile_version_id
            .as_str();
        let mapping_id = suggestion
            .view
            .mapping_version_ref
            .as_ref()
            .ok_or_else(|| association_state_invalid("suggestion mapping is missing"))?
            .mapping_version_id
            .as_str();
        if receipt.view.request_id != request_id
            || receipt_id != receipt.view.receipt_id
            || profile_id != request.value.provider_profile_version_id().as_str()
            || mapping_id != request.value.mapping_version_id().as_str()
            || party_id != request.value.target().resource_id
            || target.party_resource_version
                != i64::try_from(request.value.target().resource_version)
                    .map_err(|_| association_state_invalid("request target version exceeds i64"))?
            || target.target_field
                != crm_proto_contracts::crm::customer_enrichment::v1::EnrichmentTargetField::PartyDisplayName
                    as i32
        {
            return Err(association_state_invalid(
                "suggestion disagrees with request, receipt or target lineage",
            ));
        }
    }

    for review in records.reviews.values() {
        let suggestion_id = review
            .view
            .suggestion_ref
            .as_ref()
            .ok_or_else(|| association_state_invalid("review suggestion reference is missing"))?
            .suggestion_id
            .as_str();
        counter.charge("review suggestion")?;
        let suggestion = records.suggestions.get(suggestion_id).ok_or_else(|| {
            association_state_invalid("review decision references a missing suggestion")
        })?;
        let target =
            suggestion.view.target.as_ref().ok_or_else(|| {
                association_state_invalid("reviewed suggestion target is missing")
            })?;
        if review.view.target_party_resource_version != target.party_resource_version
            || review.view.proposed_value_digest != suggestion.view.proposed_value_digest
        {
            return Err(association_state_invalid(
                "review decision disagrees with exact suggestion value/target lineage",
            ));
        }
    }

    for application in records.applications.values() {
        if application.tenant_id != tenant_id.as_str() {
            return Err(association_state_invalid(
                "application attempt tenant differs from the bounded transaction",
            ));
        }
        let suggestion_id = application
            .view
            .suggestion_ref
            .as_ref()
            .ok_or_else(|| {
                association_state_invalid("application suggestion reference is missing")
            })?
            .suggestion_id
            .as_str();
        let review_id = application
            .view
            .review_decision_ref
            .as_ref()
            .ok_or_else(|| association_state_invalid("application review reference is missing"))?
            .review_decision_id
            .as_str();
        counter.charge("application suggestion")?;
        let suggestion = records.suggestions.get(suggestion_id).ok_or_else(|| {
            association_state_invalid("application attempt references a missing suggestion")
        })?;
        counter.charge("application review")?;
        let review = records.reviews.get(review_id).ok_or_else(|| {
            association_state_invalid("application attempt references a missing review")
        })?;
        let review_suggestion_id = review
            .view
            .suggestion_ref
            .as_ref()
            .ok_or_else(|| association_state_invalid("application review suggestion is missing"))?
            .suggestion_id
            .as_str();
        let target = application.view.target.as_ref().ok_or_else(|| {
            association_state_invalid("application target is missing from strict wire view")
        })?;
        if review_suggestion_id != suggestion_id
            || review.view.kind
                != crm_proto_contracts::crm::customer_enrichment::v1::SuggestionReviewDecisionKind::Accepted
                    as i32
            || application.view.proposed_value_digest != suggestion.view.proposed_value_digest
            || target != suggestion.view.target.as_ref().ok_or_else(|| {
                association_state_invalid("applied suggestion target is missing")
            })?
        {
            return Err(association_state_invalid(
                "application attempt disagrees with suggestion/review lineage",
            ));
        }
    }

    for usage in records.usage_entries.values() {
        counter.charge("usage parent request")?;
        let request = records
            .requests
            .get(&usage.view.request_id)
            .ok_or_else(|| {
                association_state_invalid("provider usage references a missing request")
            })?;
        if usage.view.provider_profile_version_id
            != request.value.provider_profile_version_id().as_str()
        {
            return Err(association_state_invalid(
                "provider usage profile differs from its request",
            ));
        }
        if let Some(receipt_id) = &usage.view.response_receipt_id {
            counter.charge("usage response receipt")?;
            let receipt = records
                .receipts
                .get(receipt_id)
                .ok_or_else(|| association_state_invalid("provider usage receipt is missing"))?;
            if receipt.view.request_id != usage.view.request_id
                || usage.view.recorded_at_unix_ms < receipt.view.retrieved_at_unix_ms
            {
                return Err(association_state_invalid(
                    "provider usage disagrees with request/receipt lineage",
                ));
            }
        }
    }

    Ok(relationships.relevant_requests.clone())
}

fn strict_definitions(
    profile_rows: Vec<StoredRecordRow>,
    mapping_rows: Vec<StoredRecordRow>,
) -> Result<DefinitionCatalog, SdkError> {
    let mut profiles = BTreeMap::new();
    for row in profile_rows {
        let snapshot = strict_snapshot(
            &row,
            PROVIDER_PROFILE_VERSION_RECORD_TYPE,
            provider_profile_persisted_contract(),
            DataClass::Confidential,
        )?;
        let profile = provider_profile_from_snapshot(&snapshot).map_err(map_owner_error)?;
        let id = profile.version_id().as_str().to_owned();
        if profiles.insert(id, profile).is_some() {
            return Err(stored_state_invalid(
                "duplicate strict provider-profile identity",
            ));
        }
    }
    let mut mappings = BTreeMap::new();
    for row in mapping_rows {
        let snapshot = strict_snapshot(
            &row,
            MAPPING_VERSION_RECORD_TYPE,
            mapping_persisted_contract(),
            DataClass::Confidential,
        )?;
        let mapping = mapping_from_snapshot(&snapshot).map_err(map_owner_error)?;
        if !profiles.contains_key(mapping.provider_profile_version_id().as_str()) {
            return Err(association_state_invalid(
                "mapping references a missing strict provider profile",
            ));
        }
        let id = mapping.version_id().as_str().to_owned();
        if mappings.insert(id, mapping).is_some() {
            return Err(stored_state_invalid("duplicate strict mapping identity"));
        }
    }
    Ok(DefinitionCatalog { profiles, mappings })
}

fn strict_requests(
    rows: Vec<StoredRecordRow>,
) -> Result<BTreeMap<String, Versioned<crm_customer_enrichment::EnrichmentRequest>>, SdkError> {
    rows.into_iter()
        .map(|row| {
            let snapshot = strict_snapshot(
                &row,
                ENRICHMENT_REQUEST_RECORD_TYPE,
                enrichment_request_persisted_contract(),
                DataClass::Personal,
            )?;
            let value = enrichment_request_from_snapshot(&snapshot).map_err(map_owner_error)?;
            Ok((row.record_id.as_str().to_owned(), versioned(row, value)?))
        })
        .collect()
}

fn strict_receipts(
    rows: Vec<StoredRecordRow>,
) -> Result<BTreeMap<String, ReceiptRecord>, SdkError> {
    rows.into_iter()
        .map(|row| {
            let snapshot = strict_snapshot(
                &row,
                PROVIDER_RESPONSE_RECEIPT_RECORD_TYPE,
                provider_response_receipt_persisted_contract(),
                DataClass::Personal,
            )?;
            if snapshot.version != 1 {
                return Err(stored_state_invalid(
                    "provider response receipt immutable version is invalid",
                ));
            }
            let value = decode_provider_response_receipt_state(&snapshot.payload.bytes)
                .map_err(map_owner_error)?;
            if value.receipt_id().as_str() != row.record_id.as_str() {
                return Err(stored_state_invalid(
                    "provider response receipt identity differs from its record",
                ));
            }
            let view: ProviderResponseReceiptView = serde_json::from_slice(
                &encode_provider_response_receipt_state(&value).map_err(map_owner_error)?,
            )
            .map_err(|error| stored_state_invalid(error.to_string()))?;
            let id = row.record_id.as_str().to_owned();
            Ok((
                id,
                ReceiptRecord {
                    versioned: versioned(row, value)?,
                    view,
                },
            ))
        })
        .collect()
}

fn strict_conflicts(
    rows: Vec<StoredRecordRow>,
) -> Result<BTreeMap<String, Versioned<ProviderResponseConflict>>, SdkError> {
    rows.into_iter()
        .map(|row| {
            let snapshot = strict_snapshot(
                &row,
                PROVIDER_RESPONSE_CONFLICT_RECORD_TYPE,
                provider_response_conflict_persisted_contract(),
                DataClass::Confidential,
            )?;
            let value = decode_provider_response_conflict_state(&snapshot.payload.bytes)
                .map_err(map_owner_error)?;
            if value.conflict_id().as_str() != row.record_id.as_str()
                || (snapshot.version == 1) != value.resolution().is_none()
                || !(1..=2).contains(&snapshot.version)
            {
                return Err(stored_state_invalid(
                    "provider response conflict identity/version is invalid",
                ));
            }
            Ok((row.record_id.as_str().to_owned(), versioned(row, value)?))
        })
        .collect()
}

fn strict_suggestions(
    rows: Vec<StoredRecordRow>,
) -> Result<BTreeMap<String, SuggestionRecord>, SdkError> {
    rows.into_iter()
        .map(|row| {
            let snapshot = strict_snapshot(
                &row,
                SUGGESTION_RECORD_TYPE,
                suggestion_persisted_contract(),
                DataClass::Personal,
            )?;
            let value = suggestion_from_snapshot(&snapshot).map_err(map_owner_error)?;
            let view = suggestion_to_wire(&value, None, 0).map_err(map_owner_error)?;
            let id = row.record_id.as_str().to_owned();
            Ok((
                id,
                SuggestionRecord {
                    versioned: versioned(row, value)?,
                    view,
                },
            ))
        })
        .collect()
}

fn strict_reviews(rows: Vec<StoredRecordRow>) -> Result<BTreeMap<String, ReviewRecord>, SdkError> {
    rows.into_iter()
        .map(|row| {
            let snapshot = strict_snapshot(
                &row,
                REVIEW_DECISION_RECORD_TYPE,
                review_decision_persisted_contract(),
                DataClass::Personal,
            )?;
            let value = review_decision_from_snapshot(&snapshot).map_err(map_owner_error)?;
            let view = review_decision_to_wire(&value).map_err(map_owner_error)?;
            let id = row.record_id.as_str().to_owned();
            Ok((
                id,
                ReviewRecord {
                    versioned: versioned(row, value)?,
                    view,
                },
            ))
        })
        .collect()
}

fn strict_applications(
    rows: Vec<StoredRecordRow>,
) -> Result<BTreeMap<String, ApplicationRecord>, SdkError> {
    rows.into_iter()
        .map(|row| {
            let snapshot = strict_snapshot(
                &row,
                APPLICATION_ATTEMPT_RECORD_TYPE,
                application_attempt_persisted_contract(),
                DataClass::Personal,
            )?;
            let value = application_attempt_from_snapshot(&snapshot).map_err(map_owner_error)?;
            let view = application_attempt_to_wire(&value).map_err(map_owner_error)?;
            let tenant_view: ApplicationTenantView = serde_json::from_slice(
                &encode_application_attempt_state(&value).map_err(map_owner_error)?,
            )
            .map_err(|error| stored_state_invalid(error.to_string()))?;
            let id = row.record_id.as_str().to_owned();
            Ok((
                id,
                ApplicationRecord {
                    versioned: versioned(row, value)?,
                    view,
                    tenant_id: tenant_view.tenant_id,
                },
            ))
        })
        .collect()
}

fn strict_usage_entries(
    rows: Vec<StoredRecordRow>,
) -> Result<BTreeMap<String, UsageRecord>, SdkError> {
    rows.into_iter()
        .map(|row| {
            let snapshot = strict_snapshot(
                &row,
                PROVIDER_USAGE_ENTRY_RECORD_TYPE,
                provider_usage_entry_persisted_contract(),
                DataClass::Confidential,
            )?;
            if snapshot.version != 1 {
                return Err(stored_state_invalid(
                    "provider usage immutable version is invalid",
                ));
            }
            let value = decode_provider_usage_entry_state(&snapshot.payload.bytes)
                .map_err(map_owner_error)?;
            if value.usage_entry_id().as_str() != row.record_id.as_str() {
                return Err(stored_state_invalid(
                    "provider usage identity differs from its record",
                ));
            }
            let view: ProviderUsageEntryView = serde_json::from_slice(
                &encode_provider_usage_entry_state(&value).map_err(map_owner_error)?,
            )
            .map_err(|error| stored_state_invalid(error.to_string()))?;
            let id = row.record_id.as_str().to_owned();
            Ok((
                id,
                UsageRecord {
                    versioned: versioned(row, value)?,
                    view,
                },
            ))
        })
        .collect()
}

fn append_request_resources(
    output: &mut Vec<VerifiedCustomerEnrichmentResource>,
    records: &BTreeMap<String, Versioned<crm_customer_enrichment::EnrichmentRequest>>,
    relevant: &BTreeSet<String>,
) {
    for (id, record) in records {
        if relevant.contains(id) {
            output.push(VerifiedCustomerEnrichmentResource {
                family: ResourceFamily::Request,
                record_id: record.record_id.clone(),
                resource_version: record.version,
            });
        }
    }
}

fn append_receipt_resources(
    output: &mut Vec<VerifiedCustomerEnrichmentResource>,
    records: &BTreeMap<String, ReceiptRecord>,
    relevant: &BTreeSet<String>,
) {
    for record in records.values() {
        if relevant.contains(&record.view.request_id) {
            output.push(VerifiedCustomerEnrichmentResource {
                family: ResourceFamily::ResponseReceipt,
                record_id: record.versioned.record_id.clone(),
                resource_version: record.versioned.version,
            });
        }
    }
}

fn append_conflict_resources(
    output: &mut Vec<VerifiedCustomerEnrichmentResource>,
    records: &BTreeMap<String, Versioned<ProviderResponseConflict>>,
    relevant: &BTreeSet<String>,
) {
    for record in records.values() {
        if relevant.contains(record.value.request_id().as_str()) {
            output.push(VerifiedCustomerEnrichmentResource {
                family: ResourceFamily::ResponseConflict,
                record_id: record.record_id.clone(),
                resource_version: record.version,
            });
        }
    }
}

fn append_suggestion_resources(
    output: &mut Vec<VerifiedCustomerEnrichmentResource>,
    records: &BTreeMap<String, SuggestionRecord>,
    relevant: &BTreeSet<String>,
) {
    for record in records.values() {
        if suggestion_request_id(&record.view).is_ok_and(|id| relevant.contains(id)) {
            output.push(VerifiedCustomerEnrichmentResource {
                family: ResourceFamily::Suggestion,
                record_id: record.versioned.record_id.clone(),
                resource_version: record.versioned.version,
            });
        }
    }
}

fn append_review_resources(
    output: &mut Vec<VerifiedCustomerEnrichmentResource>,
    records: &BTreeMap<String, ReviewRecord>,
    suggestions: &BTreeMap<String, SuggestionRecord>,
    relevant: &BTreeSet<String>,
) -> Result<(), SdkError> {
    for record in records.values() {
        let suggestion_id = record
            .view
            .suggestion_ref
            .as_ref()
            .ok_or_else(|| association_state_invalid("review suggestion reference is missing"))?
            .suggestion_id
            .as_str();
        let suggestion = suggestions.get(suggestion_id).ok_or_else(|| {
            association_state_invalid("review suggestion is missing during response assembly")
        })?;
        if relevant.contains(suggestion_request_id(&suggestion.view)?) {
            output.push(VerifiedCustomerEnrichmentResource {
                family: ResourceFamily::ReviewDecision,
                record_id: record.versioned.record_id.clone(),
                resource_version: record.versioned.version,
            });
        }
    }
    Ok(())
}

fn append_application_resources(
    output: &mut Vec<VerifiedCustomerEnrichmentResource>,
    records: &BTreeMap<String, ApplicationRecord>,
    suggestions: &BTreeMap<String, SuggestionRecord>,
    relevant: &BTreeSet<String>,
) -> Result<(), SdkError> {
    for record in records.values() {
        let suggestion_id = record
            .view
            .suggestion_ref
            .as_ref()
            .ok_or_else(|| {
                association_state_invalid("application suggestion reference is missing")
            })?
            .suggestion_id
            .as_str();
        let suggestion = suggestions.get(suggestion_id).ok_or_else(|| {
            association_state_invalid("application suggestion is missing during response assembly")
        })?;
        if relevant.contains(suggestion_request_id(&suggestion.view)?) {
            output.push(VerifiedCustomerEnrichmentResource {
                family: ResourceFamily::ApplicationAttempt,
                record_id: record.versioned.record_id.clone(),
                resource_version: record.versioned.version,
            });
        }
    }
    Ok(())
}

fn append_usage_resources(
    output: &mut Vec<VerifiedCustomerEnrichmentResource>,
    records: &BTreeMap<String, UsageRecord>,
    relevant: &BTreeSet<String>,
) {
    for record in records.values() {
        if relevant.contains(&record.view.request_id) {
            output.push(VerifiedCustomerEnrichmentResource {
                family: ResourceFamily::ProviderUsageEntry,
                record_id: record.versioned.record_id.clone(),
                resource_version: record.versioned.version,
            });
        }
    }
}

fn suggestion_request_id(
    value: &crm_proto_contracts::crm::customer_enrichment::v1::Suggestion,
) -> Result<&str, SdkError> {
    Ok(value
        .enrichment_request_ref
        .as_ref()
        .ok_or_else(|| association_state_invalid("suggestion request reference is missing"))?
        .enrichment_request_id
        .as_str())
}

fn suggestion_receipt_id(
    value: &crm_proto_contracts::crm::customer_enrichment::v1::Suggestion,
) -> Result<&str, SdkError> {
    Ok(value
        .provider_response_receipt_ref
        .as_ref()
        .ok_or_else(|| association_state_invalid("suggestion receipt reference is missing"))?
        .provider_response_receipt_id
        .as_str())
}

fn resource_after_cursor(
    resource: &VerifiedCustomerEnrichmentResource,
    cursor: &CursorState,
) -> bool {
    match resource.family.cmp(&cursor.family) {
        std::cmp::Ordering::Less => false,
        std::cmp::Ordering::Greater => true,
        std::cmp::Ordering::Equal => cursor
            .after_record_id
            .as_ref()
            .is_none_or(|after| resource.record_id.as_str() > after.as_str()),
    }
}

fn provider_response_receipt_persisted_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: PROVIDER_RESPONSE_RECEIPT_STATE_SCHEMA_ID,
        schema_version: LIFECYCLE_STATE_SCHEMA_VERSION,
        descriptor_hash: provider_response_receipt_state_descriptor_hash(),
        maximum_size_bytes: PROVIDER_RESPONSE_RECEIPT_STATE_MAXIMUM_BYTES,
        retention_policy_id: LIFECYCLE_STATE_RETENTION_POLICY_ID,
    }
}

fn provider_usage_entry_persisted_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: PROVIDER_USAGE_ENTRY_STATE_SCHEMA_ID,
        schema_version: PROVIDER_USAGE_ENTRY_STATE_SCHEMA_VERSION,
        descriptor_hash: provider_usage_entry_state_descriptor_hash(),
        maximum_size_bytes: PROVIDER_USAGE_ENTRY_STATE_MAXIMUM_BYTES,
        retention_policy_id: PROVIDER_USAGE_ENTRY_STATE_RETENTION_POLICY_ID,
    }
}

fn versioned<T>(row: StoredRecordRow, value: T) -> Result<Versioned<T>, SdkError> {
    Ok(Versioned {
        record_id: row.record_id,
        version: positive_version(row.version)?,
        value,
    })
}

fn map_owner_error(error: SdkError) -> SdkError {
    stored_state_invalid(format!("{}: {}", error.code, error.safe_message))
}

struct RequestRelationshipRow {
    source_record_id: String,
    target_record_id: String,
    version: i64,
    attributes_json: String,
}

async fn load_request_relationship_rows(
    transaction: &mut BoundReadTransaction<'_>,
    tenant_id: &str,
) -> Result<Vec<RequestRelationshipRow>, SdkError> {
    let mut after_source = String::new();
    let mut after_target = String::new();
    let mut output = Vec::new();
    loop {
        let rows = sqlx::query(
            r#"
            SELECT source_record_id, target_record_id, version, attributes::text AS attributes_json
            FROM crm.relationships
            WHERE tenant_id = $1
              AND relationship_type = $2
              AND source_record_type = $3
              AND target_record_type = $4
              AND (source_record_id, target_record_id) > ($5, $6)
            ORDER BY source_record_id ASC, target_record_id ASC
            LIMIT $7
            "#,
        )
        .bind(tenant_id)
        .bind(REQUEST_PARTY_RELATIONSHIP_TYPE)
        .bind(REQUEST_PARTY_SOURCE_RECORD_TYPE)
        .bind(ENRICHMENT_REQUEST_RECORD_TYPE)
        .bind(&after_source)
        .bind(&after_target)
        .bind(PRIVACY_RELATIONSHIP_SCAN_BATCH_SIZE)
        .fetch_all(&mut ***transaction)
        .await
        .map_err(database_unavailable)?;
        if rows.is_empty() {
            break;
        }
        let batch_len = rows.len();
        for row in rows {
            let decoded = RequestRelationshipRow {
                source_record_id: row
                    .try_get("source_record_id")
                    .map_err(|error| relationship_state_invalid(error.to_string()))?,
                target_record_id: row
                    .try_get("target_record_id")
                    .map_err(|error| relationship_state_invalid(error.to_string()))?,
                version: row
                    .try_get("version")
                    .map_err(|error| relationship_state_invalid(error.to_string()))?,
                attributes_json: row
                    .try_get("attributes_json")
                    .map_err(|error| relationship_state_invalid(error.to_string()))?,
            };
            after_source = decoded.source_record_id.clone();
            after_target = decoded.target_record_id.clone();
            output.push(decoded);
            if output.len() > MAX_PRIVACY_REQUEST_RELATIONSHIPS_SCANNED {
                return Err(limit_exceeded(
                    "request/Party relationship scan exceeded the frozen privacy bound",
                ));
            }
        }
        if batch_len < PRIVACY_RELATIONSHIP_SCAN_BATCH_SIZE as usize {
            break;
        }
    }
    Ok(output)
}

struct StoredRecordRow {
    record_id: RecordId,
    version: i64,
    owner_module_id: String,
    schema_id: String,
    schema_version: String,
    descriptor_hash: Vec<u8>,
    data_class: String,
    payload_encoding: String,
    maximum_payload_size: i64,
    retention_policy_id: String,
    payload_bytes: Vec<u8>,
}

async fn load_record_rows(
    transaction: &mut BoundReadTransaction<'_>,
    tenant_id: &str,
    record_type: &str,
    maximum: usize,
) -> Result<Vec<StoredRecordRow>, SdkError> {
    let mut after_record_id = String::new();
    let mut output = Vec::new();
    loop {
        let rows = sqlx::query(
            r#"
            SELECT
              record_id,
              version,
              owner_module_id,
              schema_id,
              schema_version,
              descriptor_hash,
              data_class,
              payload_encoding,
              maximum_payload_size,
              retention_policy_id,
              payload_bytes
            FROM crm.records
            WHERE tenant_id = $1
              AND owner_module_id = $2
              AND record_type = $3
              AND record_id > $4
              AND deleted_at IS NULL
            ORDER BY record_id ASC
            LIMIT $5
            "#,
        )
        .bind(tenant_id)
        .bind(MODULE_ID)
        .bind(record_type)
        .bind(&after_record_id)
        .bind(PRIVACY_OWNER_SCAN_BATCH_SIZE)
        .fetch_all(&mut ***transaction)
        .await
        .map_err(database_unavailable)?;
        if rows.is_empty() {
            break;
        }
        let batch_len = rows.len();
        for row in rows {
            let decoded = decode_stored_row(row)?;
            after_record_id = decoded.record_id.as_str().to_owned();
            output.push(decoded);
            if output.len() > maximum {
                return Err(limit_exceeded(format!(
                    "{record_type} scan exceeded the frozen privacy bound"
                )));
            }
        }
        if batch_len < PRIVACY_OWNER_SCAN_BATCH_SIZE as usize {
            break;
        }
    }
    Ok(output)
}

fn decode_stored_row(row: sqlx::postgres::PgRow) -> Result<StoredRecordRow, SdkError> {
    let invalid = |reference: String| stored_state_invalid(reference);
    Ok(StoredRecordRow {
        record_id: RecordId::try_new(
            row.try_get::<String, _>("record_id")
                .map_err(|error| invalid(error.to_string()))?,
        )
        .map_err(|error| invalid(error.to_string()))?,
        version: row
            .try_get("version")
            .map_err(|error| invalid(error.to_string()))?,
        owner_module_id: row
            .try_get("owner_module_id")
            .map_err(|error| invalid(error.to_string()))?,
        schema_id: row
            .try_get("schema_id")
            .map_err(|error| invalid(error.to_string()))?,
        schema_version: row
            .try_get("schema_version")
            .map_err(|error| invalid(error.to_string()))?,
        descriptor_hash: row
            .try_get("descriptor_hash")
            .map_err(|error| invalid(error.to_string()))?,
        data_class: row
            .try_get("data_class")
            .map_err(|error| invalid(error.to_string()))?,
        payload_encoding: row
            .try_get("payload_encoding")
            .map_err(|error| invalid(error.to_string()))?,
        maximum_payload_size: row
            .try_get("maximum_payload_size")
            .map_err(|error| invalid(error.to_string()))?,
        retention_policy_id: row
            .try_get("retention_policy_id")
            .map_err(|error| invalid(error.to_string()))?,
        payload_bytes: row
            .try_get("payload_bytes")
            .map_err(|error| invalid(error.to_string()))?,
    })
}

fn strict_snapshot(
    row: &StoredRecordRow,
    record_type: &str,
    contract: PersistedPayloadContract<'_>,
    data_class: DataClass,
) -> Result<RecordSnapshot, SdkError> {
    let expected_data_class = match data_class {
        DataClass::Confidential => "confidential",
        DataClass::Personal => "personal",
        _ => {
            return Err(stored_state_invalid(
                "unsupported Customer Enrichment persisted data class",
            ));
        }
    };
    if row.version <= 0
        || row.owner_module_id != contract.owner
        || row.schema_id != contract.schema_id
        || row.schema_version != contract.schema_version
        || row.descriptor_hash.as_slice() != contract.descriptor_hash
        || row.data_class != expected_data_class
        || row.payload_encoding != "json"
        || row.maximum_payload_size != contract.maximum_size_bytes as i64
        || row.retention_policy_id != contract.retention_policy_id
    {
        return Err(stored_state_invalid(
            "persisted metadata does not match the canonical Customer Enrichment state contract",
        ));
    }
    let snapshot = RecordSnapshot {
        reference: RecordRef {
            record_type: RecordType::try_new(record_type)
                .map_err(|error| stored_state_invalid(error.to_string()))?,
            record_id: row.record_id.clone(),
        },
        version: row.version,
        payload: TypedPayload {
            owner: ModuleId::try_new(contract.owner)
                .map_err(|error| stored_state_invalid(error.to_string()))?,
            schema_id: SchemaId::try_new(contract.schema_id)
                .map_err(|error| stored_state_invalid(error.to_string()))?,
            schema_version: SchemaVersion::try_new(contract.schema_version)
                .map_err(|error| stored_state_invalid(error.to_string()))?,
            descriptor_hash: contract.descriptor_hash,
            data_class,
            encoding: PayloadEncoding::Json,
            maximum_size_bytes: contract.maximum_size_bytes,
            retention_policy_id: RetentionPolicyId::try_new(contract.retention_policy_id)
                .map_err(|error| stored_state_invalid(error.to_string()))?,
            bytes: row.payload_bytes.clone(),
        },
    };
    snapshot
        .payload
        .validate()
        .map_err(|error| stored_state_invalid(error.to_string()))?;
    Ok(snapshot)
}

fn positive_version(version: i64) -> Result<u64, SdkError> {
    u64::try_from(version).map_err(|_| stored_state_invalid("resource version must be positive"))
}
