use crm_capability_runtime::{CapabilityDefinition, CapabilityRegistryPort};
use crm_module_sdk::{CapabilityId, CapabilityVersion, ErrorCategory, PortFuture, SdkError};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ContractUsageSurface {
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

    fn from_catalog(entry: &DeprecatedContract) -> Self {
        Self {
            capability_id: entry.capability_id.to_owned(),
            capability_version: entry.capability_version.to_owned(),
        }
    }

    fn display(&self) -> String {
        format!("{}@{}", self.capability_id, self.capability_version)
    }
}

#[derive(Debug, Clone, Copy)]
struct DeprecatedContract {
    capability_id: &'static str,
    capability_version: &'static str,
    owner_module_id: &'static str,
    metric: &'static str,
    lookback_days: u32,
}

impl From<&'static (&'static str, &'static str, &'static str, &'static str, u32)>
    for DeprecatedContract
{
    fn from(value: &'static (&'static str, &'static str, &'static str, &'static str, u32)) -> Self {
        Self {
            capability_id: value.0,
            capability_version: value.1,
            owner_module_id: value.2,
            metric: value.3,
            lookback_days: value.4,
        }
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
struct ContractUsageMetrics {
    series: BTreeMap<Coordinate, MetricSeries>,
}

impl ContractUsageMetrics {
    fn new(
        catalog: &'static [(&'static str, &'static str, &'static str, &'static str, u32)],
        mutation_definitions: &[CapabilityDefinition],
        query_definitions: &[CapabilityDefinition],
    ) -> Result<Self, SdkError> {
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
        for raw_entry in catalog {
            let entry = DeprecatedContract::from(raw_entry);
            let coordinate = Coordinate::from_catalog(&entry);
            let definition = published.get(&coordinate).ok_or_else(|| {
                configuration_error(format!(
                    "deprecated telemetry contract is not published: {}",
                    coordinate.display()
                ))
            })?;
            if definition.owner_module_id != entry.owner_module_id {
                return Err(configuration_error(format!(
                    "deprecated telemetry owner mismatch for {}: expected {}, got {}",
                    coordinate.display(),
                    entry.owner_module_id,
                    definition.owner_module_id
                )));
            }
            let metric_series = MetricSeries {
                owner_module_id: entry.owner_module_id,
                metric: entry.metric,
                lookback_days: entry.lookback_days,
                surface: definition.surface,
                count: AtomicU64::new(0),
            };
            if series.insert(coordinate.clone(), metric_series).is_some() {
                return Err(configuration_error(format!(
                    "deprecated telemetry contract is duplicated: {}",
                    coordinate.display()
                )));
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

    fn render_prometheus(&self) -> String {
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
}

fn add_published_definitions(
    published: &mut BTreeMap<Coordinate, PublishedDefinition>,
    definitions: &[CapabilityDefinition],
    surface: ContractUsageSurface,
) -> Result<(), SdkError> {
    for definition in definitions {
        let coordinate = Coordinate::from_definition(definition);
        let entry = PublishedDefinition {
            owner_module_id: definition.owner_module_id.as_str().to_owned(),
            surface,
        };
        if published.insert(coordinate.clone(), entry).is_some() {
            return Err(configuration_error(format!(
                "published contract is duplicated: {}",
                coordinate.display()
            )));
        }
    }
    Ok(())
}

fn configuration_error(internal_reference: String) -> SdkError {
    SdkError::new(
        "CONTRACT_USAGE_TELEMETRY_INVALID",
        ErrorCategory::Internal,
        false,
        "Contract usage telemetry configuration is invalid.",
    )
    .with_internal_reference(internal_reference)
}

#[derive(Clone)]
struct MeteredCapabilityRegistry {
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

/// Wrap exact mutation/query registries with PII-free deprecated-contract counters.
///
/// The callback receives a deterministic Prometheus renderer sharing the same
/// counters. Recording remains synchronous and cannot alter gateway outcomes.
pub fn meter_contract_registries<F>(
    catalog: &'static [(&'static str, &'static str, &'static str, &'static str, u32)],
    registries: [Arc<dyn CapabilityRegistryPort>; 2],
    definitions: [&[CapabilityDefinition]; 2],
    publish_metrics: F,
) -> Result<[Arc<dyn CapabilityRegistryPort>; 2], SdkError>
where
    F: FnOnce(Arc<dyn Fn() -> String + Send + Sync>),
{
    let metrics = Arc::new(ContractUsageMetrics::new(
        catalog,
        definitions[0],
        definitions[1],
    )?);
    let renderer_metrics = Arc::clone(&metrics);
    let renderer: Arc<dyn Fn() -> String + Send + Sync> =
        Arc::new(move || renderer_metrics.render_prometheus());
    publish_metrics(renderer);

    let mutation: Arc<dyn CapabilityRegistryPort> = Arc::new(MeteredCapabilityRegistry {
        inner: Arc::clone(&registries[0]),
        metrics: Arc::clone(&metrics),
        surface: ContractUsageSurface::Mutation,
    });
    let query: Arc<dyn CapabilityRegistryPort> = Arc::new(MeteredCapabilityRegistry {
        inner: Arc::clone(&registries[1]),
        metrics,
        surface: ContractUsageSurface::Query,
    });
    Ok([mutation, query])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_capability_runtime::{CapabilityRisk, PayloadContract};
    use crm_module_sdk::{DataClass, ModuleId, PayloadEncoding, SchemaId, SchemaVersion};

    static CATALOG: &[(&str, &str, &str, &str, u32)] = &[(
        "test.record.create",
        "1.0.0",
        "crm.test",
        "crm_contract_invocations_total",
        30,
    )];

    #[derive(Debug)]
    struct Registry {
        definition: Option<CapabilityDefinition>,
    }

    impl CapabilityRegistryPort for Registry {
        fn resolve<'a>(
            &'a self,
            capability_id: &'a CapabilityId,
            capability_version: &'a CapabilityVersion,
        ) -> PortFuture<'a, Result<Option<CapabilityDefinition>, SdkError>> {
            Box::pin(async move {
                Ok(self.definition.as_ref().and_then(|definition| {
                    (&definition.capability_id == capability_id
                        && &definition.capability_version == capability_version)
                        .then(|| definition.clone())
                }))
            })
        }
    }

    #[tokio::test]
    async fn exact_resolution_increments_and_wrong_surface_does_not() {
        let definition = definition("crm.test");
        let mut render = None;
        let registries: [Arc<dyn CapabilityRegistryPort>; 2] = [
            Arc::new(Registry {
                definition: Some(definition.clone()),
            }),
            Arc::new(Registry {
                definition: Some(definition.clone()),
            }),
        ];
        let [mutation, query] = meter_contract_registries(
            CATALOG,
            registries,
            [&[definition.clone()], &[]],
            |renderer| render = Some(renderer),
        )
        .unwrap();
        let render = render.unwrap();
        assert!(render().ends_with(" 0\n"));

        assert!(
            query
                .resolve(&definition.capability_id, &definition.capability_version)
                .await
                .unwrap()
                .is_some()
        );
        assert!(render().ends_with(" 0\n"));
        assert!(
            mutation
                .resolve(&definition.capability_id, &definition.capability_version)
                .await
                .unwrap()
                .is_some()
        );
        assert!(render().ends_with(" 1\n"));
    }

    #[tokio::test]
    async fn unknown_resolution_does_not_increment() {
        let definition = definition("crm.test");
        let mut render = None;
        let registries: [Arc<dyn CapabilityRegistryPort>; 2] = [
            Arc::new(Registry {
                definition: Some(definition.clone()),
            }),
            Arc::new(Registry { definition: None }),
        ];
        let [mutation, _] = meter_contract_registries(
            CATALOG,
            registries,
            [&[definition.clone()], &[]],
            |renderer| render = Some(renderer),
        )
        .unwrap();
        let missing = CapabilityId::try_new("test.record.missing").unwrap();
        assert!(
            mutation
                .resolve(&missing, &definition.capability_version)
                .await
                .unwrap()
                .is_none()
        );
        assert!(render.unwrap()().ends_with(" 0\n"));
    }

    #[test]
    fn catalog_is_fail_closed_against_live_owner_and_coordinate() {
        let no_registry: Arc<dyn CapabilityRegistryPort> = Arc::new(Registry { definition: None });
        let error = meter_contract_registries(
            CATALOG,
            [Arc::clone(&no_registry), Arc::clone(&no_registry)],
            [&[], &[]],
            |_| {},
        )
        .err()
        .expect("invalid telemetry catalog must fail");
        assert_eq!(error.code, "CONTRACT_USAGE_TELEMETRY_INVALID");

        let wrong_owner = definition("crm.other");
        let error = meter_contract_registries(
            CATALOG,
            [Arc::clone(&no_registry), no_registry],
            [&[wrong_owner], &[]],
            |_| {},
        )
        .err()
        .expect("invalid telemetry catalog must fail");
        assert_eq!(error.code, "CONTRACT_USAGE_TELEMETRY_INVALID");
    }

    #[test]
    fn prometheus_output_is_deterministic_zero_seeded_and_pii_free() {
        let definition = definition("crm.test");
        let no_registry: Arc<dyn CapabilityRegistryPort> = Arc::new(Registry { definition: None });
        let mut render = None;
        meter_contract_registries(
            CATALOG,
            [Arc::clone(&no_registry), no_registry],
            [&[definition], &[]],
            |renderer| render = Some(renderer),
        )
        .unwrap();
        let render = render.unwrap();
        let first = render();
        let second = render();

        assert_eq!(first, second);
        assert!(first.contains("crm_contract_invocations_total"));
        assert!(first.contains("capability_id=\"test.record.create\""));
        assert!(first.contains("surface=\"mutation\""));
        assert!(first.ends_with(" 0\n"));
        assert!(!first.contains("tenant"));
        assert!(!first.contains("actor"));
        assert!(!first.contains("request_id"));
    }

    fn definition(owner: &str) -> CapabilityDefinition {
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
            mutation: true,
            requires_idempotency: true,
            requires_approval: false,
            authorization_policy_id: "test.record.create".to_owned(),
            rate_limit_policy_id: None,
        }
    }
}
