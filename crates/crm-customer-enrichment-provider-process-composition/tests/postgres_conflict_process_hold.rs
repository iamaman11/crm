#![cfg(feature = "postgres-integration")]

use crm_capability_plan_support as support;
use crm_core_data::{AuditIntent, IdempotencyEvidence, PostgresDataStore, RecordCreatePlan};
use crm_core_events::ProjectionStore;
use crm_customer_enrichment::{
    EnrichmentRequest, EnrichmentRequestDraft, MappingDraft, MappingNormalization, MappingVersion,
    PartySnapshot, ProviderProfileDraft, ProviderProfileVersion, ProviderResponseConflictDraft,
    ProviderResponseReceiptId, RawPayloadPolicy, RequestPolicyEvidence, TargetField,
    TargetSnapshot,
};
use crm_customer_enrichment_capability_adapter::{
    ENRICHMENT_REQUEST_CREATED_EVENT_SCHEMA, ENRICHMENT_REQUEST_CREATED_EVENT_TYPE, MODULE_ID,
    enrichment_request_persisted_payload, enrichment_request_record_ref,
    enrichment_request_to_wire,
};
use crm_customer_enrichment_provider_process_composition::{
    CustomerEnrichmentProviderProcessWorker, PROVIDER_PROCESS_PROJECTION_ID,
    ProviderDispatchExecutorPort, ProviderDispatchSourceDisposition, ProviderDispatchSourcePort,
    ProviderDispatchSourceSnapshot,
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
const SEED_CAPABILITY: &str = "customer_enrichment.response.record";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unresolved_conflict_is_persisted_once_and_holds_checkpoint_across_restart() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping PostgreSQL provider conflict hold because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let store = PostgresDataStore::connect(&database_url, 6)
        .await
        .expect("connect provider conflict process store");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect provider conflict process evidence reader");
    let fixture = fixture();
    seed_request(&store, &fixture.request)
        .await
        .expect("seed request-created evidence");

    let source_calls = Arc::new(AtomicUsize::new(0));
    let executor_calls = Arc::new(AtomicUsize::new(0));
    let source = Arc::new(StaticSource {
        snapshot: fixture.source.clone(),
        calls: source_calls.clone(),
    });
    let executor = Arc::new(ConflictingExecutor {
        draft: fixture.conflict.clone(),
        calls: executor_calls.clone(),
    });
    let tenant_id = TenantId::try_new(TENANT_ID).unwrap();
    let actor_id = ActorId::try_new(ACTOR_ID).unwrap();

    let process = CustomerEnrichmentProviderProcessWorker::new(
        store.clone(),
        source.clone(),
        executor.clone(),
        actor_id.clone(),
    )
    .expect("compose provider conflict process");
    let first = process
        .run_cycle(tenant_id.clone(), 50_000_000)
        .await
        .expect_err("unresolved conflict must hold the created-event checkpoint");
    assert_eq!(
        first.code,
        "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_UNRESOLVED"
    );
    assert_eq!(first.category, ErrorCategory::Conflict);
    assert!(first.retryable);
    assert_eq!(source_calls.load(Ordering::SeqCst), 1);
    assert_eq!(executor_calls.load(Ordering::SeqCst), 1);
    assert!(
        ProjectionStore::projection_checkpoint(
            &store,
            tenant_id.clone(),
            PROVIDER_PROCESS_PROJECTION_ID.to_owned(),
        )
        .await
        .expect("read held provider checkpoint")
        .is_none()
    );
    assert_eq!(conflict_count(&admin).await, 1);
    assert_eq!(conflict_relationship_count(&admin).await, 1);
    let baseline = evidence_counts(&admin).await;

    drop(process);
    let restarted =
        CustomerEnrichmentProviderProcessWorker::new(store.clone(), source, executor, actor_id)
            .expect("recompose provider conflict process");
    let replay = restarted
        .run_cycle(tenant_id.clone(), 60_000_000)
        .await
        .expect_err("persisted unresolved conflict must hold restart before provider I/O");
    assert_eq!(
        replay.code,
        "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_UNRESOLVED"
    );
    assert!(replay.retryable);
    assert_eq!(source_calls.load(Ordering::SeqCst), 1);
    assert_eq!(executor_calls.load(Ordering::SeqCst), 1);
    assert!(
        ProjectionStore::projection_checkpoint(
            &store,
            tenant_id,
            PROVIDER_PROCESS_PROJECTION_ID.to_owned(),
        )
        .await
        .expect("read restart-held provider checkpoint")
        .is_none()
    );
    assert_eq!(conflict_count(&admin).await, 1);
    assert_eq!(conflict_relationship_count(&admin).await, 1);
    assert_eq!(evidence_counts(&admin).await, baseline);
}

#[derive(Clone)]
struct StaticSource {
    snapshot: ProviderDispatchSourceSnapshot,
    calls: Arc<AtomicUsize>,
}

impl ProviderDispatchSourcePort for StaticSource {
    fn load<'a>(
        &'a self,
        _tenant_id: TenantId,
        _request_id: RecordId,
        _worker_actor_id: ActorId,
        _now_unix_ms: u64,
    ) -> PortFuture<'a, Result<ProviderDispatchSourceDisposition, SdkError>> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(ProviderDispatchSourceDisposition::Ready(Box::new(
                self.snapshot.clone(),
            )))
        })
    }
}

#[derive(Clone)]
struct ConflictingExecutor {
    draft: ProviderResponseConflictDraft,
    calls: Arc<AtomicUsize>,
}

impl ProviderDispatchExecutorPort for ConflictingExecutor {
    fn execute<'a>(
        &'a self,
        _work_item: ProviderDispatchWorkItem,
    ) -> PortFuture<'a, Result<ProviderDispatchExecution, SdkError>> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(ProviderDispatchExecution::Conflicting(self.draft.clone()))
        })
    }
}

struct Fixture {
    request: EnrichmentRequest,
    source: ProviderDispatchSourceSnapshot,
    conflict: ProviderResponseConflictDraft,
}

fn fixture() -> Fixture {
    let profile = ProviderProfileVersion::publish(ProviderProfileDraft {
        provider_key: "company_registry_conflict_hold".to_owned(),
        adapter_kind: "registry_http_v1".to_owned(),
        adapter_contract_version: "1.0.0".to_owned(),
        supported_target_fields: vec![TargetField::PartyDisplayName],
        purpose_codes: vec!["customer_profile_enrichment".to_owned()],
        license_id: "Registry conflict hold licence".to_owned(),
        permitted_use_class: "customer_master_review".to_owned(),
        residency_region: "eu".to_owned(),
        retention_days: 30,
        raw_payload_policy: RawPayloadPolicy::DigestOnly,
        credential_handle_aliases: vec!["registry_conflict_hold".to_owned()],
        effective_at_unix_ms: 1,
        expires_at_unix_ms: Some(5_000),
    })
    .unwrap();
    let mapping = MappingVersion::publish(MappingDraft {
        mapping_key: "party_display_name_conflict_hold".to_owned(),
        provider_profile_version_id: profile.version_id().clone(),
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
        idempotency_key: IdempotencyKey::try_new("provider-conflict-hold-domain-request").unwrap(),
        target: TargetSnapshot::try_new(
            "party-provider-conflict-hold-1",
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
            "provider-conflict-hold-policy-v1",
        )
        .unwrap(),
        created_at_unix_ms: 10,
        deadline_at_unix_ms: 1_000,
        expires_at_unix_ms: 2_000,
    })
    .unwrap();
    let party_snapshot = PartySnapshot {
        party_id: RecordId::try_new("party-provider-conflict-hold-1").unwrap(),
        display_name: "Conflict Hold Company".to_owned(),
        resource_version: 7,
        observed_at_unix_ms: 20,
    };
    let conflict = ProviderResponseConflictDraft {
        tenant_id: TenantId::try_new(TENANT_ID).unwrap(),
        request_id: request.request_id().clone(),
        retry_generation: 0,
        first_receipt_id: receipt_id(2),
        conflicting_semantic_fingerprint: [3; 32],
        detected_at_unix_ms: 50,
    };
    Fixture {
        source: ProviderDispatchSourceSnapshot {
            request: request.clone(),
            provider_profile: profile,
            party_snapshot,
        },
        request,
        conflict,
    }
}

fn receipt_id(byte: u8) -> ProviderResponseReceiptId {
    serde_json::from_str(&format!(
        "\"enrichment-response-{}\"",
        format!("{byte:02x}").repeat(32)
    ))
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
            event_id: "provider-conflict-hold-created-event".to_owned(),
            event: DomainEvent {
                event_type: EventType::try_new(ENRICHMENT_REQUEST_CREATED_EVENT_TYPE)?,
                aggregate: record,
                expected_aggregate_version: None,
                deduplication_key: "provider-conflict-hold-created".to_owned(),
                payload: event_payload,
            },
            idempotency: IdempotencyEvidence {
                scope: format!("{SEED_CAPABILITY}@1.0.0"),
                key: "provider-conflict-hold-seed".to_owned(),
                request_hash: [41; 32],
                expires_at_unix_nanos: 86_400_000_000_000,
            },
            audit: AuditIntent {
                audit_record_id: "provider-conflict-hold-seed-audit".to_owned(),
                canonicalization_profile: "crm.cjson/v1".to_owned(),
                canonical_envelope: b"{\"operation\":\"seed_provider_conflict_hold\"}".to_vec(),
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
            request_id: RequestId::try_new("provider-conflict-hold-seed-request").unwrap(),
            correlation_id: CorrelationId::try_new("provider-conflict-hold-correlation").unwrap(),
            causation_id: CausationId::try_new("provider-conflict-hold-causation").unwrap(),
            trace_id: TraceId::try_new("provider-conflict-hold-trace").unwrap(),
            capability_id: CapabilityId::try_new(SEED_CAPABILITY).unwrap(),
            capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
            idempotency_key: IdempotencyKey::try_new("provider-conflict-hold-seed").unwrap(),
            business_transaction_id: BusinessTransactionId::try_new(
                "provider-conflict-hold-seed-tx",
            )
            .unwrap(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
            request_started_at_unix_nanos: 10_000_000,
        },
    }
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

async fn conflict_count(pool: &PgPool) -> i64 {
    scalar(
        pool,
        "SELECT count(*)::bigint FROM crm.records WHERE tenant_id = 'tenant-a' AND record_type = 'customer_enrichment.provider_response_conflict'",
    )
    .await
}

async fn conflict_relationship_count(pool: &PgPool) -> i64 {
    scalar(
        pool,
        "SELECT count(*)::bigint FROM crm.relationships WHERE tenant_id = 'tenant-a' AND relationship_type = 'customer_enrichment.request.provider_response_conflict'",
    )
    .await
}

async fn scalar(pool: &PgPool, query: &'static str) -> i64 {
    sqlx::query_scalar::<_, i64>(query)
        .fetch_one(pool)
        .await
        .expect("read provider conflict process evidence")
}
