use crm_proto_contracts::crm::{
    customer::v1 as customer, customer_enrichment::v1 as enrichment,
};
use crm_proto_contracts::message_descriptor_hash;
use prost::Message;

#[test]
fn provider_profile_and_mapping_contracts_round_trip_with_exact_versions() {
    let profile = enrichment::ProviderProfileVersion {
        provider_profile_version_ref: Some(enrichment::ProviderProfileVersionRef {
            provider_profile_version_id: "enrichment-provider-profile-example".to_owned(),
        }),
        definition: Some(enrichment::ProviderProfileDefinition {
            provider_key: "company_registry".to_owned(),
            adapter_kind: "registry_http_v1".to_owned(),
            adapter_contract_version: "1.0.0".to_owned(),
            supported_target_fields: vec![
                enrichment::EnrichmentTargetField::PartyDisplayName as i32,
            ],
            purpose_codes: vec!["customer_profile_enrichment".to_owned()],
            license_id: "Registry licence v3".to_owned(),
            permitted_use_class: "customer_master_review".to_owned(),
            residency_region: "eu".to_owned(),
            retention_days: 30,
            raw_payload_policy: enrichment::RawProviderPayloadPolicy::DigestOnly as i32,
            credential_handle_aliases: vec!["registry_primary".to_owned()],
            effective_at_unix_ms: 100,
            expires_at_unix_ms: Some(1_000),
        }),
    };
    let mapping = enrichment::MappingVersion {
        mapping_version_ref: Some(enrichment::MappingVersionRef {
            mapping_version_id: "enrichment-mapping-example".to_owned(),
        }),
        definition: Some(enrichment::MappingDefinition {
            mapping_key: "party_display_name".to_owned(),
            provider_profile_version_ref: profile.provider_profile_version_ref.clone(),
            provider_response_field_path: "organization.legal_name".to_owned(),
            target_field: enrichment::EnrichmentTargetField::PartyDisplayName as i32,
            normalization: enrichment::MappingNormalization::CanonicalPartyDisplayNameV1 as i32,
            maximum_suggestions_per_response: 1,
            confidence_required: true,
        }),
    };

    assert_eq!(
        enrichment::ProviderProfileVersion::decode(profile.encode_to_vec().as_slice()).unwrap(),
        profile
    );
    assert_eq!(
        enrichment::MappingVersion::decode(mapping.encode_to_vec().as_slice()).unwrap(),
        mapping
    );
}

#[test]
fn request_contract_binds_exact_party_snapshot_and_policy_evidence() {
    let request = enrichment::CreateEnrichmentRequestRequest {
        target: Some(enrichment::EnrichmentTargetSnapshot {
            party_ref: Some(customer::PartyRef {
                party_id: "party-123".to_owned(),
            }),
            party_resource_version: 7,
            target_field: enrichment::EnrichmentTargetField::PartyDisplayName as i32,
        }),
        provider_profile_version_ref: Some(enrichment::ProviderProfileVersionRef {
            provider_profile_version_id: "enrichment-provider-profile-example".to_owned(),
        }),
        mapping_version_ref: Some(enrichment::MappingVersionRef {
            mapping_version_id: "enrichment-mapping-example".to_owned(),
        }),
        requested_fields: vec![enrichment::EnrichmentTargetField::PartyDisplayName as i32],
        policy_evidence: Some(enrichment::EnrichmentRequestPolicyEvidence {
            purpose_code: "customer_profile_enrichment".to_owned(),
            legal_basis_code: "legitimate_interest".to_owned(),
            consent_evidence_reference: Some("consent-proof-42".to_owned()),
            policy_version: "1.0.0".to_owned(),
        }),
        deadline_at_unix_ms: 500,
        expires_at_unix_ms: 1_000,
    };

    assert_eq!(
        enrichment::CreateEnrichmentRequestRequest::decode(request.encode_to_vec().as_slice())
            .unwrap(),
        request
    );
}

#[test]
fn suggestion_application_contract_preserves_review_and_target_idempotency_evidence() {
    let attempt = enrichment::ApplicationAttempt {
        application_attempt_ref: Some(enrichment::ApplicationAttemptRef {
            application_attempt_id: "enrichment-application-example".to_owned(),
        }),
        suggestion_ref: Some(enrichment::SuggestionRef {
            suggestion_id: "enrichment-suggestion-example".to_owned(),
        }),
        review_decision_ref: Some(enrichment::ReviewDecisionRef {
            review_decision_id: "enrichment-review-example".to_owned(),
        }),
        target: Some(enrichment::EnrichmentTargetSnapshot {
            party_ref: Some(customer::PartyRef {
                party_id: "party-123".to_owned(),
            }),
            party_resource_version: 7,
            target_field: enrichment::EnrichmentTargetField::PartyDisplayName as i32,
        }),
        proposed_value_digest: vec![7; 32],
        application_generation: 0,
        owner_capability_id: "parties.party.update".to_owned(),
        owner_capability_version: "1.0.0".to_owned(),
        target_idempotency_key: "customer-enrichment-apply-example".to_owned(),
        planned_at_unix_ms: 400,
        recorded_outcome: Some(enrichment::RecordedApplicationOutcome {
            outcome: Some(enrichment::ApplicationOutcome {
                result: Some(enrichment::application_outcome::Result::Succeeded(
                    enrichment::ApplicationSucceeded {
                        business_transaction_id: "party-update-tx-42".to_owned(),
                        resulting_party_resource_version: 8,
                    },
                )),
            }),
            recorded_at_unix_ms: 450,
        }),
    };

    assert_eq!(
        enrichment::ApplicationAttempt::decode(attempt.encode_to_vec().as_slice()).unwrap(),
        attempt
    );
}

#[test]
fn internal_response_contract_contains_only_bounded_canonical_evidence() {
    let request = enrichment::RecordProviderResponseRequest {
        enrichment_request_ref: Some(enrichment::EnrichmentRequestRef {
            enrichment_request_id: "enrichment-request-example".to_owned(),
        }),
        replay_key: "provider-request-42".to_owned(),
        provider_correlation_id: Some("provider-correlation-42".to_owned()),
        response_class: enrichment::ProviderResponseClass::Success as i32,
        canonical_response_digest: vec![9; 32],
        provider_observed_at_unix_ms: Some(190),
        retrieved_at_unix_ms: 200,
        metered_units: 3,
        protected_evidence_reference: Some("governed-evidence-42".to_owned()),
        safe_provider_code: Some("success".to_owned()),
    };

    assert_eq!(
        enrichment::RecordProviderResponseRequest::decode(request.encode_to_vec().as_slice())
            .unwrap(),
        request
    );
}

#[test]
fn customer_enrichment_descriptor_identities_are_stable_and_distinct() {
    let profile = message_descriptor_hash("crm.customer_enrichment.v1.ProviderProfileDefinition");
    let request =
        message_descriptor_hash("crm.customer_enrichment.v1.CreateEnrichmentRequestRequest");
    let suggestion = message_descriptor_hash("crm.customer_enrichment.v1.Suggestion");

    assert_eq!(
        profile,
        message_descriptor_hash("crm.customer_enrichment.v1.ProviderProfileDefinition")
    );
    assert_ne!(profile, request);
    assert_ne!(request, suggestion);
}
