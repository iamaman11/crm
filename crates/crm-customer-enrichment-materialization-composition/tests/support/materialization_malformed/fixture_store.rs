struct SeedRecord {
    suffix: &'static str,
    at_unix_ms: u64,
    reference: RecordRef,
    record_payload: TypedPayload,
    event_type: &'static str,
    event_payload: TypedPayload,
}

async fn seed_record(
    store: &PostgresDataStore,
    seed: SeedRecord,
) -> Result<(), Box<dyn std::error::Error>> {
    let request_hash = semantic_input_hash(&seed.event_payload);
    let at_unix_nanos = i64::try_from(seed.at_unix_ms * 1_000_000).unwrap();
    store
        .create_record(&RecordCreatePlan {
            context: seed_context(seed.suffix, at_unix_nanos),
            record: seed.reference.clone(),
            record_payload: seed.record_payload,
            event_id: format!("materialization-malformed-seed-event-{}", seed.suffix),
            event: DomainEvent {
                event_type: EventType::try_new(seed.event_type).unwrap(),
                aggregate: seed.reference,
                expected_aggregate_version: None,
                deduplication_key: format!("materialization-malformed-seed-{}", seed.suffix),
                payload: seed.event_payload,
            },
            idempotency: IdempotencyEvidence {
                scope: format!("{SEED_CAPABILITY}@1.0.0"),
                key: format!("materialization-malformed-seed-{}", seed.suffix),
                request_hash,
                expires_at_unix_nanos: 86_400_000_000_000 + at_unix_nanos,
            },
            audit: AuditIntent {
                audit_record_id: format!(
                    "materialization-malformed-seed-audit-{}",
                    seed.suffix
                ),
                canonicalization_profile: "crm.cjson/v1".to_owned(),
                canonical_envelope: format!("{{\"seed\":\"{}\"}}", seed.suffix).into_bytes(),
                occurred_at_unix_nanos: at_unix_nanos,
            },
        })
        .await?;
    Ok(())
}

fn receipt_persisted_payload(receipt: &ProviderResponseReceipt) -> Result<TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        PersistedPayloadContract {
            owner: MODULE_ID,
            schema_id: PROVIDER_RESPONSE_RECEIPT_STATE_SCHEMA_ID,
            schema_version: LIFECYCLE_STATE_SCHEMA_VERSION,
            descriptor_hash: provider_response_receipt_state_descriptor_hash(),
            maximum_size_bytes: PROVIDER_RESPONSE_RECEIPT_STATE_MAXIMUM_BYTES,
            retention_policy_id: LIFECYCLE_STATE_RETENTION_POLICY_ID,
        },
        DataClass::Personal,
        encode_provider_response_receipt_state(receipt)?,
    )
}

fn receipt_record_ref(receipt: &ProviderResponseReceipt) -> Result<RecordRef, SdkError> {
    support::record_ref(
        PROVIDER_RESPONSE_RECEIPT_RECORD_TYPE,
        receipt.receipt_id().as_str(),
        "customer_enrichment.provider_response_receipt_ref.provider_response_receipt_id",
    )
}

fn receipt_to_wire(fixture: &Fixture) -> wire::ProviderResponseReceipt {
    wire::ProviderResponseReceipt {
        provider_response_receipt_ref: Some(wire::ProviderResponseReceiptRef {
            provider_response_receipt_id: fixture.receipt.receipt_id().as_str().to_owned(),
        }),
        enrichment_request_ref: Some(wire::EnrichmentRequestRef {
            enrichment_request_id: fixture.request.request_id().as_str().to_owned(),
        }),
        provider_profile_version_ref: Some(wire::ProviderProfileVersionRef {
            provider_profile_version_id: fixture.profile.version_id().as_str().to_owned(),
        }),
        mapping_version_ref: Some(wire::MappingVersionRef {
            mapping_version_id: fixture.mapping.version_id().as_str().to_owned(),
        }),
        replay_key: "materialization-malformed-provider-replay-1".to_owned(),
        provider_correlation_id: Some(
            "materialization-malformed-provider-correlation-1".to_owned(),
        ),
        response_class: wire::ProviderResponseClass::Success as i32,
        canonical_response_digest: vec![84; 32],
        provider_observed_at_unix_ms: Some(20),
        retrieved_at_unix_ms: 30,
        metered_units: 1,
        protected_evidence_reference: Some(FILE_ID.to_owned()),
    }
}

fn seed_context(suffix: &str, started_at_unix_nanos: i64) -> ModuleExecutionContext {
    execution_context(
        &format!("materialization-malformed-seed-request-{suffix}"),
        SEED_CAPABILITY,
        &format!("materialization-malformed-seed-idempotency-{suffix}"),
        &format!("materialization-malformed-seed-tx-{suffix}"),
        started_at_unix_nanos,
    )
}

fn artifact_context() -> ModuleExecutionContext {
    execution_context(
        "materialization-malformed-artifact-request",
        "customer_enrichment.suggestion.evidence.store",
        "materialization-malformed-artifact-idempotency",
        "materialization-malformed-artifact-tx",
        50_000_000,
    )
}

fn execution_context(
    request_id: &str,
    capability_id: &str,
    idempotency_key: &str,
    transaction_id: &str,
    started_at_unix_nanos: i64,
) -> ModuleExecutionContext {
    ModuleExecutionContext {
        module_id: ModuleId::try_new(MODULE_ID).unwrap(),
        execution: ExecutionContext {
            tenant_id: TenantId::try_new(TENANT_ID).unwrap(),
            actor_id: ActorId::try_new(ACTOR_ID).unwrap(),
            request_id: RequestId::try_new(request_id).unwrap(),
            correlation_id: CorrelationId::try_new(format!("correlation-{request_id}")).unwrap(),
            causation_id: CausationId::try_new(format!("causation-{request_id}")).unwrap(),
            trace_id: TraceId::try_new(format!("trace-{request_id}")).unwrap(),
            capability_id: CapabilityId::try_new(capability_id).unwrap(),
            capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
            idempotency_key: IdempotencyKey::try_new(idempotency_key).unwrap(),
            business_transaction_id: BusinessTransactionId::try_new(transaction_id).unwrap(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
            request_started_at_unix_nanos: started_at_unix_nanos,
        },
    }
}

async fn evidence_counts(admin: &PgPool) -> EvidenceCounts {
    EvidenceCounts {
        records: sqlx::query_scalar::<_, i64>(
            "SELECT count(*)::bigint FROM crm.records WHERE tenant_id = $1",
        )
        .bind(TENANT_ID)
        .fetch_one(admin)
        .await
        .expect("query malformed-evidence record count"),
        events: sqlx::query_scalar::<_, i64>(
            "SELECT count(*)::bigint FROM crm.outbox_events WHERE tenant_id = $1",
        )
        .bind(TENANT_ID)
        .fetch_one(admin)
        .await
        .expect("query malformed-evidence event count"),
        audits: sqlx::query_scalar::<_, i64>(
            "SELECT count(*)::bigint FROM crm.audit_records WHERE tenant_id = $1",
        )
        .bind(TENANT_ID)
        .fetch_one(admin)
        .await
        .expect("query malformed-evidence audit count"),
        idempotency: sqlx::query_scalar::<_, i64>(
            "SELECT count(*)::bigint FROM crm.idempotency_records WHERE tenant_id = $1",
        )
        .bind(TENANT_ID)
        .fetch_one(admin)
        .await
        .expect("query malformed-evidence idempotency count"),
        transactions: sqlx::query_scalar::<_, i64>(
            "SELECT count(*)::bigint FROM crm.business_transactions WHERE tenant_id = $1",
        )
        .bind(TENANT_ID)
        .fetch_one(admin)
        .await
        .expect("query malformed-evidence transaction count"),
    }
}

async fn request_version(admin: &PgPool, fixture: &Fixture) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT version::bigint FROM crm.records WHERE tenant_id = $1 AND record_type = 'customer_enrichment.request' AND record_id = $2",
    )
    .bind(TENANT_ID)
    .bind(fixture.request.request_id().as_str())
    .fetch_one(admin)
    .await
    .expect("query malformed materialization request version")
}

async fn suggestion_count(admin: &PgPool) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*)::bigint FROM crm.records WHERE tenant_id = $1 AND record_type = 'customer_enrichment.suggestion'",
    )
    .bind(TENANT_ID)
    .fetch_one(admin)
    .await
    .expect("query malformed materialization suggestions")
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
    ) -> PortFuture<'a, Result<CapabilityExecutionResult, SdkError>> {
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
