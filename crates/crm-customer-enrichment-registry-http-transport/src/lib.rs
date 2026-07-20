#![forbid(unsafe_code)]

//! Concrete bounded HTTP transport for the first Customer Enrichment provider coordinate.

use crm_customer_enrichment::{
    ProviderDispatchRequest, ProviderResponseClass, SanitizedProviderResponse,
};
use crm_customer_enrichment_provider_registry::{
    ProviderTransportFailure, ProviderTransportFailureClass, ProviderTransportPort,
    ProviderTransportRequest,
};
use crm_module_sdk::{Clock, ErrorCategory, PortFuture, SdkError};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderValue};
use reqwest::{Client, StatusCode, Url, redirect::Policy};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

pub const REGISTRY_HTTP_TRANSPORT_KEY: &str = "registry_http";
pub const REGISTRY_HTTP_ADAPTER_KIND: &str = "registry_http_v1";
pub const REGISTRY_HTTP_ADAPTER_CONTRACT_VERSION: &str = "1.0.0";
pub const REGISTRY_HTTP_PATH: &str = "/v1/enrich";

const RESPONSE_SCHEMA_VERSION: &str = "crm.customer-enrichment.registry-http.response/v1";
const REQUEST_SCHEMA_VERSION: &str = "crm.customer-enrichment.registry-http.request/v1";
const MAXIMUM_ENDPOINTS: usize = 16;
const MAXIMUM_BODY_BYTES: usize = 1_048_576;
const MAXIMUM_CORRELATION_BYTES: usize = 180;
const MAXIMUM_EVIDENCE_REFERENCE_BYTES: usize = 240;
const MAXIMUM_SAFE_CODE_BYTES: usize = 80;
const MAXIMUM_METERED_UNITS: u64 = 1_000_000_000;

#[derive(Clone)]
pub struct RegistryHttpTransportConfig {
    endpoint: Url,
    timeout: Duration,
    maximum_request_bytes: usize,
    maximum_response_bytes: usize,
}

impl RegistryHttpTransportConfig {
    pub fn try_new(
        endpoint: impl AsRef<str>,
        allowed_endpoints: impl IntoIterator<Item = String>,
        timeout: Duration,
        maximum_request_bytes: usize,
        maximum_response_bytes: usize,
    ) -> Result<Self, SdkError> {
        if timeout.is_zero()
            || maximum_request_bytes == 0
            || maximum_response_bytes == 0
            || maximum_request_bytes > MAXIMUM_BODY_BYTES
            || maximum_response_bytes > MAXIMUM_BODY_BYTES
        {
            return Err(configuration_invalid("transport bounds are invalid"));
        }
        let endpoint = parse_fixed_endpoint(endpoint.as_ref())?;
        let allowed = allowed_endpoints
            .into_iter()
            .map(|value| parse_fixed_endpoint(&value))
            .collect::<Result<Vec<_>, _>>()?;
        if allowed.is_empty()
            || allowed.len() > MAXIMUM_ENDPOINTS
            || !allowed.iter().any(|candidate| candidate == &endpoint)
        {
            return Err(configuration_invalid(
                "configured endpoint is not in the explicit allowlist",
            ));
        }
        Ok(Self {
            endpoint,
            timeout,
            maximum_request_bytes,
            maximum_response_bytes,
        })
    }

    pub fn endpoint(&self) -> &Url {
        &self.endpoint
    }
}

impl fmt::Debug for RegistryHttpTransportConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RegistryHttpTransportConfig")
            .field("endpoint", &self.endpoint)
            .field("timeout", &self.timeout)
            .field("maximum_request_bytes", &self.maximum_request_bytes)
            .field("maximum_response_bytes", &self.maximum_response_bytes)
            .finish()
    }
}

#[derive(Clone)]
pub struct RegistryHttpTransport {
    config: RegistryHttpTransportConfig,
    client: Client,
    clock: Arc<dyn Clock>,
}

impl RegistryHttpTransport {
    pub fn try_new(
        config: RegistryHttpTransportConfig,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, SdkError> {
        let client = Client::builder()
            .redirect(Policy::none())
            .no_proxy()
            .connect_timeout(config.timeout)
            .timeout(config.timeout)
            .build()
            .map_err(|_| configuration_invalid("HTTP client construction failed"))?;
        Ok(Self {
            config,
            client,
            clock,
        })
    }
}

impl fmt::Debug for RegistryHttpTransport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RegistryHttpTransport")
            .field("config", &self.config)
            .field("client", &"bounded reqwest client")
            .field("clock", &"dyn Clock")
            .finish()
    }
}

impl ProviderTransportPort for RegistryHttpTransport {
    fn dispatch<'a>(
        &'a self,
        request: ProviderTransportRequest,
    ) -> PortFuture<'a, Result<SanitizedProviderResponse, ProviderTransportFailure>> {
        Box::pin(async move {
            let provider_request = request.provider_request();
            let timeout =
                effective_timeout(provider_request, self.clock.as_ref(), self.config.timeout)?;
            let body = serialize_request(provider_request, self.config.maximum_request_bytes)?;
            let secret = exactly_one_secret(&request)?;
            let authorization = bearer_header(secret.expose_to_transport())?;
            let response = self
                .client
                .post(self.config.endpoint.clone())
                .header(AUTHORIZATION, authorization)
                .header(CONTENT_TYPE, "application/json")
                .header(
                    "idempotency-key",
                    &provider_request.provider_idempotency_key,
                )
                .timeout(timeout)
                .body(body)
                .send()
                .await
                .map_err(map_reqwest_failure)?;
            let status = response.status();
            if !status.is_success() {
                return Err(map_http_status(status));
            }
            if response
                .content_length()
                .is_some_and(|length| length > self.config.maximum_response_bytes as u64)
            {
                return Err(failure(
                    ProviderTransportFailureClass::MappingConflict,
                    "response_too_large",
                ));
            }
            let body = read_bounded_body(response, self.config.maximum_response_bytes).await?;
            parse_response(provider_request, self.clock.as_ref(), &body)
        })
    }
}

#[derive(Serialize)]
struct RegistryRequestBody<'a> {
    schema_version: &'static str,
    tenant_id: &'a str,
    enrichment_request_id: &'a str,
    provider_profile_version_id: &'a str,
    mapping_version_id: &'a str,
    retry_generation: u32,
    party_id: &'a str,
    party_resource_version: i64,
    current_display_name: &'a str,
    provider_idempotency_key: &'a str,
    deadline_at_unix_ms: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryResponseBody {
    schema_version: String,
    replay_key: String,
    provider_correlation_id: Option<String>,
    response_class: String,
    provider_observed_at_unix_ms: Option<i64>,
    metered_units: u64,
    protected_evidence_reference: Option<String>,
    safe_provider_code: Option<String>,
}

fn serialize_request(
    request: &ProviderDispatchRequest,
    maximum_bytes: usize,
) -> Result<Vec<u8>, ProviderTransportFailure> {
    let body = serde_json::to_vec(&RegistryRequestBody {
        schema_version: REQUEST_SCHEMA_VERSION,
        tenant_id: request.tenant_id.as_str(),
        enrichment_request_id: request.enrichment_request_id.as_str(),
        provider_profile_version_id: request.provider_profile_version_id.as_str(),
        mapping_version_id: request.mapping_version_id.as_str(),
        retry_generation: request.retry_generation,
        party_id: request.party_id.as_str(),
        party_resource_version: request.party_resource_version,
        current_display_name: &request.current_display_name,
        provider_idempotency_key: &request.provider_idempotency_key,
        deadline_at_unix_ms: request.deadline_at_unix_ms,
    })
    .map_err(|_| {
        failure(
            ProviderTransportFailureClass::Terminal,
            "request_encoding_failed",
        )
    })?;
    if body.len() > maximum_bytes {
        return Err(failure(
            ProviderTransportFailureClass::Terminal,
            "request_too_large",
        ));
    }
    Ok(body)
}

fn exactly_one_secret(
    request: &ProviderTransportRequest,
) -> Result<
    &crm_customer_enrichment_provider_registry::ProviderSecretMaterial,
    ProviderTransportFailure,
> {
    let aliases = request.credential_aliases().collect::<Vec<_>>();
    if aliases.len() != 1 {
        return Err(failure(
            ProviderTransportFailureClass::Terminal,
            "credential_set_invalid",
        ));
    }
    request.credential(aliases[0]).ok_or_else(|| {
        failure(
            ProviderTransportFailureClass::Terminal,
            "credential_unavailable",
        )
    })
}

fn bearer_header(secret: &[u8]) -> Result<HeaderValue, ProviderTransportFailure> {
    let mut value = Vec::with_capacity(secret.len() + 7);
    value.extend_from_slice(b"Bearer ");
    value.extend_from_slice(secret);
    HeaderValue::from_bytes(&value).map_err(|_| {
        failure(
            ProviderTransportFailureClass::Terminal,
            "credential_format_invalid",
        )
    })
}

async fn read_bounded_body(
    mut response: reqwest::Response,
    maximum_bytes: usize,
) -> Result<Vec<u8>, ProviderTransportFailure> {
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(map_reqwest_failure)? {
        if body.len().saturating_add(chunk.len()) > maximum_bytes {
            return Err(failure(
                ProviderTransportFailureClass::MappingConflict,
                "response_too_large",
            ));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn parse_response(
    request: &ProviderDispatchRequest,
    clock: &dyn Clock,
    body: &[u8],
) -> Result<SanitizedProviderResponse, ProviderTransportFailure> {
    let parsed = serde_json::from_slice::<RegistryResponseBody>(body).map_err(|_| {
        failure(
            ProviderTransportFailureClass::MappingConflict,
            "response_shape_invalid",
        )
    })?;
    if parsed.schema_version != RESPONSE_SCHEMA_VERSION
        || parsed.replay_key != request.provider_idempotency_key
    {
        return Err(failure(
            ProviderTransportFailureClass::MappingConflict,
            "response_lineage_conflict",
        ));
    }
    if parsed.metered_units > MAXIMUM_METERED_UNITS
        || !valid_optional_text(
            parsed.provider_correlation_id.as_deref(),
            MAXIMUM_CORRELATION_BYTES,
        )
        || !valid_optional_text(
            parsed.protected_evidence_reference.as_deref(),
            MAXIMUM_EVIDENCE_REFERENCE_BYTES,
        )
        || !valid_optional_safe_code(parsed.safe_provider_code.as_deref())
        || parsed
            .provider_observed_at_unix_ms
            .is_some_and(|value| value < 0)
    {
        return Err(failure(
            ProviderTransportFailureClass::MappingConflict,
            "response_value_invalid",
        ));
    }
    let response_class = match parsed.response_class.as_str() {
        "success" => ProviderResponseClass::Success,
        "no_match" => ProviderResponseClass::NoMatch,
        "retryable_failure" => ProviderResponseClass::RetryableFailure,
        "terminal_failure" => ProviderResponseClass::TerminalFailure,
        _ => {
            return Err(failure(
                ProviderTransportFailureClass::MappingConflict,
                "response_class_invalid",
            ));
        }
    };
    let retrieved_at_unix_ms = clock.now_unix_nanos() / 1_000_000;
    if retrieved_at_unix_ms < 0 {
        return Err(failure(
            ProviderTransportFailureClass::Retryable,
            "provider_clock_invalid",
        ));
    }
    let digest: [u8; 32] = Sha256::digest(body).into();
    Ok(SanitizedProviderResponse {
        replay_key: parsed.replay_key,
        provider_correlation_id: parsed.provider_correlation_id,
        response_class,
        canonical_response_digest: digest,
        provider_observed_at_unix_ms: parsed.provider_observed_at_unix_ms,
        retrieved_at_unix_ms,
        metered_units: parsed.metered_units,
        protected_evidence_reference: parsed.protected_evidence_reference,
        safe_provider_code: parsed.safe_provider_code,
    })
}

fn effective_timeout(
    request: &ProviderDispatchRequest,
    clock: &dyn Clock,
    configured: Duration,
) -> Result<Duration, ProviderTransportFailure> {
    let now_unix_ms = clock.now_unix_nanos() / 1_000_000;
    let remaining = request
        .deadline_at_unix_ms
        .checked_sub(now_unix_ms)
        .filter(|value| *value > 0)
        .ok_or_else(|| {
            failure(
                ProviderTransportFailureClass::Retryable,
                "provider_deadline_exceeded",
            )
        })?;
    let remaining = Duration::from_millis(u64::try_from(remaining).map_err(|_| {
        failure(
            ProviderTransportFailureClass::Retryable,
            "provider_deadline_invalid",
        )
    })?);
    Ok(configured.min(remaining))
}

fn map_reqwest_failure(error: reqwest::Error) -> ProviderTransportFailure {
    let safe_code = if error.is_timeout() {
        "upstream_timeout"
    } else if error.is_connect() {
        "upstream_connect_failed"
    } else {
        "upstream_transport_failed"
    };
    failure(ProviderTransportFailureClass::Retryable, safe_code)
}

fn map_http_status(status: StatusCode) -> ProviderTransportFailure {
    if status == StatusCode::TOO_MANY_REQUESTS {
        return failure(
            ProviderTransportFailureClass::QuotaExceeded,
            "upstream_quota_exceeded",
        );
    }
    if status == StatusCode::CONFLICT {
        return failure(
            ProviderTransportFailureClass::MappingConflict,
            "upstream_mapping_conflict",
        );
    }
    if status == StatusCode::REQUEST_TIMEOUT
        || status == StatusCode::TOO_EARLY
        || status.is_server_error()
    {
        return failure(
            ProviderTransportFailureClass::Retryable,
            "upstream_retryable_status",
        );
    }
    if status.is_redirection() {
        return failure(
            ProviderTransportFailureClass::Terminal,
            "upstream_redirect_rejected",
        );
    }
    failure(
        ProviderTransportFailureClass::Terminal,
        "upstream_terminal_status",
    )
}

fn parse_fixed_endpoint(value: &str) -> Result<Url, SdkError> {
    let url = Url::parse(value).map_err(|_| configuration_invalid("endpoint URL is invalid"))?;
    let host = url
        .host_str()
        .ok_or_else(|| configuration_invalid("endpoint host is missing"))?;
    let loopback = matches!(host, "localhost" | "127.0.0.1" | "::1");
    if (url.scheme() != "https" && !(url.scheme() == "http" && loopback))
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != REGISTRY_HTTP_PATH
    {
        return Err(configuration_invalid(
            "endpoint must be an exact HTTPS URL or loopback test URL",
        ));
    }
    Ok(url)
}

fn valid_optional_text(value: Option<&str>, maximum_bytes: usize) -> bool {
    value.is_none_or(|value| {
        !value.is_empty()
            && value.len() <= maximum_bytes
            && value.trim() == value
            && !value.chars().any(char::is_control)
    })
}

fn valid_optional_safe_code(value: Option<&str>) -> bool {
    value.is_none_or(|value| {
        !value.is_empty()
            && value.len() <= MAXIMUM_SAFE_CODE_BYTES
            && value.trim() == value
            && value.chars().all(|character| {
                character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
            })
    })
}

fn failure(
    class: ProviderTransportFailureClass,
    safe_code: &'static str,
) -> ProviderTransportFailure {
    ProviderTransportFailure::try_new(class, Some(safe_code.to_owned()))
        .unwrap_or_else(|_| unreachable!("static provider failure codes are canonical"))
}

fn configuration_invalid(reference: &'static str) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_REGISTRY_HTTP_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The registry HTTP provider transport is configured incorrectly.",
    )
    .with_internal_reference(reference)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Bytes;
    use axum::extract::State;
    use axum::http::{HeaderMap, StatusCode as AxumStatusCode};
    use axum::response::IntoResponse;
    use axum::routing::post;
    use axum::{Json, Router};
    use crm_customer_enrichment::{
        EnrichmentRequest, EnrichmentRequestDraft, EnrichmentRequestStatus, MappingDraft,
        MappingNormalization, MappingVersion, PartySnapshot, ProviderDispatchExpectation,
        ProviderDispatchPort, ProviderProfileDraft, ProviderProfileVersion, RawPayloadPolicy,
        RequestPolicyEvidence, TargetField, TargetSnapshot, prepare_provider_dispatch_attempt,
    };
    use crm_customer_enrichment_provider_registry::{
        ConsecutiveFailureProviderCircuitBreaker, FixedWindowProviderQuota,
        GovernedProviderAdapter, ProviderSecretMaterial, ProviderSecretRegistration,
        StaticProviderSecretHandleResolver,
    };
    use crm_module_sdk::testing::FixedClock;
    use crm_module_sdk::{ActorId, IdempotencyKey, RecordId, TenantId};
    use serde_json::{Value, json};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::net::TcpListener;
    use tokio::time::sleep;

    #[derive(Clone)]
    struct MockState {
        status: AxumStatusCode,
        delay: Duration,
        malformed: bool,
        calls: Arc<AtomicUsize>,
    }

    async fn mock_provider(
        State(state): State<MockState>,
        headers: HeaderMap,
        body: Bytes,
    ) -> impl IntoResponse {
        state.calls.fetch_add(1, Ordering::SeqCst);
        if !state.delay.is_zero() {
            sleep(state.delay).await;
        }
        if state.status != AxumStatusCode::OK {
            return (state.status, "sanitized upstream body").into_response();
        }
        if headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            != Some("Bearer super-secret-provider-token")
        {
            return (AxumStatusCode::UNAUTHORIZED, "unauthorized").into_response();
        }
        if state.malformed {
            return (AxumStatusCode::OK, "{").into_response();
        }
        let request: Value = match serde_json::from_slice(&body) {
            Ok(value) => value,
            Err(_) => return (AxumStatusCode::BAD_REQUEST, "bad request").into_response(),
        };
        let Some(replay_key) = request
            .get("provider_idempotency_key")
            .and_then(Value::as_str)
        else {
            return (AxumStatusCode::BAD_REQUEST, "missing replay key").into_response();
        };
        Json(json!({
            "schema_version": RESPONSE_SCHEMA_VERSION,
            "replay_key": replay_key,
            "provider_correlation_id": "registry-correlation-1",
            "response_class": "success",
            "provider_observed_at_unix_ms": 4_000,
            "metered_units": 1,
            "protected_evidence_reference": null,
            "safe_provider_code": "success"
        }))
        .into_response()
    }

    async fn spawn_provider(state: MockState) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let router = Router::new()
            .route(REGISTRY_HTTP_PATH, post(mock_provider))
            .with_state(state);
        tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        format!("http://{address}{REGISTRY_HTTP_PATH}")
    }

    fn provider_request() -> ProviderDispatchRequest {
        let profile = ProviderProfileVersion::publish(ProviderProfileDraft {
            provider_key: "company_registry".to_owned(),
            adapter_kind: REGISTRY_HTTP_ADAPTER_KIND.to_owned(),
            adapter_contract_version: REGISTRY_HTTP_ADAPTER_CONTRACT_VERSION.to_owned(),
            supported_target_fields: vec![TargetField::PartyDisplayName],
            purpose_codes: vec!["customer_profile_enrichment".to_owned()],
            license_id: "Registry test licence".to_owned(),
            permitted_use_class: "customer_master_review".to_owned(),
            residency_region: "eu".to_owned(),
            retention_days: 30,
            raw_payload_policy: RawPayloadPolicy::DigestOnly,
            credential_handle_aliases: vec!["registry_primary".to_owned()],
            effective_at_unix_ms: 1,
            expires_at_unix_ms: Some(20_000),
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
        let actor = ActorId::try_new("provider-worker").unwrap();
        let mut request = EnrichmentRequest::create(EnrichmentRequestDraft {
            tenant_id: TenantId::try_new("tenant-a").unwrap(),
            requested_by: actor.clone(),
            idempotency_key: IdempotencyKey::try_new("registry-http-request").unwrap(),
            target: TargetSnapshot::try_new("party-a", 7, TargetField::PartyDisplayName).unwrap(),
            provider_profile_version_id: profile.version_id().clone(),
            mapping_version_id: mapping.version_id().clone(),
            requested_fields: vec![TargetField::PartyDisplayName],
            policy_evidence: RequestPolicyEvidence::try_new(
                "customer_profile_enrichment",
                "legitimate_interest",
                None,
                "provider-adapter-policy-v1",
            )
            .unwrap(),
            created_at_unix_ms: 10,
            deadline_at_unix_ms: 10_000,
            expires_at_unix_ms: 20_000,
        })
        .unwrap();
        prepare_provider_dispatch_attempt(
            &mut request,
            ProviderDispatchExpectation {
                status: EnrichmentRequestStatus::Created,
                retry_generation: 0,
            },
            &profile,
            &PartySnapshot {
                party_id: RecordId::try_new("party-a").unwrap(),
                display_name: "Example Company".to_owned(),
                resource_version: 7,
                observed_at_unix_ms: 15,
            },
            actor,
            20,
        )
        .unwrap()
    }

    fn adapter(
        endpoint: String,
        timeout: Duration,
        maximum_attempts: u32,
        failure_threshold: u32,
        clock: Arc<FixedClock>,
    ) -> GovernedProviderAdapter {
        let config = RegistryHttpTransportConfig::try_new(
            &endpoint,
            [endpoint.clone()],
            timeout,
            64 * 1024,
            64 * 1024,
        )
        .unwrap();
        let transport = Arc::new(RegistryHttpTransport::try_new(config, clock.clone()).unwrap());
        let secrets = StaticProviderSecretHandleResolver::try_new([ProviderSecretRegistration {
            tenant_id: TenantId::try_new("tenant-a").unwrap(),
            handle_alias: "registry_primary".to_owned(),
            material: ProviderSecretMaterial::try_new(b"super-secret-provider-token".to_vec())
                .unwrap(),
        }])
        .unwrap();
        GovernedProviderAdapter::new(
            Arc::new(secrets),
            Arc::new(
                FixedWindowProviderQuota::try_new(maximum_attempts, 60_000_000_000, clock.clone())
                    .unwrap(),
            ),
            Arc::new(
                ConsecutiveFailureProviderCircuitBreaker::try_new(
                    failure_threshold,
                    60_000_000_000,
                    clock,
                )
                .unwrap(),
            ),
            transport,
        )
    }

    #[test]
    fn configuration_requires_an_exact_allowlisted_endpoint() {
        assert!(
            RegistryHttpTransportConfig::try_new(
                "https://registry.example/v1/enrich",
                ["https://other.example/v1/enrich".to_owned()],
                Duration::from_secs(1),
                1024,
                1024,
            )
            .is_err()
        );
        assert!(
            RegistryHttpTransportConfig::try_new(
                "https://registry.example/other",
                ["https://registry.example/other".to_owned()],
                Duration::from_secs(1),
                1024,
                1024,
            )
            .is_err()
        );
    }

    #[tokio::test]
    async fn successful_http_dispatch_returns_only_sanitized_evidence() {
        let calls = Arc::new(AtomicUsize::new(0));
        let endpoint = spawn_provider(MockState {
            status: AxumStatusCode::OK,
            delay: Duration::ZERO,
            malformed: false,
            calls: calls.clone(),
        })
        .await;
        let request = provider_request();
        let replay_key = request.provider_idempotency_key.clone();
        let response = adapter(
            endpoint,
            Duration::from_secs(1),
            10,
            3,
            Arc::new(FixedClock::new(5_000_000_000)),
        )
        .dispatch(request)
        .await
        .unwrap();
        assert_eq!(response.replay_key, replay_key);
        assert_eq!(response.response_class, ProviderResponseClass::Success);
        assert!(
            response
                .canonical_response_digest
                .iter()
                .any(|byte| *byte != 0)
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        let formatted = format!("{response:?}");
        assert!(!formatted.contains("super-secret-provider-token"));
    }

    #[tokio::test]
    async fn malformed_response_is_a_mapping_conflict() {
        let endpoint = spawn_provider(MockState {
            status: AxumStatusCode::OK,
            delay: Duration::ZERO,
            malformed: true,
            calls: Arc::new(AtomicUsize::new(0)),
        })
        .await;
        let error = adapter(
            endpoint,
            Duration::from_secs(1),
            10,
            3,
            Arc::new(FixedClock::new(5_000_000_000)),
        )
        .dispatch(provider_request())
        .await
        .unwrap_err();
        assert_eq!(error.code, "CUSTOMER_ENRICHMENT_PROVIDER_MAPPING_CONFLICT");
        assert!(!format!("{error:?} {error}").contains("{"));
    }

    #[tokio::test]
    async fn timeout_is_retryable_and_opens_the_exact_coordinate_circuit() {
        let calls = Arc::new(AtomicUsize::new(0));
        let endpoint = spawn_provider(MockState {
            status: AxumStatusCode::OK,
            delay: Duration::from_millis(100),
            malformed: false,
            calls: calls.clone(),
        })
        .await;
        let adapter = adapter(
            endpoint,
            Duration::from_millis(10),
            10,
            1,
            Arc::new(FixedClock::new(5_000_000_000)),
        );
        let first = adapter.dispatch(provider_request()).await.unwrap_err();
        assert_eq!(first.code, "CUSTOMER_ENRICHMENT_PROVIDER_RETRYABLE_FAILURE");
        assert!(first.retryable);
        let second = adapter.dispatch(provider_request()).await.unwrap_err();
        assert_eq!(second.code, "CUSTOMER_ENRICHMENT_PROVIDER_CIRCUIT_OPEN");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn quota_is_enforced_before_a_second_http_attempt() {
        let calls = Arc::new(AtomicUsize::new(0));
        let endpoint = spawn_provider(MockState {
            status: AxumStatusCode::OK,
            delay: Duration::ZERO,
            malformed: false,
            calls: calls.clone(),
        })
        .await;
        let adapter = adapter(
            endpoint,
            Duration::from_secs(1),
            1,
            3,
            Arc::new(FixedClock::new(5_000_000_000)),
        );
        adapter.dispatch(provider_request()).await.unwrap();
        let second = adapter.dispatch(provider_request()).await.unwrap_err();
        assert_eq!(second.code, "CUSTOMER_ENRICHMENT_PROVIDER_QUOTA_EXCEEDED");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn redirects_are_not_followed() {
        let endpoint = spawn_provider(MockState {
            status: AxumStatusCode::FOUND,
            delay: Duration::ZERO,
            malformed: false,
            calls: Arc::new(AtomicUsize::new(0)),
        })
        .await;
        let error = adapter(
            endpoint,
            Duration::from_secs(1),
            10,
            3,
            Arc::new(FixedClock::new(5_000_000_000)),
        )
        .dispatch(provider_request())
        .await
        .unwrap_err();
        assert_eq!(error.code, "CUSTOMER_ENRICHMENT_PROVIDER_TERMINAL_FAILURE");
        assert_eq!(
            error.internal_reference.as_deref(),
            Some("provider_code:upstream_redirect_rejected")
        );
    }
}
