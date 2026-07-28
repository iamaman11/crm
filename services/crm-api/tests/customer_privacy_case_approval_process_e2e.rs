#![cfg(unix)]

#[path = "support/customer_enrichment_process/mod.rs"]
mod support;

use crm_application_runtime::gateway_v1::{
    MutateResponse, QueryResponse, application_gateway_service_client::ApplicationGatewayServiceClient,
};
use crm_capability_runtime::CapabilityDefinition;
use crm_customer_privacy_query_adapter::query_capability_definition;
use crm_module_sdk::TypedPayload;
use crm_proto_contracts::crm::customer_privacy::v1 as wire;
use prost::Message;
use reqwest::Client as HttpClient;
use serde_json::Value;
use sqlx::{PgPool, Row};
use tonic::{Code, Status};

use support::{
    ACTOR, TENANT_A, TENANT_B, TENANT_OUTSIDE_TOKEN, connect_grpc, free_port, http_mutate,
    mutate, mutation_definition, payload, query, spawn_crm_api, stop_process, wait_until_ready,
};

const PRIVACY_MODULE: &str = "crm.customer-privacy";
const APPROVE_CASE: &str = "customer_privacy.case.approve";
const GET_CASE: &str = "customer_privacy.case.get";
const APPROVE_SCOPE: &str = "capability:customer_privacy.case.approve:1.0.0";
const RECORD_TYPE: &str = "customer-privacy.case";
const SUCCESS_CASE: &str = "privacy-approval-case-success";
const STALE_CASE: &str = "privacy-approval-case-stale";
const CORRUPT_CASE: &str = "privacy-approval-case-corrupt-link";
const INACTIVE_CASE: &str = "privacy-approval-case-inactive";
const CANONICAL_PARTY: &str = "privacy-approval-canonical-party";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ApprovalEvidenceCounts {
    events: i64,
    audits: i64,
    idempotency: i64,
    transactions: i64,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn customer_privacy_case_approval_real_process_is_atomic_and_fail_closed() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping Customer Privacy approval process test because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect approval evidence reader");

    for case_id in [SUCCESS_CASE, STALE_CASE, CORRUPT_CASE, INACTIVE_CASE] {
        assert_case_version(&admin, TENANT_A, case_id, 6).await;
    }

    let approve_definition = mutation_definition(APPROVE_CASE);
    let get_definition = query_capability_definition().expect("construct case-get definition");
    assert_eq!(approve_definition.owner_module_id.as_str(), PRIVACY_MODULE);
    assert_eq!(get_definition.owner_module_id.as_str(), PRIVACY_MODULE);
    assert_eq!(get_definition.capability_id.as_str(), GET_CASE);

    let http_addr = format!("127.0.0.1:{}", free_port());
    let grpc_addr = format!("127.0.0.1:{}", free_port());
    let http = HttpClient::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("build approval HTTP client");
    let mut process = spawn_crm_api(&database_url, &http_addr, &grpc_addr, true, None);
    wait_until_ready(&http, &mut process, &http_addr, true).await;
    let mut grpc = connect_grpc(&grpc_addr).await;

    let unauthenticated = http_mutate(
        &http,
        &http_addr,
        &approve_definition,
        &approval_payload(&approve_definition, SUCCESS_CASE, 6),
        TENANT_A,
        "privacy-approval-unauthenticated",
        false,
    )
    .await;
    assert_eq!(unauthenticated.status(), reqwest::StatusCode::UNAUTHORIZED);
    let unauthenticated_body: Value = unauthenticated
        .json()
        .await
        .expect("decode unauthenticated approval response");
    assert_eq!(
        unauthenticated_body,
        serde_json::json!({"error": "request_failed"})
    );
    assert_safe_text(&unauthenticated_body.to_string());
    assert_case_version(&admin, TENANT_A, SUCCESS_CASE, 6).await;

    let outside_token = mutate(
        &mut grpc,
        &approve_definition,
        approval_payload(&approve_definition, SUCCESS_CASE, 6),
        TENANT_OUTSIDE_TOKEN,
        "privacy-approval-outside-token",
        true,
    )
    .await
    .expect_err("tenant outside bearer grant must be denied before approval");
    assert_safe_status(&outside_token, Code::PermissionDenied, "TENANT_FORBIDDEN", false);
    assert_case_version(&admin, TENANT_A, SUCCESS_CASE, 6).await;

    let success_key = "privacy-approval-success";
    let first = mutate(
        &mut grpc,
        &approve_definition,
        approval_payload(&approve_definition, SUCCESS_CASE, 6),
        TENANT_A,
        success_key,
        true,
    )
    .await
    .expect("approve AwaitingApproval case through generic gRPC ingress");
    let first_case = decode_approval(&first);
    assert_approved_case(&first_case, SUCCESS_CASE);
    assert_case_version(&admin, TENANT_A, SUCCESS_CASE, 7).await;

    let persisted = query(
        &mut grpc,
        &get_definition,
        get_payload(&get_definition, SUCCESS_CASE),
        TENANT_A,
        true,
    )
    .await
    .expect("strictly rehydrate approved case through permission-aware query");
    let persisted_case = decode_get(&persisted);
    assert_approved_case(&persisted_case, SUCCESS_CASE);
    assert_eq!(persisted_case, first_case);

    let committed = approval_evidence(&admin, TENANT_A, SUCCESS_CASE, success_key).await;
    assert_eq!(
        committed,
        ApprovalEvidenceCounts {
            events: 1,
            audits: 1,
            idempotency: 1,
            transactions: 1,
        }
    );

    let replay = mutate(
        &mut grpc,
        &approve_definition,
        approval_payload(&approve_definition, SUCCESS_CASE, 6),
        TENANT_A,
        success_key,
        true,
    )
    .await
    .expect("exact approval replay returns committed output");
    assert_eq!(decode_approval(&replay), first_case);
    assert_eq!(
        approval_evidence(&admin, TENANT_A, SUCCESS_CASE, success_key).await,
        committed
    );

    let conflicting_replay = mutate(
        &mut grpc,
        &approve_definition,
        approval_payload(&approve_definition, SUCCESS_CASE, 7),
        TENANT_A,
        success_key,
        true,
    )
    .await
    .expect_err("incompatible approval replay must conflict");
    assert_safe_status(
        &conflicting_replay,
        Code::Aborted,
        "CAPABILITY_IDEMPOTENCY_KEY_REUSED",
        false,
    );
    assert_case_version(&admin, TENANT_A, SUCCESS_CASE, 7).await;

    let already_planned = mutate(
        &mut grpc,
        &approve_definition,
        approval_payload(&approve_definition, SUCCESS_CASE, 7),
        TENANT_A,
        "privacy-approval-already-planned",
        true,
    )
    .await
    .expect_err("Planned case must not be approved under a new key");
    assert_safe_status(
        &already_planned,
        Code::Aborted,
        "CUSTOMER_PRIVACY_APPROVAL_CONFLICT",
        false,
    );

    let cross_tenant = mutate(
        &mut grpc,
        &approve_definition,
        approval_payload(&approve_definition, SUCCESS_CASE, 7),
        TENANT_B,
        "privacy-approval-cross-tenant",
        true,
    )
    .await
    .expect_err("cross-tenant approval target must be concealed");
    assert_safe_status(
        &cross_tenant,
        Code::NotFound,
        "CUSTOMER_PRIVACY_CASE_NOT_FOUND",
        false,
    );

    let stale = mutate(
        &mut grpc,
        &approve_definition,
        approval_payload(&approve_definition, STALE_CASE, 5),
        TENANT_A,
        "privacy-approval-stale",
        true,
    )
    .await
    .expect_err("stale approval version must conflict");
    assert_safe_status(
        &stale,
        Code::Aborted,
        "CUSTOMER_PRIVACY_VERSION_CONFLICT",
        true,
    );
    assert_case_version(&admin, TENANT_A, STALE_CASE, 6).await;

    let corrupt = mutate(
        &mut grpc,
        &approve_definition,
        approval_payload(&approve_definition, CORRUPT_CASE, 6),
        TENANT_A,
        "privacy-approval-corrupt-link",
        true,
    )
    .await
    .expect_err("conflicting immutable planning link must fail closed");
    assert_safe_status(
        &corrupt,
        Code::Internal,
        "CUSTOMER_PRIVACY_APPROVAL_EVIDENCE_INVALID",
        false,
    );
    assert_case_version(&admin, TENANT_A, CORRUPT_CASE, 6).await;
    assert_eq!(
        approval_evidence(
            &admin,
            TENANT_A,
            CORRUPT_CASE,
            "privacy-approval-corrupt-link"
        )
        .await,
        ApprovalEvidenceCounts {
            events: 0,
            audits: 0,
            idempotency: 0,
            transactions: 0,
        }
    );

    set_privacy_module_status(&admin, "suspended").await;
    let inactive = mutate(
        &mut grpc,
        &approve_definition,
        approval_payload(&approve_definition, INACTIVE_CASE, 6),
        TENANT_A,
        "privacy-approval-inactive",
        true,
    )
    .await
    .expect_err("inactive Customer Privacy module must reject approval");
    assert_safe_status(&inactive, Code::Aborted, "MODULE_NOT_ACTIVE", false);
    assert_case_version(&admin, TENANT_A, INACTIVE_CASE, 6).await;
    stop_process(&mut process).await;
    set_privacy_module_status(&admin, "active").await;

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
    let denied = mutate(
        &mut denied_grpc,
        &approve_definition,
        approval_payload(&approve_definition, INACTIVE_CASE, 6),
        TENANT_A,
        "privacy-approval-no-grant",
        true,
    )
    .await
    .expect_err("missing live capability grant must deny approval");
    assert_safe_status(
        &denied,
        Code::PermissionDenied,
        "CAPABILITY_PERMISSION_DENIED",
        false,
    );
    assert_case_version(&admin, TENANT_A, INACTIVE_CASE, 6).await;
    stop_process(&mut denied_process).await;
}

fn approval_payload(
    definition: &CapabilityDefinition,
    case_id: &str,
    expected_version: i64,
) -> TypedPayload {
    payload(
        definition,
        wire::ApprovePrivacyCaseRequest {
            privacy_case_ref: Some(wire::PrivacyCaseRef {
                privacy_case_id: case_id.to_owned(),
            }),
            expected_version,
        },
    )
}

fn get_payload(definition: &CapabilityDefinition, case_id: &str) -> TypedPayload {
    payload(
        definition,
        wire::GetPrivacyCaseRequest {
            privacy_case_ref: Some(wire::PrivacyCaseRef {
                privacy_case_id: case_id.to_owned(),
            }),
        },
    )
}

fn decode_approval(response: &MutateResponse) -> wire::PrivacyCase {
    wire::ApprovePrivacyCaseResponse::decode(
        response
            .output
            .as_ref()
            .expect("approval output")
            .payload
            .as_slice(),
    )
    .expect("decode approval response")
    .privacy_case
    .expect("approval response case")
}

fn decode_get(response: &QueryResponse) -> wire::PrivacyCase {
    wire::GetPrivacyCaseResponse::decode(
        response
            .output
            .as_ref()
            .expect("case-get output")
            .payload
            .as_slice(),
    )
    .expect("decode case-get response")
    .privacy_case
    .expect("case-get response case")
}

fn assert_approved_case(privacy_case: &wire::PrivacyCase, expected_case_id: &str) {
    assert_eq!(
        privacy_case
            .privacy_case_ref
            .as_ref()
            .expect("approved case reference")
            .privacy_case_id,
        expected_case_id
    );
    assert_eq!(privacy_case.status, wire::PrivacyCaseStatus::Planned as i32);
    assert_eq!(privacy_case.version, 7);
    let approval = privacy_case
        .approval
        .as_ref()
        .expect("approved case contains immutable approval evidence");
    assert_eq!(approval.approved_by_actor_id, ACTOR);
    assert!(approval.approved_at_unix_ms > 0);
}

async fn approval_evidence(
    pool: &PgPool,
    tenant: &str,
    case_id: &str,
    idempotency_key: &str,
) -> ApprovalEvidenceCounts {
    ApprovalEvidenceCounts {
        events: sqlx::query_scalar(
            "SELECT count(*) FROM crm.outbox_events WHERE tenant_id = $1 AND aggregate_type = $2 AND aggregate_id = $3 AND event_type = 'customer_privacy.case.status_changed' AND aggregate_version = 7",
        )
        .bind(tenant)
        .bind(RECORD_TYPE)
        .bind(case_id)
        .fetch_one(pool)
        .await
        .expect("count approval events"),
        audits: sqlx::query_scalar(
            "SELECT count(*) FROM crm.audit_records a JOIN crm.idempotency_records i ON i.tenant_id = a.tenant_id AND i.business_transaction_id = a.business_transaction_id WHERE i.tenant_id = $1 AND i.idempotency_scope = $2 AND i.idempotency_key = $3 AND a.capability_id = $4",
        )
        .bind(tenant)
        .bind(APPROVE_SCOPE)
        .bind(idempotency_key)
        .bind(APPROVE_CASE)
        .fetch_one(pool)
        .await
        .expect("count approval audits"),
        idempotency: sqlx::query_scalar(
            "SELECT count(*) FROM crm.idempotency_records WHERE tenant_id = $1 AND idempotency_scope = $2 AND idempotency_key = $3 AND status = 'completed'",
        )
        .bind(tenant)
        .bind(APPROVE_SCOPE)
        .bind(idempotency_key)
        .fetch_one(pool)
        .await
        .expect("count approval idempotency evidence"),
        transactions: sqlx::query_scalar(
            "SELECT count(*) FROM crm.business_transactions bt JOIN crm.idempotency_records i ON i.tenant_id = bt.tenant_id AND i.business_transaction_id = bt.business_transaction_id WHERE i.tenant_id = $1 AND i.idempotency_scope = $2 AND i.idempotency_key = $3 AND bt.capability_id = $4",
        )
        .bind(tenant)
        .bind(APPROVE_SCOPE)
        .bind(idempotency_key)
        .bind(APPROVE_CASE)
        .fetch_one(pool)
        .await
        .expect("count approval business transactions"),
    }
}

async fn assert_case_version(pool: &PgPool, tenant: &str, case_id: &str, version: i64) {
    let actual: i64 = sqlx::query_scalar(
        "SELECT version FROM crm.records WHERE tenant_id = $1 AND owner_module_id = $2 AND record_type = $3 AND record_id = $4",
    )
    .bind(tenant)
    .bind(PRIVACY_MODULE)
    .bind(RECORD_TYPE)
    .bind(case_id)
    .fetch_one(pool)
    .await
    .expect("read approval case version");
    assert_eq!(actual, version);
}

async fn set_privacy_module_status(pool: &PgPool, status: &str) {
    let row = sqlx::query(
        "SELECT last_business_transaction_id FROM crm.module_installations WHERE tenant_id = $1 AND module_id = $2",
    )
    .bind(TENANT_A)
    .bind(PRIVACY_MODULE)
    .fetch_one(pool)
    .await
    .expect("read Customer Privacy installation");
    let transaction_id: String = row.get("last_business_transaction_id");
    let mut transaction = pool.begin().await.expect("start activation update");
    for (name, value) in [
        ("app.tenant_id", TENANT_A),
        ("app.actor_id", "customer-privacy-approval-process-admin"),
        ("app.request_id", "customer-privacy-approval-process-activation"),
        ("app.capability_id", "customer_privacy.process.activation"),
        ("app.capability_version", "1.0.0"),
        ("app.business_transaction_id", transaction_id.as_str()),
    ] {
        sqlx::query("SELECT set_config($1, $2, true)")
            .bind(name)
            .bind(value)
            .execute(&mut *transaction)
            .await
            .expect("bind approval activation context");
    }
    sqlx::query(
        "UPDATE crm.module_installations SET status = $1, updated_at = clock_timestamp() WHERE tenant_id = $2 AND module_id = $3",
    )
    .bind(status)
    .bind(TENANT_A)
    .bind(PRIVACY_MODULE)
    .execute(&mut *transaction)
    .await
    .expect("update Customer Privacy activation state");
    transaction
        .commit()
        .await
        .expect("commit approval activation update");
}

fn assert_safe_status(
    status: &Status,
    expected_code: Code,
    expected_error_code: &str,
    expected_retryable: bool,
) {
    assert_eq!(status.code(), expected_code);
    assert_eq!(
        status
            .metadata()
            .get("x-error-code")
            .expect("typed gRPC error code")
            .to_str()
            .expect("ASCII error code"),
        expected_error_code
    );
    assert_eq!(
        status
            .metadata()
            .get("x-error-retryable")
            .expect("retryability metadata")
            .to_str()
            .expect("ASCII retryability metadata"),
        if expected_retryable { "true" } else { "false" }
    );
    assert_safe_text(status.message());
    assert_safe_text(&format!("{:?}", status.metadata()));
}

fn assert_safe_text(value: &str) {
    for forbidden in [
        CANONICAL_PARTY,
        "privacy-action-plan-",
        "privacy-discovery-scope-",
        "payload_bytes",
        "plan_digest",
        "scope_snapshot_id",
        "postgres://",
        "sqlx",
        "SELECT",
    ] {
        assert!(
            !value.contains(forbidden),
            "safe error leaked protected approval detail: {forbidden}"
        );
    }
}
