#![cfg(unix)]

#[path = "support/data_quality_evaluation_actor.rs"]
mod data_quality_evaluation_actor;
#[path = "support/data_quality_evaluation_assertions.rs"]
mod data_quality_evaluation_assertions;
#[path = "support/data_quality_evaluation_evidence.rs"]
mod data_quality_evaluation_evidence;
#[path = "support/data_quality_evaluation_fixture.rs"]
mod data_quality_evaluation_fixture;
#[path = "support/data_quality_evaluation_gateway.rs"]
mod data_quality_evaluation_gateway;
#[path = "support/data_quality_evaluation_operations.rs"]
mod data_quality_evaluation_operations;
#[path = "support/data_quality_evaluation_process.rs"]
mod data_quality_evaluation_process;
#[path = "support/data_quality_evaluation_registry.rs"]
mod data_quality_evaluation_registry;
#[path = "support/data_quality_evaluation_worker.rs"]
mod data_quality_evaluation_worker;

use crm_module_sdk::TenantId;
use crm_proto_contracts::crm::data_quality::v1 as data_quality;
use data_quality_evaluation_actor::provision_worker_actor;
use data_quality_evaluation_assertions::{
    ExpectedEvaluationEvidence, assert_materialized_evidence, assert_restart_unchanged,
    assert_staged_evidence,
};
use data_quality_evaluation_fixture::{TENANT, profile_input, rule_set_input, unique_id};
use data_quality_evaluation_operations::{
    create_party_with_display_name, profile_version_id, publish_profile, publish_rule_set,
    request_evaluation, rule_set_version_id,
};
use data_quality_evaluation_process::{start, stop};
use data_quality_evaluation_registry::register_evaluation_capabilities;
use data_quality_evaluation_worker::build_evaluation_worker;
use sqlx::PgPool;

const EVALUATED_DISPLAY_NAME: &str = "unknown";

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn evaluation_job_completes_with_exact_findings_and_restarts_without_duplicates() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping evaluation completion process proof because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect evaluation evidence administrator");
    register_evaluation_capabilities(&admin).await;
    provision_worker_actor(&admin).await;

    let mut api = start(&database_url).await;
    let rule_set = publish_rule_set(
        &mut api.client,
        rule_set_input(),
        &unique_id("evaluation-rule-set"),
    )
    .await;
    let rule_set_id = rule_set_version_id(&rule_set);
    let profile = publish_profile(
        &mut api.client,
        profile_input(&rule_set_id),
        &unique_id("evaluation-profile"),
    )
    .await;
    let profile_id = profile_version_id(&profile);
    let party_id = unique_id("evaluation-party");
    let party = create_party_with_display_name(
        &mut api.client,
        &party_id,
        EVALUATED_DISPLAY_NAME,
        &unique_id("evaluation-party-create"),
    )
    .await;
    let party_version = party
        .resource_version
        .as_ref()
        .expect("created Party resource version")
        .version;

    let job_id = unique_id("evaluation-job");
    let created = request_evaluation(
        &mut api.client,
        &job_id,
        &party_id,
        &rule_set_id,
        &profile_id,
        &unique_id("evaluation-request"),
    )
    .await;
    assert_eq!(
        created.status,
        data_quality::PartyEvaluationJobStatus::Created as i32
    );
    assert!(created.evaluated_party_resource_version.is_none());
    assert_eq!(
        created
            .resource_version
            .as_ref()
            .expect("created evaluation job resource version")
            .version,
        1
    );
    stop(&mut api).await;

    let runtime = build_evaluation_worker(&database_url).await;
    let first = runtime
        .worker
        .run_tenant_cycle(TenantId::try_new(TENANT).unwrap())
        .await
        .expect("stage evaluation input through governed worker");
    assert_eq!(first.staged_jobs, 1);
    assert_eq!(first.materialized_jobs, 0);
    assert_eq!(first.deferred_jobs, 0);
    assert_staged_evidence(
        &runtime.store,
        &admin,
        ExpectedEvaluationEvidence {
            job_id: &job_id,
            party_id: &party_id,
            rule_set_id: &rule_set_id,
            profile_id: &profile_id,
            display_name: EVALUATED_DISPLAY_NAME,
            party_version,
            failed_rules: 1,
            score_basis_points: 4_000,
        },
    )
    .await;
    drop(runtime);

    let materializer = build_evaluation_worker(&database_url).await;
    let second = materializer
        .worker
        .run_tenant_cycle(TenantId::try_new(TENANT).unwrap())
        .await
        .expect("atomically materialize outcomes findings and completion after restart");
    assert_eq!(second.staged_jobs, 0);
    assert_eq!(second.materialized_jobs, 1);
    assert_eq!(second.deferred_jobs, 0);
    let stable = assert_materialized_evidence(
        &materializer.store,
        &admin,
        ExpectedEvaluationEvidence {
            job_id: &job_id,
            party_id: &party_id,
            rule_set_id: &rule_set_id,
            profile_id: &profile_id,
            display_name: EVALUATED_DISPLAY_NAME,
            party_version,
            failed_rules: 1,
            score_basis_points: 4_000,
        },
    )
    .await;
    drop(materializer);

    let restarted = build_evaluation_worker(&database_url).await;
    let third = restarted
        .worker
        .run_tenant_cycle(TenantId::try_new(TENANT).unwrap())
        .await
        .expect("restart completed evaluation worker");
    assert_eq!(third.staged_jobs, 0);
    assert_eq!(third.materialized_jobs, 0);
    assert_eq!(third.deferred_jobs, 0);
    assert_restart_unchanged(&restarted.store, &admin, &job_id, stable).await;
}
