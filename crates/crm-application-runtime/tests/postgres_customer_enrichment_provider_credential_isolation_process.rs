mod process {
    include!("../../crm-customer-enrichment-worker-composition/tests/postgres_worker_process.rs");

    use crm_application_runtime::{
        CustomerEnrichmentProviderAdapterConfig, CustomerEnrichmentProviderAdapterState,
        CustomerEnrichmentProviderCredentialBinding, CustomerEnrichmentProviderWorkerDependencies,
        ProviderSecretValueSourcePort, ProviderTransportRegistration,
        StaticProviderTransportCatalog, build_customer_enrichment_provider_registry,
        build_customer_enrichment_provider_worker,
    };
    use crm_capability_adapters::{
        AuthorizationGrant, LiveAuthorizationStore, LiveCapabilityAuthorizer,
    };
    use crm_capability_runtime::CapabilityDefinition;
    use crm_core_events::ProjectionStore;
    use crm_customer_enrichment::{
        PROVIDER_PROCESS_OUTCOME_RESOURCE_TYPE, ProviderAdapterCoordinate,
    };
    use crm_customer_enrichment_capability_adapter::{
        enrichment_request_from_snapshot, provider_response_capability_definition,
    };
    use crm_customer_enrichment_provider_process_composition::{
        CustomerEnrichmentProviderProcessWorker, PROVIDER_PROCESS_PROJECTION_ID,
        PROVIDER_PROCESS_WORKER_ACTOR_ID, ProviderDispatchSourceDisposition,
        ProviderDispatchSourcePort, ProviderDispatchSourceSnapshot,
    };
    use crm_customer_enrichment_registry_http_transport::{
        REGISTRY_HTTP_ADAPTER_CONTRACT_VERSION, REGISTRY_HTTP_ADAPTER_KIND,
        REGISTRY_HTTP_TRANSPORT_KEY,
    };
    use crm_module_sdk::{ErrorCategory, PortFuture, SdkError};
    use std::collections::BTreeMap;

    const WRONG_TENANT_SECRET: &str = "wrong-tenant-provider-secret";

    #[derive(Debug, Clone)]
    struct StaticProcessSecretValues {
        values: BTreeMap<String, ProviderSecretMaterial>,
    }

    impl ProviderSecretValueSourcePort for StaticProcessSecretValues {
        fn resolve(&self, environment_name: &str) -> Result<ProviderSecretMaterial, SdkError> {
            self.values.get(environment_name).cloned().ok_or_else(|| {
                SdkError::new(
                    "TEST_PROVIDER_SECRET_MISSING",
                    ErrorCategory::Internal,
                    false,
                    "The test provider secret is unavailable.",
                )
            })
        }
    }

    #[derive(Clone)]
    struct PersistedProcessSource {
        store: PostgresDataStore,
        initial_request: EnrichmentRequest,
        provider_profile: ProviderProfileVersion,
        party_snapshot: PartySnapshot,
        calls: Arc<AtomicUsize>,
    }

    impl ProviderDispatchSourcePort for PersistedProcessSource {
        fn load<'a>(
            &'a self,
            tenant_id: TenantId,
            request_id: RecordId,
            worker_actor_id: ActorId,
            now_unix_ms: u64,
        ) -> PortFuture<'a, Result<ProviderDispatchSourceDisposition, SdkError>> {
            Box::pin(async move {
                self.calls.fetch_add(1, Ordering::SeqCst);
                if &tenant_id != self.initial_request.tenant_id()
                    || request_id.as_str() != self.initial_request.request_id().as_str()
                    || worker_actor_id.as_str() != PROVIDER_PROCESS_WORKER_ACTOR_ID
                    || !matches!(now_unix_ms, 30 | 31)
                {
                    return Err(SdkError::new(
                        "TEST_PROVIDER_SOURCE_LINEAGE_MISMATCH",
                        ErrorCategory::Internal,
                        false,
                        "The test provider source received different process lineage.",
                    ));
                }
                let snapshot = self
                    .store
                    .get_record(
                        &read_context(),
                        &enrichment_request_record_ref(&self.initial_request)?,
                    )
                    .await
                    .map_err(|_| {
                        SdkError::new(
                            "TEST_PROVIDER_REQUEST_READ_FAILED",
                            ErrorCategory::Internal,
                            true,
                            "The persisted provider request could not be read safely.",
                        )
                    })?
                    .ok_or_else(|| {
                        SdkError::new(
                            "TEST_PROVIDER_REQUEST_MISSING",
                            ErrorCategory::Internal,
                            false,
                            "The persisted provider request is unavailable.",
                        )
                    })?;
                let request = enrichment_request_from_snapshot(&snapshot)?;
                Ok(ProviderDispatchSourceDisposition::Ready(Box::new(
                    ProviderDispatchSourceSnapshot {
                        request,
                        provider_profile: self.provider_profile.clone(),
                        party_snapshot: self.party_snapshot.clone(),
                    },
                )))
            })
        }
    }

    #[derive(Clone)]
    struct ForbiddenProviderState {
        calls: Arc<AtomicUsize>,
    }

    async fn forbidden_provider(
        State(state): State<ForbiddenProviderState>,
        _headers: HeaderMap,
        _body: Bytes,
    ) -> impl IntoResponse {
        state.calls.fetch_add(1, Ordering::SeqCst);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "provider I/O was forbidden",
        )
    }

    async fn spawn_forbidden_provider(calls: Arc<AtomicUsize>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind forbidden HTTP provider");
        let address = listener.local_addr().expect("read provider address");
        let router = Router::new()
            .route(REGISTRY_HTTP_PATH, post(forbidden_provider))
            .with_state(ForbiddenProviderState { calls });
        tokio::spawn(async move {
            axum::serve(listener, router)
                .await
                .expect("serve forbidden HTTP provider");
        });
        format!("http://{address}{REGISTRY_HTTP_PATH}")
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn wrong_tenant_credential_binding_prevents_provider_io_and_checkpoint() {
        let Ok(database_url) = std::env::var("DATABASE_URL") else {
            eprintln!(
                "skipping provider credential-isolation acceptance because DATABASE_URL is absent"
            );
            return;
        };
        let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
            .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
        let store = PostgresDataStore::connect(&database_url, 6)
            .await
            .expect("connect credential-isolation process store");
        let admin = PgPool::connect(&admin_database_url)
            .await
            .expect("connect credential-isolation evidence reader");
        let snapshot = credential_isolation_snapshot();
        seed_request(&store, &snapshot.request)
            .await
            .expect("seed credential-isolation request-created evidence");

        let transport_calls = Arc::new(AtomicUsize::new(0));
        let endpoint = spawn_forbidden_provider(transport_calls.clone()).await;
        let clock = Arc::new(FixedClock::new(31_000_000));
        let transport = RegistryHttpTransport::try_new(
            RegistryHttpTransportConfig::try_new(
                &endpoint,
                [endpoint.clone()],
                Duration::from_secs(1),
                64 * 1024,
                64 * 1024,
            )
            .expect("configure forbidden provider endpoint"),
            clock.clone(),
        )
        .expect("build concrete forbidden provider transport");
        let coordinate = ProviderAdapterCoordinate::try_new(
            REGISTRY_HTTP_ADAPTER_KIND,
            REGISTRY_HTTP_ADAPTER_CONTRACT_VERSION,
        )
        .unwrap();
        let catalog = StaticProviderTransportCatalog::try_new([ProviderTransportRegistration {
            transport_key: REGISTRY_HTTP_TRANSPORT_KEY.to_owned(),
            coordinate: coordinate.clone(),
            transport: Arc::new(transport),
        }])
        .expect("build exact forbidden transport catalog");
        let registry = build_customer_enrichment_provider_registry(
            &[CustomerEnrichmentProviderAdapterConfig {
                coordinate,
                state: CustomerEnrichmentProviderAdapterState::Enabled,
                transport_key: Some(REGISTRY_HTTP_TRANSPORT_KEY.to_owned()),
                maximum_attempts: Some(10),
                quota_window_seconds: Some(60),
                circuit_failure_threshold: Some(3),
                circuit_open_seconds: Some(60),
                credential_bindings: vec![CustomerEnrichmentProviderCredentialBinding {
                    tenant_id: TenantId::try_new("tenant-b").unwrap(),
                    handle_alias: "registry_primary".to_owned(),
                    secret_environment: "TEST_WRONG_TENANT_REGISTRY_TOKEN".to_owned(),
                }],
            }],
            clock,
            Arc::new(catalog),
            Arc::new(StaticProcessSecretValues {
                values: BTreeMap::from([(
                    "TEST_WRONG_TENANT_REGISTRY_TOKEN".to_owned(),
                    ProviderSecretMaterial::try_new(WRONG_TENANT_SECRET.as_bytes().to_vec())
                        .unwrap(),
                )]),
            }),
        )
        .expect("assemble exact provider registry with only a foreign tenant binding");

        let authorization_store = LiveAuthorizationStore::default();
        let authorizer = Arc::new(LiveCapabilityAuthorizer::new(
            authorization_store.clone(),
            Arc::new(FixedClock::new(30_000_000)),
        ));
        for definition in [
            request_dispatch_capability_definition().unwrap(),
            provider_response_capability_definition().unwrap(),
        ] {
            authorization_store
                .upsert(process_worker_grant(&definition))
                .expect("grant exact internal provider capability");
        }
        let provider_worker = build_customer_enrichment_provider_worker(
            CustomerEnrichmentProviderWorkerDependencies {
                store: store.clone(),
                registry: Arc::new(registry),
                authorizer,
            },
        )
        .expect("compose credential-isolation provider worker");
        let source_calls = Arc::new(AtomicUsize::new(0));
        let process = CustomerEnrichmentProviderProcessWorker::new(
            store.clone(),
            Arc::new(PersistedProcessSource {
                store: store.clone(),
                initial_request: snapshot.request.clone(),
                provider_profile: snapshot.provider_profile.clone(),
                party_snapshot: snapshot.party_snapshot.clone(),
                calls: source_calls.clone(),
            }),
            Arc::new(provider_worker),
            ActorId::try_new(PROVIDER_PROCESS_WORKER_ACTOR_ID).unwrap(),
        )
        .expect("compose credential-isolation provider process");
        let tenant_id = TenantId::try_new(TENANT_ID).unwrap();

        let first_error = process
            .run_cycle(tenant_id.clone(), 30_000_000)
            .await
            .expect_err("foreign tenant credential binding must fail closed");
        assert_eq!(
            first_error.code,
            "CUSTOMER_ENRICHMENT_PROVIDER_SECRET_UNAVAILABLE"
        );
        assert!(!first_error.retryable);
        assert!(!format!("{first_error:?} {first_error}").contains(WRONG_TENANT_SECRET));
        assert_eq!(transport_calls.load(Ordering::SeqCst), 0);
        assert_eq!(source_calls.load(Ordering::SeqCst), 1);
        let failed_checkpoint = ProjectionStore::projection_checkpoint(
            &store,
            tenant_id.clone(),
            PROVIDER_PROCESS_PROJECTION_ID.to_owned(),
        )
        .await
        .expect_err("terminal credential failure must fail the provider checkpoint");
        assert_eq!(failed_checkpoint.code, "PROJECTION_CHECKPOINT_FAILED");
        assert!(!failed_checkpoint.retryable);
        let failed_checkpoint_reference = failed_checkpoint
            .internal_reference
            .as_deref()
            .expect("failed checkpoint lineage exists");
        assert!(failed_checkpoint_reference.contains("customer-enrichment-worker-seed-event"));
        assert!(failed_checkpoint_reference.contains(
            "failure_code=CUSTOMER_ENRICHMENT_PROVIDER_SECRET_UNAVAILABLE"
        ));
        assert!(
            store
                .projection_document(
                    &tenant_id,
                    PROVIDER_PROCESS_PROJECTION_ID,
                    PROVIDER_PROCESS_OUTCOME_RESOURCE_TYPE,
                    snapshot.request.request_id().as_str(),
                )
                .await
                .unwrap()
                .is_none()
        );
        let dispatched_snapshot = store
            .get_record(
                &read_context(),
                &enrichment_request_record_ref(&snapshot.request).unwrap(),
            )
            .await
            .unwrap()
            .expect("durable dispatched request exists after credential failure");
        let dispatched_request = enrichment_request_from_snapshot(&dispatched_snapshot).unwrap();
        assert_eq!(
            dispatched_request.status(),
            EnrichmentRequestStatus::Dispatched
        );
        assert!(dispatched_request.response_receipt_id().is_none());

        let baseline = credential_isolation_evidence_counts(&admin).await;
        let replay_error = process
            .run_cycle(tenant_id, 31_000_000)
            .await
            .expect_err("failed credential checkpoint must block restart before source I/O");
        assert_eq!(replay_error.code, "PROJECTION_CHECKPOINT_FAILED");
        assert!(!replay_error.retryable);
        assert_eq!(transport_calls.load(Ordering::SeqCst), 0);
        assert_eq!(source_calls.load(Ordering::SeqCst), 1);
        assert_eq!(credential_isolation_evidence_counts(&admin).await, baseline);
    }

    fn process_worker_grant(definition: &CapabilityDefinition) -> AuthorizationGrant {
        AuthorizationGrant {
            tenant_id: TenantId::try_new(TENANT_ID).unwrap(),
            actor_id: ActorId::try_new(PROVIDER_PROCESS_WORKER_ACTOR_ID).unwrap(),
            policy_id: definition.authorization_policy_id.clone(),
            capability_id: definition.capability_id.clone(),
            capability_version: definition.capability_version.clone(),
            owner_module_id: definition.owner_module_id.clone(),
            policy_version: "provider-credential-isolation-policy-v1".to_owned(),
            expires_at_unix_nanos: Some(60_000_000),
        }
    }

    fn credential_isolation_snapshot() -> ProviderDispatchSourceSnapshot {
        let provider_profile = ProviderProfileVersion::publish(ProviderProfileDraft {
            provider_key: "company_registry_credential_isolation_process".to_owned(),
            adapter_kind: REGISTRY_HTTP_ADAPTER_KIND.to_owned(),
            adapter_contract_version: REGISTRY_HTTP_ADAPTER_CONTRACT_VERSION.to_owned(),
            supported_target_fields: vec![TargetField::PartyDisplayName],
            purpose_codes: vec!["customer_profile_enrichment".to_owned()],
            license_id: "Registry credential-isolation process licence".to_owned(),
            permitted_use_class: "customer_master_review".to_owned(),
            residency_region: "eu".to_owned(),
            retention_days: 30,
            raw_payload_policy: RawPayloadPolicy::DigestOnly,
            credential_handle_aliases: vec!["registry_primary".to_owned()],
            effective_at_unix_ms: 1,
            expires_at_unix_ms: Some(20_000),
        })
        .unwrap();
        let mapping = MappingVersion::publish(MappingDraft {
            mapping_key: "party_display_name_credential_isolation_process".to_owned(),
            provider_profile_version_id: provider_profile.version_id().clone(),
            provider_response_field_path: "organization.legal_name".to_owned(),
            target_field: TargetField::PartyDisplayName,
            normalization: MappingNormalization::CanonicalPartyDisplayNameV1,
            maximum_suggestions_per_response: 1,
            confidence_required: true,
        })
        .unwrap();
        let request = EnrichmentRequest::create(EnrichmentRequestDraft {
            tenant_id: TenantId::try_new(TENANT_ID).unwrap(),
            requested_by: ActorId::try_new(ACTOR_ID).unwrap(),
            idempotency_key: IdempotencyKey::try_new(
                "provider-credential-isolation-process-request",
            )
            .unwrap(),
            target: TargetSnapshot::try_new(
                "party-provider-credential-isolation-process-1",
                7,
                TargetField::PartyDisplayName,
            )
            .unwrap(),
            provider_profile_version_id: provider_profile.version_id().clone(),
            mapping_version_id: mapping.version_id().clone(),
            requested_fields: vec![TargetField::PartyDisplayName],
            policy_evidence: RequestPolicyEvidence::try_new(
                "customer_profile_enrichment",
                "legitimate_interest",
                None,
                "provider-credential-isolation-policy-v1",
            )
            .unwrap(),
            created_at_unix_ms: 10,
            deadline_at_unix_ms: 10_000,
            expires_at_unix_ms: 20_000,
        })
        .unwrap();
        ProviderDispatchSourceSnapshot {
            request,
            provider_profile,
            party_snapshot: PartySnapshot {
                party_id: RecordId::try_new("party-provider-credential-isolation-process-1")
                    .unwrap(),
                display_name: "Credential Isolation Process Company".to_owned(),
                resource_version: 7,
                observed_at_unix_ms: 15,
            },
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct CredentialIsolationEvidenceCounts {
        records: i64,
        events: i64,
        audits: i64,
        idempotency: i64,
        transactions: i64,
        projection_events: i64,
        projection_documents: i64,
    }

    async fn credential_isolation_evidence_counts(
        admin: &PgPool,
    ) -> CredentialIsolationEvidenceCounts {
        CredentialIsolationEvidenceCounts {
            records: scalar(
                admin,
                "SELECT count(*)::bigint FROM crm.records WHERE tenant_id = 'tenant-a' AND owner_module_id = 'crm.customer-enrichment'",
            )
            .await,
            events: scalar(
                admin,
                "SELECT count(*)::bigint FROM crm.outbox_events WHERE tenant_id = 'tenant-a' AND event_type LIKE 'customer_enrichment.%'",
            )
            .await,
            audits: scalar(
                admin,
                "SELECT count(*)::bigint FROM crm.audit_records WHERE tenant_id = 'tenant-a' AND capability_id LIKE 'customer_enrichment.%'",
            )
            .await,
            idempotency: scalar(
                admin,
                "SELECT count(*)::bigint FROM crm.idempotency_records WHERE tenant_id = 'tenant-a'",
            )
            .await,
            transactions: scalar(
                admin,
                "SELECT count(*)::bigint FROM crm.business_transactions WHERE tenant_id = 'tenant-a'",
            )
            .await,
            projection_events: scalar(
                admin,
                "SELECT count(*)::bigint FROM crm.projection_applied_events WHERE tenant_id = 'tenant-a' AND projection_id = 'customer-enrichment-provider-process-v1'",
            )
            .await,
            projection_documents: scalar(
                admin,
                "SELECT count(*)::bigint FROM crm.projection_documents WHERE tenant_id = 'tenant-a' AND projection_id = 'customer-enrichment-provider-process-v1' AND resource_type = 'customer_enrichment.provider_process_outcome'",
            )
            .await,
        }
    }
}
