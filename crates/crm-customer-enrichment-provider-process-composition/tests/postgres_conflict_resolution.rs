#![cfg(feature = "postgres-integration")]

use crm_capability_plan_support as support;
use crm_core_data::{AuditIntent, IdempotencyEvidence, PostgresDataStore, RecordCreatePlan};
use crm_customer_enrichment::{
    EnrichmentRequest, EnrichmentRequestDraft, EnrichmentRequestId, MappingDraft,
    MappingNormalization, MappingVersion, ProviderProfileDraft, ProviderProfileVersion,
    ProviderResponseConflictDecision, ProviderResponseConflictDraft,
    ProviderResponseConflictResolutionPolicyDecision, ProviderResponseConflictResolutionPolicyPort,
    ProviderResponseConflictResolutionPolicyRequest, ProviderResponseReceiptId, RawPayloadPolicy,
    RequestPolicyEvidence, TargetField, TargetSnapshot,
};
use crm_customer_enrichment_capability_adapter::{
    ENRICHMENT_REQUEST_CREATED_EVENT_SCHEMA, ENRICHMENT_REQUEST_CREATED_EVENT_TYPE, MODULE_ID,
    enrichment_request_persisted_payload, enrichment_request_record_ref,
    enrichment_request_to_wire,
};
use crm_customer_enrichment_provider_process_composition::{
    PostgresProviderResponseConflictResolutionExecutor, PostgresProviderResponseConflictStore,
    ProviderResponseConflictPersistenceLineage, ProviderResponseConflictResolutionCommand,
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn postgres_conflict_resolution_is_live_authorized_immutable_and_replay_safe() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping PostgreSQL conflict resolution because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let store = PostgresDataStore::connect(&database_url, 4)
        .await
        .expect("connect conflict resolution store");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect conflict resolution evidence reader");
    let request = canonical_request();
    seed_request(&store, &request)
        .await
        .expect("seed canonical enrichment request");
    let persistence = PostgresProviderResponseConflictStore::new(store.clone());
    let recorded = persistence
        .record(draft(request.request_id().clone()), lineage())
        .await
        .expect("record unresolved provider-response conflict");
    let conflict_id = recorded.conflict.conflict_id().as_str().to_owned();

    let denied_policy = Arc::new(StaticPolicy::new(
        ProviderResponseConflictResolutionPolicyDecision::Denied {
            policy_version: "provider-conflict-policy-v1".to_owned(),
            safe_reason_code: "operator-not-authorized".to_owned(),
        },
    ));
    let denied_executor = PostgresProviderResponseConflictResolutionExecutor::new(
        store.clone(),
        denied_policy.clone(),
    );
    let denied = denied_executor
        .execute(command(
            &conflict_id,
            ProviderResponseConflictDecision::RetainFirstReceipt,
        ))
        .await
        .expect_err("denied resolution must fail closed");
    assert_eq!(
        denied.code,
        "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_RESOLUTION_DENIED"
    );
    assert_eq!(denied_policy.calls(), 1);
    assert_eq!(record_version(&admin).await, 1);

    let allowed_policy = Arc::new(StaticPolicy::new(
        ProviderResponseConflictResolutionPolicyDecision::Allowed {
            policy_version: "provider-conflict-policy-v1".to_owned(),
        },
    ));
    let executor = PostgresProviderResponseConflictResolutionExecutor::new(
        store.clone(),
        allowed_policy.clone(),
    );
    let first = executor
        .execute(command(
            &conflict_id,
            ProviderResponseConflictDecision::RetainFirstReceipt,
        ))
        .await
        .expect("persist authorized retain-first resolution");
    assert!(!first.replayed);
    assert_eq!(
        first
            .conflict
            .resolution()
            .expect("resolution exists")
            .decision(),
        ProviderResponseConflictDecision::RetainFirstReceipt
    );
    assert_eq!(record_version(&admin).await, 2);

    let replay = executor
        .execute(command(
            &conflict_id,
            ProviderResponseConflictDecision::RetainFirstReceipt,
        ))
        .await
        .expect("replay exact authorized resolution");
    assert!(replay.replayed);
    assert_eq!(replay.conflict, first.conflict);

    let conflicting = executor
        .execute(command(
            &conflict_id,
            ProviderResponseConflictDecision::RejectRequest,
        ))
        .await
        .expect_err("contradictory immutable resolution must fail");
    assert_eq!(
        conflicting.code,
        "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_ALREADY_RESOLVED"
    );
    assert_eq!(allowed_policy.calls(), 3);

    assert_eq!(
        scalar(
            &admin,
            "SELECT count(*)::bigint FROM crm.records WHERE tenant_id = 'tenant-a' AND record_type = 'customer_enrichment.provider_response_conflict'",
        )
        .await,
        1
    );
    assert_eq!(
        scalar(
            &admin,
            "SELECT count(*)::bigint FROM crm.relationships WHERE tenant_id = 'tenant-a' AND relationship_type = 'customer_enrichment.request.provider_response_conflict'",
        )
        .await,
        1
    );
    assert_eq!(
        scalar(
            &admin,
            "SELECT count(*)::bigint FROM crm.outbox_events WHERE tenant_id = 'tenant-a' AND event_type = 'customer_enrichment.provider_response_conflict.recorded'",
        )
        .await,
        2
    );
    assert_eq!(
        scalar(
            &admin,
            "SELECT count(*)::bigint FROM crm.audit_records WHERE tenant_id = 'tenant-a' AND capability_id = 'customer_enrichment.response.record'",
        )
        .await,
        3
    );
    assert_eq!(
        scalar(
            &admin,
            "SELECT count(*)::bigint FROM crm.idempotency_records WHERE tenant_id = 'tenant-a' AND idempotency_scope = 'capability:customer_enrichment.response.record:1.0.0' AND idempotency_key LIKE 'enrichment-conflict-resolution-%'",
        )
        .await,
        1
    );
    assert_eq!(
        scalar(
            &admin,
            "SELECT count(*)::bigint FROM crm.business_transactions WHERE tenant_id = 'tenant-a' AND capability_id = 'customer_enrichment.response.record' AND business_transaction_id LIKE 'enrichment-conflict-resolution-tx-%'",
        )
        .await,
        1
    );
}

#[derive(Clone)]
struct StaticPolicy {
    decision: ProviderResponseConflictResolutionPolicyDecision,
    calls: Arc<AtomicUsize>,
}

impl StaticPolicy {
    fn new(decision: ProviderResponseConflictResolutionPolicyDecision) -> Self {
        Self {
            decision,
            calls: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl ProviderResponseConflictResolutionPolicyPort for StaticPolicy {
    fn evaluate<'a>(
        &'a self,
        _request: ProviderResponseConflictResolutionPolicyRequest,
    ) -> PortFuture<'a, Result<ProviderResponseConflictResolutionPolicyDecision, SdkError>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let decision = self.decision.clone();
        Box::pin(async move { Ok(decision) })
    }
}

fn command(
    conflict_id: &str,
    decision: ProviderResponseConflictDecision,
) -> ProviderResponseConflictResolutionCommand {
    ProviderResponseConflictResolutionCommand {
        tenant_id: TenantId::try_new(TENANT_ID).unwrap(),
        conflict_id: RecordId::try_new(conflict_id).unwrap(),
        actor_id: ActorId::try_new("actor-a").unwrap(),
        decision,
        safe_reason_code: "retain-first-receipt".to_owned(),
        approval_evidence_reference: "approval/provider-conflict/1".to_owned(),
        causation_id: CausationId::try_new("operator-command-1").unwrap(),
        correlation_id: CorrelationId::try_new("operator-correlation-1").unwrap(),
        trace_id: TraceId::try_new("operator-trace-1").unwrap(),
        resolved_at_unix_ms: 60,
    }
}

fn draft(request_id: EnrichmentRequestId) -> ProviderResponseConflictDraft {
    ProviderResponseConflictDraft {
        tenant_id: TenantId::try_new(TENANT_ID).unwrap(),
        request_id,
        retry_generation: 2,
        first_receipt_id: receipt_id(2),
        conflicting_semantic_fingerprint: [3; 32],
        detected_at_unix_ms: 50,
    }
}

fn lineage() -> ProviderResponseConflictPersistenceLineage {
    ProviderResponseConflictPersistenceLineage {
        actor_id: ActorId::try_new("actor-a").unwrap(),
        correlation_id: CorrelationId::try_new("provider-conflict-correlation").unwrap(),
        causation_id: CausationId::try_new("provider-created-event").unwrap(),
        trace_id: TraceId::try_new("provider-conflict-trace").unwrap(),
    }
}

fn canonical_request() -> EnrichmentRequest {
    let profile = ProviderProfileVersion::publish(ProviderProfileDraft {
        provider_key: "company_registry_conflict_resolution".to_owned(),
        adapter_kind: "registry_http_v1".to_owned(),
        adapter_contract_version: "1.0.0".to_owned(),
        supported_target_fields: vec![TargetField::PartyDisplayName],
        purpose_codes: vec!["customer_profile_enrichment".to_owned()],
        license_id: "Registry conflict resolution licence".to_owned(),
        permitted_use_class: "customer_master_review".to_owned(),
        residency_region: "eu".to_owned(),
        retention_days: 30,
        raw_payload_policy: RawPayloadPolicy::DigestOnly,
        credential_handle_aliases: vec!["registry_conflict_resolution".to_owned()],
        effective_at_unix_ms: 1,
        expires_at_unix_ms: Some(5_000),
    })
    .unwrap();
    let mapping = MappingVersion::publish(MappingDraft {
        mapping_key: "party_display_name_conflict_resolution".to_owned(),
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
        requested_by: ActorId::try_new("actor-a").unwrap(),
        idempotency_key: IdempotencyKey::try_new("provider-conflict-resolution-request").unwrap(),
        target: TargetSnapshot::try_new(
            "party-provider-conflict-resolution-1",
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
            "provider-conflict-resolution-policy-v1",
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
    let payload = support::protobuf_payload(
        MODULE_ID,
        ENRICHMENT_REQUEST_CREATED_EVENT_SCHEMA,
        DataClass::Personal,
        &wire::EnrichmentRequestCreatedEvent {
            enrichment_request: Some(enrichment_request_to_wire(request)?),
        },
    )?;
    store
        .create_record(&RecordCreatePlan {
            context: seed_context(),
            record: record.clone(),
            record_payload: enrichment_request_persisted_payload(request)?,
            event_id: "provider-conflict-resolution-seed-event".to_owned(),
            event: DomainEvent {
                event_type: EventType::try_new(ENRICHMENT_REQUEST_CREATED_EVENT_TYPE)?,
                aggregate: record,
                expected_aggregate_version: None,
                deduplication_key: "provider-conflict-resolution-seed".to_owned(),
                payload,
            },
            idempotency: IdempotencyEvidence {
                scope: "customer_enrichment.response.record@1.0.0".to_owned(),
                key: "provider-conflict-resolution-seed".to_owned(),
                request_hash: [51; 32],
                expires_at_unix_nanos: 86_400_000_000_000,
            },
            audit: AuditIntent {
                audit_record_id: "provider-conflict-resolution-seed-audit".to_owned(),
                canonicalization_profile: "crm.cjson/v1".to_owned(),
                canonical_envelope: b"{\"operation\":\"seed_provider_conflict_resolution\"}"
                    .to_vec(),
                occurred_at_unix_nanos: 10_000_000,
            },
        })
        .await?;
    Ok(())
}

fn seed_context() -> ModuleExecutionContext {
    ModuleExecutionContext {
        module_id: ModuleId::try_new(MODULE_ID).unwrap(),
        execution: ExecutionContext {
            tenant_id: TenantId::try_new(TENANT_ID).unwrap(),
            actor_id: ActorId::try_new("actor-a").unwrap(),
            request_id: RequestId::try_new("provider-conflict-resolution-seed-request").unwrap(),
            correlation_id: CorrelationId::try_new("provider-conflict-resolution-seed-correlation")
                .unwrap(),
            causation_id: CausationId::try_new("provider-conflict-resolution-seed-causation")
                .unwrap(),
            trace_id: TraceId::try_new("provider-conflict-resolution-seed-trace").unwrap(),
            capability_id: CapabilityId::try_new("customer_enrichment.response.record").unwrap(),
            capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
            idempotency_key: IdempotencyKey::try_new("provider-conflict-resolution-seed").unwrap(),
            business_transaction_id: BusinessTransactionId::try_new(
                "provider-conflict-resolution-seed-tx",
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

async fn record_version(pool: &PgPool) -> i64 {
    scalar(
        pool,
        "SELECT version::bigint FROM crm.records WHERE tenant_id = 'tenant-a' AND record_type = 'customer_enrichment.provider_response_conflict'",
    )
    .await
}

async fn scalar(pool: &PgPool, query: &'static str) -> i64 {
    sqlx::query_scalar::<_, i64>(query)
        .fetch_one(pool)
        .await
        .expect("read PostgreSQL conflict resolution evidence")
}
