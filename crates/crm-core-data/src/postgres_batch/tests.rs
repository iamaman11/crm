#[cfg(test)]
mod tests {
    use super::*;
    use crm_module_sdk::{
        ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId,
        CorrelationId, EventType, ExecutionContext, IdempotencyKey, ModuleId, RecordId, RecordType,
        RequestId, RetentionPolicyId, SchemaId, SchemaVersion, TenantId, TraceId,
    };

    fn context() -> ModuleExecutionContext {
        ModuleExecutionContext {
            module_id: ModuleId::try_new("crm.sales").unwrap(),
            execution: ExecutionContext {
                tenant_id: TenantId::try_new("tenant-1").unwrap(),
                actor_id: ActorId::try_new("actor-1").unwrap(),
                request_id: RequestId::try_new("request-1").unwrap(),
                correlation_id: CorrelationId::try_new("correlation-1").unwrap(),
                causation_id: CausationId::try_new("causation-1").unwrap(),
                trace_id: TraceId::try_new("trace-1").unwrap(),
                capability_id: CapabilityId::try_new("sales.deal.mutate").unwrap(),
                capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                business_transaction_id: BusinessTransactionId::try_new("transaction-1").unwrap(),
                idempotency_key: IdempotencyKey::try_new("idempotency-1").unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                request_started_at_unix_nanos: 1,
            },
        }
    }

    fn reference() -> RecordRef {
        RecordRef {
            record_type: RecordType::try_new("sales.deal").unwrap(),
            record_id: RecordId::try_new("deal-1").unwrap(),
        }
    }

    fn payload() -> TypedPayload {
        TypedPayload {
            owner: ModuleId::try_new("crm.sales").unwrap(),
            schema_id: SchemaId::try_new("sales.deal.v1").unwrap(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
            descriptor_hash: [7; 32],
            data_class: DataClass::Internal,
            encoding: PayloadEncoding::Protobuf,
            maximum_size_bytes: 128,
            retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
            bytes: vec![1, 2, 3],
        }
    }

    fn event() -> EventEvidence {
        EventEvidence {
            event_id: "event-1".to_owned(),
            event: DomainEvent {
                event_type: EventType::try_new("sales.deal.created").unwrap(),
                aggregate: reference(),
                expected_aggregate_version: None,
                payload: payload(),
                deduplication_key: "deal-1-created".to_owned(),
            },
            aggregate_version: 1,
            event_sequence: 1,
            occurred_at_unix_nanos: 10,
        }
    }

    fn audit(identifier: &str, occurred_at_unix_nanos: i64) -> AuditIntent {
        AuditIntent {
            audit_record_id: identifier.to_owned(),
            canonicalization_profile: "crm.cjson/v1".to_owned(),
            canonical_envelope: format!(r#"{{"audit":"{identifier}"}}"#).into_bytes(),
            occurred_at_unix_nanos,
        }
    }

    fn plan() -> BatchMutationPlan {
        BatchMutationPlan {
            context: context(),
            records: vec![RecordMutation::Create {
                reference: reference(),
                payload: payload(),
            }],
            relationships: Vec::new(),
            events: vec![event()],
            idempotency: IdempotencyEvidence {
                scope: "sales.deal.mutate".to_owned(),
                key: "idempotency-1".to_owned(),
                request_hash: [8; 32],
                expires_at_unix_nanos: 1_000,
            },
            audits: vec![audit("audit-1", 11)],
        }
    }

    #[test]
    fn accepts_valid_batch() {
        plan().validate().unwrap();
    }

    #[test]
    fn rejects_duplicate_record_mutation() {
        let mut value = plan();
        value.records.push(RecordMutation::Update {
            reference: reference(),
            expected_version: 1,
            payload: payload(),
        });
        assert!(matches!(value.validate(), Err(BatchError::InvalidPlan(_))));
    }

    #[test]
    fn rejects_duplicate_audit_record_identity() {
        let mut value = plan();
        value.audits.push(audit("audit-1", 12));
        assert!(matches!(value.validate(), Err(BatchError::InvalidPlan(_))));
    }

    #[test]
    fn rejects_empty_audit_envelope() {
        let mut value = plan();
        value.audits[0].canonical_envelope.clear();
        assert!(matches!(value.validate(), Err(BatchError::InvalidPlan(_))));
    }

    #[test]
    fn descriptor_hash_is_stable() {
        assert_eq!(batch_result_descriptor_hash(), batch_result_descriptor_hash());
        assert_ne!(batch_result_descriptor_hash(), [0; 32]);
    }
}
