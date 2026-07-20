from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one replacement anchor, found {count}: {old[:120]!r}")
    file.write_text(text.replace(old, new, 1))


background = "crates/crm-application-runtime/src/background.rs"
replace_once(
    background,
    "use crm_customer_enrichment_capability_adapter::MODULE_ID as CUSTOMER_ENRICHMENT_MODULE_ID;\nuse crm_customer_enrichment_provider_process_composition::{",
    "use crm_customer_enrichment_capability_adapter::MODULE_ID as CUSTOMER_ENRICHMENT_MODULE_ID;\nuse crm_customer_enrichment_materialization_composition::{\n    CustomerEnrichmentMaterializationProcessWorker, MATERIALIZATION_PROCESS_WORKER_ID,\n};\nuse crm_customer_enrichment_provider_process_composition::{",
)
replace_once(
    background,
    "const CUSTOMER_ENRICHMENT_PROVIDER_PROCESS_PHASE: BackgroundWorkerPhase =\n    BackgroundWorkerPhase::new(240);\nconst CUSTOMER_ENRICHMENT_APPLICATION_PHASE: BackgroundWorkerPhase =\n    BackgroundWorkerPhase::new(250);",
    "const CUSTOMER_ENRICHMENT_PROVIDER_PROCESS_PHASE: BackgroundWorkerPhase =\n    BackgroundWorkerPhase::new(240);\nconst CUSTOMER_ENRICHMENT_MATERIALIZATION_PHASE: BackgroundWorkerPhase =\n    BackgroundWorkerPhase::new(245);\nconst CUSTOMER_ENRICHMENT_APPLICATION_PHASE: BackgroundWorkerPhase =\n    BackgroundWorkerPhase::new(250);",
)
replace_once(
    background,
    "    pub customer_enrichment_provider_process: Arc<CustomerEnrichmentProviderProcessWorker>,\n    pub customer_enrichment_application_worker: Arc<CustomerEnrichmentPartyApplicationWorker>,",
    "    pub customer_enrichment_provider_process: Arc<CustomerEnrichmentProviderProcessWorker>,\n    pub customer_enrichment_materialization_process:\n        Arc<CustomerEnrichmentMaterializationProcessWorker>,\n    pub customer_enrichment_application_worker: Arc<CustomerEnrichmentPartyApplicationWorker>,",
)
replace_once(
    background,
    "        customer_enrichment_provider_process,\n        customer_enrichment_application_worker,",
    "        customer_enrichment_provider_process,\n        customer_enrichment_materialization_process,\n        customer_enrichment_application_worker,",
)
replace_once(
    background,
    "        customer_enrichment_provider_process,\n        customer_enrichment_application_worker,\n    )?;",
    "        customer_enrichment_provider_process,\n        customer_enrichment_materialization_process,\n        customer_enrichment_application_worker,\n    )?;",
)
replace_once(
    background,
    """fn add_customer_enrichment_workers(
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
""",
    """fn add_customer_enrichment_workers(
    builder: &mut BackgroundWorkerRegistryBuilder,
    activation: Arc<dyn ModuleActivationPort>,
    provider_process: Arc<dyn TenantBackgroundWorker>,
    materialization_process: Arc<dyn TenantBackgroundWorker>,
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
        activation.clone(),
        CUSTOMER_ENRICHMENT_MATERIALIZATION_PHASE,
        CUSTOMER_ENRICHMENT_MODULE_ID,
        MATERIALIZATION_PROCESS_WORKER_ID,
        materialization_process,
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
""",
)
replace_once(
    background,
    "async fn provider_precedes_application_and_stops_after_disable_or_uninstall()",
    "async fn enrichment_workers_run_in_phase_order_and_stop_after_disable_or_uninstall()",
)
replace_once(
    background,
    """            Arc::new(RecordingWorker {
                calls: calls.clone(),
                label: "provider",
            }),
            Arc::new(RecordingWorker {
                calls: calls.clone(),
                label: "application",
            }),
""",
    """            Arc::new(RecordingWorker {
                calls: calls.clone(),
                label: "provider",
            }),
            Arc::new(RecordingWorker {
                calls: calls.clone(),
                label: "materialization",
            }),
            Arc::new(RecordingWorker {
                calls: calls.clone(),
                label: "application",
            }),
""",
)
replace_once(
    background,
    """                (
                    250,
                    CUSTOMER_ENRICHMENT_MODULE_ID.to_owned(),
                    PARTY_DISPLAY_NAME_APPLICATION_WORKER_ID.to_owned(),
                ),
""",
    """                (
                    245,
                    CUSTOMER_ENRICHMENT_MODULE_ID.to_owned(),
                    MATERIALIZATION_PROCESS_WORKER_ID.to_owned(),
                ),
                (
                    250,
                    CUSTOMER_ENRICHMENT_MODULE_ID.to_owned(),
                    PARTY_DISPLAY_NAME_APPLICATION_WORKER_ID.to_owned(),
                ),
""",
)
replace_once(
    background,
    'assert_eq!(*calls.lock().unwrap(), vec!["provider", "application"]);',
    'assert_eq!(\n            *calls.lock().unwrap(),\n            vec!["provider", "materialization", "application"]\n        );',
)
replace_once(
    background,
    'assert_eq!(*calls.lock().unwrap(), vec!["provider", "application"]);',
    'assert_eq!(\n            *calls.lock().unwrap(),\n            vec!["provider", "materialization", "application"]\n        );',
)
replace_once(
    background,
    'vec!["provider", "application", "provider", "application"]',
    'vec![\n                "provider",\n                "materialization",\n                "application",\n                "provider",\n                "materialization",\n                "application",\n            ]',
)
replace_once(
    background,
    'vec!["provider", "application", "provider", "application"]',
    'vec![\n                "provider",\n                "materialization",\n                "application",\n                "provider",\n                "materialization",\n                "application",\n            ]',
)

runtime = "crates/crm-application-runtime/src/runtime.rs"
replace_once(
    runtime,
    "    CustomerEnrichmentApplicationWorkerDependencies, CustomerEnrichmentProviderProcessDependencies,\n    CustomerEnrichmentProviderWorkerDependencies, GovernedPartyExportSelectionSource,",
    "    CustomerEnrichmentApplicationWorkerDependencies,\n    CustomerEnrichmentMaterializationProcessDependencies,\n    CustomerEnrichmentProviderProcessDependencies, CustomerEnrichmentProviderWorkerDependencies,\n    GovernedPartyExportSelectionSource,",
)
replace_once(
    runtime,
    "    build_customer_enrichment_application_worker, build_customer_enrichment_provider_process,\n    build_customer_enrichment_provider_worker, build_production_background_workers,",
    "    build_customer_enrichment_application_worker,\n    build_customer_enrichment_materialization_process, build_customer_enrichment_provider_process,\n    build_customer_enrichment_provider_worker, build_production_background_workers,",
)
replace_once(
    runtime,
    "use crm_customer_enrichment_capability_adapter::{\n    provider_response_capability_definition, request_dispatch_capability_definition,\n};\nuse crm_customer_enrichment_provider_process_composition::PROVIDER_PROCESS_WORKER_ACTOR_ID;",
    "use crm_customer_enrichment_capability_adapter::{\n    provider_response_capability_definition, request_dispatch_capability_definition,\n};\nuse crm_customer_enrichment_materialization_adapter::suggestion_materialization_capability_definition;\nuse crm_customer_enrichment_materialization_composition::MATERIALIZATION_PROCESS_WORKER_ACTOR_ID;\nuse crm_customer_enrichment_provider_process_composition::PROVIDER_PROCESS_WORKER_ACTOR_ID;",
)
replace_once(
    runtime,
    "        let customer_enrichment_response_definition = provider_response_capability_definition()\n            .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;",
    "        let customer_enrichment_response_definition = provider_response_capability_definition()\n            .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;\n        let customer_enrichment_materialization_definition =\n            suggestion_materialization_capability_definition()\n                .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;",
)
replace_once(
    runtime,
    "        let customer_enrichment_provider_worker_actor_id =\n            ActorId::try_new(PROVIDER_PROCESS_WORKER_ACTOR_ID)\n                .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;",
    "        let customer_enrichment_provider_worker_actor_id =\n            ActorId::try_new(PROVIDER_PROCESS_WORKER_ACTOR_ID)\n                .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;\n        let customer_enrichment_materialization_worker_actor_id =\n            ActorId::try_new(MATERIALIZATION_PROCESS_WORKER_ACTOR_ID)\n                .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;",
)
replace_once(
    runtime,
    """            bootstrap_customer_enrichment_provider_process_access(
                &config,
                now,
                &authorization_store,
                &visibility_store,
                &query_definitions,
                &customer_enrichment_dispatch_definition,
                &customer_enrichment_response_definition,
                &customer_enrichment_provider_worker_actor_id,
            )?;
""",
    """            bootstrap_customer_enrichment_provider_process_access(
                &config,
                now,
                &authorization_store,
                &visibility_store,
                &query_definitions,
                &customer_enrichment_dispatch_definition,
                &customer_enrichment_response_definition,
                &customer_enrichment_provider_worker_actor_id,
            )?;
            bootstrap_customer_enrichment_materialization_process_access(
                &config,
                now,
                &authorization_store,
                &customer_enrichment_materialization_definition,
                &customer_enrichment_materialization_worker_actor_id,
            )?;
""",
)
replace_once(
    runtime,
    """        let customer_enrichment_provider_process = build_customer_enrichment_provider_process(
            CustomerEnrichmentProviderProcessDependencies {
                store: store.clone(),
                executor: customer_enrichment_provider_executor,
                query_authorizer,
                visibility_authorizer: query_visibility,
                cursor_key,
            },
        )
        .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
""",
    """        let customer_enrichment_provider_process = build_customer_enrichment_provider_process(
            CustomerEnrichmentProviderProcessDependencies {
                store: store.clone(),
                executor: customer_enrichment_provider_executor,
                query_authorizer,
                visibility_authorizer: query_visibility,
                cursor_key,
            },
        )
        .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
        let customer_enrichment_materialization_process = Arc::new(
            build_customer_enrichment_materialization_process(
                CustomerEnrichmentMaterializationProcessDependencies {
                    store: store.clone(),
                    authorizer: authorizer.clone(),
                },
            )
            .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?,
        );
""",
)
replace_once(
    runtime,
    "                customer_enrichment_provider_process,\n                customer_enrichment_application_worker,",
    "                customer_enrichment_provider_process,\n                customer_enrichment_materialization_process,\n                customer_enrichment_application_worker,",
)
replace_once(
    runtime,
    "fn bootstrap_customer_enrichment_application_worker_access(\n",
    """fn bootstrap_customer_enrichment_materialization_process_access(
    config: &ApplicationConfig,
    now_unix_nanos: i64,
    authorization_store: &LiveAuthorizationStore,
    materialization_definition: &CapabilityDefinition,
    worker_actor_id: &ActorId,
) -> Result<(), ApplicationRuntimeError> {
    let expires_at = expiry(now_unix_nanos)?;
    for tenant_id in &config.tenant_ids {
        authorization_store
            .upsert(AuthorizationGrant {
                tenant_id: tenant_id.clone(),
                actor_id: worker_actor_id.clone(),
                policy_id: materialization_definition.authorization_policy_id.clone(),
                capability_id: materialization_definition.capability_id.clone(),
                capability_version: materialization_definition.capability_version.clone(),
                owner_module_id: materialization_definition.owner_module_id.clone(),
                policy_version: BOOTSTRAP_POLICY_VERSION.to_owned(),
                expires_at_unix_nanos: Some(expires_at),
            })
            .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
    }
    Ok(())
}

fn bootstrap_customer_enrichment_application_worker_access(
""",
)
