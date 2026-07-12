#![forbid(unsafe_code)]

//! Generic orchestration for rebuildable event-history projections.
//!
//! This crate owns projection mechanics only: registration, checkpoint-based
//! history paging, deterministic handler invocation, poison/failure marking and
//! rebuild orchestration. Concrete business-event decoding remains outside the
//! runtime and durable storage remains behind [`ProjectionStore`].

use crm_core_events::{
    EventHistoryRequest, MAX_EVENT_HISTORY_PAGE_SIZE, ProjectionDocumentWrite,
    ProjectionEventApplication, ProjectionFailure, ProjectionStore,
};
use crm_module_sdk::{ErrorCategory, EventDelivery, EventType, ModuleId, SdkError, TenantId};
use std::borrow::Borrow;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProjectionId(String);

impl ProjectionId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, SdkError> {
        let value = value.into();
        if value.is_empty() || value.len() > 180 || value.chars().any(char::is_control) {
            return Err(configuration_error(
                "PROJECTION_ID_INVALID",
                "The projection identifier is invalid.",
                value,
            ));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for ProjectionId {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for ProjectionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Pure deterministic mapping from one immutable event delivery to projection writes.
pub trait ProjectionHandler: Send + Sync {
    fn project(&self, delivery: &EventDelivery) -> Result<Vec<ProjectionDocumentWrite>, SdkError>;
}

#[derive(Clone)]
pub struct ProjectionDefinition {
    projection_id: ProjectionId,
    consumer_module_id: ModuleId,
    event_types: Vec<EventType>,
    handler: Arc<dyn ProjectionHandler>,
}

impl fmt::Debug for ProjectionDefinition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProjectionDefinition")
            .field("projection_id", &self.projection_id)
            .field("consumer_module_id", &self.consumer_module_id)
            .field("event_types", &self.event_types)
            .field("handler", &"dyn ProjectionHandler")
            .finish()
    }
}

impl ProjectionDefinition {
    pub fn new(
        projection_id: ProjectionId,
        consumer_module_id: ModuleId,
        event_types: Vec<EventType>,
        handler: Arc<dyn ProjectionHandler>,
    ) -> Result<Self, SdkError> {
        if event_types.is_empty() {
            return Err(configuration_error(
                "PROJECTION_EVENT_TYPES_EMPTY",
                "The projection subscription is invalid.",
                projection_id.to_string(),
            ));
        }
        let unique = event_types
            .iter()
            .map(|event_type| event_type.as_str())
            .collect::<BTreeSet<_>>();
        if unique.len() != event_types.len() {
            return Err(configuration_error(
                "PROJECTION_EVENT_TYPES_DUPLICATE",
                "The projection subscription is invalid.",
                projection_id.to_string(),
            ));
        }
        Ok(Self {
            projection_id,
            consumer_module_id,
            event_types,
            handler,
        })
    }

    pub fn projection_id(&self) -> &ProjectionId {
        &self.projection_id
    }

    pub fn consumer_module_id(&self) -> &ModuleId {
        &self.consumer_module_id
    }

    pub fn event_types(&self) -> &[EventType] {
        &self.event_types
    }
}

#[derive(Debug, Clone, Default)]
pub struct ProjectionRegistry {
    definitions: BTreeMap<ProjectionId, ProjectionDefinition>,
}

impl ProjectionRegistry {
    pub fn new(definitions: Vec<ProjectionDefinition>) -> Result<Self, SdkError> {
        let mut registry = Self::default();
        for definition in definitions {
            let projection_id = definition.projection_id.clone();
            if registry
                .definitions
                .insert(projection_id.clone(), definition)
                .is_some()
            {
                return Err(configuration_error(
                    "PROJECTION_ID_DUPLICATE",
                    "The projection registry configuration is invalid.",
                    projection_id.to_string(),
                ));
            }
        }
        Ok(registry)
    }

    pub fn get(&self, projection_id: &str) -> Result<&ProjectionDefinition, SdkError> {
        self.definitions.get(projection_id).ok_or_else(|| {
            configuration_error(
                "PROJECTION_NOT_REGISTERED",
                "The requested projection is not registered.",
                projection_id,
            )
        })
    }

    pub fn len(&self) -> usize {
        self.definitions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }

    pub fn projection_ids(&self) -> impl Iterator<Item = &ProjectionId> {
        self.definitions.keys()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionBatchResult {
    pub events_seen: u32,
    pub events_applied: u32,
    pub replayed_events: u32,
    pub has_more: bool,
}

#[derive(Debug)]
struct ProjectionApplicationFailure {
    delivery: EventDelivery,
    error: SdkError,
}

#[derive(Clone)]
pub struct ProjectionRunner {
    store: Arc<dyn ProjectionStore>,
    registry: ProjectionRegistry,
}

impl fmt::Debug for ProjectionRunner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProjectionRunner")
            .field("store", &"dyn ProjectionStore")
            .field("registry", &self.registry)
            .finish()
    }
}

impl ProjectionRunner {
    pub fn new(store: Arc<dyn ProjectionStore>, registry: ProjectionRegistry) -> Self {
        Self { store, registry }
    }

    pub fn registry(&self) -> &ProjectionRegistry {
        &self.registry
    }

    pub async fn run_batch(
        &self,
        tenant_id: TenantId,
        projection_id: &str,
        page_size: u32,
    ) -> Result<ProjectionBatchResult, SdkError> {
        validate_page_size(page_size)?;
        let definition = self.registry.get(projection_id)?;
        let checkpoint = self
            .store
            .projection_checkpoint(tenant_id.clone(), projection_id.to_owned())
            .await?;
        let page = self
            .store
            .list_event_history(EventHistoryRequest {
                tenant_id: tenant_id.clone(),
                consumer_module_id: definition.consumer_module_id.clone(),
                event_types: definition.event_types.clone(),
                after: checkpoint.map(|checkpoint| checkpoint.cursor),
                page_size,
            })
            .await?;
        let has_more = page.next_cursor.is_some();
        let events_seen = u32::try_from(page.deliveries.len()).unwrap_or(u32::MAX);
        let mut events_applied = 0_u32;
        let mut replayed_events = 0_u32;

        for delivery in page.deliveries {
            let application = match projection_application(definition, &tenant_id, delivery) {
                Ok(application) => application,
                Err(failure) => {
                    let ProjectionApplicationFailure { delivery, error } = *failure;
                    if delivery.tenant_id == tenant_id {
                        self.mark_failed(definition, &delivery, &error).await?;
                    }
                    return Err(error);
                }
            };
            let result = self.store.apply_projection_event(application).await?;
            if result.replayed {
                replayed_events = replayed_events.saturating_add(1);
            } else {
                events_applied = events_applied.saturating_add(1);
            }
        }

        Ok(ProjectionBatchResult {
            events_seen,
            events_applied,
            replayed_events,
            has_more,
        })
    }

    pub async fn rebuild(
        &self,
        tenant_id: TenantId,
        projection_id: &str,
        page_size: u32,
    ) -> Result<u64, SdkError> {
        validate_page_size(page_size)?;
        self.registry.get(projection_id)?;
        self.store
            .reset_projection(tenant_id.clone(), projection_id.to_owned())
            .await?;
        let mut applied = 0_u64;
        loop {
            let result = self
                .run_batch(tenant_id.clone(), projection_id, page_size)
                .await?;
            applied = applied.saturating_add(u64::from(result.events_applied));
            if !result.has_more {
                return Ok(applied);
            }
        }
    }

    async fn mark_failed(
        &self,
        definition: &ProjectionDefinition,
        delivery: &EventDelivery,
        error: &SdkError,
    ) -> Result<(), SdkError> {
        let failure = ProjectionFailure {
            tenant_id: delivery.tenant_id.clone(),
            projection_id: definition.projection_id.to_string(),
            event_id: delivery.event_id.clone(),
            occurred_at_unix_nanos: delivery.occurred_at_unix_nanos,
            failure_code: error.code.clone(),
        };
        failure.validate().map_err(|message| {
            configuration_error(
                "PROJECTION_FAILURE_RECORD_INVALID",
                "The projection failure record is invalid.",
                message,
            )
        })?;
        self.store.mark_projection_failed(failure).await
    }
}

fn projection_application(
    definition: &ProjectionDefinition,
    tenant_id: &TenantId,
    delivery: EventDelivery,
) -> Result<ProjectionEventApplication, Box<ProjectionApplicationFailure>> {
    if let Err(error) = validate_delivery_binding(definition, tenant_id, &delivery) {
        return Err(Box::new(ProjectionApplicationFailure { delivery, error }));
    }
    let writes = match definition.handler.project(&delivery) {
        Ok(writes) => writes,
        Err(error) => {
            return Err(Box::new(ProjectionApplicationFailure { delivery, error }));
        }
    };
    let application = ProjectionEventApplication {
        projection_id: definition.projection_id.to_string(),
        delivery,
        writes,
    };
    if let Err(message) = application.validate() {
        let error = SdkError::new(
            "PROJECTION_HANDLER_OUTPUT_INVALID",
            ErrorCategory::Internal,
            false,
            "The projection handler produced invalid output.",
        )
        .with_internal_reference(message);
        return Err(Box::new(ProjectionApplicationFailure {
            delivery: application.delivery,
            error,
        }));
    }
    Ok(application)
}

fn validate_delivery_binding(
    definition: &ProjectionDefinition,
    tenant_id: &TenantId,
    delivery: &EventDelivery,
) -> Result<(), SdkError> {
    delivery.validate()?;
    if &delivery.tenant_id != tenant_id {
        return Err(runtime_error(
            "PROJECTION_DELIVERY_TENANT_MISMATCH",
            "The projection delivery binding is invalid.",
            delivery.event_id.as_str(),
        ));
    }
    if delivery.consumer_module_id != definition.consumer_module_id {
        return Err(runtime_error(
            "PROJECTION_DELIVERY_CONSUMER_MISMATCH",
            "The projection delivery binding is invalid.",
            delivery.event_id.as_str(),
        ));
    }
    if !definition.event_types.contains(&delivery.event_type) {
        return Err(runtime_error(
            "PROJECTION_EVENT_TYPE_NOT_SUBSCRIBED",
            "The projection delivery binding is invalid.",
            delivery.event_type.as_str(),
        ));
    }
    Ok(())
}

fn validate_page_size(page_size: u32) -> Result<(), SdkError> {
    if page_size > MAX_EVENT_HISTORY_PAGE_SIZE {
        return Err(SdkError::new(
            "PROJECTION_PAGE_SIZE_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "The projection page size is invalid.",
        ));
    }
    Ok(())
}

fn configuration_error(
    code: &'static str,
    safe_message: &'static str,
    internal: impl Into<String>,
) -> SdkError {
    SdkError::new(code, ErrorCategory::Internal, false, safe_message)
        .with_internal_reference(internal)
}

fn runtime_error(
    code: &'static str,
    safe_message: &'static str,
    internal: impl Into<String>,
) -> SdkError {
    SdkError::new(code, ErrorCategory::Conflict, false, safe_message)
        .with_internal_reference(internal)
}

/// Architecture marker for `crm-projection-runtime`.
pub const CRATE_NAME: &str = "crm-projection-runtime";

#[cfg(test)]
mod tests {
    use super::*;
    use crm_core_events::{
        EventHistoryCursor, EventHistoryPage, ProjectionApplyResult, ProjectionCheckpoint,
        ProjectionStoreFuture,
    };
    use crm_module_sdk::{
        ActorId, CorrelationId, DataClass, DeliveryId, EventId, EventVersion, PayloadEncoding,
        RecordId, RecordRef, RecordType, RetentionPolicyId, SchemaId, SchemaVersion, TraceId,
        TypedPayload,
    };
    use serde_json::json;
    use std::sync::Mutex;

    #[derive(Default)]
    struct TestState {
        delivery: Option<EventDelivery>,
        checkpoint: Option<ProjectionCheckpoint>,
        applied: u32,
        failures: Vec<ProjectionFailure>,
        resets: u32,
    }

    #[derive(Default)]
    struct TestStore {
        state: Mutex<TestState>,
    }

    impl TestStore {
        fn with_delivery(delivery: EventDelivery) -> Self {
            Self {
                state: Mutex::new(TestState {
                    delivery: Some(delivery),
                    ..TestState::default()
                }),
            }
        }
    }

    impl ProjectionStore for TestStore {
        fn projection_checkpoint(
            &self,
            _tenant_id: TenantId,
            _projection_id: String,
        ) -> ProjectionStoreFuture<'_, Option<ProjectionCheckpoint>> {
            Box::pin(async move {
                let state = self.state.lock().expect("test store lock");
                if let Some(failure) = state.failures.last() {
                    return Err(SdkError::new(
                        "PROJECTION_CHECKPOINT_FAILED",
                        ErrorCategory::Conflict,
                        false,
                        "The projection checkpoint is failed.",
                    )
                    .with_internal_reference(failure.failure_code.clone()));
                }
                Ok(state.checkpoint.clone())
            })
        }

        fn list_event_history(
            &self,
            request: EventHistoryRequest,
        ) -> ProjectionStoreFuture<'_, EventHistoryPage> {
            Box::pin(async move {
                let state = self.state.lock().expect("test store lock");
                let deliveries = match (&state.delivery, &request.after) {
                    (Some(delivery), Some(cursor)) if cursor.event_id == delivery.event_id => {
                        Vec::new()
                    }
                    (Some(delivery), _) => vec![delivery.clone()],
                    (None, _) => Vec::new(),
                };
                Ok(EventHistoryPage {
                    deliveries,
                    next_cursor: None,
                })
            })
        }

        fn apply_projection_event(
            &self,
            application: ProjectionEventApplication,
        ) -> ProjectionStoreFuture<'_, ProjectionApplyResult> {
            Box::pin(async move {
                let mut state = self.state.lock().expect("test store lock");
                state.applied = state.applied.saturating_add(1);
                state.checkpoint = Some(ProjectionCheckpoint {
                    tenant_id: application.delivery.tenant_id.clone(),
                    projection_id: application.projection_id,
                    cursor: EventHistoryCursor {
                        occurred_at_unix_nanos: application.delivery.occurred_at_unix_nanos,
                        event_id: application.delivery.event_id,
                    },
                    applied_event_count: u64::from(state.applied),
                });
                Ok(ProjectionApplyResult {
                    replayed: false,
                    documents_written: u32::try_from(application.writes.len()).unwrap_or(u32::MAX),
                })
            })
        }

        fn mark_projection_failed(
            &self,
            failure: ProjectionFailure,
        ) -> ProjectionStoreFuture<'_, ()> {
            Box::pin(async move {
                self.state
                    .lock()
                    .expect("test store lock")
                    .failures
                    .push(failure);
                Ok(())
            })
        }

        fn reset_projection(
            &self,
            _tenant_id: TenantId,
            _projection_id: String,
        ) -> ProjectionStoreFuture<'_, ()> {
            Box::pin(async move {
                let mut state = self.state.lock().expect("test store lock");
                state.checkpoint = None;
                state.failures.clear();
                state.resets = state.resets.saturating_add(1);
                Ok(())
            })
        }
    }

    struct WriteHandler;

    impl ProjectionHandler for WriteHandler {
        fn project(
            &self,
            delivery: &EventDelivery,
        ) -> Result<Vec<ProjectionDocumentWrite>, SdkError> {
            Ok(vec![ProjectionDocumentWrite {
                resource_type: "test.resource".to_owned(),
                resource_id: delivery.aggregate.record_id.as_str().to_owned(),
                source_version: delivery.aggregate_version,
                document: json!({"event_id": delivery.event_id.as_str()}),
            }])
        }
    }

    struct FailingHandler;

    impl ProjectionHandler for FailingHandler {
        fn project(
            &self,
            _delivery: &EventDelivery,
        ) -> Result<Vec<ProjectionDocumentWrite>, SdkError> {
            Err(SdkError::new(
                "TEST_PROJECTION_POISON",
                ErrorCategory::InvalidArgument,
                false,
                "The test projection event is invalid.",
            ))
        }
    }

    #[test]
    fn registry_rejects_duplicate_projection_ids() {
        let definition = definition(Arc::new(WriteHandler));
        let error = ProjectionRegistry::new(vec![definition.clone(), definition])
            .expect_err("duplicate projection ids must fail");
        assert_eq!(error.code, "PROJECTION_ID_DUPLICATE");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn runner_resumes_from_checkpoint_and_rebuild_resets_state() {
        let store = Arc::new(TestStore::with_delivery(delivery_for_tenant("tenant-a")));
        let runner = ProjectionRunner::new(
            store.clone(),
            ProjectionRegistry::new(vec![definition(Arc::new(WriteHandler))]).unwrap(),
        );

        let first = runner
            .run_batch(tenant(), "test.projection.v1", 10)
            .await
            .expect("first projection batch");
        assert_eq!(first.events_seen, 1);
        assert_eq!(first.events_applied, 1);

        let idle = runner
            .run_batch(tenant(), "test.projection.v1", 10)
            .await
            .expect("checkpoint resume");
        assert_eq!(idle.events_seen, 0);

        let rebuilt = runner
            .rebuild(tenant(), "test.projection.v1", 10)
            .await
            .expect("projection rebuild");
        assert_eq!(rebuilt, 1);
        let state = store.state.lock().unwrap();
        assert_eq!(state.resets, 1);
        assert_eq!(state.applied, 2);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn deterministic_handler_failure_poison_marks_without_advancing_checkpoint() {
        let store = Arc::new(TestStore::with_delivery(delivery_for_tenant("tenant-a")));
        let runner = ProjectionRunner::new(
            store.clone(),
            ProjectionRegistry::new(vec![definition(Arc::new(FailingHandler))]).unwrap(),
        );

        let error = runner
            .run_batch(tenant(), "test.projection.v1", 10)
            .await
            .expect_err("poison event must fail the projection");
        assert_eq!(error.code, "TEST_PROJECTION_POISON");

        let state = store.state.lock().unwrap();
        assert_eq!(state.applied, 0);
        assert!(state.checkpoint.is_none());
        assert_eq!(state.failures.len(), 1);
        assert_eq!(state.failures[0].event_id.as_str(), "event-1");
        assert_eq!(state.failures[0].failure_code, "TEST_PROJECTION_POISON");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn cross_tenant_delivery_is_rejected_without_poisoning_another_tenant() {
        let store = Arc::new(TestStore::with_delivery(delivery_for_tenant("tenant-b")));
        let runner = ProjectionRunner::new(
            store.clone(),
            ProjectionRegistry::new(vec![definition(Arc::new(WriteHandler))]).unwrap(),
        );

        let error = runner
            .run_batch(tenant(), "test.projection.v1", 10)
            .await
            .expect_err("cross-tenant delivery must be rejected");
        assert_eq!(error.code, "PROJECTION_DELIVERY_TENANT_MISMATCH");

        let state = store.state.lock().unwrap();
        assert_eq!(state.applied, 0);
        assert!(state.checkpoint.is_none());
        assert!(state.failures.is_empty());
    }

    fn definition(handler: Arc<dyn ProjectionHandler>) -> ProjectionDefinition {
        ProjectionDefinition::new(
            ProjectionId::try_new("test.projection.v1").unwrap(),
            ModuleId::try_new("crm.test-projection").unwrap(),
            vec![EventType::try_new("test.event").unwrap()],
            handler,
        )
        .unwrap()
    }

    fn tenant() -> TenantId {
        TenantId::try_new("tenant-a").unwrap()
    }

    fn delivery_for_tenant(tenant_id: &str) -> EventDelivery {
        EventDelivery {
            delivery_id: DeliveryId::try_new("delivery-1").unwrap(),
            event_id: EventId::try_new("event-1").unwrap(),
            tenant_id: TenantId::try_new(tenant_id).unwrap(),
            source_module_id: ModuleId::try_new("crm.test-source").unwrap(),
            consumer_module_id: ModuleId::try_new("crm.test-projection").unwrap(),
            source_actor_id: ActorId::try_new("actor-a").unwrap(),
            event_type: EventType::try_new("test.event").unwrap(),
            event_version: EventVersion::try_new("1.0.0").unwrap(),
            aggregate: RecordRef {
                record_type: RecordType::try_new("test.record").unwrap(),
                record_id: RecordId::try_new("record-1").unwrap(),
            },
            aggregate_version: 1,
            occurred_at_unix_nanos: 100,
            correlation_id: CorrelationId::try_new("correlation-1").unwrap(),
            trace_id: TraceId::try_new("trace-1").unwrap(),
            payload: TypedPayload {
                owner: ModuleId::try_new("crm.test-source").unwrap(),
                schema_id: SchemaId::try_new("test.Event").unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                descriptor_hash: [1; 32],
                data_class: DataClass::Internal,
                encoding: PayloadEncoding::Json,
                maximum_size_bytes: 2,
                retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
                bytes: vec![b'{', b'}'],
            },
        }
    }
}
