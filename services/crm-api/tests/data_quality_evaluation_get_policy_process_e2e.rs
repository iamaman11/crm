#![cfg(unix)]

#[path = "support/data_quality_evaluation_actor.rs"]
mod data_quality_evaluation_actor;
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
use data_quality_evaluation_fixture::{TENANT, profile_input, rule_set_input, unique_id};
use data_quality_evaluation_gateway::{payload, query_definition, query_for_tenant};
use data_quality_evaluation_operations::{
    create_party_with_display_name, profile_version_id, publish_profile, publish_rule_set,
    request_evaluation, rule_set_version_id,
};
use data_quality_evaluation_process::{start, start_with_environment, stop};
use data_quality_evaluation_registry::register_evaluation_capabilities;
use data_quality_evaluation_worker::build_evaluation_worker;
use prost::Message;
use sqlx::PgPool;
use tonic::Code;

const OTHER_TENANT: &str = "tenant-evaluation-policy-other";
const GET_EVALUATION: &str = "data_quality.party.evaluation.get";

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn evaluation_get_enforces_authorization_redaction_and_cross_tenant_concealment() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping evaluation-get policy proof because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect evaluation-get policy administrator");
    register_evaluation_capabilities(&admin).await;
    provision_worker_actor(&admin).await;

    let mut api = start(&database_url).await;
    let rule_set = publish_rule_set(
        &mut api.client,
        rule_set_input(),
        &unique_id("evaluation-policy-rule-set"),
    )
    .await;
    let rule_set_id = rule_set_version_id(&rule_set);
    let profile = publish_profile(
        &mut api.client,
        profile_input(&rule_set_id),
        &unique_id("evaluation-policy-profile"),
    )
    .await;
    let profile_id = profile_version_id(&profile);
    let party_id = unique_id("evaluation-policy-party");
    create_party_with_display_name(
        &mut api.client,
        &party_id,
        "unknown",
        &unique_id("evaluation-policy-party-create"),
    )
    .await;
    let job_id = unique_id("evaluation-policy-job");
    request_evaluation(
        &mut api.client,
        &job_id,
        &party_id,
        &rule_set_id,
        &profile_id,
        &unique_id("evaluation-policy-request"),
    )
    .await;
    stop(&mut api).await;

    let worker = build_evaluation_worker(&database_url).await;
    let tenant = TenantId::try_new(TENANT).unwrap();
    for _ in 0..8 {
        let result = worker
            .worker
            .run_tenant_cycle(tenant.clone())
            .await
            .expect("run evaluation-get policy worker");
        if result.staged_jobs == 0 && result.materialized_jobs == 0 && result.deferred_jobs == 0 {
            break;
        }
    }

    let tenants = format!("{TENANT},{OTHER_TENANT}");
    let hidden = "data_quality.party.evaluation.get|crm.data-quality|data_quality.party_evaluation_job|failed_rules";
    let policy_environment = [
        ("CRM_API_TENANTS", tenants.as_str()),
        ("CRM_QUERY_HIDDEN_FIELDS", hidden),
    ];
    let mut policy_api = start_with_environment(&database_url, &policy_environment).await;
    let definition = query_definition(GET_EVALUATION);
    let own_response = query_for_tenant(
        &mut policy_api.client,
        &definition,
        payload(&definition, request(&job_id)),
        TENANT,
    )
    .await
    .expect("authorized evaluation-get request");
    let own = data_quality::GetPartyEvaluationJobResponse::decode(
        own_response
            .output
            .expect("evaluation-get output")
            .payload
            .as_slice(),
    )
    .expect("decode evaluation-get output")
    .evaluation_job
    .expect("evaluation job");
    assert_eq!(
        own.status,
        data_quality::PartyEvaluationJobStatus::Completed as i32
    );
    assert_eq!(own.evaluated_rules, 2);
    assert_eq!(own.failed_rules, 0, "failed_rules must be redacted");

    let cross_tenant = query_for_tenant(
        &mut policy_api.client,
        &definition,
        payload(&definition, request(&job_id)),
        OTHER_TENANT,
    )
    .await
    .expect_err("cross-tenant evaluation-get must not disclose existence");
    assert_eq!(cross_tenant.code(), Code::NotFound);
    stop(&mut policy_api).await;

    let denied_tenants = format!("{TENANT},{OTHER_TENANT}");
    let denied_environment = [
        ("CRM_API_TENANTS", denied_tenants.as_str()),
        ("CRM_AUTHORIZATION_DENIED_CAPABILITIES", GET_EVALUATION),
    ];
    let mut denied_api = start_with_environment(&database_url, &denied_environment).await;
    let denied = query_for_tenant(
        &mut denied_api.client,
        &definition,
        payload(&definition, request(&job_id)),
        TENANT,
    )
    .await
    .expect_err("deployment authorization ceiling must deny evaluation-get");
    assert_eq!(denied.code(), Code::PermissionDenied);
    stop(&mut denied_api).await;
}

fn request(job_id: &str) -> data_quality::GetPartyEvaluationJobRequest {
    data_quality::GetPartyEvaluationJobRequest {
        evaluation_job_ref: Some(data_quality::PartyEvaluationJobRef {
            evaluation_job_id: job_id.to_owned(),
        }),
    }
}
