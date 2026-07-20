#![cfg(feature = "postgres-integration")]

use crm_core_data::PostgresDataStore;
use crm_customer_enrichment::{
    EnrichmentRequestId, PROVIDER_RESPONSE_CONFLICT_RECORD_TYPE, ProviderResponseConflictDraft,
    ProviderResponseReceiptId,
};
use crm_customer_enrichment_provider_process_composition::{
    PROVIDER_RESPONSE_CONFLICT_RECORDED_EVENT_TYPE, PostgresProviderResponseConflictStore,
    ProviderResponseConflictPersistenceLineage,
};
use crm_module_sdk::{ActorId, CausationId, CorrelationId, TenantId, TraceId};
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
    let persistence = PostgresProviderResponseConflictStore::new(store);

    let first = persistence
        .record(draft(), lineage())
        .await
        .expect("record exact provider-response conflict");
    assert!(!first.replayed);
    let replay = persistence
        .record(draft(), lineage())
        .await
        .expect("replay exact provider-response conflict");
    assert!(replay.replayed);
    assert_eq!(first.conflict, replay.conflict);
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
        1
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

fn draft() -> ProviderResponseConflictDraft {
    ProviderResponseConflictDraft {
        tenant_id: TenantId::try_new(TENANT_ID).unwrap(),
        request_id: request_id(1),
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

fn request_id(byte: u8) -> EnrichmentRequestId {
    serde_json::from_str(&format!(
        "\"enrichment-request-{}\"",
        format!("{byte:02x}").repeat(32)
    ))
    .unwrap()
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
