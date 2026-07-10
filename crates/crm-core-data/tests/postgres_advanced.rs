use crm_core_data::{
    AuditEvidence, BatchError, BatchMutationPlan, EventEvidence, FaultInjection,
    IdempotencyEvidence, PostgresDataStore, RecordMutation, RelationshipMutation,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId,
    CorrelationId, DataClass, DomainEvent, EventType, ExecutionContext, IdempotencyKey,
    ModuleExecutionContext, ModuleId, PayloadEncoding, RecordId, RecordRef, RecordType,
    RelationshipRef, RelationshipType, RequestId, RetentionPolicyId, SchemaId, SchemaVersion,
    TenantId, TraceId, TypedPayload,
};
use sqlx::{Postgres, Transaction};

const TENANT: &str = "tenant-a";
const ACTOR: &str = "actor-a";
const CAPABILITY: &str = "test.record.mutate";
const CAPABILITY_VERSION: &str = "1.0.0";

fn context(transaction_id: &str, idempotency_key: &str) -> ModuleExecutionContext {
    ModuleExecutionContext {
        module_id: ModuleId::try_new("crm.test").unwrap(),
        execution: ExecutionContext {
            tenant_id: TenantId::try_new(TENANT).unwrap(),
            actor_id: ActorId::try_new(ACTOR).unwrap(),
            request_id: RequestId::try_new(format!("request-{transaction_id}")).unwrap(),
            correlation_id: CorrelationId::try_new(format!("correlation-{transaction_id}"))
                .unwrap(),
            causation_id: CausationId::try_new(format!("causation-{transaction_id}"))
                .unwrap(),
            trace_id: TraceId::try_new(format!("trace-{transaction_id}")).unwrap(),
            capability_id: CapabilityId::try_new(CAPABILITY).unwrap(),
            capability_version: CapabilityVersion::try_new(CAPABILITY_VERSION).unwrap(),
            idempotency_key: IdempotencyKey::try_new(idempotency_key).unwrap(),
            business_transaction_id: BusinessTransactionId::try_new(transaction_id).unwrap(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
            request_started_at_unix_nanos: 1_700_000_000_000_000_000,
        },
    }
}

fn payload(value: u8, schema_id: &str) -> TypedPayload {
    TypedPayload {
        owner: ModuleId::try_new("crm.test").unwrap(),
        schema_id: SchemaId::try_new(schema_id).unwrap(),
        schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
        descriptor_hash: [value.max(1); 32],
        data_class: DataClass::Internal,
        encoding: PayloadEncoding::Protobuf,
        maximum_size_bytes: 1024,
        retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
        bytes: vec![value],
    }
}

fn record(record_id: &str) -> RecordRef {
    RecordRef {
        record_type: RecordType::try_new("test.batch_record").unwrap(),
        record_id: RecordId::try_new(record_id).unwrap(),
    }
}

fn relationship() -> RelationshipRef {
    RelationshipRef {
        relationship_type: RelationshipType::try_new("test.related_to").unwrap(),
        source: record("batch-a"),
        target: record("batch-b"),
    }
}

fn record_event(
    event_id: &str,
    event_type: &str,
    aggregate: RecordRef,
    aggregate_version: i64,
    event_sequence: i64,
    payload_value: u8,
) -> EventEvidence {
    EventEvidence {
        event_id: event_id.to_owned(),
        event: DomainEvent {
            event_type: EventType::try_new(event_type).unwrap(),
            aggregate,
            expected_aggregate_version: None,
            deduplication_key: format!("dedupe-{event_id}"),
            payload: payload(payload_value, &format!("{event_type}.v1")),
        },
        aggregate_version,
        event_sequence,
        occurred_at_unix_nanos: 1_700_000_000_000_000_000 + event_sequence,
    }
}

fn audit(
    sequence: i64,
    audit_id: &str,
    previous_hash: [u8; 32],
    record_hash: [u8; 32],
) -> AuditEvidence {
    AuditEvidence {
        audit_sequence: sequence,
        audit_record_id: audit_id.to_owned(),
        canonicalization_profile: "crm.cjson/v1".to_owned(),
        previous_hash,
        record_hash,
        canonical_envelope: format!(r#"{{"audit":"{audit_id}"}}"#).into_bytes(),
        occurred_at_unix_nanos: 1_700_000_000_000_000_000 + sequence,
    }
}

fn idempotency(key: &str, request_hash: [u8; 32]) -> IdempotencyEvidence {
    IdempotencyEvidence {
        scope: "test.record.mutate@1.0.0".to_owned(),
        key: key.to_owned(),
        request_hash,
        expires_at_unix_nanos: 1_800_000_000_000_000_000,
    }
}

fn faulted_multi_record_plan() -> BatchMutationPlan {
    let transaction_id = "tx-batch-fault";
    let idempotency_key = "idem-batch-fault";
    BatchMutationPlan {
        context: context(transaction_id, idempotency_key),
        records: vec![
            RecordMutation::Create {
                reference: record("batch-fault-a"),
                payload: payload(0x61, "test.batch_record.v1"),
            },
            RecordMutation::Create {
                reference: record("batch-fault-b"),
                payload: payload(0x62, "test.batch_record.v1"),
            },
        ],
        relationships: Vec::new(),
        events: vec![
            record_event(
                "event-batch-fault-a",
                "test.batch_record.created",
                record("batch-fault-a"),
                1,
                1,
                0x63,
            ),
            record_event(
                "event-batch-fault-b",
                "test.batch_record.created",
                record("batch-fault-b"),
                1,
                1,
                0x64,
            ),
        ],
        idempotency: idempotency(idempotency_key, [0x65; 32]),
        audits: vec![audit(5, "audit-batch-fault", [0x55; 32], [0x60; 32])],
    }
}

fn create_and_link_plan() -> BatchMutationPlan {
    let transaction_id = "tx-batch-create";
    let idempotency_key = "idem-batch-create";
    BatchMutationPlan {
        context: context(transaction_id, idempotency_key),
        records: vec![
            RecordMutation::Create {
                reference: record("batch-a"),
                payload: payload(0x71, "test.batch_record.v1"),
            },
            RecordMutation::Create {
                reference: record("batch-b"),
                payload: payload(0x72, "test.batch_record.v1"),
            },
        ],
        relationships: vec![RelationshipMutation::Link {
            relationship: relationship(),
            payload: payload(0x73, "test.related_to.v1"),
        }],
        events: vec![
            record_event(
                "event-batch-a-created",
                "test.batch_record.created",
                record("batch-a"),
                1,
                1,
                0x74,
            ),
            record_event(
                "event-batch-b-created",
                "test.batch_record.created",
                record("batch-b"),
                1,
                1,
                0x75,
            ),
            record_event(
                "event-batch-relationship-linked",
                "test.relationship.linked",
                RecordRef {
                    record_type: RecordType::try_new("test.relationship").unwrap(),
                    record_id: RecordId::try_new("batch-a-related-batch-b").unwrap(),
                },
                1,
                1,
                0x76,
            ),
        ],
        idempotency: idempotency(idempotency_key, [0x77; 32]),
        audits: vec![
            audit(5, "audit-batch-create-1", [0x55; 32], [0x66; 32]),
            audit(6, "audit-batch-create-2", [0x66; 32], [0x77; 32]),
        ],
    }
}

fn update_plan(
    transaction_id: &str,
    idempotency_key: &str,
    expected_version: i64,
    result_version: i64,
    audit_sequence: i64,
    previous_hash: [u8; 32],
    record_hash: [u8; 32],
    payload_value: u8,
) -> BatchMutationPlan {
    BatchMutationPlan {
        context: context(transaction_id, idempotency_key),
        records: vec![RecordMutation::Update {
            reference: record("batch-a"),
            expected_version,
            payload: payload(payload_value, "test.batch_record.v1"),
        }],
        relationships: Vec::new(),
        events: vec![record_event(
            &format!("event-{transaction_id}"),
            "test.batch_record.updated",
            record("batch-a"),
            result_version,
            result_version,
            payload_value.wrapping_add(1),
        )],
        idempotency: idempotency(idempotency_key, [payload_value; 32]),
        audits: vec![audit(
            audit_sequence,
            &format!("audit-{transaction_id}"),
            previous_hash,
            record_hash,
        )],
    }
}

fn unlink_plan() -> BatchMutationPlan {
    let transaction_id = "tx-batch-unlink";
    let idempotency_key = "idem-batch-unlink";
    BatchMutationPlan {
        context: context(transaction_id, idempotency_key),
        records: Vec::new(),
        relationships: vec![RelationshipMutation::Unlink {
            relationship: relationship(),
        }],
        events: vec![record_event(
            "event-batch-relationship-unlinked",
            "test.relationship.unlinked",
            RecordRef {
                record_type: RecordType::try_new("test.relationship").unwrap(),
                record_id: RecordId::try_new("batch-a-related-batch-b").unwrap(),
            },
            2,
            2,
            0xab,
        )],
        idempotency: idempotency(idempotency_key, [0xac; 32]),
        audits: vec![audit(
            9,
            "audit-batch-unlink",
            [0x99; 32],
            [0xaa; 32],
        )],
    }
}

async fn bind_context(
    transaction: &mut Transaction<'_, Postgres>,
    context: &ModuleExecutionContext,
) {
    sqlx::query(
        r#"
        SELECT
          set_config('app.tenant_id', $1, true),
          set_config('app.actor_id', $2, true),
          set_config('app.request_id', $3, true),
          set_config('app.capability_id', $4, true),
          set_config('app.capability_version', $5, true),
          set_config('app.business_transaction_id', $6, true)
        "#,
    )
    .bind(context.execution.tenant_id.as_str())
    .bind(context.execution.actor_id.as_str())
    .bind(context.execution.request_id.as_str())
    .bind(context.execution.capability_id.as_str())
    .bind(context.execution.capability_version.as_str())
    .bind(context.execution.business_transaction_id.as_str())
    .execute(&mut **transaction)
    .await
    .unwrap();
}

async fn record_count(
    store: &PostgresDataStore,
    context: &ModuleExecutionContext,
    record_ids: &[&str],
) -> i64 {
    let mut transaction = store.pool().begin().await.unwrap();
    bind_context(&mut transaction, context).await;
    let record_ids: Vec<String> = record_ids.iter().map(|value| (*value).to_owned()).collect();
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM crm.records WHERE tenant_id = $1 AND record_id = ANY($2)",
    )
    .bind(context.execution.tenant_id.as_str())
    .bind(record_ids)
    .fetch_one(&mut *transaction)
    .await
    .unwrap();
    transaction.commit().await.unwrap();
    count
}

async fn relationship_count(
    store: &PostgresDataStore,
    context: &ModuleExecutionContext,
) -> i64 {
    let mut transaction = store.pool().begin().await.unwrap();
    bind_context(&mut transaction, context).await;
    let count: i64 = sqlx::query_scalar(
        r#"
        SELECT count(*)
          FROM crm.relationships
         WHERE tenant_id = $1
           AND relationship_type = 'test.related_to'
           AND source_record_id = 'batch-a'
           AND target_record_id = 'batch-b'
        "#,
    )
    .bind(context.execution.tenant_id.as_str())
    .fetch_one(&mut *transaction)
    .await
    .unwrap();
    transaction.commit().await.unwrap();
    count
}

#[tokio::test(flavor = "current_thread")]
async fn batch_executor_is_atomic_idempotent_and_optimistic() {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be configured");
    let store = PostgresDataStore::connect(&database_url, 4)
        .await
        .expect("connect to PostgreSQL");

    let faulted = faulted_multi_record_plan();
    assert!(
        store
            .execute_batch_with_fault(&faulted, FaultInjection::OmitAudit)
            .await
            .is_err(),
        "missing audit evidence must abort the complete multi-record transaction"
    );
    assert_eq!(
        record_count(
            &store,
            &faulted.context,
            &["batch-fault-a", "batch-fault-b"]
        )
        .await,
        0
    );

    let create = create_and_link_plan();
    let created = store.execute_batch(&create).await.unwrap();
    assert!(!created.replayed);
    assert_eq!(created.records.len(), 2);
    assert_eq!(created.linked_relationships, vec![relationship()]);
    assert_eq!(relationship_count(&store, &create.context).await, 1);

    let replayed = store.execute_batch(&create).await.unwrap();
    assert!(replayed.replayed);
    assert_eq!(replayed.records, created.records);
    assert_eq!(record_count(&store, &create.context, &["batch-a", "batch-b"]).await, 2);
    assert_eq!(relationship_count(&store, &create.context).await, 1);

    let mut mismatched = create.clone();
    mismatched.idempotency.request_hash = [0x78; 32];
    assert!(matches!(
        store.execute_batch(&mismatched).await,
        Err(BatchError::IdempotencyKeyReused)
    ));

    let update_one = update_plan(
        "tx-batch-update-1",
        "idem-batch-update-1",
        1,
        2,
        7,
        [0x77; 32],
        [0x88; 32],
        0x81,
    );
    let updated = store.execute_batch(&update_one).await.unwrap();
    assert_eq!(updated.records[0].version, 2);

    let stale = update_plan(
        "tx-batch-stale",
        "idem-batch-stale",
        1,
        3,
        8,
        [0x88; 32],
        [0x98; 32],
        0x91,
    );
    assert!(matches!(
        store.execute_batch(&stale).await,
        Err(BatchError::Conflict(_))
    ));

    let update_two = update_plan(
        "tx-batch-update-2",
        "idem-batch-update-2",
        2,
        3,
        8,
        [0x88; 32],
        [0x99; 32],
        0x92,
    );
    let updated = store.execute_batch(&update_two).await.unwrap();
    assert_eq!(updated.records[0].version, 3);

    let unlink = unlink_plan();
    let unlinked = store.execute_batch(&unlink).await.unwrap();
    assert_eq!(unlinked.unlinked_relationships, vec![relationship()]);
    assert_eq!(relationship_count(&store, &unlink.context).await, 0);
}
