#![cfg(feature = "postgres-integration")]

use crm_core_data::PostgresDataStore;
use crm_core_events::{ProjectionFailure, ProjectionStore};
use crm_module_sdk::{EventId, TenantId};
use sqlx::{PgPool, Row};

const TENANT: &str = "tenant-a";
const OTHER_TENANT: &str = "tenant-b";
const PROJECTION_ID: &str = "phase7.test-poison.v1";

#[tokio::test(flavor = "current_thread")]
async fn failed_projection_checkpoint_preserves_last_success_and_requires_reset() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!(
            "skipping projection runtime PostgreSQL acceptance because DATABASE_URL is absent"
        );
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let store = PostgresDataStore::connect(&database_url, 4)
        .await
        .expect("connect projection runtime store");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect projection runtime evidence reader");

    cleanup(&admin).await;
    sqlx::query(
        r#"
        INSERT INTO crm.projection_checkpoints (
          tenant_id, projection_id, last_occurred_at, last_event_id,
          applied_event_count, status
        )
        VALUES (
          $1, $2,
          TIMESTAMPTZ 'epoch' + ($3::bigint / 1000) * INTERVAL '1 microsecond',
          $4, 7, 'active'
        )
        "#,
    )
    .bind(TENANT)
    .bind(PROJECTION_ID)
    .bind(1_700_000_000_000_000_000_i64)
    .bind("event-last-success")
    .execute(&admin)
    .await
    .expect("seed successful projection checkpoint");

    ProjectionStore::mark_projection_failed(
        &store,
        ProjectionFailure {
            tenant_id: TenantId::try_new(TENANT).unwrap(),
            projection_id: PROJECTION_ID.to_owned(),
            event_id: EventId::try_new("event-poison").unwrap(),
            occurred_at_unix_nanos: 1_700_000_100_000_000_000,
            failure_code: "TEST_PROJECTION_POISON".to_owned(),
        },
    )
    .await
    .expect("persist failed projection checkpoint");

    let row = sqlx::query(
        r#"
        SELECT last_event_id, applied_event_count, status, failure_event_id, failure_code
        FROM crm.projection_checkpoints
        WHERE tenant_id = $1 AND projection_id = $2
        "#,
    )
    .bind(TENANT)
    .bind(PROJECTION_ID)
    .fetch_one(&admin)
    .await
    .expect("read failed projection checkpoint evidence");
    assert_eq!(row.get::<String, _>("last_event_id"), "event-last-success");
    assert_eq!(row.get::<i64, _>("applied_event_count"), 7);
    assert_eq!(row.get::<String, _>("status"), "failed");
    assert_eq!(
        row.get::<Option<String>, _>("failure_event_id").as_deref(),
        Some("event-poison")
    );
    assert_eq!(
        row.get::<Option<String>, _>("failure_code").as_deref(),
        Some("TEST_PROJECTION_POISON")
    );

    let error = ProjectionStore::projection_checkpoint(
        &store,
        TenantId::try_new(TENANT).unwrap(),
        PROJECTION_ID.to_owned(),
    )
    .await
    .expect_err("failed checkpoint must block replay until reset or repair");
    assert_eq!(error.code, "PROJECTION_CHECKPOINT_FAILED");

    let other_tenant = ProjectionStore::projection_checkpoint(
        &store,
        TenantId::try_new(OTHER_TENANT).unwrap(),
        PROJECTION_ID.to_owned(),
    )
    .await
    .expect("cross-tenant checkpoint lookup remains non-disclosing");
    assert!(other_tenant.is_none());

    ProjectionStore::reset_projection(
        &store,
        TenantId::try_new(TENANT).unwrap(),
        PROJECTION_ID.to_owned(),
    )
    .await
    .expect("reset failed projection checkpoint");
    let after_reset = ProjectionStore::projection_checkpoint(
        &store,
        TenantId::try_new(TENANT).unwrap(),
        PROJECTION_ID.to_owned(),
    )
    .await
    .expect("read reset projection checkpoint");
    assert!(after_reset.is_none());
}

async fn cleanup(admin: &PgPool) {
    sqlx::query(
        "DELETE FROM crm.projection_checkpoints WHERE tenant_id = $1 AND projection_id = $2",
    )
    .bind(TENANT)
    .bind(PROJECTION_ID)
    .execute(admin)
    .await
    .expect("clean projection runtime acceptance state");
}
