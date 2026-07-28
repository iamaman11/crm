#![cfg(unix)]

#[path = "support/customer_enrichment_process/mod.rs"]
mod support;

use crm_application_runtime::gateway_v1::application_gateway_service_client::ApplicationGatewayServiceClient;
use crm_capability_runtime::CapabilityDefinition;
use crm_module_sdk::TypedPayload;
use crm_proto_contracts::crm::{
    contact_points::v1 as contact_points, customer::v1 as customer,
    customer_privacy::v1 as privacy, parties::v1 as parties,
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
const CONTACT_POINT_CREATE: &str = "contact-points.contact-point.create";
const RESTRICTION_PLACE: &str = "customer_privacy.restriction.place";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EvidenceCounts {
    records: i64,
    events: i64,
    audits: i64,
    idempotency: i64,
    transactions: i64,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn restriction_place_blocks_protected_contact_point_create_atomically() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping restriction placement process test because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect restriction process evidence reader");

    for fixture in [
        include_str!("../../../database/tests/0005_party_adapter.sql"),
        include_str!("../../../database/tests/0007_contact_point_adapter.sql"),
        include_str!("../../../database/tests/0020_customer_privacy_persistence_fixture.sql"),
    ] {
        admin
            .execute(sqlx::raw_sql(fixture))
            .await
            .expect("publish restriction process registry fixture");
    }

    let party_create = mutation_definition(PARTY_CREATE);
    let contact_create = mutation_definition(CONTACT_POINT_CREATE);
    let restriction_place = mutation_definition(RESTRICTION_PLACE);
    assert_eq!(restriction_place.owner_module_id.as_str(), "crm.customer-privacy");

    let http_addr = format!("127.0.0.1:{}", free_port());
    let grpc_addr = format!("127.0.0.1:{}", free_port());
    let http = reqwest::Client::new();
    let mut process = spawn_crm_api(&database_url, &http_addr, &grpc_addr, true, None);
    wait_until_ready(&http, &mut process, &http_addr, true).await;
    let mut grpc = connect_grpc(&grpc_addr).await;

    let protected_party = unique_id("restriction-protected-party");
    create_party(
        &mut grpc,
        &party_create,
        &protected_party,
        "Protected Contact Owner",
        "restriction-process-protected-party",
    )
    .await;

    let contact_before = contact_point_evidence(&admin).await;
    let allowed_contact = unique_id("restriction-before-contact");
    mutate(
        &mut grpc,
        &contact_create,
        contact_point_payload(
            &contact_create,
            &allowed_contact,
            &protected_party,
            "before-restriction@example.com",
        ),
        TENANT_A,
        "restriction-process-before-contact",
        true,
    )
    .await
    .expect("Contact Point create must work before restriction placement");
    assert_incremented(contact_before, contact_point_evidence(&admin).await);

    let privacy_before = restriction_evidence(&admin).await;
    let placement = mutate(
        &mut grpc,
        &restriction_place,
        restriction_payload(&restriction_place, &protected_party),
        TENANT_A,
        "restriction-process-place",
        true,
    )
    .await
    .expect("place active processing restriction through generic ingress");
    let placed = privacy::PlaceProcessingRestrictionResponse::decode(
        placement
            .output
            .as_ref()
            .expect("restriction placement output")
            .payload
            .as_slice(),
    )
    .expect("decode restriction placement response")
    .processing_restriction
    .expect("placed processing restriction");
    assert_eq!(
        placed
            .canonical_party_ref
            .as_ref()
            .expect("placed canonical Party reference")
            .party_id,
        protected_party
    );
    assert_eq!(
        placed.scope,
        privacy::ProcessingRestrictionScope::Processing as i32
    );
    assert_eq!(
        placed.status,
        privacy::ProcessingRestrictionStatus::Active as i32
    );
    assert_eq!(placed.version, 1);
    assert_incremented(privacy_before, restriction_evidence(&admin).await);

    let before_denied = contact_point_evidence(&admin).await;
    let denied = mutate(
        &mut grpc,
        &contact_create,
        contact_point_payload(
            &contact_create,
            &unique_id("restriction-denied-contact"),
            &protected_party,
            "blocked@example.com",
        ),
        TENANT_A,
        "restriction-process-denied-contact",
        true,
    )
    .await
    .expect_err("active processing restriction must deny protected owner mutation");
    assert_safe_status(
        &denied,
        Code::Aborted,
        "CUSTOMER_PRIVACY_RESTRICTION_ACTIVE",
        false,
    );
    assert_eq!(contact_point_evidence(&admin).await, before_denied);

    let unprotected_party = unique_id("restriction-unprotected-party");
    create_party(
        &mut grpc,
        &party_create,
        &unprotected_party,
        "Unprotected Contact Owner",
        "restriction-process-unprotected-party",
    )
    .await;
    let before_unprotected = contact_point_evidence(&admin).await;
    mutate(
        &mut grpc,
        &contact_create,
        contact_point_payload(
            &contact_create,
            &unique_id("restriction-unprotected-contact"),
            &unprotected_party,
            "allowed@example.com",
        ),
        TENANT_A,
        "restriction-process-unprotected-contact",
        true,
    )
    .await
    .expect("restriction must not leak to another canonical Party");
    assert_incremented(before_unprotected, contact_point_evidence(&admin).await);

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

fn contact_point_payload(
    definition: &CapabilityDefinition,
    contact_point_id: &str,
    party_id: &str,
    value: &str,
) -> TypedPayload {
    payload(
        definition,
        contact_points::CreateContactPointRequest {
            contact_point_ref: Some(customer::ContactPointRef {
                contact_point_id: contact_point_id.to_owned(),
            }),
            party_ref: Some(customer::PartyRef {
                party_id: party_id.to_owned(),
            }),
            kind: contact_points::ContactPointKind::Email as i32,
            value: value.to_owned(),
            preferred: false,
            valid_from: None,
            valid_until: None,
        },
    )
}

fn restriction_payload(definition: &CapabilityDefinition, party_id: &str) -> TypedPayload {
    payload(
        definition,
        privacy::PlaceProcessingRestrictionRequest {
            canonical_party_ref: Some(customer::PartyRef {
                party_id: party_id.to_owned(),
            }),
            scope: privacy::ProcessingRestrictionScope::Processing as i32,
            policy_version: "privacy-policy/1".to_owned(),
            effective_from_unix_ms: 0,
            expires_at_unix_ms: None,
        },
    )
}

async fn contact_point_evidence(admin: &PgPool) -> EvidenceCounts {
    evidence(admin, "crm.contact-points", "contact-points.contact_point", CONTACT_POINT_CREATE,
        "contact-points.contact-point.created").await
}

async fn restriction_evidence(admin: &PgPool) -> EvidenceCounts {
    evidence(admin, "crm.customer-privacy", "customer-privacy.restriction", RESTRICTION_PLACE,
        "customer_privacy.restriction.placed").await
}

async fn evidence(
    admin: &PgPool,
    owner_module_id: &str,
    record_type: &str,
    capability_id: &str,
    event_type: &str,
) -> EvidenceCounts {
    let records = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.records WHERE tenant_id = $1 AND owner_module_id = $2 AND record_type = $3 AND deleted_at IS NULL",
    )
    .bind(TENANT_A)
    .bind(owner_module_id)
    .bind(record_type)
    .fetch_one(admin)
    .await
    .expect("count governed records");
    let events = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.outbox_events WHERE tenant_id = $1 AND event_type = $2",
    )
    .bind(TENANT_A)
    .bind(event_type)
    .fetch_one(admin)
    .await
    .expect("count governed events");
    let audits = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.audit_records WHERE tenant_id = $1 AND capability_id = $2",
    )
    .bind(TENANT_A)
    .bind(capability_id)
    .fetch_one(admin)
    .await
    .expect("count governed audits");
    let idempotency = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.idempotency_records AS i JOIN crm.business_transactions AS b USING (tenant_id, business_transaction_id) WHERE b.tenant_id = $1 AND b.capability_id = $2",
    )
    .bind(TENANT_A)
    .bind(capability_id)
    .fetch_one(admin)
    .await
    .expect("count governed idempotency evidence");
    let transactions = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.business_transactions WHERE tenant_id = $1 AND capability_id = $2",
    )
    .bind(TENANT_A)
    .bind(capability_id)
    .fetch_one(admin)
    .await
    .expect("count governed business transactions");
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
    for forbidden in ["blocked@example.com", "payload_bytes", "postgres://", "sqlx", "SELECT"] {
        assert!(
            !status.message().contains(forbidden)
                && !format!("{:?}", status.metadata()).contains(forbidden),
            "safe restriction denial leaked protected detail: {forbidden}"
        );
    }
}

fn unique_id(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after Unix epoch")
        .as_nanos();
    format!("{prefix}-{}-{nanos}", std::process::id())
}
