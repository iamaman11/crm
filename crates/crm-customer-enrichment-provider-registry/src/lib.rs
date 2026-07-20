#![forbid(unsafe_code)]

//! Infrastructure-owned exact provider-adapter registry and governed adapter shell for Customer
//! Enrichment.
//!
//! The registry is immutable after construction. It resolves the complete adapter kind/version
//! coordinate and never falls back to another version, a default adapter or kind-only matching.
//! The governed adapter shell keeps secret resolution, quota enforcement, circuit isolation and
//! provider-specific transport outside the pure business core. Raw credentials and provider bodies
//! are intentionally absent from every result and error type crossing this boundary.

use crm_customer_enrichment::{
    ProviderAdapterCoordinate, ProviderAdapterRegistryPort, ProviderDispatchPort,
    ProviderDispatchRequest, ProviderResponseClass, SanitizedProviderResponse,
};
use crm_module_sdk::{Clock, ErrorCategory, PortFuture, SdkError, TenantId};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::{Arc, Mutex};

const MAX_SECRET_BYTES: usize = 16 * 1024;
const MAX_SAFE_PROVIDER_CODE_BYTES: usize = 80;
const MAX_CREDENTIAL_HANDLE_ALIASES: usize = 8;
const DETERMINISTIC_RESPONSE_DOMAIN: &[u8] =
    b"crm.customer-enrichment.deterministic-provider-response/v1";

#[derive(Clone)]
enum RegistryEntry {
    Enabled(Arc<dyn ProviderDispatchPort>),
    Disabled,
}

/// One exact adapter registration supplied while constructing the immutable registry.
#[derive(Clone)]
pub struct ProviderAdapterRegistration {
    coordinate: ProviderAdapterCoordinate,
    entry: RegistryEntry,
}

impl ProviderAdapterRegistration {
    pub fn enabled<A>(coordinate: ProviderAdapterCoordinate, adapter: A) -> Self
    where
        A: ProviderDispatchPort + 'static,
    {
        Self {
            coordinate,
            entry: RegistryEntry::Enabled(Arc::new(adapter)),
        }
    }

    pub fn enabled_arc(
        coordinate: ProviderAdapterCoordinate,
        adapter: Arc<dyn ProviderDispatchPort>,
    ) -> Self {
        Self {
            coordinate,
            entry: RegistryEntry::Enabled(adapter),
        }
    }

    pub fn disabled(coordinate: ProviderAdapterCoordinate) -> Self {
        Self {
            coordinate,
            entry: RegistryEntry::Disabled,
        }
    }

    pub fn coordinate(&self) -> &ProviderAdapterCoordinate {
        &self.coordinate
    }
}

impl fmt::Debug for ProviderAdapterRegistration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderAdapterRegistration")
            .field("coordinate", &self.coordinate)
            .field(
                "state",
                &match &self.entry {
                    RegistryEntry::Enabled(_) => "enabled",
                    RegistryEntry::Disabled => "disabled",
                },
            )
            .finish()
    }
}

/// Immutable exact-coordinate registry used by provider-dispatch workers.
#[derive(Clone, Default)]
pub struct ExactProviderAdapterRegistry {
    entries: BTreeMap<ProviderAdapterCoordinate, RegistryEntry>,
}

impl ExactProviderAdapterRegistry {
    pub fn try_new(
        registrations: impl IntoIterator<Item = ProviderAdapterRegistration>,
    ) -> Result<Self, SdkError> {
        let mut entries = BTreeMap::new();
        for registration in registrations {
            let coordinate = registration.coordinate;
            if entries
                .insert(coordinate.clone(), registration.entry)
                .is_some()
            {
                return Err(duplicate_registration(&coordinate));
            }
        }
        Ok(Self { entries })
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Resolves one complete adapter coordinate or fails closed.
    pub fn resolve_exact(
        &self,
        coordinate: &ProviderAdapterCoordinate,
    ) -> Result<Arc<dyn ProviderDispatchPort>, SdkError> {
        match self.entries.get(coordinate) {
            Some(RegistryEntry::Enabled(adapter)) => Ok(adapter.clone()),
            Some(RegistryEntry::Disabled) => Err(adapter_disabled(coordinate)),
            None => Err(adapter_unavailable(coordinate)),
        }
    }
}

impl fmt::Debug for ExactProviderAdapterRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExactProviderAdapterRegistry")
            .field("coordinates", &self.entries.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl ProviderAdapterRegistryPort for ExactProviderAdapterRegistry {
    fn dispatch_exact<'a>(
        &'a self,
        request: ProviderDispatchRequest,
    ) -> PortFuture<'a, Result<SanitizedProviderResponse, SdkError>> {
        let adapter = self.resolve_exact(&request.adapter_coordinate);
        Box::pin(async move {
            let adapter = adapter?;
            adapter.dispatch(request).await
        })
    }
}

/// Secret material resolved immediately before provider I/O.
///
/// The bytes are deliberately redacted from `Debug` and are never exposed to the module core.
#[derive(Clone, PartialEq, Eq)]
pub struct ProviderSecretMaterial {
    bytes: Arc<[u8]>,
}

impl ProviderSecretMaterial {
    pub fn try_new(bytes: impl Into<Vec<u8>>) -> Result<Self, SdkError> {
        let bytes = bytes.into();
        if bytes.is_empty() || bytes.len() > MAX_SECRET_BYTES {
            return Err(provider_configuration_invalid(
                "provider secret material must be non-empty and bounded",
            ));
        }
        Ok(Self {
            bytes: Arc::from(bytes),
        })
    }

    /// Exposes secret bytes only to an infrastructure transport implementation.
    pub fn expose_to_transport(&self) -> &[u8] {
        &self.bytes
    }
}

impl fmt::Debug for ProviderSecretMaterial {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderSecretMaterial")
            .field("state", &"redacted")
            .finish()
    }
}

/// One immutable tenant-bound secret-handle registration.
#[derive(Clone)]
pub struct ProviderSecretRegistration {
    pub tenant_id: TenantId,
    pub handle_alias: String,
    pub material: ProviderSecretMaterial,
}

impl fmt::Debug for ProviderSecretRegistration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderSecretRegistration")
            .field("tenant_id", &self.tenant_id)
            .field("handle_alias", &self.handle_alias)
            .field("material", &"redacted")
            .finish()
    }
}

/// Runtime secret-handle resolver. Implementations must not return secret values in errors.
pub trait ProviderSecretHandleResolverPort: Send + Sync {
    fn resolve<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        handle_alias: &'a str,
    ) -> PortFuture<'a, Result<ProviderSecretMaterial, SdkError>>;
}

/// Immutable explicit secret-handle resolver suitable for configuration-backed composition.
#[derive(Clone, Default)]
pub struct StaticProviderSecretHandleResolver {
    entries: Arc<BTreeMap<(TenantId, String), ProviderSecretMaterial>>,
}

impl StaticProviderSecretHandleResolver {
    pub fn try_new(
        registrations: impl IntoIterator<Item = ProviderSecretRegistration>,
    ) -> Result<Self, SdkError> {
        let mut entries = BTreeMap::new();
        for registration in registrations {
            validate_handle_alias(&registration.handle_alias)?;
            let key = (registration.tenant_id, registration.handle_alias);
            if entries.insert(key.clone(), registration.material).is_some() {
                return Err(provider_configuration_invalid(format!(
                    "duplicate provider secret handle registration for {}:{}",
                    key.0, key.1
                )));
            }
        }
        Ok(Self {
            entries: Arc::new(entries),
        })
    }
}

impl fmt::Debug for StaticProviderSecretHandleResolver {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let handles = self
            .entries
            .keys()
            .map(|(tenant_id, alias)| format!("{tenant_id}:{alias}"))
            .collect::<Vec<_>>();
        formatter
            .debug_struct("StaticProviderSecretHandleResolver")
            .field("handles", &handles)
            .finish()
    }
}

impl ProviderSecretHandleResolverPort for StaticProviderSecretHandleResolver {
    fn resolve<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        handle_alias: &'a str,
    ) -> PortFuture<'a, Result<ProviderSecretMaterial, SdkError>> {
        Box::pin(async move {
            self.entries
                .get(&(tenant_id.clone(), handle_alias.to_owned()))
                .cloned()
                .ok_or_else(provider_secret_unavailable)
        })
    }
}

/// Provider-attempt quota boundary. A successful acquisition is consumed before I/O.
pub trait ProviderQuotaPort: Send + Sync {
    fn acquire(&self, request: &ProviderDispatchRequest) -> Result<(), SdkError>;
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ProviderAttemptCoordinate {
    tenant_id: TenantId,
    adapter_coordinate: ProviderAdapterCoordinate,
}

impl ProviderAttemptCoordinate {
    fn from_request(request: &ProviderDispatchRequest) -> Self {
        Self {
            tenant_id: request.tenant_id.clone(),
            adapter_coordinate: request.adapter_coordinate.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct QuotaWindow {
    started_at_unix_nanos: i64,
    attempts: u32,
}

/// Tenant- and exact-coordinate-scoped fixed-window quota gate.
pub struct FixedWindowProviderQuota {
    maximum_attempts: u32,
    window_nanos: i64,
    clock: Arc<dyn Clock>,
    windows: Mutex<BTreeMap<ProviderAttemptCoordinate, QuotaWindow>>,
}

impl FixedWindowProviderQuota {
    pub fn try_new(
        maximum_attempts: u32,
        window_nanos: i64,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, SdkError> {
        if maximum_attempts == 0 || window_nanos <= 0 {
            return Err(provider_configuration_invalid(
                "provider quota limits must be positive",
            ));
        }
        Ok(Self {
            maximum_attempts,
            window_nanos,
            clock,
            windows: Mutex::new(BTreeMap::new()),
        })
    }
}

impl fmt::Debug for FixedWindowProviderQuota {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FixedWindowProviderQuota")
            .field("maximum_attempts", &self.maximum_attempts)
            .field("window_nanos", &self.window_nanos)
            .finish_non_exhaustive()
    }
}

impl ProviderQuotaPort for FixedWindowProviderQuota {
    fn acquire(&self, request: &ProviderDispatchRequest) -> Result<(), SdkError> {
        let now = valid_now(self.clock.as_ref())?;
        let coordinate = ProviderAttemptCoordinate::from_request(request);
        let mut windows = self.windows.lock().map_err(|_| provider_state_unavailable())?;
        let window = windows.entry(coordinate).or_insert(QuotaWindow {
            started_at_unix_nanos: now,
            attempts: 0,
        });
        let expires_at = window
            .started_at_unix_nanos
            .checked_add(self.window_nanos)
            .ok_or_else(provider_state_unavailable)?;
        if now >= expires_at {
            *window = QuotaWindow {
                started_at_unix_nanos: now,
                attempts: 0,
            };
        }
        if window.attempts >= self.maximum_attempts {
            return Err(provider_quota_exceeded());
        }
        window.attempts = window
            .attempts
            .checked_add(1)
            .ok_or_else(provider_state_unavailable)?;
        Ok(())
    }
}

/// Circuit-isolation boundary around one exact provider attempt coordinate.
pub trait ProviderCircuitBreakerPort: Send + Sync {
    fn before_attempt(&self, request: &ProviderDispatchRequest) -> Result<(), SdkError>;
    fn record_success(&self, request: &ProviderDispatchRequest) -> Result<(), SdkError>;
    fn record_failure(&self, request: &ProviderDispatchRequest) -> Result<(), SdkError>;
}

#[derive(Debug, Clone, Copy, Default)]
struct CircuitState {
    consecutive_failures: u32,
    opened_at_unix_nanos: Option<i64>,
}

/// Exact-coordinate circuit breaker opened by consecutive provider transport failures.
pub struct ConsecutiveFailureProviderCircuitBreaker {
    failure_threshold: u32,
    open_nanos: i64,
    clock: Arc<dyn Clock>,
    states: Mutex<BTreeMap<ProviderAttemptCoordinate, CircuitState>>,
}

impl ConsecutiveFailureProviderCircuitBreaker {
    pub fn try_new(
        failure_threshold: u32,
        open_nanos: i64,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, SdkError> {
        if failure_threshold == 0 || open_nanos <= 0 {
            return Err(provider_configuration_invalid(
                "provider circuit limits must be positive",
            ));
        }
        Ok(Self {
            failure_threshold,
            open_nanos,
            clock,
            states: Mutex::new(BTreeMap::new()),
        })
    }
}

impl fmt::Debug for ConsecutiveFailureProviderCircuitBreaker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConsecutiveFailureProviderCircuitBreaker")
            .field("failure_threshold", &self.failure_threshold)
            .field("open_nanos", &self.open_nanos)
            .finish_non_exhaustive()
    }
}

impl ProviderCircuitBreakerPort for ConsecutiveFailureProviderCircuitBreaker {
    fn before_attempt(&self, request: &ProviderDispatchRequest) -> Result<(), SdkError> {
        let now = valid_now(self.clock.as_ref())?;
        let coordinate = ProviderAttemptCoordinate::from_request(request);
        let mut states = self.states.lock().map_err(|_| provider_state_unavailable())?;
        let state = states.entry(coordinate).or_default();
        let Some(opened_at) = state.opened_at_unix_nanos else {
            return Ok(());
        };
        let closes_at = opened_at
            .checked_add(self.open_nanos)
            .ok_or_else(provider_state_unavailable)?;
        if now < closes_at {
            return Err(provider_circuit_open());
        }
        *state = CircuitState::default();
        Ok(())
    }

    fn record_success(&self, request: &ProviderDispatchRequest) -> Result<(), SdkError> {
        let coordinate = ProviderAttemptCoordinate::from_request(request);
        self.states
            .lock()
            .map_err(|_| provider_state_unavailable())?
            .insert(coordinate, CircuitState::default());
        Ok(())
    }

    fn record_failure(&self, request: &ProviderDispatchRequest) -> Result<(), SdkError> {
        let now = valid_now(self.clock.as_ref())?;
        let coordinate = ProviderAttemptCoordinate::from_request(request);
        let mut states = self.states.lock().map_err(|_| provider_state_unavailable())?;
        let state = states.entry(coordinate).or_default();
        state.consecutive_failures = state
            .consecutive_failures
            .checked_add(1)
            .ok_or_else(provider_state_unavailable)?;
        if state.consecutive_failures >= self.failure_threshold {
            state.opened_at_unix_nanos = Some(now);
        }
        Ok(())
    }
}

/// Provider-specific transport input. Secret values are redacted from all formatting.
pub struct ProviderTransportRequest {
    provider_request: ProviderDispatchRequest,
    credentials: BTreeMap<String, ProviderSecretMaterial>,
}

impl ProviderTransportRequest {
    pub fn provider_request(&self) -> &ProviderDispatchRequest {
        &self.provider_request
    }

    pub fn credential(&self, handle_alias: &str) -> Option<&ProviderSecretMaterial> {
        self.credentials.get(handle_alias)
    }

    pub fn credential_aliases(&self) -> impl Iterator<Item = &str> {
        self.credentials.keys().map(String::as_str)
    }
}

impl fmt::Debug for ProviderTransportRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderTransportRequest")
            .field("tenant_id", &self.provider_request.tenant_id)
            .field(
                "enrichment_request_id",
                &self.provider_request.enrichment_request_id,
            )
            .field("adapter_coordinate", &self.provider_request.adapter_coordinate)
            .field(
                "credential_aliases",
                &self.credentials.keys().collect::<Vec<_>>(),
            )
            .field("credentials", &"redacted")
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderTransportFailureClass {
    QuotaExceeded,
    Retryable,
    Terminal,
    MappingConflict,
}

/// Bounded provider transport failure without raw upstream text or payload fragments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderTransportFailure {
    class: ProviderTransportFailureClass,
    safe_provider_code: Option<String>,
}

impl ProviderTransportFailure {
    pub fn try_new(
        class: ProviderTransportFailureClass,
        safe_provider_code: Option<String>,
    ) -> Result<Self, SdkError> {
        if let Some(code) = safe_provider_code.as_deref() {
            validate_safe_provider_code(code)?;
        }
        Ok(Self {
            class,
            safe_provider_code,
        })
    }

    pub const fn class(&self) -> ProviderTransportFailureClass {
        self.class
    }

    pub fn safe_provider_code(&self) -> Option<&str> {
        self.safe_provider_code.as_deref()
    }
}

/// Provider-specific authentication, network I/O, raw payload parsing and sanitization boundary.
pub trait ProviderTransportPort: Send + Sync {
    fn dispatch<'a>(
        &'a self,
        request: ProviderTransportRequest,
    ) -> PortFuture<'a, Result<SanitizedProviderResponse, ProviderTransportFailure>>;
}

/// Concrete policy shell around one provider-specific transport.
#[derive(Clone)]
pub struct GovernedProviderAdapter {
    secrets: Arc<dyn ProviderSecretHandleResolverPort>,
    quota: Arc<dyn ProviderQuotaPort>,
    circuit: Arc<dyn ProviderCircuitBreakerPort>,
    transport: Arc<dyn ProviderTransportPort>,
}

impl GovernedProviderAdapter {
    pub fn new(
        secrets: Arc<dyn ProviderSecretHandleResolverPort>,
        quota: Arc<dyn ProviderQuotaPort>,
        circuit: Arc<dyn ProviderCircuitBreakerPort>,
        transport: Arc<dyn ProviderTransportPort>,
    ) -> Self {
        Self {
            secrets,
            quota,
            circuit,
            transport,
        }
    }
}

impl fmt::Debug for GovernedProviderAdapter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GovernedProviderAdapter")
            .field("secrets", &"dyn ProviderSecretHandleResolverPort")
            .field("quota", &"dyn ProviderQuotaPort")
            .field("circuit", &"dyn ProviderCircuitBreakerPort")
            .field("transport", &"dyn ProviderTransportPort")
            .finish()
    }
}

impl ProviderDispatchPort for GovernedProviderAdapter {
    fn dispatch<'a>(
        &'a self,
        request: ProviderDispatchRequest,
    ) -> PortFuture<'a, Result<SanitizedProviderResponse, SdkError>> {
        Box::pin(async move {
            validate_provider_request(&request)?;
            self.circuit.before_attempt(&request)?;

            let mut credentials = BTreeMap::new();
            for handle_alias in &request.credential_handle_aliases {
                let material = self
                    .secrets
                    .resolve(&request.tenant_id, handle_alias)
                    .await
                    .map_err(|_| provider_secret_unavailable())?;
                credentials.insert(handle_alias.clone(), material);
            }

            self.quota.acquire(&request)?;
            let transport_request = ProviderTransportRequest {
                provider_request: request.clone(),
                credentials,
            };
            match self.transport.dispatch(transport_request).await {
                Ok(response) => {
                    if let Err(error) = validate_transport_response(&request, &response) {
                        self.circuit.record_failure(&request)?;
                        return Err(error);
                    }
                    self.circuit.record_success(&request)?;
                    Ok(response)
                }
                Err(failure) => {
                    if matches!(
                        failure.class(),
                        ProviderTransportFailureClass::Retryable
                            | ProviderTransportFailureClass::Terminal
                    ) {
                        self.circuit.record_failure(&request)?;
                    }
                    Err(transport_failure_error(&failure))
                }
            }
        })
    }
}

/// Deterministic sanitized transport used by fresh-process acceptance.
///
/// It performs no arbitrary network access and returns only bounded canonical evidence. Production
/// vendors supply their own `ProviderTransportPort` implementation behind the same policy shell.
pub struct DeterministicSanitizedProviderTransport {
    clock: Arc<dyn Clock>,
    response_class: ProviderResponseClass,
    metered_units: u64,
}

impl DeterministicSanitizedProviderTransport {
    pub fn new(
        clock: Arc<dyn Clock>,
        response_class: ProviderResponseClass,
        metered_units: u64,
    ) -> Self {
        Self {
            clock,
            response_class,
            metered_units,
        }
    }
}

impl fmt::Debug for DeterministicSanitizedProviderTransport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DeterministicSanitizedProviderTransport")
            .field("response_class", &self.response_class)
            .field("metered_units", &self.metered_units)
            .finish_non_exhaustive()
    }
}

impl ProviderTransportPort for DeterministicSanitizedProviderTransport {
    fn dispatch<'a>(
        &'a self,
        request: ProviderTransportRequest,
    ) -> PortFuture<'a, Result<SanitizedProviderResponse, ProviderTransportFailure>> {
        Box::pin(async move {
            if request.credentials.is_empty()
                || request
                    .credentials
                    .values()
                    .any(|secret| secret.expose_to_transport().is_empty())
            {
                return Err(ProviderTransportFailure {
                    class: ProviderTransportFailureClass::Terminal,
                    safe_provider_code: Some("credential_unavailable".to_owned()),
                });
            }
            let now_unix_nanos = self.clock.now_unix_nanos();
            if now_unix_nanos < 0 {
                return Err(ProviderTransportFailure {
                    class: ProviderTransportFailureClass::Retryable,
                    safe_provider_code: Some("provider_clock_invalid".to_owned()),
                });
            }
            let retrieved_at_unix_ms = now_unix_nanos / 1_000_000;
            let provider_request = request.provider_request();
            let digest = deterministic_response_digest(&[
                provider_request.tenant_id.as_str().as_bytes(),
                provider_request.enrichment_request_id.as_str().as_bytes(),
                provider_request.provider_idempotency_key.as_bytes(),
                &provider_request.retry_generation.to_be_bytes(),
                &provider_request.party_resource_version.to_be_bytes(),
                provider_request.current_display_name.as_bytes(),
            ]);
            let correlation = digest[..12]
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            Ok(SanitizedProviderResponse {
                replay_key: provider_request.provider_idempotency_key.clone(),
                provider_correlation_id: Some(format!("deterministic-{correlation}")),
                response_class: self.response_class,
                canonical_response_digest: digest,
                provider_observed_at_unix_ms: Some(retrieved_at_unix_ms),
                retrieved_at_unix_ms,
                metered_units: self.metered_units,
                protected_evidence_reference: None,
                safe_provider_code: Some(match self.response_class {
                    ProviderResponseClass::Success => "success",
                    ProviderResponseClass::NoMatch => "no_match",
                    ProviderResponseClass::RetryableFailure => "retryable_failure",
                    ProviderResponseClass::TerminalFailure => "terminal_failure",
                }
                .to_owned()),
            })
        })
    }
}

fn deterministic_response_digest(parts: &[&[u8]]) -> [u8; 32] {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x00000100000001b3;
    let mut output = [0_u8; 32];
    for lane in 0..4_u64 {
        let mut hash = FNV_OFFSET ^ lane.wrapping_mul(0x9e3779b97f4a7c15);
        for byte in DETERMINISTIC_RESPONSE_DOMAIN {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        for part in parts {
            for byte in (part.len() as u64).to_be_bytes() {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(FNV_PRIME);
            }
            for byte in *part {
                hash ^= u64::from(*byte);
                hash = hash.wrapping_mul(FNV_PRIME);
            }
        }
        output[(lane as usize) * 8..(lane as usize + 1) * 8]
            .copy_from_slice(&hash.to_be_bytes());
    }
    output
}

fn validate_provider_request(request: &ProviderDispatchRequest) -> Result<(), SdkError> {
    if request.credential_handle_aliases.is_empty()
        || request.credential_handle_aliases.len() > MAX_CREDENTIAL_HANDLE_ALIASES
    {
        return Err(provider_configuration_invalid(
            "provider dispatch requires a bounded non-empty credential handle set",
        ));
    }
    let mut unique = BTreeSet::new();
    for handle_alias in &request.credential_handle_aliases {
        validate_handle_alias(handle_alias)?;
        if !unique.insert(handle_alias) {
            return Err(provider_configuration_invalid(
                "provider dispatch contains a duplicate credential handle alias",
            ));
        }
    }
    Ok(())
}

fn validate_transport_response(
    request: &ProviderDispatchRequest,
    response: &SanitizedProviderResponse,
) -> Result<(), SdkError> {
    if response.replay_key != request.provider_idempotency_key
        || response.canonical_response_digest.iter().all(|byte| *byte == 0)
        || response.retrieved_at_unix_ms < 0
        || response
            .provider_observed_at_unix_ms
            .is_some_and(|value| value < 0 || value > response.retrieved_at_unix_ms)
    {
        return Err(provider_response_invalid());
    }
    if let Some(code) = response.safe_provider_code.as_deref() {
        validate_safe_provider_code(code)?;
    }
    Ok(())
}

fn validate_handle_alias(handle_alias: &str) -> Result<(), SdkError> {
    if handle_alias.is_empty()
        || handle_alias.len() > MAX_SAFE_PROVIDER_CODE_BYTES
        || handle_alias.trim() != handle_alias
        || handle_alias.chars().any(char::is_control)
    {
        return Err(provider_configuration_invalid(
            "provider credential handle alias is not canonical",
        ));
    }
    Ok(())
}

fn validate_safe_provider_code(code: &str) -> Result<(), SdkError> {
    if code.is_empty()
        || code.len() > MAX_SAFE_PROVIDER_CODE_BYTES
        || code.trim() != code
        || code.chars().any(|character| {
            !(character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || character == '_')
        })
    {
        return Err(provider_response_invalid());
    }
    Ok(())
}

fn valid_now(clock: &dyn Clock) -> Result<i64, SdkError> {
    let now = clock.now_unix_nanos();
    if now < 0 {
        return Err(provider_state_unavailable());
    }
    Ok(now)
}

fn duplicate_registration(coordinate: &ProviderAdapterCoordinate) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_ADAPTER_DUPLICATE",
        ErrorCategory::Internal,
        false,
        "The provider adapter registry is configured incorrectly.",
    )
    .with_internal_reference(coordinate_reference(coordinate))
}

fn adapter_unavailable(coordinate: &ProviderAdapterCoordinate) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_ADAPTER_UNAVAILABLE",
        ErrorCategory::Dependency,
        false,
        "The exact provider adapter is unavailable.",
    )
    .with_internal_reference(coordinate_reference(coordinate))
}

fn adapter_disabled(coordinate: &ProviderAdapterCoordinate) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_ADAPTER_DISABLED",
        ErrorCategory::Dependency,
        false,
        "The exact provider adapter is disabled.",
    )
    .with_internal_reference(coordinate_reference(coordinate))
}

fn provider_configuration_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The provider adapter is not configured safely.",
    )
    .with_internal_reference(reference.into())
}

fn provider_secret_unavailable() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_SECRET_UNAVAILABLE",
        ErrorCategory::Dependency,
        false,
        "The provider credential handle is unavailable.",
    )
}

fn provider_quota_exceeded() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_QUOTA_EXCEEDED",
        ErrorCategory::RateLimit,
        true,
        "The provider quota is currently exhausted.",
    )
}

fn provider_circuit_open() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_CIRCUIT_OPEN",
        ErrorCategory::Unavailable,
        true,
        "The provider circuit is open.",
    )
}

fn provider_state_unavailable() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_STATE_UNAVAILABLE",
        ErrorCategory::Internal,
        true,
        "The provider isolation state is unavailable.",
    )
}

fn provider_response_invalid() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_INVALID",
        ErrorCategory::Dependency,
        false,
        "The provider returned invalid sanitized evidence.",
    )
}

fn transport_failure_error(failure: &ProviderTransportFailure) -> SdkError {
    let (code, category, retryable, safe_message) = match failure.class() {
        ProviderTransportFailureClass::QuotaExceeded => (
            "CUSTOMER_ENRICHMENT_PROVIDER_QUOTA_EXCEEDED",
            ErrorCategory::RateLimit,
            true,
            "The provider quota is currently exhausted.",
        ),
        ProviderTransportFailureClass::Retryable => (
            "CUSTOMER_ENRICHMENT_PROVIDER_RETRYABLE_FAILURE",
            ErrorCategory::Dependency,
            true,
            "The provider request failed and may be retried.",
        ),
        ProviderTransportFailureClass::Terminal => (
            "CUSTOMER_ENRICHMENT_PROVIDER_TERMINAL_FAILURE",
            ErrorCategory::Dependency,
            false,
            "The provider request failed terminally.",
        ),
        ProviderTransportFailureClass::MappingConflict => (
            "CUSTOMER_ENRICHMENT_PROVIDER_MAPPING_CONFLICT",
            ErrorCategory::Conflict,
            false,
            "The provider response does not match the immutable mapping contract.",
        ),
    };
    let error = SdkError::new(code, category, retryable, safe_message);
    match failure.safe_provider_code() {
        Some(safe_code) => error.with_internal_reference(format!("provider_code:{safe_code}")),
        None => error,
    }
}

fn coordinate_reference(coordinate: &ProviderAdapterCoordinate) -> String {
    format!(
        "{}@{}",
        coordinate.adapter_kind(),
        coordinate.adapter_contract_version()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_customer_enrichment::{
        EnrichmentRequest, EnrichmentRequestDraft, EnrichmentRequestStatus, MappingDraft,
        MappingNormalization, MappingVersion, PartySnapshot, ProviderDispatchExpectation,
        ProviderProfileDraft, ProviderProfileVersion, RawPayloadPolicy, RequestPolicyEvidence,
        TargetField, TargetSnapshot, prepare_provider_dispatch_attempt,
    };
    use crm_module_sdk::testing::FixedClock;
    use crm_module_sdk::{ActorId, IdempotencyKey, RecordId};
    use std::collections::VecDeque;
    use std::future::Future;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::task::{Context, Poll, Wake, Waker};

    #[derive(Debug)]
    struct NoopWake;

    impl Wake for NoopWake {
        fn wake(self: Arc<Self>) {}
    }

    fn block_on<F>(future: F) -> F::Output
    where
        F: Future,
    {
        let waker = Waker::from(Arc::new(NoopWake));
        let mut context = Context::from_waker(&waker);
        let mut future = Box::pin(future);
        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(output) => return output,
                Poll::Pending => std::thread::yield_now(),
            }
        }
    }

    #[derive(Debug, Clone, Copy)]
    struct NoopAdapter;

    impl ProviderDispatchPort for NoopAdapter {
        fn dispatch<'a>(
            &'a self,
            _request: ProviderDispatchRequest,
        ) -> PortFuture<'a, Result<SanitizedProviderResponse, SdkError>> {
            Box::pin(async {
                Err(SdkError::new(
                    "TEST_NOOP_ADAPTER",
                    ErrorCategory::Dependency,
                    false,
                    "No test provider call was performed.",
                ))
            })
        }
    }

    #[derive(Debug)]
    struct ScriptedTransport {
        calls: AtomicUsize,
        outcomes: Mutex<VecDeque<Result<SanitizedProviderResponse, ProviderTransportFailure>>>,
    }

    impl ScriptedTransport {
        fn new(
            outcomes: impl IntoIterator<
                Item = Result<SanitizedProviderResponse, ProviderTransportFailure>,
            >,
        ) -> Self {
            Self {
                calls: AtomicUsize::new(0),
                outcomes: Mutex::new(outcomes.into_iter().collect()),
            }
        }

        fn calls(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }

    impl ProviderTransportPort for ScriptedTransport {
        fn dispatch<'a>(
            &'a self,
            _request: ProviderTransportRequest,
        ) -> PortFuture<'a, Result<SanitizedProviderResponse, ProviderTransportFailure>> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let outcome = self
                .outcomes
                .lock()
                .unwrap()
                .pop_front()
                .expect("scripted transport outcome");
            Box::pin(async move { outcome })
        }
    }

    fn coordinate(version: &str) -> ProviderAdapterCoordinate {
        ProviderAdapterCoordinate::try_new("registry_http_v1", version).unwrap()
    }

    fn provider_request() -> ProviderDispatchRequest {
        let profile = ProviderProfileVersion::publish(ProviderProfileDraft {
            provider_key: "company_registry".to_owned(),
            adapter_kind: "registry_http_v1".to_owned(),
            adapter_contract_version: "1.0.0".to_owned(),
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
            idempotency_key: IdempotencyKey::try_new("provider-adapter-domain-request").unwrap(),
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

    fn resolver() -> StaticProviderSecretHandleResolver {
        StaticProviderSecretHandleResolver::try_new([ProviderSecretRegistration {
            tenant_id: TenantId::try_new("tenant-a").unwrap(),
            handle_alias: "registry_primary".to_owned(),
            material: ProviderSecretMaterial::try_new(b"super-secret-provider-token".to_vec())
                .unwrap(),
        }])
        .unwrap()
    }

    fn adapter_with_transport(
        clock: Arc<FixedClock>,
        maximum_attempts: u32,
        failure_threshold: u32,
        transport: Arc<dyn ProviderTransportPort>,
    ) -> GovernedProviderAdapter {
        GovernedProviderAdapter::new(
            Arc::new(resolver()),
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
    fn exact_coordinate_resolves_enabled_adapter() {
        let exact = coordinate("1.0.0");
        let registry =
            ExactProviderAdapterRegistry::try_new([ProviderAdapterRegistration::enabled(
                exact.clone(),
                NoopAdapter,
            )])
            .unwrap();
        assert_eq!(registry.len(), 1);
        assert!(registry.resolve_exact(&exact).is_ok());
    }

    #[test]
    fn another_contract_version_does_not_fallback() {
        let registry =
            ExactProviderAdapterRegistry::try_new([ProviderAdapterRegistration::enabled(
                coordinate("1.0.0"),
                NoopAdapter,
            )])
            .unwrap();
        let error = registry.resolve_exact(&coordinate("1.1.0")).err().unwrap();
        assert_eq!(
            error.code,
            "CUSTOMER_ENRICHMENT_PROVIDER_ADAPTER_UNAVAILABLE"
        );
    }

    #[test]
    fn disabled_exact_coordinate_fails_closed() {
        let exact = coordinate("1.0.0");
        let registry =
            ExactProviderAdapterRegistry::try_new([ProviderAdapterRegistration::disabled(
                exact.clone(),
            )])
            .unwrap();
        let error = registry.resolve_exact(&exact).err().unwrap();
        assert_eq!(error.code, "CUSTOMER_ENRICHMENT_PROVIDER_ADAPTER_DISABLED");
    }

    #[test]
    fn duplicate_exact_coordinate_is_rejected() {
        let exact = coordinate("1.0.0");
        let error = ExactProviderAdapterRegistry::try_new([
            ProviderAdapterRegistration::enabled(exact.clone(), NoopAdapter),
            ProviderAdapterRegistration::disabled(exact),
        ])
        .unwrap_err();
        assert_eq!(error.code, "CUSTOMER_ENRICHMENT_PROVIDER_ADAPTER_DUPLICATE");
    }

    #[test]
    fn secret_debug_surfaces_are_redacted() {
        let material = ProviderSecretMaterial::try_new(b"super-secret-provider-token".to_vec())
            .unwrap();
        let registration = ProviderSecretRegistration {
            tenant_id: TenantId::try_new("tenant-a").unwrap(),
            handle_alias: "registry_primary".to_owned(),
            material,
        };
        let debug = format!("{registration:?}");
        assert!(debug.contains("redacted"));
        assert!(!debug.contains("super-secret-provider-token"));
        let resolver = resolver();
        let debug = format!("{resolver:?}");
        assert!(!debug.contains("super-secret-provider-token"));
    }

    #[test]
    fn governed_adapter_resolves_secret_and_returns_only_sanitized_evidence() {
        let clock = Arc::new(FixedClock::new(5_000_000_000));
        let transport = Arc::new(DeterministicSanitizedProviderTransport::new(
            clock.clone(),
            ProviderResponseClass::Success,
            1,
        ));
        let adapter = adapter_with_transport(clock, 10, 3, transport);
        let request = provider_request();
        let expected_replay_key = request.provider_idempotency_key.clone();
        let response = block_on(adapter.dispatch(request)).unwrap();
        assert_eq!(response.replay_key, expected_replay_key);
        assert_eq!(response.response_class, ProviderResponseClass::Success);
        assert!(
            response
                .canonical_response_digest
                .iter()
                .any(|byte| *byte != 0)
        );
        assert_eq!(response.safe_provider_code.as_deref(), Some("success"));
        assert!(response.protected_evidence_reference.is_none());
    }

    #[test]
    fn missing_secret_fails_closed_without_secret_or_raw_error_leakage() {
        let clock = Arc::new(FixedClock::new(5_000_000_000));
        let adapter = GovernedProviderAdapter::new(
            Arc::new(StaticProviderSecretHandleResolver::default()),
            Arc::new(
                FixedWindowProviderQuota::try_new(10, 60_000_000_000, clock.clone()).unwrap(),
            ),
            Arc::new(
                ConsecutiveFailureProviderCircuitBreaker::try_new(
                    3,
                    60_000_000_000,
                    clock.clone(),
                )
                .unwrap(),
            ),
            Arc::new(DeterministicSanitizedProviderTransport::new(
                clock,
                ProviderResponseClass::Success,
                1,
            )),
        );
        let error = block_on(adapter.dispatch(provider_request())).unwrap_err();
        assert_eq!(
            error.code,
            "CUSTOMER_ENRICHMENT_PROVIDER_SECRET_UNAVAILABLE"
        );
        assert!(error.internal_reference.is_none());
        let formatted = format!("{error:?} {error}");
        assert!(!formatted.contains("super-secret-provider-token"));
        assert!(!formatted.contains("raw provider body"));
    }

    #[test]
    fn quota_is_tenant_and_exact_coordinate_scoped() {
        let clock = Arc::new(FixedClock::new(5_000_000_000));
        let transport = Arc::new(DeterministicSanitizedProviderTransport::new(
            clock.clone(),
            ProviderResponseClass::Success,
            1,
        ));
        let adapter = adapter_with_transport(clock, 1, 3, transport);
        block_on(adapter.dispatch(provider_request())).unwrap();
        let error = block_on(adapter.dispatch(provider_request())).unwrap_err();
        assert_eq!(
            error.code,
            "CUSTOMER_ENRICHMENT_PROVIDER_QUOTA_EXCEEDED"
        );
        assert!(error.retryable);
    }

    #[test]
    fn retryable_transport_failure_opens_circuit_without_second_provider_call() {
        let failure = ProviderTransportFailure::try_new(
            ProviderTransportFailureClass::Retryable,
            Some("upstream_timeout".to_owned()),
        )
        .unwrap();
        let transport = Arc::new(ScriptedTransport::new([Err(failure)]));
        let clock = Arc::new(FixedClock::new(5_000_000_000));
        let adapter = adapter_with_transport(clock, 10, 1, transport.clone());

        let first = block_on(adapter.dispatch(provider_request())).unwrap_err();
        assert_eq!(
            first.code,
            "CUSTOMER_ENRICHMENT_PROVIDER_RETRYABLE_FAILURE"
        );
        assert!(first.retryable);
        assert_eq!(transport.calls(), 1);

        let second = block_on(adapter.dispatch(provider_request())).unwrap_err();
        assert_eq!(second.code, "CUSTOMER_ENRICHMENT_PROVIDER_CIRCUIT_OPEN");
        assert_eq!(transport.calls(), 1);
    }

    #[test]
    fn mapping_conflict_is_typed_bounded_and_does_not_open_circuit() {
        let failure = ProviderTransportFailure::try_new(
            ProviderTransportFailureClass::MappingConflict,
            Some("unsupported_shape".to_owned()),
        )
        .unwrap();
        let request = provider_request();
        let success = SanitizedProviderResponse {
            replay_key: request.provider_idempotency_key.clone(),
            provider_correlation_id: Some("provider-correlation-a".to_owned()),
            response_class: ProviderResponseClass::Success,
            canonical_response_digest: [9; 32],
            provider_observed_at_unix_ms: Some(5_000),
            retrieved_at_unix_ms: 5_000,
            metered_units: 1,
            protected_evidence_reference: None,
            safe_provider_code: Some("success".to_owned()),
        };
        let transport = Arc::new(ScriptedTransport::new([Err(failure), Ok(success)]));
        let clock = Arc::new(FixedClock::new(5_000_000_000));
        let adapter = adapter_with_transport(clock, 10, 1, transport.clone());

        let first = block_on(adapter.dispatch(request.clone())).unwrap_err();
        assert_eq!(
            first.code,
            "CUSTOMER_ENRICHMENT_PROVIDER_MAPPING_CONFLICT"
        );
        assert!(!first.retryable);
        assert_eq!(
            first.internal_reference.as_deref(),
            Some("provider_code:unsupported_shape")
        );

        block_on(adapter.dispatch(request)).unwrap();
        assert_eq!(transport.calls(), 2);
    }

    #[test]
    fn transport_failure_rejects_unbounded_or_unsafe_provider_codes() {
        assert!(
            ProviderTransportFailure::try_new(
                ProviderTransportFailureClass::Terminal,
                Some("raw body: bearer secret".to_owned()),
            )
            .is_err()
        );
        assert!(
            ProviderTransportFailure::try_new(
                ProviderTransportFailureClass::Terminal,
                Some("X".repeat(MAX_SAFE_PROVIDER_CODE_BYTES + 1)),
            )
            .is_err()
        );
    }

    fn assert_registry_port<T: ProviderAdapterRegistryPort + Send + Sync>() {}
    fn assert_provider_port<T: ProviderDispatchPort + Send + Sync>() {}

    #[test]
    fn provider_boundaries_are_thread_safe() {
        assert_registry_port::<ExactProviderAdapterRegistry>();
        assert_provider_port::<GovernedProviderAdapter>();
    }
}
