#![forbid(unsafe_code)]

//! Deterministic preparation and event-driven orchestration for the internal Customer Enrichment
//! provider process.
//!
//! This crate converts one governed request plus exact provider-profile and Party snapshots into
//! the durable worker work item consumed by `crm-customer-enrichment-worker-composition`. It owns no
//! provider network I/O and registers no public capability route.

mod conflict_persistence;
mod conflict_rejection;
mod conflict_resolution;
mod worker;

pub use conflict_persistence::*;
pub use conflict_rejection::*;
pub use conflict_resolution::*;
pub use worker::*;

use crm_capability_ingress::semantic_input_hash;
use crm_capability_plan_support as support;
use crm_capability_runtime::CapabilityRequest;
use crm_customer_enrichment::{
    EnrichmentRequest, EnrichmentRequestStatus, PartySnapshot, ProviderDispatchExpectation,
    ProviderDispatchRequest, ProviderProfileVersion, prepare_provider_dispatch_attempt,
    recover_provider_dispatch_attempt,
};
use crm_customer_enrichment_capability_adapter::{
    DISPATCH_ENRICHMENT_REQUEST_REQUEST_SCHEMA, MODULE_ID, enrichment_request_to_wire,
    request_dispatch_capability_definition,
};
use crm_customer_enrichment_worker_composition::ProviderDispatchWorkItem;
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CausationId, CorrelationId, DataClass, ErrorCategory,
    ExecutionContext, IdempotencyKey, ModuleExecutionContext, RequestId, SchemaVersion, SdkError,
    TraceId,
};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;
use sha2::{Digest, Sha256};

const DISPATCH_WORK_ITEM_IDENTITY_DOMAIN: &[u8] = b"crm.customer-enrichment.provider-work-item/v1";

/// Stable crate identity for architecture tooling.
pub const CRATE_NAME: &str = "crm-customer-enrichment-provider-process-composition";

/// Exact governed inputs required to prepare one provider worker attempt.
#[derive(Debug, Clone, Copy)]
pub struct ProviderDispatchWorkItemInput<'a> {
    pub request: &'a EnrichmentRequest,
    pub provider_profile: &'a ProviderProfileVersion,
    pub party_snapshot: &'a PartySnapshot,
    pub worker_actor_id: &'a ActorId,
    pub now_unix_ms: u64,
}

/// Builds a deterministic dispatch capability request and exact provider envelope.
///
/// `Created` and `FailedRetryable` requests prepare a new generation against a clone. A durable
/// `Dispatched` request reconstructs the original capability request identity from its persisted
/// dispatch timestamp while revalidating the exact profile and Party snapshot at the current time.
/// `Queued` is rejected because Phase 8A.10 does not persist a separately recoverable queue origin.
pub fn build_provider_dispatch_work_item(
    input: ProviderDispatchWorkItemInput<'_>,
) -> Result<ProviderDispatchWorkItem, SdkError> {
    let ProviderDispatchWorkItemInput {
        request,
        provider_profile,
        party_snapshot,
        worker_actor_id,
        now_unix_ms,
    } = input;

    let (expected_status, expected_retry_generation, dispatch_started_at_unix_ms, provider_request) =
        match request.status() {
            EnrichmentRequestStatus::Created => {
                let mut next = request.to_owned();
                let provider_request = prepare_provider_dispatch_attempt(
                    &mut next,
                    ProviderDispatchExpectation {
                        status: EnrichmentRequestStatus::Created,
                        retry_generation: request.retry_generation(),
                    },
                    provider_profile,
                    party_snapshot,
                    worker_actor_id.clone(),
                    now_unix_ms,
                )?;
                (
                    wire::EnrichmentRequestStatus::Created,
                    request.retry_generation(),
                    now_unix_ms,
                    provider_request,
                )
            }
            EnrichmentRequestStatus::FailedRetryable => {
                let mut next = request.to_owned();
                let provider_request = prepare_provider_dispatch_attempt(
                    &mut next,
                    ProviderDispatchExpectation {
                        status: EnrichmentRequestStatus::FailedRetryable,
                        retry_generation: request.retry_generation(),
                    },
                    provider_profile,
                    party_snapshot,
                    worker_actor_id.clone(),
                    now_unix_ms,
                )?;
                (
                    wire::EnrichmentRequestStatus::FailedRetryable,
                    request.retry_generation(),
                    now_unix_ms,
                    provider_request,
                )
            }
            EnrichmentRequestStatus::Dispatched => {
                let final_generation = request.retry_generation();
                let (expected_status, expected_retry_generation) = if final_generation == 0 {
                    (wire::EnrichmentRequestStatus::Created, 0)
                } else {
                    (
                        wire::EnrichmentRequestStatus::FailedRetryable,
                        final_generation - 1,
                    )
                };
                let public_request = enrichment_request_to_wire(request)?;
                let dispatch_started_at_unix_ms =
                    nonnegative_u64(public_request.updated_at_unix_ms)?;
                let provider_request = recover_provider_dispatch_attempt(
                    request,
                    final_generation,
                    provider_profile,
                    party_snapshot,
                    worker_actor_id.clone(),
                    now_unix_ms,
                )?;
                (
                    expected_status,
                    expected_retry_generation,
                    dispatch_started_at_unix_ms,
                    provider_request,
                )
            }
            EnrichmentRequestStatus::Queued => return Err(queue_state_unrecoverable()),
            status => return Err(request_not_actionable(status)),
        };

    let dispatch_request = build_dispatch_capability_request(
        request,
        &provider_request,
        expected_status,
        expected_retry_generation,
        dispatch_started_at_unix_ms,
    )?;
    Ok(ProviderDispatchWorkItem {
        dispatch_request,
        provider_request,
    })
}

fn build_dispatch_capability_request(
    request: &EnrichmentRequest,
    provider_request: &ProviderDispatchRequest,
    expected_status: wire::EnrichmentRequestStatus,
    expected_retry_generation: u32,
    dispatch_started_at_unix_ms: u64,
) -> Result<CapabilityRequest, SdkError> {
    let definition = request_dispatch_capability_definition()?;
    let command = wire::DispatchEnrichmentRequestRequest {
        enrichment_request_ref: Some(wire::EnrichmentRequestRef {
            enrichment_request_id: request.request_id().as_str().to_owned(),
        }),
        expected_status: expected_status as i32,
        expected_retry_generation,
    };
    let input = support::protobuf_payload(
        MODULE_ID,
        DISPATCH_ENRICHMENT_REQUEST_REQUEST_SCHEMA,
        DataClass::Personal,
        &command,
    )?;
    let suffix = hex(&dispatch_work_item_identity(provider_request));
    let request_started_at_unix_nanos = dispatch_started_at_unix_ms
        .checked_mul(1_000_000)
        .and_then(|value| i64::try_from(value).ok())
        .ok_or_else(dispatch_timestamp_invalid)?;
    let context = ModuleExecutionContext {
        module_id: definition.owner_module_id.clone(),
        execution: ExecutionContext {
            tenant_id: provider_request.tenant_id.clone(),
            actor_id: provider_request.actor_id.clone(),
            request_id: configured(RequestId::try_new(format!(
                "enrichment-dispatch-request-{suffix}"
            )))?,
            correlation_id: configured(CorrelationId::try_new(format!(
                "enrichment-dispatch-correlation-{suffix}"
            )))?,
            causation_id: configured(CausationId::try_new(request.request_id().as_str()))?,
            trace_id: configured(TraceId::try_new(format!(
                "enrichment-dispatch-trace-{suffix}"
            )))?,
            capability_id: definition.capability_id.clone(),
            capability_version: definition.capability_version.clone(),
            idempotency_key: configured(IdempotencyKey::try_new(format!(
                "enrichment-dispatch-{suffix}"
            )))?,
            business_transaction_id: configured(BusinessTransactionId::try_new(format!(
                "enrichment-dispatch-tx-{suffix}"
            )))?,
            schema_version: configured(SchemaVersion::try_new(support::CONTRACT_VERSION))?,
            request_started_at_unix_nanos,
        },
    };
    let input_hash = semantic_input_hash(&input);
    Ok(CapabilityRequest {
        context,
        input,
        input_hash,
        approval: None,
    })
}

fn dispatch_work_item_identity(request: &ProviderDispatchRequest) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hash_frame(&mut hasher, DISPATCH_WORK_ITEM_IDENTITY_DOMAIN);
    hash_frame(
        &mut hasher,
        request.enrichment_request_id.as_str().as_bytes(),
    );
    hash_frame(&mut hasher, &request.retry_generation.to_be_bytes());
    hash_frame(&mut hasher, request.provider_idempotency_key.as_bytes());
    hasher.finalize().into()
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

fn nonnegative_u64(value: i64) -> Result<u64, SdkError> {
    u64::try_from(value).map_err(|_| dispatch_timestamp_invalid())
}

fn configured<T>(value: Result<T, crm_module_sdk::IdentifierError>) -> Result<T, SdkError> {
    value.map_err(|error| work_item_invalid(error.to_string()))
}

fn dispatch_timestamp_invalid() -> SdkError {
    work_item_invalid("persisted dispatch timestamp cannot be represented safely")
}

fn queue_state_unrecoverable() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_QUEUE_STATE_UNRECOVERABLE",
        ErrorCategory::Conflict,
        false,
        "The queued enrichment request cannot be dispatched without exact queue evidence.",
    )
}

fn request_not_actionable(status: EnrichmentRequestStatus) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_REQUEST_NOT_ACTIONABLE",
        ErrorCategory::Conflict,
        false,
        "The enrichment request is not eligible for provider dispatch.",
    )
    .with_internal_reference(format!("request status is {status:?}"))
}

fn work_item_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_WORK_ITEM_PREPARATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The provider work item could not be prepared safely.",
    )
    .with_internal_reference(reference.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_customer_enrichment::{
        EnrichmentRequestDraft, MappingDraft, MappingNormalization, MappingVersion,
        ProviderProfileDraft, RawPayloadPolicy, RequestPolicyEvidence, TargetField, TargetSnapshot,
    };
    use crm_module_sdk::{IdempotencyKey, RecordId, TenantId};

    #[test]
    fn dispatched_recovery_rebuilds_the_exact_created_work_item() {
        let fixture = fixture();
        let initial = build_provider_dispatch_work_item(ProviderDispatchWorkItemInput {
            request: &fixture.request,
            provider_profile: &fixture.profile,
            party_snapshot: &fixture.party,
            worker_actor_id: &fixture.actor,
            now_unix_ms: 20,
        })
        .unwrap();

        let mut dispatched = fixture.request.clone();
        prepare_provider_dispatch_attempt(
            &mut dispatched,
            ProviderDispatchExpectation {
                status: EnrichmentRequestStatus::Created,
                retry_generation: 0,
            },
            &fixture.profile,
            &fixture.party,
            fixture.actor.clone(),
            20,
        )
        .unwrap();
        let recovered = build_provider_dispatch_work_item(ProviderDispatchWorkItemInput {
            request: &dispatched,
            provider_profile: &fixture.profile,
            party_snapshot: &fixture.party,
            worker_actor_id: &fixture.actor,
            now_unix_ms: 21,
        })
        .unwrap();

        assert_eq!(initial, recovered);
    }

    #[test]
    fn dispatched_recovery_rebuilds_the_exact_retry_work_item() {
        let mut fixture = fixture();
        fixture
            .request
            .fail_retryable("provider_timeout", 20)
            .unwrap();
        let initial = build_provider_dispatch_work_item(ProviderDispatchWorkItemInput {
            request: &fixture.request,
            provider_profile: &fixture.profile,
            party_snapshot: &fixture.party,
            worker_actor_id: &fixture.actor,
            now_unix_ms: 30,
        })
        .unwrap();
        assert_eq!(initial.provider_request.retry_generation, 1);

        let mut dispatched = fixture.request.clone();
        prepare_provider_dispatch_attempt(
            &mut dispatched,
            ProviderDispatchExpectation {
                status: EnrichmentRequestStatus::FailedRetryable,
                retry_generation: 0,
            },
            &fixture.profile,
            &fixture.party,
            fixture.actor.clone(),
            30,
        )
        .unwrap();
        let recovered = build_provider_dispatch_work_item(ProviderDispatchWorkItemInput {
            request: &dispatched,
            provider_profile: &fixture.profile,
            party_snapshot: &fixture.party,
            worker_actor_id: &fixture.actor,
            now_unix_ms: 31,
        })
        .unwrap();
        assert_eq!(initial, recovered);
    }

    #[test]
    fn exact_profile_party_and_deadline_are_revalidated_before_recovery() {
        let fixture = fixture();
        let mut dispatched = fixture.request.clone();
        prepare_provider_dispatch_attempt(
            &mut dispatched,
            ProviderDispatchExpectation {
                status: EnrichmentRequestStatus::Created,
                retry_generation: 0,
            },
            &fixture.profile,
            &fixture.party,
            fixture.actor.clone(),
            20,
        )
        .unwrap();

        let mut stale_party = fixture.party.clone();
        stale_party.resource_version += 1;
        let error = build_provider_dispatch_work_item(ProviderDispatchWorkItemInput {
            request: &dispatched,
            provider_profile: &fixture.profile,
            party_snapshot: &stale_party,
            worker_actor_id: &fixture.actor,
            now_unix_ms: 21,
        })
        .unwrap_err();
        assert_eq!(error.code, "CUSTOMER_ENRICHMENT_DISPATCH_TARGET_CONFLICT");

        let error = build_provider_dispatch_work_item(ProviderDispatchWorkItemInput {
            request: &dispatched,
            provider_profile: &fixture.profile,
            party_snapshot: &fixture.party,
            worker_actor_id: &fixture.actor,
            now_unix_ms: 1_000,
        })
        .unwrap_err();
        assert_eq!(error.code, "CUSTOMER_ENRICHMENT_DISPATCH_WINDOW_CLOSED");
    }

    #[test]
    fn queued_and_terminal_states_are_not_recoverable() {
        let fixture = fixture();
        let mut queued = fixture.request.clone();
        queued.queue(20).unwrap();
        assert_eq!(
            build_provider_dispatch_work_item(ProviderDispatchWorkItemInput {
                request: &queued,
                provider_profile: &fixture.profile,
                party_snapshot: &fixture.party,
                worker_actor_id: &fixture.actor,
                now_unix_ms: 21,
            })
            .unwrap_err()
            .code,
            "CUSTOMER_ENRICHMENT_PROVIDER_QUEUE_STATE_UNRECOVERABLE"
        );

        let mut cancelled = fixture.request.clone();
        cancelled.cancel(20).unwrap();
        assert_eq!(
            build_provider_dispatch_work_item(ProviderDispatchWorkItemInput {
                request: &cancelled,
                provider_profile: &fixture.profile,
                party_snapshot: &fixture.party,
                worker_actor_id: &fixture.actor,
                now_unix_ms: 21,
            })
            .unwrap_err()
            .code,
            "CUSTOMER_ENRICHMENT_PROVIDER_REQUEST_NOT_ACTIONABLE"
        );
    }

    struct Fixture {
        request: EnrichmentRequest,
        profile: ProviderProfileVersion,
        party: PartySnapshot,
        actor: ActorId,
    }

    fn fixture() -> Fixture {
        let profile = ProviderProfileVersion::publish(ProviderProfileDraft {
            provider_key: "company_registry".to_owned(),
            adapter_kind: "registry_http_v1".to_owned(),
            adapter_contract_version: "1.0.0".to_owned(),
            supported_target_fields: vec![TargetField::PartyDisplayName],
            purpose_codes: vec!["customer_profile_enrichment".to_owned()],
            license_id: "Registry process licence".to_owned(),
            permitted_use_class: "customer_master_review".to_owned(),
            residency_region: "eu".to_owned(),
            retention_days: 30,
            raw_payload_policy: RawPayloadPolicy::DigestOnly,
            credential_handle_aliases: vec!["registry_primary".to_owned()],
            effective_at_unix_ms: 1,
            expires_at_unix_ms: Some(5_000),
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
        let actor = ActorId::try_new("provider-process-worker").unwrap();
        let request = EnrichmentRequest::create(EnrichmentRequestDraft {
            tenant_id: TenantId::try_new("tenant-a").unwrap(),
            requested_by: actor.clone(),
            idempotency_key: IdempotencyKey::try_new("provider-process-request").unwrap(),
            target: TargetSnapshot::try_new("party-1", 7, TargetField::PartyDisplayName).unwrap(),
            provider_profile_version_id: profile.version_id().clone(),
            mapping_version_id: mapping.version_id().clone(),
            requested_fields: vec![TargetField::PartyDisplayName],
            policy_evidence: RequestPolicyEvidence::try_new(
                "customer_profile_enrichment",
                "legitimate_interest",
                None,
                "provider-process-policy-v1",
            )
            .unwrap(),
            created_at_unix_ms: 10,
            deadline_at_unix_ms: 1_000,
            expires_at_unix_ms: 2_000,
        })
        .unwrap();
        let party = PartySnapshot {
            party_id: RecordId::try_new("party-1").unwrap(),
            display_name: "Example Company".to_owned(),
            resource_version: 7,
            observed_at_unix_ms: 15,
        };
        Fixture {
            request,
            profile,
            party,
            actor,
        }
    }
}
