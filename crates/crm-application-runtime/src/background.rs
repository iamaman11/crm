use crm_application_composition::{
    ActivationGatedBackgroundWorker, BackgroundWorkerPhase, BackgroundWorkerRegistry,
    BackgroundWorkerRegistryBuilder, ModuleActivationPort, TenantBackgroundWorker,
};
use crm_core_data::PostgresDataStore;
use crm_core_events::EventHistoryRequest;
use crm_customer_360_composition::Customer360ProjectionWorker;
use crm_customer_360_query_adapter::MODULE_ID as CUSTOMER_360_MODULE_ID;
use crm_customer_data_operations_capability_adapter::MODULE_ID as CUSTOMER_DATA_OPERATIONS_MODULE_ID;
use crm_customer_data_operations_execution_composition::{
    PartyExportSelectionWorker, PartyImportExecutionWorker,
};
use crm_customer_enrichment_application_composition::{
    CustomerEnrichmentPartyApplicationWorker, PARTY_DISPLAY_NAME_APPLICATION_WORKER_ID,
};
use crm_customer_enrichment_capability_adapter::MODULE_ID as CUSTOMER_ENRICHMENT_MODULE_ID;
use crm_customer_enrichment_provider_process_composition::{
    CustomerEnrichmentProviderProcessWorker, PROVIDER_PROCESS_WORKER_ID,
};
use crm_global_search_composition::GlobalSearchWorker;
use crm_module_sdk::{ErrorCategory, EventType, ModuleId, PortFuture, SdkError, TenantId};
use crm_sales_activities_capability_composition::{
    DEAL_TIMELINE_PROJECTION_ID, Phase6ProjectionWorker, SalesActivitiesLinkDeliveryOutcome,
    SalesActivitiesLinkEventProcessor, TASK_STATUS_PROJECTION_ID,
};
use crm_sales_activities_link::MODULE_ID as LINK_MODULE_ID;
use crm_search_query_adapter::SEARCH_MODULE_ID;
use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

const SALES_MODULE_ID: &str = "crm.sales";
const ACTIVITIES_MODULE_ID: &str = "crm.activities";
const LINK_SCAN_PAGE_SIZE: u32 = 200;
const PROJECTION_PAGE_SIZE: u32 = 200;
const SEARCH_PAGE_SIZE: u32 = 200;

const IMPORT_EXECUTION_WORKER_ID: &str = "party-import-execution";
const EXPORT_SELECTION_WORKER_ID: &str = "party-export-selection";
const SALES_ACTIVITIES_LINK_WORKER_ID: &str = "sales-activities-link";
const DEAL_TIMELINE_PROJECTION_WORKER_ID: &str = "deal-timeline-projection";
const TASK_STATUS_PROJECTION_WORKER_ID: &str = "task-status-projection";
const CUSTOMER_360_PROJECTION_WORKER_ID: &str = "customer-360-projection";
const GLOBAL_SEARCH_WORKER_ID: &str = "global-search-index";
const CUSTOMER_ENRICHMENT_PROVIDER_PROCESS_PHASE: BackgroundWorkerPhase =
    BackgroundWorkerPhase::new(240);
const CUSTOMER_ENRICHMENT_APPLICATION_PHASE: BackgroundWorkerPhase =
    BackgroundWorkerPhase::new(250);

pub(crate) struct ProductionBackgroundWorkerDependencies {
    pub module_ids: BTreeSet<String>,
    pub activation: Arc<dyn ModuleActivationPort>,
    pub store: PostgresDataStore,
    pub import_execution_worker: Arc<PartyImportExecutionWorker>,
    pub export_selection_worker: Arc<PartyExportSelectionWorker>,
    pub customer_enrichment_provider_process: Arc<CustomerEnrichmentProviderProcessWorker>,
    pub customer_enrichment_application_worker: Arc<CustomerEnrichmentPartyApplicationWorker>,
    pub link_processor: Arc<SalesActivitiesLinkEventProcessor>,
    pub projection_worker: Arc<Phase6ProjectionWorker>,
    pub customer_360_worker: Arc<Customer360ProjectionWorker>,
    pub search_worker: Arc<GlobalSearchWorker>,
}

pub(crate) fn build_production_background_workers(
    dependencies: ProductionBackgroundWorkerDependencies,
) -> Result<BackgroundWorkerRegistry, SdkError> {
    let ProductionBackgroundWorkerDependencies {
        module_ids,
        activation,
        store,
        import_execution_worker,
        export_selection_worker,
        customer_enrichment_provider_process,
        customer_enrichment_application_worker,
        link_processor,
        projection_worker,
        customer_360_worker,
        search_worker,
    } = dependencies;
    let mut builder = BackgroundWorkerRegistryBuilder::new(module_ids);

    add_worker(
        &mut builder,
        activation.clone(),
        BackgroundWorkerPhase::SOURCE_INGESTION,
        CUSTOMER_DATA_OPERATIONS_MODULE_ID,
        IMPORT_EXECUTION_WORKER_ID,
        Arc::new(ImportExecutionBackgroundWorker::new(
            import_execution_worker,
        )),
    )?;
    add_worker(
        &mut builder,
        activation.clone(),
        BackgroundWorkerPhase::new(110),
        CUSTOMER_DATA_OPERATIONS_MODULE_ID,
        EXPORT_SELECTION_WORKER_ID,
        Arc::new(ExportSelectionBackgroundWorker::new(
            export_selection_worker,
        )),
    )?;
    add_worker(
        &mut builder,
        activation.clone(),
        BackgroundWorkerPhase::DOMAIN_LINKING,
        LINK_MODULE_ID,
        SALES_ACTIVITIES_LINK_WORKER_ID,
        Arc::new(SalesActivitiesLinkBackgroundWorker::new(
            store,
            link_processor,
        )),
    )?;
    add_customer_enrichment_workers(
        &mut builder,
        activation.clone(),
        customer_enrichment_provider_process,
        customer_enrichment_application_worker,
    )?;
    add_worker(
        &mut builder,
        activation.clone(),
        BackgroundWorkerPhase::PROJECTION,
        SALES_MODULE_ID,
        DEAL_TIMELINE_PROJECTION_WORKER_ID,
        Arc::new(Phase6ProjectionBackgroundWorker::new(
            projection_worker.clone(),
            DEAL_TIMELINE_PROJECTION_ID,
        )),
    )?;
    add_worker(
        &mut builder,
        activation.clone(),
        BackgroundWorkerPhase::new(310),
        ACTIVITIES_MODULE_ID,
        TASK_STATUS_PROJECTION_WORKER_ID,
        Arc::new(Phase6ProjectionBackgroundWorker::new(
            projection_worker,
            TASK_STATUS_PROJECTION_ID,
        )),
    )?;
    add_worker(
        &mut builder,
        activation.clone(),
        BackgroundWorkerPhase::DERIVED_VIEW,
        CUSTOMER_360_MODULE_ID,
        CUSTOMER_360_PROJECTION_WORKER_ID,
        Arc::new(Customer360BackgroundWorker::new(customer_360_worker)),
    )?;
    add_worker(
        &mut builder,
        activation,
        BackgroundWorkerPhase::SEARCH_INDEX,
        SEARCH_MODULE_ID,
        GLOBAL_SEARCH_WORKER_ID,
        Arc::new(GlobalSearchBackgroundWorker::new(search_worker)),
    )?;

    Ok(builder.build())
}

fn add_customer_enrichment_workers(
    builder: &mut BackgroundWorkerRegistryBuilder,
    activation: Arc<dyn ModuleActivationPort>,
    provider_process: Arc<dyn TenantBackgroundWorker>,
    application_worker: Arc<dyn TenantBackgroundWorker>,
) -> Result<(), SdkError> {
    add_worker(
        builder,
        activation.clone(),
        CUSTOMER_ENRICHMENT_PROVIDER_PROCESS_PHASE,
        CUSTOMER_ENRICHMENT_MODULE_ID,
        PROVIDER_PROCESS_WORKER_ID,
        provider_process,
    )?;
    add_worker(
        builder,
        activation,
        CUSTOMER_ENRICHMENT_APPLICATION_PHASE,
        CUSTOMER_ENRICHMENT_MODULE_ID,
        PARTY_DISPLAY_NAME_APPLICATION_WORKER_ID,
        application_worker,
    )
}

fn add_worker(
    builder: &mut BackgroundWorkerRegistryBuilder,
    activation: Arc<dyn ModuleActivationPort>,
    phase: BackgroundWorkerPhase,
    owner_module_id: &str,
    worker_id: &str,
    worker: Arc<dyn TenantBackgroundWorker>,
) -> Result<(), SdkError> {
    let module_id = ModuleId::try_new(owner_module_id).map_err(configuration_error)?;
    let gated: Arc<dyn TenantBackgroundWorker> = Arc::new(ActivationGatedBackgroundWorker::new(
        activation,
        module_id.clone(),
        worker,
    ));
    builder
        .add_in_phase(phase, module_id, worker_id, gated)
        .map(|_| ())
        .map_err(background_composition_error)
}

#[derive(Clone)]
struct ImportExecutionBackgroundWorker {
    inner: Arc<PartyImportExecutionWorker>,
}

impl ImportExecutionBackgroundWorker {
    fn new(inner: Arc<PartyImportExecutionWorker>) -> Self {
        Self { inner }
    }
}

impl fmt::Debug for ImportExecutionBackgroundWorker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ImportExecutionBackgroundWorker")
            .finish_non_exhaustive()
    }
}

impl TenantBackgroundWorker for ImportExecutionBackgroundWorker {
    fn run_tenant_cycle<'a>(
        &'a self,
        tenant_id: TenantId,
        _now_unix_nanos: i64,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            self.inner
                .run_tenant_cycle(tenant_id)
                .await
                .map(|_| ())
                .map_err(|error| worker_error("IMPORT_EXECUTION", error))
        })
    }
}

#[derive(Clone)]
struct ExportSelectionBackgroundWorker {
    inner: Arc<PartyExportSelectionWorker>,
}

impl ExportSelectionBackgroundWorker {
    fn new(inner: Arc<PartyExportSelectionWorker>) -> Self {
        Self { inner }
    }
}

impl fmt::Debug for ExportSelectionBackgroundWorker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExportSelectionBackgroundWorker")
            .finish_non_exhaustive()
    }
}

impl TenantBackgroundWorker for ExportSelectionBackgroundWorker {
    fn run_tenant_cycle<'a>(
        &'a self,
        tenant_id: TenantId,
        _now_unix_nanos: i64,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            self.inner
                .run_tenant_cycle(tenant_id)
                .await
                .map(|_| ())
                .map_err(|error| worker_error("EXPORT_SELECTION", error))
        })
    }
}

#[derive(Clone)]
struct SalesActivitiesLinkBackgroundWorker {
    store: PostgresDataStore,
    processor: Arc<SalesActivitiesLinkEventProcessor>,
}

impl SalesActivitiesLinkBackgroundWorker {
    fn new(store: PostgresDataStore, processor: Arc<SalesActivitiesLinkEventProcessor>) -> Self {
        Self { store, processor }
    }
}

impl fmt::Debug for SalesActivitiesLinkBackgroundWorker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SalesActivitiesLinkBackgroundWorker")
            .field("store", &self.store)
            .finish_non_exhaustive()
    }
}

impl TenantBackgroundWorker for SalesActivitiesLinkBackgroundWorker {
    fn run_tenant_cycle<'a>(
        &'a self,
        tenant_id: TenantId,
        now_unix_nanos: i64,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            let event_type =
                EventType::try_new("sales.deal.stage_changed").map_err(configuration_error)?;
            let consumer_module_id =
                ModuleId::try_new(LINK_MODULE_ID).map_err(configuration_error)?;
            let mut after = None;
            loop {
                let page = self
                    .store
                    .list_event_history(&EventHistoryRequest {
                        tenant_id: tenant_id.clone(),
                        consumer_module_id: consumer_module_id.clone(),
                        event_types: vec![event_type.clone()],
                        after,
                        page_size: LINK_SCAN_PAGE_SIZE,
                    })
                    .await?;
                for delivery in page.deliveries {
                    let outcome = self
                        .processor
                        .process(tenant_id.clone(), delivery.event_id, now_unix_nanos)
                        .await
                        .map_err(|error| worker_error("SALES_ACTIVITIES_LINK", error))?;
                    if let SalesActivitiesLinkDeliveryOutcome::DeadLettered { error_code } = outcome
                    {
                        return Err(SdkError::new(
                            "SALES_ACTIVITIES_LINK_DEAD_LETTERED",
                            ErrorCategory::Unavailable,
                            true,
                            "A Sales-to-Activities event delivery was dead-lettered.",
                        )
                        .with_internal_reference(error_code));
                    }
                }
                let Some(next) = page.next_cursor else {
                    return Ok(());
                };
                after = Some(next);
            }
        })
    }
}

#[derive(Clone)]
struct Phase6ProjectionBackgroundWorker {
    inner: Arc<Phase6ProjectionWorker>,
    projection_id: &'static str,
}

impl Phase6ProjectionBackgroundWorker {
    fn new(inner: Arc<Phase6ProjectionWorker>, projection_id: &'static str) -> Self {
        Self {
            inner,
            projection_id,
        }
    }
}

impl fmt::Debug for Phase6ProjectionBackgroundWorker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Phase6ProjectionBackgroundWorker")
            .field("projection_id", &self.projection_id)
            .finish_non_exhaustive()
    }
}

impl TenantBackgroundWorker for Phase6ProjectionBackgroundWorker {
    fn run_tenant_cycle<'a>(
        &'a self,
        tenant_id: TenantId,
        _now_unix_nanos: i64,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            loop {
                let result = self
                    .inner
                    .run_batch(tenant_id.clone(), self.projection_id, PROJECTION_PAGE_SIZE)
                    .await
                    .map_err(|error| worker_error("PHASE6_PROJECTION", error))?;
                if !result.has_more {
                    return Ok(());
                }
            }
        })
    }
}

#[derive(Clone)]
struct Customer360BackgroundWorker {
    inner: Arc<Customer360ProjectionWorker>,
}

impl Customer360BackgroundWorker {
    fn new(inner: Arc<Customer360ProjectionWorker>) -> Self {
        Self { inner }
    }
}

impl fmt::Debug for Customer360BackgroundWorker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Customer360BackgroundWorker")
            .finish_non_exhaustive()
    }
}

impl TenantBackgroundWorker for Customer360BackgroundWorker {
    fn run_tenant_cycle<'a>(
        &'a self,
        tenant_id: TenantId,
        _now_unix_nanos: i64,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            loop {
                let result = self
                    .inner
                    .run_batch(tenant_id.clone(), PROJECTION_PAGE_SIZE)
                    .await
                    .map_err(|error| worker_error("CUSTOMER_360_PROJECTION", error))?;
                if !result.has_more {
                    return Ok(());
                }
            }
        })
    }
}

#[derive(Clone)]
struct GlobalSearchBackgroundWorker {
    inner: Arc<GlobalSearchWorker>,
}

impl GlobalSearchBackgroundWorker {
    fn new(inner: Arc<GlobalSearchWorker>) -> Self {
        Self { inner }
    }
}

impl fmt::Debug for GlobalSearchBackgroundWorker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GlobalSearchBackgroundWorker")
            .finish_non_exhaustive()
    }
}

impl TenantBackgroundWorker for GlobalSearchBackgroundWorker {
    fn run_tenant_cycle<'a>(
        &'a self,
        tenant_id: TenantId,
        _now_unix_nanos: i64,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            self.inner
                .ensure_ready(tenant_id, SEARCH_PAGE_SIZE)
                .await
                .map(|_| ())
                .map_err(|error| worker_error("GLOBAL_SEARCH", error))
        })
    }
}

fn worker_error(prefix: &str, error: impl fmt::Display) -> SdkError {
    SdkError::new(
        format!("APPLICATION_{prefix}_WORKER_FAILED"),
        ErrorCategory::Unavailable,
        true,
        "A module background worker failed.",
    )
    .with_internal_reference(error.to_string())
}

fn background_composition_error(error: impl fmt::Display) -> SdkError {
    SdkError::new(
        "APPLICATION_BACKGROUND_COMPOSITION_INVALID",
        ErrorCategory::Internal,
        false,
        "The production background-worker composition is invalid.",
    )
    .with_internal_reference(error.to_string())
}

fn configuration_error(error: impl fmt::Display) -> SdkError {
    SdkError::new(
        "APPLICATION_BACKGROUND_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The production background-worker configuration is invalid.",
    )
    .with_internal_reference(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU8, Ordering};

    const ACTIVE: u8 = 1;
    const DISABLED: u8 = 2;
    const UNINSTALLED: u8 = 3;

    #[derive(Debug)]
    struct MutableActivation {
        state: AtomicU8,
    }

    impl MutableActivation {
        fn active() -> Self {
            Self {
                state: AtomicU8::new(ACTIVE),
            }
        }

        fn set(&self, state: u8) {
            self.state.store(state, Ordering::Release);
        }
    }

    impl ModuleActivationPort for MutableActivation {
        fn is_active<'a>(
            &'a self,
            _tenant_id: &'a TenantId,
            module_id: &'a ModuleId,
        ) -> PortFuture<'a, Result<bool, SdkError>> {
            Box::pin(async move {
                Ok(module_id.as_str() == CUSTOMER_ENRICHMENT_MODULE_ID
                    && self.state.load(Ordering::Acquire) == ACTIVE)
            })
        }
    }

    #[derive(Debug)]
    struct RecordingWorker {
        calls: Arc<Mutex<Vec<&'static str>>>,
        label: &'static str,
    }

    impl TenantBackgroundWorker for RecordingWorker {
        fn run_tenant_cycle<'a>(
            &'a self,
            _tenant_id: TenantId,
            _now_unix_nanos: i64,
        ) -> PortFuture<'a, Result<(), SdkError>> {
            Box::pin(async move {
                self.calls.lock().unwrap().push(self.label);
                Ok(())
            })
        }
    }

    #[tokio::test]
    async fn provider_precedes_application_and_stops_after_disable_or_uninstall() {
        let activation = Arc::new(MutableActivation::active());
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut builder = BackgroundWorkerRegistryBuilder::new(BTreeSet::from([
            CUSTOMER_ENRICHMENT_MODULE_ID.to_owned(),
        ]));
        let activation_port: Arc<dyn ModuleActivationPort> = activation.clone();
        add_customer_enrichment_workers(
            &mut builder,
            activation_port,
            Arc::new(RecordingWorker {
                calls: calls.clone(),
                label: "provider",
            }),
            Arc::new(RecordingWorker {
                calls: calls.clone(),
                label: "application",
            }),
        )
        .unwrap();
        let registry = builder.build();
        let scheduled = registry
            .scheduled_coordinates()
            .map(|(phase, module_id, worker_id)| {
                (phase.order(), module_id.to_owned(), worker_id.to_owned())
            })
            .collect::<Vec<_>>();
        assert_eq!(
            scheduled,
            vec![
                (
                    240,
                    CUSTOMER_ENRICHMENT_MODULE_ID.to_owned(),
                    PROVIDER_PROCESS_WORKER_ID.to_owned(),
                ),
                (
                    250,
                    CUSTOMER_ENRICHMENT_MODULE_ID.to_owned(),
                    PARTY_DISPLAY_NAME_APPLICATION_WORKER_ID.to_owned(),
                ),
            ]
        );

        let tenant_id = TenantId::try_new("tenant-a").unwrap();
        registry
            .run_tenant_cycle(tenant_id.clone(), 1)
            .await
            .unwrap();
        assert_eq!(*calls.lock().unwrap(), vec!["provider", "application"]);

        activation.set(DISABLED);
        registry
            .run_tenant_cycle(tenant_id.clone(), 2)
            .await
            .unwrap();
        assert_eq!(*calls.lock().unwrap(), vec!["provider", "application"]);

        activation.set(ACTIVE);
        registry
            .run_tenant_cycle(tenant_id.clone(), 3)
            .await
            .unwrap();
        assert_eq!(
            *calls.lock().unwrap(),
            vec!["provider", "application", "provider", "application"]
        );

        activation.set(UNINSTALLED);
        registry.run_tenant_cycle(tenant_id, 4).await.unwrap();
        assert_eq!(
            *calls.lock().unwrap(),
            vec!["provider", "application", "provider", "application"]
        );
    }
}
