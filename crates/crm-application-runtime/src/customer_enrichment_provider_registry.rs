use crate::{
    CustomerEnrichmentProviderAdapterConfig, CustomerEnrichmentProviderAdapterState,
};
use crm_customer_enrichment::ProviderAdapterCoordinate;
use crm_customer_enrichment_provider_registry::{
    ConsecutiveFailureProviderCircuitBreaker, ExactProviderAdapterRegistry,
    FixedWindowProviderQuota, GovernedProviderAdapter, ProviderAdapterRegistration,
    ProviderSecretMaterial, ProviderSecretRegistration, ProviderTransportPort,
    StaticProviderSecretHandleResolver,
};
use crm_module_sdk::{Clock, ErrorCategory, SdkError};
use std::collections::BTreeMap;
use std::env;
use std::fmt;
use std::sync::Arc;

const NANOS_PER_SECOND: u64 = 1_000_000_000;

/// Resolves one exact transport implementation selected by explicit production configuration.
pub trait ProviderTransportCatalogPort: Send + Sync {
    fn resolve_exact(
        &self,
        transport_key: &str,
        coordinate: &ProviderAdapterCoordinate,
    ) -> Result<Arc<dyn ProviderTransportPort>, SdkError>;
}

/// Resolves one configured environment-backed secret without exposing its value in errors or debug
/// output.
pub trait ProviderSecretValueSourcePort: Send + Sync {
    fn resolve(&self, environment_name: &str) -> Result<ProviderSecretMaterial, SdkError>;
}

/// One exact transport-catalog registration supplied by the process host.
#[derive(Clone)]
pub struct ProviderTransportRegistration {
    pub transport_key: String,
    pub coordinate: ProviderAdapterCoordinate,
    pub transport: Arc<dyn ProviderTransportPort>,
}

impl fmt::Debug for ProviderTransportRegistration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderTransportRegistration")
            .field("transport_key", &self.transport_key)
            .field("coordinate", &self.coordinate)
            .field("transport", &"dyn ProviderTransportPort")
            .finish()
    }
}

/// Immutable exact transport catalog. It never falls back by transport key or adapter kind alone.
#[derive(Clone, Default)]
pub struct StaticProviderTransportCatalog {
    entries: Arc<
        BTreeMap<
            (String, ProviderAdapterCoordinate),
            Arc<dyn ProviderTransportPort>,
        >,
    >,
}

impl StaticProviderTransportCatalog {
    pub fn try_new(
        registrations: impl IntoIterator<Item = ProviderTransportRegistration>,
    ) -> Result<Self, SdkError> {
        let mut entries = BTreeMap::new();
        for registration in registrations {
            validate_transport_key(&registration.transport_key)?;
            let key = (registration.transport_key, registration.coordinate);
            if entries.insert(key.clone(), registration.transport).is_some() {
                return Err(provider_configuration_invalid(format!(
                    "duplicate provider transport registration {}:{}@{}",
                    key.0,
                    key.1.adapter_kind(),
                    key.1.adapter_contract_version()
                )));
            }
        }
        Ok(Self {
            entries: Arc::new(entries),
        })
    }
}

impl fmt::Debug for StaticProviderTransportCatalog {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StaticProviderTransportCatalog")
            .field("entry_count", &self.entries.len())
            .field(
                "coordinates",
                &self
                    .entries
                    .keys()
                    .map(|(transport_key, coordinate)| {
                        format!(
                            "{}:{}@{}",
                            transport_key,
                            coordinate.adapter_kind(),
                            coordinate.adapter_contract_version()
                        )
                    })
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl ProviderTransportCatalogPort for StaticProviderTransportCatalog {
    fn resolve_exact(
        &self,
        transport_key: &str,
        coordinate: &ProviderAdapterCoordinate,
    ) -> Result<Arc<dyn ProviderTransportPort>, SdkError> {
        self.entries
            .get(&(transport_key.to_owned(), coordinate.clone()))
            .cloned()
            .ok_or_else(|| provider_transport_unavailable(transport_key, coordinate))
    }
}

/// Process-environment secret source used by the default production assembly.
#[derive(Debug, Clone, Copy, Default)]
pub struct ProcessProviderSecretValueSource;

impl ProviderSecretValueSourcePort for ProcessProviderSecretValueSource {
    fn resolve(&self, environment_name: &str) -> Result<ProviderSecretMaterial, SdkError> {
        let value = env::var_os(environment_name)
            .ok_or_else(|| provider_secret_configuration_missing(environment_name))?
            .into_string()
            .map_err(|_| provider_secret_configuration_invalid(environment_name))?;
        ProviderSecretMaterial::try_new(value.into_bytes())
            .map_err(|_| provider_secret_configuration_invalid(environment_name))
    }
}

/// Builds the immutable exact registry from explicit application configuration and host-supplied
/// transport implementations. An empty configuration remains an empty fail-closed registry.
pub fn build_customer_enrichment_provider_registry(
    configurations: &[CustomerEnrichmentProviderAdapterConfig],
    clock: Arc<dyn Clock>,
    transport_catalog: Arc<dyn ProviderTransportCatalogPort>,
    secret_values: Arc<dyn ProviderSecretValueSourcePort>,
) -> Result<ExactProviderAdapterRegistry, SdkError> {
    let mut registrations = Vec::with_capacity(configurations.len());
    for configuration in configurations {
        let coordinate = configuration.coordinate.clone();
        match configuration.state {
            CustomerEnrichmentProviderAdapterState::Disabled => {
                registrations.push(ProviderAdapterRegistration::disabled(coordinate));
            }
            CustomerEnrichmentProviderAdapterState::Enabled => {
                let transport_key = configuration
                    .transport_key
                    .as_deref()
                    .ok_or_else(|| provider_configuration_invalid("enabled transport key missing"))?;
                let transport = transport_catalog.resolve_exact(transport_key, &coordinate)?;
                let secret_registrations = configuration
                    .credential_bindings
                    .iter()
                    .map(|binding| {
                        Ok(ProviderSecretRegistration {
                            tenant_id: binding.tenant_id.clone(),
                            handle_alias: binding.handle_alias.clone(),
                            material: secret_values.resolve(&binding.secret_environment)?,
                        })
                    })
                    .collect::<Result<Vec<_>, SdkError>>()?;
                let secrets = Arc::new(StaticProviderSecretHandleResolver::try_new(
                    secret_registrations,
                )?);
                let quota = Arc::new(FixedWindowProviderQuota::try_new(
                    configuration.maximum_attempts.ok_or_else(|| {
                        provider_configuration_invalid("enabled quota maximum missing")
                    })?,
                    seconds_to_nanos(
                        configuration.quota_window_seconds.ok_or_else(|| {
                            provider_configuration_invalid("enabled quota window missing")
                        })?,
                    )?,
                    Arc::clone(&clock),
                )?);
                let circuit = Arc::new(ConsecutiveFailureProviderCircuitBreaker::try_new(
                    configuration.circuit_failure_threshold.ok_or_else(|| {
                        provider_configuration_invalid("enabled circuit threshold missing")
                    })?,
                    seconds_to_nanos(
                        configuration.circuit_open_seconds.ok_or_else(|| {
                            provider_configuration_invalid("enabled circuit window missing")
                        })?,
                    )?,
                    Arc::clone(&clock),
                )?);
                registrations.push(ProviderAdapterRegistration::enabled(
                    coordinate,
                    GovernedProviderAdapter::new(secrets, quota, circuit, transport),
                ));
            }
        }
    }
    ExactProviderAdapterRegistry::try_new(registrations)
}

fn seconds_to_nanos(seconds: u64) -> Result<i64, SdkError> {
    seconds
        .checked_mul(NANOS_PER_SECOND)
        .and_then(|value| i64::try_from(value).ok())
        .filter(|value| *value > 0)
        .ok_or_else(|| provider_configuration_invalid("provider duration is not representable"))
}

fn validate_transport_key(value: &str) -> Result<(), SdkError> {
    if value.is_empty()
        || value.len() > 80
        || value.trim() != value
        || value.chars().any(|character| {
            !(character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '_' | '-' | '.'))
        })
    {
        return Err(provider_configuration_invalid(
            "provider transport key is not canonical",
        ));
    }
    Ok(())
}

fn provider_transport_unavailable(
    transport_key: &str,
    coordinate: &ProviderAdapterCoordinate,
) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_TRANSPORT_UNAVAILABLE",
        ErrorCategory::Dependency,
        false,
        "The configured provider transport is unavailable.",
    )
    .with_internal_reference(format!(
        "{}:{}@{}",
        transport_key,
        coordinate.adapter_kind(),
        coordinate.adapter_contract_version()
    ))
}

fn provider_secret_configuration_missing(environment_name: &str) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_SECRET_CONFIGURATION_MISSING",
        ErrorCategory::Internal,
        false,
        "A configured provider secret is missing.",
    )
    .with_internal_reference(format!("environment:{environment_name}"))
}

fn provider_secret_configuration_invalid(environment_name: &str) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_SECRET_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "A configured provider secret is invalid.",
    )
    .with_internal_reference(format!("environment:{environment_name}"))
}

fn provider_configuration_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The provider adapter configuration is invalid.",
    )
    .with_internal_reference(reference.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_customer_enrichment::{
        ProviderDispatchRequest, ProviderResponseClass, SanitizedProviderResponse,
    };
    use crm_customer_enrichment_provider_registry::DeterministicSanitizedProviderTransport;
    use crm_module_sdk::testing::FixedClock;
    use crm_module_sdk::{PortFuture, TenantId};
    use std::collections::BTreeMap;

    #[derive(Debug, Clone)]
    struct StaticSecretValues {
        values: BTreeMap<String, ProviderSecretMaterial>,
    }

    impl ProviderSecretValueSourcePort for StaticSecretValues {
        fn resolve(&self, environment_name: &str) -> Result<ProviderSecretMaterial, SdkError> {
            self.values
                .get(environment_name)
                .cloned()
                .ok_or_else(|| provider_secret_configuration_missing(environment_name))
        }
    }

    #[derive(Debug)]
    struct NoopTransport;

    impl ProviderTransportPort for NoopTransport {
        fn dispatch<'a>(
            &'a self,
            _request: crm_customer_enrichment_provider_registry::ProviderTransportRequest,
        ) -> PortFuture<'a, Result<SanitizedProviderResponse, crm_customer_enrichment_provider_registry::ProviderTransportFailure>> {
            Box::pin(async {
                Ok(SanitizedProviderResponse {
                    replay_key: "unused".to_owned(),
                    provider_correlation_id: None,
                    response_class: ProviderResponseClass::Success,
                    canonical_response_digest: [1; 32],
                    provider_observed_at_unix_ms: Some(1),
                    retrieved_at_unix_ms: 1,
                    metered_units: 1,
                    protected_evidence_reference: None,
                    safe_provider_code: Some("success".to_owned()),
                })
            })
        }
    }

    fn coordinate() -> ProviderAdapterCoordinate {
        ProviderAdapterCoordinate::try_new("registry_http_v1", "1.0.0").unwrap()
    }

    fn enabled_config() -> CustomerEnrichmentProviderAdapterConfig {
        CustomerEnrichmentProviderAdapterConfig {
            coordinate: coordinate(),
            state: CustomerEnrichmentProviderAdapterState::Enabled,
            transport_key: Some("registry_http".to_owned()),
            maximum_attempts: Some(10),
            quota_window_seconds: Some(60),
            circuit_failure_threshold: Some(3),
            circuit_open_seconds: Some(30),
            credential_bindings: vec![crate::CustomerEnrichmentProviderCredentialBinding {
                tenant_id: TenantId::try_new("tenant-a").unwrap(),
                handle_alias: "registry_primary".to_owned(),
                secret_environment: "REGISTRY_PRIMARY_TOKEN".to_owned(),
            }],
        }
    }

    #[test]
    fn empty_configuration_stays_fail_closed() {
        let registry = build_customer_enrichment_provider_registry(
            &[],
            Arc::new(FixedClock::new(1_000_000_000)),
            Arc::new(StaticProviderTransportCatalog::default()),
            Arc::new(StaticSecretValues {
                values: BTreeMap::new(),
            }),
        )
        .unwrap();
        assert!(registry.is_empty());
    }

    #[test]
    fn disabled_coordinate_needs_no_transport_or_secret() {
        let registry = build_customer_enrichment_provider_registry(
            &[CustomerEnrichmentProviderAdapterConfig {
                coordinate: coordinate(),
                state: CustomerEnrichmentProviderAdapterState::Disabled,
                transport_key: None,
                maximum_attempts: None,
                quota_window_seconds: None,
                circuit_failure_threshold: None,
                circuit_open_seconds: None,
                credential_bindings: Vec::new(),
            }],
            Arc::new(FixedClock::new(1_000_000_000)),
            Arc::new(StaticProviderTransportCatalog::default()),
            Arc::new(StaticSecretValues {
                values: BTreeMap::new(),
            }),
        )
        .unwrap();
        let error = registry.resolve_exact(&coordinate()).unwrap_err();
        assert_eq!(error.code, "CUSTOMER_ENRICHMENT_PROVIDER_ADAPTER_DISABLED");
    }

    #[test]
    fn enabled_coordinate_requires_exact_transport_and_secret_environment() {
        let clock = Arc::new(FixedClock::new(1_000_000_000));
        let transport = Arc::new(DeterministicSanitizedProviderTransport::new(
            clock.clone(),
            ProviderResponseClass::Success,
            1,
        ));
        let catalog = StaticProviderTransportCatalog::try_new([
            ProviderTransportRegistration {
                transport_key: "registry_http".to_owned(),
                coordinate: coordinate(),
                transport,
            },
        ])
        .unwrap();
        let registry = build_customer_enrichment_provider_registry(
            &[enabled_config()],
            clock,
            Arc::new(catalog),
            Arc::new(StaticSecretValues {
                values: BTreeMap::from([(
                    "REGISTRY_PRIMARY_TOKEN".to_owned(),
                    ProviderSecretMaterial::try_new(b"secret-token".to_vec()).unwrap(),
                )]),
            }),
        )
        .unwrap();
        assert!(registry.resolve_exact(&coordinate()).is_ok());
    }

    #[test]
    fn unknown_transport_and_missing_secret_fail_startup_without_value_leakage() {
        let config = enabled_config();
        let error = build_customer_enrichment_provider_registry(
            &[config.clone()],
            Arc::new(FixedClock::new(1_000_000_000)),
            Arc::new(StaticProviderTransportCatalog::default()),
            Arc::new(StaticSecretValues {
                values: BTreeMap::new(),
            }),
        )
        .unwrap_err();
        assert_eq!(
            error.code,
            "CUSTOMER_ENRICHMENT_PROVIDER_TRANSPORT_UNAVAILABLE"
        );

        let catalog = StaticProviderTransportCatalog::try_new([
            ProviderTransportRegistration {
                transport_key: "registry_http".to_owned(),
                coordinate: coordinate(),
                transport: Arc::new(NoopTransport),
            },
        ])
        .unwrap();
        let error = build_customer_enrichment_provider_registry(
            &[config],
            Arc::new(FixedClock::new(1_000_000_000)),
            Arc::new(catalog),
            Arc::new(StaticSecretValues {
                values: BTreeMap::new(),
            }),
        )
        .unwrap_err();
        assert_eq!(
            error.code,
            "CUSTOMER_ENRICHMENT_PROVIDER_SECRET_CONFIGURATION_MISSING"
        );
        assert!(!format!("{error:?} {error}").contains("secret-token"));
    }

    #[test]
    fn transport_catalog_does_not_fallback_across_versions() {
        let catalog = StaticProviderTransportCatalog::try_new([
            ProviderTransportRegistration {
                transport_key: "registry_http".to_owned(),
                coordinate: coordinate(),
                transport: Arc::new(NoopTransport),
            },
        ])
        .unwrap();
        let other = ProviderAdapterCoordinate::try_new("registry_http_v1", "1.1.0").unwrap();
        let error = catalog.resolve_exact("registry_http", &other).unwrap_err();
        assert_eq!(
            error.code,
            "CUSTOMER_ENRICHMENT_PROVIDER_TRANSPORT_UNAVAILABLE"
        );
    }

    #[allow(dead_code)]
    fn _assert_dispatch_request_is_send(_: ProviderDispatchRequest) {}
}
