use crate::ports::{
    CapabilityClient, CapabilityInvocation, CapabilityOutcome, DomainEvent, EventPublisher,
    ModuleStateEntry, ModuleStateStore, ObservabilityContext, PortFuture, PortResult,
    PublishedEvent, PutModuleStateRequest, TelemetryEvent,
};
use crate::types::{ErrorCategory, ModuleExecutionContext, SdkError, StateKey};
use std::collections::{BTreeMap, VecDeque};
use std::sync::Mutex;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};

#[derive(Debug)]
pub struct FixedClock {
    now_unix_nanos: AtomicI64,
}

impl FixedClock {
    pub const fn new(now_unix_nanos: i64) -> Self {
        Self {
            now_unix_nanos: AtomicI64::new(now_unix_nanos),
        }
    }

    pub fn set(&self, now_unix_nanos: i64) {
        self.now_unix_nanos.store(now_unix_nanos, Ordering::SeqCst);
    }

    pub fn advance(&self, delta_nanos: i64) -> i64 {
        self.now_unix_nanos.fetch_add(delta_nanos, Ordering::SeqCst) + delta_nanos
    }
}

impl crate::ports::Clock for FixedClock {
    fn now_unix_nanos(&self) -> i64 {
        self.now_unix_nanos.load(Ordering::SeqCst)
    }
}

#[derive(Debug, Default)]
pub struct DeterministicRandom {
    bytes: Mutex<VecDeque<u8>>,
}

impl DeterministicRandom {
    pub fn from_bytes(bytes: impl IntoIterator<Item = u8>) -> Self {
        Self {
            bytes: Mutex::new(bytes.into_iter().collect()),
        }
    }

    pub fn remaining(&self) -> usize {
        self.bytes
            .lock()
            .expect("deterministic random mutex poisoned")
            .len()
    }
}

impl crate::ports::RandomSource for DeterministicRandom {
    fn fill_bytes(&self, destination: &mut [u8]) -> PortResult<()> {
        let mut source = self
            .bytes
            .lock()
            .expect("deterministic random mutex poisoned");
        if source.len() < destination.len() {
            return Err(SdkError::new(
                "SDK_TEST_RANDOM_EXHAUSTED",
                ErrorCategory::Dependency,
                false,
                "The deterministic random source is exhausted.",
            ));
        }
        for byte in destination {
            *byte = source.pop_front().expect("length checked above");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedCapabilityCall {
    pub context: ModuleExecutionContext,
    pub request: CapabilityInvocation,
}

#[derive(Debug, Default)]
pub struct RecordingCapabilityClient {
    calls: Mutex<Vec<RecordedCapabilityCall>>,
    responses: Mutex<VecDeque<PortResult<CapabilityOutcome>>>,
}

impl RecordingCapabilityClient {
    pub fn push_response(&self, response: PortResult<CapabilityOutcome>) {
        self.responses
            .lock()
            .expect("capability response mutex poisoned")
            .push_back(response);
    }

    pub fn calls(&self) -> Vec<RecordedCapabilityCall> {
        self.calls
            .lock()
            .expect("capability call mutex poisoned")
            .clone()
    }
}

impl CapabilityClient for RecordingCapabilityClient {
    fn invoke<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        request: CapabilityInvocation,
    ) -> PortFuture<'a, PortResult<CapabilityOutcome>> {
        Box::pin(async move {
            context.validate()?;
            request.input.validate()?;
            self.calls
                .lock()
                .expect("capability call mutex poisoned")
                .push(RecordedCapabilityCall {
                    context: context.clone(),
                    request,
                });
            self.responses
                .lock()
                .expect("capability response mutex poisoned")
                .pop_front()
                .unwrap_or_else(|| {
                    Ok(CapabilityOutcome {
                        output: None,
                        affected_resources: Vec::new(),
                    })
                })
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedEvent {
    pub context: ModuleExecutionContext,
    pub event: DomainEvent,
}

#[derive(Debug, Default)]
pub struct RecordingEventPublisher {
    events: Mutex<Vec<RecordedEvent>>,
    next_event_sequence: AtomicU64,
}

impl RecordingEventPublisher {
    pub fn events(&self) -> Vec<RecordedEvent> {
        self.events.lock().expect("event mutex poisoned").clone()
    }
}

impl EventPublisher for RecordingEventPublisher {
    fn publish<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        event: DomainEvent,
    ) -> PortFuture<'a, PortResult<PublishedEvent>> {
        Box::pin(async move {
            context.validate()?;
            event.payload.validate()?;
            if event.deduplication_key.is_empty() {
                return Err(SdkError::invalid_argument(
                    "event.deduplication_key",
                    "deduplication key must not be empty",
                ));
            }
            self.events
                .lock()
                .expect("event mutex poisoned")
                .push(RecordedEvent {
                    context: context.clone(),
                    event,
                });
            let sequence = self.next_event_sequence.fetch_add(1, Ordering::SeqCst) + 1;
            Ok(PublishedEvent {
                event_id: format!("test-event-{sequence}"),
                aggregate_version: sequence as i64,
            })
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct StateStorageKey {
    tenant_id: String,
    module_id: String,
    key: String,
}

#[derive(Debug, Default)]
pub struct InMemoryModuleStateStore {
    entries: Mutex<BTreeMap<StateStorageKey, ModuleStateEntry>>,
}

impl InMemoryModuleStateStore {
    fn storage_key(context: &ModuleExecutionContext, key: &StateKey) -> StateStorageKey {
        StateStorageKey {
            tenant_id: context.execution.tenant_id.to_string(),
            module_id: context.module_id.to_string(),
            key: key.to_string(),
        }
    }

    pub fn entry_count(&self) -> usize {
        self.entries
            .lock()
            .expect("module state mutex poisoned")
            .len()
    }
}

impl ModuleStateStore for InMemoryModuleStateStore {
    fn get<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        key: StateKey,
    ) -> PortFuture<'a, PortResult<Option<ModuleStateEntry>>> {
        Box::pin(async move {
            context.validate()?;
            let storage_key = Self::storage_key(context, &key);
            Ok(self
                .entries
                .lock()
                .expect("module state mutex poisoned")
                .get(&storage_key)
                .cloned())
        })
    }

    fn put<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        request: PutModuleStateRequest,
    ) -> PortFuture<'a, PortResult<ModuleStateEntry>> {
        Box::pin(async move {
            context.validate()?;
            request.value.validate()?;
            if request.value.owner != context.module_id {
                return Err(SdkError::new(
                    "SDK_STATE_OWNER_MISMATCH",
                    ErrorCategory::Authorization,
                    false,
                    "A module may write only payloads it owns.",
                ));
            }

            let storage_key = Self::storage_key(context, &request.key);
            let mut entries = self.entries.lock().expect("module state mutex poisoned");
            let current = entries.get(&storage_key);
            match (current, request.expected_version) {
                (None, Some(_)) => {
                    return Err(version_conflict("module state entry does not exist"));
                }
                (Some(_), None) => {
                    return Err(version_conflict(
                        "expected_version is required when replacing module state",
                    ));
                }
                (Some(entry), Some(expected)) if entry.version != expected => {
                    return Err(version_conflict(format!(
                        "expected version {expected}, found {}",
                        entry.version
                    )));
                }
                _ => {}
            }

            let version = current.map_or(1, |entry| entry.version + 1);
            let entry = ModuleStateEntry {
                key: request.key,
                version,
                value: request.value,
            };
            entries.insert(storage_key, entry.clone());
            Ok(entry)
        })
    }

    fn delete<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        key: StateKey,
        expected_version: Option<i64>,
    ) -> PortFuture<'a, PortResult<()>> {
        Box::pin(async move {
            context.validate()?;
            let storage_key = Self::storage_key(context, &key);
            let mut entries = self.entries.lock().expect("module state mutex poisoned");
            let Some(current) = entries.get(&storage_key) else {
                return Ok(());
            };
            if expected_version.is_some_and(|expected| expected != current.version) {
                return Err(version_conflict(format!(
                    "expected version {}, found {}",
                    expected_version.expect("checked as Some"),
                    current.version
                )));
            }
            entries.remove(&storage_key);
            Ok(())
        })
    }
}

fn version_conflict(message: impl Into<String>) -> SdkError {
    SdkError::new(
        "SDK_VERSION_CONFLICT",
        ErrorCategory::Conflict,
        false,
        message,
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedTelemetryEvent {
    pub context: ModuleExecutionContext,
    pub event: TelemetryEvent,
}

#[derive(Debug, Default)]
pub struct RecordingObservabilityContext {
    events: Mutex<Vec<RecordedTelemetryEvent>>,
}

impl RecordingObservabilityContext {
    pub fn events(&self) -> Vec<RecordedTelemetryEvent> {
        self.events
            .lock()
            .expect("telemetry mutex poisoned")
            .clone()
    }
}

impl ObservabilityContext for RecordingObservabilityContext {
    fn emit<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        event: TelemetryEvent,
    ) -> PortFuture<'a, PortResult<()>> {
        Box::pin(async move {
            context.validate()?;
            self.events
                .lock()
                .expect("telemetry mutex poisoned")
                .push(RecordedTelemetryEvent {
                    context: context.clone(),
                    event,
                });
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::{Clock, RandomSource};
    use crate::types::{
        ActorId, CapabilityId, CapabilityVersion, CausationId, CorrelationId, DataClass,
        ExecutionContext, IdempotencyKey, ModuleId, PayloadEncoding, RequestId, RetentionPolicyId,
        SchemaId, SchemaVersion, TenantId, TraceId, TypedPayload,
    };
    use std::future::Future;
    use std::sync::Arc;
    use std::task::{Context, Poll, Wake, Waker};

    struct NoopWake;

    impl Wake for NoopWake {
        fn wake(self: Arc<Self>) {}
    }

    fn run_ready<F: Future>(future: F) -> F::Output {
        let waker = Waker::from(Arc::new(NoopWake));
        let mut context = Context::from_waker(&waker);
        let mut future = Box::pin(future);
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => output,
            Poll::Pending => panic!("test double future unexpectedly returned Pending"),
        }
    }

    fn context(module: &str, tenant: &str) -> ModuleExecutionContext {
        ModuleExecutionContext {
            module_id: ModuleId::try_new(module).unwrap(),
            execution: ExecutionContext {
                tenant_id: TenantId::try_new(tenant).unwrap(),
                actor_id: ActorId::try_new("actor-1").unwrap(),
                request_id: RequestId::try_new("request-1").unwrap(),
                correlation_id: CorrelationId::try_new("correlation-1").unwrap(),
                causation_id: CausationId::try_new("causation-1").unwrap(),
                trace_id: TraceId::try_new("trace-1").unwrap(),
                capability_id: CapabilityId::try_new("sales.test").unwrap(),
                capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                idempotency_key: IdempotencyKey::try_new("idempotency-1").unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                request_started_at_unix_nanos: 1,
            },
        }
    }

    fn payload(owner: &str, value: u8) -> TypedPayload {
        TypedPayload {
            owner: ModuleId::try_new(owner).unwrap(),
            schema_id: SchemaId::try_new("test.state.v1").unwrap(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
            descriptor_hash: [value.max(1); 32],
            data_class: DataClass::Internal,
            encoding: PayloadEncoding::Binary,
            maximum_size_bytes: 1,
            retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
            bytes: vec![value],
        }
    }

    #[test]
    fn deterministic_time_and_randomness_are_controllable() {
        let clock = FixedClock::new(10);
        assert_eq!(clock.advance(5), 15);
        assert_eq!(clock.now_unix_nanos(), 15);

        let random = DeterministicRandom::from_bytes([1, 2, 3]);
        let mut output = [0; 2];
        random.fill_bytes(&mut output).unwrap();
        assert_eq!(output, [1, 2]);
        assert_eq!(random.remaining(), 1);
    }

    #[test]
    fn module_state_is_tenant_and_module_scoped() {
        let store = InMemoryModuleStateStore::default();
        let sales = context("crm.sales", "tenant-a");
        let activities = context("crm.activities", "tenant-a");
        let key = StateKey::try_new("cursor").unwrap();

        let created = run_ready(store.put(
            &sales,
            PutModuleStateRequest {
                key: key.clone(),
                expected_version: None,
                value: payload("crm.sales", 1),
            },
        ))
        .unwrap();
        assert_eq!(created.version, 1);
        assert!(
            run_ready(store.get(&activities, key.clone()))
                .unwrap()
                .is_none()
        );
        assert!(run_ready(store.get(&sales, key)).unwrap().is_some());
    }

    #[test]
    fn module_state_requires_optimistic_version() {
        let store = InMemoryModuleStateStore::default();
        let sales = context("crm.sales", "tenant-a");
        let key = StateKey::try_new("cursor").unwrap();
        run_ready(store.put(
            &sales,
            PutModuleStateRequest {
                key: key.clone(),
                expected_version: None,
                value: payload("crm.sales", 1),
            },
        ))
        .unwrap();

        let error = run_ready(store.put(
            &sales,
            PutModuleStateRequest {
                key,
                expected_version: Some(99),
                value: payload("crm.sales", 2),
            },
        ))
        .unwrap_err();
        assert_eq!(error.category, ErrorCategory::Conflict);
    }
}
