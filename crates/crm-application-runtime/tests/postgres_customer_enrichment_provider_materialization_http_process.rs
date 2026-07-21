mod process {
    include!("../../crm-customer-enrichment-worker-composition/tests/postgres_worker_process.rs");

    use crm_application_runtime::{
        CustomerEnrichmentMaterializationProcessDependencies,
        CustomerEnrichmentProviderAdapterConfig, CustomerEnrichmentProviderAdapterState,
        CustomerEnrichmentProviderCredentialBinding, CustomerEnrichmentProviderWorkerDependencies,
        ProviderSecretValueSourcePort, ProviderTransportRegistration,
        StaticProviderTransportCatalog, build_customer_enrichment_materialization_process,
        build_customer_enrichment_provider_registry, build_customer_enrichment_provider_worker,
    };
    use crm_capability_adapters::{
        AuthorizationGrant, LiveAuthorizationStore, LiveCapabilityAuthorizer,
    };
    use crm_capability_runtime::CapabilityDefinition;
    use crm_core_data::{
        AppendImmutableFileChunk, CreateImmutableFileArtifact, ImmutableFileArtifactStore,
        PostgresImmutableFileArtifactStore,
    };
    use crm_core_events::ProjectionStore;
    use crm_customer_enrichment::{
        PROVIDER_PROCESS_OUTCOME_RESOURCE_TYPE, ProviderAdapterCoordinate,
        ProviderProcessCanonicalOutcome, ProviderProcessOutcomeKind,
    };
    use crm_customer_enrichment_capability_adapter::{
        MAPPING_PUBLISHED_EVENT_SCHEMA, MAPPING_PUBLISHED_EVENT_TYPE,
        PROVIDER_PROFILE_PUBLISHED_EVENT_SCHEMA, PROVIDER_PROFILE_PUBLISHED_EVENT_TYPE,
        enrichment_request_from_snapshot, mapping_persisted_payload, mapping_record_ref,
        mapping_to_wire, provider_profile_persisted_payload, provider_profile_record_ref,
        provider_profile_to_wire, provider_response_capability_definition,
    };
    use crm_customer_enrichment_materialization_adapter::suggestion_materialization_capability_definition;
    use crm_customer_enrichment_materialization_composition::{
        MATERIALIZATION_PROCESS_PROJECTION_ID, MATERIALIZATION_PROCESS_WORKER_ACTOR_ID,
        PROVIDER_SUGGESTION_CANDIDATE_EVIDENCE_MEDIA_TYPE,
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
    use crm_module_sdk::{
        ErrorCategory, FileId, PortFuture, RecordRef, RetentionPolicyId, SdkError, TypedPayload,
    };
    use crm_proto_contracts::crm::customer::v1 as customer;
    use prost::Message;
    use sha2::{Digest, Sha256};
    use std::collections::BTreeMap;

    const PROVIDER_EVIDENCE_FILE_ID: &str = "provider-http-process-candidate-evidence-1";
    const PROVIDER_EVIDENCE_RETENTION_POLICY_ID: &str =
        "crm.customer_enrichment.provider_suggestion_evidence";

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
    struct StaticProcessSource {
        snapshot: ProviderDispatchSourceSnapshot,
        calls: Arc<AtomicUsize>,
    }

    impl ProviderDispatchSourcePort for StaticProcessSource {
        fn load<'a>(
            &'a self,
            tenant_id: TenantId,
            request_id: RecordId,
            worker_actor_id: ActorId,
            now_unix_ms: u64,
        ) -> PortFuture<'a, Result<ProviderDispatchSourceDisposition, SdkError>> {
            Box::pin(async move {
                self.calls.fetch_add(1, Ordering::SeqCst);
                if &tenant_id != self.snapshot.request.tenant_id()
                    || request_id.as_str() != self.snapshot.request.request_id().as_str()
                    || worker_actor_id.as_str() != PROVIDER_PROCESS_WORKER_ACTOR_ID
                    || now_unix_ms != 30
                {
                    return Err(SdkError::new(
                        "TEST_PROVIDER_SOURCE_LINEAGE_MISMATCH",
                        ErrorCategory::Internal,
                        false,
                        "The test provider source received different process lineage.",
                    ));
                }
                Ok(ProviderDispatchSourceDisposition::Ready(Box::new(
                    self.snapshot.clone(),
                )))
            })
        }
    }

    #[derive(Clone)]
    struct GovernedRegistryProviderState {
        expected_key: String,
        calls: Arc<AtomicUsize>,
        evidence_reference: String,
    }

    async fn governed_registry_provider(
        State(state): State<GovernedRegistryProviderState>,
        headers: HeaderMap,
        body: Bytes,
    ) -> impl IntoResponse {
        state.calls.fetch_add(1, Ordering::SeqCst);
        if headers
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            != Some("Bearer super-secret-provider-token")
        {
            return (StatusCode::UNAUTHORIZED, "unauthorized").into_response();
        }
        if headers
            .get("idempotency-key")
            .and_then(|value| value.to_str().ok())
            != Some(state.expected_key.as_str())
        {
            return (StatusCode::CONFLICT, "idempotency conflict").into_response();
        }
        let request: Value = match serde_json::from_slice(&body) {
            Ok(value) => value,
            Err(_) => return (StatusCode::BAD_REQUEST, "invalid request").into_response(),
        };
        if request
            .get("provider_idempotency_key")
            .and_then(Value::as_str)
            != Some(state.expected_key.as_str())
        {
            return (StatusCode::CONFLICT, "lineage conflict").into_response();
        }
        Json(json!({
            "schema_version": PROVIDER_RESPONSE_SCHEMA,
            "replay_key": state.expected_key,
            "provider_correlation_id": "provider-correlation-materialization-process-1",
            "response_class": "success",
            "provider_observed_at_unix_ms": 30,
            "metered_units": 3,
            "protected_evidence_reference": state.evidence_reference,
            "safe_provider_code": "success"
        }))
        .into_response()
    }

    async fn spawn_governed_registry_provider(
        expected_key: String,
        calls: Arc<AtomicUsize>,
        evidence_reference: String,
    ) -> String {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind governed registry HTTP provider");
        let address = listener.local_addr().expect("read provider address");
        let router = Router::new()
            .route(REGISTRY_HTTP_PATH, post(governed_registry_provider))
            .with_state(GovernedRegistryProviderState {
                expected_key,
                calls,
                evidence_reference,
            });
        tokio::spawn(async move {
            axum::serve(listener, router)
                .await
                .expect("serve governed registry HTTP provider");
        });
        format!("http://{address}{REGISTRY_HTTP_PATH}")
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn exact_host_http_response_materializes_one_governed_suggestion() {
        let Ok(database_url) = std::env::var("DATABASE_URL") else {
            eprintln!(
                "skipping provider HTTP materialization acceptance because DATABASE_URL is absent"
            );
            return;
        };
        let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
            .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
        let store = PostgresDataStore::connect(&database_url, 6)
            .await
            .expect("connect provider HTTP materialization store");
        let admin = PgPool::connect(&admin_database_url)
            .await
            .expect("connect provider HTTP materialization evidence reader");
        let fixture = process_fixture();
        seed_provider_dependencies(&store, &fixture)
            .await
            .expect("seed exact provider profile and mapping snapshots");
        seed_request(&store, &fixture.snapshot.request)
            .await
            .expect("seed provider HTTP request-created evidence");

        let process_actor = ActorId::try_new(PROVIDER_PROCESS_WORKER_ACTOR_ID).unwrap();
        let expected_work_item = build_provider_dispatch_work_item(ProviderDispatchWorkItemInput {
            request: &fixture.snapshot.request,
            provider_profile: &fixture.snapshot.provider_profile,
            party_snapshot: &fixture.snapshot.party_snapshot,
            worker_actor_id: &process_actor,
            now_unix_ms: 30,
        })
        .expect("derive exact provider HTTP work item");
        let calls = Arc::new(AtomicUsize::new(0));
        let endpoint = spawn_governed_registry_provider(
            expected_work_item
                .provider_request
                .provider_idempotency_key
                .clone(),
            calls.clone(),
            PROVIDER_EVIDENCE_FILE_ID.to_owned(),
        )
        .await;

        let clock = Arc::new(FixedClock::new(31_000_000));
        let transport = RegistryHttpTransport::try_new(
            RegistryHttpTransportConfig::try_new(
                &endpoint,
                [endpoint.clone()],
                Duration::from_secs(1),
                64 * 1024,
                64 * 1024,
            )
            .expect("configure exact host HTTP endpoint"),
            clock.clone(),
        )
        .expect("build concrete registry HTTP transport");
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
        .expect("build exact host transport catalog");
        let configuration = CustomerEnrichmentProviderAdapterConfig {
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
        };
        let registry = build_customer_enrichment_provider_registry(
            &[configuration],
            clock,
            Arc::new(catalog),
            Arc::new(StaticProcessSecretValues {
                values: BTreeMap::from([(
                    "TEST_REGISTRY_PRIMARY_TOKEN".to_owned(),
                    ProviderSecretMaterial::try_new(PROVIDER_SECRET.as_bytes().to_vec()).unwrap(),
                )]),
            }),
        )
        .expect("assemble governed exact provider registry");

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
                .upsert(worker_grant(&definition, PROVIDER_PROCESS_WORKER_ACTOR_ID))
                .expect("grant exact internal provider capability");
        }
        authorization_store
            .upsert(worker_grant(
                &suggestion_materialization_capability_definition().unwrap(),
                MATERIALIZATION_PROCESS_WORKER_ACTOR_ID,
            ))
            .expect("grant exact internal materialization capability");

        let provider_worker = build_customer_enrichment_provider_worker(
            CustomerEnrichmentProviderWorkerDependencies {
                store: store.clone(),
                registry: Arc::new(registry),
                authorizer: authorizer.clone(),
            },
        )
        .expect("compose authorized provider worker");
        let source_calls = Arc::new(AtomicUsize::new(0));
        let provider_process = CustomerEnrichmentProviderProcessWorker::new(
            store.clone(),
            Arc::new(StaticProcessSource {
                snapshot: fixture.snapshot.clone(),
                calls: source_calls.clone(),
            }),
            Arc::new(provider_worker),
            process_actor,
        )
        .expect("compose event-driven provider process");

        let tenant_id = TenantId::try_new(TENANT_ID).unwrap();
        let provider_cycle = provider_process
            .run_cycle(tenant_id.clone(), 30_000_000)
            .await
            .expect("dispatch request through concrete HTTP provider");
        assert_eq!(provider_cycle.created_events, 1);
        assert_eq!(provider_cycle.dispatched, 1);
        assert_eq!(provider_cycle.dispatch_replays, 0);
        assert_eq!(provider_cycle.response_replays, 0);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(source_calls.load(Ordering::SeqCst), 1);

        let provider_request_snapshot = store
            .get_record(
                &read_context(),
                &enrichment_request_record_ref(&fixture.snapshot.request).unwrap(),
            )
            .await
            .unwrap()
            .expect("response-recorded request exists");
        let provider_request =
            enrichment_request_from_snapshot(&provider_request_snapshot).unwrap();
        assert_eq!(
            provider_request.status(),
            EnrichmentRequestStatus::ResponseRecorded
        );
        let receipt_id = provider_request
            .response_receipt_id()
            .expect("response receipt lineage exists")
            .as_str()
            .to_owned();

        let provider_checkpoint = ProjectionStore::projection_checkpoint(
            &store,
            tenant_id.clone(),
            PROVIDER_PROCESS_PROJECTION_ID.to_owned(),
        )
        .await
        .unwrap()
        .expect("provider process checkpoint exists");
        assert_eq!(provider_checkpoint.applied_event_count, 1);
        let document = store
            .projection_document(
                &tenant_id,
                PROVIDER_PROCESS_PROJECTION_ID,
                PROVIDER_PROCESS_OUTCOME_RESOURCE_TYPE,
                fixture.snapshot.request.request_id().as_str(),
            )
            .await
            .unwrap()
            .expect("canonical provider process outcome exists");
        let outcome = ProviderProcessCanonicalOutcome::from_projection_document(document).unwrap();
        assert_eq!(
            outcome.request_id(),
            fixture.snapshot.request.request_id().as_str()
        );
        assert_eq!(outcome.retry_generation(), 0);
        assert_eq!(outcome.kind(), ProviderProcessOutcomeKind::ResponseRecorded);
        assert_eq!(
            outcome.provider_response_receipt_id(),
            Some(receipt_id.as_str())
        );
        assert!(outcome.provider_response_conflict_id().is_none());

        let materialization_process = build_customer_enrichment_materialization_process(
            CustomerEnrichmentMaterializationProcessDependencies {
                store: store.clone(),
                authorizer: authorizer.clone(),
            },
        )
        .expect("compose production materialization process");
        let pending = materialization_process
            .run_cycle(tenant_id.clone(), 35_000_000)
            .await
            .expect_err("materialization must wait for finalized governed evidence");
        assert_eq!(
            pending.code,
            "CUSTOMER_ENRICHMENT_SUGGESTION_EVIDENCE_UNAVAILABLE"
        );
        assert!(pending.retryable);
        assert!(
            ProjectionStore::projection_checkpoint(
                &store,
                tenant_id.clone(),
                MATERIALIZATION_PROCESS_PROJECTION_ID.to_owned(),
            )
            .await
            .unwrap()
            .is_none()
        );
        assert_eq!(suggestion_count(&admin).await, 0);

        upload_candidate_evidence(&store, &fixture.snapshot, &receipt_id)
            .await
            .expect("finalize canonical provider candidate evidence");
        let materialized = materialization_process
            .run_cycle(tenant_id.clone(), 40_000_000)
            .await
            .expect("materialize one suggestion after provider checkpoint");
        assert_eq!(materialized.response_events, 1);
        assert_eq!(materialized.materialized, 1);
        assert_eq!(materialized.replays, 0);
        assert_eq!(materialized.skipped_failed_responses, 0);
        assert_eq!(materialized.skipped_rejected_requests, 0);
        assert_eq!(suggestion_count(&admin).await, 1);

        let materialized_request_snapshot = store
            .get_record(
                &read_context(),
                &enrichment_request_record_ref(&fixture.snapshot.request).unwrap(),
            )
            .await
            .unwrap()
            .expect("materialized request exists");
        let materialized_request =
            enrichment_request_from_snapshot(&materialized_request_snapshot).unwrap();
        assert_eq!(
            materialized_request.status(),
            EnrichmentRequestStatus::SuggestionsMaterialized
        );
        assert_eq!(
            materialized_request_snapshot.version,
            provider_request_snapshot.version + 1
        );
        let materialization_checkpoint = ProjectionStore::projection_checkpoint(
            &store,
            tenant_id.clone(),
            MATERIALIZATION_PROCESS_PROJECTION_ID.to_owned(),
        )
        .await
        .unwrap()
        .expect("materialization checkpoint exists");
        assert_eq!(materialization_checkpoint.applied_event_count, 1);

        let baseline = process_evidence_counts(&admin).await;
        drop(provider_process);
        drop(materialization_process);

        let restarted_provider = CustomerEnrichmentProviderProcessWorker::new(
            store.clone(),
            Arc::new(StaticProcessSource {
                snapshot: fixture.snapshot.clone(),
                calls: source_calls.clone(),
            }),
            Arc::new(ForbiddenProcessExecutor),
            ActorId::try_new(PROVIDER_PROCESS_WORKER_ACTOR_ID).unwrap(),
        )
        .expect("recompose provider process after checkpoint");
        let provider_replay = restarted_provider
            .run_cycle(tenant_id.clone(), 45_000_000)
            .await
            .expect("checkpointed provider restart is a no-op");
        assert_eq!(provider_replay.created_events, 0);
        assert_eq!(provider_replay.dispatched, 0);

        let restarted_materialization = build_customer_enrichment_materialization_process(
            CustomerEnrichmentMaterializationProcessDependencies {
                store: store.clone(),
                authorizer,
            },
        )
        .expect("recompose production materialization process after checkpoint");
        let materialization_replay = restarted_materialization
            .run_cycle(tenant_id, 50_000_000)
            .await
            .expect("checkpointed materialization restart is a no-op");
        assert_eq!(materialization_replay.response_events, 0);
        assert_eq!(materialization_replay.materialized, 0);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(source_calls.load(Ordering::SeqCst), 1);
        assert_eq!(suggestion_count(&admin).await, 1);
        assert_eq!(process_evidence_counts(&admin).await, baseline);
    }

    #[derive(Clone)]
    struct ForbiddenProcessExecutor;

    impl crm_customer_enrichment_provider_process_composition::ProviderDispatchExecutorPort
        for ForbiddenProcessExecutor
    {
        fn execute<'a>(
            &'a self,
            _work_item: ProviderDispatchWorkItem,
        ) -> PortFuture<'a, Result<ProviderDispatchExecution, SdkError>> {
            Box::pin(async {
                Err(SdkError::new(
                    "TEST_PROVIDER_EXECUTOR_MUST_NOT_RUN",
                    ErrorCategory::Internal,
                    false,
                    "The provider executor must not run after checkpoint recovery.",
                ))
            })
        }
    }

    fn worker_grant(definition: &CapabilityDefinition, actor_id: &str) -> AuthorizationGrant {
        AuthorizationGrant {
            tenant_id: TenantId::try_new(TENANT_ID).unwrap(),
            actor_id: ActorId::try_new(actor_id).unwrap(),
            policy_id: definition.authorization_policy_id.clone(),
            capability_id: definition.capability_id.clone(),
            capability_version: definition.capability_version.clone(),
            owner_module_id: definition.owner_module_id.clone(),
            policy_version: "provider-http-materialization-policy-v1".to_owned(),
            expires_at_unix_nanos: Some(100_000_000),
        }
    }

    struct ProcessFixture {
        snapshot: ProviderDispatchSourceSnapshot,
        mapping: MappingVersion,
    }

    fn process_fixture() -> ProcessFixture {
        let provider_profile = ProviderProfileVersion::publish(ProviderProfileDraft {
            provider_key: "company_registry_http_materialization_process".to_owned(),
            adapter_kind: REGISTRY_HTTP_ADAPTER_KIND.to_owned(),
            adapter_contract_version: REGISTRY_HTTP_ADAPTER_CONTRACT_VERSION.to_owned(),
            supported_target_fields: vec![TargetField::PartyDisplayName],
            purpose_codes: vec!["customer_profile_enrichment".to_owned()],
            license_id: "Registry HTTP materialization licence".to_owned(),
            permitted_use_class: "customer_master_review".to_owned(),
            residency_region: "eu".to_owned(),
            retention_days: 30,
            raw_payload_policy: RawPayloadPolicy::GovernedProtectedEvidence,
            credential_handle_aliases: vec!["registry_primary".to_owned()],
            effective_at_unix_ms: 1,
            expires_at_unix_ms: Some(20_000),
        })
        .unwrap();
        let mapping = MappingVersion::publish(MappingDraft {
            mapping_key: "party_display_name_http_materialization_process".to_owned(),
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
                "provider-http-materialization-process-request",
            )
            .unwrap(),
            target: TargetSnapshot::try_new(
                "party-provider-http-materialization-process-1",
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
                "provider-http-materialization-policy-v1",
            )
            .unwrap(),
            created_at_unix_ms: 10,
            deadline_at_unix_ms: 10_000,
            expires_at_unix_ms: 20_000,
        })
        .unwrap();
        ProcessFixture {
            snapshot: ProviderDispatchSourceSnapshot {
                request,
                provider_profile,
                party_snapshot: PartySnapshot {
                    party_id: RecordId::try_new("party-provider-http-materialization-process-1")
                        .unwrap(),
                    display_name: "HTTP Materialization Process Company".to_owned(),
                    resource_version: 7,
                    observed_at_unix_ms: 15,
                },
            },
            mapping,
        }
    }

    async fn seed_provider_dependencies(
        store: &PostgresDataStore,
        fixture: &ProcessFixture,
    ) -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            fixture.mapping.version_id(),
            fixture.snapshot.request.mapping_version_id()
        );
        seed_definition_record(
            store,
            DefinitionSeed {
                suffix: "provider-profile",
                occurred_at_unix_nanos: 1_000_000,
                record: provider_profile_record_ref(&fixture.snapshot.provider_profile)?,
                record_payload: provider_profile_persisted_payload(
                    &fixture.snapshot.provider_profile,
                )?,
                event_type: PROVIDER_PROFILE_PUBLISHED_EVENT_TYPE,
                event_payload: support::protobuf_payload(
                    MODULE_ID,
                    PROVIDER_PROFILE_PUBLISHED_EVENT_SCHEMA,
                    DataClass::Confidential,
                    &wire::ProviderProfileVersionPublishedEvent {
                        provider_profile_version: Some(provider_profile_to_wire(
                            &fixture.snapshot.provider_profile,
                        )),
                    },
                )?,
            },
        )
        .await?;
        seed_definition_record(
            store,
            DefinitionSeed {
                suffix: "mapping",
                occurred_at_unix_nanos: 2_000_000,
                record: mapping_record_ref(&fixture.mapping)?,
                record_payload: mapping_persisted_payload(&fixture.mapping)?,
                event_type: MAPPING_PUBLISHED_EVENT_TYPE,
                event_payload: support::protobuf_payload(
                    MODULE_ID,
                    MAPPING_PUBLISHED_EVENT_SCHEMA,
                    DataClass::Confidential,
                    &wire::MappingVersionPublishedEvent {
                        mapping_version: Some(mapping_to_wire(&fixture.mapping)),
                    },
                )?,
            },
        )
        .await?;
        Ok(())
    }

    struct DefinitionSeed {
        suffix: &'static str,
        occurred_at_unix_nanos: i64,
        record: RecordRef,
        record_payload: TypedPayload,
        event_type: &'static str,
        event_payload: TypedPayload,
    }

    async fn seed_definition_record(
        store: &PostgresDataStore,
        seed: DefinitionSeed,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let request_hash = semantic_input_hash(&seed.event_payload);
        let context = worker_context(
            &format!("provider-http-materialization-seed-{}", seed.suffix),
            SEED_CAPABILITY,
            &format!(
                "provider-http-materialization-seed-idempotency-{}",
                seed.suffix
            ),
            &format!("provider-http-materialization-seed-tx-{}", seed.suffix),
            seed.occurred_at_unix_nanos,
        );
        store
            .create_record(&RecordCreatePlan {
                context,
                record: seed.record.clone(),
                record_payload: seed.record_payload,
                event_id: format!("provider-http-materialization-seed-event-{}", seed.suffix),
                event: DomainEvent {
                    event_type: EventType::try_new(seed.event_type)?,
                    aggregate: seed.record,
                    expected_aggregate_version: None,
                    deduplication_key: format!(
                        "provider-http-materialization-seed-{}",
                        seed.suffix
                    ),
                    payload: seed.event_payload,
                },
                idempotency: IdempotencyEvidence {
                    scope: format!("{SEED_CAPABILITY}@1.0.0"),
                    key: format!("provider-http-materialization-seed-{}", seed.suffix),
                    request_hash,
                    expires_at_unix_nanos: 86_400_000_000_000 + seed.occurred_at_unix_nanos,
                },
                audit: AuditIntent {
                    audit_record_id: format!(
                        "provider-http-materialization-seed-audit-{}",
                        seed.suffix
                    ),
                    canonicalization_profile: "crm.cjson/v1".to_owned(),
                    canonical_envelope: format!("{{\"seed\":\"{}\"}}", seed.suffix).into_bytes(),
                    occurred_at_unix_nanos: seed.occurred_at_unix_nanos,
                },
            })
            .await?;
        Ok(())
    }

    async fn upload_candidate_evidence(
        store: &PostgresDataStore,
        snapshot: &ProviderDispatchSourceSnapshot,
        receipt_id: &str,
    ) -> Result<(), SdkError> {
        let command = wire::MaterializeSuggestionsRequest {
            enrichment_request_ref: Some(wire::EnrichmentRequestRef {
                enrichment_request_id: snapshot.request.request_id().as_str().to_owned(),
            }),
            provider_response_receipt_ref: Some(wire::ProviderResponseReceiptRef {
                provider_response_receipt_id: receipt_id.to_owned(),
            }),
            candidates: vec![provider_candidate()],
        };
        let bytes = command.encode_to_vec();
        let digest: [u8; 32] = Sha256::digest(&bytes).into();
        let artifacts = PostgresImmutableFileArtifactStore::new(store.clone());
        let context = worker_context(
            "provider-http-materialization-evidence-request",
            "customer_enrichment.suggestion.evidence.store",
            "provider-http-materialization-evidence-idempotency",
            "provider-http-materialization-evidence-tx",
            36_000_000,
        );
        let file_id = FileId::try_new(PROVIDER_EVIDENCE_FILE_ID).unwrap();
        artifacts
            .create(
                &context,
                CreateImmutableFileArtifact {
                    file_id: file_id.clone(),
                    owner_module_id: ModuleId::try_new(MODULE_ID).unwrap(),
                    media_type: PROVIDER_SUGGESTION_CANDIDATE_EVIDENCE_MEDIA_TYPE.to_owned(),
                    data_class: DataClass::Personal,
                    retention_policy_id: RetentionPolicyId::try_new(
                        PROVIDER_EVIDENCE_RETENTION_POLICY_ID,
                    )
                    .unwrap(),
                    expected_size_bytes: bytes.len() as u64,
                    expected_sha256: digest,
                },
            )
            .await?;
        artifacts
            .append_chunk(
                &context,
                AppendImmutableFileChunk {
                    file_id: file_id.clone(),
                    chunk_index: 0,
                    chunk_sha256: digest,
                    bytes,
                },
            )
            .await?;
        artifacts.finalize(&context, &file_id).await?;
        Ok(())
    }

    fn provider_candidate() -> wire::ProviderSuggestionCandidate {
        wire::ProviderSuggestionCandidate {
            target: Some(wire::EnrichmentTargetSnapshot {
                party_ref: Some(customer::PartyRef {
                    party_id: "party-provider-http-materialization-process-1".to_owned(),
                }),
                party_resource_version: 7,
                target_field: wire::EnrichmentTargetField::PartyDisplayName as i32,
            }),
            proposed_value: "HTTP Materialized Company".to_owned(),
            observed_at_unix_ms: Some(30),
            effective_at_unix_ms: 30,
            fresh_until_unix_ms: 10_000,
            expires_at_unix_ms: 15_000,
            confidence_basis_points: Some(9_000),
            policy_evidence: Some(wire::ProviderPolicyEvidence {
                license_id: "Registry HTTP materialization licence".to_owned(),
                permitted_use_class: "customer_master_review".to_owned(),
                residency_region: "eu".to_owned(),
                retention_days: 30,
                consent_evidence_reference: None,
            }),
            evidence_references: vec![PROVIDER_EVIDENCE_FILE_ID.to_owned()],
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct ProcessEvidenceCounts {
        records: i64,
        events: i64,
        audits: i64,
        idempotency: i64,
        transactions: i64,
        provider_projection_events: i64,
        provider_projection_documents: i64,
        materialization_projection_events: i64,
    }

    async fn process_evidence_counts(admin: &PgPool) -> ProcessEvidenceCounts {
        ProcessEvidenceCounts {
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
            provider_projection_events: scalar(
                admin,
                "SELECT count(*)::bigint FROM crm.projection_applied_events WHERE tenant_id = 'tenant-a' AND projection_id = 'customer-enrichment-provider-process-v1'",
            )
            .await,
            provider_projection_documents: scalar(
                admin,
                "SELECT count(*)::bigint FROM crm.projection_documents WHERE tenant_id = 'tenant-a' AND projection_id = 'customer-enrichment-provider-process-v1' AND resource_type = 'customer_enrichment.provider_process_outcome'",
            )
            .await,
            materialization_projection_events: scalar(
                admin,
                "SELECT count(*)::bigint FROM crm.projection_applied_events WHERE tenant_id = 'tenant-a' AND projection_id = 'customer-enrichment-materialization-process-v1'",
            )
            .await,
        }
    }

    async fn suggestion_count(admin: &PgPool) -> i64 {
        scalar(
            admin,
            "SELECT count(*)::bigint FROM crm.records WHERE tenant_id = 'tenant-a' AND record_type = 'customer_enrichment.suggestion'",
        )
        .await
    }
}
