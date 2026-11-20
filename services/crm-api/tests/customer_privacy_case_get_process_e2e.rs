#![cfg(unix)]

#[path = "support/customer_enrichment_process/mod.rs"]
mod support;

use crm_application_runtime::gateway_v1::application_gateway_service_client::ApplicationGatewayServiceClient;
use crm_capability_runtime::CapabilityDefinition;
use crm_customer_privacy_query_adapter::query_capability_definition;
use crm_customer_privacy_subject_capability_adapter::capability_definition as subject_definition;
use crm_module_sdk::TypedPayload;
use crm_proto_contracts::crm::{
    customer::v1 as customer_wire, customer_privacy::v1 as wire, parties::v1 as parties_wire,
};
use prost::Message;
use reqwest::{Client as HttpClient, Response as HttpResponse};
use serde_json::Value;
use sqlx::{PgPool, Row};
use tonic::{Code, Status};

use support::{
    TENANT_A, TENANT_B, TENANT_OUTSIDE_TOKEN, connect_grpc, free_port, mutate, mutation_definition,
    payload, query, spawn_crm_api, stop_process, wait_until_ready,
};

const PRIVACY_MODULE: &str = "crm.customer-privacy";
const PARTY_CREATE: &str = "parties.party.create";
const CREATE_CASE: &str = "customer_privacy.case.create";
const SUBMIT_CASE: &str = "customer_privacy.case.submit";
const GET_CASE: &str = "customer_privacy.case.get";
const RECORD_TYPE: &str = "customer-privacy.case";
const PARTY_A: &str = "privacy-case-get-party-a";
const RAW_MARKER: &str = "raw-privacy-case-query-payload-must-not-leak";
const HIDDEN_SUBJECT_BINDING: &str =
    "customer_privacy.case.get|crm.customer-privacy|customer-privacy.case|subject_binding";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct QueryEvidenceCounts {
    audits: i64,
    idempotency: i64,
    transactions: i64,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn customer_privacy_case_get_real_process_is_permission_aware_and_side_effect_free() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping Customer Privacy case-get crm-api test because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect case-get evidence reader");

    let party_definition = mutation_definition(PARTY_CREATE);
    let create_definition = mutation_definition(CREATE_CASE);
    let submit_definition = mutation_definition(SUBMIT_CASE);
    let verify_definition =
        subject_definition().expect("construct subject verification definition");
    let get_definition = query_capability_definition().expect("construct case-get definition");
    assert_eq!(get_definition.owner_module_id.as_str(), PRIVACY_MODULE);
    assert_eq!(get_definition.capability_id.as_str(), GET_CASE);

    let http_addr = format!("127.0.0.1:{}", free_port());
    let grpc_addr = format!("127.0.0.1:{}", free_port());
    let http = HttpClient::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("build case-get HTTP client");
    let mut process = spawn_crm_api(
        &database_url,
        &http_addr,
        &grpc_addr,
        true,
        Some(HIDDEN_SUBJECT_BINDING),
    );
    wait_until_ready(&http, &mut process, &http_addr, true).await;
    let mut grpc = connect_grpc(&grpc_addr).await;

    create_party(
        &mut grpc,
        &party_definition,
        TENANT_A,
        PARTY_A,
        "privacy-case-get-party-create",
    )
    .await;
    let case_id = create_submit_and_verify_case(
        &mut grpc,
        &create_definition,
        &submit_definition,
        &verify_definition,
    )
    .await;
    assert_record_version(&admin, TENANT_A, &case_id, 3).await;
    let baseline = query_evidence_counts(&admin, TENANT_A).await;

    let unauthenticated = http_query(
        &http,
        &http_addr,
        &get_definition,
        &get_payload(&get_definition, &case_id),
        TENANT_A,
        false,
    )
    .await;
    assert_eq!(unauthenticated.status(), reqwest::StatusCode::UNAUTHORIZED);
    let unauthenticated_body: Value = unauthenticated
        .json()
        .await
        .expect("decode unauthenticated case-get response");
    assert_eq!(
        unauthenticated_body,
        serde_json::json!({"error": "request_failed"})
    );
    assert_safe_text(&unauthenticated_body.to_string());
    assert_eq!(query_evidence_counts(&admin, TENANT_A).await, baseline);

    let outside_token = query(
        &mut grpc,
        &get_definition,
        get_payload(&get_definition, &case_id),
        TENANT_OUTSIDE_TOKEN,
        true,
    )
    .await
    .expect_err("tenant outside bearer grant must be denied before case lookup");
    assert_safe_status(&outside_token, Code::PermissionDenied, "TENANT_FORBIDDEN");
    assert_eq!(query_evidence_counts(&admin, TENANT_A).await, baseline);

    let visible = query(
        &mut grpc,
        &get_definition,
        get_payload(&get_definition, &case_id),
        TENANT_A,
        true,
    )
    .await
    .expect("query verified privacy case through generic gRPC ingress");
    let visible_case = decode_grpc_case(&visible);
    assert_eq!(
        visible_case
            .privacy_case_ref
            .as_ref()
            .expect("case-get response reference")
            .privacy_case_id,
        case_id
    );
    assert_eq!(
        visible_case.status,
        wire::PrivacyCaseStatus::SubjectVerified as i32
    );
    assert_eq!(visible_case.version, 3);
    assert!(
        visible_case.subject_binding.is_none(),
        "deployment field ceiling must redact subject binding"
    );
    assert_eq!(query_evidence_counts(&admin, TENANT_A).await, baseline);
    assert_record_version(&admin, TENANT_A, &case_id, 3).await;

    let http_visible = http_query(
        &http,
        &http_addr,
        &get_definition,
        &get_payload(&get_definition, &case_id),
        TENANT_A,
        true,
    )
    .await;
    assert_eq!(http_visible.status(), reqwest::StatusCode::OK);
    let http_payload: TypedPayload = http_visible
        .json()
        .await
        .expect("decode governed HTTP query payload");
    let http_case = decode_typed_case(&http_payload);
    assert_eq!(http_case, visible_case);
    assert_eq!(query_evidence_counts(&admin, TENANT_A).await, baseline);

    let cross_tenant = query(
        &mut grpc,
        &get_definition,
        get_payload(&get_definition, &case_id),
        TENANT_B,
        true,
    )
    .await
    .expect_err("cross-tenant privacy case must be concealed");
    assert_safe_status(
        &cross_tenant,
        Code::NotFound,
        "CUSTOMER_PRIVACY_CASE_NOT_FOUND",
    );
    assert_eq!(query_evidence_counts(&admin, TENANT_A).await, baseline);

    let missing = query(
        &mut grpc,
        &get_definition,
        get_payload(&get_definition, "privacy-case-get-missing"),
        TENANT_A,
        true,
    )
    .await
    .expect_err("missing privacy case must be concealed");
    assert_safe_status(&missing, Code::NotFound, "CUSTOMER_PRIVACY_CASE_NOT_FOUND");
    assert_eq!(query_evidence_counts(&admin, TENANT_A).await, baseline);

    set_module_status(&admin, TENANT_A, "suspended").await;
    let inactive = query(
        &mut grpc,
        &get_definition,
        get_payload(&get_definition, &case_id),
        TENANT_A,
        true,
    )
    .await
    .expect_err("inactive Customer Privacy module must reject case reads");
    assert_safe_status(&inactive, Code::Aborted, "MODULE_NOT_ACTIVE");
    assert_eq!(query_evidence_counts(&admin, TENANT_A).await, baseline);
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
    let authorization_denied = query(
        &mut denied_grpc,
        &get_definition,
        get_payload(&get_definition, &case_id),
        TENANT_A,
        true,
    )
    .await
    .expect_err("authenticated query without live grant must fail");
    assert_safe_status(
        &authorization_denied,
        Code::PermissionDenied,
        "QUERY_PERMISSION_DENIED",
    );
    assert_record_version(&admin, TENANT_A, &case_id, 3).await;
    assert_eq!(query_evidence_counts(&admin, TENANT_A).await, baseline);
    stop_process(&mut denied_process).await;
}

async fn create_party(
    grpc: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: &CapabilityDefinition,
    tenant: &str,
    party_id: &str,
    idempotency_key: &str,
) {
    mutate(
        grpc,
        definition,
        payload(
            definition,
            parties_wire::CreatePartyRequest {
                party_ref: Some(customer_wire::PartyRef {
                    party_id: party_id.to_owned(),
                }),
                kind: parties_wire::PartyKind::Person as i32,
                display_name: "Privacy case-get process fixture".to_owned(),
            },
        ),
        tenant,
        idempotency_key,
        true,
    )
    .await
    .expect("create Party through generic ingress");
}

async fn create_submit_and_verify_case(
    grpc: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    create_definition: &CapabilityDefinition,
    submit_definition: &CapabilityDefinition,
    verify_definition: &CapabilityDefinition,
) -> String {
    let created = mutate(
        grpc,
        create_definition,
        payload(
            create_definition,
            wire::CreatePrivacyCaseRequest {
                kind: wire::PrivacyCaseKind::Erasure as i32,
                policy_version: "privacy-policy/1".to_owned(),
                previous_privacy_case_ref: None,
            },
        ),
        TENANT_A,
        "privacy-case-get-create",
        true,
    )
    .await
    .expect("create case through generic ingress");
    let created_case = wire::CreatePrivacyCaseResponse::decode(
        created
            .output
            .as_ref()
            .expect("create output")
            .payload
            .as_slice(),
    )
    .expect("decode create response")
    .privacy_case
    .expect("create response case");
    let case_id = created_case
        .privacy_case_ref
        .expect("create response reference")
        .privacy_case_id;

    mutate(
        grpc,
        submit_definition,
        payload(
            submit_definition,
            wire::SubmitPrivacyCaseRequest {
                privacy_case_ref: Some(wire::PrivacyCaseRef {
                    privacy_case_id: case_id.clone(),
                }),
                expected_version: 1,
            },
        ),
        TENANT_A,
        "privacy-case-get-submit",
        true,
    )
    .await
    .expect("submit case through generic ingress");

    let verified = mutate(
        grpc,
        verify_definition,
        payload(
            verify_definition,
            wire::VerifyPrivacyCaseSubjectRequest {
                privacy_case_ref: Some(wire::PrivacyCaseRef {
                    privacy_case_id: case_id.clone(),
                }),
                expected_version: 2,
                submitted_party_ref: Some(customer_wire::PartyRef {
                    party_id: PARTY_A.to_owned(),
                }),
                canonical_party_ref: Some(customer_wire::PartyRef {
                    party_id: PARTY_A.to_owned(),
                }),
                identity_resolution_generation: 1,
                verification_method: wire::SubjectVerificationMethod::VerifiedDocument as i32,
            },
        ),
        TENANT_A,
        "privacy-case-get-verify",
        true,
    )
    .await
    .expect("verify case subject through generic ingress");
    let verified_case = wire::VerifyPrivacyCaseSubjectResponse::decode(
        verified
            .output
            .as_ref()
            .expect("verify output")
            .payload
            .as_slice(),
    )
    .expect("decode verify response")
    .privacy_case
    .expect("verify response case");
    assert!(verified_case.subject_binding.is_some());
    case_id
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

fn decode_grpc_case(
    response: &crm_application_runtime::gateway_v1::QueryResponse,
) -> wire::PrivacyCase {
    wire::GetPrivacyCaseResponse::decode(
        response
            .output
            .as_ref()
            .expect("case-get output")
            .payload
            .as_slice(),
    )
    .expect("decode exact GetPrivacyCaseResponse")
    .privacy_case
    .expect("case-get response contains case")
}

fn decode_typed_case(output: &TypedPayload) -> wire::PrivacyCase {
    wire::GetPrivacyCaseResponse::decode(output.bytes.as_slice())
        .expect("decode HTTP GetPrivacyCaseResponse")
        .privacy_case
        .expect("HTTP case-get response contains case")
}

async fn http_query(
    client: &HttpClient,
    http_addr: &str,
    definition: &CapabilityDefinition,
    input: &TypedPayload,
    tenant_id: &str,
    authenticated: bool,
) -> HttpResponse {
    let mut request = client
        .post(format!(
            "http://{http_addr}/v1/queries/{}/{}/{}",
            definition.owner_module_id, definition.capability_id, definition.capability_version
        ))
        .header("x-tenant-id", tenant_id)
        .json(input);
    if authenticated {
        request = request.bearer_auth(support::TOKEN);
    }
    request.send().await.expect("send HTTP query")
}

async fn query_evidence_counts(pool: &PgPool, tenant: &str) -> QueryEvidenceCounts {
    QueryEvidenceCounts {
        audits: count(
            pool,
            tenant,
            "SELECT count(*) FROM crm.audit_records WHERE tenant_id = $1 AND capability_id = 'customer_privacy.case.get'",
        )
        .await,
        idempotency: count(
            pool,
            tenant,
            "SELECT count(*) FROM crm.idempotency_records WHERE tenant_id = $1 AND idempotency_scope LIKE 'capability:customer_privacy.case.get:%'",
        )
        .await,
        transactions: count(
            pool,
            tenant,
            "SELECT count(*) FROM crm.business_transactions WHERE tenant_id = $1 AND capability_id = 'customer_privacy.case.get'",
        )
        .await,
    }
}

async fn count(pool: &PgPool, tenant: &str, sql: &'static str) -> i64 {
    sqlx::query_scalar(sql)
        .bind(tenant)
        .fetch_one(pool)
        .await
        .expect("read case-get evidence count")
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
    .expect("read privacy-case version");
    assert_eq!(actual, version);
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
        ("app.actor_id", "customer-privacy-case-get-process-admin"),
        (
            "app.request_id",
            "customer-privacy-case-get-process-activation",
        ),
        ("app.capability_id", "customer_privacy.process.activation"),
        ("app.capability_version", "1.0.0"),
        ("app.business_transaction_id", transaction_id.as_str()),
    ] {
        sqlx::query("SELECT set_config($1, $2, true)")
            .bind(name)
            .bind(value)
            .execute(&mut *transaction)
            .await
            .expect("bind case-get activation update context");
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
        .expect("commit case-get activation update");
}

fn assert_safe_status(status: &Status, expected_code: Code, expected_error_code: &str) {
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
        "false"
    );
    assert_safe_text(status.message());
    assert_safe_text(&format!("{:?}", status.metadata()));
}

fn assert_safe_text(value: &str) {
    for forbidden in [
        RAW_MARKER,
        PARTY_A,
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
            "safe case-get transport surface leaked {forbidden}: {value}"
        );
    }
}
