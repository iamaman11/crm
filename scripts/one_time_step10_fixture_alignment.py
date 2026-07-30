from pathlib import Path

path = Path("crates/crm-application-runtime/tests/customer_privacy_access_export_postgres.rs")
text = path.read_text(encoding="utf-8")
old_begin = '''    let mut transaction = admin.begin().await.expect("begin access-export fixture");
    seed_transaction(
'''
new_begin = '''    let mut transaction = admin.begin().await.expect("begin access-export fixture");
    sqlx::query("SET LOCAL session_replication_role = replica")
        .execute(&mut *transaction)
        .await
        .expect("disable trigger-backed access-export fixture verification");
    seed_transaction(
'''
if text.count(old_begin) != 1:
    raise SystemExit(f"fixture begin block count: {text.count(old_begin)}")
text = text.replace(old_begin, new_begin)
old_tx = '''async fn seed_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    tenant: &str,
    business_transaction_id: &str,
    request_id: &str,
) {
    let correlation_id = format!("{business_transaction_id}-correlation");
    let trace_id = format!("{business_transaction_id}-trace");
    sqlx::query(
        r#"
        SELECT set_config('app.tenant_id', $1, true),
               set_config('app.actor_id', $2, true),
               set_config('app.request_id', $3, true),
               set_config('app.correlation_id', $4, true),
               set_config('app.trace_id', $5, true),
               set_config('app.capability_id', $6, true),
               set_config('app.capability_version', $7, true),
               set_config('app.business_transaction_id', $8, true)
        "#,
    )
    .bind(tenant)
    .bind(ACTOR)
    .bind(request_id)
    .bind(&correlation_id)
    .bind(&trace_id)
    .bind("customer_privacy.test.fixture")
    .bind("1.0.0")
    .bind(business_transaction_id)
    .execute(&mut **transaction)
    .await
    .expect("bind access-export fixture transaction context");
    sqlx::query(
        r#"
        INSERT INTO crm.business_transactions (
          tenant_id, business_transaction_id, actor_id, request_id,
          correlation_id, trace_id, capability_id, capability_version,
          expected_outbox_events, expected_audit_records, expected_idempotency_records
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,0,0,0)
        "#,
    )
    .bind(tenant)
    .bind(business_transaction_id)
    .bind(ACTOR)
    .bind(request_id)
    .bind(correlation_id)
    .bind(trace_id)
    .bind("customer_privacy.test.fixture")
    .bind("1.0.0")
    .execute(&mut **transaction)
    .await
    .expect("insert access-export fixture transaction");
}
'''
new_tx = '''async fn seed_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    tenant: &str,
    business_transaction_id: &str,
    request_id: &str,
) {
    sqlx::query(
        r#"
        INSERT INTO crm.business_transactions (
          tenant_id, business_transaction_id, actor_id, request_id,
          correlation_id, trace_id, capability_id, capability_version,
          expected_outbox_events, expected_audit_records, expected_idempotency_records
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,1,1,1)
        "#,
    )
    .bind(tenant)
    .bind(business_transaction_id)
    .bind(ACTOR)
    .bind(request_id)
    .bind(format!("{business_transaction_id}-correlation"))
    .bind(format!("{business_transaction_id}-trace"))
    .bind("customer_privacy.test.fixture")
    .bind("1.0.0")
    .execute(&mut **transaction)
    .await
    .expect("insert access-export fixture transaction");
}
'''
if text.count(old_tx) != 1:
    raise SystemExit(f"fixture transaction block count: {text.count(old_tx)}")
path.write_text(text.replace(old_tx, new_tx), encoding="utf-8")
