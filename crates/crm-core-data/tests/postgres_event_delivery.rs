#![cfg(feature = "postgres-integration")]

use crm_core_data::{
    AuditIntent, BatchMutationPlan, EventDeliveryClaim, EventDeliveryCompletion,
    EventDeliveryQuery, EventEvidence, IdempotencyEvidence, PostgresDataStore, RecordMutation,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, CorrelationId,
    DataClass, DomainEvent, EventId, EventType, ExecutionContext, IdempotencyKey,
    ModuleExecutionContext, ModuleId, PayloadEncoding, RecordId, RecordRef, RecordType, RequestId,
    RetentionPolicyId, SchemaId, SchemaVersion, TenantId, TraceId, TypedPayload,
};
use sqlx::{PgPool, Row};

const TENANT: &str = "tenant-a";
const OTHER_TENANT: &str = "tenant-b";
const MODULE: &str = "crm.test";
const EVENT_ID: &str = "phase6i-event-delivery-source";
const TRANSACTION_ID: &str = "phase6i-event-delivery-source-tx";
const CORRELATION_ID: &str = "phase6i-event-delivery-correlation";
const TRACE_ID: &str = "phase6i-event-delivery-trace";
const STARTED_AT: i64 = 1_700_000_600_000_000_000;

#[tokio::test(flavor = "current_thread")]
async fn reconstructs_restart_safe_delivery_and_enforces_active_installation() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping PostgreSQL event-delivery acceptance because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let store = PostgresDataStore::connect(&database_url, 4)
        .await
        .expect("connect event delivery store");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect event delivery admin");

    store
        .execute_batch(&seed_plan())
        .await
        .expect("seed authoritative source event");
    set_installation_status(&admin, "active").await;

    let query = EventDeliveryQuery {
        tenant_id: tenant(TENANT),
        event_id: EventId::try_new(EVENT_ID).unwrap(),
        consumer_module_id: module(MODULE),
    };
    let first = store
        .get_event_delivery(&query)
        .await
        .expect("read source delivery")
        .expect("source event must exist");
    let replay = store
        .get_event_delivery(&query)
        .await
        .expect("read source delivery replay")
        .expect("source event must remain available");

    assert_eq!(first.delivery_id, replay.delivery_id);
    assert_eq!(first.event_id.as_str(), EVENT_ID);
    assert_eq!(first.tenant_id.as_str(), TENANT);
    assert_eq!(first.source_module_id.as_str(), MODULE);
    assert_eq!(first.consumer_module_id.as_str(), MODULE);
    assert_eq!(first.source_actor_id.as_str(), "actor-a");
    assert_eq!(first.correlation_id.as_str(), CORRELATION_ID);
    assert_eq!(first.trace_id.as_str(), TRACE_ID);
    assert_eq!(first.event_type.as_str(), "test.event_delivery.created");
    assert_eq!(first.event_version.as_str(), "1.0.0");
    assert_eq!(first.aggregate.record_type.as_str(), "test.event_delivery");
    assert_eq!(
        first.aggregate.record_id.as_str(),
        "phase6i-event-delivery-record"
    );
    assert_eq!(first.aggregate_version, 1);
    assert_eq!(first.payload.owner.as_str(), MODULE);
    assert_eq!(first.payload.bytes, b"event-delivery-payload");

    let foreign = store
        .get_event_delivery(&EventDeliveryQuery {
            tenant_id: tenant(OTHER_TENANT),
            event_id: EventId::try_new(EVENT_ID).unwrap(),
            consumer_module_id: module(MODULE),
        })
        .await
        .expect("cross-tenant read remains non-disclosing");
    assert!(foreign.is_none());

    assert!(
        store
            .is_module_active(&tenant(TENANT), &module(MODULE))
            .await
            .expect("active installation query")
    );

    let now = STARTED_AT + 1_000;
    let first_claim = store
        .claim_event_delivery(&query, "worker-a", now, now + 10_000)
        .await
        .expect("first delivery claim");
    let claimed_delivery_id = match first_claim {
        EventDeliveryClaim::Claimed(claimed) => {
            assert_eq!(claimed.attempt_count, 1);
            assert_eq!(claimed.delivery.delivery_id, first.delivery_id);
            claimed.delivery.delivery_id
        }
        other => panic!("expected first delivery claim, got {other:?}"),
    };

    assert_eq!(
        store
            .claim_event_delivery(&query, "worker-b", now + 1, now + 20_000)
            .await
            .expect("concurrent claim remains safe"),
        EventDeliveryClaim::NotReady
    );
    let stale_worker_error = store
        .complete_event_delivery(
            &tenant(TENANT),
            claimed_delivery_id.as_str(),
            "worker-b",
            EventDeliveryCompletion::Applied,
        )
        .await
        .expect_err("non-owner must not complete another worker lease");
    assert_eq!(stale_worker_error.code, "EVENT_DELIVERY_LEASE_CONFLICT");

    let retry_at = now + 20_000;
    store
        .retry_event_delivery(
            &tenant(TENANT),
            claimed_delivery_id.as_str(),
            "worker-a",
            "TARGET_TEMPORARILY_UNAVAILABLE",
            retry_at,
        )
        .await
        .expect("lease owner may schedule retry");
    assert_eq!(
        store
            .claim_event_delivery(&query, "worker-b", retry_at - 1, retry_at + 10_000)
            .await
            .expect("early retry remains unavailable"),
        EventDeliveryClaim::NotReady
    );
    let retry_claim = store
        .claim_event_delivery(&query, "worker-b", retry_at, retry_at + 10_000)
        .await
        .expect("retry becomes claimable at schedule");
    match retry_claim {
        EventDeliveryClaim::Claimed(claimed) => {
            assert_eq!(claimed.attempt_count, 2);
            assert_eq!(claimed.delivery.delivery_id, claimed_delivery_id);
        }
        other => panic!("expected retry delivery claim, got {other:?}"),
    }
    store
        .complete_event_delivery(
            &tenant(TENANT),
            claimed_delivery_id.as_str(),
            "worker-b",
            EventDeliveryCompletion::Applied,
        )
        .await
        .expect("lease owner completes delivery");
    assert_eq!(
        store
            .claim_event_delivery(&query, "worker-c", retry_at + 1, retry_at + 20_000)
            .await
            .expect("completed delivery remains replay-safe"),
        EventDeliveryClaim::NotReady
    );

    set_installation_status(&admin, "suspended").await;
    assert!(
        !store
            .is_module_active(&tenant(TENANT), &module(MODULE))
            .await
            .expect("suspended installation query")
    );
    assert_eq!(
        store
            .claim_event_delivery(&query, "worker-c", retry_at + 2, retry_at + 30_000)
            .await
            .expect("suspended consumer must be skipped"),
        EventDeliveryClaim::InactiveConsumer
    );
    assert!(
        !store
            .is_module_active(&tenant(OTHER_TENANT), &module(MODULE))
            .await
            .expect("foreign tenant installation remains non-disclosing")
    );

    let persisted = sqlx::query(
        r#"
        SELECT correlation_id, trace_id
        FROM crm.business_transactions
        WHERE tenant_id = $1
          AND business_transaction_id = $2
        "#,
    )
    .bind(TENANT)
    .bind(TRANSACTION_ID)
    .fetch_one(&admin)
    .await
    .expect("read persisted lineage evidence");
    assert_eq!(
        persisted.try_get::<String, _>("correlation_id").unwrap(),
        CORRELATION_ID
    );
    assert_eq!(
        persisted.try_get::<String, _>("trace_id").unwrap(),
        TRACE_ID
    );
}

fn seed_plan() -> BatchMutationPlan {
    let reference = RecordRef {
        record_type: RecordType::try_new("test.event_delivery").unwrap(),
        record_id: RecordId::try_new("phase6i-event-delivery-record").unwrap(),
    };
    let payload = TypedPayload {
        owner: module(MODULE),
        schema_id: SchemaId::try_new("crm.test.event_delivery.v1").unwrap(),
        schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
        descriptor_hash: [0x66; 32],
        data_class: DataClass::Internal,
        encoding: PayloadEncoding::Protobuf,
        maximum_size_bytes: 128,
        retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
        bytes: b"event-delivery-payload".to_vec(),
    };
    let context = ModuleExecutionContext {
        module_id: module(MODULE),
        execution: ExecutionContext {
            tenant_id: tenant(TENANT),
            actor_id: ActorId::try_new("actor-a").unwrap(),
            request_id: RequestId::try_new("phase6i-event-delivery-request").unwrap(),
            correlation_id: CorrelationId::try_new(CORRELATION_ID).unwrap(),
            causation_id: CausationId::try_new("phase6i-event-delivery-causation").unwrap(),
            trace_id: TraceId::try_new(TRACE_ID).unwrap(),
            capability_id: CapabilityId::try_new("test.record.mutate").unwrap(),
            capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
            idempotency_key: IdempotencyKey::try_new("phase6i-event-delivery-idem").unwrap(),
            business_transaction_id: BusinessTransactionId::try_new(TRANSACTION_ID).unwrap(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
            request_started_at_unix_nanos: STARTED_AT,
        },
    };

    BatchMutationPlan {
        context,
        records: vec![RecordMutation::Create {
            reference: reference.clone(),
            payload: payload.clone(),
        }],
        relationships: Vec::new(),
        events: vec![EventEvidence {
            event_id: EVENT_ID.to_owned(),
            event: DomainEvent {
                event_type: EventType::try_new("test.event_delivery.created").unwrap(),
                aggregate: reference,
                expected_aggregate_version: None,
                deduplication_key: "phase6i-event-delivery-created".to_owned(),
                payload,
            },
            aggregate_version: 1,
            event_sequence: 1,
            occurred_at_unix_nanos: STARTED_AT + 1,
        }],
        idempotency: IdempotencyEvidence {
            scope: "test.record.mutate@1.0.0".to_owned(),
            key: "phase6i-event-delivery-idem".to_owned(),
            request_hash: [0x67; 32],
            expires_at_unix_nanos: STARTED_AT + 86_400_000_000_000,
        },
        audits: vec![AuditIntent {
            audit_record_id: "phase6i-event-delivery-audit".to_owned(),
            canonicalization_profile: "crm.cjson/v1".to_owned(),
            canonical_envelope: br#"{"phase":"6i","operation":"event-delivery-seed"}"#.to_vec(),
            occurred_at_unix_nanos: STARTED_AT + 2,
        }],
    }
}

async fn set_installation_status(admin: &PgPool, status: &str) {
    let mut transaction = admin.begin().await.expect("begin installation fixture");
    sqlx::query(
        r#"
        SELECT
          set_config('app.tenant_id', $1, true),
          set_config('app.actor_id', 'actor-a', true),
          set_config('app.request_id', 'phase6i-installation-fixture', true),
          set_config('app.capability_id', 'test.record.mutate', true),
          set_config('app.capability_version', '1.0.0', true),
          set_config('app.business_transaction_id', 'tx-bootstrap-a', true)
        "#,
    )
    .bind(TENANT)
    .execute(&mut *transaction)
    .await
    .expect("bind installation fixture context");
    sqlx::query(
        r#"
        INSERT INTO crm.module_installations (
          tenant_id, install_id, module_id, current_version, status,
          generation, grant_set_digest, last_business_transaction_id
        )
        VALUES ($1, 'phase6i-installation', $2, '1.0.0', $3, 1, $4, 'tx-bootstrap-a')
        ON CONFLICT (tenant_id, module_id)
        DO UPDATE SET
          status = EXCLUDED.status,
          generation = crm.module_installations.generation + 1,
          last_business_transaction_id = EXCLUDED.last_business_transaction_id,
          updated_at = clock_timestamp()
        "#,
    )
    .bind(TENANT)
    .bind(MODULE)
    .bind(status)
    .bind(vec![0x68_u8; 32])
    .execute(&mut *transaction)
    .await
    .expect("upsert installation fixture");
    transaction
        .commit()
        .await
        .expect("commit installation fixture");
}

fn tenant(value: &str) -> TenantId {
    TenantId::try_new(value).unwrap()
}

fn module(value: &str) -> ModuleId {
    ModuleId::try_new(value).unwrap()
}
