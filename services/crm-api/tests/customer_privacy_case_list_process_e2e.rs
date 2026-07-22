#![cfg(unix)]

#[path = "support/customer_enrichment_process/mod.rs"]
mod support;

use crm_application_runtime::gateway_v1::application_gateway_service_client::ApplicationGatewayServiceClient;
use crm_capability_runtime::CapabilityDefinition;
use crm_customer_privacy_cancel_capability_adapter::capability_definition as cancel_definition;
use crm_customer_privacy_query_adapter::list_privacy_cases_capability_definition;
use crm_customer_privacy_subject_capability_adapter::capability_definition as subject_definition;
use crm_module_sdk::TypedPayload;
use crm_proto_contracts::crm::{
    customer::v1 as customer_wire, customer_privacy::v1 as wire, parties::v1 as parties_wire,
};
use prost::Message;
use reqwest::{Client as HttpClient, Response as HttpResponse};
use serde_json::Value;
use sqlx::{PgPool, Row};
use std::collections::BTreeSet;
use tonic::{Code, Status};

use support::{
    TENANT_A, TENANT_B, TENANT_OUTSIDE_TOKEN, connect_grpc, free_port, mutate,
    mutation_definition, payload, query, spawn_crm_api, stop_process, wait_until_ready,
};

const PRIVACY_MODULE: &str = "crm.customer-privacy";
const PARTY_CREATE: &str = "parties.party.create";
const CREATE_CASE: &str = "customer_privacy.case.create";
const SUBMIT_CASE: &str = "customer_privacy.case.submit";
const LIST_CASES: &str = "customer_privacy.case.list";
const RECORD_TYPE: &str = "customer-privacy.case";
const PARTY_A: &str = "privacy-case-list-party-a";
const PARTY_B: &str = "privacy-case-list-party-b";
const RAW_MARKER: &str = "raw-privacy-case-list-payload-must-not-leak";
const HIDDEN_SUBJECT_BINDING: &str =
    "customer_privacy.case.list|crm.customer-privacy|customer-privacy.case|subject_binding";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct QueryEvidenceCounts {
    audits: i64,
    idempotency: i64,
    transactions: i64,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn customer_privacy_case_list_real_process_is_bounded_permission_aware_and_side_effect_free() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping Customer Privacy case-list crm-api test because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect case-list evidence reader");

    let party_definition = mutation_definition(PARTY_CREATE);
    let create_definition = mutation_definition(CREATE_CASE);
    let submit_definition = mutation_definition(SUBMIT_CASE);
    let verify_definition = subject_definition().expect("construct subject verification definition");
    let cancel_definition = cancel_definition().expect("construct cancellation definition");
    let list_definition =
        list_privacy_cases_capability_definition().expect("construct case-list definition");
    assert_eq!(list_definition.owner_module_id.as_str(), PRIVACY_MODULE);
    assert_eq!(list_definition.capability_id.as_str(), LIST_CASES);

    let http_addr = format!("127.0.0.1:{}", free_port());
    let grpc_addr = format!("127.0.0.1:{}", free_port());
    let http = HttpClient::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("build case-list HTTP client");
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
        PARTY_A,
        "privacy-case-list-party-a-create",
    )
    .await;
    create_party(
        &mut grpc,
        &party_definition,
        PARTY_B,
        "privacy-case-list-party-b-create",
    )
    .await;

    let cancelled_case = create_submit_verify_case(
        &mut grpc,
        &create_definition,
        &submit_definition,
        &verify_definition,
        PARTY_A,
        wire::PrivacyCaseKind::Erasure,
        "privacy-case-list-a-erasure",
    )
    .await;
    cancel_case(
        &mut grpc,
        &cancel_definition,
        &cancelled_case,
        "privacy-case-list-a-erasure-cancel",
    )
    .await;
    let access_case = create_submit_verify_case(
        &mut grpc,
        &create_definition,
        &submit_definition,
        &verify_definition,
        PARTY_A,
        wire::PrivacyCaseKind::Access,
        "privacy-case-list-a-access",
    )
    .await;
    let other_subject_case = create_submit_verify_case(
        &mut grpc,
        &create_definition,
        &submit_definition,
        &verify_definition,
        PARTY_B,
        wire::PrivacyCaseKind::Erasure,
        "privacy-case-list-b-erasure",
    )
    .await;

    assert_record_version(&admin, &cancelled_case, 4).await;
    assert_record_version(&admin, &access_case, 3).await;
    assert_record_version(&admin, &other_subject_case, 3).await;
    let baseline = query_evidence_counts(&admin).await;

    let unauthenticated = http_query(
        &http,
        &http_addr,
        &list_definition,
        &list_payload(&list_definition, PARTY_A, None, None, 1, ""),
        TENANT_A,
        false,
    )
    .await;
    assert_eq!(unauthenticated.status(), reqwest::StatusCode::UNAUTHORIZED);
    let unauthenticated_body: Value = unauthenticated
        .json()
        .await
        .expect("decode unauthenticated case-list response");
    assert_eq!(
        unauthenticated_body,
        serde_json::json!({"error": "request_failed"})
    );
    assert_safe_text(&unauthenticated_body.to_string());
    assert_eq!(query_evidence_counts(&admin).await, baseline);

    let outside_token = query(
        &mut grpc,
        &list_definition,
        list_payload(&list_definition, PARTY_A, None, None, 1, ""),
        TENANT_OUTSIDE_TOKEN,
        true,
    )
    .await
    .expect_err("tenant outside bearer grant must be denied before case-list scan");
    assert_safe_status(&outside_token, Code::PermissionDenied, "TENANT_FORBIDDEN", false);

    let first = query(
        &mut grpc,
        &list_definition,
        list_payload(&list_definition, PARTY_A, None, None, 1, ""),
        TENANT_A,
        true,
    )
    .await
    .expect("query first privacy case page through generic gRPC ingress");
    let first_page = decode_grpc_list(&first);
    assert_eq!(first_page.privacy_cases.len(), 1);
    assert!(!first_page.next_cursor.is_empty());
    assert!(first_page.privacy_cases[0].subject_binding.is_none());

    let second = query(
        &mut grpc,
        &list_definition,
        list_payload(
            &list_definition,
            PARTY_A,
            None,
            None,
            1,
            &first_page.next_cursor,
        ),
        TENANT_A,
        true,
    )
    .await
    .expect("query second privacy case page through generic gRPC ingress");
    let second_page = decode_grpc_list(&second);
    assert_eq!(second_page.privacy_cases.len(), 1);
    assert!(second_page.next_cursor.is_empty());
    assert!(second_page.privacy_cases[0].subject_binding.is_none());

    let listed_ids = first_page
        .privacy_cases
        .iter()
        .chain(second_page.privacy_cases.iter())
        .map(case_id)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        listed_ids,
        BTreeSet::from([cancelled_case.as_str(), access_case.as_str()])
    );
    assert!(!listed_ids.contains(other_subject_case.as_str()));

    let access_filter = query(
        &mut grpc,
        &list_definition,
        list_payload(
            &list_definition,
            PARTY_A,
            Some(wire::PrivacyCaseKind::Access),
            None,
            10,
            "",
        ),
        TENANT_A,
        true,
    )
    .await
    .expect("filter privacy case list by kind");
    let access_filter = decode_grpc_list(&access_filter);
    assert_eq!(access_filter.privacy_cases.len(), 1);
    assert_eq!(case_id(&access_filter.privacy_cases[0]), access_case);

    let cancelled_filter = query(
        &mut grpc,
        &list_definition,
        list_payload(
            &list_definition,
            PARTY_A,
            None,
            Some(wire::PrivacyCaseStatus::Cancelled),
            10,
            "",
        ),
        TENANT_A,
        true,
    )
    .await
    .expect("filter privacy case list by status");
    let cancelled_filter = decode_grpc_list(&cancelled_filter);
    assert_eq!(cancelled_filter.privacy_cases.len(), 1);
    assert_eq!(case_id(&cancelled_filter.privacy_cases[0]), cancelled_case);

    let http_visible = http_query(
        &http,
        &http_addr,
        &list_definition,
        &list_payload(&list_definition, PARTY_A, None, None, 10, ""),
        TENANT_A,
        true,
    )
    .await;
    assert_eq!(http_visible.status(), reqwest::StatusCode::OK);
    let http_payload: TypedPayload = http_visible
        .json()
        .await
        .expect("decode governed HTTP case-list payload");
    let http_page = decode_typed_list(&http_payload);
    assert_eq!(
        http_page
            .privacy_cases
            .iter()
            .map(case_id)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([cancelled_case.as_str(), access_case.as_str()])
    );

    let mut tampered = first_page.next_cursor.clone();
    tampered.push('x');
    let tampered_cursor = query(
        &mut grpc,
        &list_definition,
        list_payload(&list_definition, PARTY_A, None, None, 1, &tampered),
        TENANT_A,
        true,
    )
    .await
    .expect_err("tampered cursor must fail closed");
    assert_safe_status(
        &tampered_cursor,
        Code::InvalidArgument,
        "CUSTOMER_PRIVACY_CASE_LIST_CURSOR_INVALID",
        false,
    );

    let rebound_cursor = query(
        &mut grpc,
        &list_definition,
        list_payload(
            &list_definition,
            PARTY_A,
            Some(wire::PrivacyCaseKind::Access),
            None,
            1,
            &first_page.next_cursor,
        ),
        TENANT_A,
        true,
    )
    .await
    .expect_err("cursor may not be rebound to different filters");
    assert_safe_status(
        &rebound_cursor,
        Code::InvalidArgument,
        "CUSTOMER_PRIVACY_CASE_LIST_CURSOR_INVALID",
        false,
    );

    let cross_tenant = query(
        &mut grpc,
        &list_definition,
        list_payload(&list_definition, PARTY_A, None, None, 10, ""),
        TENANT_B,
        true,
    )
    .await
    .expect("cross-tenant subject scope must be uniformly concealed as an empty page");
    let cross_tenant = decode_grpc_list(&cross_tenant);
    assert!(cross_tenant.privacy_cases.is_empty());
    assert!(cross_tenant.next_cursor.is_empty());

    assert_record_version(&admin, &cancelled_case, 4).await;
    assert_record_version(&admin, &access_case, 3).await;
    assert_record_version(&admin, &other_subject_case, 3).await;
    assert_eq!(query_evidence_counts(&admin).await, baseline);

    set_module_status(&admin, "suspended").await;
    let inactive = query(
        &mut grpc,
        &list_definition,
        list_payload(&list_definition, PARTY_A, None, None, 10, ""),
        TENANT_A,
        true,
    )
    .await
    .expect_err("inactive Customer Privacy module must reject case listing");
    assert_safe_status(&inactive, Code::Aborted, "MODULE_NOT_ACTIVE", false);
    stop_process(&mut process).await;
    set_module_status(&admin, "active").await;

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
    let mut denied_grpc = connect_grpc(&denied_grpc_addr).await;
    let authorization_denied = query(
        &mut denied_grpc,
        &list_definition,
        list_payload(&list_definition, PARTY_A, None, None, 10, ""),
        TENANT_A,
        true,
    )
    .await
    .expect_err("authenticated case-list query without live grant must fail");
    assert_safe_status(
        &authorization_denied,
        Code::PermissionDenied,
        "QUERY_PERMISSION_DENIED",
        false,
    );
    assert_eq!(query_evidence_counts(&admin).await, baseline);
    stop_process(&mut denied_process).await;
}

async fn create_party(
    grpc: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: &CapabilityDefinition,
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
                display_name: "Privacy case-list process fixture".to_owned(),
            },
        ),
        TENANT_A,
        idempotency_key,
        true,
    )
    .await
    .expect("create Party through generic ingress");
}

async fn create_submit_verify_case(
    grpc: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    create_definition: &CapabilityDefinition,
    submit_definition: &CapabilityDefinition,
    verify_definition: &CapabilityDefinition,
    party_id: &str,
    kind: wire::PrivacyCaseKind,
    prefix: &str,
) -> String {
    let create_key = format!("{prefix}-create");
    let submit_key = format!("{prefix}-submit");
    let verify_key = format!("{prefix}-verify");
    let created = mutate(
        grpc,
        create_definition,
        payload(
            create_definition,
            wire::CreatePrivacyCaseRequest {
                kind: kind as i32,
                policy_version: "privacy-policy/1".to_owned(),
                previous_privacy_case_ref: None,
            },
        ),
        TENANT_A,
        &create_key,
        true,
    )
    .await
    .expect("create privacy case through generic ingress");
    let case_id = wire::CreatePrivacyCaseResponse::decode(
        created
            .output
            .as_ref()
            .expect("create output")
            .payload
            .as_slice(),
    )
    .expect("decode create response")
    .privacy_case
    .expect("created privacy case")
    .privacy_case_ref
    .expect("created privacy case reference")
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
        &submit_key,
        true,
    )
    .await
    .expect("submit privacy case through generic ingress");

    mutate(
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
                    party_id: party_id.to_owned(),
                }),
                canonical_party_ref: Some(customer_wire::PartyRef {
                    party_id: party_id.to_owned(),
                }),
                identity_resolution_generation: 1,
                verification_method: wire::SubjectVerificationMethod::VerifiedDocument as i32,
            },
        ),
        TENANT_A,
        &verify_key,
        true,
    )
    .await
    .expect("verify privacy case subject through generic ingress");
    case_id
}

async fn cancel_case(
    grpc: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: &CapabilityDefinition,
    case_id: &str,
    idempotency_key: &str,
) {
    let cancelled = mutate(
        grpc,
        definition,
        payload(
            definition,
            wire::CancelPrivacyCaseRequest {
                privacy_case_ref: Some(wire::PrivacyCaseRef {
                    privacy_case_id: case_id.to_owned(),
                }),
                expected_version: 3,
            },
        ),
        TENANT_A,
        idempotency_key,
        true,
    )
    .await
    .expect("cancel privacy case through generic ingress");
    let cancelled = wire::CancelPrivacyCaseResponse::decode(
        cancelled
            .output
            .as_ref()
            .expect("cancel output")
            .payload
            .as_slice(),
    )
    .expect("decode cancellation response")
    .privacy_case
    .expect("cancelled privacy case");
    assert_eq!(cancelled.status, wire::PrivacyCaseStatus::Cancelled as i32);
    assert_eq!(cancelled.version, 4);
}

fn list_payload(
    definition: &CapabilityDefinition,
    party_id: &str,
    kind: Option<wire::PrivacyCaseKind>,
    status: Option<wire::PrivacyCaseStatus>,
    page_size: i32,
    cursor: &str,
) -> TypedPayload {
    payload(
        definition,
        wire::ListPrivacyCasesRequest {
            canonical_party_ref: Some(customer_wire::PartyRef {
                party_id: party_id.to_owned(),
            }),
            kind: kind.map(|value| value as i32),
            status: status.map(|value| value as i32),
            page_size,
            cursor: cursor.to_owned(),
        },
    )
}

fn decode_grpc_list(
    response: &crm_application_runtime::gateway_v1::QueryResponse,
) -> wire::ListPrivacyCasesResponse {
    wire::ListPrivacyCasesResponse::decode(
        response
            .output
            .as_ref()
            .expect("case-list output")
            .payload
            .as_slice(),
    )
    .expect("decode exact ListPrivacyCasesResponse")
}

fn decode_typed_list(output: &TypedPayload) -> wire::ListPrivacyCasesResponse {
    wire::ListPrivacyCasesResponse::decode(output.bytes.as_slice())
        .expect("decode HTTP ListPrivacyCasesResponse")
}

fn case_id(value: &wire::PrivacyCase) -> &str {
    value
        .privacy_case_ref
        .as_ref()
        .expect("listed privacy case reference")
        .privacy_case_id
        .as_str()
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

async fn query_evidence_counts(pool: &PgPool) -> QueryEvidenceCounts {
    QueryEvidenceCounts {
        audits: count(
            pool,
            "SELECT count(*) FROM crm.audit_records WHERE tenant_id = $1 AND capability_id = 'customer_privacy.case.list'",
        )
        .await,
        idempotency: count(
            pool,
            "SELECT count(*) FROM crm.idempotency_records WHERE tenant_id = $1 AND idempotency_scope LIKE 'capability:customer_privacy.case.list:%'",
        )
        .await,
        transactions: count(
            pool,
            "SELECT count(*) FROM crm.business_transactions WHERE tenant_id = $1 AND capability_id = 'customer_privacy.case.list'",
        )
        .await,
    }
}

async fn count(pool: &PgPool, sql: &'static str) -> i64 {
    sqlx::query_scalar(sql)
        .bind(TENANT_A)
        .fetch_one(pool)
        .await
        .expect("read case-list evidence count")
}

async fn assert_record_version(pool: &PgPool, case_id: &str, version: i64) {
    let actual: i64 = sqlx::query_scalar(
        "SELECT version FROM crm.records WHERE tenant_id = $1 AND record_type = $2 AND record_id = $3",
    )
    .bind(TENANT_A)
    .bind(RECORD_TYPE)
    .bind(case_id)
    .fetch_one(pool)
    .await
    .expect("read privacy-case version");
    assert_eq!(actual, version);
}

async fn set_module_status(pool: &PgPool, status: &str) {
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
        ("app.actor_id", "customer-privacy-case-list-process-admin"),
        ("app.request_id", "customer-privacy-case-list-process-activation"),
        ("app.capability_id", "customer_privacy.process.activation"),
        ("app.capability_version", "1.0.0"),
        ("app.business_transaction_id", transaction_id.as_str()),
    ] {
        sqlx::query("SELECT set_config($1, $2, true)")
            .bind(name)
            .bind(value)
            .execute(&mut *transaction)
            .await
            .expect("bind case-list activation update context");
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
        .expect("commit case-list activation update");
}

fn assert_safe_status(
    status: &Status,
    expected_code: Code,
    expected_error_code: &str,
    retryable: bool,
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
        retryable.to_string()
    );
    assert_safe_text(status.message());
    assert_safe_text(&format!("{:?}", status.metadata()));
}

fn assert_safe_text(value: &str) {
    for forbidden in [
        RAW_MARKER,
        PARTY_A,
        PARTY_B,
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
            "safe case-list transport surface leaked {forbidden}: {value}"
        );
    }
}
