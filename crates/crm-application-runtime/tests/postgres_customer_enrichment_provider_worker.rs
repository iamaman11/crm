mod process {
    include!("../../crm-customer-enrichment-worker-composition/tests/postgres_worker_process.rs");

    use crm_application_runtime::{
        CustomerEnrichmentProviderWorkerDependencies, build_customer_enrichment_provider_worker,
    };
    use crm_capability_adapters::{
        AuthorizationGrant, LiveAuthorizationStore, LiveCapabilityAuthorizer,
    };
    use crm_capability_runtime::CapabilityDefinition;
    use crm_customer_enrichment_capability_adapter::{
        enrichment_request_from_snapshot, record_provider_response_capability_definition,
    };
    use crm_module_sdk::testing::FixedClock;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn provider_worker_requires_dispatch_and_response_grants_and_recovers() {
        let Ok(database_url) = std::env::var("DATABASE_URL") else {
            return;
        };
        let admin_database_url = std::env::var("ADMIN_DATABASE_URL").unwrap();
        let store = PostgresDataStore::connect(&database_url, 6).await.unwrap();
        let admin = PgPool::connect(&admin_database_url).await.unwrap();
        let fixture = fixture();
        seed_request(&store, &fixture.created_request)
            .await
            .unwrap();

        let calls = Arc::new(AtomicUsize::new(0));
        let registry = ExactProviderAdapterRegistry::try_new([
            ProviderAdapterRegistration::enabled(
                fixture.work_item.provider_request.adapter_coordinate.clone(),
                ReplaySafeProvider {
                    expected_key: fixture.provider_key.clone(),
                    calls: calls.clone(),
                },
            ),
        ])
        .unwrap();
        let authorization_store = LiveAuthorizationStore::default();
        let clock: Arc<dyn crm_module_sdk::Clock> = Arc::new(FixedClock::new(30_000_000));
        let authorizer = Arc::new(LiveCapabilityAuthorizer::new(
            authorization_store.clone(),
            clock,
        ));
        let worker = build_customer_enrichment_provider_worker(
            CustomerEnrichmentProviderWorkerDependencies {
                store: store.clone(),
                registry: Arc::new(registry),
                authorizer,
            },
        )
        .unwrap();
        let dispatch_definition = request_dispatch_capability_definition().unwrap();
        let response_definition = record_provider_response_capability_definition().unwrap();

        let dispatch_denied = worker.execute(fixture.work_item.clone()).await.unwrap_err();
        assert_eq!(
            dispatch_denied.code,
            "CUSTOMER_ENRICHMENT_DISPATCH_PERMISSION_DENIED"
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            request_status(&store, &fixture.created_request).await,
            EnrichmentRequestStatus::Created
        );

        authorization_store
            .upsert(worker_grant(&dispatch_definition))
            .unwrap();
        let response_denied = worker.execute(fixture.work_item.clone()).await.unwrap_err();
        assert_eq!(
            response_denied.code,
            "CUSTOMER_ENRICHMENT_RESPONSE_PERMISSION_DENIED"
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            request_status(&store, &fixture.created_request).await,
            EnrichmentRequestStatus::Dispatched
        );
        assert_eq!(
            scalar(
                &admin,
                "SELECT count(*)::bigint FROM crm.records WHERE tenant_id = 'tenant-a' AND record_type = 'customer_enrichment.provider_response_receipt'",
            )
            .await,
            0
        );

        authorization_store
            .upsert(worker_grant(&response_definition))
            .unwrap();
        let recovered = worker.execute(fixture.work_item.clone()).await.unwrap();
        assert!(recovered.dispatch_replayed);
        assert!(!recovered.response_replayed);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert_eq!(
            request_status(&store, &fixture.created_request).await,
            EnrichmentRequestStatus::ResponseRecorded
        );
        assert_eq!(
            scalar(
                &admin,
                "SELECT count(*)::bigint FROM crm.records WHERE tenant_id = 'tenant-a' AND record_type = 'customer_enrichment.provider_response_receipt'",
            )
            .await,
            1
        );
    }

    fn worker_grant(definition: &CapabilityDefinition) -> AuthorizationGrant {
        AuthorizationGrant {
            tenant_id: TenantId::try_new(TENANT_ID).unwrap(),
            actor_id: ActorId::try_new(ACTOR_ID).unwrap(),
            policy_id: definition.authorization_policy_id.clone(),
            capability_id: definition.capability_id.clone(),
            capability_version: definition.capability_version.clone(),
            owner_module_id: definition.owner_module_id.clone(),
            policy_version: "provider-worker-policy-v1".to_owned(),
            expires_at_unix_nanos: Some(60_000_000),
        }
    }

    async fn request_status(
        store: &PostgresDataStore,
        request: &EnrichmentRequest,
    ) -> EnrichmentRequestStatus {
        let snapshot = store
            .get_record(
                &read_context(),
                &enrichment_request_record_ref(request).unwrap(),
            )
            .await
            .unwrap()
            .unwrap();
        enrichment_request_from_snapshot(&snapshot)
            .unwrap()
            .status()
    }
}
