use crm_capability_ingress::semantic_input_hash;
use crm_capability_plan_support as support;
use crm_capability_runtime::CapabilityRequest;
use crm_core_data::{AuditIntent, IdempotencyEvidence, PostgresDataStore, RecordCreatePlan};
use crm_customer_enrichment::{
    EnrichmentRequest, EnrichmentRequestDraft, EnrichmentRequestStatus, MappingDraft,
    MappingNormalization, MappingVersion, PartySnapshot, ProviderDispatchExpectation,
    ProviderDispatchPort, ProviderDispatchRequest, ProviderProfileDraft, ProviderProfileVersion,
    ProviderResponseClass, RawPayloadPolicy, RequestPolicyEvidence, SanitizedProviderResponse,
    TargetField, TargetSnapshot, prepare_provider_dispatch_attempt,
};
use crm_customer_enrichment_capability_adapter::{
    DISPATCH_ENRICHMENT_REQUEST_REQUEST_SCHEMA, ENRICHMENT_REQUEST_CREATED_EVENT_SCHEMA,
    ENRICHMENT_REQUEST_CREATED_EVENT_TYPE, MODULE_ID, enrichment_request_persisted_payload,
    enrichment_request_record_ref, enrichment_request_to_wire,
    request_dispatch_capability_definition,
};
use crm_customer_enrichment_provider_registry::{
    ExactProviderAdapterRegistry, ProviderAdapterRegistration,
};
use crm_customer_enrichment_worker_composition::{
    CustomerEnrichmentProviderWorker, ProviderDispatchWorkItem,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, CorrelationId,
    DataClass, DomainEvent, EventType, ExecutionContext, IdempotencyKey, ModuleExecutionContext,
    ModuleId, PortFuture, RecordId, RequestId, SchemaVersion, SdkError, TenantId, TraceId,
};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;
use sqlx::PgPool;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

const TENANT_ID: &str = "tenant-a";
const ACTOR_ID: &str = "actor-a";
const SEED_CAPABILITY: &str = "customer_enrichment.request.seed";

#[derive(Clone)]
struct ReplaySafeProvider {
    expected_key: String,
    calls: Arc<AtomicUsize>,
}

impl ProviderDispatchPort for ReplaySafeProvider {
    fn dispatch<'a>(
        &'a self,
        request: ProviderDispatchRequest,
    ) -> PortFuture<'a, Result<SanitizedProviderResponse, SdkError>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let expected_key = self.expected_key.clone();
        Box::pin(async move {
            if request.provider_idempotency_key != expected_key {
                return Err(SdkError::new(
                    "TEST_PROVIDER_REPLAY_KEY_CHANGED",
                    crm_module_sdk::ErrorCategory::Dependency,
                    false,
                    "The provider replay key changed.",
                ));
            }
            Ok(SanitizedProviderResponse {
                replay_key: request.provider_idempotency_key,
                provider_correlation_id: Some("provider-correlation-process-1".to_owned()),
                response_class: ProviderResponseClass::Success,
                canonical_response_digest: [91; 32],
                provider_observed_at_unix_ms: Some(30),
                retrieved_at_unix_ms: 31,
                metered_units: 3,
                protected_evidence_reference: None,
                safe_provider_code: Some("success".to_owned()),
            })
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn postgres_worker_commits_and_replays_without_duplicates() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping PostgreSQL worker process because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let store = PostgresDataStore::connect(&database_url, 6)
        .await
        .expect("connect worker store");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect worker evidence reader");

    let fixture = fixture();
    seed_request(&store, &fixture.created_request)
        .await
        .expect("seed canonical Created request");

    let calls = Arc::new(AtomicUsize::new(0));
    let registry = ExactProviderAdapterRegistry::try_new([ProviderAdapterRegistration::enabled(
        fixture.provider_request.adapter_coordinate.clone(),
        ReplaySafeProvider {
            expected_key: fixture.provider_request.provider_idempotency_key.clone(),
            calls: calls.clone(),
        },
    )])
    .expect("build exact provider registry");
    let worker = CustomerEnrichmentProviderWorker::postgres(store.clone(), Arc::new(registry))
        .expect("compose PostgreSQL worker");

    let first = worker
        .execute(fixture.work_item.clone())
        .await
        .expect("commit first provider attempt");
    assert!(!first.dispatch_replayed);
    assert!(!first.response_replayed);

    let second = worker
        .execute(fixture.work_item.clone())
        .await
        .expect("replay provider attempt safely");
    assert!(second.dispatch_replayed);
    assert!(second.response_replayed);
    assert_eq!(calls.load(Ordering::SeqCst), 2);

    let request_snapshot = store
        .get_record(
            &read_context(),
            &enrichment_request_record_ref(&fixture.created_request).unwrap(),
        )
        .await
        .expect("read final request")
        .expect("request remains present");
    assert_eq!(request_snapshot.version, 3);

    assert_eq!(
        scalar(
            &admin,
            "SELECT count(*)::bigint FROM crm.records WHERE tenant_id = 'tenant-a' AND owner_module_id = 'crm.customer-enrichment'",
        )
        .await,
        5
    );
    assert_eq!(
        scalar(
            &admin,
            "SELECT count(*)::bigint FROM crm.records WHERE tenant_id = 'tenant-a' AND record_type = 'customer_enrichment.request'",
        )
        .await,
        1
    );
    assert_eq!(
        scalar(
            &admin,
            "SELECT count(*)::bigint FROM crm.records WHERE tenant_id = 'tenant-a' AND record_type = 'customer_enrichment.provider_response_receipt'",
        )
        .await,
        1
    );
    assert_eq!(
        scalar(
            &admin,
            "SELECT count(*)::bigint FROM crm.records WHERE tenant_id = 'tenant-a' AND record_type = 'customer_enrichment.provider_usage_entry'",
        )
        .await,
        3
    );
    assert_eq!(
        scalar(
            &admin,
            "SELECT count(*)::bigint FROM crm.outbox_events WHERE tenant_id = 'tenant-a' AND event_type LIKE 'customer_enrichment.%'",
        )
        .await,
        7
    );
    assert_eq!(
        scalar(
            &admin,
            "SELECT count(*)::bigint FROM crm.audit_records WHERE tenant_id = 'tenant-a' AND capability_id LIKE 'customer_enrichment.%'",
        )
        .await,
        7
    );
    assert_eq!(
        scalar(
            &admin,
            "SELECT count(*)::bigint FROM crm.idempotency_records WHERE tenant_id = 'tenant-a' AND (idempotency_scope = 'customer_enrichment.request.seed@1.0.0' OR idempotency_scope LIKE 'capability:customer_enrichment.%:1.0.0')",
        )
        .await,
        3
    );
    assert_eq!(
        scalar(
            &admin,
            "SELECT count(*)::bigint FROM crm.business_transactions WHERE tenant_id = 'tenant-a' AND capability_id LIKE 'customer_enrichment.%'",
        )
        .await,
        3
    );
}

struct Fixture {
    created_request: EnrichmentRequest,
    provider_request: ProviderDispatchRequest,
    work_item: ProviderDispatchWorkItem,
}

fn fixture() -> Fixture {
    let profile = ProviderProfileVersion::publish(ProviderProfileDraft {
        provider_key: "company_registry".to_owned(),
        adapter_kind: "registry_http_v1".to_owned(),
        adapter_contract_version: "1.0.0".to_owned(),
        supported_target_fields: vec![TargetField::PartyDisplayName],
        purpose_codes: vec!["customer_profile_enrichment".to_owned()],
        license_id: "Registry process licence".to_owned(),
        permitted_use_class: "customer_master_review".to_owned(),
        residency_region: "eu".to_owned(),
        retention_days: 30,
        raw_payload_policy: RawPayloadPolicy::DigestOnly,
        credential_handle_aliases: vec!["registry_primary".to_owned()],
        effective_at_unix_ms: 1,
        expires_at_unix_ms: Some(5_000),
    })
    .unwrap();
    let mapping = MappingVersion::publish(MappingDraft {
        mapping_key: "party_display_name".to_owned(),
        provider_profile_version_id: profile.version_id().clone(),
        provider_response_field_path: "organization.legal_name".to_owned(),
        target_field: TargetField::PartyDisplayName,
        normalization: MappingNormalization::CanonicalPartyDisplayNameV1,
        maximum_suggestions_per_response: 1,
        confidence_required: true,
    })
    .unwrap();
    let actor = ActorId::try_new(ACTOR_ID).unwrap();
    let created_request = EnrichmentRequest::create(EnrichmentRequestDraft {
        tenant_id: TenantId::try_new(TENANT_ID).unwrap(),
        requested_by: actor.clone(),
        idempotency_key: IdempotencyKey::try_new("worker-process-domain-request").unwrap(),
        target: TargetSnapshot::try_new("party-process-1", 7, TargetField::PartyDisplayName)
            .unwrap(),
        provider_profile_version_id: profile.version_id().clone(),
        mapping_version_id: mapping.version_id().clone(),
        requested_fields: vec![TargetField::PartyDisplayName],
        policy_evidence: RequestPolicyEvidence::try_new(
            "customer_profile_enrichment",
            "legitimate_interest",
            None,
            "worker-process-policy-v1",
        )
        .unwrap(),
        created_at_unix_ms: 10,
        deadline_at_unix_ms: 1_000,
        expires_at_unix_ms: 2_000,
    })
    .unwrap();
    let party = PartySnapshot {
        party_id: RecordId::try_new("party-process-1").unwrap(),
        display_name: "Process Company".to_owned(),
        resource_version: 7,
        observed_at_unix_ms: 15,
    };
    let mut dispatched_request = created_request.clone();
    let provider_request = prepare_provider_dispatch_attempt(
        &mut dispatched_request,
        ProviderDispatchExpectation {
            status: EnrichmentRequestStatus::Created,
            retry_generation: 0,
        },
        &profile,
        &party,
        actor,
        20,
    )
    .unwrap();
    let dispatch_request = dispatch_capability_request(&created_request);
    Fixture {
        created_request,
        provider_request: provider_request.clone(),
        work_item: ProviderDispatchWorkItem {
            dispatch_request,
            provider_request,
        },
    }
}

async fn seed_request(
    store: &PostgresDataStore,
    request: &EnrichmentRequest,
) -> Result<(), Box<dyn std::error::Error>> {
    let context = seed_context();
    let record = enrichment_request_record_ref(request)?;
    let public_request = enrichment_request_to_wire(request)?;
    let event_payload = support::protobuf_payload(
        MODULE_ID,
        ENRICHMENT_REQUEST_CREATED_EVENT_SCHEMA,
        DataClass::Personal,
        &wire::EnrichmentRequestCreatedEvent {
            enrichment_request: Some(public_request),
        },
    )?;
    store
        .create_record(&RecordCreatePlan {
            context,
            record: record.clone(),
            record_payload: enrichment_request_persisted_payload(request)?,
            event_id: "customer-enrichment-worker-seed-event".to_owned(),
            event: DomainEvent {
                event_type: EventType::try_new(ENRICHMENT_REQUEST_CREATED_EVENT_TYPE)?,
                aggregate: record,
                expected_aggregate_version: None,
                deduplication_key: "customer-enrichment-worker-seed".to_owned(),
                payload: event_payload,
            },
            idempotency: IdempotencyEvidence {
                scope: format!("{SEED_CAPABILITY}@1.0.0"),
                key: "customer-enrichment-worker-seed".to_owned(),
                request_hash: [41; 32],
                expires_at_unix_nanos: 86_400_000_000_000,
            },
            audit: AuditIntent {
                audit_record_id: "customer-enrichment-worker-seed-audit".to_owned(),
                canonicalization_profile: "crm.cjson/v1".to_owned(),
                canonical_envelope: b"{\"operation\":\"seed_enrichment_request\"}".to_vec(),
                occurred_at_unix_nanos: 10_000_000,
            },
        })
        .await?;
    Ok(())
}

fn dispatch_capability_request(request: &EnrichmentRequest) -> CapabilityRequest {
    let definition = request_dispatch_capability_definition().unwrap();
    let input = support::protobuf_payload(
        MODULE_ID,
        DISPATCH_ENRICHMENT_REQUEST_REQUEST_SCHEMA,
        DataClass::Personal,
        &wire::DispatchEnrichmentRequestRequest {
            enrichment_request_ref: Some(wire::EnrichmentRequestRef {
                enrichment_request_id: request.request_id().as_str().to_owned(),
            }),
            expected_status: wire::EnrichmentRequestStatus::Created as i32,
            expected_retry_generation: 0,
        },
    )
    .unwrap();
    let input_hash = semantic_input_hash(&input);
    CapabilityRequest {
        context: worker_context(
            "worker-process-dispatch-request",
            definition.capability_id.as_str(),
            "worker-process-dispatch-idempotency",
            "worker-process-dispatch-tx",
            20_000_000,
        ),
        input,
        input_hash,
        approval: None,
    }
}

fn seed_context() -> ModuleExecutionContext {
    worker_context(
        "worker-process-seed-request",
        SEED_CAPABILITY,
        "worker-process-seed-idempotency",
        "worker-process-seed-tx",
        10_000_000,
    )
}

fn read_context() -> ModuleExecutionContext {
    worker_context(
        "worker-process-read-request",
        SEED_CAPABILITY,
        "worker-process-read-idempotency",
        "worker-process-read-tx",
        40_000_000,
    )
}

fn worker_context(
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

async fn scalar(pool: &PgPool, query: &'static str) -> i64 {
    sqlx::query_scalar::<_, i64>(query)
        .fetch_one(pool)
        .await
        .expect("query worker evidence")
}
