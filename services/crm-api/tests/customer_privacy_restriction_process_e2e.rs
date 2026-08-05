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
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tonic::{Code, Status};

use support::{
    TENANT_A, TENANT_B, connect_grpc, free_port, mutate, mutation_definition, payload, query,
    query_definition, spawn_crm_api, stop_process, wait_until_ready,
};

const PARTY_CREATE: &str = "parties.party.create";
const CONTACT_POINT_CREATE: &str = "contact-points.contact-point.create";
const RESTRICTION_PLACE: &str = "customer_privacy.restriction.place";
const RESTRICTION_RELEASE: &str = "customer_privacy.restriction.release";
const RESTRICTION_GET: &str = "customer_privacy.restriction.get";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EvidenceCounts {
    records: i64,
    events: i64,
    audits: i64,
    idempotency: i64,
    transactions: i64,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn restriction_lifecycle_blocks_then_releases_protected_contact_point_create_atomically() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping restriction lifecycle process test because DATABASE_URL is absent");
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
    ] {
        admin
            .execute(sqlx::raw_sql(fixture))
            .await
            .expect("publish restriction process registry fixture");
    }

    let party_create = mutation_definition(PARTY_CREATE);
    let contact_create = mutation_definition(CONTACT_POINT_CREATE);
    let restriction_place = mutation_definition(RESTRICTION_PLACE);
    let restriction_release = mutation_definition(RESTRICTION_RELEASE);
    let restriction_get = query_definition(RESTRICTION_GET);
    for definition in [&restriction_place, &restriction_release, &restriction_get] {
        assert_eq!(definition.owner_module_id.as_str(), "crm.customer-privacy");
    }

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

    let effective_from_unix_ms = now_millis() + 5_000;
    let privacy_before = restriction_place_evidence(&admin).await;
    let placement = mutate(
        &mut grpc,
        &restriction_place,
        restriction_payload(&restriction_place, &protected_party, effective_from_unix_ms),
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
    let restriction_ref = placed
        .processing_restriction_ref
        .clone()
        .expect("placed processing restriction reference");
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
    assert_eq!(placed.effective_from_unix_ms, effective_from_unix_ms);
    assert_incremented(privacy_before, restriction_place_evidence(&admin).await);
    wait_until_effective(effective_from_unix_ms).await;

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

    let before_get = restriction_module_evidence(&admin).await;
    let active = get_restriction(
        &mut grpc,
        &restriction_get,
        restriction_ref.clone(),
        TENANT_A,
        true,
    )
    .await
    .expect("read active restriction through production query ingress");
    assert_eq!(
        active.status,
        privacy::ProcessingRestrictionStatus::Active as i32
    );
    assert_eq!(active.version, 1);
    assert_eq!(restriction_module_evidence(&admin).await, before_get);

    let unauthenticated = get_restriction(
        &mut grpc,
        &restriction_get,
        restriction_ref.clone(),
        TENANT_A,
        false,
    )
    .await
    .expect_err("unauthenticated restriction read must be denied");
    assert_eq!(unauthenticated.code(), Code::Unauthenticated);
    assert_eq!(restriction_module_evidence(&admin).await, before_get);

    let cross_tenant = get_restriction(
        &mut grpc,
        &restriction_get,
        restriction_ref.clone(),
        TENANT_B,
        true,
    )
    .await
    .expect_err("cross-tenant restriction read must be concealed");
    assert_eq!(cross_tenant.code(), Code::NotFound);
    assert_eq!(restriction_module_evidence(&admin).await, before_get);

    let release_before = restriction_release_evidence(&admin).await;
    let release = mutate(
        &mut grpc,
        &restriction_release,
        payload(
            &restriction_release,
            privacy::ReleaseProcessingRestrictionRequest {
                processing_restriction_ref: Some(restriction_ref.clone()),
                expected_version: 1,
            },
        ),
        TENANT_A,
        "restriction-process-release",
        true,
    )
    .await
    .expect("release processing restriction through generic ingress");
    let released = privacy::ReleaseProcessingRestrictionResponse::decode(
        release
            .output
            .as_ref()
            .expect("restriction release output")
            .payload
            .as_slice(),
    )
    .expect("decode restriction release response")
    .processing_restriction
    .expect("released processing restriction");
    assert_eq!(
        released.status,
        privacy::ProcessingRestrictionStatus::Released as i32
    );
    assert_eq!(released.version, 2);
    assert_release_incremented(release_before, restriction_release_evidence(&admin).await);

    let after_release = restriction_module_evidence(&admin).await;
    let replay = mutate(
        &mut grpc,
        &restriction_release,
        payload(
            &restriction_release,
            privacy::ReleaseProcessingRestrictionRequest {
                processing_restriction_ref: Some(restriction_ref.clone()),
                expected_version: 1,
            },
        ),
        TENANT_A,
        "restriction-process-release",
        true,
    )
    .await
    .expect("exact restriction release replay must succeed");
    assert_eq!(replay.output, release.output);
    assert_eq!(restriction_module_evidence(&admin).await, after_release);

    let released_read = get_restriction(
        &mut grpc,
        &restriction_get,
        restriction_ref,
        TENANT_A,
        true,
    )
    .await
    .expect("read released restriction through production query ingress");
    assert_eq!(
        released_read.status,
        privacy::ProcessingRestrictionStatus::Released as i32
    );
    assert_eq!(released_read.version, 2);
    assert_eq!(restriction_module_evidence(&admin).await, after_release);

    let before_after_release = contact_point_evidence(&admin).await;
    mutate(
        &mut grpc,
        &contact_create,
        contact_point_payload(
            &contact_create,
            &unique_id("restriction-after-release-contact"),
            &protected_party,
            "after-release@example.com",
        ),
        TENANT_A,
        "restriction-process-after-release-contact",
        true,
    )
    .await
    .expect("released restriction must no longer deny protected owner mutation");
    assert_incremented(
        before_after_release,
        contact_point_evidence(&admin).await,
    );

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

async fn get_restriction(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: &CapabilityDefinition,
    restriction_ref: privacy::ProcessingRestrictionRef,
    tenant_id: &str,
    authenticated: bool,
) -> Result<privacy::ProcessingRestriction, Status> {
    let response = query(
        client,
        definition,
        payload(
            definition,
            privacy::GetProcessingRestrictionRequest {
                processing_restriction_ref: Some(restriction_ref),
            },
        ),
        tenant_id,
        authenticated,
    )
    .await?;
    Ok(privacy::GetProcessingRestrictionResponse::decode(
        response
            .output
            .expect("restriction query output")
            .payload
            .as_slice(),
    )
    .expect("decode restriction query response")
    .processing_restriction
    .expect("queried processing restriction"))
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

fn restriction_payload(
    definition: &CapabilityDefinition,
    party_id: &str,
    effective_from_unix_ms: i64,
) -> TypedPayload {
    payload(
        definition,
        privacy::PlaceProcessingRestrictionRequest {
            canonical_party_ref: Some(customer::PartyRef {
                party_id: party_id.to_owned(),
            }),
            scope: privacy::ProcessingRestrictionScope::Processing as i32,
            policy_version: "privacy-policy/1".to_owned(),
            effective_from_unix_ms,
            expires_at_unix_ms: None,
        },
    )
}

async fn contact_point_evidence(admin: &PgPool) -> EvidenceCounts {
    evidence(
        admin,
        "crm.contact-points",
        "contact-points.contact_point",
        CONTACT_POINT_CREATE,
        "contact-points.contact-point.created",
    )
    .await
}

async fn restriction_place_evidence(admin: &PgPool) -> EvidenceCounts {
    evidence(
        admin,
        "crm.customer-privacy",
        "customer-privacy.restriction",
        RESTRICTION_PLACE,
        "customer_privacy.restriction.placed",
    )
    .await
}

async fn restriction_release_evidence(admin: &PgPool) -> EvidenceCounts {
    evidence(
        admin,
        "crm.customer-privacy",
        "customer-privacy.restriction",
        RESTRICTION_RELEASE,
        "customer_privacy.restriction.released",
    )
    .await
}

async fn restriction_module_evidence(admin: &PgPool) -> EvidenceCounts {
    let records = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.records WHERE tenant_id = $1 AND owner_module_id = 'crm.customer-privacy' AND record_type = 'customer-privacy.restriction' AND deleted_at IS NULL",
    )
    .bind(TENANT_A)
    .fetch_one(admin)
    .await
    .expect("count restriction records");
    let events = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.outbox_events WHERE tenant_id = $1 AND event_type LIKE 'customer_privacy.restriction.%'",
    )
    .bind(TENANT_A)
    .fetch_one(admin)
    .await
    .expect("count restriction events");
    let audits = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.audit_records WHERE tenant_id = $1 AND capability_id LIKE 'customer_privacy.restriction.%'",
    )
    .bind(TENANT_A)
    .fetch_one(admin)
    .await
    .expect("count restriction audits");
    let idempotency = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.idempotency_records AS i JOIN crm.business_transactions AS b USING (tenant_id, business_transaction_id) WHERE b.tenant_id = $1 AND b.capability_id LIKE 'customer_privacy.restriction.%'",
    )
    .bind(TENANT_A)
    .fetch_one(admin)
    .await
    .expect("count restriction idempotency evidence");
    let transactions = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.business_transactions WHERE tenant_id = $1 AND capability_id LIKE 'customer_privacy.restriction.%'",
    )
    .bind(TENANT_A)
    .fetch_one(admin)
    .await
    .expect("count restriction business transactions");
    EvidenceCounts {
        records,
        events,
        audits,
        idempotency,
        transactions,
    }
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

fn assert_release_incremented(before: EvidenceCounts, after: EvidenceCounts) {
    assert_eq!(after.records, before.records);
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
    for forbidden in [
        "blocked@example.com",
        "payload_bytes",
        "postgres://",
        "sqlx",
        "SELECT",
    ] {
        assert!(
            !status.message().contains(forbidden)
                && !format!("{:?}", status.metadata()).contains(forbidden),
            "safe restriction denial leaked protected detail: {forbidden}"
        );
    }
}

async fn wait_until_effective(effective_from_unix_ms: i64) {
    let remaining_millis = effective_from_unix_ms
        .saturating_sub(now_millis())
        .saturating_add(50);
    if remaining_millis > 0 {
        tokio::time::sleep(Duration::from_millis(
            u64::try_from(remaining_millis).expect("positive effective delay fits u64"),
        ))
        .await;
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
