use sqlx::{PgPool, Row};

use super::{MODULE_ID, TENANT_A};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvidenceCounts {
    pub request_records: i64,
    pub events: i64,
    pub audits: i64,
    pub idempotency: i64,
    pub transactions: i64,
}

pub async fn evidence_counts(pool: &PgPool) -> EvidenceCounts {
    EvidenceCounts {
        request_records: scalar_with_owner(
            pool,
            "SELECT count(*)::bigint FROM crm.records WHERE tenant_id = $1 AND owner_module_id = $2 AND record_type = 'customer_enrichment.request'",
        )
        .await,
        events: scalar_for_tenant(
            pool,
            "SELECT count(*)::bigint FROM crm.outbox_events WHERE tenant_id = $1 AND starts_with(event_type, 'customer_enrichment.')",
        )
        .await,
        audits: scalar_for_tenant(
            pool,
            "SELECT count(*)::bigint FROM crm.audit_records WHERE tenant_id = $1 AND starts_with(capability_id, 'customer_enrichment.')",
        )
        .await,
        idempotency: scalar_for_tenant(
            pool,
            "SELECT count(*)::bigint FROM crm.idempotency_records WHERE tenant_id = $1 AND starts_with(idempotency_scope, 'capability:customer_enrichment.')",
        )
        .await,
        transactions: scalar_for_tenant(
            pool,
            "SELECT count(*)::bigint FROM crm.business_transactions WHERE tenant_id = $1 AND starts_with(capability_id, 'customer_enrichment.')",
        )
        .await,
    }
}

pub async fn set_customer_enrichment_status(pool: &PgPool, status: &str) {
    let row = sqlx::query(
        "SELECT last_business_transaction_id FROM crm.module_installations WHERE tenant_id = $1 AND module_id = $2",
    )
    .bind(TENANT_A)
    .bind(MODULE_ID)
    .fetch_one(pool)
    .await
    .expect("read Customer Enrichment installation");
    let transaction_id: String = row.get("last_business_transaction_id");
    let mut transaction = pool.begin().await.expect("start activation update");
    for (name, value) in [
        ("app.tenant_id", TENANT_A),
        ("app.actor_id", "customer-enrichment-process-admin"),
        ("app.request_id", "customer-enrichment-process-activation"),
        (
            "app.capability_id",
            "customer_enrichment.process.activation",
        ),
        ("app.capability_version", "1.0.0"),
        ("app.business_transaction_id", transaction_id.as_str()),
    ] {
        sqlx::query("SELECT set_config($1, $2, true)")
            .bind(name)
            .bind(value)
            .execute(&mut *transaction)
            .await
            .expect("bind activation update context");
    }
    sqlx::query(
        "UPDATE crm.module_installations SET status = $1, updated_at = clock_timestamp() WHERE tenant_id = $2 AND module_id = $3",
    )
    .bind(status)
    .bind(TENANT_A)
    .bind(MODULE_ID)
    .execute(&mut *transaction)
    .await
    .expect("update Customer Enrichment activation state");
    transaction
        .commit()
        .await
        .expect("commit activation update");
}

async fn scalar_for_tenant(pool: &PgPool, sql: &'static str) -> i64 {
    sqlx::query_scalar(sql)
        .bind(TENANT_A)
        .fetch_one(pool)
        .await
        .expect("read tenant-scoped Customer Enrichment evidence count")
}

async fn scalar_with_owner(pool: &PgPool, sql: &'static str) -> i64 {
    sqlx::query_scalar(sql)
        .bind(TENANT_A)
        .bind(MODULE_ID)
        .fetch_one(pool)
        .await
        .expect("read owner-scoped Customer Enrichment evidence count")
}
