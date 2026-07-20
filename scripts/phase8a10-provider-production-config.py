from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one anchor, found {count}: {old[:180]!r}")
    file.write_text(text.replace(old, new, 1))


runtime = "crates/crm-application-runtime/src/runtime.rs"
replace_once(
    runtime,
    '''    ApplicationConfig, ApplicationGatewayService, BootstrapVisibilityResource,
    CustomerEnrichmentApplicationWorkerDependencies,
''',
    '''    ApplicationConfig, ApplicationGatewayService, BootstrapVisibilityResource,
    CustomerEnrichmentApplicationWorkerDependencies,
''',
)
replace_once(
    runtime,
    '''    CustomerEnrichmentProviderProcessDependencies, CustomerEnrichmentProviderWorkerDependencies,
    GovernedPartyExportSelectionSource, PartyExportArtifactDownloadService,
''',
    '''    CustomerEnrichmentProviderProcessDependencies, CustomerEnrichmentProviderWorkerDependencies,
    GovernedPartyExportSelectionSource, PartyExportArtifactDownloadService,
    ProcessProviderSecretValueSource, ProviderSecretValueSourcePort,
    ProviderTransportCatalogPort, StaticProviderTransportCatalog,
''',
)
replace_once(
    runtime,
    '''    build_customer_enrichment_materialization_process, build_customer_enrichment_provider_process,
    build_customer_enrichment_provider_worker, build_production_background_workers,
''',
    '''    build_customer_enrichment_materialization_process, build_customer_enrichment_provider_process,
    build_customer_enrichment_provider_registry, build_customer_enrichment_provider_worker,
    build_production_background_workers,
''',
)
replace_once(
    runtime,
    'use crm_customer_enrichment_provider_registry::ExactProviderAdapterRegistry;\n',
    '',
)
replace_once(
    runtime,
    '''impl ApplicationRuntime {
    pub async fn assemble(config: ApplicationConfig) -> Result<Self, ApplicationRuntimeError> {
        config.validate()?;
''',
    '''impl ApplicationRuntime {
    pub async fn assemble(config: ApplicationConfig) -> Result<Self, ApplicationRuntimeError> {
        Self::assemble_with_provider_infrastructure(
            config,
            Arc::new(StaticProviderTransportCatalog::default()),
            Arc::new(ProcessProviderSecretValueSource),
        )
        .await
    }

    /// Assembles the process with an explicit exact provider-transport catalog. The default
    /// `assemble` path supplies an empty catalog, so enabled adapter configuration fails startup
    /// until the process host intentionally links a matching vendor transport implementation.
    pub async fn assemble_with_provider_infrastructure(
        config: ApplicationConfig,
        provider_transport_catalog: Arc<dyn ProviderTransportCatalogPort>,
        provider_secret_values: Arc<dyn ProviderSecretValueSourcePort>,
    ) -> Result<Self, ApplicationRuntimeError> {
        config.validate()?;
''',
)
replace_once(
    runtime,
    '''        // No adapter coordinate is enabled implicitly. Concrete provider adapters are added only
        // through exact production configuration; the empty immutable registry fails closed.
        let customer_enrichment_provider_executor = Arc::new(
            build_customer_enrichment_provider_worker(
                CustomerEnrichmentProviderWorkerDependencies {
                    store: store.clone(),
                    registry: Arc::new(ExactProviderAdapterRegistry::default()),
                    authorizer: authorizer.clone(),
                },
            )
            .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?,
        );
''',
    '''        // No adapter coordinate is enabled implicitly. Exact configuration is assembled only
        // against host-linked transport implementations and environment-backed secret handles.
        let customer_enrichment_provider_registry = build_customer_enrichment_provider_registry(
            &config.customer_enrichment_provider_adapters,
            Arc::clone(&clock),
            provider_transport_catalog,
            provider_secret_values,
        )
        .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
        let customer_enrichment_provider_executor = Arc::new(
            build_customer_enrichment_provider_worker(
                CustomerEnrichmentProviderWorkerDependencies {
                    store: store.clone(),
                    registry: Arc::new(customer_enrichment_provider_registry),
                    authorizer: authorizer.clone(),
                },
            )
            .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?,
        );
''',
)
