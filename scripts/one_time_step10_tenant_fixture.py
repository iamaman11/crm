from pathlib import Path

path = Path("crates/crm-application-runtime/tests/customer_privacy_access_export_postgres.rs")
text = path.read_text(encoding="utf-8")
old_call = '''    cleanup(&admin).await;

    let (privacy_case, plan, decision) = build_case_plan_and_decision();
'''
new_call = '''    cleanup(&admin).await;
    seed_tenant_and_actor(&admin).await;

    let (privacy_case, plan, decision) = build_case_plan_and_decision();
'''
if text.count(old_call) != 1:
    raise SystemExit(f"tenant fixture call block count: {text.count(old_call)}")
text = text.replace(old_call, new_call)
marker = '''async fn seed_record(
'''
helper = '''async fn seed_tenant_and_actor(admin: &PgPool) {
    let mut transaction = admin
        .begin()
        .await
        .expect("begin access-export tenant fixture");
    sqlx::query("SET LOCAL session_replication_role = replica")
        .execute(&mut *transaction)
        .await
        .expect("disable trigger-backed tenant fixture verification");
    sqlx::query(
        "INSERT INTO crm.tenants (tenant_id, status, data_region) VALUES ($1, 'active', 'eu-central') ON CONFLICT DO NOTHING",
    )
    .bind(TENANT_A)
    .execute(&mut *transaction)
    .await
    .expect("insert access-export fixture tenant");
    sqlx::query(
        r#"
        INSERT INTO crm.actors (
          tenant_id, actor_id, actor_type, status, display_name,
          last_business_transaction_id
        ) VALUES ($1,$2,'service','active','Access export fixture actor',$3)
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(TENANT_A)
    .bind(ACTOR)
    .bind("access-export-fixture-actor")
    .execute(&mut *transaction)
    .await
    .expect("insert access-export fixture actor");
    transaction
        .commit()
        .await
        .expect("commit access-export tenant fixture");
}

async fn seed_record(
'''
if text.count(marker) != 1:
    raise SystemExit(f"seed record marker count: {text.count(marker)}")
text = text.replace(marker, helper)
old_cleanup = '''        "DELETE FROM crm.records WHERE tenant_id IN ($1, $2)",
        "DELETE FROM crm.business_transactions WHERE tenant_id IN ($1, $2)",
'''
new_cleanup = '''        "DELETE FROM crm.records WHERE tenant_id IN ($1, $2)",
        "DELETE FROM crm.business_transactions WHERE tenant_id IN ($1, $2)",
        "DELETE FROM crm.actors WHERE tenant_id IN ($1, $2)",
        "DELETE FROM crm.tenants WHERE tenant_id IN ($1, $2)",
'''
if text.count(old_cleanup) != 1:
    raise SystemExit(f"tenant cleanup block count: {text.count(old_cleanup)}")
path.write_text(text.replace(old_cleanup, new_cleanup), encoding="utf-8")
