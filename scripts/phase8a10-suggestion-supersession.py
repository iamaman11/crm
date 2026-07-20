from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one anchor, found {count}: {old[:160]!r}")
    file.write_text(text.replace(old, new, 1))


lifecycle = "modules/crm-customer-enrichment/src/lifecycle.rs"
replace_once(
    lifecycle,
    "use sha2::{Digest, Sha256};\n",
    "use sha2::{Digest, Sha256};\nuse std::collections::BTreeMap;\n",
)
replace_once(
    lifecycle,
    '''#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewDecisionKind {
''',
    '''#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct SuggestionSupersessionCoordinate {
    provider_profile_version_id: ProviderProfileVersionId,
    mapping_version_id: MappingVersionId,
    resource_id: String,
    target_field: TargetField,
    proposed_value_digest: [u8; 32],
}

/// Derives deterministic one-to-one supersession among the supplied visible suggestions.
///
/// A suggestion can supersede another only when both represent the same logical proposition:
/// exact provider-profile and mapping versions, the same Party and target field, and the same
/// normalized proposed-value digest. Target resource version is intentionally excluded so a
/// refreshed proposition against a newer Party version can supersede its older evidence. The
/// newest provider retrieval wins; exact receipt and suggestion identities break timestamp ties.
pub fn derive_suggestion_supersession<'a>(
    suggestions: impl IntoIterator<Item = &'a Suggestion>,
) -> BTreeMap<SuggestionId, SuggestionId> {
    let suggestions = suggestions.into_iter().collect::<Vec<_>>();
    let mut latest = BTreeMap::<SuggestionSupersessionCoordinate, &Suggestion>::new();
    for suggestion in &suggestions {
        let coordinate = suggestion_supersession_coordinate(suggestion);
        let replace = latest
            .get(&coordinate)
            .is_none_or(|current| suggestion_supersession_order(suggestion) > suggestion_supersession_order(current));
        if replace {
            latest.insert(coordinate, suggestion);
        }
    }

    let mut output = BTreeMap::new();
    for suggestion in suggestions {
        let coordinate = suggestion_supersession_coordinate(suggestion);
        let successor = latest
            .get(&coordinate)
            .expect("every supplied suggestion must have a latest logical proposition");
        if successor.suggestion_id() != suggestion.suggestion_id() {
            output.insert(
                suggestion.suggestion_id().clone(),
                successor.suggestion_id().clone(),
            );
        }
    }
    output
}

fn suggestion_supersession_coordinate(
    suggestion: &Suggestion,
) -> SuggestionSupersessionCoordinate {
    SuggestionSupersessionCoordinate {
        provider_profile_version_id: suggestion.provider_profile_version_id.clone(),
        mapping_version_id: suggestion.mapping_version_id.clone(),
        resource_id: suggestion.target.resource_id.as_str().to_owned(),
        target_field: suggestion.target.target_field,
        proposed_value_digest: suggestion.proposed_value_digest,
    }
}

fn suggestion_supersession_order(
    suggestion: &Suggestion,
) -> (u64, &ProviderResponseReceiptId, &SuggestionId) {
    (
        suggestion.retrieved_at_unix_ms,
        &suggestion.response_receipt_id,
        &suggestion.suggestion_id,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewDecisionKind {
''',
)

lib = "modules/crm-customer-enrichment/src/lib.rs"
replace_once(
    lib,
    "    derive_suggestion_status,\n",
    "    derive_suggestion_status, derive_suggestion_supersession,\n",
)

review = "crates/crm-customer-enrichment-review-adapter/src/lib.rs"
replace_once(
    review,
    "    REVIEW_DECISION_STATE_SCHEMA_ID, ReviewDecision, ReviewDecisionKind, SUGGESTION_RECORD_TYPE,\n    SUGGESTION_STATE_MAXIMUM_BYTES, SUGGESTION_STATE_SCHEMA_ID, Suggestion,\n",
    "    REVIEW_DECISION_STATE_SCHEMA_ID, ReviewDecision, ReviewDecisionKind, SUGGESTION_RECORD_TYPE,\n    SUGGESTION_STATE_MAXIMUM_BYTES, SUGGESTION_STATE_SCHEMA_ID, Suggestion, SuggestionId,\n",
)
replace_once(
    review,
    '''pub fn suggestion_to_wire(
    suggestion: &Suggestion,
    latest_decision: Option<&ReviewDecision>,
    at_unix_ms: u64,
) -> Result<wire::Suggestion, SdkError> {
    let state: SuggestionStateView = serde_json::from_slice(&encode_suggestion_state(suggestion)?)
''',
    '''pub fn suggestion_to_wire(
    suggestion: &Suggestion,
    latest_decision: Option<&ReviewDecision>,
    at_unix_ms: u64,
) -> Result<wire::Suggestion, SdkError> {
    suggestion_to_wire_with_supersession(suggestion, latest_decision, None, at_unix_ms)
}

pub fn suggestion_to_wire_with_supersession(
    suggestion: &Suggestion,
    latest_decision: Option<&ReviewDecision>,
    superseded_by: Option<&SuggestionId>,
    at_unix_ms: u64,
) -> Result<wire::Suggestion, SdkError> {
    let state: SuggestionStateView = serde_json::from_slice(&encode_suggestion_state(suggestion)?)
''',
)
replace_once(
    review,
    '''            latest_decision,
            None,
            None,
            at_unix_ms,
        )),
        superseded_by_suggestion_ref: None,
''',
    '''            latest_decision,
            None,
            superseded_by,
            at_unix_ms,
        )),
        superseded_by_suggestion_ref: superseded_by.map(|suggestion_id| wire::SuggestionRef {
            suggestion_id: suggestion_id.as_str().to_owned(),
        }),
''',
)

query = "crates/crm-customer-enrichment-suggestion-query-adapter/src/lib.rs"
replace_once(
    query,
    '''use crm_customer_enrichment::{
    REVIEW_DECISION_RECORD_TYPE, ReviewDecision, SUGGESTION_RECORD_TYPE, Suggestion,
};
''',
    '''use crm_customer_enrichment::{
    REVIEW_DECISION_RECORD_TYPE, ReviewDecision, SUGGESTION_RECORD_TYPE, Suggestion, SuggestionId,
    derive_suggestion_supersession,
};
''',
)
replace_once(
    query,
    '''use crm_customer_enrichment_review_adapter::{
    review_decision_from_snapshot, review_decision_to_wire, suggestion_from_snapshot,
    suggestion_to_wire,
};
''',
    '''use crm_customer_enrichment_review_adapter::{
    review_decision_from_snapshot, review_decision_to_wire, suggestion_from_snapshot,
    suggestion_to_wire_with_supersession,
};
''',
)
replace_once(
    query,
    '''        let suggestion = suggestion_from_snapshot(&snapshot)?;
        self.ensure_party_visible(request, &suggestion).await?;
        let suggestion_visibility = self
            .visibility
            .authorize_visibility(request, &snapshot.reference)
            .await?;
        if !suggestion_visibility.resource_visible {
            return Err(suggestion_not_found());
        }

        let reviews = self.load_visible_latest_reviews(request).await?;
        let latest_review = reviews.get(suggestion.suggestion_id().as_str());
        let at_unix_ms = request_started_at_unix_ms(request)?;
        let mut public_suggestion = suggestion_to_wire(
            &suggestion,
            latest_review.map(|review| &review.decision),
            at_unix_ms,
        )?;
        redact_suggestion(&mut public_suggestion, |field| {
            suggestion_visibility.allows_field(field)
        });
''',
    '''        let suggestion = suggestion_from_snapshot(&snapshot)?;
        self.ensure_party_visible(request, &suggestion).await?;
        let visible_suggestions = self
            .load_visible_suggestions_for_party(
                request,
                suggestion.target().resource_id.as_str(),
            )
            .await?;
        let visible = visible_suggestions
            .get(suggestion.suggestion_id().as_str())
            .ok_or_else(suggestion_not_found)?;
        if visible.suggestion != suggestion {
            return Err(query_state_invalid(
                "direct suggestion lookup differs from bounded lifecycle scan",
            ));
        }

        let reviews = self.load_visible_latest_reviews(request).await?;
        let latest_review = reviews.get(suggestion.suggestion_id().as_str());
        let at_unix_ms = request_started_at_unix_ms(request)?;
        let mut public_suggestion = suggestion_to_wire_with_supersession(
            &suggestion,
            latest_review.map(|review| &review.decision),
            visible.superseded_by.as_ref(),
            at_unix_ms,
        )?;
        redact_suggestion(&mut public_suggestion, |field| {
            visible.visibility.allows_field(field)
        });
''',
)
replace_once(
    query,
    '''}

impl std::fmt::Debug for CustomerEnrichmentSuggestionQueryAdapter {
''',
    '''    pub(crate) async fn load_visible_suggestions_for_party(
        &self,
        request: &QueryRequest,
        party_id: &str,
    ) -> Result<BTreeMap<String, VisibleSuggestion>, SdkError> {
        let mut values = Vec::<(Suggestion, QueryVisibilityDecision)>::new();
        let mut after: Option<RecordQueryContinuation> = None;
        let mut scanned = 0_usize;
        loop {
            let page = self
                .store
                .list_records_for_query(&RecordListQuery {
                    tenant_id: request.context.tenant_id.clone(),
                    owner_module_id: module_id()?,
                    record_type: suggestion_record_type()?,
                    page_size: INTERNAL_SCAN_PAGE_SIZE,
                    sort: RecordQuerySort::UpdatedAtDescending,
                    after: after.clone(),
                })
                .await?;
            scanned = scanned.saturating_add(page.records.len());
            enforce_scan_limit(scanned)?;
            for snapshot in page.records {
                let suggestion = suggestion_from_snapshot(&snapshot)?;
                if suggestion.target().resource_id.as_str() != party_id {
                    continue;
                }
                let visibility = self
                    .visibility
                    .authorize_visibility(request, &snapshot.reference)
                    .await?;
                if visibility.resource_visible {
                    values.push((suggestion, visibility));
                }
            }
            after = page.next;
            if after.is_none() {
                break;
            }
        }

        let supersession = derive_suggestion_supersession(
            values.iter().map(|(suggestion, _)| suggestion),
        );
        let mut output = BTreeMap::new();
        for (suggestion, visibility) in values {
            let superseded_by = supersession.get(suggestion.suggestion_id()).cloned();
            output.insert(
                suggestion.suggestion_id().as_str().to_owned(),
                VisibleSuggestion {
                    suggestion,
                    visibility,
                    superseded_by,
                },
            );
        }
        Ok(output)
    }
}

impl std::fmt::Debug for CustomerEnrichmentSuggestionQueryAdapter {
''',
)
replace_once(
    query,
    '''#[derive(Clone)]
pub(crate) struct VisibleReview {
''',
    '''#[derive(Clone)]
pub(crate) struct VisibleSuggestion {
    pub(crate) suggestion: Suggestion,
    pub(crate) visibility: QueryVisibilityDecision,
    pub(crate) superseded_by: Option<SuggestionId>,
}

#[derive(Clone)]
pub(crate) struct VisibleReview {
''',
)

list_mod = "crates/crm-customer-enrichment-suggestion-query-adapter/src/list/mod.rs"
replace_once(
    list_mod,
    '''    let reviews = adapter.load_visible_latest_reviews(request).await?;
    let (items, next) = scan::collect(
''',
    '''    let visible_suggestions = adapter
        .load_visible_suggestions_for_party(request, party_id.as_str())
        .await?;
    let reviews = adapter.load_visible_latest_reviews(request).await?;
    let (items, next) = scan::collect(
''',
)
replace_once(
    list_mod,
    '''        after,
        &reviews,
    )
''',
    '''        after,
        &reviews,
        &visible_suggestions,
    )
''',
)

scan = "crates/crm-customer-enrichment-suggestion-query-adapter/src/list/scan.rs"
replace_once(
    scan,
    '''use crate::{
    CustomerEnrichmentSuggestionQueryAdapter, VisibleReview, enforce_scan_limit, module_id,
    query_configuration_invalid, request_started_at_unix_ms, suggestion_record_type,
};
''',
    '''use crate::{
    CustomerEnrichmentSuggestionQueryAdapter, VisibleReview, VisibleSuggestion, enforce_scan_limit,
    module_id, query_configuration_invalid, request_started_at_unix_ms, suggestion_record_type,
};
''',
)
replace_once(
    scan,
    '''use crm_customer_enrichment_review_adapter::{suggestion_from_snapshot, suggestion_to_wire};
''',
    '''use crm_customer_enrichment_review_adapter::suggestion_to_wire_with_supersession;
''',
)
replace_once(
    scan,
    '''    reviews: &BTreeMap<String, VisibleReview>,
) -> Result<(Vec<wire::Suggestion>, Option<RecordQueryContinuation>), SdkError> {
''',
    '''    reviews: &BTreeMap<String, VisibleReview>,
    visible_suggestions: &BTreeMap<String, VisibleSuggestion>,
) -> Result<(Vec<wire::Suggestion>, Option<RecordQueryContinuation>), SdkError> {
''',
)
replace_once(
    scan,
    '''                reviews,
                &mut scanned,
''',
    '''                reviews,
                visible_suggestions,
                &mut scanned,
''',
)
replace_once(
    scan,
    '''        for snapshot in &page.records {
            let suggestion = suggestion_from_snapshot(snapshot)?;
            let review = reviews.get(suggestion.suggestion_id().as_str());
            let mut public = suggestion_to_wire(
                &suggestion,
                review.map(|value| &value.decision),
                request_started_at_unix_ms(request)?,
            )?;
            if !matches(&public, party_id, profile_id, status) {
                continue;
            }
            let visibility = adapter
                .visibility
                .authorize_visibility(request, &snapshot.reference)
                .await?;
            if !visibility.resource_visible {
                continue;
            }
            crate::redact_suggestion(&mut public, |field| visibility.allows_field(field));
            output.push(public);
        }
''',
    '''        for snapshot in &page.records {
            let Some(visible) = visible_suggestions.get(snapshot.reference.record_id.as_str()) else {
                continue;
            };
            let suggestion = &visible.suggestion;
            let review = reviews.get(suggestion.suggestion_id().as_str());
            let mut public = suggestion_to_wire_with_supersession(
                suggestion,
                review.map(|value| &value.decision),
                visible.superseded_by.as_ref(),
                request_started_at_unix_ms(request)?,
            )?;
            if !matches(&public, party_id, profile_id, status) {
                continue;
            }
            crate::redact_suggestion(&mut public, |field| {
                visible.visibility.allows_field(field)
            });
            output.push(public);
        }
''',
)
replace_once(
    scan,
    '''    reviews: &BTreeMap<String, VisibleReview>,
    scanned: &mut usize,
''',
    '''    reviews: &BTreeMap<String, VisibleReview>,
    visible_suggestions: &BTreeMap<String, VisibleSuggestion>,
    scanned: &mut usize,
''',
)
replace_once(
    scan,
    '''        for snapshot in &page.records {
            let suggestion = suggestion_from_snapshot(snapshot)?;
            let review = reviews.get(suggestion.suggestion_id().as_str());
            let public = suggestion_to_wire(
                &suggestion,
                review.map(|value| &value.decision),
                request_started_at_unix_ms(request)?,
            )?;
            if matches(&public, party_id, profile_id, status)
                && adapter
                    .visibility
                    .authorize_visibility(request, &snapshot.reference)
                    .await?
                    .resource_visible
            {
                return Ok(true);
            }
        }
''',
    '''        for snapshot in &page.records {
            let Some(visible) = visible_suggestions.get(snapshot.reference.record_id.as_str()) else {
                continue;
            };
            let suggestion = &visible.suggestion;
            let review = reviews.get(suggestion.suggestion_id().as_str());
            let public = suggestion_to_wire_with_supersession(
                suggestion,
                review.map(|value| &value.decision),
                visible.superseded_by.as_ref(),
                request_started_at_unix_ms(request)?,
            )?;
            if matches(&public, party_id, profile_id, status) {
                return Ok(true);
            }
        }
''',
)

test = Path("modules/crm-customer-enrichment/tests/suggestion_supersession.rs")
test.parent.mkdir(parents=True, exist_ok=True)
test.write_text('''use crm_customer_enrichment::{
    EnrichmentRequest, EnrichmentRequestDraft, MappingDraft, MappingNormalization, MappingVersion,
    ProviderProfileDraft, ProviderProfileVersion, ProviderResponseClass, ProviderResponseReceipt,
    ProviderResponseReceiptDraft, RawPayloadPolicy, RequestPolicyEvidence, Suggestion,
    SuggestionDraft, TargetField, TargetSnapshot, derive_suggestion_supersession,
};
use crm_module_sdk::{ActorId, IdempotencyKey, TenantId};

#[test]
fn newer_exact_logical_proposition_supersedes_older_evidence_deterministically() {
    let older = suggestion("request-old", "replay-old", 200, "Acme Company");
    let newer = suggestion("request-new", "replay-new", 300, "Acme Company");
    let alternative = suggestion("request-alt", "replay-alt", 400, "Acme Holdings");

    let first = derive_suggestion_supersession([&alternative, &newer, &older]);
    let reordered = derive_suggestion_supersession([&older, &alternative, &newer]);

    assert_eq!(first, reordered);
    assert_eq!(
        first.get(older.suggestion_id()),
        Some(newer.suggestion_id())
    );
    assert!(!first.contains_key(newer.suggestion_id()));
    assert!(!first.contains_key(alternative.suggestion_id()));
}

fn suggestion(
    request_key: &str,
    replay_key: &str,
    retrieved_at_unix_ms: u64,
    proposed_value: &str,
) -> Suggestion {
    let (profile, mapping) = definitions();
    let request = EnrichmentRequest::create(EnrichmentRequestDraft {
        tenant_id: TenantId::try_new("tenant-a").unwrap(),
        requested_by: ActorId::try_new("worker-a").unwrap(),
        idempotency_key: IdempotencyKey::try_new(request_key).unwrap(),
        target: TargetSnapshot::try_new(
            "party-a",
            7,
            TargetField::PartyDisplayName,
        )
        .unwrap(),
        provider_profile_version_id: profile.version_id().clone(),
        mapping_version_id: mapping.version_id().clone(),
        requested_fields: vec![TargetField::PartyDisplayName],
        policy_evidence: RequestPolicyEvidence::try_new(
            "customer_profile_enrichment",
            "legitimate_interest",
            None,
            "policy-v1",
        )
        .unwrap(),
        created_at_unix_ms: 100,
        deadline_at_unix_ms: 1_000,
        expires_at_unix_ms: 2_000,
    })
    .unwrap();
    let receipt = ProviderResponseReceipt::record(ProviderResponseReceiptDraft {
        request_id: request.request_id().clone(),
        provider_profile_version_id: profile.version_id().clone(),
        mapping_version_id: mapping.version_id().clone(),
        replay_key: replay_key.to_owned(),
        provider_correlation_id: None,
        response_class: ProviderResponseClass::Success,
        canonical_response_digest: [7; 32],
        provider_observed_at_unix_ms: Some(retrieved_at_unix_ms - 1),
        retrieved_at_unix_ms,
        metered_units: 1,
        protected_evidence_reference: None,
    })
    .unwrap();
    Suggestion::materialize(SuggestionDraft {
        request_id: request.request_id().clone(),
        response_receipt_id: receipt.receipt_id().clone(),
        provider_profile_version_id: profile.version_id().clone(),
        mapping_version_id: mapping.version_id().clone(),
        target: request.target().clone(),
        proposed_value: proposed_value.to_owned(),
        observed_at_unix_ms: Some(retrieved_at_unix_ms - 1),
        retrieved_at_unix_ms,
        effective_at_unix_ms: retrieved_at_unix_ms,
        fresh_until_unix_ms: retrieved_at_unix_ms + 500,
        expires_at_unix_ms: retrieved_at_unix_ms + 1_000,
        confidence_basis_points: Some(9_000),
        purpose_code: "customer_profile_enrichment".to_owned(),
        legal_basis_code: "legitimate_interest".to_owned(),
        license_id: "Registry licence".to_owned(),
        permitted_use_class: "customer_master_review".to_owned(),
        residency_region: "eu".to_owned(),
        retention_days: 30,
        consent_evidence_reference: None,
        evidence_references: Vec::new(),
    })
    .unwrap()
}

fn definitions() -> (ProviderProfileVersion, MappingVersion) {
    let profile = ProviderProfileVersion::publish(ProviderProfileDraft {
        provider_key: "registry".to_owned(),
        adapter_kind: "registry_http_v1".to_owned(),
        adapter_contract_version: "1.0.0".to_owned(),
        supported_target_fields: vec![TargetField::PartyDisplayName],
        purpose_codes: vec!["customer_profile_enrichment".to_owned()],
        license_id: "Registry licence".to_owned(),
        permitted_use_class: "customer_master_review".to_owned(),
        residency_region: "eu".to_owned(),
        retention_days: 30,
        raw_payload_policy: RawPayloadPolicy::DigestOnly,
        credential_handle_aliases: vec!["registry_primary".to_owned()],
        effective_at_unix_ms: 1,
        expires_at_unix_ms: None,
    })
    .unwrap();
    let mapping = MappingVersion::publish(MappingDraft {
        mapping_key: "party_display_name".to_owned(),
        provider_profile_version_id: profile.version_id().clone(),
        provider_response_field_path: "organization.legal_name".to_owned(),
        target_field: TargetField::PartyDisplayName,
        normalization: MappingNormalization::CanonicalPartyDisplayNameV1,
        maximum_suggestions_per_response: 4,
        confidence_required: true,
    })
    .unwrap();
    (profile, mapping)
}
''')
