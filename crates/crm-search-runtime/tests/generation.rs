use crm_core_events::{
    EventHistoryPage, EventHistoryRequest, ProjectionApplyResult, ProjectionCheckpoint,
    ProjectionEventApplication, ProjectionFailure, ProjectionStore, ProjectionStoreFuture,
};
use crm_module_sdk::{EventType, ModuleId, SdkError, TenantId};
use crm_projection_runtime::{
    ProjectionDefinition, ProjectionHandler, ProjectionId, ProjectionRegistry, ProjectionRunner,
};
use crm_search_runtime::{
    SearchGenerationAction, SearchGenerationStatus, SearchGenerationStore, SearchIndexGeneration,
    SearchIndexId, SearchReindexCoordinator,
};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct EmptyProjectionStore {
    resets: Mutex<u32>,
}

impl ProjectionStore for EmptyProjectionStore {
    fn projection_checkpoint(
        &self,
        _tenant_id: TenantId,
        _projection_id: String,
    ) -> ProjectionStoreFuture<'_, Option<ProjectionCheckpoint>> {
        Box::pin(async { Ok(None) })
    }

    fn list_event_history(
        &self,
        _request: EventHistoryRequest,
    ) -> ProjectionStoreFuture<'_, EventHistoryPage> {
        Box::pin(async {
            Ok(EventHistoryPage {
                deliveries: Vec::new(),
                next_cursor: None,
            })
        })
    }

    fn apply_projection_event(
        &self,
        _application: ProjectionEventApplication,
    ) -> ProjectionStoreFuture<'_, ProjectionApplyResult> {
        Box::pin(async {
            Ok(ProjectionApplyResult {
                replayed: false,
                documents_written: 0,
            })
        })
    }

    fn mark_projection_failed(
        &self,
        _failure: ProjectionFailure,
    ) -> ProjectionStoreFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }

    fn reset_projection(
        &self,
        _tenant_id: TenantId,
        _projection_id: String,
    ) -> ProjectionStoreFuture<'_, ()> {
        Box::pin(async move {
            *self.resets.lock().unwrap() += 1;
            Ok(())
        })
    }
}

#[derive(Default)]
struct GenerationState {
    active: Option<SearchIndexGeneration>,
    registered: Vec<SearchIndexGeneration>,
    activations: Vec<String>,
}

#[derive(Default)]
struct TestGenerationStore {
    state: Mutex<GenerationState>,
}

impl SearchGenerationStore for TestGenerationStore {
    fn active_generation<'a>(
        &'a self,
        _tenant_id: TenantId,
        _index_id: SearchIndexId,
    ) -> crm_module_sdk::PortFuture<'a, Result<Option<SearchIndexGeneration>, SdkError>> {
        Box::pin(async move { Ok(self.state.lock().unwrap().active.clone()) })
    }

    fn register_building_generation<'a>(
        &'a self,
        generation: SearchIndexGeneration,
    ) -> crm_module_sdk::PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            self.state.lock().unwrap().registered.push(generation);
            Ok(())
        })
    }

    fn activate_generation<'a>(
        &'a self,
        tenant_id: TenantId,
        index_id: SearchIndexId,
        generation_id: String,
    ) -> crm_module_sdk::PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            let mut state = self.state.lock().unwrap();
            state.activations.push(generation_id.clone());
            state.active = Some(SearchIndexGeneration {
                tenant_id,
                index_id,
                generation_id,
                projection_id: "search.global.g1".to_owned(),
                schema_version: "1".to_owned(),
                status: SearchGenerationStatus::Active,
            });
            Ok(())
        })
    }
}

struct EmptyHandler;

impl ProjectionHandler for EmptyHandler {
    fn project(
        &self,
        _delivery: &crm_module_sdk::EventDelivery,
    ) -> Result<Vec<crm_core_events::ProjectionDocumentWrite>, SdkError> {
        Ok(Vec::new())
    }
}

#[tokio::test(flavor = "current_thread")]
async fn inactive_generation_rebuilds_before_activation() {
    let projection_store = Arc::new(EmptyProjectionStore::default());
    let generations = Arc::new(TestGenerationStore::default());
    let coordinator = coordinator(projection_store.clone(), generations.clone());

    let action = coordinator
        .ensure_ready(TenantId::try_new("tenant-a").unwrap(), 100)
        .await
        .unwrap();

    assert_eq!(
        action,
        SearchGenerationAction::RebuiltAndActivated { applied_events: 0 }
    );
    assert_eq!(*projection_store.resets.lock().unwrap(), 1);
    let state = generations.state.lock().unwrap();
    assert_eq!(state.registered.len(), 1);
    assert_eq!(state.registered[0].status, SearchGenerationStatus::Building);
    assert_eq!(state.activations, vec!["g1"]);
}

#[tokio::test(flavor = "current_thread")]
async fn matching_active_generation_catches_up_without_reset_or_reactivation() {
    let projection_store = Arc::new(EmptyProjectionStore::default());
    let generations = Arc::new(TestGenerationStore::default());
    generations.state.lock().unwrap().active = Some(SearchIndexGeneration {
        tenant_id: TenantId::try_new("tenant-a").unwrap(),
        index_id: SearchIndexId::try_new("crm.global-search").unwrap(),
        generation_id: "g1".to_owned(),
        projection_id: "search.global.g1".to_owned(),
        schema_version: "1".to_owned(),
        status: SearchGenerationStatus::Active,
    });
    let coordinator = coordinator(projection_store.clone(), generations.clone());

    let action = coordinator
        .ensure_ready(TenantId::try_new("tenant-a").unwrap(), 100)
        .await
        .unwrap();

    assert!(matches!(action, SearchGenerationAction::CaughtUp(_)));
    assert_eq!(*projection_store.resets.lock().unwrap(), 0);
    let state = generations.state.lock().unwrap();
    assert!(state.registered.is_empty());
    assert!(state.activations.is_empty());
}

fn coordinator(
    projection_store: Arc<EmptyProjectionStore>,
    generations: Arc<TestGenerationStore>,
) -> SearchReindexCoordinator {
    let definition = ProjectionDefinition::new(
        ProjectionId::try_new("search.global.g1").unwrap(),
        ModuleId::try_new("crm.search-indexer").unwrap(),
        vec![EventType::try_new("test.event").unwrap()],
        Arc::new(EmptyHandler),
    )
    .unwrap();
    let runner = ProjectionRunner::new(
        projection_store,
        ProjectionRegistry::new(vec![definition]).unwrap(),
    );
    SearchReindexCoordinator::new(
        runner,
        generations,
        SearchIndexId::try_new("crm.global-search").unwrap(),
        "g1",
        "search.global.g1",
        "1",
    )
    .unwrap()
}
