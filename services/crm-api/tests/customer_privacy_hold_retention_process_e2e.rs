#![cfg(unix)]

#[path = "support/customer_enrichment_process/mod.rs"]
mod support;

use crm_application_runtime::gateway_v1::application_gateway_service_client::ApplicationGatewayServiceClient;
use crm_capability_runtime::CapabilityDefinition;
use crm_module_sdk::TypedPayload;
use crm_proto_contracts::crm::{
    customer::v1 as customer, customer_privacy::v1 as privacy, parties::v1 as parties,
};
use prost::Message;
use sqlx::{Executor, PgPool};
use std::time::{SystemTime, UNIX_EPOCH};
use tonic::{Code, Status};

use support::{
    TENANT_A, connect_grpc, free_port, mutate, mutation_definition, payload, spawn_crm_api,
    stop_process, wait_until_ready,
};

const PARTY_CREATE: &str = "parties.party.create";
const LEGAL_HOLD_PLACE: &str = "customer_privacy.legal_hold.place";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EvidenceCounts {
    records: i64,
    events: i64,
    audits: i64,
    idempotency: i64,
    transactions: i64,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn legal_hold_place_is_authorized_canonical_idempotent_and_atomic() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping legal-hold process test because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect legal-hold process evidence reader");

    admin
        .execute(sqlx::raw_sql(include_str!(
            "../../../database/tests/0005_party_adapter.sql"
        )))
        .await
        .expect("publish Party process fixture");

    let party_create = mutation_definition(PARTY_CREATE);
    let legal_hold_place = mutation_definition(LEGAL_HOLD_PLACE);
    assert_eq!(
        legal_hold_place.owner_module_id.as_str(),
        "crm.customer-privacy"
    );

    let http_addr = format!("127.0.0.1:{}", free_port());
    let grpc_addr = format!("127.0.0.1:{}", free_port());
    let http = reqwest::Client::new();
    let mut process = spawn_crm_api(&database_url, &http_addr, &grpc_addr, true, None);
    wait_until_ready(&http, &mut process, &http_addr, true).await;
    let mut grpc = connect_grpc(&grpc_addr).await;

    let party_id = unique_id("legal-hold-party");
    create_party(
        &mut grpc,
        &party_create,
        &party_id,
        "Legal Hold Subject",
        "legal-hold-party-create",
    )
    .await;

    let effective_from_unix_ms = now_millis();
    let input = legal_hold_payload(&legal_hold_place, &party_id, effective_from_unix_ms);

    let before_unauthorized = legal_hold_evidence(&admin).await;
    let unauthorized = mutate(
        &mut grpc,
        &legal_hold_place,
        input.clone(),
        TENANT_A,
        "legal-hold-unauthorized",
        false,
    )
    .await
    .expect_err("legal-hold placement must require authentication");
    assert_safe_status(&unauthorized, Code::Unauthenticated);
    assert_eq!(legal_hold_evidence(&admin).await, before_unauthorized);

    let before = legal_hold_evidence(&admin).await;
    let first = mutate(
        &mut grpc,
        &legal_hold_place,
        input.clone(),
        TENANT_A,
        "legal-hold-place",
        true,
    )
    .await
    .expect("place customer-data legal hold through generic ingress");
    let placed = decode_hold(&first);
    assert_eq!(
        placed
            .canonical_party_ref
            .as_ref()
            .expect("canonical Party reference")
            .party_id,
        party_id
    );
    assert_eq!(
        placed.status,
        privacy::CustomerDataLegalHoldStatus::Active as i32
    );
    assert_eq!(placed.version, 1);
    assert_eq!(placed.authority_reference_id, "authority-litigation-1");
    assert_eq!(placed.reason_code, "LITIGATION_HOLD");
    assert_eq!(placed.policy_version, "privacy-policy/1");
    assert_eq!(placed.effective_from_unix_ms, effective_from_unix_ms);
    let scope = placed.scope.expect("legal-hold scope");
    assert!(matches!(
        scope.scope,
        Some(privacy::customer_data_legal_hold_scope::Scope::AllCustomerData(true))
    ));
    let after_first = legal_hold_evidence(&admin).await;
    assert_incremented(before, after_first);

    let replay = mutate(
        &mut grpc,
        &legal_hold_place,
        input,
        TENANT_A,
        "legal-hold-place",
        true,
    )
    .await
    .expect("replay legal-hold placement");
    assert_eq!(
        first.output.as_ref().expect("first output").payload,
        replay.output.as_ref().expect("replay output").payload
    );
    assert_eq!(legal_hold_evidence(&admin).await, after_first);

    let before_missing = legal_hold_evidence(&admin).await;
    let missing = mutate(
        &mut grpc,
        &legal_hold_place,
        legal_hold_payload(
            &legal_hold_place,
            &unique_id("missing-canonical-party"),
            effective_from_unix_ms,
        ),
        TENANT_A,
        "legal-hold-missing-party",
        true,
    )
    .await
    .expect_err("legal hold must require a current canonical Party");
    assert!(matches!(missing.code(), Code::NotFound | Code::Aborted));
    assert_eq!(legal_hold_evidence(&admin).await, before_missing);

    stop_process(&mut process).await;
}

async fn create_party(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: &CapabilityDefinition,
    party_id: &str,
    display_name: &str,
    idempotency_key: &str,
) {
    mutate(
        client,
        definition,
        payload(
            definition,
            parties::CreatePartyRequest {
                party_ref: Some(customer::PartyRef {
                    party_id: party_id.to_owned(),
                }),
                kind: parties::PartyKind::Person as i32,
                display_name: display_name.to_owned(),
            },
        ),
        TENANT_A,
        idempotency_key,
        true,
    )
    .await
    .expect("create Party prerequisite through production gateway");
}

fn legal_hold_payload(
    definition: &CapabilityDefinition,
    party_id: &str,
    effective_from_unix_ms: i64,
) -> TypedPayload {
    payload(
        definition,
        privacy::PlaceCustomerDataLegalHoldRequest {
            canonical_party_ref: Some(customer::PartyRef {
                party_id: party_id.to_owned(),
            }),
            scope: Some(privacy::CustomerDataLegalHoldScope {
                scope: Some(privacy::customer_data_legal_hold_scope::Scope::AllCustomerData(true)),
            }),
            authority_reference_id: "authority-litigation-1".to_owned(),
            reason_code: "LITIGATION_HOLD".to_owned(),
            policy_version: "privacy-policy/1".to_owned(),
            effective_from_unix_ms,
            effective_until_unix_ms: None,
        },
    )
}

fn decode_hold(
    response: &crm_application_runtime::gateway_v1::MutateResponse,
) -> privacy::CustomerDataLegalHold {
    privacy::PlaceCustomerDataLegalHoldResponse::decode(
        response
            .output
            .as_ref()
            .expect("legal-hold placement output")
            .payload
            .as_slice(),
    )
    .expect("decode legal-hold placement response")
    .customer_data_legal_hold
    .expect("placed customer-data legal hold")
}

async fn legal_hold_evidence(admin: &PgPool) -> EvidenceCounts {
    let records = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.records WHERE tenant_id = $1 AND owner_module_id = 'crm.customer-privacy' AND record_type = 'customer-privacy.legal-hold' AND deleted_at IS NULL",
    )
    .bind(TENANT_A)
    .fetch_one(admin)
    .await
    .expect("count legal-hold records");
    let events = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.outbox_events WHERE tenant_id = $1 AND event_type = 'customer_privacy.legal_hold.placed'",
    )
    .bind(TENANT_A)
    .fetch_one(admin)
    .await
    .expect("count legal-hold events");
    let audits = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.audit_records WHERE tenant_id = $1 AND capability_id = $2",
    )
    .bind(TENANT_A)
    .bind(LEGAL_HOLD_PLACE)
    .fetch_one(admin)
    .await
    .expect("count legal-hold audits");
    let idempotency = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.idempotency_records AS i JOIN crm.business_transactions AS b USING (tenant_id, business_transaction_id) WHERE b.tenant_id = $1 AND b.capability_id = $2",
    )
    .bind(TENANT_A)
    .bind(LEGAL_HOLD_PLACE)
    .fetch_one(admin)
    .await
    .expect("count legal-hold idempotency evidence");
    let transactions = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.business_transactions WHERE tenant_id = $1 AND capability_id = $2",
    )
    .bind(TENANT_A)
    .bind(LEGAL_HOLD_PLACE)
    .fetch_one(admin)
    .await
    .expect("count legal-hold business transactions");
    EvidenceCounts {
        records,
        events,
        audits,
        idempotency,
        transactions,
    }
}

fn assert_incremented(before: EvidenceCounts, after: EvidenceCounts) {
    assert_eq!(after.records, before.records + 1);
    assert_eq!(after.events, before.events + 1);
    assert_eq!(after.audits, before.audits + 1);
    assert_eq!(after.idempotency, before.idempotency + 1);
    assert_eq!(after.transactions, before.transactions + 1);
}

fn assert_safe_status(status: &Status, expected_code: Code) {
    assert_eq!(status.code(), expected_code);
    for forbidden in [
        "authority-litigation-1",
        "LITIGATION_HOLD",
        "payload_bytes",
        "postgres://",
        "sqlx",
        "SELECT",
    ] {
        assert!(
            !status.message().contains(forbidden)
                && !format!("{:?}", status.metadata()).contains(forbidden),
            "safe legal-hold failure leaked protected detail: {forbidden}"
        );
    }
}

fn now_millis() -> i64 {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after Unix epoch")
            .as_millis(),
    )
    .expect("current timestamp fits i64")
}

fn unique_id(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after Unix epoch")
        .as_nanos();
    format!("{prefix}-{}-{nanos}", std::process::id())
}
