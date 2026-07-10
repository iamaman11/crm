#[tokio::test(flavor = "current_thread")]
async fn batch_executor_is_atomic_idempotent_and_optimistic() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping PostgreSQL integration scenario because DATABASE_URL is not configured");
        return;
    };
    let store = PostgresDataStore::connect(&database_url, 4)
        .await
        .expect("connect to PostgreSQL");

    let head_context = context("tx-audit-head-read", "idem-audit-head-read");
    let (base_sequence, base_hash) = audit_head(&store, &head_context).await;

    let faulted = faulted_multi_record_plan(base_sequence, base_hash);
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

    let create = create_and_link_plan(base_sequence, base_hash);
    let created = store.execute_batch(&create).await.unwrap();
    assert!(!created.replayed);
    assert_eq!(created.records.len(), 2);
    assert_eq!(created.linked_relationships, vec![relationship()]);
    assert_eq!(relationship_count(&store, &create.context).await, 1);

    let replayed = store.execute_batch(&create).await.unwrap();
    assert!(replayed.replayed);
    assert_eq!(replayed.records, created.records);
    assert_eq!(
        record_count(&store, &create.context, &["batch-a", "batch-b"]).await,
        2
    );
    assert_eq!(relationship_count(&store, &create.context).await, 1);

    let mut mismatched = create.clone();
    mismatched.idempotency.request_hash = [0x78; 32];
    assert!(matches!(
        store.execute_batch(&mismatched).await,
        Err(BatchError::IdempotencyKeyReused)
    ));

    let update_one = update_plan(UpdatePlanSpec {
        transaction_id: "tx-batch-update-1",
        idempotency_key: "idem-batch-update-1",
        expected_version: 1,
        result_version: 2,
        audit_sequence: base_sequence + 3,
        previous_hash: [0x77; 32],
        record_hash: [0x88; 32],
        payload_value: 0x81,
    });
    let updated = store.execute_batch(&update_one).await.unwrap();
    assert_eq!(updated.records[0].version, 2);

    let stale = update_plan(UpdatePlanSpec {
        transaction_id: "tx-batch-stale",
        idempotency_key: "idem-batch-stale",
        expected_version: 1,
        result_version: 3,
        audit_sequence: base_sequence + 4,
        previous_hash: [0x88; 32],
        record_hash: [0x98; 32],
        payload_value: 0x91,
    });
    assert!(matches!(
        store.execute_batch(&stale).await,
        Err(BatchError::Conflict(_))
    ));

    let update_two = update_plan(UpdatePlanSpec {
        transaction_id: "tx-batch-update-2",
        idempotency_key: "idem-batch-update-2",
        expected_version: 2,
        result_version: 3,
        audit_sequence: base_sequence + 4,
        previous_hash: [0x88; 32],
        record_hash: [0x99; 32],
        payload_value: 0x92,
    });
    let updated = store.execute_batch(&update_two).await.unwrap();
    assert_eq!(updated.records[0].version, 3);

    let unlink = unlink_plan(base_sequence + 5, [0x99; 32]);
    let unlinked = store.execute_batch(&unlink).await.unwrap();
    assert_eq!(unlinked.unlinked_relationships, vec![relationship()]);
    assert_eq!(relationship_count(&store, &unlink.context).await, 0);
}
