use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::header::{AUTHORIZATION, CONTENT_LENGTH, CONTENT_TYPE};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use crm_customer_enrichment::{
    EnrichmentRequest, EnrichmentRequestDraft, EnrichmentRequestStatus, MappingDraft,
    MappingNormalization, MappingVersion, PartySnapshot, ProviderDispatchExpectation,
    ProviderDispatchPort, ProviderProfileDraft, ProviderProfileVersion, RawPayloadPolicy,
    RequestPolicyEvidence, TargetField, TargetSnapshot, prepare_provider_dispatch_attempt,
};
use crm_customer_enrichment_provider_registry::{
    ConsecutiveFailureProviderCircuitBreaker, FixedWindowProviderQuota, GovernedProviderAdapter,
    ProviderSecretMaterial, ProviderSecretRegistration, StaticProviderSecretHandleResolver,
};
use crm_customer_enrichment_registry_http_transport::{
    REGISTRY_HTTP_ADAPTER_CONTRACT_VERSION, REGISTRY_HTTP_ADAPTER_KIND, REGISTRY_HTTP_PATH,
    RegistryHttpTransport, RegistryHttpTransportConfig,
};
use crm_module_sdk::testing::FixedClock;
use crm_module_sdk::{ActorId, IdempotencyKey, RecordId, SdkError, TenantId};
use serde_json::{Value, json};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::TcpListener;

const SECRET_MARKER: &str = "raw-upstream-secret-marker";

#[derive(Debug, Clone, Copy)]
enum ResponseMode {
    Oversized,
    WrongSchema,
}

#[derive(Clone)]
struct ProviderState {
    mode: ResponseMode,
    calls: Arc<AtomicUsize>,
    observed_keys: Arc<Mutex<Vec<String>>>,
}

async fn provider(
    State(state): State<ProviderState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    state.calls.fetch_add(1, Ordering::SeqCst);
    if headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        != Some("Bearer super-secret-provider-token")
    {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let request: Value = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let Some(replay_key) = request
        .get("provider_idempotency_key")
        .and_then(Value::as_str)
    else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    if headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        != Some(replay_key)
    {
        return StatusCode::CONFLICT.into_response();
    }
    state
        .observed_keys
        .lock()
        .expect("record provider idempotency key")
        .push(replay_key.to_owned());

    match state.mode {
        ResponseMode::Oversized => {
            let oversized = SECRET_MARKER.repeat(128);
            Response::builder()
                .status(StatusCode::OK)
                .header(CONTENT_TYPE, "application/json")
                .header(CONTENT_LENGTH, oversized.len().to_string())
                .body(Body::from(oversized))
                .expect("build oversized provider response")
        }
        ResponseMode::WrongSchema => Json(json!({
            "schema_version": "crm.customer-enrichment.registry-http.response/v999",
            "replay_key": replay_key,
            "provider_correlation_id": SECRET_MARKER,
            "response_class": "success",
            "provider_observed_at_unix_ms": 4_000,
            "metered_units": 1,
            "protected_evidence_reference": null,
            "safe_provider_code": "success"
        }))
        .into_response(),
    }
}

async fn spawn_provider(state: ProviderState) -> String {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind response-contract provider");
    let address = listener.local_addr().expect("read provider address");
    let router = Router::new()
        .route(REGISTRY_HTTP_PATH, post(provider))
        .with_state(state);
    tokio::spawn(async move {
        axum::serve(listener, router)
            .await
            .expect("serve response-contract provider");
    });
    format!("http://{address}{REGISTRY_HTTP_PATH}")
}

#[tokio::test]
async fn oversized_response_is_bounded_sanitized_and_deterministic() {
    verify_rejected_response(
        ResponseMode::Oversized,
        128,
        "provider_code:response_too_large",
    )
    .await;
}

#[tokio::test]
async fn wrong_schema_is_sanitized_and_deterministic() {
    verify_rejected_response(
        ResponseMode::WrongSchema,
        4 * 1024,
        "provider_code:response_lineage_conflict",
    )
    .await;
}

async fn verify_rejected_response(
    mode: ResponseMode,
    maximum_response_bytes: usize,
    expected_reference: &str,
) {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed_keys = Arc::new(Mutex::new(Vec::new()));
    let endpoint = spawn_provider(ProviderState {
        mode,
        calls: calls.clone(),
        observed_keys: observed_keys.clone(),
    })
    .await;
    let adapter = adapter(endpoint, maximum_response_bytes);
    let request = provider_request();
    let replay_key = request.provider_idempotency_key.clone();

    let first = adapter
        .dispatch(request.clone())
        .await
        .expect_err("invalid provider response must fail closed");
    let second = adapter
        .dispatch(request)
        .await
        .expect_err("exact repeated response must fail identically");

    assert_mapping_conflict(&first, expected_reference);
    assert_mapping_conflict(&second, expected_reference);
    assert_eq!(first.code, second.code);
    assert_eq!(first.internal_reference, second.internal_reference);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(
        *observed_keys.lock().expect("read provider keys"),
        vec![replay_key.clone(), replay_key]
    );
}

fn assert_mapping_conflict(error: &SdkError, expected_reference: &str) {
    assert_eq!(error.code, "CUSTOMER_ENRICHMENT_PROVIDER_MAPPING_CONFLICT");
    assert!(!error.retryable);
    assert_eq!(error.internal_reference.as_deref(), Some(expected_reference));
    assert!(!format!("{error:?} {error}").contains(SECRET_MARKER));
}

fn adapter(endpoint: String, maximum_response_bytes: usize) -> GovernedProviderAdapter {
    let clock = Arc::new(FixedClock::new(5_000_000_000));
    let config = RegistryHttpTransportConfig::try_new(
        &endpoint,
        [endpoint.clone()],
        Duration::from_secs(1),
        64 * 1024,
        maximum_response_bytes,
    )
    .expect("configure response-contract transport");
    let transport = Arc::new(
        RegistryHttpTransport::try_new(config, clock.clone())
            .expect("build response-contract transport"),
    );
    let secrets = StaticProviderSecretHandleResolver::try_new([ProviderSecretRegistration {
        tenant_id: TenantId::try_new("tenant-a").expect("build tenant id"),
        handle_alias: "registry_primary".to_owned(),
        material: ProviderSecretMaterial::try_new(b"super-secret-provider-token".to_vec())
            .expect("build provider secret"),
    }])
    .expect("build provider secret resolver");
    GovernedProviderAdapter::new(
        Arc::new(secrets),
        Arc::new(
            FixedWindowProviderQuota::try_new(10, 60_000_000_000, clock.clone())
                .expect("build provider quota"),
        ),
        Arc::new(
            ConsecutiveFailureProviderCircuitBreaker::try_new(
                3,
                60_000_000_000,
                clock,
            )
            .expect("build provider circuit"),
        ),
        transport,
    )
}

fn provider_request() -> crm_customer_enrichment::ProviderDispatchRequest {
    let profile = ProviderProfileVersion::publish(ProviderProfileDraft {
        provider_key: "company_registry_response_contract".to_owned(),
        adapter_kind: REGISTRY_HTTP_ADAPTER_KIND.to_owned(),
        adapter_contract_version: REGISTRY_HTTP_ADAPTER_CONTRACT_VERSION.to_owned(),
        supported_target_fields: vec![TargetField::PartyDisplayName],
        purpose_codes: vec!["customer_profile_enrichment".to_owned()],
        license_id: "Registry response-contract licence".to_owned(),
        permitted_use_class: "customer_master_review".to_owned(),
        residency_region: "eu".to_owned(),
        retention_days: 30,
        raw_payload_policy: RawPayloadPolicy::DigestOnly,
        credential_handle_aliases: vec!["registry_primary".to_owned()],
        effective_at_unix_ms: 1,
        expires_at_unix_ms: Some(20_000),
    })
    .expect("publish provider profile");
    let mapping = MappingVersion::publish(MappingDraft {
        mapping_key: "party_display_name_response_contract".to_owned(),
        provider_profile_version_id: profile.version_id().clone(),
        provider_response_field_path: "organization.legal_name".to_owned(),
        target_field: TargetField::PartyDisplayName,
        normalization: MappingNormalization::CanonicalPartyDisplayNameV1,
        maximum_suggestions_per_response: 1,
        confidence_required: true,
    })
    .expect("publish mapping");
    let actor = ActorId::try_new("provider-worker").expect("build actor id");
    let mut request = EnrichmentRequest::create(EnrichmentRequestDraft {
        tenant_id: TenantId::try_new("tenant-a").expect("build tenant id"),
        requested_by: actor.clone(),
        idempotency_key: IdempotencyKey::try_new("registry-http-response-contract-request")
            .expect("build idempotency key"),
        target: TargetSnapshot::try_new("party-a", 7, TargetField::PartyDisplayName)
            .expect("build target snapshot"),
        provider_profile_version_id: profile.version_id().clone(),
        mapping_version_id: mapping.version_id().clone(),
        requested_fields: vec![TargetField::PartyDisplayName],
        policy_evidence: RequestPolicyEvidence::try_new(
            "customer_profile_enrichment",
            "legitimate_interest",
            None,
            "provider-response-contract-policy-v1",
        )
        .expect("build policy evidence"),
        created_at_unix_ms: 10,
        deadline_at_unix_ms: 10_000,
        expires_at_unix_ms: 20_000,
    })
    .expect("create enrichment request");
    prepare_provider_dispatch_attempt(
        &mut request,
        ProviderDispatchExpectation {
            status: EnrichmentRequestStatus::Created,
            retry_generation: 0,
        },
        &profile,
        &PartySnapshot {
            party_id: RecordId::try_new("party-a").expect("build party id"),
            display_name: "Example Company".to_owned(),
            resource_version: 7,
            observed_at_unix_ms: 15,
        },
        actor,
        20,
    )
    .expect("prepare provider dispatch attempt")
}
