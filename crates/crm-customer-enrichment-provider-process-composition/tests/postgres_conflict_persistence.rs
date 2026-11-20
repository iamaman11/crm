#![cfg(feature = "postgres-integration")]

use crm_capability_plan_support as support;
use crm_core_data::{AuditIntent, IdempotencyEvidence, PostgresDataStore, RecordCreatePlan};
use crm_customer_enrichment::{
    EnrichmentRequest, EnrichmentRequestDraft, EnrichmentRequestId, MappingDraft,
    MappingNormalization, MappingVersion, PROVIDER_RESPONSE_CONFLICT_RECORD_TYPE,
    ProviderProfileDraft, ProviderProfileVersion, ProviderResponseConflictDraft,
    ProviderResponseReceiptId, RawPayloadPolicy, RequestPolicyEvidence, TargetField,
    TargetSnapshot,
};
use crm_customer_enrichment_capability_adapter::{
    ENRICHMENT_REQUEST_CREATED_EVENT_SCHEMA, ENRICHMENT_REQUEST_CREATED_EVENT_TYPE, MODULE_ID,
    enrichment_request_persisted_payload, enrichment_request_record_ref,
    enrichment_request_to_wire,
};
use crm_customer_enrichment_provider_process_composition::{
    PROVIDER_RESPONSE_CONFLICT_RECORDED_EVENT_TYPE, PostgresProviderResponseConflictStore,
    ProviderResponseConflictPersistenceLineage,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, CorrelationId,
    DataClass, DomainEvent, EventType, ExecutionContext, IdempotencyKey, ModuleExecutionContext,
    ModuleId, RecordId, RequestId, SchemaVersion, TenantId, TraceId,
};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;
use sqlx::PgPool;

const TENANT_ID: &str = "tenant-a";
const ACTOR_ID: &str = "actor-a";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn postgres_conflict_persistence_is_atomic_and_exactly_replayable() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping PostgreSQL conflict persistence because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let store = PostgresDataStore::connect(&database_url, 4)
        .await
        .expect("connect conflict persistence store");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect conflict evidence reader");
    let request = canonical_request();
    seed_request(&store, &request)
        .await
        .expect("seed canonical enrichment request for conflict relationship");
    let persistence = PostgresProviderResponseConflictStore::new(store.clone());

    let first = persistence
        .record(draft(request.request_id().clone()), lineage())
        .await
        .expect("record exact provider-response conflict");
    assert!(!first.replayed);
    let replay = persistence
        .record(draft(request.request_id().clone()), lineage())
        .await
        .expect("replay exact provider-response conflict");
    assert!(replay.replayed);
    assert_eq!(first.conflict, replay.conflict);
    let unresolved = persistence
        .unresolved_for_request(
            TenantId::try_new(TENANT_ID).unwrap(),
            RecordId::try_new(first.conflict.request_id().as_str().to_owned()).unwrap(),
        )
        .await
        .expect("load unresolved provider-response conflict")
        .expect("unresolved provider-response conflict exists");
    assert_eq!(unresolved, first.conflict);
    assert_eq!(
        PROVIDER_RESPONSE_CONFLICT_RECORD_TYPE,
        "customer_enrichment.provider_response_conflict"
    );
    assert_eq!(
        PROVIDER_RESPONSE_CONFLICT_RECORDED_EVENT_TYPE,
        "customer_enrichment.provider_response_conflict.recorded"
    );

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
        1
    );
    assert_eq!(
        scalar(
            &admin,
            "SELECT count(*)::bigint FROM crm.audit_records WHERE tenant_id = 'tenant-a' AND capability_id = 'customer_enrichment.response.record'",
        )
        .await,
        2
    );
    assert_eq!(
        scalar(
            &admin,
            "SELECT count(*)::bigint FROM crm.idempotency_records WHERE tenant_id = 'tenant-a' AND idempotency_scope = 'capability:customer_enrichment.response.record:1.0.0' AND idempotency_key LIKE 'enrichment-conflict-%'",
        )
        .await,
        1
    );
    assert_eq!(
        scalar(
            &admin,
            "SELECT count(*)::bigint FROM crm.business_transactions WHERE tenant_id = 'tenant-a' AND capability_id = 'customer_enrichment.response.record' AND business_transaction_id LIKE 'enrichment-conflict-tx-%'",
        )
        .await,
        1
    );
    assert_eq!(
        scalar(
            &admin,
            "SELECT version::bigint FROM crm.records WHERE tenant_id = 'tenant-a' AND record_type = 'customer_enrichment.provider_response_conflict'",
        )
        .await,
        1
    );
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
        actor_id: ActorId::try_new(ACTOR_ID).unwrap(),
        correlation_id: CorrelationId::try_new("provider-conflict-correlation").unwrap(),
        causation_id: CausationId::try_new("provider-created-event").unwrap(),
        trace_id: TraceId::try_new("provider-conflict-trace").unwrap(),
    }
}

fn canonical_request() -> EnrichmentRequest {
    let profile = ProviderProfileVersion::publish(ProviderProfileDraft {
        provider_key: "company_registry_conflict_persistence".to_owned(),
        adapter_kind: "registry_http_v1".to_owned(),
        adapter_contract_version: "1.0.0".to_owned(),
        supported_target_fields: vec![TargetField::PartyDisplayName],
        purpose_codes: vec!["customer_profile_enrichment".to_owned()],
        license_id: "Registry conflict persistence licence".to_owned(),
        permitted_use_class: "customer_master_review".to_owned(),
        residency_region: "eu".to_owned(),
        retention_days: 30,
        raw_payload_policy: RawPayloadPolicy::DigestOnly,
        credential_handle_aliases: vec!["registry_conflict_persistence".to_owned()],
        effective_at_unix_ms: 1,
        expires_at_unix_ms: Some(5_000),
    })
    .unwrap();
    let mapping = MappingVersion::publish(MappingDraft {
        mapping_key: "party_display_name_conflict_persistence".to_owned(),
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
        idempotency_key: IdempotencyKey::try_new("provider-conflict-persistence-request").unwrap(),
        target: TargetSnapshot::try_new(
            "party-provider-conflict-persistence-1",
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
            "provider-conflict-persistence-policy-v1",
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
            event_id: "provider-conflict-persistence-seed-event".to_owned(),
            event: DomainEvent {
                event_type: EventType::try_new(ENRICHMENT_REQUEST_CREATED_EVENT_TYPE)?,
                aggregate: record,
                expected_aggregate_version: None,
                deduplication_key: "provider-conflict-persistence-seed".to_owned(),
                payload,
            },
            idempotency: IdempotencyEvidence {
                scope: "customer_enrichment.response.record@1.0.0".to_owned(),
                key: "provider-conflict-persistence-seed".to_owned(),
                request_hash: [41; 32],
                expires_at_unix_nanos: 86_400_000_000_000,
            },
            audit: AuditIntent {
                audit_record_id: "provider-conflict-persistence-seed-audit".to_owned(),
                canonicalization_profile: "crm.cjson/v1".to_owned(),
                canonical_envelope: b"{\"operation\":\"seed_provider_conflict_persistence\"}"
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
            actor_id: ActorId::try_new(ACTOR_ID).unwrap(),
            request_id: RequestId::try_new("provider-conflict-persistence-seed-request").unwrap(),
            correlation_id: CorrelationId::try_new(
                "provider-conflict-persistence-seed-correlation",
            )
            .unwrap(),
            causation_id: CausationId::try_new("provider-conflict-persistence-seed-causation")
                .unwrap(),
            trace_id: TraceId::try_new("provider-conflict-persistence-seed-trace").unwrap(),
            capability_id: CapabilityId::try_new("customer_enrichment.response.record").unwrap(),
            capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
            idempotency_key: IdempotencyKey::try_new("provider-conflict-persistence-seed").unwrap(),
            business_transaction_id: BusinessTransactionId::try_new(
                "provider-conflict-persistence-seed-tx",
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

async fn scalar(pool: &PgPool, query: &'static str) -> i64 {
    sqlx::query_scalar::<_, i64>(query)
        .fetch_one(pool)
        .await
        .expect("read PostgreSQL conflict evidence")
}
