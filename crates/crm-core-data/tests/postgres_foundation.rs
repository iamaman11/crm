use crm_core_data::{
    AuditEvidence, FaultInjection, IdempotencyEvidence, PostgresDataStore, RecordCreatePlan,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId,
    CorrelationId, DataClass, DomainEvent, EventType, ExecutionContext, IdempotencyKey,
    ModuleExecutionContext, ModuleId, PayloadEncoding, RecordId, RecordRef, RecordType,
    RequestId, RetentionPolicyId, SchemaId, SchemaVersion, TenantId, TraceId, TypedPayload,
};

fn context(
    tenant_id: &str,
    transaction_id: &str,
    idempotency_key: &str,
) -> ModuleExecutionContext {
    ModuleExecutionContext {
        module_id: ModuleId::try_new("crm.test").unwrap(),
        execution: ExecutionContext {
            tenant_id: TenantId::try_new(tenant_id).unwrap(),
            actor_id: ActorId::try_new(if tenant_id == "tenant-a" {
                "actor-a"
            } else {
                "actor-b"
            })
            .unwrap(),
            request_id: RequestId::try_new(format!("request-{transaction_id}")).unwrap(),
            correlation_id: CorrelationId::try_new(format!("correlation-{transaction_id}"))
                .unwrap(),
            causation_id: CausationId::try_new(format!("causation-{transaction_id}")).unwrap(),
            trace_id: TraceId::try_new(format!("trace-{transaction_id}")).unwrap(),
            capability_id: CapabilityId::try_new("test.record.mutate").unwrap(),
            capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
            idempotency_key: IdempotencyKey::try_new(idempotency_key).unwrap(),
            business_transaction_id: BusinessTransactionId::try_new(transaction_id).unwrap(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
            request_started_at_unix_nanos: 1_700_000_000_000_000_000,
        },
    }
}

fn payload(value: u8, schema: &str) -> TypedPayload {
    TypedPayload {
        owner: ModuleId::try_new("crm.test").unwrap(),
        schema_id: SchemaId::try_new(schema).unwrap(),
        schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
        descriptor_hash: [value.max(1); 32],
        data_class: DataClass::Internal,
        encoding: PayloadEncoding::Protobuf,
        maximum_size_bytes: 1024,
        retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
        bytes: vec![value],
    }
}

fn plan(
    record_id: &str,
    transaction_id: &str,
    idempotency_key: &str,
    audit_sequence: i64,
    previous_hash: [u8; 32],
    record_hash: [u8; 32],
) -> RecordCreatePlan {
    let record = RecordRef {
        record_type: RecordType::try_new("test.record").unwrap(),
        record_id: RecordId::try_new(record_id).unwrap(),
    };
    RecordCreatePlan {
        context: context("tenant-a", transaction_id, idempotency_key),
        record: record.clone(),
        record_payload: payload(71, "test.record.v1"),
        event_id: format!("event-{transaction_id}"),
        event: DomainEvent {
            event_type: EventType::try_new("test.record.created").unwrap(),
            aggregate: record,
            expected_aggregate_version: None,
            deduplication_key: format!("dedupe-{transaction_id}"),
            payload: payload(72, "test.record.created.v1"),
        },
        idempotency: IdempotencyEvidence {
            scope: "test.record.mutate@1.0.0".to_owned(),
            key: idempotency_key.to_owned(),
            request_hash: [73; 32],
            expires_at_unix_nanos: 1_800_000_000_000_000_000,
        },
        audit: AuditEvidence {
            audit_sequence,
            audit_record_id: format!("audit-{transaction_id}"),
            canonicalization_profile: "crm.cjson/v1".to_owned(),
            previous_hash,
            record_hash,
            canonical_envelope: format!(r#"{{"transaction":"{transaction_id}"}}"#).into_bytes(),
            occurred_at_unix_nanos: 1_700_000_000_000_000_000,
        },
    }
}

#[tokio::test(flavor = "current_thread")]
async fn postgres_adapter_enforces_atomicity_and_tenant_visibility() {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be configured");
    let store = PostgresDataStore::connect(&database_url, 4)
        .await
        .expect("connect to PostgreSQL");

    let valid = plan(
        "rust-valid-record",
        "tx-rust-valid",
        "idem-rust-valid",
        3,
        [0x22; 32],
        [0x33; 32],
    );
    let created = store
        .create_record(&valid)
        .await
        .expect("complete mutation transaction");
    assert_eq!(created.version, 1);

    let visible = store
        .get_record(&valid.context, &valid.record)
        .await
        .expect("read own tenant record");
    assert_eq!(visible, Some(created));

    let other_tenant = context("tenant-b", "tx-rust-read-b", "idem-rust-read-b");
    let hidden = store
        .get_record(&other_tenant, &valid.record)
        .await
        .expect("cross-tenant query must be safely filtered");
    assert!(hidden.is_none());

    let faulted = plan(
        "rust-fault-record",
        "tx-rust-fault",
        "idem-rust-fault",
        4,
        [0x33; 32],
        [0x44; 32],
    );
    assert!(
        store
            .create_record_with_fault(&faulted, FaultInjection::OmitOutbox)
            .await
            .is_err(),
        "deferred evidence check must reject a transaction without outbox evidence"
    );

    let rolled_back = store
        .get_record(&faulted.context, &faulted.record)
        .await
        .expect("query fault-injected record");
    assert!(rolled_back.is_none(), "failed transaction must leave no record");

    let after_fault = plan(
        "rust-after-fault-record",
        "tx-rust-after-fault",
        "idem-rust-after-fault",
        4,
        [0x33; 32],
        [0x55; 32],
    );
    store
        .create_record(&after_fault)
        .await
        .expect("audit sequence and head must have rolled back with the failed transaction");
}
