from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one exact fix anchor, found {count}")
    file.write_text(text.replace(old, new, 1))


path = "crates/crm-customer-enrichment-materialization-composition/tests/postgres_materialization_event_process.rs"
replace_once(
    path,
    "use crm_capability_plan_support::{self as support, PersistedPayloadContract};\n",
    "use crm_capability_plan_support::{self as support, PersistedPayloadContract};\nuse crm_capability_runtime::CapabilityRequest;\n",
)
replace_once(
    path,
    '''async fn evidence_counts(admin: &PgPool) -> EvidenceCounts {
    EvidenceCounts {
        records: tenant_count(admin, "crm.records").await,
        events: tenant_count(admin, "crm.outbox_events").await,
        audits: tenant_count(admin, "crm.audit_records").await,
        idempotency: tenant_count(admin, "crm.idempotency_records").await,
        transactions: tenant_count(admin, "crm.business_transactions").await,
    }
}

async fn tenant_count(admin: &PgPool, table: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(&format!(
        "SELECT count(*)::bigint FROM {table} WHERE tenant_id = $1"
    ))
    .bind(TENANT_ID)
    .fetch_one(admin)
    .await
    .expect("query tenant evidence count")
}
''',
    '''async fn evidence_counts(admin: &PgPool) -> EvidenceCounts {
    EvidenceCounts {
        records: sqlx::query_scalar::<_, i64>(
            "SELECT count(*)::bigint FROM crm.records WHERE tenant_id = $1",
        )
        .bind(TENANT_ID)
        .fetch_one(admin)
        .await
        .expect("query tenant record count"),
        events: sqlx::query_scalar::<_, i64>(
            "SELECT count(*)::bigint FROM crm.outbox_events WHERE tenant_id = $1",
        )
        .bind(TENANT_ID)
        .fetch_one(admin)
        .await
        .expect("query tenant event count"),
        audits: sqlx::query_scalar::<_, i64>(
            "SELECT count(*)::bigint FROM crm.audit_records WHERE tenant_id = $1",
        )
        .bind(TENANT_ID)
        .fetch_one(admin)
        .await
        .expect("query tenant audit count"),
        idempotency: sqlx::query_scalar::<_, i64>(
            "SELECT count(*)::bigint FROM crm.idempotency_records WHERE tenant_id = $1",
        )
        .bind(TENANT_ID)
        .fetch_one(admin)
        .await
        .expect("query tenant idempotency count"),
        transactions: sqlx::query_scalar::<_, i64>(
            "SELECT count(*)::bigint FROM crm.business_transactions WHERE tenant_id = $1",
        )
        .bind(TENANT_ID)
        .fetch_one(admin)
        .await
        .expect("query tenant transaction count"),
    }
}
''',
)
