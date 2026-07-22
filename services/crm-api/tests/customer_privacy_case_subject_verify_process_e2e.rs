#![cfg(unix)]

#[path = "support/customer_enrichment_process/mod.rs"]
mod support;

use crm_application_runtime::gateway_v1::application_gateway_service_client::ApplicationGatewayServiceClient;
use crm_capability_runtime::CapabilityDefinition;
use crm_customer_privacy_subject_capability_adapter::capability_definition as subject_definition;
use crm_module_sdk::TypedPayload;
use crm_proto_contracts::crm::{
    customer::v1 as customer_wire, customer_privacy::v1 as wire, parties::v1 as parties_wire,
};
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
const PARTY_CREATE: &str = "parties.party.create";
const CREATE_CASE: &str = "customer_privacy.case.create";
const SUBMIT_CASE: &str = "customer_privacy.case.submit";
const VERIFY_SUBJECT: &str = "customer_privacy.case.subject.verify";
const RECORD_TYPE: &str = "customer-privacy.case";
const SUBJECT_EVENT: &str = "customer_privacy.case.subject_verified";
const SUBJECT_SCOPE: &str = "capability:customer_privacy.case.subject.verify:1.0.0";
const PARTY_A: &str = "privacy-subject-process-party-a";
const RAW_MARKER: &str = "raw-privacy-subject-payload-must-not-leak";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SubjectEvidenceCounts {
    events: i64,
    audits: i64,
    idempotency: i64,
    transactions: i64,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn customer_privacy_subject_verify_real_process_is_bounded_and_replay_safe() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!(
            "skipping Customer Privacy subject-verification crm-api test because DATABASE_URL is absent"
        );
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect subject-verification evidence reader");

    let party_definition = mutation_definition(PARTY_CREATE);
    let create_definition = mutation_definition(CREATE_CASE);
    let submit_definition = mutation_definition(SUBMIT_CASE);
    let verify_definition =
        subject_definition().expect("construct subject verification definition");
    assert_eq!(verify_definition.owner_module_id.as_str(), PRIVACY_MODULE);
    assert_eq!(verify_definition.capability_id.as_str(), VERIFY_SUBJECT);

    let http_addr = format!("127.0.0.1:{}", free_port());
    let grpc_addr = format!("127.0.0.1:{}", free_port());
    let http = HttpClient::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("build subject-verification HTTP client");
    let mut process = spawn_crm_api(&database_url, &http_addr, &grpc_addr, true, None);
    wait_until_ready(&http, &mut process, &http_addr, true).await;
    let mut grpc = connect_grpc(&grpc_addr).await;

    let initial_a = subject_evidence_counts(&admin, TENANT_A).await;
    let initial_b = subject_evidence_counts(&admin, TENANT_B).await;

    let unauthenticated = http_mutate(
        &http,
        &http_addr,
        &verify_definition,
        &verify_payload(
            &verify_definition,
            "privacy-case-hidden",
            2,
            PARTY_A,
            PARTY_A,
            1,
        ),
        TENANT_A,
        "privacy-subject-process-unauthenticated",
        false,
    )
    .await;
    assert_eq!(unauthenticated.status(), reqwest::StatusCode::UNAUTHORIZED);
    let unauthenticated_body: Value = unauthenticated
        .json()
        .await
        .expect("decode unauthenticated subject response");
    assert_eq!(
        unauthenticated_body,
        serde_json::json!({"error": "request_failed"})
    );
    assert_safe_text(&unauthenticated_body.to_string());
    assert_eq!(subject_evidence_counts(&admin, TENANT_A).await, initial_a);

    let outside_token = mutate(
        &mut grpc,
        &verify_definition,
        verify_payload(
            &verify_definition,
            "privacy-case-hidden",
            2,
            PARTY_A,
            PARTY_A,
            1,
        ),
        TENANT_OUTSIDE_TOKEN,
        "privacy-subject-process-outside-token",
        true,
    )
    .await
    .expect_err("tenant outside bearer grant must be denied before subject lookup");
    assert_safe_status(
        &outside_token,
        Code::PermissionDenied,
        "TENANT_FORBIDDEN",
        false,
    );
    assert_eq!(subject_evidence_counts(&admin, TENANT_A).await, initial_a);

    create_party(
        &mut grpc,
        &party_definition,
        TENANT_A,
        PARTY_A,
        "privacy-subject-process-party-a-create",
    )
    .await;
    create_party(
        &mut grpc,
        &party_definition,
        TENANT_A,
        RAW_MARKER,
        "privacy-subject-process-secret-party-create",
    )
    .await;

    let success_case = create_and_submit_case(
        &mut grpc,
        &create_definition,
        &submit_definition,
        TENANT_A,
        "privacy-subject-process-success",
    )
    .await;
    let verify_key = "privacy-subject-process-verify";
    let first = mutate(
        &mut grpc,
        &verify_definition,
        verify_payload(&verify_definition, &success_case, 2, PARTY_A, PARTY_A, 1),
        TENANT_A,
        verify_key,
        true,
    )
    .await
    .expect("verify privacy subject through generic gRPC ingress");
    let first_case = decode_verify_case(&first);
    assert_eq!(
        first_case.status,
        wire::PrivacyCaseStatus::SubjectVerified as i32
    );
    assert_eq!(first_case.version, 3);
    let binding = first_case
        .subject_binding
        .as_ref()
        .expect("verified process response contains subject binding");
    assert_eq!(
        binding
            .submitted_party_ref
            .as_ref()
            .expect("submitted Party response reference")
            .party_id,
        PARTY_A
    );
    assert_eq!(
        binding
            .canonical_party_ref
            .as_ref()
            .expect("canonical Party response reference")
            .party_id,
        PARTY_A
    );
    assert_eq!(binding.identity_resolution_generation, 1);
    assert_record_version(&admin, TENANT_A, &success_case, 3).await;
    let committed_a = subject_evidence_counts(&admin, TENANT_A).await;
    assert_eq!(committed_a.events, initial_a.events + 1);
    assert_eq!(committed_a.audits, initial_a.audits + 1);
    assert_eq!(committed_a.idempotency, initial_a.idempotency + 1);
    assert_eq!(committed_a.transactions, initial_a.transactions + 1);

    let replay = mutate(
        &mut grpc,
        &verify_definition,
        verify_payload(&verify_definition, &success_case, 2, PARTY_A, PARTY_A, 1),
        TENANT_A,
        verify_key,
        true,
    )
    .await
    .expect("exact subject-verification replay returns committed response");
    assert_eq!(decode_verify_case(&replay), first_case);
    assert_record_version(&admin, TENANT_A, &success_case, 3).await;
    assert_eq!(subject_evidence_counts(&admin, TENANT_A).await, committed_a);

    let conflicting_http = http_mutate(
        &http,
        &http_addr,
        &verify_definition,
        &verify_payload(&verify_definition, &success_case, 3, PARTY_A, PARTY_A, 1),
        TENANT_A,
        verify_key,
        true,
    )
    .await;
    assert!(conflicting_http.status().is_client_error());
    let conflict_body = conflicting_http
        .text()
        .await
        .expect("read conflicting subject HTTP response");
    assert_safe_text(&conflict_body);
    assert_eq!(subject_evidence_counts(&admin, TENANT_A).await, committed_a);

    let conflicting_grpc = mutate(
        &mut grpc,
        &verify_definition,
        verify_payload(&verify_definition, &success_case, 3, PARTY_A, PARTY_A, 1),
        TENANT_A,
        verify_key,
        true,
    )
    .await
    .expect_err("conflicting subject-verification replay must fail closed");
    assert_safe_status(
        &conflicting_grpc,
        Code::Aborted,
        "CAPABILITY_IDEMPOTENCY_KEY_REUSED",
        false,
    );
    assert_eq!(subject_evidence_counts(&admin, TENANT_A).await, committed_a);

    let stale_case = create_and_submit_case(
        &mut grpc,
        &create_definition,
        &submit_definition,
        TENANT_A,
        "privacy-subject-process-stale-generation",
    )
    .await;
    let stale_key = "privacy-subject-process-stale-generation-verify";
    let stale_http = http_mutate(
        &http,
        &http_addr,
        &verify_definition,
        &verify_payload(&verify_definition, &stale_case, 2, PARTY_A, PARTY_A, 2),
        TENANT_A,
        stale_key,
        true,
    )
    .await;
    assert!(stale_http.status().is_client_error());
    let stale_body = stale_http
        .text()
        .await
        .expect("read stale-generation HTTP response");
    assert_safe_text(&stale_body);
    assert_record_version(&admin, TENANT_A, &stale_case, 2).await;
    assert_eq!(subject_evidence_counts(&admin, TENANT_A).await, committed_a);

    let stale_grpc = mutate(
        &mut grpc,
        &verify_definition,
        verify_payload(&verify_definition, &stale_case, 2, PARTY_A, PARTY_A, 2),
        TENANT_A,
        stale_key,
        true,
    )
    .await
    .expect_err("stale Identity Resolution generation must fail closed");
    assert_safe_status(
        &stale_grpc,
        Code::Aborted,
        "CUSTOMER_PRIVACY_SUBJECT_GENERATION_STALE",
        true,
    );
    assert_record_version(&admin, TENANT_A, &stale_case, 2).await;
    assert_eq!(subject_evidence_counts(&admin, TENANT_A).await, committed_a);

    let tenant_b_case = create_and_submit_case(
        &mut grpc,
        &create_definition,
        &submit_definition,
        TENANT_B,
        "privacy-subject-process-cross-tenant",
    )
    .await;
    let concealed = mutate(
        &mut grpc,
        &verify_definition,
        verify_payload(
            &verify_definition,
            &tenant_b_case,
            2,
            RAW_MARKER,
            RAW_MARKER,
            1,
        ),
        TENANT_B,
        "privacy-subject-process-cross-tenant-verify",
        true,
    )
    .await
    .expect_err("tenant B must not observe tenant A Party references");
    assert_safe_status(
        &concealed,
        Code::NotFound,
        "CUSTOMER_PRIVACY_SUBJECT_REFERENCE_UNAVAILABLE",
        false,
    );
    assert_record_version(&admin, TENANT_B, &tenant_b_case, 2).await;
    assert_eq!(subject_evidence_counts(&admin, TENANT_B).await, initial_b);

    let inactive_case = create_and_submit_case(
        &mut grpc,
        &create_definition,
        &submit_definition,
        TENANT_A,
        "privacy-subject-process-inactive",
    )
    .await;
    set_module_status(&admin, TENANT_A, "suspended").await;
    let inactive = mutate(
        &mut grpc,
        &verify_definition,
        verify_payload(&verify_definition, &inactive_case, 2, PARTY_A, PARTY_A, 1),
        TENANT_A,
        "privacy-subject-process-inactive-verify",
        true,
    )
    .await
    .expect_err("inactive Customer Privacy module must reject subject verification");
    assert_safe_status(&inactive, Code::Aborted, "MODULE_NOT_ACTIVE", false);
    assert_record_version(&admin, TENANT_A, &inactive_case, 2).await;
    assert_eq!(subject_evidence_counts(&admin, TENANT_A).await, committed_a);
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
        &verify_definition,
        verify_payload(&verify_definition, &inactive_case, 2, PARTY_A, PARTY_A, 1),
        TENANT_A,
        "privacy-subject-process-no-capability-grant",
        true,
    )
    .await
    .expect_err("authenticated subject verification without live grant must fail");
    assert_safe_status(
        &authorization_denied,
        Code::PermissionDenied,
        "CAPABILITY_PERMISSION_DENIED",
        false,
    );
    assert_record_version(&admin, TENANT_A, &inactive_case, 2).await;
    assert_eq!(subject_evidence_counts(&admin, TENANT_A).await, committed_a);
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
                display_name: "Privacy subject process fixture".to_owned(),
            },
        ),
        tenant,
        idempotency_key,
        true,
    )
    .await
    .expect("create Party through generic crm-api ingress");
}

async fn create_and_submit_case(
    grpc: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    create_definition: &CapabilityDefinition,
    submit_definition: &CapabilityDefinition,
    tenant: &str,
    identity: &str,
) -> String {
    let created = mutate(
        grpc,
        create_definition,
        create_payload(create_definition),
        tenant,
        &format!("{identity}-create"),
        true,
    )
    .await
    .expect("create privacy case through generic crm-api ingress");
    let created_case = decode_create_case(&created);
    let case_id = created_case
        .privacy_case_ref
        .as_ref()
        .expect("created privacy case reference")
        .privacy_case_id
        .clone();
    assert_eq!(created_case.status, wire::PrivacyCaseStatus::Draft as i32);
    assert_eq!(created_case.version, 1);

    let submitted = mutate(
        grpc,
        submit_definition,
        submit_payload(submit_definition, &case_id, 1),
        tenant,
        &format!("{identity}-submit"),
        true,
    )
    .await
    .expect("submit privacy case through generic crm-api ingress");
    let submitted_case = decode_submit_case(&submitted);
    assert_eq!(
        submitted_case.status,
        wire::PrivacyCaseStatus::Submitted as i32
    );
    assert_eq!(submitted_case.version, 2);
    case_id
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

fn verify_payload(
    definition: &CapabilityDefinition,
    privacy_case_id: &str,
    expected_version: i64,
    submitted_party_id: &str,
    canonical_party_id: &str,
    generation: u64,
) -> TypedPayload {
    payload(
        definition,
        wire::VerifyPrivacyCaseSubjectRequest {
            privacy_case_ref: Some(wire::PrivacyCaseRef {
                privacy_case_id: privacy_case_id.to_owned(),
            }),
            expected_version,
            submitted_party_ref: Some(customer_wire::PartyRef {
                party_id: submitted_party_id.to_owned(),
            }),
            canonical_party_ref: Some(customer_wire::PartyRef {
                party_id: canonical_party_id.to_owned(),
            }),
            identity_resolution_generation: generation,
            verification_method: wire::SubjectVerificationMethod::VerifiedDocument as i32,
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

fn decode_verify_case(
    response: &crm_application_runtime::gateway_v1::MutateResponse,
) -> wire::PrivacyCase {
    wire::VerifyPrivacyCaseSubjectResponse::decode(
        response
            .output
            .as_ref()
            .expect("subject-verification output")
            .payload
            .as_slice(),
    )
    .expect("decode exact VerifyPrivacyCaseSubjectResponse")
    .privacy_case
    .expect("subject-verification response contains privacy case")
}

async fn subject_evidence_counts(pool: &PgPool, tenant: &str) -> SubjectEvidenceCounts {
    SubjectEvidenceCounts {
        events: count(
            pool,
            tenant,
            "SELECT count(*) FROM crm.outbox_events WHERE tenant_id = $1 AND event_type = 'customer_privacy.case.subject_verified'",
        )
        .await,
        audits: count(
            pool,
            tenant,
            "SELECT count(*) FROM crm.audit_records WHERE tenant_id = $1 AND capability_id = 'customer_privacy.case.subject.verify'",
        )
        .await,
        idempotency: count(
            pool,
            tenant,
            "SELECT count(*) FROM crm.idempotency_records WHERE tenant_id = $1 AND idempotency_scope = 'capability:customer_privacy.case.subject.verify:1.0.0'",
        )
        .await,
        transactions: count(
            pool,
            tenant,
            "SELECT count(*) FROM crm.business_transactions WHERE tenant_id = $1 AND capability_id = 'customer_privacy.case.subject.verify'",
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
    .expect("read privacy-case version");
    assert_eq!(actual, version);
}

async fn count(pool: &PgPool, tenant: &str, sql: &'static str) -> i64 {
    sqlx::query_scalar(sql)
        .bind(tenant)
        .fetch_one(pool)
        .await
        .expect("read Customer Privacy subject evidence count")
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
        ("app.actor_id", "customer-privacy-subject-process-admin"),
        (
            "app.request_id",
            "customer-privacy-subject-process-activation",
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
            .expect("bind subject activation update context");
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
        .expect("commit subject activation update");
}

fn assert_safe_status(
    status: &Status,
    expected_code: Code,
    expected_error_code: &str,
    expected_retryable: bool,
) {
    assert_eq!(status.code(), expected_code);
    assert_error_code(status, expected_error_code);
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
        "crm.relationships",
        "identity_resolution_topology_generations",
        "payload_bytes",
        "descriptor_hash",
        "sqlx",
        "SELECT",
        "postgres://",
        RECORD_TYPE,
        SUBJECT_SCOPE,
        SUBJECT_EVENT,
    ] {
        assert!(
            !value.contains(forbidden),
            "safe subject-verification transport surface leaked {forbidden}: {value}"
        );
    }
}
