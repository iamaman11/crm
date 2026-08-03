use crm_capability_runtime::{CapabilityDefinition, CapabilityRegistryPort};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, ErrorCategory, EventDelivery, PortFuture, SdkError,
};
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
struct CapabilityCoordinate {
    capability_id: String,
    capability_version: String,
}

impl CapabilityCoordinate {
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct EventDeliveryCoordinate {
    event_type: String,
    event_version: String,
    provider_module_id: String,
    consumer_module_id: String,
}

impl EventDeliveryCoordinate {
    fn from_catalog(entry: &DeprecatedEventDelivery) -> Self {
        Self {
            event_type: entry.event_type.to_owned(),
            event_version: entry.event_version.to_owned(),
            provider_module_id: entry.provider_module_id.to_owned(),
            consumer_module_id: entry.consumer_module_id.to_owned(),
        }
    }

    fn from_delivery(delivery: &EventDelivery) -> Self {
        Self {
            event_type: delivery.event_type.as_str().to_owned(),
            event_version: delivery.event_version.as_str().to_owned(),
            provider_module_id: delivery.source_module_id.as_str().to_owned(),
            consumer_module_id: delivery.consumer_module_id.as_str().to_owned(),
        }
    }

    fn display(&self) -> String {
        format!(
            "{}@{}:{}->{}",
            self.event_type, self.event_version, self.provider_module_id, self.consumer_module_id
        )
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

#[derive(Debug, Clone, Copy)]
struct DeprecatedEventDelivery {
    event_type: &'static str,
    event_version: &'static str,
    provider_module_id: &'static str,
    consumer_module_id: &'static str,
    metric: &'static str,
    lookback_days: u32,
}

impl
    From<&'static (
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        u32,
    )> for DeprecatedEventDelivery
{
    fn from(
        value: &'static (
            &'static str,
            &'static str,
            &'static str,
            &'static str,
            &'static str,
            u32,
        ),
    ) -> Self {
        Self {
            event_type: value.0,
            event_version: value.1,
            provider_module_id: value.2,
            consumer_module_id: value.3,
            metric: value.4,
            lookback_days: value.5,
        }
    }
}

#[derive(Debug, Clone)]
struct PublishedDefinition {
    owner_module_id: String,
    surface: ContractUsageSurface,
}

#[derive(Debug)]
struct CapabilityMetricSeries {
    owner_module_id: &'static str,
    metric: &'static str,
    lookback_days: u32,
    surface: ContractUsageSurface,
    count: AtomicU64,
}

#[derive(Debug)]
struct EventMetricSeries {
    metric: &'static str,
    lookback_days: u32,
    count: AtomicU64,
}

#[derive(Debug, Default)]
struct ContractUsageMetrics {
    capability_series: BTreeMap<CapabilityCoordinate, CapabilityMetricSeries>,
    event_series: BTreeMap<EventDeliveryCoordinate, EventMetricSeries>,
}

impl ContractUsageMetrics {
    fn new(
        capability_catalog: &'static [(
            &'static str,
            &'static str,
            &'static str,
            &'static str,
            u32,
        )],
        event_catalog: &'static [(
            &'static str,
            &'static str,
            &'static str,
            &'static str,
            &'static str,
            u32,
        )],
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

        let mut capability_series = BTreeMap::new();
        for raw_entry in capability_catalog {
            let entry = DeprecatedContract::from(raw_entry);
            let coordinate = CapabilityCoordinate::from_catalog(&entry);
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
            let series = CapabilityMetricSeries {
                owner_module_id: entry.owner_module_id,
                metric: entry.metric,
                lookback_days: entry.lookback_days,
                surface: definition.surface,
                count: AtomicU64::new(0),
            };
            if capability_series
                .insert(coordinate.clone(), series)
                .is_some()
            {
                return Err(configuration_error(format!(
                    "deprecated telemetry contract is duplicated: {}",
                    coordinate.display()
                )));
            }
        }

        let mut event_series = BTreeMap::new();
        for raw_entry in event_catalog {
            let entry = DeprecatedEventDelivery::from(raw_entry);
            let coordinate = EventDeliveryCoordinate::from_catalog(&entry);
            let series = EventMetricSeries {
                metric: entry.metric,
                lookback_days: entry.lookback_days,
                count: AtomicU64::new(0),
            };
            if event_series.insert(coordinate.clone(), series).is_some() {
                return Err(configuration_error(format!(
                    "deprecated event delivery telemetry is duplicated: {}",
                    coordinate.display()
                )));
            }
        }

        Ok(Self {
            capability_series,
            event_series,
        })
    }

    fn record_resolved(&self, definition: &CapabilityDefinition, surface: ContractUsageSurface) {
        let coordinate = CapabilityCoordinate::from_definition(definition);
        let Some(series) = self.capability_series.get(&coordinate) else {
            return;
        };
        if series.surface != surface
            || series.owner_module_id != definition.owner_module_id.as_str()
        {
            return;
        }
        increment(&series.count);
    }

    fn record_event_delivery(&self, delivery: &EventDelivery) {
        let coordinate = EventDeliveryCoordinate::from_delivery(delivery);
        if let Some(series) = self.event_series.get(&coordinate) {
            increment(&series.count);
        }
    }

    fn render_prometheus(&self) -> String {
        let metrics: BTreeSet<_> = self
            .capability_series
            .values()
            .map(|series| series.metric)
            .chain(self.event_series.values().map(|series| series.metric))
            .collect();
        let mut output = String::new();
        for metric in metrics {
            output.push_str("# HELP ");
            output.push_str(metric);
            output.push_str(" Observed usage of deprecated exact-version contracts.\n");
            output.push_str("# TYPE ");
            output.push_str(metric);
            output.push_str(" counter\n");
            self.render_capability_series(metric, &mut output);
            self.render_event_series(metric, &mut output);
        }
        output
    }

    fn render_capability_series(&self, metric: &str, output: &mut String) {
        for (coordinate, series) in &self.capability_series {
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

    fn render_event_series(&self, metric: &str, output: &mut String) {
        for (coordinate, series) in &self.event_series {
            if series.metric != metric {
                continue;
            }
            output.push_str(metric);
            output.push_str("{event_type=\"");
            output.push_str(&coordinate.event_type);
            output.push_str("\",event_version=\"");
            output.push_str(&coordinate.event_version);
            output.push_str("\",provider_module_id=\"");
            output.push_str(&coordinate.provider_module_id);
            output.push_str("\",consumer_module_id=\"");
            output.push_str(&coordinate.consumer_module_id);
            output.push_str("\",lookback_days=\"");
            output.push_str(&series.lookback_days.to_string());
            output.push_str("\"} ");
            output.push_str(&series.count.load(Ordering::Relaxed).to_string());
            output.push('\n');
        }
    }
}

fn increment(count: &AtomicU64) {
    let _ = count.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
        current.checked_add(1)
    });
}

fn add_published_definitions(
    published: &mut BTreeMap<CapabilityCoordinate, PublishedDefinition>,
    definitions: &[CapabilityDefinition],
    surface: ContractUsageSurface,
) -> Result<(), SdkError> {
    for definition in definitions {
        let coordinate = CapabilityCoordinate::from_definition(definition);
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

/// Wrap exact mutation/query registries and publish PII-free deprecated-contract telemetry.
///
/// Event delivery observation and rendering share the same counters. Recording
/// remains synchronous and cannot alter capability gateway outcomes.
pub fn meter_contract_registries<F, G>(
    capability_catalog: &'static [(&'static str, &'static str, &'static str, &'static str, u32)],
    event_catalog: &'static [(
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        u32,
    )],
    registries: [Arc<dyn CapabilityRegistryPort>; 2],
    definitions: [&[CapabilityDefinition]; 2],
    publish_event_observer: F,
    publish_metrics: G,
) -> Result<[Arc<dyn CapabilityRegistryPort>; 2], SdkError>
where
    F: FnOnce(Arc<dyn Fn(&EventDelivery) + Send + Sync>),
    G: FnOnce(Arc<dyn Fn() -> String + Send + Sync>),
{
    let metrics = Arc::new(ContractUsageMetrics::new(
        capability_catalog,
        event_catalog,
        definitions[0],
        definitions[1],
    )?);
    let observer_metrics = Arc::clone(&metrics);
    publish_event_observer(Arc::new(move |delivery| {
        observer_metrics.record_event_delivery(delivery);
    }));
    let renderer_metrics = Arc::clone(&metrics);
    publish_metrics(Arc::new(move || renderer_metrics.render_prometheus()));

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
    use crm_module_sdk::{
        ActorId, CorrelationId, DataClass, DeliveryId, EventId, EventType, EventVersion, ModuleId,
        PayloadEncoding, RecordId, RecordRef, RecordType, RetentionPolicyId, SchemaId,
        SchemaVersion, TenantId, TraceId, TypedPayload,
    };

    static CAPABILITY_CATALOG: &[(&str, &str, &str, &str, u32)] = &[(
        "test.record.create",
        "1.0.0",
        "crm.test",
        "crm_contract_usage_total",
        30,
    )];
    static EVENT_CATALOG: &[(&str, &str, &str, &str, &str, u32)] = &[(
        "test.record.created",
        "1.0.0",
        "crm.test",
        "crm.consumer",
        "crm_contract_usage_total",
        45,
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
    async fn exact_capability_resolution_and_event_delivery_increment() {
        let definition = definition("crm.test");
        let registries: [Arc<dyn CapabilityRegistryPort>; 2] = [
            Arc::new(Registry {
                definition: Some(definition.clone()),
            }),
            Arc::new(Registry {
                definition: Some(definition.clone()),
            }),
        ];
        let mut observe = None;
        let mut render = None;
        let [mutation, query] = meter_contract_registries(
            CAPABILITY_CATALOG,
            EVENT_CATALOG,
            registries,
            [std::slice::from_ref(&definition), &[]],
            |observer| observe = Some(observer),
            |renderer| render = Some(renderer),
        )
        .unwrap();
        let observe = observe.unwrap();
        let render = render.unwrap();
        assert_eq!(render().matches(" 0\n").count(), 2);

        assert!(
            query
                .resolve(&definition.capability_id, &definition.capability_version)
                .await
                .unwrap()
                .is_some()
        );
        assert_eq!(render().matches(" 0\n").count(), 2);
        assert!(
            mutation
                .resolve(&definition.capability_id, &definition.capability_version)
                .await
                .unwrap()
                .is_some()
        );
        observe(&delivery("crm.test", "crm.consumer", "1.0.0"));
        assert_eq!(render().matches(" 1\n").count(), 2);
    }

    #[test]
    fn wrong_event_provider_consumer_or_version_does_not_increment() {
        let definition = definition("crm.test");
        let no_registry: Arc<dyn CapabilityRegistryPort> = Arc::new(Registry { definition: None });
        let mut observe = None;
        let mut render = None;
        meter_contract_registries(
            CAPABILITY_CATALOG,
            EVENT_CATALOG,
            [Arc::clone(&no_registry), no_registry],
            [&[definition], &[]],
            |observer| observe = Some(observer),
            |renderer| render = Some(renderer),
        )
        .unwrap();
        let observe = observe.unwrap();
        let render = render.unwrap();

        observe(&delivery("crm.other", "crm.consumer", "1.0.0"));
        observe(&delivery("crm.test", "crm.other", "1.0.0"));
        observe(&delivery("crm.test", "crm.consumer", "2.0.0"));
        assert_eq!(render().matches(" 0\n").count(), 2);
    }

    #[tokio::test]
    async fn unknown_resolution_does_not_increment() {
        let definition = definition("crm.test");
        let registries: [Arc<dyn CapabilityRegistryPort>; 2] = [
            Arc::new(Registry {
                definition: Some(definition.clone()),
            }),
            Arc::new(Registry { definition: None }),
        ];
        let mut render = None;
        let [mutation, _] = meter_contract_registries(
            CAPABILITY_CATALOG,
            &[],
            registries,
            [std::slice::from_ref(&definition), &[]],
            |_| {},
            |renderer| render = Some(renderer),
        )
        .unwrap();
        let render = render.unwrap();
        let missing = CapabilityId::try_new("test.record.missing").unwrap();
        assert!(
            mutation
                .resolve(&missing, &definition.capability_version)
                .await
                .unwrap()
                .is_none()
        );
        assert!(render().ends_with(" 0\n"));
    }

    #[test]
    fn catalogs_fail_closed_against_live_capabilities_and_duplicates() {
        let no_registry: Arc<dyn CapabilityRegistryPort> = Arc::new(Registry { definition: None });
        let error = meter_contract_registries(
            CAPABILITY_CATALOG,
            &[],
            [Arc::clone(&no_registry), Arc::clone(&no_registry)],
            [&[], &[]],
            |_| {},
            |_| {},
        )
        .err()
        .expect("invalid telemetry catalog must fail");
        assert_eq!(error.code, "CONTRACT_USAGE_TELEMETRY_INVALID");

        let wrong_owner = definition("crm.other");
        let error = meter_contract_registries(
            CAPABILITY_CATALOG,
            &[],
            [Arc::clone(&no_registry), Arc::clone(&no_registry)],
            [&[wrong_owner], &[]],
            |_| {},
            |_| {},
        )
        .err()
        .expect("invalid telemetry catalog must fail");
        assert_eq!(error.code, "CONTRACT_USAGE_TELEMETRY_INVALID");

        let duplicate_events = [EVENT_CATALOG[0], EVENT_CATALOG[0]];
        let error = meter_contract_registries(
            &[],
            Box::leak(Box::new(duplicate_events)),
            [Arc::clone(&no_registry), no_registry],
            [&[], &[]],
            |_| {},
            |_| {},
        )
        .err()
        .expect("duplicate event telemetry must fail");
        assert_eq!(error.code, "CONTRACT_USAGE_TELEMETRY_INVALID");
    }

    #[test]
    fn prometheus_output_is_deterministic_zero_seeded_and_pii_free() {
        let definition = definition("crm.test");
        let no_registry: Arc<dyn CapabilityRegistryPort> = Arc::new(Registry { definition: None });
        let mut render = None;
        meter_contract_registries(
            CAPABILITY_CATALOG,
            EVENT_CATALOG,
            [Arc::clone(&no_registry), no_registry],
            [&[definition], &[]],
            |_| {},
            |renderer| render = Some(renderer),
        )
        .unwrap();
        let render = render.unwrap();
        let first = render();
        let second = render();

        assert_eq!(first, second);
        assert_eq!(first.matches("# HELP crm_contract_usage_total").count(), 1);
        assert!(first.contains("capability_id=\"test.record.create\""));
        assert!(first.contains("surface=\"mutation\""));
        assert!(first.contains("event_type=\"test.record.created\""));
        assert!(first.contains("provider_module_id=\"crm.test\""));
        assert!(first.contains("consumer_module_id=\"crm.consumer\""));
        assert_eq!(first.matches(" 0\n").count(), 2);
        for forbidden in [
            "tenant",
            "actor",
            "request_id",
            "correlation",
            "trace",
            "payload",
            "aggregate",
            "delivery_id",
            "event_id",
            "status",
            "error",
        ] {
            assert!(!first.contains(forbidden), "forbidden label: {forbidden}");
        }
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

    fn delivery(provider: &str, consumer: &str, version: &str) -> EventDelivery {
        EventDelivery {
            delivery_id: DeliveryId::try_new("delivery-1").unwrap(),
            event_id: EventId::try_new("event-1").unwrap(),
            tenant_id: TenantId::try_new("tenant-1").unwrap(),
            source_module_id: ModuleId::try_new(provider).unwrap(),
            consumer_module_id: ModuleId::try_new(consumer).unwrap(),
            source_actor_id: ActorId::try_new("actor-1").unwrap(),
            event_type: EventType::try_new("test.record.created").unwrap(),
            event_version: EventVersion::try_new(version).unwrap(),
            aggregate: RecordRef {
                record_type: RecordType::try_new("test.record").unwrap(),
                record_id: RecordId::try_new("record-1").unwrap(),
            },
            aggregate_version: 1,
            occurred_at_unix_nanos: 1,
            correlation_id: CorrelationId::try_new("correlation-1").unwrap(),
            trace_id: TraceId::try_new("trace-1").unwrap(),
            payload: TypedPayload {
                owner: ModuleId::try_new(provider).unwrap(),
                schema_id: SchemaId::try_new("crm.test.v1.RecordCreated").unwrap(),
                schema_version: SchemaVersion::try_new(version).unwrap(),
                descriptor_hash: [0x52; 32],
                data_class: DataClass::Internal,
                encoding: PayloadEncoding::Protobuf,
                maximum_size_bytes: 1024,
                retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
                bytes: Vec::new(),
            },
        }
    }
}
