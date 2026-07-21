use crm_application_runtime::{
    CustomerEnrichmentProviderAdapterConfig, CustomerEnrichmentProviderAdapterState,
    CustomerEnrichmentProviderCredentialBinding, ProviderSecretValueSourcePort,
    ProviderTransportRegistration, StaticProviderTransportCatalog,
    build_customer_enrichment_provider_registry,
};
use crm_customer_enrichment::{
    ProviderAdapterCoordinate, SanitizedProviderResponse,
};
use crm_customer_enrichment_provider_registry::{
    ProviderSecretMaterial, ProviderTransportFailure, ProviderTransportFailureClass,
    ProviderTransportPort, ProviderTransportRequest,
};
use crm_module_sdk::testing::FixedClock;
use crm_module_sdk::{PortFuture, SdkError, TenantId};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

const TRANSPORT_KEY: &str = "registry_http";
const ADAPTER_KIND: &str = "registry_http_v1";
const ADAPTER_VERSION: &str = "1.0.0";
const SECRET_ENVIRONMENT: &str = "REGISTRY_PRIMARY_TOKEN";
const SECRET_VALUE: &[u8] = b"transport-isolation-secret";

#[derive(Debug)]
struct ForbiddenTransport {
    calls: Arc<AtomicUsize>,
}

impl ProviderTransportPort for ForbiddenTransport {
    fn dispatch<'a>(
        &'a self,
        _request: ProviderTransportRequest,
    ) -> PortFuture<'a, Result<SanitizedProviderResponse, ProviderTransportFailure>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async {
            Err(ProviderTransportFailure::try_new(
                ProviderTransportFailureClass::Terminal,
                Some("transport_must_not_run".to_owned()),
            )
            .expect("construct forbidden transport failure"))
        })
    }
}

#[derive(Debug)]
struct CountingSecretValues {
    calls: Arc<AtomicUsize>,
}

impl ProviderSecretValueSourcePort for CountingSecretValues {
    fn resolve(&self, _environment_name: &str) -> Result<ProviderSecretMaterial, SdkError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        ProviderSecretMaterial::try_new(SECRET_VALUE.to_vec())
    }
}

#[test]
fn exact_host_transport_mismatches_fail_before_secret_or_provider_io() {
    let transport_calls = Arc::new(AtomicUsize::new(0));
    let secret_calls = Arc::new(AtomicUsize::new(0));
    let registered_coordinate = coordinate(ADAPTER_VERSION);
    let catalog = StaticProviderTransportCatalog::try_new([ProviderTransportRegistration {
        transport_key: TRANSPORT_KEY.to_owned(),
        coordinate: registered_coordinate.clone(),
        transport: Arc::new(ForbiddenTransport {
            calls: transport_calls.clone(),
        }),
    }])
    .expect("build exact host transport catalog");

    let cases = [
        (
            "registry_http_shadow",
            registered_coordinate,
            "registry_http_shadow:registry_http_v1@1.0.0",
        ),
        (
            TRANSPORT_KEY,
            coordinate("1.1.0"),
            "registry_http:registry_http_v1@1.1.0",
        ),
    ];

    for (transport_key, requested_coordinate, expected_reference) in cases {
        let error = build_customer_enrichment_provider_registry(
            &[enabled_config(transport_key, requested_coordinate)],
            Arc::new(FixedClock::new(1_000_000_000)),
            Arc::new(catalog.clone()),
            Arc::new(CountingSecretValues {
                calls: secret_calls.clone(),
            }),
        )
        .expect_err("host composition must reject a non-exact transport registration");

        assert_eq!(
            error.code,
            "CUSTOMER_ENRICHMENT_PROVIDER_TRANSPORT_UNAVAILABLE"
        );
        assert!(!error.retryable);
        assert_eq!(
            error.internal_reference.as_deref(),
            Some(expected_reference)
        );
        assert!(!format!("{error:?} {error}").contains("transport-isolation-secret"));
    }

    assert_eq!(secret_calls.load(Ordering::SeqCst), 0);
    assert_eq!(transport_calls.load(Ordering::SeqCst), 0);
}

fn coordinate(version: &str) -> ProviderAdapterCoordinate {
    ProviderAdapterCoordinate::try_new(ADAPTER_KIND, version).expect("build adapter coordinate")
}

fn enabled_config(
    transport_key: &str,
    coordinate: ProviderAdapterCoordinate,
) -> CustomerEnrichmentProviderAdapterConfig {
    CustomerEnrichmentProviderAdapterConfig {
        coordinate,
        state: CustomerEnrichmentProviderAdapterState::Enabled,
        transport_key: Some(transport_key.to_owned()),
        maximum_attempts: Some(10),
        quota_window_seconds: Some(60),
        circuit_failure_threshold: Some(3),
        circuit_open_seconds: Some(60),
        credential_bindings: vec![CustomerEnrichmentProviderCredentialBinding {
            tenant_id: TenantId::try_new("tenant-a").expect("build tenant id"),
            handle_alias: "registry_primary".to_owned(),
            secret_environment: SECRET_ENVIRONMENT.to_owned(),
        }],
    }
}
