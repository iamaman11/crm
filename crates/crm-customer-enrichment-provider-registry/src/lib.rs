#![forbid(unsafe_code)]

//! Infrastructure-owned exact provider-adapter registry for Customer Enrichment.
//!
//! The registry is immutable after construction. It resolves the complete adapter kind/version
//! coordinate and never falls back to another version, a default adapter or kind-only matching.

use crm_customer_enrichment::{
    ProviderAdapterCoordinate, ProviderAdapterRegistryPort, ProviderDispatchPort,
    ProviderDispatchRequest, SanitizedProviderResponse,
};
use crm_module_sdk::{ErrorCategory, PortFuture, SdkError};
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

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
                &match self.entry {
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
            if entries.insert(coordinate.clone(), registration.entry).is_some() {
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

    fn coordinate(version: &str) -> ProviderAdapterCoordinate {
        ProviderAdapterCoordinate::try_new("registry_http_v1", version).unwrap()
    }

    #[test]
    fn exact_coordinate_resolves_enabled_adapter() {
        let exact = coordinate("1.0.0");
        let registry = ExactProviderAdapterRegistry::try_new([
            ProviderAdapterRegistration::enabled(exact.clone(), NoopAdapter),
        ])
        .unwrap();
        assert_eq!(registry.len(), 1);
        assert!(registry.resolve_exact(&exact).is_ok());
    }

    #[test]
    fn another_contract_version_does_not_fallback() {
        let registry = ExactProviderAdapterRegistry::try_new([
            ProviderAdapterRegistration::enabled(coordinate("1.0.0"), NoopAdapter),
        ])
        .unwrap();
        let error = registry.resolve_exact(&coordinate("1.1.0")).unwrap_err();
        assert_eq!(
            error.code,
            "CUSTOMER_ENRICHMENT_PROVIDER_ADAPTER_UNAVAILABLE"
        );
    }

    #[test]
    fn disabled_exact_coordinate_fails_closed() {
        let exact = coordinate("1.0.0");
        let registry = ExactProviderAdapterRegistry::try_new([
            ProviderAdapterRegistration::disabled(exact.clone()),
        ])
        .unwrap();
        let error = registry.resolve_exact(&exact).unwrap_err();
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

    fn assert_registry_port<T: ProviderAdapterRegistryPort + Send + Sync>() {}

    #[test]
    fn registry_implements_thread_safe_dispatch_port() {
        assert_registry_port::<ExactProviderAdapterRegistry>();
    }
}
