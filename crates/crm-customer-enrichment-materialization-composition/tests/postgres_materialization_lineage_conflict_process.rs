mod process {
    include!("postgres_materialization_event_process.rs");

    use crm_customer_enrichment_materialization_composition::{
        ProviderSuggestionCandidateEvidenceRequest, ProviderSuggestionCandidateEvidenceSourcePort,
        SuggestionMaterializationExecutorPort,
    };
    use crm_module_sdk::PortFuture;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn lineage_mismatched_evidence_fails_checkpoint_before_materialization() {
        let Ok(database_url) = std::env::var("DATABASE_URL") else {
            eprintln!(
                "skipping PostgreSQL materialization lineage-conflict process because DATABASE_URL is absent"
            );
            return;
        };
        let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
            .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
        let store = PostgresDataStore::connect(&database_url, 6)
            .await
            .expect("connect materialization lineage-conflict store");
        let admin = PgPool::connect(&admin_database_url)
            .await
            .expect("connect materialization lineage-conflict evidence reader");
        let fixture = fixture();
        seed_dependencies(&store, &fixture)
            .await
            .expect("seed materialization lineage-conflict dependencies");
        apply_recorded_provider_outcome(&store, &fixture)
            .await
            .expect("apply canonical provider outcome with provider checkpoint");

        let artifacts = Arc::new(PostgresImmutableFileArtifactStore::new(store.clone()));
        upload_lineage_mismatched_candidate_evidence(artifacts.as_ref(), &fixture)
            .await
            .expect("finalize lineage-mismatched candidate evidence");
        let process = materialization_process(store.clone(), artifacts);
        let tenant_id = TenantId::try_new(TENANT_ID).unwrap();

        let conflict = process
            .run_cycle(tenant_id.clone(), 50_000_000)
            .await
            .expect_err("lineage-mismatched evidence must fail closed");
        assert_eq!(
            conflict.code,
            "CUSTOMER_ENRICHMENT_SUGGESTION_EVIDENCE_CONFLICT"
        );
        assert!(!conflict.retryable);
        assert_eq!(suggestion_count(&admin).await, 0);
        assert_eq!(request_version(&admin, &fixture).await, 1);

        let failed_checkpoint = ProjectionStore::projection_checkpoint(
            &store,
            tenant_id.clone(),
            MATERIALIZATION_PROCESS_PROJECTION_ID.to_owned(),
        )
        .await
        .expect_err("terminal evidence conflict must fail the materialization checkpoint");
        assert_eq!(failed_checkpoint.code, "PROJECTION_CHECKPOINT_FAILED");
        assert!(!failed_checkpoint.retryable);
        let failed_reference = failed_checkpoint
            .internal_reference
            .as_deref()
            .expect("failed materialization checkpoint lineage exists");
        assert!(failed_reference.contains("failure_code=CUSTOMER_ENRICHMENT_SUGGESTION_EVIDENCE_CONFLICT"));

        let baseline = evidence_counts(&admin).await;
        drop(process);
        let evidence_calls = Arc::new(AtomicUsize::new(0));
        let executor_calls = Arc::new(AtomicUsize::new(0));
        let restarted = CustomerEnrichmentMaterializationProcessWorker::new(
            store,
            Arc::new(ForbiddenEvidenceSource {
                calls: evidence_calls.clone(),
            }),
            Arc::new(ForbiddenMaterializationExecutor {
                calls: executor_calls.clone(),
            }),
            ActorId::try_new(ACTOR_ID).unwrap(),
        )
        .expect("recompose materialization process after terminal evidence conflict");

        let replay = restarted
            .run_cycle(tenant_id, 60_000_000)
            .await
            .expect_err("failed materialization checkpoint must block restart");
        assert_eq!(replay.code, "PROJECTION_CHECKPOINT_FAILED");
        assert!(!replay.retryable);
        assert_eq!(evidence_calls.load(Ordering::SeqCst), 0);
        assert_eq!(executor_calls.load(Ordering::SeqCst), 0);
        assert_eq!(suggestion_count(&admin).await, 0);
        assert_eq!(request_version(&admin, &fixture).await, 1);
        assert_eq!(evidence_counts(&admin).await, baseline);
    }

    #[derive(Clone)]
    struct ForbiddenEvidenceSource {
        calls: Arc<AtomicUsize>,
    }

    impl ProviderSuggestionCandidateEvidenceSourcePort for ForbiddenEvidenceSource {
        fn load<'a>(
            &'a self,
            _request: ProviderSuggestionCandidateEvidenceRequest,
        ) -> PortFuture<'a, Result<wire::MaterializeSuggestionsRequest, SdkError>> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Box::pin(async {
                Err(SdkError::new(
                    "TEST_EVIDENCE_SOURCE_MUST_NOT_RUN",
                    crm_module_sdk::ErrorCategory::Internal,
                    false,
                    "The evidence source must not run after a failed checkpoint.",
                ))
            })
        }
    }

    #[derive(Clone)]
    struct ForbiddenMaterializationExecutor {
        calls: Arc<AtomicUsize>,
    }

    impl SuggestionMaterializationExecutorPort for ForbiddenMaterializationExecutor {
        fn execute<'a>(
            &'a self,
            _request: CapabilityRequest,
        ) -> PortFuture<'a, Result<crm_capability_runtime::CapabilityExecutionResult, SdkError>> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Box::pin(async {
                Err(SdkError::new(
                    "TEST_MATERIALIZATION_EXECUTOR_MUST_NOT_RUN",
                    crm_module_sdk::ErrorCategory::Internal,
                    false,
                    "The materialization executor must not run after a failed checkpoint.",
                ))
            })
        }
    }

    async fn upload_lineage_mismatched_candidate_evidence(
        artifacts: &PostgresImmutableFileArtifactStore,
        fixture: &Fixture,
    ) -> Result<(), SdkError> {
        let command = wire::MaterializeSuggestionsRequest {
            enrichment_request_ref: Some(wire::EnrichmentRequestRef {
                enrichment_request_id: "different-enrichment-request".to_owned(),
            }),
            provider_response_receipt_ref: Some(wire::ProviderResponseReceiptRef {
                provider_response_receipt_id: fixture.receipt.receipt_id().as_str().to_owned(),
            }),
            candidates: vec![candidate()],
        };
        let bytes = command.encode_to_vec();
        let digest: [u8; 32] = Sha256::digest(&bytes).into();
        let context = artifact_context();
        let file_id = FileId::try_new(FILE_ID).unwrap();
        artifacts
            .create(
                &context,
                CreateImmutableFileArtifact {
                    file_id: file_id.clone(),
                    owner_module_id: ModuleId::try_new(MODULE_ID).unwrap(),
                    media_type: PROVIDER_SUGGESTION_CANDIDATE_EVIDENCE_MEDIA_TYPE.to_owned(),
                    data_class: DataClass::Personal,
                    retention_policy_id: RetentionPolicyId::try_new(
                        "crm.customer_enrichment.provider_suggestion_evidence",
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
}
