#![cfg(unix)]

#[path = "support/customer_enrichment_process/mod.rs"]
mod support;

use crm_application_runtime::gateway_v1::application_gateway_service_client::ApplicationGatewayServiceClient;
use crm_capability_runtime::CapabilityDefinition;
use crm_module_sdk::TypedPayload;
use crm_proto_contracts::crm::customer_privacy::v1 as wire;
use prost::Message;
use reqwest::Client as HttpClient;
use serde_json::Value;
use sqlx::{PgPool, Row};
use tonic::{Code, Status};

use support::{
    TENANT_A, TENANT_B, TENANT_OUTSIDE_TOKEN, connect_grpc, free_port, http_mutate, mutate,
    mutation_definition, payload, spawn_crm_api, stop_process, wait_until_ready,
};

const PRIVACY_MODULE: &str = "crm.customer-privacy";
const CREATE_CASE: &str = "customer_privacy.case.create";
const SUBMIT_CASE: &str = "customer_privacy.case.submit";
const RECORD_TYPE: &str = "customer-privacy.case";
const RAW_MARKER: &str = "raw-privacy-submit-payload-must-not-leak";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SubmitEvidenceCounts {
    status_events: i64,
    audits: i64,
    idempotency: i64,
    transactions: i64,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn customer_privacy_case_submit_real_process_is_bounded_and_replay_safe() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping Customer Privacy case-submit crm-api test because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect Customer Privacy submit evidence reader");
    let create_definition = mutation_definition(CREATE_CASE);
    let submit_definition = mutation_definition(SUBMIT_CASE);
    assert_eq!(create_definition.owner_module_id.as_str(), PRIVACY_MODULE);
    assert_eq!(submit_definition.owner_module_id.as_str(), PRIVACY_MODULE);

    let http_addr = format!("127.0.0.1:{}", free_port());
    let grpc_addr = format!("127.0.0.1:{}", free_port());
    let http = HttpClient::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("build Customer Privacy submit HTTP client");
    let mut process = spawn_crm_api(&database_url, &http_addr, &grpc_addr, true, None);
    wait_until_ready(&http, &mut process, &http_addr, true).await;
    let mut grpc = connect_grpc(&grpc_addr).await;

    let initial = submit_evidence_counts(&admin, TENANT_A).await;
    let unauthenticated = http_mutate(
        &http,
        &http_addr,
        &submit_definition,
        &submit_payload(&submit_definition, "privacy-case-hidden", 1),
        TENANT_A,
        "privacy-submit-process-unauthenticated",
        false,
    )
    .await;
    assert_eq!(unauthenticated.status(), reqwest::StatusCode::UNAUTHORIZED);
    let unauthenticated_body: Value = unauthenticated
        .json()
        .await
        .expect("decode unauthenticated submit response");
    assert_eq!(
        unauthenticated_body,
        serde_json::json!({"error": "request_failed"})
    );
    assert_safe_text(&unauthenticated_body.to_string());
    assert_eq!(submit_evidence_counts(&admin, TENANT_A).await, initial);

    let outside_token = mutate(
        &mut grpc,
        &submit_definition,
        submit_payload(&submit_definition, "privacy-case-hidden", 1),
        TENANT_OUTSIDE_TOKEN,
        "privacy-submit-process-outside-token",
        true,
    )
    .await
    .expect_err("tenant outside bearer grant must be denied before submit lookup");
    assert_safe_status(&outside_token, Code::PermissionDenied, "TENANT_FORBIDDEN");
    assert_eq!(submit_evidence_counts(&admin, TENANT_A).await, initial);

    let created = mutate(
        &mut grpc,
        &create_definition,
        create_payload(&create_definition),
        TENANT_A,
        "privacy-submit-process-create",
        true,
    )
    .await
    .expect("create draft case through generic crm-api ingress");
    let created_case = decode_create_case(&created);
    let case_id = created_case
        .privacy_case_ref
        .as_ref()
        .expect("created submit-process case reference")
        .privacy_case_id
        .clone();
    assert_eq!(created_case.status, wire::PrivacyCaseStatus::Draft as i32);
    assert_eq!(created_case.version, 1);

    let submit_key = "privacy-submit-process-submit";
    let first = mutate(
        &mut grpc,
        &submit_definition,
        submit_payload(&submit_definition, &case_id, 1),
        TENANT_A,
        submit_key,
        true,
    )
    .await
    .expect("submit privacy case through generic crm-api ingress");
    let first_case = decode_submit_case(&first);
    assert_eq!(first_case.status, wire::PrivacyCaseStatus::Submitted as i32);
    assert_eq!(first_case.version, 2);
    assert_record_version(&admin, TENANT_A, &case_id, 2).await;
    let committed = submit_evidence_counts(&admin, TENANT_A).await;
    assert_eq!(committed.status_events, initial.status_events + 1);
    assert_eq!(committed.audits, initial.audits + 1);
    assert_eq!(committed.idempotency, initial.idempotency + 1);
    assert_eq!(committed.transactions, initial.transactions + 1);

    let replay = mutate(
        &mut grpc,
        &submit_definition,
        submit_payload(&submit_definition, &case_id, 1),
        TENANT_A,
        submit_key,
        true,
    )
    .await
    .expect("exact case-submit replay returns committed response");
    assert_eq!(decode_submit_case(&replay), first_case);
    assert_record_version(&admin, TENANT_A, &case_id, 2).await;
    assert_eq!(submit_evidence_counts(&admin, TENANT_A).await, committed);

    let conflicting_http = http_mutate(
        &http,
        &http_addr,
        &submit_definition,
        &submit_payload(&submit_definition, &case_id, 2),
        TENANT_A,
        submit_key,
        true,
    )
    .await;
    assert!(conflicting_http.status().is_client_error());
    let conflict_body = conflicting_http
        .text()
        .await
        .expect("read conflicting submit HTTP response");
    assert_safe_text(&conflict_body);
    assert_eq!(submit_evidence_counts(&admin, TENANT_A).await, committed);

    let conflicting_grpc = mutate(
        &mut grpc,
        &submit_definition,
        submit_payload(&submit_definition, &case_id, 2),
        TENANT_A,
        submit_key,
        true,
    )
    .await
    .expect_err("conflicting case-submit replay must fail closed");
    assert_safe_status(
        &conflicting_grpc,
        Code::Aborted,
        "CAPABILITY_IDEMPOTENCY_KEY_REUSED",
    );
    assert_eq!(submit_evidence_counts(&admin, TENANT_A).await, committed);

    let wrong_state = mutate(
        &mut grpc,
        &submit_definition,
        submit_payload(&submit_definition, &case_id, 2),
        TENANT_A,
        "privacy-submit-process-wrong-state",
        true,
    )
    .await
    .expect_err("submitted case cannot be submitted again under another key");
    assert_safe_status(
        &wrong_state,
        Code::Aborted,
        "CUSTOMER_PRIVACY_INVALID_TRANSITION",
    );
    assert_record_version(&admin, TENANT_A, &case_id, 2).await;
    assert_eq!(submit_evidence_counts(&admin, TENANT_A).await, committed);

    let tenant_b_before = submit_evidence_counts(&admin, TENANT_B).await;
    let concealed = mutate(
        &mut grpc,
        &submit_definition,
        submit_payload(&submit_definition, &case_id, 1),
        TENANT_B,
        "privacy-submit-process-cross-tenant",
        true,
    )
    .await
    .expect_err("tenant B must not observe tenant A case");
    assert_safe_status(
        &concealed,
        Code::NotFound,
        "CAPABILITY_AGGREGATE_NOT_FOUND",
    );
    assert_eq!(submit_evidence_counts(&admin, TENANT_B).await, tenant_b_before);

    set_module_status(&admin, TENANT_A, "suspended").await;
    let inactive = mutate(
        &mut grpc,
        &submit_definition,
        submit_payload(&submit_definition, "privacy-case-inactive", 1),
        TENANT_A,
        "privacy-submit-process-inactive",
        true,
    )
    .await
    .expect_err("inactive Customer Privacy module must reject submit");
    assert_safe_status(&inactive, Code::Aborted, "MODULE_NOT_ACTIVE");
    assert_eq!(submit_evidence_counts(&admin, TENANT_A).await, committed);
    stop_process(&mut process).await;
    set_module_status(&admin, TENANT_A, "active").await;

    let denied_http_addr = format!("127.0.0.1:{}", free_port());
    let denied_grpc_addr = format!("127.0.0.1:{}", free_port());
    let mut denied_process = spawn_crm_api(
        &database_url,
        &denied_http_addr,
        &denied_grpc_addr,
        false,
        None,
    );
    wait_until_ready(&http, &mut denied_process, &denied_http_addr, false).await;
    let mut denied_grpc: ApplicationGatewayServiceClient<tonic::transport::Channel> =
        connect_grpc(&denied_grpc_addr).await;
    let authorization_denied = mutate(
        &mut denied_grpc,
        &submit_definition,
        submit_payload(&submit_definition, "privacy-case-no-grant", 1),
        TENANT_A,
        "privacy-submit-process-no-capability-grant",
        true,
    )
    .await
    .expect_err("authenticated submit without live capability grant must fail");
    assert_safe_status(
        &authorization_denied,
        Code::PermissionDenied,
        "CAPABILITY_PERMISSION_DENIED",
    );
    assert_eq!(submit_evidence_counts(&admin, TENANT_A).await, committed);
    stop_process(&mut denied_process).await;
}

fn create_payload(definition: &CapabilityDefinition) -> TypedPayload {
    payload(
        definition,
        wire::CreatePrivacyCaseRequest {
            kind: wire::PrivacyCaseKind::Erasure as i32,
            policy_version: "privacy-policy/1".to_owned(),
            previous_privacy_case_ref: None,
        },
    )
}

fn submit_payload(
    definition: &CapabilityDefinition,
    privacy_case_id: &str,
    expected_version: i64,
) -> TypedPayload {
    payload(
        definition,
        wire::SubmitPrivacyCaseRequest {
            privacy_case_ref: Some(wire::PrivacyCaseRef {
                privacy_case_id: privacy_case_id.to_owned(),
            }),
            expected_version,
        },
    )
}

fn decode_create_case(
    response: &crm_application_runtime::gateway_v1::MutateResponse,
) -> wire::PrivacyCase {
    wire::CreatePrivacyCaseResponse::decode(
        response
            .output
            .as_ref()
            .expect("case-create output")
            .payload
            .as_slice(),
    )
    .expect("decode exact CreatePrivacyCaseResponse")
    .privacy_case
    .expect("create response contains privacy case")
}

fn decode_submit_case(
    response: &crm_application_runtime::gateway_v1::MutateResponse,
) -> wire::PrivacyCase {
    wire::SubmitPrivacyCaseResponse::decode(
        response
            .output
            .as_ref()
            .expect("case-submit output")
            .payload
            .as_slice(),
    )
    .expect("decode exact SubmitPrivacyCaseResponse")
    .privacy_case
    .expect("submit response contains privacy case")
}

async fn submit_evidence_counts(pool: &PgPool, tenant: &str) -> SubmitEvidenceCounts {
    SubmitEvidenceCounts {
        status_events: count(
            pool,
            tenant,
            "SELECT count(*) FROM crm.outbox_events WHERE tenant_id = $1 AND event_type = 'customer_privacy.case.status_changed'",
        )
        .await,
        audits: count(
            pool,
            tenant,
            "SELECT count(*) FROM crm.audit_records WHERE tenant_id = $1 AND capability_id = 'customer_privacy.case.submit'",
        )
        .await,
        idempotency: count(
            pool,
            tenant,
            "SELECT count(*) FROM crm.idempotency_records WHERE tenant_id = $1 AND idempotency_scope = 'capability:customer_privacy.case.submit:1.0.0'",
        )
        .await,
        transactions: count(
            pool,
            tenant,
            "SELECT count(*) FROM crm.business_transactions WHERE tenant_id = $1 AND capability_id = 'customer_privacy.case.submit'",
        )
        .await,
    }
}

async fn assert_record_version(pool: &PgPool, tenant: &str, case_id: &str, version: i64) {
    let actual: i64 = sqlx::query_scalar(
        "SELECT version FROM crm.records WHERE tenant_id = $1 AND record_type = $2 AND record_id = $3",
    )
    .bind(tenant)
    .bind(RECORD_TYPE)
    .bind(case_id)
    .fetch_one(pool)
    .await
    .expect("read submitted privacy-case version");
    assert_eq!(actual, version);
}

async fn count(pool: &PgPool, tenant: &str, sql: &'static str) -> i64 {
    sqlx::query_scalar(sql)
        .bind(tenant)
        .fetch_one(pool)
        .await
        .expect("read Customer Privacy submit evidence count")
}

async fn set_module_status(pool: &PgPool, tenant: &str, status: &str) {
    let row = sqlx::query(
        "SELECT last_business_transaction_id FROM crm.module_installations WHERE tenant_id = $1 AND module_id = $2",
    )
    .bind(tenant)
    .bind(PRIVACY_MODULE)
    .fetch_one(pool)
    .await
    .expect("read Customer Privacy installation");
    let transaction_id: String = row.get("last_business_transaction_id");
    let mut transaction = pool.begin().await.expect("start activation update");
    for (name, value) in [
        ("app.tenant_id", tenant),
        ("app.actor_id", "customer-privacy-submit-process-admin"),
        ("app.request_id", "customer-privacy-submit-process-activation"),
        ("app.capability_id", "customer_privacy.process.activation"),
        ("app.capability_version", "1.0.0"),
        ("app.business_transaction_id", transaction_id.as_str()),
    ] {
        sqlx::query("SELECT set_config($1, $2, true)")
            .bind(name)
            .bind(value)
            .execute(&mut *transaction)
            .await
            .expect("bind submit activation update context");
    }
    sqlx::query(
        "UPDATE crm.module_installations SET status = $1, updated_at = clock_timestamp() WHERE tenant_id = $2 AND module_id = $3",
    )
    .bind(status)
    .bind(tenant)
    .bind(PRIVACY_MODULE)
    .execute(&mut *transaction)
    .await
    .expect("update Customer Privacy activation state");
    transaction
        .commit()
        .await
        .expect("commit submit activation update");
}

fn assert_safe_status(status: &Status, expected_code: Code, expected_error_code: &str) {
    assert_eq!(status.code(), expected_code);
    assert_error_code(status, expected_error_code);
    assert_eq!(
        status
            .metadata()
            .get("x-error-retryable")
            .expect("retryability metadata")
            .to_str()
            .expect("ASCII retryability metadata"),
        "false"
    );
    assert_safe_text(status.message());
    assert_safe_text(&format!("{:?}", status.metadata()));
}

fn assert_error_code(status: &Status, expected_error_code: &str) {
    assert_eq!(
        status
            .metadata()
            .get("x-error-code")
            .expect("typed gRPC error code")
            .to_str()
            .expect("ASCII gRPC error code"),
        expected_error_code
    );
}

fn assert_safe_text(value: &str) {
    for forbidden in [
        RAW_MARKER,
        "internal_reference",
        "crm.records",
        "payload_bytes",
        "descriptor_hash",
        "sqlx",
        "SELECT",
        "postgres://",
        RECORD_TYPE,
    ] {
        assert!(
            !value.contains(forbidden),
            "safe submit transport surface leaked {forbidden}: {value}"
        );
    }
}
