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
    let initial_head = audit_head(&store, &head_context).await;

    let faulted = faulted_multi_record_plan();
    assert!(
        store
            .execute_batch_with_fault(&faulted, FaultInjection::OmitAudit)
            .await
            .is_err(),
        "missing audit evidence must abort the complete multi-record transaction"
    );
    assert_eq!(audit_head(&store, &head_context).await, initial_head);
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
    let after_create_head = audit_head(&store, &head_context).await;
    assert_eq!(after_create_head.0, initial_head.0 + 2);
    assert_ne!(after_create_head.1, initial_head.1);

    let replayed = store.execute_batch(&create).await.unwrap();
    assert!(replayed.replayed);
    assert_eq!(replayed.records, created.records);
    assert_eq!(audit_head(&store, &head_context).await, after_create_head);
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
        payload_value: 0x81,
    });
    let updated = store.execute_batch(&update_one).await.unwrap();
    assert_eq!(updated.records[0].version, 2);
    let after_update_one_head = audit_head(&store, &head_context).await;
    assert_eq!(after_update_one_head.0, after_create_head.0 + 1);

    let stale = update_plan(UpdatePlanSpec {
        transaction_id: "tx-batch-stale",
        idempotency_key: "idem-batch-stale",
        expected_version: 1,
        result_version: 3,
        payload_value: 0x91,
    });
    assert!(matches!(
        store.execute_batch(&stale).await,
        Err(BatchError::Conflict(_))
    ));
    assert_eq!(audit_head(&store, &head_context).await, after_update_one_head);

    let update_two = update_plan(UpdatePlanSpec {
        transaction_id: "tx-batch-update-2",
        idempotency_key: "idem-batch-update-2",
        expected_version: 2,
        result_version: 3,
        payload_value: 0x92,
    });
    let updated = store.execute_batch(&update_two).await.unwrap();
    assert_eq!(updated.records[0].version, 3);

    let unlink = unlink_plan();
    let unlinked = store.execute_batch(&unlink).await.unwrap();
    assert_eq!(unlinked.unlinked_relationships, vec![relationship()]);
    assert_eq!(relationship_count(&store, &unlink.context).await, 0);

    let concurrent_initial_head = audit_head(&store, &head_context).await;
    let plan_1 = concurrent_create_plan(1);
    let plan_2 = concurrent_create_plan(2);
    let plan_3 = concurrent_create_plan(3);
    let plan_4 = concurrent_create_plan(4);
    let plan_5 = concurrent_create_plan(5);
    let plan_6 = concurrent_create_plan(6);
    let plan_7 = concurrent_create_plan(7);
    let plan_8 = concurrent_create_plan(8);
    let results = tokio::join!(
        store.execute_batch(&plan_1),
        store.execute_batch(&plan_2),
        store.execute_batch(&plan_3),
        store.execute_batch(&plan_4),
        store.execute_batch(&plan_5),
        store.execute_batch(&plan_6),
        store.execute_batch(&plan_7),
        store.execute_batch(&plan_8),
    );
    for result in [
        results.0, results.1, results.2, results.3,
        results.4, results.5, results.6, results.7,
    ] {
        assert!(result.is_ok(), "concurrent batch failed: {result:?}");
    }

    let final_head = audit_head(&store, &head_context).await;
    assert_eq!(final_head.0, concurrent_initial_head.0 + 8);
    assert_ne!(final_head.1, concurrent_initial_head.1);
    assert_eq!(
        record_count(
            &store,
            &head_context,
            &[
                "batch-concurrent-1",
                "batch-concurrent-2",
                "batch-concurrent-3",
                "batch-concurrent-4",
                "batch-concurrent-5",
                "batch-concurrent-6",
                "batch-concurrent-7",
                "batch-concurrent-8",
            ],
        )
        .await,
        8
    );
}
