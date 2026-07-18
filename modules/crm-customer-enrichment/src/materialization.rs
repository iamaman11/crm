use crate::{
    EnrichmentRequest, EnrichmentRequestStatus, MappingVersion, ProviderProfileVersion,
    ProviderResponseClass, ProviderResponseReceipt, Suggestion, SuggestionDraft, TargetSnapshot,
};
use crm_module_sdk::{ErrorCategory, SdkError};
use serde::Deserialize;
use std::collections::BTreeSet;

/// One provider-neutral candidate after infrastructure has parsed and sanitized the raw response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuggestionCandidateDraft {
    pub target: TargetSnapshot,
    pub proposed_value: String,
    pub observed_at_unix_ms: Option<u64>,
    pub effective_at_unix_ms: u64,
    pub fresh_until_unix_ms: u64,
    pub expires_at_unix_ms: u64,
    pub confidence_basis_points: Option<u16>,
    pub license_id: String,
    pub permitted_use_class: String,
    pub residency_region: String,
    pub retention_days: u32,
    pub consent_evidence_reference: Option<String>,
    pub evidence_references: Vec<String>,
}

/// Validates exact immutable lineage and atomically advances a response-recorded request.
///
/// The response receipt, mapping and provider profile are immutable snapshots loaded by the worker.
/// The request is the only mutable aggregate. It is not changed until every candidate has been
/// validated and materialized successfully.
pub fn materialize_suggestions(
    request: &mut EnrichmentRequest,
    receipt: &ProviderResponseReceipt,
    profile: &ProviderProfileVersion,
    mapping: &MappingVersion,
    candidates: Vec<SuggestionCandidateDraft>,
    materialized_at_unix_ms: u64,
) -> Result<Vec<Suggestion>, SdkError> {
    let request_state = request_state(request)?;
    let receipt_state = receipt_state(receipt)?;

    validate_lineage(
        request,
        &request_state,
        receipt,
        &receipt_state,
        profile,
        mapping,
        materialized_at_unix_ms,
    )?;
    validate_candidate_count(receipt.response_class(), mapping, candidates.len())?;

    let protected_reference = receipt_state.protected_evidence_reference.as_deref();
    let mut suggestions = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        validate_candidate(
            request,
            &request_state,
            profile,
            mapping,
            &candidate,
            protected_reference,
        )?;
        suggestions.push(Suggestion::materialize(SuggestionDraft {
            request_id: request.request_id().clone(),
            response_receipt_id: receipt.receipt_id().clone(),
            provider_profile_version_id: request.provider_profile_version_id().clone(),
            mapping_version_id: request.mapping_version_id().clone(),
            target: candidate.target,
            proposed_value: candidate.proposed_value,
            observed_at_unix_ms: candidate.observed_at_unix_ms,
            retrieved_at_unix_ms: receipt_state.retrieved_at_unix_ms,
            effective_at_unix_ms: candidate.effective_at_unix_ms,
            fresh_until_unix_ms: candidate.fresh_until_unix_ms,
            expires_at_unix_ms: candidate.expires_at_unix_ms,
            confidence_basis_points: candidate.confidence_basis_points,
            purpose_code: request_state.policy_evidence.purpose_code.clone(),
            legal_basis_code: request_state.policy_evidence.legal_basis_code.clone(),
            license_id: candidate.license_id,
            permitted_use_class: candidate.permitted_use_class,
            residency_region: candidate.residency_region,
            retention_days: candidate.retention_days,
            consent_evidence_reference: candidate.consent_evidence_reference,
            evidence_references: candidate.evidence_references,
        })?);
    }

    suggestions.sort_by(|left, right| left.suggestion_id().cmp(right.suggestion_id()));
    if suggestions
        .windows(2)
        .any(|pair| pair[0].suggestion_id() == pair[1].suggestion_id())
    {
        return Err(materialization_conflict(
            "CUSTOMER_ENRICHMENT_SUGGESTION_DUPLICATE",
            "the same deterministic suggestion was supplied more than once",
        ));
    }

    request.mark_suggestions_materialized(materialized_at_unix_ms)?;
    Ok(suggestions)
}

fn validate_lineage(
    request: &EnrichmentRequest,
    request_state: &RequestStateView,
    receipt: &ProviderResponseReceipt,
    receipt_state: &ReceiptStateView,
    profile: &ProviderProfileVersion,
    mapping: &MappingVersion,
    materialized_at_unix_ms: u64,
) -> Result<(), SdkError> {
    if request.status() != EnrichmentRequestStatus::ResponseRecorded {
        return Err(materialization_conflict(
            "CUSTOMER_ENRICHMENT_MATERIALIZATION_STATUS_CONFLICT",
            "only a response-recorded enrichment request can materialize suggestions",
        ));
    }
    if request.response_receipt_id() != Some(receipt.receipt_id())
        || receipt.request_id() != request.request_id()
        || receipt_state.provider_profile_version_id
            != request.provider_profile_version_id().as_str()
        || receipt_state.mapping_version_id != request.mapping_version_id().as_str()
        || profile.version_id() != request.provider_profile_version_id()
        || mapping.version_id() != request.mapping_version_id()
        || mapping.provider_profile_version_id() != profile.version_id()
    {
        return Err(materialization_conflict(
            "CUSTOMER_ENRICHMENT_MATERIALIZATION_LINEAGE_CONFLICT",
            "request, response receipt, mapping and provider profile do not share exact lineage",
        ));
    }
    if mapping.target_field() != request.target().target_field
        || !profile
            .supported_target_fields()
            .contains(&request.target().target_field)
    {
        return Err(materialization_conflict(
            "CUSTOMER_ENRICHMENT_MATERIALIZATION_TARGET_CONFLICT",
            "mapping or provider profile does not support the exact request target field",
        ));
    }
    if !profile
        .purpose_codes()
        .iter()
        .any(|purpose| purpose == &request_state.policy_evidence.purpose_code)
    {
        return Err(materialization_conflict(
            "CUSTOMER_ENRICHMENT_MATERIALIZATION_PURPOSE_CONFLICT",
            "request purpose is not allowed by the exact provider profile",
        ));
    }
    if !profile.is_effective_at(receipt_state.retrieved_at_unix_ms) {
        return Err(materialization_conflict(
            "CUSTOMER_ENRICHMENT_MATERIALIZATION_PROFILE_WINDOW_CONFLICT",
            "provider profile was not effective at the exact response retrieval time",
        ));
    }
    if materialized_at_unix_ms < receipt_state.retrieved_at_unix_ms
        || materialized_at_unix_ms < request_state.updated_at_unix_ms
    {
        return Err(SdkError::invalid_argument(
            "customer_enrichment.materialized_at_unix_ms",
            "materialization time must not precede the response or request state",
        ));
    }
    Ok(())
}

fn validate_candidate_count(
    response_class: ProviderResponseClass,
    mapping: &MappingVersion,
    count: usize,
) -> Result<(), SdkError> {
    match response_class {
        ProviderResponseClass::Success if count == 0 => Err(materialization_conflict(
            "CUSTOMER_ENRICHMENT_SUCCESS_RESPONSE_EMPTY",
            "a successful provider response must materialize at least one suggestion",
        )),
        ProviderResponseClass::NoMatch if count != 0 => Err(materialization_conflict(
            "CUSTOMER_ENRICHMENT_NO_MATCH_HAS_SUGGESTIONS",
            "a no-match provider response cannot materialize suggestions",
        )),
        ProviderResponseClass::RetryableFailure | ProviderResponseClass::TerminalFailure => {
            Err(materialization_conflict(
                "CUSTOMER_ENRICHMENT_FAILED_RESPONSE_NOT_MATERIALIZABLE",
                "a failed provider response cannot materialize suggestions",
            ))
        }
        _ if count > mapping.maximum_suggestions_per_response() as usize => {
            Err(SdkError::invalid_argument(
                "customer_enrichment.candidates",
                "candidate count exceeds the exact mapping limit",
            ))
        }
        _ => Ok(()),
    }
}

fn validate_candidate(
    request: &EnrichmentRequest,
    request_state: &RequestStateView,
    profile: &ProviderProfileVersion,
    mapping: &MappingVersion,
    candidate: &SuggestionCandidateDraft,
    protected_reference: Option<&str>,
) -> Result<(), SdkError> {
    if &candidate.target != request.target()
        || candidate.target.target_field != mapping.target_field()
    {
        return Err(materialization_conflict(
            "CUSTOMER_ENRICHMENT_CANDIDATE_TARGET_CONFLICT",
            "candidate target does not match the exact request snapshot and mapping field",
        ));
    }
    if mapping.confidence_required() && candidate.confidence_basis_points.is_none() {
        return Err(SdkError::invalid_argument(
            "customer_enrichment.candidates.confidence_basis_points",
            "confidence is required by the exact mapping version",
        ));
    }
    if candidate.license_id != profile.license_id()
        || candidate.permitted_use_class != profile.permitted_use_class()
        || candidate.residency_region != profile.residency_region()
        || candidate.retention_days != profile.retention_days()
        || candidate.consent_evidence_reference
            != request_state.policy_evidence.consent_evidence_reference
    {
        return Err(materialization_conflict(
            "CUSTOMER_ENRICHMENT_CANDIDATE_POLICY_CONFLICT",
            "candidate policy evidence does not match exact request and provider-profile evidence",
        ));
    }
    if protected_reference.is_some_and(|reference| {
        !candidate
            .evidence_references
            .iter()
            .any(|candidate_reference| candidate_reference == reference)
    }) {
        return Err(materialization_conflict(
            "CUSTOMER_ENRICHMENT_PROTECTED_EVIDENCE_MISSING",
            "candidate does not reference the governed protected provider evidence",
        ));
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct RequestStateView {
    policy_evidence: RequestPolicyEvidenceView,
    updated_at_unix_ms: u64,
}

#[derive(Debug, Deserialize)]
struct RequestPolicyEvidenceView {
    purpose_code: String,
    legal_basis_code: String,
    consent_evidence_reference: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ReceiptStateView {
    provider_profile_version_id: String,
    mapping_version_id: String,
    retrieved_at_unix_ms: u64,
    protected_evidence_reference: Option<String>,
}

fn request_state(request: &EnrichmentRequest) -> Result<RequestStateView, SdkError> {
    serde_json::from_value(serde_json::to_value(request).map_err(materialization_internal)?)
        .map_err(materialization_internal)
}

fn receipt_state(receipt: &ProviderResponseReceipt) -> Result<ReceiptStateView, SdkError> {
    serde_json::from_value(serde_json::to_value(receipt).map_err(materialization_internal)?)
        .map_err(materialization_internal)
}

fn materialization_internal(error: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_MATERIALIZATION_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "Customer Enrichment materialization state is invalid.",
    )
    .with_internal_reference(error.to_string())
}

fn materialization_conflict(code: &'static str, message: &'static str) -> SdkError {
    SdkError::new(code, ErrorCategory::Conflict, false, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        EnrichmentRequestDraft, MappingDraft, MappingNormalization, ProviderProfileDraft,
        ProviderResponseReceiptDraft, RawPayloadPolicy, RequestPolicyEvidence, TargetField,
    };
    use crm_module_sdk::{ActorId, IdempotencyKey, TenantId};

    #[test]
    fn success_materializes_deterministically_and_advances_request_once() {
        let (mut request, receipt, profile, mapping) =
            fixture(ProviderResponseClass::Success, true);
        let second = candidate(&request, "Zeta Company", Some(8_000));
        let first = candidate(&request, "Alpha Company", Some(9_000));

        let suggestions = materialize_suggestions(
            &mut request,
            &receipt,
            &profile,
            &mapping,
            vec![second, first],
            40,
        )
        .unwrap();

        assert_eq!(
            request.status(),
            EnrichmentRequestStatus::SuggestionsMaterialized
        );
        assert_eq!(suggestions.len(), 2);
        assert!(suggestions[0].suggestion_id() < suggestions[1].suggestion_id());
    }

    #[test]
    fn no_match_advances_with_no_suggestion_records() {
        let (mut request, receipt, profile, mapping) =
            fixture(ProviderResponseClass::NoMatch, false);
        let suggestions =
            materialize_suggestions(&mut request, &receipt, &profile, &mapping, Vec::new(), 40)
                .unwrap();
        assert!(suggestions.is_empty());
        assert_eq!(
            request.status(),
            EnrichmentRequestStatus::SuggestionsMaterialized
        );
    }

    #[test]
    fn invalid_candidate_leaves_request_unchanged() {
        let (mut request, receipt, profile, mapping) =
            fixture(ProviderResponseClass::Success, true);
        let before = request.clone();
        let mut invalid = candidate(&request, "Example Company", None);
        invalid.target.resource_version += 1;

        let error = materialize_suggestions(
            &mut request,
            &receipt,
            &profile,
            &mapping,
            vec![invalid],
            40,
        )
        .unwrap_err();

        assert_eq!(error.code, "CUSTOMER_ENRICHMENT_CANDIDATE_TARGET_CONFLICT");
        assert_eq!(request, before);
    }

    #[test]
    fn duplicate_normalized_candidates_are_rejected_without_mutation() {
        let (mut request, receipt, profile, mapping) =
            fixture(ProviderResponseClass::Success, true);
        let before = request.clone();
        let first = candidate(&request, "Example   Company", Some(9_000));
        let second = candidate(&request, "Example Company", Some(9_000));

        let error = materialize_suggestions(
            &mut request,
            &receipt,
            &profile,
            &mapping,
            vec![first, second],
            40,
        )
        .unwrap_err();

        assert_eq!(error.code, "CUSTOMER_ENRICHMENT_SUGGESTION_DUPLICATE");
        assert_eq!(request, before);
    }

    fn fixture(
        response_class: ProviderResponseClass,
        confidence_required: bool,
    ) -> (
        EnrichmentRequest,
        ProviderResponseReceipt,
        ProviderProfileVersion,
        MappingVersion,
    ) {
        let profile = ProviderProfileVersion::publish(ProviderProfileDraft {
            provider_key: "provider".to_owned(),
            adapter_kind: "adapter".to_owned(),
            adapter_contract_version: "1.0.0".to_owned(),
            supported_target_fields: vec![TargetField::PartyDisplayName],
            purpose_codes: vec!["customer_profile_enrichment".to_owned()],
            license_id: "provider-license".to_owned(),
            permitted_use_class: "customer_master_review".to_owned(),
            residency_region: "eu".to_owned(),
            retention_days: 30,
            raw_payload_policy: RawPayloadPolicy::DigestOnly,
            credential_handle_aliases: vec!["provider_primary".to_owned()],
            effective_at_unix_ms: 1,
            expires_at_unix_ms: Some(1_000),
        })
        .unwrap();
        let mapping = MappingVersion::publish(MappingDraft {
            mapping_key: "party_display_name".to_owned(),
            provider_profile_version_id: profile.version_id().clone(),
            provider_response_field_path: "organization.legal_name".to_owned(),
            target_field: TargetField::PartyDisplayName,
            normalization: MappingNormalization::CanonicalPartyDisplayNameV1,
            maximum_suggestions_per_response: 2,
            confidence_required,
        })
        .unwrap();
        let mut request = EnrichmentRequest::create(EnrichmentRequestDraft {
            tenant_id: TenantId::try_new("tenant-a").unwrap(),
            requested_by: ActorId::try_new("worker-a").unwrap(),
            idempotency_key: IdempotencyKey::try_new("request-a").unwrap(),
            target: TargetSnapshot::try_new("party-a", 7, TargetField::PartyDisplayName).unwrap(),
            provider_profile_version_id: profile.version_id().clone(),
            mapping_version_id: mapping.version_id().clone(),
            requested_fields: vec![TargetField::PartyDisplayName],
            policy_evidence: RequestPolicyEvidence::try_new(
                "customer_profile_enrichment",
                "legitimate_interest",
                Some("consent-a".to_owned()),
                "policy-v1",
            )
            .unwrap(),
            created_at_unix_ms: 1,
            deadline_at_unix_ms: 100,
            expires_at_unix_ms: 200,
        })
        .unwrap();
        request.queue(10).unwrap();
        request.mark_dispatched(10).unwrap();
        let receipt = ProviderResponseReceipt::record(ProviderResponseReceiptDraft {
            request_id: request.request_id().clone(),
            provider_profile_version_id: profile.version_id().clone(),
            mapping_version_id: mapping.version_id().clone(),
            replay_key: "provider-replay-a".to_owned(),
            provider_correlation_id: None,
            response_class,
            canonical_response_digest: [7; 32],
            provider_observed_at_unix_ms: Some(20),
            retrieved_at_unix_ms: 30,
            metered_units: 1,
            protected_evidence_reference: None,
        })
        .unwrap();
        request
            .record_response(receipt.receipt_id().clone(), 30)
            .unwrap();
        (request, receipt, profile, mapping)
    }

    fn candidate(
        request: &EnrichmentRequest,
        proposed_value: &str,
        confidence_basis_points: Option<u16>,
    ) -> SuggestionCandidateDraft {
        SuggestionCandidateDraft {
            target: request.target().clone(),
            proposed_value: proposed_value.to_owned(),
            observed_at_unix_ms: Some(20),
            effective_at_unix_ms: 20,
            fresh_until_unix_ms: 100,
            expires_at_unix_ms: 150,
            confidence_basis_points,
            license_id: "provider-license".to_owned(),
            permitted_use_class: "customer_master_review".to_owned(),
            residency_region: "eu".to_owned(),
            retention_days: 30,
            consent_evidence_reference: Some("consent-a".to_owned()),
            evidence_references: Vec::new(),
        }
    }
}
