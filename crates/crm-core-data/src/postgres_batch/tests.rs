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
                deadline_unix_nanos: 100,
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
                payload: payload(),
                deduplication_key: "deal-1-created".to_owned(),
            },
            aggregate_version: 1,
            event_sequence: 1,
            occurred_at_unix_nanos: 10,
        }
    }

    fn audit(sequence: i64, previous_hash: [u8; 32], record_hash: [u8; 32]) -> AuditEvidence {
        AuditEvidence {
            audit_sequence: sequence,
            audit_record_id: format!("audit-{sequence}"),
            canonicalization_profile: "crm.cjson/v1".to_owned(),
            previous_hash,
            record_hash,
            canonical_envelope: vec![sequence as u8],
            occurred_at_unix_nanos: 10 + sequence,
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
            audits: vec![audit(1, [0; 32], [9; 32])],
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
    fn rejects_disconnected_audit_chain() {
        let mut value = plan();
        value.audits.push(audit(2, [4; 32], [10; 32]));
        assert!(matches!(value.validate(), Err(BatchError::InvalidPlan(_))));
    }

    #[test]
    fn descriptor_hash_is_stable() {
        assert_eq!(batch_result_descriptor_hash(), batch_result_descriptor_hash());
        assert_ne!(batch_result_descriptor_hash(), [0; 32]);
    }
}
