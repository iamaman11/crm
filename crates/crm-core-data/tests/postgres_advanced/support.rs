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

fn audit(audit_id: &str, occurred_at_offset: i64) -> AuditIntent {
    AuditIntent {
        audit_record_id: audit_id.to_owned(),
        canonicalization_profile: "crm.cjson/v1".to_owned(),
        canonical_envelope: format!(r#"{{"audit":"{audit_id}"}}"#).into_bytes(),
        occurred_at_unix_nanos: 1_700_000_000_000_000_000 + occurred_at_offset,
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

struct LockedAggregatePlanner;

impl TransactionalAggregatePlanner for LockedAggregatePlanner {
    fn target(
        &self,
        _definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        let value: serde_json::Value =
            serde_json::from_slice(&request.input.bytes).map_err(|error| {
                SdkError::invalid_argument("input", format!("invalid aggregate command: {error}"))
            })?;
        let record_id = value
            .get("record_id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| SdkError::invalid_argument("input.record_id", "record id is required"))?;
        Ok(AggregateTarget {
            reference: record(record_id),
            presence: AggregatePresence::MustExist,
        })
    }

    fn plan(
        &self,
        _definition: &CapabilityDefinition,
        request: &CapabilityRequest,
        current: Option<&crm_module_sdk::RecordSnapshot>,
    ) -> Result<CapabilityBatchExecutionPlan, SdkError> {
        let current = current.ok_or_else(|| {
            SdkError::new(
                "TEST_AGGREGATE_NOT_FOUND",
                crm_module_sdk::ErrorCategory::NotFound,
                false,
                "The test aggregate was not found.",
            )
        })?;
        let value: serde_json::Value =
            serde_json::from_slice(&request.input.bytes).map_err(|error| {
                SdkError::invalid_argument("input", format!("invalid aggregate command: {error}"))
            })?;
        let expected_version = value
            .get("expected_version")
            .and_then(serde_json::Value::as_i64)
            .ok_or_else(|| {
                SdkError::invalid_argument("input.expected_version", "expected version is required")
            })?;
        if current.version != expected_version {
            return Err(SdkError::new(
                "TEST_AGGREGATE_VERSION_CONFLICT",
                crm_module_sdk::ErrorCategory::Conflict,
                false,
                "The aggregate version is stale.",
            ));
        }
        let next_value = value
            .get("value")
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| u8::try_from(value).ok())
            .ok_or_else(|| SdkError::invalid_argument("input.value", "value must fit in u8"))?;
        let next_version = current.version + 1;
        let tx = request.context.execution.business_transaction_id.as_str();
        let aggregate_payload = payload(next_value, "test.batch_record.v1");
        let output = aggregate_output(
            current.reference.record_id.as_str(),
            next_version,
            next_value,
        );
        Ok(CapabilityBatchExecutionPlan {
            batch: BatchMutationPlan {
                context: request.context.clone(),
                records: vec![RecordMutation::Update {
                    reference: current.reference.clone(),
                    expected_version: current.version,
                    payload: aggregate_payload,
                }],
                relationships: Vec::new(),
                events: vec![record_event(
                    &format!("event-{tx}"),
                    "test.batch_record.updated",
                    current.reference.clone(),
                    next_version,
                    next_version,
                    next_value.wrapping_add(1),
                )],
                idempotency: capability_idempotency(request),
                audits: vec![audit(&format!("audit-{tx}"), next_version + 500)],
            },
            output: Some(output),
        })
    }
}

struct CreatingAggregatePlanner;

impl TransactionalAggregatePlanner for CreatingAggregatePlanner {
    fn target(
        &self,
        _definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        let value = aggregate_command(request)?;
        let record_id = value
            .get("record_id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| SdkError::invalid_argument("input.record_id", "record id is required"))?;
        Ok(AggregateTarget {
            reference: record(record_id),
            presence: AggregatePresence::MustBeAbsent,
        })
    }

    fn plan(
        &self,
        _definition: &CapabilityDefinition,
        request: &CapabilityRequest,
        current: Option<&crm_module_sdk::RecordSnapshot>,
    ) -> Result<CapabilityBatchExecutionPlan, SdkError> {
        if current.is_some() {
            return Err(SdkError::new(
                "TEST_AGGREGATE_ALREADY_EXISTS",
                crm_module_sdk::ErrorCategory::Conflict,
                false,
                "The test aggregate already exists.",
            ));
        }
        let value = aggregate_command(request)?;
        let record_id = value
            .get("record_id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| SdkError::invalid_argument("input.record_id", "record id is required"))?;
        let payload_value = value
            .get("value")
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| u8::try_from(value).ok())
            .ok_or_else(|| SdkError::invalid_argument("input.value", "value must fit in u8"))?;
        let reference = record(record_id);
        let tx = request.context.execution.business_transaction_id.as_str();
        Ok(CapabilityBatchExecutionPlan {
            batch: BatchMutationPlan {
                context: request.context.clone(),
                records: vec![RecordMutation::Create {
                    reference: reference.clone(),
                    payload: payload(payload_value, "test.batch_record.v1"),
                }],
                relationships: Vec::new(),
                events: vec![record_event(
                    &format!("event-{tx}"),
                    "test.batch_record.created",
                    reference,
                    1,
                    1,
                    payload_value,
                )],
                idempotency: capability_idempotency(request),
                audits: vec![audit(
                    &format!("audit-{tx}"),
                    i64::from(payload_value) + 700,
                )],
            },
            output: Some(aggregate_output(record_id, 1, payload_value)),
        })
    }
}

fn aggregate_command(request: &CapabilityRequest) -> Result<serde_json::Value, SdkError> {
    serde_json::from_slice(&request.input.bytes).map_err(|error| {
        SdkError::invalid_argument("input", format!("invalid aggregate command: {error}"))
    })
}

fn capability_idempotency(request: &CapabilityRequest) -> IdempotencyEvidence {
    IdempotencyEvidence {
        scope: "capability:test.record.mutate:1.0.0".to_owned(),
        key: request.context.execution.idempotency_key.to_string(),
        request_hash: request.input_hash,
        expires_at_unix_nanos: 1_800_000_000_000_000_000,
    }
}

fn aggregate_output(record_id: &str, version: i64, value: u8) -> TypedPayload {
    TypedPayload {
        owner: ModuleId::try_new("crm.test").unwrap(),
        schema_id: SchemaId::try_new("test.aggregate.output").unwrap(),
        schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
        descriptor_hash: [0xd1; 32],
        data_class: DataClass::Internal,
        encoding: PayloadEncoding::Json,
        maximum_size_bytes: 1024,
        retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
        bytes: serde_json::to_vec(&serde_json::json!({
            "record_id": record_id,
            "version": version,
            "value": value,
        }))
        .unwrap(),
    }
}

fn aggregate_definition() -> CapabilityDefinition {
    CapabilityDefinition {
        capability_id: CapabilityId::try_new(CAPABILITY).unwrap(),
        capability_version: CapabilityVersion::try_new(CAPABILITY_VERSION).unwrap(),
        owner_module_id: ModuleId::try_new("crm.test").unwrap(),
        input_contract: aggregate_contract("test.aggregate.command", [0xc1; 32]),
        output_contract: Some(aggregate_contract("test.aggregate.output", [0xd1; 32])),
        risk: CapabilityRisk::Low,
        mutation: true,
        requires_idempotency: true,
        requires_approval: false,
        authorization_policy_id: "test.record.mutate".to_owned(),
        rate_limit_policy_id: None,
    }
}

fn aggregate_contract(schema_id: &str, descriptor_hash: [u8; 32]) -> PayloadContract {
    PayloadContract {
        owner: ModuleId::try_new("crm.test").unwrap(),
        schema_id: SchemaId::try_new(schema_id).unwrap(),
        schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
        descriptor_hash,
        allowed_data_classes: vec![DataClass::Internal],
        allowed_encodings: vec![PayloadEncoding::Json],
        maximum_size_bytes: 1024,
    }
}

fn aggregate_request(
    transaction_id: &str,
    idempotency_key: &str,
    expected_version: i64,
    value: u8,
) -> CapabilityRequest {
    let input = TypedPayload {
        owner: ModuleId::try_new("crm.test").unwrap(),
        schema_id: SchemaId::try_new("test.aggregate.command").unwrap(),
        schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
        descriptor_hash: [0xc1; 32],
        data_class: DataClass::Internal,
        encoding: PayloadEncoding::Json,
        maximum_size_bytes: 1024,
        retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
        bytes: serde_json::to_vec(&serde_json::json!({
            "record_id": "aggregate-locked",
            "expected_version": expected_version,
            "value": value,
        }))
        .unwrap(),
    };
    CapabilityRequest {
        context: context(transaction_id, idempotency_key),
        input,
        input_hash: [value.max(1); 32],
        approval: None,
    }
}

fn creating_request(
    transaction_id: &str,
    idempotency_key: &str,
    record_id: &str,
    value: u8,
) -> CapabilityRequest {
    let input = TypedPayload {
        owner: ModuleId::try_new("crm.test").unwrap(),
        schema_id: SchemaId::try_new("test.aggregate.command").unwrap(),
        schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
        descriptor_hash: [0xc1; 32],
        data_class: DataClass::Internal,
        encoding: PayloadEncoding::Json,
        maximum_size_bytes: 1024,
        retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
        bytes: serde_json::to_vec(&serde_json::json!({
            "record_id": record_id,
            "value": value,
        }))
        .unwrap(),
    };
    CapabilityRequest {
        context: context(transaction_id, idempotency_key),
        input,
        input_hash: [value.max(1); 32],
        approval: None,
    }
}
