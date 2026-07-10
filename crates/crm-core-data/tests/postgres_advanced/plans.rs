fn faulted_multi_record_plan(base_sequence: i64, base_hash: [u8; 32]) -> BatchMutationPlan {
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
        audits: vec![audit(
            base_sequence + 1,
            "audit-batch-fault",
            base_hash,
            [0x60; 32],
        )],
    }
}

fn create_and_link_plan(base_sequence: i64, base_hash: [u8; 32]) -> BatchMutationPlan {
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
            audit(
                base_sequence + 1,
                "audit-batch-create-1",
                base_hash,
                [0x66; 32],
            ),
            audit(
                base_sequence + 2,
                "audit-batch-create-2",
                [0x66; 32],
                [0x77; 32],
            ),
        ],
    }
}

struct UpdatePlanSpec<'a> {
    transaction_id: &'a str,
    idempotency_key: &'a str,
    expected_version: i64,
    result_version: i64,
    audit_sequence: i64,
    previous_hash: [u8; 32],
    record_hash: [u8; 32],
    payload_value: u8,
}

fn update_plan(spec: UpdatePlanSpec<'_>) -> BatchMutationPlan {
    BatchMutationPlan {
        context: context(spec.transaction_id, spec.idempotency_key),
        records: vec![RecordMutation::Update {
            reference: record("batch-a"),
            expected_version: spec.expected_version,
            payload: payload(spec.payload_value, "test.batch_record.v1"),
        }],
        relationships: Vec::new(),
        events: vec![record_event(
            &format!("event-{}", spec.transaction_id),
            "test.batch_record.updated",
            record("batch-a"),
            spec.result_version,
            spec.result_version,
            spec.payload_value.wrapping_add(1),
        )],
        idempotency: idempotency(spec.idempotency_key, [spec.payload_value; 32]),
        audits: vec![audit(
            spec.audit_sequence,
            &format!("audit-{}", spec.transaction_id),
            spec.previous_hash,
            spec.record_hash,
        )],
    }
}

fn unlink_plan(audit_sequence: i64, previous_hash: [u8; 32]) -> BatchMutationPlan {
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
            audit_sequence,
            "audit-batch-unlink",
            previous_hash,
            [0xaa; 32],
        )],
    }
}
