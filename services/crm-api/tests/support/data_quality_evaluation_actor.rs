use super::data_quality_evaluation_fixture::{INTERNAL_STAGE, TENANT, WORKER_ACTOR};
use sqlx::PgPool;

pub async fn provision_worker_actor(admin: &PgPool) {
    let mut transaction = admin
        .begin()
        .await
        .expect("begin evaluation worker actor provisioning");
    for (key, value) in [
        ("app.tenant_id", TENANT),
        ("app.actor_id", "actor-a"),
        ("app.request_id", "evaluation-worker-actor-provision"),
        ("app.capability_id", INTERNAL_STAGE),
        ("app.capability_version", "1.0.0"),
        (
            "app.business_transaction_id",
            "evaluation-worker-actor-provision",
        ),
    ] {
        sqlx::query("SELECT set_config($1, $2, true)")
            .bind(key)
            .bind(value)
            .execute(&mut *transaction)
            .await
            .unwrap_or_else(|error| panic!("set evaluation worker context {key}: {error}"));
    }
    sqlx::query(
        "INSERT INTO crm.actors (
           tenant_id, actor_id, actor_type, status, display_name,
           last_business_transaction_id
         ) VALUES ($1, $2, 'service', 'active', $3, $4)
         ON CONFLICT (tenant_id, actor_id) DO NOTHING",
    )
    .bind(TENANT)
    .bind(WORKER_ACTOR)
    .bind("Data Quality evaluation worker")
    .bind("evaluation-worker-actor-provision")
    .execute(&mut *transaction)
    .await
    .expect("provision evaluation worker actor");
    transaction
        .commit()
        .await
        .expect("commit evaluation worker actor provisioning");
}
