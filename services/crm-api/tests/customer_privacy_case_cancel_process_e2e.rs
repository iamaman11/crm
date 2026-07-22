#![cfg(unix)]

#[path = "support/customer_enrichment_process/mod.rs"]
mod support;

use crm_application_runtime::gateway_v1::application_gateway_service_client::ApplicationGatewayServiceClient;
use crm_capability_runtime::CapabilityDefinition;
use crm_module_sdk::TypedPayload;
use crm_proto_contracts::crm::{
    customer::v1 as customer_wire, customer_privacy::v1 as wire, parties::v1 as parties_wire,
};
use prost::Message;
use reqwest::Client as HttpClient;
use serde_json::Value;
use sqlx::{PgPool, Postgres, Row, Transaction};
use tonic::{Code, Status};

use support::{
    TENANT_A, TENANT_B, TENANT_OUTSIDE_TOKEN, connect_grpc, free_port, http_mutate, mutate,
    mutation_definition, payload, spawn_crm_api, stop_process, wait_until_ready,
};

const PRIVACY_MODULE: &str = "crm.customer-privacy";
const PARTY_CREATE: &str = "parties.party.create";
const CREATE_CASE: &str = "customer_privacy.case.create";
const SUBMIT_CASE: &str = "customer_privacy.case.submit";
const VERIFY_CASE: &str = "customer_privacy.case.subject.verify";
const CANCEL_CASE: &str = "customer_privacy.case.cancel";
const RECORD_TYPE: &str = "customer-privacy.case";
const PARTY_A: &str = "privacy-cancel-party-a";
const RAW_MARKER: &str = "raw-privacy-cancel-payload-must-not-leak";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CancelEvidence {
    events: i64,
    audits: i64,
    idempotency: i64,
    transactions: i64,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn customer_privacy_case_cancel_real_process_is_atomic_locked_and_replay_safe() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping Customer Privacy cancellation process test because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect cancellation evidence reader");

    let party_definition = mutation_definition(PARTY_CREATE);
    let create_definition = mutation_definition(CREATE_CASE);
    let submit_definition = mutation_definition(SUBMIT_CASE);
    let verify_definition = mutation_definition(VERIFY_CASE);
    let cancel_definition = mutation_definition(CANCEL_CASE);
    assert_eq!(cancel_definition.owner_module_id.as_str(), PRIVACY_MODULE);

    let http_addr = format!("127.0.0.1:{}", free_port());
    let grpc_addr = format!("127.0.0.1:{}", free_port());
    let http = HttpClient::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("build cancellation HTTP client");
    let mut process = spawn_crm_api(&database_url, &http_addr, &grpc_addr, true, None);
    wait_until_ready(&http, &mut process, &http_addr, true).await;
    let mut grpc = connect_grpc(&grpc_addr).await;

    create_party(&mut grpc, &party_definition).await;
    let success_case = verified_case(
        &mut grpc,
        &create_definition,
        &submit_definition,
        &verify_definition,
        "success",
    )
    .await;

    let unauthenticated = http_mutate(
        &http,
        &http_addr,
        &cancel_definition,
        &cancel_payload(&cancel_definition, &success_case, 3),
        TENANT_A,
        "privacy-cancel-unauthenticated",
        false,
    )
    .await;
    assert_eq!(unauthenticated.status(), reqwest::StatusCode::UNAUTHORIZED);
    let unauthenticated_body: Value = unauthenticated
        .json()
        .await
        .expect("decode unauthenticated cancellation response");
    assert_eq!(
        unauthenticated_body,
        serde_json::json!({"error": "request_failed"})
    );
    assert_safe_text(&unauthenticated_body.to_string());
    assert_record_version(&admin, TENANT_A, &success_case, 3).await;

    let outside_token = mutate(
        &mut grpc,
        &cancel_definition,
        cancel_payload(&cancel_definition, &success_case, 3),
        TENANT_OUTSIDE_TOKEN,
        "privacy-cancel-outside-token",
        true,
    )
    .await
    .expect_err("tenant outside bearer grant must be denied before cancellation");
    assert_safe_status(&outside_token, Code::PermissionDenied, "TENANT_FORBIDDEN");
    assert_record_version(&admin, TENANT_A, &success_case, 3).await;

    let success_key = "privacy-cancel-success";
    let first = mutate(
        &mut grpc,
        &cancel_definition,
        cancel_payload(&cancel_definition, &success_case, 3),
        TENANT_A,
        success_key,
        true,
    )
    .await
    .expect("cancel verified case through generic gRPC ingress");
    let first_case = decode_cancel(&first);
    assert_eq!(first_case.status, wire::PrivacyCaseStatus::Cancelled as i32);
    assert_eq!(first_case.version, 4);
    let binding = first_case
        .subject_binding
        .as_ref()
        .expect("cancellation preserves verified subject binding");
    assert_eq!(
        binding
            .canonical_party_ref
            .as_ref()
            .expect("canonical Party reference")
            .party_id,
        PARTY_A
    );
    assert_record_version(&admin, TENANT_A, &success_case, 4).await;
    let committed = cancel_evidence(&admin, TENANT_A, &success_case, success_key).await;
    assert_eq!(
        committed,
        CancelEvidence {
            events: 1,
            audits: 1,
            idempotency: 1,
            transactions: 1,
        }
    );

    let replay = mutate(
        &mut grpc,
        &cancel_definition,
        cancel_payload(&cancel_definition, &success_case, 3),
        TENANT_A,
        success_key,
        true,
    )
    .await
    .expect("exact cancellation replay returns committed output");
    assert_eq!(decode_cancel(&replay), first_case);
    assert_eq!(cancel_evidence(&admin, TENANT_A, &success_case, success_key).await, committed);

    let conflict = mutate(
        &mut grpc,
        &cancel_definition,
        cancel_payload(&cancel_definition, &success_case, 4),
        TENANT_A,
        success_key,
        true,
    )
    .await
    .expect_err("incompatible cancellation replay must conflict");
    assert_safe_status(
        &conflict,
        Code::AlreadyExists,
        "CAPABILITY_IDEMPOTENCY_KEY_REUSED",
    );
    assert_record_version(&admin, TENANT_A, &success_case, 4).await;

    let terminal = mutate(
        &mut grpc,
        &cancel_definition,
        cancel_payload(&cancel_definition, &success_case, 4),
        TENANT_A,
        "privacy-cancel-terminal",
        true,
    )
    .await
    .expect_err("terminal case must not be cancelled twice under a new key");
    assert_safe_status(
        &terminal,
        Code::FailedPrecondition,
        "CUSTOMER_PRIVACY_INVALID_TRANSITION",
    );
    assert_record_version(&admin, TENANT_A, &success_case, 4).await;

    let cross_tenant = mutate(
        &mut grpc,
        &cancel_definition,
        cancel_payload(&cancel_definition, &success_case, 4),
        TENANT_B,
        "privacy-cancel-cross-tenant",
        true,
    )
    .await
    .expect_err("cross-tenant privacy case must be concealed");
    assert_safe_status(
        &cross_tenant,
        Code::NotFound,
        "CUSTOMER_PRIVACY_CASE_NOT_FOUND",
    );

    let draft_case = create_case(&mut grpc, &create_definition, "draft").await;
    let draft_cancel = mutate(
        &mut grpc,
        &cancel_definition,
        cancel_payload(&cancel_definition, &draft_case, 1),
        TENANT_A,
        "privacy-cancel-draft",
        true,
    )
    .await
    .expect("unbound Draft case cancels without a subject lock");
    let draft_cancelled = decode_cancel(&draft_cancel);
    assert_eq!(draft_cancelled.status, wire::PrivacyCaseStatus::Cancelled as i32);
    assert_eq!(draft_cancelled.version, 2);
    assert!(draft_cancelled.subject_binding.is_none());

    let stale_case = submitted_case(
        &mut grpc,
        &create_definition,
        &submit_definition,
        "stale",
    )
    .await;
    let stale = mutate(
        &mut grpc,
        &cancel_definition,
        cancel_payload(&cancel_definition, &stale_case, 1),
        TENANT_A,
        "privacy-cancel-stale",
        true,
    )
    .await
    .expect_err("stale expected version must conflict");
    assert_safe_status(
        &stale,
        Code::Aborted,
        "CUSTOMER_PRIVACY_VERSION_CONFLICT",
    );
    assert_record_version(&admin, TENANT_A, &stale_case, 2).await;

    let contended_case = verified_case(
        &mut grpc,
        &create_definition,
        &submit_definition,
        &verify_definition,
        "contended",
    )
    .await;
    let mut lock_holder = admin.begin().await.expect("start subject lock holder");
    bind_context(
        &mut lock_holder,
        "privacy-cancel-lock-holder",
        "privacy-cancel-lock-holder-tx",
    )
    .await;
    sqlx::query("SELECT crm.lock_customer_subject($1, $2)")
        .bind(TENANT_A)
        .bind(PARTY_A)
        .execute(&mut *lock_holder)
        .await
        .expect("hold shared canonical subject lock");
    let contended = mutate(
        &mut grpc,
        &cancel_definition,
        cancel_payload(&cancel_definition, &contended_case, 3),
        TENANT_A,
        "privacy-cancel-contended",
        true,
    )
    .await
    .expect_err("contended canonical subject must fail bounded and retryable");
    assert_safe_status(
        &contended,
        Code::Unavailable,
        "CUSTOMER_PRIVACY_CANCELLATION_SUBJECT_LOCK_UNAVAILABLE",
    );
    assert_record_version(&admin, TENANT_A, &contended_case, 3).await;
    assert_eq!(
        cancel_evidence(
            &admin,
            TENANT_A,
            &contended_case,
            "privacy-cancel-contended"
        )
        .await,
        CancelEvidence {
            events: 0,
            audits: 0,
            idempotency: 0,
            transactions: 0,
        }
    );
    lock_holder.rollback().await.expect("release subject lock");
    let retried = mutate(
        &mut grpc,
        &cancel_definition,
        cancel_payload(&cancel_definition, &contended_case, 3),
        TENANT_A,
        "privacy-cancel-contended",
        true,
    )
    .await
    .expect("same cancellation retries after lock release");
    assert_eq!(decode_cancel(&retried).version, 4);

    let inactive_case = submitted_case(
        &mut grpc,
        &create_definition,
        &submit_definition,
        "inactive",
    )
    .await;
    set_module_status(&admin, TENANT_A, "suspended").await;
    let inactive = mutate(
        &mut grpc,
        &cancel_definition,
        cancel_payload(&cancel_definition, &inactive_case, 2),
        TENANT_A,
        "privacy-cancel-inactive",
        true,
    )
    .await
    .expect_err("inactive Customer Privacy module must reject cancellation");
    assert_safe_status(&inactive, Code::Aborted, "MODULE_NOT_ACTIVE");
    assert_record_version(&admin, TENANT_A, &inactive_case, 2).await;
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
    let denied = mutate(
        &mut denied_grpc,
        &cancel_definition,
        cancel_payload(&cancel_definition, &inactive_case, 2),
        TENANT_A,
        "privacy-cancel-no-grant",
        true,
    )
    .await
    .expect_err("authenticated cancellation without live grant must fail");
    assert_safe_status(
        &denied,
        Code::PermissionDenied,
        "CAPABILITY_PERMISSION_DENIED",
    );
    assert_record_version(&admin, TENANT_A, &inactive_case, 2).await;
    stop_process(&mut denied_process).await;
}

async fn create_party(
    grpc: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: &CapabilityDefinition,
) {
    mutate(
        grpc,
        definition,
        payload(
            definition,
            parties_wire::CreatePartyRequest {
                party_ref: Some(customer_wire::PartyRef {
                    party_id: PARTY_A.to_owned(),
                }),
                kind: parties_wire::PartyKind::Person as i32,
                display_name: "Privacy cancellation fixture".to_owned(),
            },
        ),
        TENANT_A,
        "privacy-cancel-party-create",
        true,
    )
    .await
    .expect("create authoritative cancellation Party");
}

async fn create_case(
    grpc: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: &CapabilityDefinition,
    name: &str,
) -> String {
    let response = mutate(
        grpc,
        definition,
        payload(
            definition,
            wire::CreatePrivacyCaseRequest {
                kind: wire::PrivacyCaseKind::Erasure as i32,
                policy_version: "privacy-policy/1".to_owned(),
                previous_privacy_case_ref: None,
            },
        ),
        TENANT_A,
        &format!("privacy-cancel-{name}-create"),
        true,
    )
    .await
    .expect("create cancellation privacy case");
    wire::CreatePrivacyCaseResponse::decode(
        response
            .output
            .as_ref()
            .expect("create output")
            .payload
            .as_slice(),
    )
    .expect("decode create output")
    .privacy_case
    .expect("create output case")
    .privacy_case_ref
    .expect("create output reference")
    .privacy_case_id
}

async fn submitted_case(
    grpc: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    create_definition: &CapabilityDefinition,
    submit_definition: &CapabilityDefinition,
    name: &str,
) -> String {
    let case_id = create_case(grpc, create_definition, name).await;
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
        &format!("privacy-cancel-{name}-submit"),
        true,
    )
    .await
    .expect("submit cancellation privacy case");
    case_id
}

async fn verified_case(
    grpc: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    create_definition: &CapabilityDefinition,
    submit_definition: &CapabilityDefinition,
    verify_definition: &CapabilityDefinition,
    name: &str,
) -> String {
    let case_id = submitted_case(grpc, create_definition, submit_definition, name).await;
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
        &format!("privacy-cancel-{name}-verify"),
        true,
    )
    .await
    .expect("verify cancellation case subject");
    case_id
}

fn cancel_payload(
    definition: &CapabilityDefinition,
    case_id: &str,
    expected_version: i64,
) -> TypedPayload {
    payload(
        definition,
        wire::CancelPrivacyCaseRequest {
            privacy_case_ref: Some(wire::PrivacyCaseRef {
                privacy_case_id: case_id.to_owned(),
            }),
            expected_version,
        },
    )
}

fn decode_cancel(
    response: &crm_application_runtime::gateway_v1::MutationResponse,
) -> wire::PrivacyCase {
    wire::CancelPrivacyCaseResponse::decode(
        response
            .output
            .as_ref()
            .expect("cancel output")
            .payload
            .as_slice(),
    )
    .expect("decode cancellation response")
    .privacy_case
    .expect("cancellation response case")
}

async fn cancel_evidence(
    pool: &PgPool,
    tenant: &str,
    case_id: &str,
    idempotency_key: &str,
) -> CancelEvidence {
    CancelEvidence {
        events: sqlx::query_scalar(
            "SELECT count(*) FROM crm.outbox_events WHERE tenant_id = $1 AND aggregate_type = $2 AND aggregate_id = $3 AND event_type = 'customer_privacy.case.status_changed' AND aggregate_version = 4",
        )
        .bind(tenant)
        .bind(RECORD_TYPE)
        .bind(case_id)
        .fetch_one(pool)
        .await
        .expect("count cancellation events"),
        audits: sqlx::query_scalar(
            "SELECT count(*) FROM crm.audit_records WHERE tenant_id = $1 AND aggregate_type = $2 AND aggregate_id = $3 AND capability_id = $4",
        )
        .bind(tenant)
        .bind(RECORD_TYPE)
        .bind(case_id)
        .bind(CANCEL_CASE)
        .fetch_one(pool)
        .await
        .expect("count cancellation audits"),
        idempotency: sqlx::query_scalar(
            "SELECT count(*) FROM crm.idempotency_records WHERE tenant_id = $1 AND idempotency_scope = 'capability:customer_privacy.case.cancel:1.0.0' AND idempotency_key = $2 AND status = 'completed'",
        )
        .bind(tenant)
        .bind(idempotency_key)
        .fetch_one(pool)
        .await
        .expect("count cancellation idempotency evidence"),
        transactions: sqlx::query_scalar(
            "SELECT count(*) FROM crm.business_transactions WHERE tenant_id = $1 AND capability_id = $2 AND status = 'committed'",
        )
        .bind(tenant)
        .bind(CANCEL_CASE)
        .fetch_one(pool)
        .await
        .expect("count cancellation business transactions"),
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
    .expect("read cancellation case version");
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
        ("app.actor_id", "customer-privacy-cancel-process-admin"),
        ("app.request_id", "customer-privacy-cancel-process-activation"),
        ("app.capability_id", "customer_privacy.process.activation"),
        ("app.capability_version", "1.0.0"),
        ("app.business_transaction_id", transaction_id.as_str()),
    ] {
        sqlx::query("SELECT set_config($1, $2, true)")
            .bind(name)
            .bind(value)
            .execute(&mut *transaction)
            .await
            .expect("bind cancellation activation context");
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
        .expect("commit cancellation activation update");
}

async fn bind_context(
    transaction: &mut Transaction<'_, Postgres>,
    request_id: &str,
    business_transaction_id: &str,
) {
    for (name, value) in [
        ("app.tenant_id", TENANT_A),
        ("app.actor_id", "customer-privacy-cancel-lock-holder"),
        ("app.request_id", request_id),
        (
            "app.capability_id",
            "customer_privacy.case.cancel.lock_fixture",
        ),
        ("app.capability_version", "1.0.0"),
        ("app.business_transaction_id", business_transaction_id),
    ] {
        sqlx::query("SELECT set_config($1, $2, true)")
            .bind(name)
            .bind(value)
            .execute(&mut **transaction)
            .await
            .expect("bind cancellation lock context");
    }
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
        if expected_error_code
            == "CUSTOMER_PRIVACY_CANCELLATION_SUBJECT_LOCK_UNAVAILABLE"
            || expected_error_code == "CUSTOMER_PRIVACY_VERSION_CONFLICT"
        {
            "true"
        } else {
            "false"
        }
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
    ] {
        assert!(
            !value.contains(forbidden),
            "safe cancellation surface leaked {forbidden}: {value}"
        );
    }
}
