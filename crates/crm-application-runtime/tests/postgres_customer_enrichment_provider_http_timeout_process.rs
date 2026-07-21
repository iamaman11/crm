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
        ProviderProcessCanonicalOutcome, ProviderProcessOutcomeKind,
    };
    use crm_customer_enrichment_capability_adapter::{
        enrichment_request_from_snapshot, provider_response_capability_definition,
    };
    use crm_customer_enrichment_provider_process_composition::{
        CustomerEnrichmentProviderProcessWorker, PROVIDER_PROCESS_PROJECTION_ID,
        PROVIDER_PROCESS_WORKER_ACTOR_ID, ProviderDispatchSourceDisposition,
        ProviderDispatchSourcePort, ProviderDispatchSourceSnapshot, ProviderDispatchWorkItemInput,
        build_provider_dispatch_work_item,
    };
    use crm_customer_enrichment_registry_http_transport::{
        REGISTRY_HTTP_ADAPTER_CONTRACT_VERSION, REGISTRY_HTTP_ADAPTER_KIND,
        REGISTRY_HTTP_TRANSPORT_KEY,
    };
    use crm_module_sdk::{ErrorCategory, PortFuture, SdkError};
    use std::collections::BTreeMap;
    use std::sync::Mutex;

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
                    .await?
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
    struct TimeoutThenSuccessState {
        calls: Arc<AtomicUsize>,
        observed_keys: Arc<Mutex<Vec<String>>>,
    }

    async fn timeout_then_success_provider(
        State(state): State<TimeoutThenSuccessState>,
        headers: HeaderMap,
        body: Bytes,
    ) -> impl IntoResponse {
        let call_index = state.calls.fetch_add(1, Ordering::SeqCst);
        if headers
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            != Some("Bearer super-secret-provider-token")
        {
            return (StatusCode::UNAUTHORIZED, "unauthorized").into_response();
        }
        let header_key = match headers
            .get("idempotency-key")
            .and_then(|value| value.to_str().ok())
        {
            Some(value) => value.to_owned(),
            None => return (StatusCode::BAD_REQUEST, "missing idempotency key").into_response(),
        };
        let request: Value = match serde_json::from_slice(&body) {
            Ok(value) => value,
            Err(_) => return (StatusCode::BAD_REQUEST, "invalid request").into_response(),
        };
        if request
            .get("provider_idempotency_key")
            .and_then(Value::as_str)
            != Some(header_key.as_str())
        {
            return (StatusCode::CONFLICT, "lineage conflict").into_response();
        }
        state
            .observed_keys
            .lock()
            .expect("record provider idempotency key")
            .push(header_key.clone());
        if call_index == 0 {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Json(json!({
            "schema_version": PROVIDER_RESPONSE_SCHEMA,
            "replay_key": header_key,
            "provider_correlation_id": "provider-correlation-timeout-process-1",
            "response_class": "success",
            "provider_observed_at_unix_ms": 30,
            "metered_units": 2,
            "protected_evidence_reference": null,
            "safe_provider_code": "success"
        }))
        .into_response()
    }

    async fn spawn_timeout_then_success_provider(
        calls: Arc<AtomicUsize>,
        observed_keys: Arc<Mutex<Vec<String>>>,
    ) -> String {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind timeout HTTP provider");
        let address = listener.local_addr().expect("read provider address");
        let router = Router::new()
            .route(REGISTRY_HTTP_PATH, post(timeout_then_success_provider))
            .with_state(TimeoutThenSuccessState {
                calls,
                observed_keys,
            });
        tokio::spawn(async move {
            axum::serve(listener, router)
                .await
                .expect("serve timeout HTTP provider");
        });
        format!("http://{address}{REGISTRY_HTTP_PATH}")
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn timeout_recovery_reuses_exact_provider_key_and_checkpoint_lineage() {
        let Ok(database_url) = std::env::var("DATABASE_URL") else {
            eprintln!("skipping provider HTTP timeout acceptance because DATABASE_URL is absent");
            return;
        };
        let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
            .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
        let store = PostgresDataStore::connect(&database_url, 6)
            .await
            .expect("connect provider HTTP timeout process store");
        let admin = PgPool::connect(&admin_database_url)
            .await
            .expect("connect provider HTTP timeout evidence reader");
        let snapshot = timeout_process_snapshot();
        seed_request(&store, &snapshot.request)
            .await
            .expect("seed timeout request-created evidence");

        let process_actor = ActorId::try_new(PROVIDER_PROCESS_WORKER_ACTOR_ID).unwrap();
        let initial_work_item = build_provider_dispatch_work_item(ProviderDispatchWorkItemInput {
            request: &snapshot.request,
            provider_profile: &snapshot.provider_profile,
            party_snapshot: &snapshot.party_snapshot,
            worker_actor_id: &process_actor,
            now_unix_ms: 30,
        })
        .expect("derive initial timeout work item");
        let expected_provider_key = initial_work_item
            .provider_request
            .provider_idempotency_key
            .clone();
        let calls = Arc::new(AtomicUsize::new(0));
        let observed_keys = Arc::new(Mutex::new(Vec::new()));
        let endpoint =
            spawn_timeout_then_success_provider(calls.clone(), observed_keys.clone()).await;

        let clock = Arc::new(FixedClock::new(31_000_000));
        let transport = RegistryHttpTransport::try_new(
            RegistryHttpTransportConfig::try_new(
                &endpoint,
                [endpoint.clone()],
                Duration::from_millis(20),
                64 * 1024,
                64 * 1024,
            )
            .expect("configure bounded timeout endpoint"),
            clock.clone(),
        )
        .expect("build concrete timeout transport");
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
        .expect("build exact timeout transport catalog");
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
                    tenant_id: TenantId::try_new(TENANT_ID).unwrap(),
                    handle_alias: "registry_primary".to_owned(),
                    secret_environment: "TEST_REGISTRY_PRIMARY_TOKEN".to_owned(),
                }],
            }],
            clock,
            Arc::new(catalog),
            Arc::new(StaticProcessSecretValues {
                values: BTreeMap::from([(
                    "TEST_REGISTRY_PRIMARY_TOKEN".to_owned(),
                    ProviderSecretMaterial::try_new(PROVIDER_SECRET.as_bytes().to_vec()).unwrap(),
                )]),
            }),
        )
        .expect("assemble timeout provider registry");

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
        .expect("compose timeout provider worker");
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
            process_actor,
        )
        .expect("compose timeout provider process");
        let tenant_id = TenantId::try_new(TENANT_ID).unwrap();

        let timeout = process
            .run_cycle(tenant_id.clone(), 30_000_000)
            .await
            .expect_err("first provider call must time out");
        assert_eq!(
            timeout.code,
            "CUSTOMER_ENRICHMENT_PROVIDER_RETRYABLE_FAILURE"
        );
        assert!(timeout.retryable);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(source_calls.load(Ordering::SeqCst), 1);
        assert!(
            ProjectionStore::projection_checkpoint(
                &store,
                tenant_id.clone(),
                PROVIDER_PROCESS_PROJECTION_ID.to_owned(),
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
            .expect("durable dispatched request exists after timeout");
        let dispatched_request = enrichment_request_from_snapshot(&dispatched_snapshot).unwrap();
        assert_eq!(
            dispatched_request.status(),
            EnrichmentRequestStatus::Dispatched
        );
        assert!(dispatched_request.response_receipt_id().is_none());

        let recovered = process
            .run_cycle(tenant_id.clone(), 31_000_000)
            .await
            .expect("retry after timeout records the exact response");
        assert_eq!(recovered.created_events, 1);
        assert_eq!(recovered.dispatched, 1);
        assert_eq!(recovered.dispatch_replays, 1);
        assert_eq!(recovered.response_replays, 0);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert_eq!(source_calls.load(Ordering::SeqCst), 2);
        let observed = observed_keys.lock().expect("read observed provider keys");
        assert_eq!(observed.len(), 2);
        assert!(observed.iter().all(|key| key == &expected_provider_key));
        drop(observed);

        let recorded_snapshot = store
            .get_record(
                &read_context(),
                &enrichment_request_record_ref(&snapshot.request).unwrap(),
            )
            .await
            .unwrap()
            .expect("response-recorded request exists after timeout recovery");
        let recorded_request = enrichment_request_from_snapshot(&recorded_snapshot).unwrap();
        assert_eq!(
            recorded_request.status(),
            EnrichmentRequestStatus::ResponseRecorded
        );
        let receipt_id = recorded_request
            .response_receipt_id()
            .expect("one response receipt is linked")
            .as_str()
            .to_owned();
        let checkpoint = ProjectionStore::projection_checkpoint(
            &store,
            tenant_id.clone(),
            PROVIDER_PROCESS_PROJECTION_ID.to_owned(),
        )
        .await
        .unwrap()
        .expect("provider checkpoint exists after timeout recovery");
        assert_eq!(checkpoint.applied_event_count, 1);
        let document = store
            .projection_document(
                &tenant_id,
                PROVIDER_PROCESS_PROJECTION_ID,
                PROVIDER_PROCESS_OUTCOME_RESOURCE_TYPE,
                snapshot.request.request_id().as_str(),
            )
            .await
            .unwrap()
            .expect("canonical provider outcome exists after recovery");
        let outcome = ProviderProcessCanonicalOutcome::from_projection_document(document).unwrap();
        assert_eq!(outcome.kind(), ProviderProcessOutcomeKind::ResponseRecorded);
        assert_eq!(
            outcome.provider_response_receipt_id(),
            Some(receipt_id.as_str())
        );

        let baseline = timeout_evidence_counts(&admin).await;
        let replay = process
            .run_cycle(tenant_id, 40_000_000)
            .await
            .expect("checkpointed timeout recovery restart is a no-op");
        assert_eq!(replay.created_events, 0);
        assert_eq!(replay.dispatched, 0);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert_eq!(source_calls.load(Ordering::SeqCst), 2);
        assert_eq!(timeout_evidence_counts(&admin).await, baseline);
    }

    fn process_worker_grant(definition: &CapabilityDefinition) -> AuthorizationGrant {
        AuthorizationGrant {
            tenant_id: TenantId::try_new(TENANT_ID).unwrap(),
            actor_id: ActorId::try_new(PROVIDER_PROCESS_WORKER_ACTOR_ID).unwrap(),
            policy_id: definition.authorization_policy_id.clone(),
            capability_id: definition.capability_id.clone(),
            capability_version: definition.capability_version.clone(),
            owner_module_id: definition.owner_module_id.clone(),
            policy_version: "provider-http-timeout-policy-v1".to_owned(),
            expires_at_unix_nanos: Some(60_000_000),
        }
    }

    fn timeout_process_snapshot() -> ProviderDispatchSourceSnapshot {
        let provider_profile = ProviderProfileVersion::publish(ProviderProfileDraft {
            provider_key: "company_registry_http_timeout_process".to_owned(),
            adapter_kind: REGISTRY_HTTP_ADAPTER_KIND.to_owned(),
            adapter_contract_version: REGISTRY_HTTP_ADAPTER_CONTRACT_VERSION.to_owned(),
            supported_target_fields: vec![TargetField::PartyDisplayName],
            purpose_codes: vec!["customer_profile_enrichment".to_owned()],
            license_id: "Registry HTTP timeout process licence".to_owned(),
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
            mapping_key: "party_display_name_http_timeout_process".to_owned(),
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
            idempotency_key: IdempotencyKey::try_new("provider-http-timeout-process-request")
                .unwrap(),
            target: TargetSnapshot::try_new(
                "party-provider-http-timeout-process-1",
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
                "provider-http-timeout-policy-v1",
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
                party_id: RecordId::try_new("party-provider-http-timeout-process-1").unwrap(),
                display_name: "HTTP Timeout Process Company".to_owned(),
                resource_version: 7,
                observed_at_unix_ms: 15,
            },
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct TimeoutEvidenceCounts {
        records: i64,
        events: i64,
        audits: i64,
        idempotency: i64,
        transactions: i64,
        projection_events: i64,
        projection_documents: i64,
    }

    async fn timeout_evidence_counts(admin: &PgPool) -> TimeoutEvidenceCounts {
        TimeoutEvidenceCounts {
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
