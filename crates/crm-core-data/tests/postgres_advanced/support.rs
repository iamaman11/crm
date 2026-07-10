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
            causation_id: CausationId::try_new(format!("causation-{transaction_id}")).unwrap(),
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
