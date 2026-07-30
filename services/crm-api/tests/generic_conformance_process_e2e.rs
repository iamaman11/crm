#![cfg(unix)]

#[path = "support/generic_conformance.rs"]
mod conformance;
#[path = "support/customer_enrichment_process/mod.rs"]
mod process_support;

use conformance::{EvidenceSnapshot, MutationConformanceSuite, QueryConformanceSuite};
use crm_application_runtime::gateway_v1::{
    MutateResponse, QueryResponse,
    application_gateway_service_client::ApplicationGatewayServiceClient,
};
use crm_capability_runtime::CapabilityDefinition;
use crm_customer_privacy_query_adapter::list_privacy_cases_capability_definition;
use crm_customer_privacy_subject_capability_adapter::capability_definition as subject_definition;
use crm_module_sdk::TypedPayload;
use crm_proto_contracts::crm::{
    customer::v1 as customer_wire, customer_enrichment::v1 as enrichment_wire,
    customer_privacy::v1 as privacy_wire, parties::v1 as parties_wire,
};
use prost::Message;
use reqwest::Client as HttpClient;
use sqlx::{PgPool, Row};
use std::collections::BTreeSet;
use std::time::Duration;
use tokio::time::{Instant, sleep};
use tonic::Code;

use process_support::{
    PARTY_CREATE, PARTY_ID, PUBLISH_MAPPING, PUBLISH_PROFILE, TENANT_A, TENANT_B,
    TENANT_OUTSIDE_TOKEN, connect_grpc, decode_mapping_id, decode_profile_id, free_port,
    http_mutate, legitimate_interest_request_payload, mapping_payload, mutate, mutation_definition,
    party_payload, payload, profile_payload, query, spawn_crm_api, stop_process, wait_until_ready,
};

const CUSTOMER_ENRICHMENT_MODULE: &str = "crm.customer-enrichment";
const CREATE_ENRICHMENT_REQUEST: &str = "customer_enrichment.request.create";
const CUSTOMER_PRIVACY_MODULE: &str = "crm.customer-privacy";
const CREATE_PRIVACY_CASE: &str = "customer_privacy.case.create";
const SUBMIT_PRIVACY_CASE: &str = "customer_privacy.case.submit";
const LIST_PRIVACY_CASES: &str = "customer_privacy.case.list";
const PRIVACY_PARTY_ID: &str = "generic-conformance-privacy-party";

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn representative_owners_adopt_generic_mutation_and_query_conformance() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping generic conformance process test because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect generic conformance evidence reader");

    let profile_definition = mutation_definition(PUBLISH_PROFILE);
    let mapping_definition = mutation_definition(PUBLISH_MAPPING);
    let enrichment_request_definition = mutation_definition(CREATE_ENRICHMENT_REQUEST);
    let party_definition = mutation_definition(PARTY_CREATE);
    let privacy_create_definition = mutation_definition(CREATE_PRIVACY_CASE);
    let privacy_submit_definition = mutation_definition(SUBMIT_PRIVACY_CASE);
    let privacy_subject_definition =
        subject_definition().expect("construct privacy subject-verification definition");
    let privacy_list_definition =
        list_privacy_cases_capability_definition().expect("construct privacy case-list definition");

    assert_eq!(
        enrichment_request_definition.owner_module_id.as_str(),
        CUSTOMER_ENRICHMENT_MODULE
    );
    assert_eq!(
        privacy_list_definition.owner_module_id.as_str(),
        CUSTOMER_PRIVACY_MODULE
    );
    assert_eq!(
        privacy_list_definition.capability_id.as_str(),
        LIST_PRIVACY_CASES
    );

    let http_addr = format!("127.0.0.1:{}", free_port());
    let grpc_addr = format!("127.0.0.1:{}", free_port());
    let http = HttpClient::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("build generic conformance HTTP client");
    let mut process = spawn_crm_api(&database_url, &http_addr, &grpc_addr, true, None);
    wait_until_ready(&http, &mut process, &http_addr, true).await;
    let mut grpc = connect_grpc(&grpc_addr).await;

    mutate(
        &mut grpc,
        &party_definition,
        party_payload(&party_definition),
        TENANT_A,
        "generic-conformance-enrichment-party",
        true,
    )
    .await
    .expect("create Customer Enrichment Party fixture");
    let profile = mutate(
        &mut grpc,
        &profile_definition,
        profile_payload(&profile_definition, "generic-conformance-profile"),
        TENANT_A,
        "generic-conformance-profile",
        true,
    )
    .await
    .expect("publish Customer Enrichment provider profile");
    let profile_id = decode_profile_id(&profile);
    let mapping = mutate(
        &mut grpc,
        &mapping_definition,
        mapping_payload(&mapping_definition, &profile_id),
        TENANT_A,
        "generic-conformance-mapping",
        true,
    )
    .await
    .expect("publish Customer Enrichment mapping");
    let mapping_id = decode_mapping_id(&mapping);

    let mutation_suite = MutationConformanceSuite::new([
        PARTY_ID.to_owned(),
        profile_id.clone(),
        mapping_id.clone(),
        "internal_reference".to_owned(),
        "payload_bytes".to_owned(),
        "descriptor_hash".to_owned(),
        "sqlx".to_owned(),
        "SELECT".to_owned(),
        "postgres://".to_owned(),
    ]);
    let valid_request = legitimate_interest_request_payload(
        &enrichment_request_definition,
        &profile_id,
        &mapping_id,
    );

    let before_unauthenticated = evidence_snapshot(&admin).await;
    let unauthenticated = http_mutate(
        &http,
        &http_addr,
        &enrichment_request_definition,
        &valid_request,
        TENANT_A,
        "generic-conformance-unauthenticated",
        false,
    )
    .await;
    let unauthenticated_status = unauthenticated.status();
    let unauthenticated_body = unauthenticated
        .json()
        .await
        .expect("decode unauthenticated conformance response");
    mutation_suite.assert_unauthenticated_http(
        unauthenticated_status,
        &unauthenticated_body,
        before_unauthenticated,
        evidence_snapshot(&admin).await,
    );

    let before_tenant_mismatch = evidence_snapshot(&admin).await;
    let tenant_mismatch = mutate(
        &mut grpc,
        &enrichment_request_definition,
        valid_request.clone(),
        TENANT_OUTSIDE_TOKEN,
        "generic-conformance-tenant-mismatch",
        true,
    )
    .await
    .expect_err("mutation tenant outside bearer grant must be denied");
    mutation_suite.assert_denied(
        &tenant_mismatch,
        Code::PermissionDenied,
        "TENANT_FORBIDDEN",
        false,
        before_tenant_mismatch,
        evidence_snapshot(&admin).await,
    );

    let before_malformed = evidence_snapshot(&admin).await;
    let mut malformed_request = valid_request.clone();
    malformed_request.bytes = vec![0xff];
    let malformed = mutate(
        &mut grpc,
        &enrichment_request_definition,
        malformed_request,
        TENANT_A,
        "generic-conformance-malformed",
        true,
    )
    .await
    .expect_err("malformed mutation payload must fail closed");
    mutation_suite.assert_denied(
        &malformed,
        Code::InvalidArgument,
        "CAPABILITY_INPUT_PROTOBUF_INVALID",
        false,
        before_malformed,
        evidence_snapshot(&admin).await,
    );

    let success_key = "generic-conformance-enrichment-success";
    let before_success = evidence_snapshot(&admin).await;
    let first = mutate(
        &mut grpc,
        &enrichment_request_definition,
        valid_request.clone(),
        TENANT_A,
        success_key,
        true,
    )
    .await
    .expect("execute representative mutation through generic ingress");
    let committed = evidence_snapshot(&admin).await;
    mutation_suite.assert_atomic_commit(before_success, committed);

    let replay = mutate(
        &mut grpc,
        &enrichment_request_definition,
        valid_request.clone(),
        TENANT_A,
        success_key,
        true,
    )
    .await
    .expect("exact mutation replay must return committed output");
    mutation_suite.assert_exact_replay(
        mutation_output(&first),
        mutation_output(&replay),
        committed,
        evidence_snapshot(&admin).await,
    );

    let mut conflicting_request = valid_request.clone();
    let mut conflicting_command = enrichment_wire::CreateEnrichmentRequestRequest::decode(
        conflicting_request.bytes.as_slice(),
    )
    .expect("decode valid enrichment request for replay-conflict fixture");
    conflicting_command.deadline_at_unix_ms += 1;
    conflicting_command.expires_at_unix_ms += 1;
    conflicting_request.bytes = conflicting_command.encode_to_vec();
    let conflicting_replay = mutate(
        &mut grpc,
        &enrichment_request_definition,
        conflicting_request,
        TENANT_A,
        success_key,
        true,
    )
    .await
    .expect_err("incompatible mutation replay must conflict");
    mutation_suite.assert_denied(
        &conflicting_replay,
        Code::Aborted,
        "CAPABILITY_IDEMPOTENCY_KEY_REUSED",
        false,
        committed,
        evidence_snapshot(&admin).await,
    );

    wait_for_customer_enrichment_dispatch(&admin).await;

    set_module_status(&admin, CUSTOMER_ENRICHMENT_MODULE, "suspended").await;
    let before_inactive_mutation = evidence_snapshot(&admin).await;
    let inactive_mutation = mutate(
        &mut grpc,
        &profile_definition,
        profile_payload(&profile_definition, "generic-conformance-inactive-profile"),
        TENANT_A,
        "generic-conformance-inactive-mutation",
        true,
    )
    .await
    .expect_err("inactive owner module must reject mutation");
    mutation_suite.assert_denied(
        &inactive_mutation,
        Code::Aborted,
        "MODULE_NOT_ACTIVE",
        false,
        before_inactive_mutation,
        evidence_snapshot(&admin).await,
    );
    set_module_status(&admin, CUSTOMER_ENRICHMENT_MODULE, "active").await;
    set_module_status(&admin, CUSTOMER_PRIVACY_MODULE, "active").await;

    create_party(
        &mut grpc,
        &party_definition,
        PRIVACY_PARTY_ID,
        "generic-conformance-privacy-party",
    )
    .await;
    let privacy_case_a = create_submit_verify_case(
        &mut grpc,
        &privacy_create_definition,
        &privacy_submit_definition,
        &privacy_subject_definition,
        privacy_wire::PrivacyCaseKind::Access,
        "generic-conformance-privacy-a",
    )
    .await;
    let privacy_case_b = create_submit_verify_case(
        &mut grpc,
        &privacy_create_definition,
        &privacy_submit_definition,
        &privacy_subject_definition,
        privacy_wire::PrivacyCaseKind::Erasure,
        "generic-conformance-privacy-b",
    )
    .await;

    let query_suite = QueryConformanceSuite::new([
        PRIVACY_PARTY_ID.to_owned(),
        privacy_case_a.clone(),
        privacy_case_b.clone(),
        "internal_reference".to_owned(),
        "payload_bytes".to_owned(),
        "descriptor_hash".to_owned(),
        "sqlx".to_owned(),
        "SELECT".to_owned(),
        "postgres://".to_owned(),
    ]);
    let query_baseline = evidence_snapshot(&admin).await;

    let tenant_mismatch_query = query(
        &mut grpc,
        &privacy_list_definition,
        list_payload(&privacy_list_definition, 1, ""),
        TENANT_OUTSIDE_TOKEN,
        true,
    )
    .await
    .expect_err("query tenant outside bearer grant must be denied");
    query_suite.assert_denied(
        &tenant_mismatch_query,
        Code::PermissionDenied,
        "TENANT_FORBIDDEN",
        false,
        query_baseline,
        evidence_snapshot(&admin).await,
    );

    let first_page = query(
        &mut grpc,
        &privacy_list_definition,
        list_payload(&privacy_list_definition, 1, ""),
        TENANT_A,
        true,
    )
    .await
    .expect("query first representative keyset page");
    let first_page = decode_list(&first_page);
    let second_page = query(
        &mut grpc,
        &privacy_list_definition,
        list_payload(&privacy_list_definition, 1, &first_page.next_cursor),
        TENANT_A,
        true,
    )
    .await
    .expect("query second representative keyset page");
    let second_page = decode_list(&second_page);
    query_suite.assert_keyset_pages(
        &listed_ids(&first_page),
        &first_page.next_cursor,
        &listed_ids(&second_page),
        &second_page.next_cursor,
        &BTreeSet::from([privacy_case_a.clone(), privacy_case_b.clone()]),
        query_baseline,
        evidence_snapshot(&admin).await,
    );

    let mut malformed_cursor = first_page.next_cursor.clone();
    malformed_cursor.push('x');
    let malformed_cursor = query(
        &mut grpc,
        &privacy_list_definition,
        list_payload(&privacy_list_definition, 1, &malformed_cursor),
        TENANT_A,
        true,
    )
    .await
    .expect_err("malformed query cursor must fail closed");
    query_suite.assert_denied(
        &malformed_cursor,
        Code::InvalidArgument,
        "CUSTOMER_PRIVACY_CASE_LIST_CURSOR_INVALID",
        false,
        query_baseline,
        evidence_snapshot(&admin).await,
    );

    let concealed = query(
        &mut grpc,
        &privacy_list_definition,
        list_payload(&privacy_list_definition, 10, ""),
        TENANT_B,
        true,
    )
    .await
    .expect("cross-tenant query target must be concealed as an empty page");
    let concealed = decode_list(&concealed);
    query_suite.assert_not_found_concealed(
        concealed.privacy_cases.len(),
        &concealed.next_cursor,
        query_baseline,
        evidence_snapshot(&admin).await,
    );

    set_module_status(&admin, CUSTOMER_PRIVACY_MODULE, "suspended").await;
    let inactive_query = query(
        &mut grpc,
        &privacy_list_definition,
        list_payload(&privacy_list_definition, 10, ""),
        TENANT_A,
        true,
    )
    .await
    .expect_err("inactive owner module must reject query");
    query_suite.assert_denied(
        &inactive_query,
        Code::Aborted,
        "MODULE_NOT_ACTIVE",
        false,
        query_baseline,
        evidence_snapshot(&admin).await,
    );
    set_module_status(&admin, CUSTOMER_PRIVACY_MODULE, "active").await;
    stop_process(&mut process).await;

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

    let denied_baseline = evidence_snapshot(&admin).await;
    let denied_mutation = mutate(
        &mut denied_grpc,
        &profile_definition,
        profile_payload(&profile_definition, "generic-conformance-live-auth-denied"),
        TENANT_A,
        "generic-conformance-live-auth-denied",
        true,
    )
    .await
    .expect_err("mutation without live grant must be denied");
    mutation_suite.assert_denied(
        &denied_mutation,
        Code::PermissionDenied,
        "CAPABILITY_PERMISSION_DENIED",
        false,
        denied_baseline,
        evidence_snapshot(&admin).await,
    );

    let denied_query = query(
        &mut denied_grpc,
        &privacy_list_definition,
        list_payload(&privacy_list_definition, 10, ""),
        TENANT_A,
        true,
    )
    .await
    .expect_err("query without live grant must be denied");
    query_suite.assert_denied(
        &denied_query,
        Code::PermissionDenied,
        "QUERY_PERMISSION_DENIED",
        false,
        denied_baseline,
        evidence_snapshot(&admin).await,
    );
    stop_process(&mut denied_process).await;
}

async fn wait_for_customer_enrichment_dispatch(pool: &PgPool) {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let completed: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM crm.idempotency_records WHERE tenant_id = $1 AND idempotency_scope = 'capability:customer_enrichment.request.dispatch:1.0.0' AND status = 'completed'",
        )
        .bind(TENANT_A)
        .fetch_one(pool)
        .await
        .expect("read Customer Enrichment dispatch completion evidence");
        if completed == 1 {
            return;
        }
        assert_eq!(
            completed, 0,
            "generic conformance created multiple dispatches"
        );
        assert!(
            Instant::now() < deadline,
            "Customer Enrichment dispatch did not quiesce before query conformance"
        );
        sleep(Duration::from_millis(50)).await;
    }
}

async fn evidence_snapshot(pool: &PgPool) -> EvidenceSnapshot {
    EvidenceSnapshot {
        records: count(pool, "SELECT count(*) FROM crm.records").await,
        relationships: count(pool, "SELECT count(*) FROM crm.relationships").await,
        events: count(pool, "SELECT count(*) FROM crm.outbox_events").await,
        audits: count(pool, "SELECT count(*) FROM crm.audit_records").await,
        idempotency: count(pool, "SELECT count(*) FROM crm.idempotency_records").await,
        transactions: count(pool, "SELECT count(*) FROM crm.business_transactions").await,
    }
}

async fn count(pool: &PgPool, statement: &'static str) -> i64 {
    sqlx::query_scalar(statement)
        .fetch_one(pool)
        .await
        .expect("read generic conformance evidence count")
}

async fn set_module_status(pool: &PgPool, module_id: &str, status: &str) {
    let row = sqlx::query(
        "SELECT last_business_transaction_id FROM crm.module_installations WHERE tenant_id = $1 AND module_id = $2",
    )
    .bind(TENANT_A)
    .bind(module_id)
    .fetch_one(pool)
    .await
    .expect("read module installation for generic conformance");
    let transaction_id: String = row.get("last_business_transaction_id");
    let mut transaction = pool.begin().await.expect("start module activation update");
    for (name, value) in [
        ("app.tenant_id", TENANT_A),
        ("app.actor_id", "generic-conformance-admin"),
        ("app.request_id", "generic-conformance-activation"),
        ("app.capability_id", "generic_conformance.activation"),
        ("app.capability_version", "1.0.0"),
        ("app.business_transaction_id", transaction_id.as_str()),
    ] {
        sqlx::query("SELECT set_config($1, $2, true)")
            .bind(name)
            .bind(value)
            .execute(&mut *transaction)
            .await
            .expect("bind generic conformance activation context");
    }
    sqlx::query(
        "UPDATE crm.module_installations SET status = $1, updated_at = clock_timestamp() WHERE tenant_id = $2 AND module_id = $3",
    )
    .bind(status)
    .bind(TENANT_A)
    .bind(module_id)
    .execute(&mut *transaction)
    .await
    .expect("update module activation for generic conformance");
    transaction
        .commit()
        .await
        .expect("commit generic conformance activation update");
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
                display_name: "Generic conformance privacy Party".to_owned(),
            },
        ),
        TENANT_A,
        idempotency_key,
        true,
    )
    .await
    .expect("create privacy Party fixture through generic ingress");
}

async fn create_submit_verify_case(
    grpc: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    create_definition: &CapabilityDefinition,
    submit_definition: &CapabilityDefinition,
    verify_definition: &CapabilityDefinition,
    kind: privacy_wire::PrivacyCaseKind,
    prefix: &str,
) -> String {
    let created = mutate(
        grpc,
        create_definition,
        payload(
            create_definition,
            privacy_wire::CreatePrivacyCaseRequest {
                kind: kind as i32,
                policy_version: "generic-conformance-policy/1".to_owned(),
                previous_privacy_case_ref: None,
            },
        ),
        TENANT_A,
        &format!("{prefix}-create"),
        true,
    )
    .await
    .expect("create privacy case fixture");
    let case_id = privacy_wire::CreatePrivacyCaseResponse::decode(mutation_output(&created))
        .expect("decode privacy case creation")
        .privacy_case
        .and_then(|privacy_case| privacy_case.privacy_case_ref)
        .expect("created privacy case reference")
        .privacy_case_id;

    mutate(
        grpc,
        submit_definition,
        payload(
            submit_definition,
            privacy_wire::SubmitPrivacyCaseRequest {
                privacy_case_ref: Some(privacy_wire::PrivacyCaseRef {
                    privacy_case_id: case_id.clone(),
                }),
                expected_version: 1,
            },
        ),
        TENANT_A,
        &format!("{prefix}-submit"),
        true,
    )
    .await
    .expect("submit privacy case fixture");

    mutate(
        grpc,
        verify_definition,
        payload(
            verify_definition,
            privacy_wire::VerifyPrivacyCaseSubjectRequest {
                privacy_case_ref: Some(privacy_wire::PrivacyCaseRef {
                    privacy_case_id: case_id.clone(),
                }),
                expected_version: 2,
                submitted_party_ref: Some(customer_wire::PartyRef {
                    party_id: PRIVACY_PARTY_ID.to_owned(),
                }),
                canonical_party_ref: Some(customer_wire::PartyRef {
                    party_id: PRIVACY_PARTY_ID.to_owned(),
                }),
                identity_resolution_generation: 1,
                verification_method: privacy_wire::SubjectVerificationMethod::VerifiedDocument
                    as i32,
            },
        ),
        TENANT_A,
        &format!("{prefix}-verify"),
        true,
    )
    .await
    .expect("verify privacy case subject fixture");
    case_id
}

fn list_payload(definition: &CapabilityDefinition, page_size: i32, cursor: &str) -> TypedPayload {
    payload(
        definition,
        privacy_wire::ListPrivacyCasesRequest {
            canonical_party_ref: Some(customer_wire::PartyRef {
                party_id: PRIVACY_PARTY_ID.to_owned(),
            }),
            kind: None,
            status: None,
            page_size,
            cursor: cursor.to_owned(),
        },
    )
}

fn decode_list(response: &QueryResponse) -> privacy_wire::ListPrivacyCasesResponse {
    privacy_wire::ListPrivacyCasesResponse::decode(
        response
            .output
            .as_ref()
            .expect("case-list output")
            .payload
            .as_slice(),
    )
    .expect("decode privacy case-list response")
}

fn listed_ids(response: &privacy_wire::ListPrivacyCasesResponse) -> Vec<String> {
    response
        .privacy_cases
        .iter()
        .map(|privacy_case| {
            privacy_case
                .privacy_case_ref
                .as_ref()
                .expect("listed privacy case reference")
                .privacy_case_id
                .clone()
        })
        .collect()
}

fn mutation_output(response: &MutateResponse) -> &[u8] {
    response
        .output
        .as_ref()
        .expect("mutation output")
        .payload
        .as_slice()
}
