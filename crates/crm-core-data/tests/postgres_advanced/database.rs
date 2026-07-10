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

async fn relationship_count(store: &PostgresDataStore, context: &ModuleExecutionContext) -> i64 {
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
