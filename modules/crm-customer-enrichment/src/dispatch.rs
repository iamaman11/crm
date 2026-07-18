use crate::{
    EnrichmentRequest, EnrichmentRequestStatus, PartySnapshot, ProviderAdapterCoordinate,
    ProviderDispatchRequest, ProviderProfileVersion, encode_enrichment_request_state,
};
use crm_module_sdk::{ActorId, ErrorCategory, RecordId, SdkError};
use serde::Deserialize;
use sha2::{Digest, Sha256};

const DISPATCH_REPLAY_KEY_DOMAIN: &[u8] = b"crm.customer-enrichment.provider-dispatch/v1";
const MAX_DISPLAY_NAME_BYTES: usize = 320;

/// Exact optimistic expectation for one pre-I/O provider dispatch attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderDispatchExpectation {
    pub status: EnrichmentRequestStatus,
    pub retry_generation: u32,
}

/// Validates and prepares one provider dispatch before network I/O.
///
/// All profile, Party, lifecycle and time-window checks happen against a clone. The supplied request
/// is changed to `Dispatched` only after the complete provider envelope has been constructed. The
/// updated request state must be committed before invoking the infrastructure registry. A crash
/// after that commit can use [`recover_provider_dispatch_attempt`] to reproduce the same exact
/// adapter coordinate and provider idempotency key.
pub fn prepare_provider_dispatch_attempt(
    request: &mut EnrichmentRequest,
    expectation: ProviderDispatchExpectation,
    provider_profile: &ProviderProfileVersion,
    party_snapshot: &PartySnapshot,
    actor_id: ActorId,
    dispatched_at_unix_ms: u64,
) -> Result<ProviderDispatchRequest, SdkError> {
    if request.status() != expectation.status
        || request.retry_generation() != expectation.retry_generation
        || !matches!(
            expectation.status,
            EnrichmentRequestStatus::Created
                | EnrichmentRequestStatus::Queued
                | EnrichmentRequestStatus::FailedRetryable
        )
    {
        return Err(dispatch_expectation_conflict());
    }

    validate_dispatch_inputs(
        request,
        provider_profile,
        party_snapshot,
        dispatched_at_unix_ms,
    )?;

    let mut next = request.clone();
    match expectation.status {
        EnrichmentRequestStatus::Created | EnrichmentRequestStatus::FailedRetryable => {
            next.queue(dispatched_at_unix_ms)?;
            next.mark_dispatched(dispatched_at_unix_ms)?;
        }
        EnrichmentRequestStatus::Queued => next.mark_dispatched(dispatched_at_unix_ms)?,
        _ => return Err(dispatch_expectation_conflict()),
    }

    let dispatch = build_dispatch_request(
        &next,
        provider_profile,
        party_snapshot,
        actor_id,
        dispatched_at_unix_ms,
    )?;
    *request = next;
    Ok(dispatch)
}

/// Rebuilds the exact provider envelope for a request durably marked `Dispatched` before a crash.
///
/// The retry generation and exact profile/Party snapshot must still match. The resulting provider
/// idempotency key is identical to the original pre-I/O attempt; no lifecycle mutation occurs.
pub fn recover_provider_dispatch_attempt(
    request: &EnrichmentRequest,
    expected_retry_generation: u32,
    provider_profile: &ProviderProfileVersion,
    party_snapshot: &PartySnapshot,
    actor_id: ActorId,
    recovered_at_unix_ms: u64,
) -> Result<ProviderDispatchRequest, SdkError> {
    if request.status() != EnrichmentRequestStatus::Dispatched
        || request.retry_generation() != expected_retry_generation
    {
        return Err(dispatch_expectation_conflict());
    }
    validate_dispatch_inputs(
        request,
        provider_profile,
        party_snapshot,
        recovered_at_unix_ms,
    )?;
    build_dispatch_request(
        request,
        provider_profile,
        party_snapshot,
        actor_id,
        recovered_at_unix_ms,
    )
}

/// Stable provider-side idempotency key for one exact request dispatch generation and adapter.
pub fn provider_dispatch_replay_key(
    request: &EnrichmentRequest,
    adapter_coordinate: &ProviderAdapterCoordinate,
) -> String {
    let mut hasher = Sha256::new();
    hash_frame(&mut hasher, DISPATCH_REPLAY_KEY_DOMAIN);
    hash_frame(&mut hasher, request.request_id().as_str().as_bytes());
    hash_frame(&mut hasher, &request.retry_generation().to_be_bytes());
    hash_frame(
        &mut hasher,
        request.provider_profile_version_id().as_str().as_bytes(),
    );
    hash_frame(&mut hasher, adapter_coordinate.adapter_kind().as_bytes());
    hash_frame(
        &mut hasher,
        adapter_coordinate.adapter_contract_version().as_bytes(),
    );
    format!("enrichment-dispatch-{}", hex(&hasher.finalize()))
}

fn build_dispatch_request(
    request: &EnrichmentRequest,
    provider_profile: &ProviderProfileVersion,
    party_snapshot: &PartySnapshot,
    actor_id: ActorId,
    at_unix_ms: u64,
) -> Result<ProviderDispatchRequest, SdkError> {
    let state = request_state(request)?;
    ensure_dispatch_window(&state, at_unix_ms)?;
    let adapter_coordinate = ProviderAdapterCoordinate::try_new(
        provider_profile.adapter_kind(),
        provider_profile.adapter_contract_version(),
    )?;
    let provider_idempotency_key = provider_dispatch_replay_key(request, &adapter_coordinate);
    Ok(ProviderDispatchRequest {
        tenant_id: request.tenant_id().clone(),
        actor_id,
        enrichment_request_id: request.request_id().clone(),
        provider_profile_version_id: request.provider_profile_version_id().clone(),
        mapping_version_id: request.mapping_version_id().clone(),
        adapter_coordinate,
        retry_generation: request.retry_generation(),
        party_id: party_snapshot.party_id.clone(),
        party_resource_version: party_snapshot.resource_version,
        current_display_name: validated_display_name(&party_snapshot.display_name)?,
        provider_idempotency_key,
        credential_handle_aliases: provider_profile.credential_handle_aliases().to_vec(),
        deadline_at_unix_ms: checked_i64(state.deadline_at_unix_ms, "request deadline")?,
    })
}

fn validate_dispatch_inputs(
    request: &EnrichmentRequest,
    provider_profile: &ProviderProfileVersion,
    party_snapshot: &PartySnapshot,
    at_unix_ms: u64,
) -> Result<(), SdkError> {
    let state = request_state(request)?;
    ensure_dispatch_window(&state, at_unix_ms)?;
    if provider_profile.version_id() != request.provider_profile_version_id()
        || !provider_profile.is_effective_at(at_unix_ms)
        || !provider_profile
            .supported_target_fields()
            .contains(&request.target().target_field)
    {
        return Err(dispatch_profile_conflict());
    }

    let expected_party_id = RecordId::try_new(request.target().resource_id.clone())
        .map_err(|error| dispatch_plan_invalid(error.to_string()))?;
    let expected_party_version = checked_i64(
        request.target().resource_version,
        "target Party resource version",
    )?;
    let at_unix_ms = checked_i64(at_unix_ms, "dispatch timestamp")?;
    if party_snapshot.party_id != expected_party_id
        || party_snapshot.resource_version != expected_party_version
        || party_snapshot.resource_version <= 0
        || party_snapshot.observed_at_unix_ms < 0
        || party_snapshot.observed_at_unix_ms > at_unix_ms
    {
        return Err(dispatch_target_conflict());
    }
    validated_display_name(&party_snapshot.display_name)?;
    Ok(())
}

fn ensure_dispatch_window(
    state: &DispatchRequestStateView,
    at_unix_ms: u64,
) -> Result<(), SdkError> {
    if at_unix_ms >= state.deadline_at_unix_ms || at_unix_ms >= state.expires_at_unix_ms {
        return Err(SdkError::new(
            "CUSTOMER_ENRICHMENT_DISPATCH_WINDOW_CLOSED",
            ErrorCategory::Conflict,
            false,
            "The enrichment request can no longer be dispatched.",
        ));
    }
    Ok(())
}

fn request_state(request: &EnrichmentRequest) -> Result<DispatchRequestStateView, SdkError> {
    serde_json::from_slice(&encode_enrichment_request_state(request)?)
        .map_err(|error| dispatch_plan_invalid(error.to_string()))
}

fn validated_display_name(value: &str) -> Result<String, SdkError> {
    if value.is_empty()
        || value.len() > MAX_DISPLAY_NAME_BYTES
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(SdkError::invalid_argument(
            "provider_dispatch.current_display_name",
            format!(
                "display name must be non-empty, canonical and no longer than {MAX_DISPLAY_NAME_BYTES} bytes"
            ),
        ));
    }
    Ok(value.to_owned())
}

fn checked_i64(value: u64, label: &'static str) -> Result<i64, SdkError> {
    i64::try_from(value).map_err(|_| dispatch_plan_invalid(format!("{label} exceeds wire range")))
}

fn hash_frame(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}

fn dispatch_expectation_conflict() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_DISPATCH_EXPECTATION_CONFLICT",
        ErrorCategory::Conflict,
        false,
        "The enrichment request changed before provider dispatch.",
    )
}

fn dispatch_profile_conflict() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_DISPATCH_PROFILE_CONFLICT",
        ErrorCategory::Conflict,
        false,
        "The exact provider profile is unavailable for dispatch.",
    )
}

fn dispatch_target_conflict() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_DISPATCH_TARGET_CONFLICT",
        ErrorCategory::Conflict,
        false,
        "The exact Party snapshot is unavailable for dispatch.",
    )
}

fn dispatch_plan_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_DISPATCH_PLAN_INVALID",
        ErrorCategory::Internal,
        false,
        "The provider dispatch could not be prepared safely.",
    )
    .with_internal_reference(reference.into())
}

#[derive(Debug, Deserialize)]
struct DispatchRequestStateView {
    deadline_at_unix_ms: u64,
    expires_at_unix_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        EnrichmentRequestDraft, IdempotencyKey, MappingDraft, MappingNormalization, MappingVersion,
        ProviderProfileDraft, RawPayloadPolicy, RequestPolicyEvidence, TargetField, TargetSnapshot,
    };
    use crm_module_sdk::{TenantId, IdempotencyKey as SdkIdempotencyKey};

    fn fixture() -> (EnrichmentRequest, ProviderProfileVersion, PartySnapshot, ActorId) {
        let profile = ProviderProfileVersion::publish(ProviderProfileDraft {
            provider_key: "company_registry".to_owned(),
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
            expires_at_unix_ms: Some(1_000),
        })
        .unwrap();
        let mapping = MappingVersion::publish(MappingDraft {
            mapping_key: "party_display_name".to_owned(),
            provider_profile_version_id: profile.version_id().clone(),
            provider_response_field_path: "organization.legal_name".to_owned(),
            target_field: TargetField::PartyDisplayName,
            normalization: MappingNormalization::CanonicalPartyDisplayNameV1,
            maximum_suggestions_per_response: 1,
            confidence_required: true,
        })
        .unwrap();
        let actor = ActorId::try_new("worker-actor").unwrap();
        let request = EnrichmentRequest::create(EnrichmentRequestDraft {
            tenant_id: TenantId::try_new("tenant-1").unwrap(),
            requested_by: ActorId::try_new("requester-1").unwrap(),
            idempotency_key: SdkIdempotencyKey::try_new("request-key-1").unwrap(),
            target: TargetSnapshot::try_new("party-1", 7, TargetField::PartyDisplayName).unwrap(),
            provider_profile_version_id: profile.version_id().clone(),
            mapping_version_id: mapping.version_id().clone(),
            requested_fields: vec![TargetField::PartyDisplayName],
            policy_evidence: RequestPolicyEvidence::try_new(
                "customer_profile_enrichment",
                "legitimate_interest",
                None,
                "1.0.0",
            )
            .unwrap(),
            created_at_unix_ms: 10,
            deadline_at_unix_ms: 100,
            expires_at_unix_ms: 200,
        })
        .unwrap();
        let party = PartySnapshot {
            party_id: RecordId::try_new("party-1").unwrap(),
            display_name: "Example Company".to_owned(),
            resource_version: 7,
            observed_at_unix_ms: 15,
        };
        (request, profile, party, actor)
    }

    #[test]
    fn prepare_and_recover_share_the_exact_replay_key() {
        let (mut request, profile, party, actor) = fixture();
        let prepared = prepare_provider_dispatch_attempt(
            &mut request,
            ProviderDispatchExpectation {
                status: EnrichmentRequestStatus::Created,
                retry_generation: 0,
            },
            &profile,
            &party,
            actor.clone(),
            20,
        )
        .unwrap();
        assert_eq!(request.status(), EnrichmentRequestStatus::Dispatched);
        assert_eq!(prepared.retry_generation, 0);
        assert_eq!(prepared.adapter_coordinate.adapter_kind(), "registry_http_v1");
        assert_eq!(
            prepared.adapter_coordinate.adapter_contract_version(),
            "1.0.0"
        );

        let recovered = recover_provider_dispatch_attempt(
            &request,
            0,
            &profile,
            &party,
            actor,
            21,
        )
        .unwrap();
        assert_eq!(prepared, recovered);
        assert!(prepared
            .provider_idempotency_key
            .starts_with("enrichment-dispatch-"));
    }

    #[test]
    fn retry_dispatch_advances_generation_before_deriving_replay_key() {
        let (mut request, profile, party, actor) = fixture();
        request.fail_retryable("provider_unavailable", 16).unwrap();
        let prepared = prepare_provider_dispatch_attempt(
            &mut request,
            ProviderDispatchExpectation {
                status: EnrichmentRequestStatus::FailedRetryable,
                retry_generation: 0,
            },
            &profile,
            &party,
            actor,
            20,
        )
        .unwrap();
        assert_eq!(request.retry_generation(), 1);
        assert_eq!(prepared.retry_generation, 1);
        assert_eq!(
            prepared.provider_idempotency_key,
            provider_dispatch_replay_key(&request, &prepared.adapter_coordinate)
        );
    }

    #[test]
    fn profile_mismatch_fails_before_request_mutation() {
        let (mut request, _, party, actor) = fixture();
        let other_profile = ProviderProfileVersion::publish(ProviderProfileDraft {
            provider_key: "other_registry".to_owned(),
            adapter_kind: "registry_http_v1".to_owned(),
            adapter_contract_version: "1.0.0".to_owned(),
            supported_target_fields: vec![TargetField::PartyDisplayName],
            purpose_codes: vec!["customer_profile_enrichment".to_owned()],
            license_id: "Other licence".to_owned(),
            permitted_use_class: "customer_master_review".to_owned(),
            residency_region: "eu".to_owned(),
            retention_days: 30,
            raw_payload_policy: RawPayloadPolicy::DigestOnly,
            credential_handle_aliases: vec!["other_primary".to_owned()],
            effective_at_unix_ms: 1,
            expires_at_unix_ms: Some(1_000),
        })
        .unwrap();
        let original = request.clone();
        assert!(prepare_provider_dispatch_attempt(
            &mut request,
            ProviderDispatchExpectation {
                status: EnrichmentRequestStatus::Created,
                retry_generation: 0,
            },
            &other_profile,
            &party,
            actor,
            20,
        )
        .is_err());
        assert_eq!(request, original);
    }

    #[test]
    fn stale_party_snapshot_fails_before_request_mutation() {
        let (mut request, profile, mut party, actor) = fixture();
        party.resource_version = 8;
        let original = request.clone();
        assert!(prepare_provider_dispatch_attempt(
            &mut request,
            ProviderDispatchExpectation {
                status: EnrichmentRequestStatus::Created,
                retry_generation: 0,
            },
            &profile,
            &party,
            actor,
            20,
        )
        .is_err());
        assert_eq!(request, original);
    }

    #[test]
    fn closed_dispatch_window_fails_before_request_mutation() {
        let (mut request, profile, party, actor) = fixture();
        let original = request.clone();
        assert!(prepare_provider_dispatch_attempt(
            &mut request,
            ProviderDispatchExpectation {
                status: EnrichmentRequestStatus::Created,
                retry_generation: 0,
            },
            &profile,
            &party,
            actor,
            100,
        )
        .is_err());
        assert_eq!(request, original);
    }
}
