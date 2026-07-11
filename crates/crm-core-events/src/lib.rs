#![forbid(unsafe_code)]

//! Governed event-delivery orchestration shared by link, workflow and automation runtimes.
//!
//! This crate contains no broker, database or business-module implementation. It defines the
//! stable host-side ports that turn immutable published events into consumer-scoped deliveries,
//! gate execution on the consumer module lifecycle and invoke one governed handler.

use crm_module_sdk::{
    DeliveryId, EventDelivery, EventId, ModuleId, PortFuture, PortResult, ResourceRef, SdkError,
    TenantId,
};
use std::fmt;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventDeliveryLookup {
    pub tenant_id: TenantId,
    pub consumer_module_id: ModuleId,
    pub event_id: EventId,
    pub delivery_id: DeliveryId,
}

pub trait EventDeliveryReader: Send + Sync {
    fn load<'a>(
        &'a self,
        lookup: &'a EventDeliveryLookup,
    ) -> PortFuture<'a, PortResult<Option<EventDelivery>>>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleActivationState {
    Active,
    Inactive,
    Missing,
}

pub trait ModuleActivationReader: Send + Sync {
    fn activation_state<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        module_id: &'a ModuleId,
    ) -> PortFuture<'a, PortResult<ModuleActivationState>>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventHandlingDisposition {
    Applied,
    Ignored,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventHandlingResult {
    pub disposition: EventHandlingDisposition,
    pub replayed: bool,
    pub affected_resources: Vec<ResourceRef>,
}

pub trait GovernedEventHandler: Send + Sync {
    fn handle<'a>(
        &'a self,
        delivery: &'a EventDelivery,
    ) -> PortFuture<'a, PortResult<EventHandlingResult>>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventDeliveryDisposition {
    Applied,
    Ignored,
    SkippedInactive,
    SkippedMissing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventDeliveryResult {
    pub disposition: EventDeliveryDisposition,
    pub replayed: bool,
    pub affected_resources: Vec<ResourceRef>,
}

#[derive(Clone)]
pub struct EventDeliveryRuntime {
    reader: Arc<dyn EventDeliveryReader>,
    activations: Arc<dyn ModuleActivationReader>,
    handler: Arc<dyn GovernedEventHandler>,
}

impl fmt::Debug for EventDeliveryRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EventDeliveryRuntime")
            .field("reader", &"dyn EventDeliveryReader")
            .field("activations", &"dyn ModuleActivationReader")
            .field("handler", &"dyn GovernedEventHandler")
            .finish()
    }
}

impl EventDeliveryRuntime {
    pub fn new(
        reader: Arc<dyn EventDeliveryReader>,
        activations: Arc<dyn ModuleActivationReader>,
        handler: Arc<dyn GovernedEventHandler>,
    ) -> Self {
        Self {
            reader,
            activations,
            handler,
        }
    }

    pub async fn deliver(
        &self,
        lookup: EventDeliveryLookup,
    ) -> Result<EventDeliveryResult, SdkError> {
        match self
            .activations
            .activation_state(&lookup.tenant_id, &lookup.consumer_module_id)
            .await?
        {
            ModuleActivationState::Missing => {
                return Ok(EventDeliveryResult {
                    disposition: EventDeliveryDisposition::SkippedMissing,
                    replayed: false,
                    affected_resources: Vec::new(),
                });
            }
            ModuleActivationState::Inactive => {
                return Ok(EventDeliveryResult {
                    disposition: EventDeliveryDisposition::SkippedInactive,
                    replayed: false,
                    affected_resources: Vec::new(),
                });
            }
            ModuleActivationState::Active => {}
        }

        let delivery = self
            .reader
            .load(&lookup)
            .await?
            .ok_or_else(event_delivery_not_found)?;
        validate_lookup_binding(&lookup, &delivery)?;

        let handled = self.handler.handle(&delivery).await?;
        Ok(EventDeliveryResult {
            disposition: match handled.disposition {
                EventHandlingDisposition::Applied => EventDeliveryDisposition::Applied,
                EventHandlingDisposition::Ignored => EventDeliveryDisposition::Ignored,
            },
            replayed: handled.replayed,
            affected_resources: handled.affected_resources,
        })
    }
}

fn validate_lookup_binding(
    lookup: &EventDeliveryLookup,
    delivery: &EventDelivery,
) -> Result<(), SdkError> {
    delivery.validate()?;
    if delivery.tenant_id != lookup.tenant_id
        || delivery.consumer_module_id != lookup.consumer_module_id
        || delivery.event_id != lookup.event_id
        || delivery.delivery_id != lookup.delivery_id
    {
        return Err(SdkError::new(
            "EVENT_DELIVERY_BINDING_INVALID",
            crm_module_sdk::ErrorCategory::Internal,
            false,
            "The loaded event delivery does not match its immutable lookup identity.",
        ));
    }
    Ok(())
}

fn event_delivery_not_found() -> SdkError {
    SdkError::new(
        "EVENT_DELIVERY_NOT_FOUND",
        crm_module_sdk::ErrorCategory::NotFound,
        false,
        "The source event delivery could not be found.",
    )
}

/// Architecture marker for `crm-core-events`.
pub const CRATE_NAME: &str = "crm-core-events";

#[cfg(test)]
mod tests {
    use super::*;
    use crm_module_sdk::{
        ActorId, CorrelationId, DataClass, EventType, EventVersion, ModuleId, PayloadEncoding,
        RecordId, RecordRef, RecordType, RetentionPolicyId, SchemaId, SchemaVersion, TraceId,
        TypedPayload,
    };
    use std::future::Future;
    use std::sync::Mutex;
    use std::task::{Context, Poll, Waker};

    fn run_ready<F: Future>(future: F) -> F::Output {
        let mut context = Context::from_waker(Waker::noop());
        let mut future = Box::pin(future);
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => output,
            Poll::Pending => panic!("test future unexpectedly returned Pending"),
        }
    }

    #[derive(Debug)]
    struct FixedActivation(ModuleActivationState);

    impl ModuleActivationReader for FixedActivation {
        fn activation_state<'a>(
            &'a self,
            _tenant_id: &'a TenantId,
            _module_id: &'a ModuleId,
        ) -> PortFuture<'a, PortResult<ModuleActivationState>> {
            let state = self.0;
            Box::pin(async move { Ok(state) })
        }
    }

    #[derive(Debug)]
    struct FixedReader {
        delivery: EventDelivery,
        calls: Mutex<usize>,
    }

    impl EventDeliveryReader for FixedReader {
        fn load<'a>(
            &'a self,
            _lookup: &'a EventDeliveryLookup,
        ) -> PortFuture<'a, PortResult<Option<EventDelivery>>> {
            *self.calls.lock().expect("reader mutex poisoned") += 1;
            let delivery = self.delivery.clone();
            Box::pin(async move { Ok(Some(delivery)) })
        }
    }

    #[derive(Debug, Default)]
    struct RecordingHandler {
        calls: Mutex<usize>,
    }

    impl GovernedEventHandler for RecordingHandler {
        fn handle<'a>(
            &'a self,
            _delivery: &'a EventDelivery,
        ) -> PortFuture<'a, PortResult<EventHandlingResult>> {
            *self.calls.lock().expect("handler mutex poisoned") += 1;
            Box::pin(async move {
                Ok(EventHandlingResult {
                    disposition: EventHandlingDisposition::Applied,
                    replayed: false,
                    affected_resources: Vec::new(),
                })
            })
        }
    }

    #[test]
    fn inactive_consumer_is_skipped_before_source_event_read() {
        let reader = Arc::new(FixedReader {
            delivery: delivery(),
            calls: Mutex::new(0),
        });
        let handler = Arc::new(RecordingHandler::default());
        let runtime = EventDeliveryRuntime::new(
            reader.clone(),
            Arc::new(FixedActivation(ModuleActivationState::Inactive)),
            handler.clone(),
        );

        let result = run_ready(runtime.deliver(lookup())).unwrap();

        assert_eq!(result.disposition, EventDeliveryDisposition::SkippedInactive);
        assert_eq!(*reader.calls.lock().unwrap(), 0);
        assert_eq!(*handler.calls.lock().unwrap(), 0);
    }

    #[test]
    fn active_consumer_loads_and_handles_exact_delivery_once() {
        let reader = Arc::new(FixedReader {
            delivery: delivery(),
            calls: Mutex::new(0),
        });
        let handler = Arc::new(RecordingHandler::default());
        let runtime = EventDeliveryRuntime::new(
            reader.clone(),
            Arc::new(FixedActivation(ModuleActivationState::Active)),
            handler.clone(),
        );

        let result = run_ready(runtime.deliver(lookup())).unwrap();

        assert_eq!(result.disposition, EventDeliveryDisposition::Applied);
        assert_eq!(*reader.calls.lock().unwrap(), 1);
        assert_eq!(*handler.calls.lock().unwrap(), 1);
    }

    #[test]
    fn missing_consumer_is_non_executing() {
        let reader = Arc::new(FixedReader {
            delivery: delivery(),
            calls: Mutex::new(0),
        });
        let handler = Arc::new(RecordingHandler::default());
        let runtime = EventDeliveryRuntime::new(
            reader.clone(),
            Arc::new(FixedActivation(ModuleActivationState::Missing)),
            handler.clone(),
        );

        let result = run_ready(runtime.deliver(lookup())).unwrap();

        assert_eq!(result.disposition, EventDeliveryDisposition::SkippedMissing);
        assert_eq!(*reader.calls.lock().unwrap(), 0);
        assert_eq!(*handler.calls.lock().unwrap(), 0);
    }

    fn lookup() -> EventDeliveryLookup {
        EventDeliveryLookup {
            tenant_id: TenantId::try_new("tenant-a").unwrap(),
            consumer_module_id: ModuleId::try_new("crm.link").unwrap(),
            event_id: EventId::try_new("event-1").unwrap(),
            delivery_id: DeliveryId::try_new("delivery-1").unwrap(),
        }
    }

    fn delivery() -> EventDelivery {
        EventDelivery {
            delivery_id: DeliveryId::try_new("delivery-1").unwrap(),
            event_id: EventId::try_new("event-1").unwrap(),
            tenant_id: TenantId::try_new("tenant-a").unwrap(),
            source_module_id: ModuleId::try_new("crm.sales").unwrap(),
            consumer_module_id: ModuleId::try_new("crm.link").unwrap(),
            source_actor_id: ActorId::try_new("actor-a").unwrap(),
            event_type: EventType::try_new("sales.changed").unwrap(),
            event_version: EventVersion::try_new("1.0.0").unwrap(),
            aggregate: RecordRef {
                record_type: RecordType::try_new("sales.deal").unwrap(),
                record_id: RecordId::try_new("deal-1").unwrap(),
            },
            aggregate_version: 2,
            occurred_at_unix_nanos: 100,
            correlation_id: CorrelationId::try_new("correlation-1").unwrap(),
            trace_id: TraceId::try_new("trace-1").unwrap(),
            payload: TypedPayload {
                owner: ModuleId::try_new("crm.sales").unwrap(),
                schema_id: SchemaId::try_new("crm.sales.v1.Changed").unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                descriptor_hash: [1; 32],
                data_class: DataClass::Confidential,
                encoding: PayloadEncoding::Protobuf,
                maximum_size_bytes: 1024,
                retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
                bytes: vec![1],
            },
        }
    }
}
