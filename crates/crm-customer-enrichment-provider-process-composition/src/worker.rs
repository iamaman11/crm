use crate::{ProviderDispatchWorkItemInput, build_provider_dispatch_work_item};
use crm_application_composition::TenantBackgroundWorker;
use crm_capability_plan_support as support;
use crm_core_data::PostgresDataStore;
use crm_core_events::{
    EventHistoryRequest, ProjectionEventApplication, ProjectionFailure, ProjectionStore,
};
use crm_customer_enrichment::{
    ENRICHMENT_REQUEST_RECORD_TYPE, EnrichmentRequest, PartySnapshot, ProviderProfileVersion,
};
use crm_customer_enrichment_capability_adapter::{
    ENRICHMENT_REQUEST_CREATED_EVENT_SCHEMA, ENRICHMENT_REQUEST_CREATED_EVENT_TYPE, MODULE_ID,
};
use crm_customer_enrichment_worker_composition::{
    CustomerEnrichmentProviderWorker, ProviderDispatchWorkItem, ProviderDispatchWorkerResult,
};
use crm_module_sdk::{
    ActorId, DataClass, EventDelivery, EventType, ModuleId, PayloadEncoding, PortFuture, RecordId,
    SdkError, TenantId,
};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;
use prost::Message;
use std::fmt;
use std::sync::Arc;

pub const PROVIDER_PROCESS_WORKER_ID: &str = "customer-enrichment-provider-process";
pub const PROVIDER_PROCESS_PROJECTION_ID: &str = "customer-enrichment-provider-process-v1";
pub const PROVIDER_PROCESS_WORKER_ACTOR_ID: &str = "customer-enrichment-provider-worker";
const DEFAULT_PAGE_SIZE: u32 = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderDispatchSourceSnapshot {
    pub request: EnrichmentRequest,
    pub provider_profile: ProviderProfileVersion,
    pub party_snapshot: PartySnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderDispatchSourceDisposition {
    Ready(ProviderDispatchSourceSnapshot),
    Skip,
}

pub trait ProviderDispatchSourcePort: Send + Sync {
    fn load<'a>(
        &'a self,
        tenant_id: TenantId,
        request_id: RecordId,
        worker_actor_id: ActorId,
        now_unix_ms: u64,
    ) -> PortFuture<'a, Result<ProviderDispatchSourceDisposition, SdkError>>;
}

pub trait ProviderDispatchExecutorPort: Send + Sync {
    fn execute<'a>(
        &'a self,
        work_item: ProviderDispatchWorkItem,
    ) -> PortFuture<'a, Result<ProviderDispatchWorkerResult, SdkError>>;
}

impl ProviderDispatchExecutorPort for CustomerEnrichmentProviderWorker {
    fn execute<'a>(
        &'a self,
        work_item: ProviderDispatchWorkItem,
    ) -> PortFuture<'a, Result<ProviderDispatchWorkerResult, SdkError>> {
        Box::pin(async move { CustomerEnrichmentProviderWorker::execute(self, work_item).await })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ProviderProcessCycle {
    pub created_events: u32,
    pub dispatched: u32,
    pub skipped: u32,
    pub dispatch_replays: u32,
    pub response_replays: u32,
}

#[derive(Clone)]
pub struct CustomerEnrichmentProviderProcessWorker {
    store: PostgresDataStore,
    source: Arc<dyn ProviderDispatchSourcePort>,
    executor: Arc<dyn ProviderDispatchExecutorPort>,
    actor_id: ActorId,
    page_size: u32,
}

impl fmt::Debug for CustomerEnrichmentProviderProcessWorker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CustomerEnrichmentProviderProcessWorker")
            .field("store", &self.store)
            .field("source", &"dyn ProviderDispatchSourcePort")
            .field("executor", &"dyn ProviderDispatchExecutorPort")
            .field("actor_id", &self.actor_id)
            .field("page_size", &self.page_size)
            .finish()
    }
}

impl CustomerEnrichmentProviderProcessWorker {
    pub fn new(
        store: PostgresDataStore,
        source: Arc<dyn ProviderDispatchSourcePort>,
        executor: Arc<dyn ProviderDispatchExecutorPort>,
        actor_id: ActorId,
    ) -> Result<Self, SdkError> {
        Self::with_page_size(store, source, executor, actor_id, DEFAULT_PAGE_SIZE)
    }

    pub fn with_page_size(
        store: PostgresDataStore,
        source: Arc<dyn ProviderDispatchSourcePort>,
        executor: Arc<dyn ProviderDispatchExecutorPort>,
        actor_id: ActorId,
        page_size: u32,
    ) -> Result<Self, SdkError> {
        if page_size == 0 || page_size > crm_core_events::MAX_EVENT_HISTORY_PAGE_SIZE {
            return Err(worker_configuration_invalid(
                "provider-process page size is outside the governed limit",
            ));
        }
        Ok(Self {
            store,
            source,
            executor,
            actor_id,
            page_size,
        })
    }

    pub fn run_cycle<'a>(
        &'a self,
        tenant_id: TenantId,
        now_unix_nanos: i64,
    ) -> PortFuture<'a, Result<ProviderProcessCycle, SdkError>> {
        Box::pin(async move {
            let now_unix_ms = current_time_ms(now_unix_nanos)?;
            let module_id = module_id()?;
            let event_type = event_type()?;
            let checkpoint = ProjectionStore::projection_checkpoint(
                &self.store,
                tenant_id.clone(),
                PROVIDER_PROCESS_PROJECTION_ID.to_owned(),
            )
            .await?;
            let mut after = checkpoint.map(|value| value.cursor);
            let mut cycle = ProviderProcessCycle::default();

            loop {
                let page = ProjectionStore::list_event_history(
                    &self.store,
                    EventHistoryRequest {
                        tenant_id: tenant_id.clone(),
                        consumer_module_id: module_id.clone(),
                        event_types: vec![event_type.clone()],
                        after: after.clone(),
                        page_size: self.page_size,
                    },
                )
                .await?;

                if page.deliveries.is_empty() {
                    return Ok(cycle);
                }
                let next_cursor = page.next_cursor.clone();
                for delivery in page.deliveries {
                    cycle.created_events = cycle.created_events.saturating_add(1);
                    match self
                        .process_delivery(&tenant_id, now_unix_ms, &delivery)
                        .await
                    {
                        Ok(DeliveryDisposition::Executed(result)) => {
                            cycle.dispatched = cycle.dispatched.saturating_add(1);
                            if result.dispatch_replayed {
                                cycle.dispatch_replays = cycle.dispatch_replays.saturating_add(1);
                            }
                            if result.response_replayed {
                                cycle.response_replays = cycle.response_replays.saturating_add(1);
                            }
                        }
                        Ok(DeliveryDisposition::Skipped) => {
                            cycle.skipped = cycle.skipped.saturating_add(1);
                        }
                        Err(error) => {
                            if !error.retryable {
                                let _ = ProjectionStore::mark_projection_failed(
                                    &self.store,
                                    ProjectionFailure {
                                        tenant_id: tenant_id.clone(),
                                        projection_id: PROVIDER_PROCESS_PROJECTION_ID.to_owned(),
                                        event_id: delivery.event_id.clone(),
                                        occurred_at_unix_nanos: delivery.occurred_at_unix_nanos,
                                        failure_code: error.code.clone(),
                                    },
                                )
                                .await;
                            }
                            return Err(error);
                        }
                    }
                    ProjectionStore::apply_projection_event(
                        &self.store,
                        ProjectionEventApplication {
                            projection_id: PROVIDER_PROCESS_PROJECTION_ID.to_owned(),
                            delivery,
                            writes: Vec::new(),
                        },
                    )
                    .await?;
                }

                let Some(next) = next_cursor else {
                    return Ok(cycle);
                };
                after = Some(next);
            }
        })
    }

    async fn process_delivery(
        &self,
        tenant_id: &TenantId,
        now_unix_ms: u64,
        delivery: &EventDelivery,
    ) -> Result<DeliveryDisposition, SdkError> {
        let event = decode_created_event(delivery)?;
        let created = event.enrichment_request.ok_or_else(created_event_invalid)?;
        let request_ref = created
            .enrichment_request_ref
            .as_ref()
            .ok_or_else(created_event_invalid)?;
        if tenant_id != &delivery.tenant_id
            || delivery.aggregate.record_type.as_str() != ENRICHMENT_REQUEST_RECORD_TYPE
            || delivery.aggregate.record_id.as_str() != request_ref.enrichment_request_id
            || wire::EnrichmentRequestStatus::try_from(created.status).ok()
                != Some(wire::EnrichmentRequestStatus::Created)
        {
            return Err(created_event_invalid());
        }
        let request_id = RecordId::try_new(request_ref.enrichment_request_id.clone())
            .map_err(worker_identifier_invalid)?;
        let source = self
            .source
            .load(
                tenant_id.clone(),
                request_id.clone(),
                self.actor_id.clone(),
                now_unix_ms,
            )
            .await?;
        let ProviderDispatchSourceDisposition::Ready(source) = source else {
            return Ok(DeliveryDisposition::Skipped);
        };
        let party_resource_version = u64::try_from(source.party_snapshot.resource_version)
            .map_err(|_| source_snapshot_invalid())?;
        let party_observed_at_unix_ms = u64::try_from(source.party_snapshot.observed_at_unix_ms)
            .map_err(|_| source_snapshot_invalid())?;
        if source.request.request_id().as_str() != request_id.as_str()
            || source.request.tenant_id() != tenant_id
            || source.provider_profile.version_id() != source.request.provider_profile_version_id()
            || source.party_snapshot.party_id.as_str() != source.request.target().resource_id
            || party_resource_version != source.request.target().resource_version
            || party_observed_at_unix_ms > now_unix_ms
        {
            return Err(source_snapshot_invalid());
        }
        let work_item = build_provider_dispatch_work_item(ProviderDispatchWorkItemInput {
            request: &source.request,
            provider_profile: &source.provider_profile,
            party_snapshot: &source.party_snapshot,
            worker_actor_id: &self.actor_id,
            now_unix_ms,
        })?;
        let result = self.executor.execute(work_item).await?;
        Ok(DeliveryDisposition::Executed(result))
    }
}

impl TenantBackgroundWorker for CustomerEnrichmentProviderProcessWorker {
    fn run_tenant_cycle<'a>(
        &'a self,
        tenant_id: TenantId,
        now_unix_nanos: i64,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            self.run_cycle(tenant_id, now_unix_nanos).await?;
            Ok(())
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
enum DeliveryDisposition {
    Executed(ProviderDispatchWorkerResult),
    Skipped,
}

fn decode_created_event(
    delivery: &EventDelivery,
) -> Result<wire::EnrichmentRequestCreatedEvent, SdkError> {
    delivery.validate()?;
    let module_id = module_id()?;
    let contract = support::protobuf_contract(
        MODULE_ID,
        ENRICHMENT_REQUEST_CREATED_EVENT_SCHEMA,
        vec![DataClass::Personal],
    )?;
    if delivery.source_module_id != module_id
        || delivery.consumer_module_id != module_id
        || delivery.event_type.as_str() != ENRICHMENT_REQUEST_CREATED_EVENT_TYPE
        || delivery.event_version.as_str() != support::CONTRACT_VERSION
        || !contract.matches(&delivery.payload)
        || delivery.payload.encoding != PayloadEncoding::Protobuf
    {
        return Err(created_event_invalid());
    }
    wire::EnrichmentRequestCreatedEvent::decode(delivery.payload.bytes.as_slice()).map_err(
        |error| {
            created_event_invalid()
                .with_internal_reference(format!("created event decode: {error}"))
        },
    )
}

fn current_time_ms(now_unix_nanos: i64) -> Result<u64, SdkError> {
    if now_unix_nanos <= 0 {
        return Err(worker_configuration_invalid(
            "provider-process worker clock is invalid",
        ));
    }
    let now_unix_ms = u64::try_from(now_unix_nanos / 1_000_000).map_err(|_| {
        worker_configuration_invalid("provider-process worker clock cannot be represented")
    })?;
    if now_unix_ms == 0 {
        return Err(worker_configuration_invalid(
            "provider-process worker clock has sub-millisecond precision only",
        ));
    }
    Ok(now_unix_ms)
}

fn module_id() -> Result<ModuleId, SdkError> {
    ModuleId::try_new(MODULE_ID).map_err(worker_identifier_invalid)
}

fn event_type() -> Result<EventType, SdkError> {
    EventType::try_new(ENRICHMENT_REQUEST_CREATED_EVENT_TYPE).map_err(worker_identifier_invalid)
}

fn worker_identifier_invalid(error: crm_module_sdk::IdentifierError) -> SdkError {
    worker_configuration_invalid(error.to_string())
}

fn worker_configuration_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_PROCESS_CONFIGURATION_INVALID",
        crm_module_sdk::ErrorCategory::Internal,
        false,
        "The Customer Enrichment provider process is not configured safely.",
    )
    .with_internal_reference(reference.into())
}

fn created_event_invalid() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_REQUEST_CREATED_EVENT_INVALID",
        crm_module_sdk::ErrorCategory::Internal,
        false,
        "Customer Enrichment request-created evidence is invalid.",
    )
}

fn source_snapshot_invalid() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_SOURCE_SNAPSHOT_INVALID",
        crm_module_sdk::ErrorCategory::Internal,
        false,
        "The provider process source snapshot is invalid.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_module_sdk::{
        ActorId, CorrelationId, DeliveryId, EventId, EventVersion, RecordId, RecordRef, RecordType,
        TraceId,
    };

    #[test]
    fn worker_identity_and_event_coordinate_are_canonical() {
        assert!(ActorId::try_new(PROVIDER_PROCESS_WORKER_ACTOR_ID).is_ok());
        assert!(EventType::try_new(ENRICHMENT_REQUEST_CREATED_EVENT_TYPE).is_ok());
        assert_eq!(
            PROVIDER_PROCESS_PROJECTION_ID,
            "customer-enrichment-provider-process-v1"
        );
    }

    #[test]
    fn worker_clock_requires_a_positive_representable_millisecond() {
        let error = current_time_ms(999_999).unwrap_err();
        assert_eq!(
            error.code,
            "CUSTOMER_ENRICHMENT_PROVIDER_PROCESS_CONFIGURATION_INVALID"
        );
        assert_eq!(current_time_ms(1_999_999).unwrap(), 1);
    }

    #[test]
    fn decodes_only_self_consumed_request_created_evidence() {
        let payload = support::protobuf_payload(
            MODULE_ID,
            ENRICHMENT_REQUEST_CREATED_EVENT_SCHEMA,
            DataClass::Personal,
            &wire::EnrichmentRequestCreatedEvent {
                enrichment_request: None,
            },
        )
        .unwrap();
        let mut delivery = EventDelivery {
            delivery_id: DeliveryId::try_new("provider-created-delivery").unwrap(),
            event_id: EventId::try_new("provider-created-event").unwrap(),
            tenant_id: TenantId::try_new("tenant-1").unwrap(),
            source_module_id: module_id().unwrap(),
            consumer_module_id: module_id().unwrap(),
            source_actor_id: ActorId::try_new("requester-1").unwrap(),
            event_type: event_type().unwrap(),
            event_version: EventVersion::try_new(support::CONTRACT_VERSION).unwrap(),
            aggregate: RecordRef {
                record_type: RecordType::try_new(ENRICHMENT_REQUEST_RECORD_TYPE).unwrap(),
                record_id: RecordId::try_new("request-1").unwrap(),
            },
            aggregate_version: 1,
            occurred_at_unix_nanos: 1_000_000,
            correlation_id: CorrelationId::try_new("correlation-1").unwrap(),
            trace_id: TraceId::try_new("trace-1").unwrap(),
            payload,
        };
        assert!(decode_created_event(&delivery).is_ok());

        delivery.consumer_module_id = ModuleId::try_new("crm.parties").unwrap();
        let error = decode_created_event(&delivery).unwrap_err();
        assert_eq!(
            error.code,
            "CUSTOMER_ENRICHMENT_REQUEST_CREATED_EVENT_INVALID"
        );
    }
}
