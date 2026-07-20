use crm_customer_enrichment::{
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
    assert_eq!(first.len(), 1);
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
        target: TargetSnapshot::try_new("party-a", 7, TargetField::PartyDisplayName).unwrap(),
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
