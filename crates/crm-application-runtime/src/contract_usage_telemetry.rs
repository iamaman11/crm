use crate::generated_contract_telemetry::DEPRECATED_CONTRACTS;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRegistryPort};
use crm_module_sdk::{CapabilityId, CapabilityVersion, PortFuture, SdkError};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ContractUsageSurface {
    Mutation,
    Query,
}

impl ContractUsageSurface {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Mutation => "mutation",
            Self::Query => "query",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DeprecatedContractTelemetry {
    pub capability_id: &'static str,
    pub capability_version: &'static str,
    pub owner_module_id: &'static str,
    pub metric: &'static str,
    pub lookback_days: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Coordinate {
    capability_id: String,
    capability_version: String,
}

impl Coordinate {
    fn from_definition(definition: &CapabilityDefinition) -> Self {
        Self {
            capability_id: definition.capability_id.as_str().to_owned(),
            capability_version: definition.capability_version.as_str().to_owned(),
        }
    }

    fn from_catalog(entry: &DeprecatedContractTelemetry) -> Self {
        Self {
            capability_id: entry.capability_id.to_owned(),
            capability_version: entry.capability_version.to_owned(),
        }
    }

    fn display(&self) -> String {
        format!("{}@{}", self.capability_id, self.capability_version)
    }
}

#[derive(Debug, Clone)]
struct PublishedDefinition {
    owner_module_id: String,
    surface: ContractUsageSurface,
}

#[derive(Debug)]
struct MetricSeries {
    owner_module_id: &'static str,
    metric: &'static str,
    lookback_days: u32,
    surface: ContractUsageSurface,
    count: AtomicU64,
}

#[derive(Debug, Default)]
pub struct ContractUsageMetrics {
    series: BTreeMap<Coordinate, MetricSeries>,
}

impl ContractUsageMetrics {
    pub fn production(
        mutation_definitions: &[CapabilityDefinition],
        query_definitions: &[CapabilityDefinition],
    ) -> Result<Self, ContractUsageTelemetryError> {
        Self::from_catalog(
            DEPRECATED_CONTRACTS,
            mutation_definitions,
            query_definitions,
        )
    }

    fn from_catalog(
        catalog: &'static [DeprecatedContractTelemetry],
        mutation_definitions: &[CapabilityDefinition],
        query_definitions: &[CapabilityDefinition],
    ) -> Result<Self, ContractUsageTelemetryError> {
        let mut published = BTreeMap::new();
        add_published_definitions(
            &mut published,
            mutation_definitions,
            ContractUsageSurface::Mutation,
        )?;
        add_published_definitions(
            &mut published,
            query_definitions,
            ContractUsageSurface::Query,
        )?;

        let mut series = BTreeMap::new();
        for entry in catalog {
            let coordinate = Coordinate::from_catalog(entry);
            let definition = published.get(&coordinate).ok_or_else(|| {
                ContractUsageTelemetryError::UnpublishedContract(coordinate.display())
            })?;
            if definition.owner_module_id != entry.owner_module_id {
                return Err(ContractUsageTelemetryError::OwnerMismatch {
                    coordinate: coordinate.display(),
                    expected: entry.owner_module_id.to_owned(),
                    actual: definition.owner_module_id.clone(),
                });
            }
            let metric_series = MetricSeries {
                owner_module_id: entry.owner_module_id,
                metric: entry.metric,
                lookback_days: entry.lookback_days,
                surface: definition.surface,
                count: AtomicU64::new(0),
            };
            if series.insert(coordinate.clone(), metric_series).is_some() {
                return Err(ContractUsageTelemetryError::DuplicateContract(
                    coordinate.display(),
                ));
            }
        }
        Ok(Self { series })
    }

    fn record_resolved(&self, definition: &CapabilityDefinition, surface: ContractUsageSurface) {
        let coordinate = Coordinate::from_definition(definition);
        let Some(series) = self.series.get(&coordinate) else {
            return;
        };
        if series.surface != surface
            || series.owner_module_id != definition.owner_module_id.as_str()
        {
            return;
        }
        let _ = series
            .count
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            });
    }

    pub fn render_prometheus(&self) -> String {
        let metrics: BTreeSet<_> = self.series.values().map(|series| series.metric).collect();
        let mut output = String::new();
        for metric in metrics {
            output.push_str("# HELP ");
            output.push_str(metric);
            output.push_str(
                " Resolved requests for deprecated exact-version capability contracts.\n",
            );
            output.push_str("# TYPE ");
            output.push_str(metric);
            output.push_str(" counter\n");
            for (coordinate, series) in &self.series {
                if series.metric != metric {
                    continue;
                }
                output.push_str(metric);
                output.push_str("{capability_id=\"");
                output.push_str(&coordinate.capability_id);
                output.push_str("\",capability_version=\"");
                output.push_str(&coordinate.capability_version);
                output.push_str("\",owner_module_id=\"");
                output.push_str(series.owner_module_id);
                output.push_str("\",surface=\"");
                output.push_str(series.surface.as_str());
                output.push_str("\",lookback_days=\"");
                output.push_str(&series.lookback_days.to_string());
                output.push_str("\"} ");
                output.push_str(&series.count.load(Ordering::Relaxed).to_string());
                output.push('\n');
            }
        }
        output
    }

    #[cfg(test)]
    fn count(&self, capability_id: &str, capability_version: &str) -> Option<u64> {
        self.series
            .get(&Coordinate {
                capability_id: capability_id.to_owned(),
                capability_version: capability_version.to_owned(),
            })
            .map(|series| series.count.load(Ordering::Relaxed))
    }
}

fn add_published_definitions(
    published: &mut BTreeMap<Coordinate, PublishedDefinition>,
    definitions: &[CapabilityDefinition],
    surface: ContractUsageSurface,
) -> Result<(), ContractUsageTelemetryError> {
    for definition in definitions {
        let coordinate = Coordinate::from_definition(definition);
        let entry = PublishedDefinition {
            owner_module_id: definition.owner_module_id.as_str().to_owned(),
            surface,
        };
        if published.insert(coordinate.clone(), entry).is_some() {
            return Err(ContractUsageTelemetryError::DuplicatePublishedContract(
                coordinate.display(),
            ));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractUsageTelemetryError {
    DuplicatePublishedContract(String),
    DuplicateContract(String),
    UnpublishedContract(String),
    OwnerMismatch {
        coordinate: String,
        expected: String,
        actual: String,
    },
}

impl fmt::Display for ContractUsageTelemetryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicatePublishedContract(coordinate) => {
                write!(formatter, "published contract is duplicated: {coordinate}")
            }
            Self::DuplicateContract(coordinate) => {
                write!(
                    formatter,
                    "deprecated telemetry contract is duplicated: {coordinate}"
                )
            }
            Self::UnpublishedContract(coordinate) => {
                write!(
                    formatter,
                    "deprecated telemetry contract is not published: {coordinate}"
                )
            }
            Self::OwnerMismatch {
                coordinate,
                expected,
                actual,
            } => write!(
                formatter,
                "deprecated telemetry owner mismatch for {coordinate}: expected {expected}, got {actual}"
            ),
        }
    }
}

impl std::error::Error for ContractUsageTelemetryError {}

#[derive(Clone)]
pub struct MeteredCapabilityRegistry {
    inner: Arc<dyn CapabilityRegistryPort>,
    metrics: Arc<ContractUsageMetrics>,
    surface: ContractUsageSurface,
}

impl fmt::Debug for MeteredCapabilityRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MeteredCapabilityRegistry")
            .field("inner", &"dyn CapabilityRegistryPort")
            .field("surface", &self.surface)
            .finish()
    }
}

impl MeteredCapabilityRegistry {
    pub fn new(
        inner: Arc<dyn CapabilityRegistryPort>,
        metrics: Arc<ContractUsageMetrics>,
        surface: ContractUsageSurface,
    ) -> Self {
        Self {
            inner,
            metrics,
            surface,
        }
    }
}

impl CapabilityRegistryPort for MeteredCapabilityRegistry {
    fn resolve<'a>(
        &'a self,
        capability_id: &'a CapabilityId,
        capability_version: &'a CapabilityVersion,
    ) -> PortFuture<'a, Result<Option<CapabilityDefinition>, SdkError>> {
        Box::pin(async move {
            let definition = self
                .inner
                .resolve(capability_id, capability_version)
                .await?;
            if let Some(definition) = definition.as_ref() {
                self.metrics.record_resolved(definition, self.surface);
            }
            Ok(definition)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_capability_runtime::{CapabilityRisk, PayloadContract};
    use crm_module_sdk::{DataClass, ModuleId, PayloadEncoding, SchemaId, SchemaVersion};

    static CATALOG: &[DeprecatedContractTelemetry] = &[DeprecatedContractTelemetry {
        capability_id: "test.record.create",
        capability_version: "1.0.0",
        owner_module_id: "crm.test",
        metric: "crm_contract_invocations_total",
        lookback_days: 30,
    }];

    #[derive(Debug)]
    struct Registry {
        definition: CapabilityDefinition,
    }

    impl CapabilityRegistryPort for Registry {
        fn resolve<'a>(
            &'a self,
            capability_id: &'a CapabilityId,
            capability_version: &'a CapabilityVersion,
        ) -> PortFuture<'a, Result<Option<CapabilityDefinition>, SdkError>> {
            Box::pin(async move {
                if &self.definition.capability_id == capability_id
                    && &self.definition.capability_version == capability_version
                {
                    Ok(Some(self.definition.clone()))
                } else {
                    Ok(None)
                }
            })
        }
    }

    #[tokio::test]
    async fn exact_deprecated_resolution_increments_before_later_gateway_checks() {
        let definition = definition(true, "crm.test");
        let metrics = Arc::new(
            ContractUsageMetrics::from_catalog(CATALOG, &[definition.clone()], &[]).unwrap(),
        );
        let registry = MeteredCapabilityRegistry::new(
            Arc::new(Registry {
                definition: definition.clone(),
            }),
            Arc::clone(&metrics),
            ContractUsageSurface::Mutation,
        );

        assert_eq!(metrics.count("test.record.create", "1.0.0"), Some(0));
        assert!(
            registry
                .resolve(&definition.capability_id, &definition.capability_version)
                .await
                .unwrap()
                .is_some()
        );
        assert_eq!(metrics.count("test.record.create", "1.0.0"), Some(1));
    }

    #[tokio::test]
    async fn unknown_or_wrong_surface_resolution_does_not_increment() {
        let definition = definition(true, "crm.test");
        let metrics = Arc::new(
            ContractUsageMetrics::from_catalog(CATALOG, &[definition.clone()], &[]).unwrap(),
        );
        let registry = MeteredCapabilityRegistry::new(
            Arc::new(Registry {
                definition: definition.clone(),
            }),
            Arc::clone(&metrics),
            ContractUsageSurface::Query,
        );
        let missing = CapabilityId::try_new("test.record.missing").unwrap();

        assert!(
            registry
                .resolve(&missing, &definition.capability_version)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            registry
                .resolve(&definition.capability_id, &definition.capability_version)
                .await
                .unwrap()
                .is_some()
        );
        assert_eq!(metrics.count("test.record.create", "1.0.0"), Some(0));
    }

    #[test]
    fn catalog_is_fail_closed_against_live_owner_and_coordinate() {
        let wrong_owner = definition(true, "crm.other");
        let owner_error =
            ContractUsageMetrics::from_catalog(CATALOG, &[wrong_owner], &[]).unwrap_err();
        assert!(matches!(
            owner_error,
            ContractUsageTelemetryError::OwnerMismatch { .. }
        ));

        let missing_error = ContractUsageMetrics::from_catalog(CATALOG, &[], &[]).unwrap_err();
        assert!(matches!(
            missing_error,
            ContractUsageTelemetryError::UnpublishedContract(_)
        ));
    }

    #[test]
    fn prometheus_output_is_deterministic_zero_seeded_and_pii_free() {
        let definition = definition(true, "crm.test");
        let metrics = ContractUsageMetrics::from_catalog(CATALOG, &[definition], &[]).unwrap();
        let first = metrics.render_prometheus();
        let second = metrics.render_prometheus();

        assert_eq!(first, second);
        assert!(first.contains("crm_contract_invocations_total"));
        assert!(first.contains("capability_id=\"test.record.create\""));
        assert!(first.contains("surface=\"mutation\""));
        assert!(first.ends_with(" 0\n"));
        assert!(!first.contains("tenant"));
        assert!(!first.contains("actor"));
        assert!(!first.contains("request_id"));
    }

    fn definition(mutation: bool, owner: &str) -> CapabilityDefinition {
        let contract = PayloadContract {
            owner: ModuleId::try_new(owner).unwrap(),
            schema_id: SchemaId::try_new("crm.test.v1.RecordRequest").unwrap(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
            descriptor_hash: [0x51; 32],
            allowed_data_classes: vec![DataClass::Internal],
            allowed_encodings: vec![PayloadEncoding::Protobuf],
            maximum_size_bytes: 1024,
        };
        CapabilityDefinition {
            capability_id: CapabilityId::try_new("test.record.create").unwrap(),
            capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
            owner_module_id: ModuleId::try_new(owner).unwrap(),
            input_contract: contract.clone(),
            output_contract: Some(contract),
            risk: CapabilityRisk::Low,
            mutation,
            requires_idempotency: mutation,
            requires_approval: false,
            authorization_policy_id: "test.record.create".to_owned(),
            rate_limit_policy_id: None,
        }
    }
}
