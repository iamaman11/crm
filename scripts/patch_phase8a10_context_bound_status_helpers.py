from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one exact status-helper anchor, found {count}")
    file.write_text(text.replace(old, new, 1))


replace_once(
    "crates/crm-customer-enrichment-application-composition/tests/postgres_application_worker_process.rs",
    '''    async fn set_status(admin: &PgPool, status: &str) {
        sqlx::query(
            "UPDATE crm.module_installations SET status = $1, generation = generation + 1, updated_at = clock_timestamp() WHERE tenant_id = $2 AND module_id = $3",
        )
        .bind(status)
        .bind(TENANT)
        .bind(MODULE_ID)
        .execute(admin)
        .await
        .unwrap();
    }
''',
    '''    async fn set_status(admin: &PgPool, status: &str) {
        let request_id = format!("application-worker-status-{status}");
        let transaction_id = format!("application-worker-status-{status}-transaction");
        sqlx::query(
            r#"
            WITH context AS (
              SELECT
                set_config('app.tenant_id', $2, true),
                set_config('app.actor_id', $4, true),
                set_config('app.request_id', $5, true),
                set_config('app.capability_id', $7, true),
                set_config('app.capability_version', '1.0.0', true),
                set_config('app.business_transaction_id', $6, true)
            )
            UPDATE crm.module_installations
            SET status = $1,
                generation = generation + 1,
                last_business_transaction_id = $6,
                updated_at = clock_timestamp()
            FROM context
            WHERE tenant_id = $2 AND module_id = $3
            "#,
        )
        .bind(status)
        .bind(TENANT)
        .bind(MODULE_ID)
        .bind(ACTOR)
        .bind(request_id)
        .bind(transaction_id)
        .bind(SEED)
        .execute(admin)
        .await
        .unwrap();
    }
''',
)

replace_once(
    "crates/crm-application-runtime/tests/support/customer_enrichment_suggestion_get/domain.rs",
    '''pub async fn set_installation_status(admin: &PgPool, status: &str) {
    sqlx::query(
        "UPDATE crm.module_installations SET status = $1, generation = generation + 1, updated_at = clock_timestamp() WHERE tenant_id = $2 AND module_id = $3",
    )
    .bind(status)
    .bind(TENANT)
    .bind(MODULE_ID)
    .execute(admin)
    .await
    .expect("update durable module installation status");
}
''',
    '''pub async fn set_installation_status(admin: &PgPool, status: &str) {
    let actor_id = actor();
    let request_id = format!("suggestion-production-status-{status}");
    let transaction_id = format!("suggestion-production-status-{status}-transaction");
    sqlx::query(
        r#"
        WITH context AS (
          SELECT
            set_config('app.tenant_id', $2, true),
            set_config('app.actor_id', $4, true),
            set_config('app.request_id', $5, true),
            set_config('app.capability_id', $7, true),
            set_config('app.capability_version', '1.0.0', true),
            set_config('app.business_transaction_id', $6, true)
        )
        UPDATE crm.module_installations
        SET status = $1,
            generation = generation + 1,
            last_business_transaction_id = $6,
            updated_at = clock_timestamp()
        FROM context
        WHERE tenant_id = $2 AND module_id = $3
        "#,
    )
    .bind(status)
    .bind(TENANT)
    .bind(MODULE_ID)
    .bind(actor_id.as_str())
    .bind(request_id)
    .bind(transaction_id)
    .bind(SEED_CAPABILITY)
    .execute(admin)
    .await
    .expect("update durable module installation status");
}
''',
)
