#![cfg(feature = "postgres-integration")]

use crm_capability_plan_support as support;
use crm_core_data::{AuditIntent, IdempotencyEvidence, PostgresDataStore, RecordCreatePlan};
use crm_core_events::ProjectionStore;
use crm_customer_enrichment::{
    EnrichmentRequest, EnrichmentRequestDraft, EnrichmentRequestStatus, MappingDraft,
    MappingNormalization, MappingVersion, ProviderProfileDraft, ProviderProfileVersion,
    ProviderResponseConflictDecision, ProviderResponseConflictDraft,
    ProviderResponseConflictResolutionPolicyDecision, ProviderResponseConflictResolutionPolicyPort,
    ProviderResponseConflictResolutionPolicyRequest, ProviderResponseReceiptId, RawPayloadPolicy,
    RequestPolicyEvidence, TargetField, TargetSnapshot,
};
use crm_customer_enrichment_capability_adapter::{
    ENRICHMENT_REQUEST_CREATED_EVENT_SCHEMA, ENRICHMENT_REQUEST_CREATED_EVENT_TYPE, MODULE_ID,
    enrichment_request_from_snapshot, enrichment_request_persisted_payload,
    enrichment_request_record_ref, enrichment_request_to_wire,
};
use crm_customer_enrichment_provider_process_composition::{
    CustomerEnrichmentProviderProcessWorker, PROVIDER_PROCESS_PROJECTION_ID,
    PostgresProviderResponseConflictRejectExecutor,
    PostgresProviderResponseConflictResolutionExecutor, PostgresProviderResponseConflictStore,
    ProviderDispatchExecutorPort, ProviderDispatchSourceDisposition, ProviderDispatchSourcePort,
    ProviderResponseConflictPersistenceLineage, ProviderResponseConflictRejectionLineage,
    ProviderResponseConflictResolutionCommand,
};
use crm_customer_enrichment_worker_composition::{
    ProviderDispatchExecution, ProviderDispatchWorkItem,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, CorrelationId,
    DataClass, DomainEvent, ErrorCategory, EventType, ExecutionContext, IdempotencyKey,
    ModuleExecutionContext, ModuleId, PortFuture, RecordId, RequestId, SchemaVersion, SdkError,
    TenantId, TraceId,
};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;
use sqlx::PgPool;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

const TENANT_ID: &str = "tenant-a";
const ACTOR_ID: &str = "actor-a";
const SAFE_REASON: &str = "provider-response-conflict-rejected";
const CORRELATION_ID: &str = "provider-conflict-reject-correlation";
const TRACE_ID: &str = "provider-conflict-reject-trace";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn approved_reject_terminalizes_once_then_resumes_checkpoint_without_provider_io() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping PostgreSQL conflict reject process because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let store = PostgresDataStore::connect(&database_url, 6)
        .await
        .expect("connect provider conflict reject store");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect provider conflict reject evidence reader");
    let request = canonical_request();
    seed_request(&store, &request)
        .await
        .expect("seed reject request-created evidence");

    let conflict_store = PostgresProviderResponseConflictStore::new(store.clone());
    let recorded = conflict_store
        .record(
            ProviderResponseConflictDraft {
                tenant_id: TenantId::try_new(TENANT_ID).unwrap(),
                request_id: request.request_id().clone(),
                retry_generation: request.retry_generation(),
                first_receipt_id: receipt_id(7),
                conflicting_semantic_fingerprint: [9; 32],
                detected_at_unix_ms: 50,
            },
            ProviderResponseConflictPersistenceLineage {
                actor_id: ActorId::try_new(ACTOR_ID).unwrap(),
                correlation_id: CorrelationId::try_new(CORRELATION_ID).unwrap(),
                causation_id: CausationId::try_new("provider-conflict-reject-created-event")
                    .unwrap(),
                trace_id: TraceId::try_new(TRACE_ID).unwrap(),
            },
        )
        .await
        .expect("persist reject conflict");
    let resolver = PostgresProviderResponseConflictResolutionExecutor::new(
        store.clone(),
        Arc::new(AllowRejectPolicy),
    );
    let resolved = resolver
        .execute(ProviderResponseConflictResolutionCommand {
            tenant_id: TenantId::try_new(TENANT_ID).unwrap(),
            conflict_id: RecordId::try_new(recorded.conflict.conflict_id().as_str().to_owned())
                .unwrap(),
            actor_id: ActorId::try_new(ACTOR_ID).unwrap(),
            decision: ProviderResponseConflictDecision::RejectRequest,
            safe_reason_code: SAFE_REASON.to_owned(),
            approval_evidence_reference: "approval/provider-conflict/reject-request".to_owned(),
            causation_id: CausationId::try_new("provider-conflict-reject-command").unwrap(),
            correlation_id: CorrelationId::try_new(CORRELATION_ID).unwrap(),
            trace_id: TraceId::try_new(TRACE_ID).unwrap(),
            resolved_at_unix_ms: 70,
        })
        .await
        .expect("persist governed reject resolution");
    assert!(!resolved.replayed);
    assert_eq!(
        resolved
            .conflict
            .resolution()
            .expect("resolution exists")
            .decision(),
        ProviderResponseConflictDecision::RejectRequest
    );

    let before_terminal = evidence_counts(&admin).await;
    let reject_executor = PostgresProviderResponseConflictRejectExecutor::new(store.clone());
    let terminal = reject_executor
        .execute(
            &resolved.conflict,
            ProviderResponseConflictRejectionLineage {
                actor_id: ActorId::try_new(ACTOR_ID).unwrap(),
                correlation_id: CorrelationId::try_new(CORRELATION_ID).unwrap(),
                trace_id: TraceId::try_new(TRACE_ID).unwrap(),
            },
        )
        .await
        .expect("atomically terminalize rejected request");
    assert!(!terminal.replayed);
    assert_eq!(
        terminal.request.status(),
        EnrichmentRequestStatus::FailedTerminal
    );
    assert_eq!(terminal.request.last_safe_failure_code(), Some(SAFE_REASON));
    let after_terminal = evidence_counts(&admin).await;
    assert_eq!(after_terminal.records, before_terminal.records);
    assert_eq!(after_terminal.relationships, before_terminal.relationships);
    assert_eq!(after_terminal.events, before_terminal.events + 1);
    assert_eq!(after_terminal.audits, before_terminal.audits + 1);
    assert_eq!(after_terminal.idempotency, before_terminal.idempotency + 1);
    assert_eq!(
        after_terminal.transactions,
        before_terminal.transactions + 1
    );
    assert_eq!(request_record_version(&admin).await, 2);
    assert_eq!(status_changed_events(&admin).await, 1);
    assert_eq!(suggestion_records(&admin).await, 0);

    let source_calls = Arc::new(AtomicUsize::new(0));
    let executor_calls = Arc::new(AtomicUsize::new(0));
    let worker = CustomerEnrichmentProviderProcessWorker::new(
        store.clone(),
        Arc::new(ForbiddenSource {
            calls: source_calls.clone(),
        }),
        Arc::new(ForbiddenExecutor {
            calls: executor_calls.clone(),
        }),
        ActorId::try_new(ACTOR_ID).unwrap(),
    )
    .expect("compose reject recovery process");
    let resumed = worker
        .run_cycle(TenantId::try_new(TENANT_ID).unwrap(), 80_000_000)
        .await
        .expect("replayed terminal rejection must advance held checkpoint");
    assert_eq!(resumed.created_events, 1);
    assert_eq!(resumed.rejected_requests, 1);
    assert_eq!(resumed.rejection_replays, 1);
    assert_eq!(resumed.dispatched, 0);
    assert_eq!(source_calls.load(Ordering::SeqCst), 0);
    assert_eq!(executor_calls.load(Ordering::SeqCst), 0);
    assert!(
        ProjectionStore::projection_checkpoint(
            &store,
            TenantId::try_new(TENANT_ID).unwrap(),
            PROVIDER_PROCESS_PROJECTION_ID.to_owned(),
        )
        .await
        .expect("read reject-resumed checkpoint")
        .is_some()
    );
    assert_eq!(evidence_counts(&admin).await, after_terminal);

    let snapshot = store
        .get_record(
            &execution_context(),
            &enrichment_request_record_ref(&request).unwrap(),
        )
        .await
        .expect("reload terminal request")
        .expect("terminal request exists");
    let persisted = enrichment_request_from_snapshot(&snapshot).expect("decode terminal request");
    assert_eq!(persisted.status(), EnrichmentRequestStatus::FailedTerminal);
    assert_eq!(persisted.last_safe_failure_code(), Some(SAFE_REASON));
    assert_eq!(snapshot.version, 2);

    let no_op = worker
        .run_cycle(TenantId::try_new(TENANT_ID).unwrap(), 90_000_000)
        .await
        .expect("post-checkpoint reject replay must be a no-op");
    assert_eq!(no_op.created_events, 0);
    assert_eq!(no_op.rejected_requests, 0);
    assert_eq!(source_calls.load(Ordering::SeqCst), 0);
    assert_eq!(executor_calls.load(Ordering::SeqCst), 0);
    assert_eq!(evidence_counts(&admin).await, after_terminal);
}

#[derive(Clone)]
struct AllowRejectPolicy;

impl ProviderResponseConflictResolutionPolicyPort for AllowRejectPolicy {
    fn evaluate<'a>(
        &'a self,
        request: ProviderResponseConflictResolutionPolicyRequest,
    ) -> PortFuture<'a, Result<ProviderResponseConflictResolutionPolicyDecision, SdkError>> {
        Box::pin(async move {
            assert_eq!(
                request.decision,
                ProviderResponseConflictDecision::RejectRequest
            );
            assert_eq!(request.safe_reason_code, SAFE_REASON);
            Ok(ProviderResponseConflictResolutionPolicyDecision::Allowed {
                policy_version: "provider-conflict-policy-v1".to_owned(),
            })
        })
    }
}

#[derive(Clone)]
struct ForbiddenSource {
    calls: Arc<AtomicUsize>,
}

impl ProviderDispatchSourcePort for ForbiddenSource {
    fn load<'a>(
        &'a self,
        _tenant_id: TenantId,
        _request_id: RecordId,
        _worker_actor_id: ActorId,
        _now_unix_ms: u64,
    ) -> PortFuture<'a, Result<ProviderDispatchSourceDisposition, SdkError>> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Err(SdkError::new(
                "TEST_PROVIDER_SOURCE_MUST_NOT_RUN",
                ErrorCategory::Internal,
                false,
                "Provider source must not run after approved rejection.",
            ))
        })
    }
}

#[derive(Clone)]
struct ForbiddenExecutor {
    calls: Arc<AtomicUsize>,
}

impl ProviderDispatchExecutorPort for ForbiddenExecutor {
    fn execute<'a>(
        &'a self,
        _work_item: ProviderDispatchWorkItem,
    ) -> PortFuture<'a, Result<ProviderDispatchExecution, SdkError>> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Err(SdkError::new(
                "TEST_PROVIDER_EXECUTOR_MUST_NOT_RUN",
                ErrorCategory::Internal,
                false,
                "Provider executor must not run after approved rejection.",
            ))
        })
    }
}

fn canonical_request() -> EnrichmentRequest {
    let profile = ProviderProfileVersion::publish(ProviderProfileDraft {
        provider_key: "company_registry_conflict_reject".to_owned(),
        adapter_kind: "registry_http_v1".to_owned(),
        adapter_contract_version: "1.0.0".to_owned(),
        supported_target_fields: vec![TargetField::PartyDisplayName],
        purpose_codes: vec!["customer_profile_enrichment".to_owned()],
        license_id: "Registry conflict reject licence".to_owned(),
        permitted_use_class: "customer_master_review".to_owned(),
        residency_region: "eu".to_owned(),
        retention_days: 30,
        raw_payload_policy: RawPayloadPolicy::DigestOnly,
        credential_handle_aliases: vec!["registry_conflict_reject".to_owned()],
        effective_at_unix_ms: 1,
        expires_at_unix_ms: Some(5_000),
    })
    .unwrap();
    let mapping = MappingVersion::publish(MappingDraft {
        mapping_key: "party_display_name_conflict_reject".to_owned(),
        provider_profile_version_id: profile.version_id().clone(),
        provider_response_field_path: "organization.legal_name".to_owned(),
        target_field: TargetField::PartyDisplayName,
        normalization: MappingNormalization::CanonicalPartyDisplayNameV1,
        maximum_suggestions_per_response: 1,
        confidence_required: true,
    })
    .unwrap();
    EnrichmentRequest::create(EnrichmentRequestDraft {
        tenant_id: TenantId::try_new(TENANT_ID).unwrap(),
        requested_by: ActorId::try_new(ACTOR_ID).unwrap(),
        idempotency_key: IdempotencyKey::try_new("provider-conflict-reject-domain-request")
            .unwrap(),
        target: TargetSnapshot::try_new(
            "party-provider-conflict-reject-1",
            7,
            TargetField::PartyDisplayName,
        )
        .unwrap(),
        provider_profile_version_id: profile.version_id().clone(),
        mapping_version_id: mapping.version_id().clone(),
        requested_fields: vec![TargetField::PartyDisplayName],
        policy_evidence: RequestPolicyEvidence::try_new(
            "customer_profile_enrichment",
            "legitimate_interest",
            None,
            "provider-conflict-reject-policy-v1",
        )
        .unwrap(),
        created_at_unix_ms: 10,
        deadline_at_unix_ms: 1_000,
        expires_at_unix_ms: 2_000,
    })
    .unwrap()
}

async fn seed_request(
    store: &PostgresDataStore,
    request: &EnrichmentRequest,
) -> Result<(), Box<dyn std::error::Error>> {
    let record = enrichment_request_record_ref(request)?;
    let event_payload = support::protobuf_payload(
        MODULE_ID,
        ENRICHMENT_REQUEST_CREATED_EVENT_SCHEMA,
        DataClass::Personal,
        &wire::EnrichmentRequestCreatedEvent {
            enrichment_request: Some(enrichment_request_to_wire(request)?),
        },
    )?;
    store
        .create_record(&RecordCreatePlan {
            context: execution_context(),
            record: record.clone(),
            record_payload: enrichment_request_persisted_payload(request)?,
            event_id: "provider-conflict-reject-created-event".to_owned(),
            event: DomainEvent {
                event_type: EventType::try_new(ENRICHMENT_REQUEST_CREATED_EVENT_TYPE)?,
                aggregate: record,
                expected_aggregate_version: None,
                deduplication_key: "provider-conflict-reject-created".to_owned(),
                payload: event_payload,
            },
            idempotency: IdempotencyEvidence {
                scope: "customer_enrichment.response.record@1.0.0".to_owned(),
                key: "provider-conflict-reject-seed".to_owned(),
                request_hash: [61; 32],
                expires_at_unix_nanos: 86_400_000_000_000,
            },
            audit: AuditIntent {
                audit_record_id: "provider-conflict-reject-seed-audit".to_owned(),
                canonicalization_profile: "crm.cjson/v1".to_owned(),
                canonical_envelope: b"{\"operation\":\"seed_provider_conflict_reject\"}".to_vec(),
                occurred_at_unix_nanos: 10_000_000,
            },
        })
        .await?;
    Ok(())
}

fn execution_context() -> ModuleExecutionContext {
    ModuleExecutionContext {
        module_id: ModuleId::try_new(MODULE_ID).unwrap(),
        execution: ExecutionContext {
            tenant_id: TenantId::try_new(TENANT_ID).unwrap(),
            actor_id: ActorId::try_new(ACTOR_ID).unwrap(),
            request_id: RequestId::try_new("provider-conflict-reject-seed-request").unwrap(),
            correlation_id: CorrelationId::try_new(CORRELATION_ID).unwrap(),
            causation_id: CausationId::try_new("provider-conflict-reject-seed-causation").unwrap(),
            trace_id: TraceId::try_new(TRACE_ID).unwrap(),
            capability_id: CapabilityId::try_new("customer_enrichment.response.record").unwrap(),
            capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
            idempotency_key: IdempotencyKey::try_new("provider-conflict-reject-seed").unwrap(),
            business_transaction_id: BusinessTransactionId::try_new(
                "provider-conflict-reject-seed-tx",
            )
            .unwrap(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
            request_started_at_unix_nanos: 10_000_000,
        },
    }
}

fn receipt_id(byte: u8) -> ProviderResponseReceiptId {
    serde_json::from_str(&format!(
        "\"enrichment-response-{}\"",
        format!("{byte:02x}").repeat(32)
    ))
    .unwrap()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EvidenceCounts {
    records: i64,
    relationships: i64,
    events: i64,
    audits: i64,
    idempotency: i64,
    transactions: i64,
}

async fn evidence_counts(pool: &PgPool) -> EvidenceCounts {
    EvidenceCounts {
        records: scalar(
            pool,
            "SELECT count(*)::bigint FROM crm.records WHERE tenant_id = 'tenant-a'",
        )
        .await,
        relationships: scalar(
            pool,
            "SELECT count(*)::bigint FROM crm.relationships WHERE tenant_id = 'tenant-a'",
        )
        .await,
        events: scalar(
            pool,
            "SELECT count(*)::bigint FROM crm.outbox_events WHERE tenant_id = 'tenant-a'",
        )
        .await,
        audits: scalar(
            pool,
            "SELECT count(*)::bigint FROM crm.audit_records WHERE tenant_id = 'tenant-a'",
        )
        .await,
        idempotency: scalar(
            pool,
            "SELECT count(*)::bigint FROM crm.idempotency_records WHERE tenant_id = 'tenant-a'",
        )
        .await,
        transactions: scalar(
            pool,
            "SELECT count(*)::bigint FROM crm.business_transactions WHERE tenant_id = 'tenant-a'",
        )
        .await,
    }
}

async fn request_record_version(pool: &PgPool) -> i64 {
    scalar(
        pool,
        "SELECT version::bigint FROM crm.records WHERE tenant_id = 'tenant-a' AND record_type = 'customer_enrichment.request'",
    )
    .await
}

async fn status_changed_events(pool: &PgPool) -> i64 {
    scalar(
        pool,
        "SELECT count(*)::bigint FROM crm.outbox_events WHERE tenant_id = 'tenant-a' AND event_type = 'customer_enrichment.request.status_changed'",
    )
    .await
}

async fn suggestion_records(pool: &PgPool) -> i64 {
    scalar(
        pool,
        "SELECT count(*)::bigint FROM crm.records WHERE tenant_id = 'tenant-a' AND record_type = 'customer_enrichment.suggestion'",
    )
    .await
}

async fn scalar(pool: &PgPool, query: &'static str) -> i64 {
    sqlx::query_scalar::<_, i64>(query)
        .fetch_one(pool)
        .await
        .expect("read provider conflict reject evidence")
}
