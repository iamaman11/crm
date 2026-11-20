#![cfg(unix)]

use crm_application_runtime::{
    application_mutation_definitions, application_query_definitions,
    gateway_v1::{
        MutateRequest as GatewayMutateRequest, QueryRequest as GatewayQueryRequest,
        TypedPayload as GatewayTypedPayload,
        application_gateway_service_client::ApplicationGatewayServiceClient,
    },
};
use crm_capability_runtime::CapabilityDefinition;
use crm_module_sdk::{DataClass, PayloadEncoding, RetentionPolicyId, TypedPayload};
use crm_proto_contracts::crm::{
    contact_points::v1 as contact_points, core::v1 as core, customer::v1 as customer,
    parties::v1 as parties,
};
use prost::Message;
use sqlx::{Executor, PgPool};
use std::collections::BTreeSet;
use std::net::TcpListener;
use std::process::Stdio;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::process::{Child, Command};
use tokio::time::{sleep, timeout};
use tonic::{Code, Request, Status};

const TENANT_A: &str = "tenant-a";
const TENANT_B: &str = "tenant-b";
const ACTOR: &str = "actor-a";
const TOKEN: &str = "contact-point-process-bearer-token-0123456789abcdef0123456789abcdef";
const PARTY_CREATE: &str = "parties.party.create";
const CONTACT_POINT_CREATE: &str = "contact-points.contact-point.create";
const CONTACT_POINT_UPDATE: &str = "contact-points.contact-point.update";
const CONTACT_POINT_VERIFY: &str = "contact-points.contact-point.verify";
const CONTACT_POINT_GET: &str = "contact-points.contact-point.get";
const CONTACT_POINT_LIST: &str = "contact-points.contact-point.list";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EvidenceCounts {
    records: i64,
    events: i64,
    audits: i64,
    idempotency: i64,
    transactions: i64,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn crm_api_process_serves_governed_contact_point_lifecycle_and_party_integrity() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping Contact Point process acceptance because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect Contact Point process evidence reader");
    admin
        .execute(sqlx::raw_sql(include_str!(
            "../../../database/tests/0005_party_adapter.sql"
        )))
        .await
        .expect("publish Party module/capability registry fixture");
    admin
        .execute(sqlx::raw_sql(include_str!(
            "../../../database/tests/0007_contact_point_adapter.sql"
        )))
        .await
        .expect("publish Contact Point module/capability registry fixture");

    let http_port = free_port();
    let mut grpc_port = free_port();
    while grpc_port == http_port {
        grpc_port = free_port();
    }
    let http_addr = format!("127.0.0.1:{http_port}");
    let grpc_addr = format!("127.0.0.1:{grpc_port}");

    let mut child = Command::new(env!("CARGO_BIN_EXE_crm-api"))
        .env("CRM_DATABASE_URL", &database_url)
        .env("CRM_HTTP_BIND", &http_addr)
        .env("CRM_GRPC_BIND", &grpc_addr)
        .env("CRM_API_BEARER_TOKEN", TOKEN)
        .env("CRM_API_ACTOR_ID", ACTOR)
        .env("CRM_API_TENANTS", format!("{TENANT_A},{TENANT_B}"))
        .env(
            "CRM_CURSOR_SIGNING_KEY",
            "contact-point-process-cursor-signing-key-0123456789abcdef",
        )
        .env(
            "CRM_APPROVAL_SIGNING_KEY",
            "contact-point-process-approval-signing-key-0123456789abcdef",
        )
        .env("CRM_BOOTSTRAP_ALLOW_PHASE6", "true")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn production crm-api process for Contact Point acceptance");

    let http = reqwest::Client::new();
    wait_until_ready(&http, &mut child, &http_addr).await;
    let mut grpc = connect_grpc(&grpc_addr).await;

    let party_create = mutation_definition(PARTY_CREATE);
    let party_a_id = unique_id("party-contact-point-a");
    let party_b_id = unique_id("party-contact-point-b");
    create_party(
        &mut grpc,
        &party_create,
        TENANT_A,
        &party_a_id,
        parties::PartyKind::Person,
        "Ada Contact Owner",
        "contact-point-process-party-a",
    )
    .await;
    create_party(
        &mut grpc,
        &party_create,
        TENANT_B,
        &party_b_id,
        parties::PartyKind::Person,
        "Tenant B Contact Owner",
        "contact-point-process-party-b",
    )
    .await;

    let create = mutation_definition(CONTACT_POINT_CREATE);
    let update = mutation_definition(CONTACT_POINT_UPDATE);
    let verify = mutation_definition(CONTACT_POINT_VERIFY);
    let get = query_definition(CONTACT_POINT_GET);
    let list = query_definition(CONTACT_POINT_LIST);
    let baseline = contact_point_evidence_counts(&admin, TENANT_A).await;

    let unauthenticated_mutation = mutate(
        &mut grpc,
        &create,
        create_contact_point_payload(
            &create,
            &unique_id("contact-point-unauthenticated"),
            &party_a_id,
            contact_points::ContactPointKind::Email,
            "unauthenticated@example.com",
            false,
        ),
        TENANT_A,
        "contact-point-process-unauthenticated-create",
        false,
    )
    .await
    .expect_err("unauthenticated Contact Point mutation must fail");
    assert_eq!(unauthenticated_mutation.code(), Code::Unauthenticated);
    assert_eq!(
        contact_point_evidence_counts(&admin, TENANT_A).await,
        baseline
    );

    let missing_status = mutate(
        &mut grpc,
        &create,
        create_contact_point_payload(
            &create,
            &unique_id("contact-point-missing-party"),
            &unique_id("party-missing"),
            contact_points::ContactPointKind::Email,
            "missing@example.com",
            false,
        ),
        TENANT_A,
        "contact-point-process-missing-party",
        true,
    )
    .await
    .expect_err("missing Party reference must be rejected");
    assert_eq!(missing_status.code(), Code::InvalidArgument);
    assert_eq!(
        contact_point_evidence_counts(&admin, TENANT_A).await,
        baseline
    );

    let cross_tenant_status = mutate(
        &mut grpc,
        &create,
        create_contact_point_payload(
            &create,
            &unique_id("contact-point-cross-tenant-party"),
            &party_b_id,
            contact_points::ContactPointKind::Email,
            "cross-tenant@example.com",
            false,
        ),
        TENANT_A,
        "contact-point-process-cross-tenant-party",
        true,
    )
    .await
    .expect_err("tenant A must not reference tenant B Party");
    assert_eq!(cross_tenant_status.code(), Code::InvalidArgument);
    assert_eq!(cross_tenant_status.message(), missing_status.message());
    assert_eq!(
        contact_point_evidence_counts(&admin, TENANT_A).await,
        baseline
    );

    let primary_id = unique_id("contact-point-primary-phone");
    let primary_payload = create_contact_point_payload(
        &create,
        &primary_id,
        &party_a_id,
        contact_points::ContactPointKind::Phone,
        "+370 (612) 34-567",
        true,
    );
    let created = mutate(
        &mut grpc,
        &create,
        primary_payload.clone(),
        TENANT_A,
        "contact-point-process-create-primary",
        true,
    )
    .await
    .expect("create Contact Point through production gateway");
    assert!(!created.replayed);
    assert_eq!(created.affected_resources.len(), 1);
    assert_eq!(
        created.affected_resources[0].resource_type,
        "contact-points.contact_point"
    );
    assert_eq!(created.affected_resources[0].resource_id, primary_id);
    assert_eq!(created.affected_resources[0].version, Some(1));
    let created_output = created.output.expect("Contact Point create output");
    let created_contact_point = decode_create_contact_point(&created_output.payload);
    assert_eq!(contact_point_id_of(&created_contact_point), primary_id);
    assert_eq!(party_id_of(&created_contact_point), party_a_id);
    assert_eq!(created_contact_point.normalized_value, "+37061234567");
    assert_eq!(created_contact_point.display_value, "+370 (612) 34-567");
    assert_eq!(
        created_contact_point.status,
        contact_points::ContactPointStatus::Active as i32
    );
    assert!(created_contact_point.preferred);
    assert_eq!(
        verification_status(&created_contact_point),
        contact_points::ContactPointVerificationStatus::Unverified
    );
    assert_eq!(resource_version(&created_contact_point), 1);

    let after_create = contact_point_evidence_counts(&admin, TENANT_A).await;
    assert_eq!(after_create.records, baseline.records + 1);
    assert_eq!(after_create.events, baseline.events + 1);
    assert_eq!(after_create.audits, baseline.audits + 1);
    assert_eq!(after_create.idempotency, baseline.idempotency + 1);
    assert_eq!(after_create.transactions, baseline.transactions + 1);

    let replay = mutate(
        &mut grpc,
        &create,
        primary_payload,
        TENANT_A,
        "contact-point-process-create-primary",
        true,
    )
    .await
    .expect("replay Contact Point create through production gateway");
    assert!(replay.replayed);
    assert_eq!(
        replay.output.expect("Contact Point replay output").payload,
        created_output.payload
    );
    assert_eq!(
        contact_point_evidence_counts(&admin, TENANT_A).await,
        after_create
    );

    let idempotency_conflict = mutate(
        &mut grpc,
        &create,
        create_contact_point_payload(
            &create,
            &unique_id("contact-point-conflicting-replay"),
            &party_a_id,
            contact_points::ContactPointKind::Email,
            "different@example.com",
            false,
        ),
        TENANT_A,
        "contact-point-process-create-primary",
        true,
    )
    .await
    .expect_err("same idempotency key with a different request must conflict");
    assert_eq!(idempotency_conflict.code(), Code::Aborted);
    assert_eq!(
        contact_point_evidence_counts(&admin, TENANT_A).await,
        after_create
    );

    let queried = query(
        &mut grpc,
        &get,
        get_contact_point_payload(&get, &primary_id),
        TENANT_A,
        true,
    )
    .await
    .expect("get Contact Point through production query gateway");
    let queried_contact_point = decode_get_contact_point(queried);
    assert_eq!(queried_contact_point.normalized_value, "+37061234567");
    assert_eq!(resource_version(&queried_contact_point), 1);

    let verify_payload =
        verify_contact_point_payload(&verify, &primary_id, 1, "verification-evidence-primary");
    let verified = mutate(
        &mut grpc,
        &verify,
        verify_payload.clone(),
        TENANT_A,
        "contact-point-process-verify-primary-v1",
        true,
    )
    .await
    .expect("verify Contact Point through production gateway");
    assert!(!verified.replayed);
    assert_eq!(verified.affected_resources[0].version, Some(2));
    let verified_output = verified.output.expect("Contact Point verify output");
    let verified_contact_point = decode_verify_contact_point(&verified_output.payload);
    assert_eq!(
        verification_status(&verified_contact_point),
        contact_points::ContactPointVerificationStatus::Verified
    );
    assert_eq!(resource_version(&verified_contact_point), 2);
    let verification = verified_contact_point
        .verification
        .as_ref()
        .expect("verified Contact Point verification state");
    assert_eq!(
        verification.evidence_ref.as_deref(),
        Some("verification-evidence-primary")
    );
    assert!(verification.verified_at.is_some());

    let after_verify = contact_point_evidence_counts(&admin, TENANT_A).await;
    assert_eq!(after_verify.records, after_create.records);
    assert_eq!(after_verify.events, after_create.events + 1);
    assert_eq!(after_verify.audits, after_create.audits + 1);
    assert_eq!(after_verify.idempotency, after_create.idempotency + 1);
    assert_eq!(after_verify.transactions, after_create.transactions + 1);

    let verify_replay = mutate(
        &mut grpc,
        &verify,
        verify_payload,
        TENANT_A,
        "contact-point-process-verify-primary-v1",
        true,
    )
    .await
    .expect("replay Contact Point verification");
    assert!(verify_replay.replayed);
    assert_eq!(
        verify_replay
            .output
            .expect("Contact Point verify replay output")
            .payload,
        verified_output.payload
    );
    assert_eq!(
        contact_point_evidence_counts(&admin, TENANT_A).await,
        after_verify
    );

    let display_only_update = update_contact_point_payload(
        &update,
        &primary_id,
        2,
        "+370 612 34 567",
        contact_points::ContactPointStatus::Active,
        true,
    );
    let display_updated = mutate(
        &mut grpc,
        &update,
        display_only_update,
        TENANT_A,
        "contact-point-process-display-update-v2",
        true,
    )
    .await
    .expect("update Contact Point display formatting");
    let display_updated_contact_point = decode_update_contact_point(
        &display_updated
            .output
            .expect("display-only Contact Point update output")
            .payload,
    );
    assert_eq!(
        display_updated_contact_point.normalized_value,
        "+37061234567"
    );
    assert_eq!(
        verification_status(&display_updated_contact_point),
        contact_points::ContactPointVerificationStatus::Verified
    );
    assert_eq!(resource_version(&display_updated_contact_point), 3);

    let value_change_update = update_contact_point_payload(
        &update,
        &primary_id,
        3,
        "+370 699 99-999",
        contact_points::ContactPointStatus::Active,
        true,
    );
    let value_changed = mutate(
        &mut grpc,
        &update,
        value_change_update.clone(),
        TENANT_A,
        "contact-point-process-value-update-v3",
        true,
    )
    .await
    .expect("change Contact Point canonical endpoint value");
    let value_changed_output = value_changed
        .output
        .expect("value-changed Contact Point output");
    let value_changed_contact_point = decode_update_contact_point(&value_changed_output.payload);
    assert_eq!(value_changed_contact_point.normalized_value, "+37069999999");
    assert_eq!(
        verification_status(&value_changed_contact_point),
        contact_points::ContactPointVerificationStatus::Unverified
    );
    assert_eq!(resource_version(&value_changed_contact_point), 4);

    let after_value_change = contact_point_evidence_counts(&admin, TENANT_A).await;
    assert_eq!(after_value_change.events, after_verify.events + 2);
    assert_eq!(after_value_change.audits, after_verify.audits + 2);
    assert_eq!(after_value_change.idempotency, after_verify.idempotency + 2);
    assert_eq!(
        after_value_change.transactions,
        after_verify.transactions + 2
    );

    let update_replay = mutate(
        &mut grpc,
        &update,
        value_change_update,
        TENANT_A,
        "contact-point-process-value-update-v3",
        true,
    )
    .await
    .expect("replay Contact Point value change");
    assert!(update_replay.replayed);
    assert_eq!(
        update_replay
            .output
            .expect("Contact Point update replay output")
            .payload,
        value_changed_output.payload
    );
    assert_eq!(
        contact_point_evidence_counts(&admin, TENANT_A).await,
        after_value_change
    );

    let stale = mutate(
        &mut grpc,
        &update,
        update_contact_point_payload(
            &update,
            &primary_id,
            3,
            "+370 600 00-000",
            contact_points::ContactPointStatus::Active,
            true,
        ),
        TENANT_A,
        "contact-point-process-stale-update",
        true,
    )
    .await
    .expect_err("stale Contact Point version must fail");
    assert_eq!(stale.code(), Code::Aborted);
    assert_eq!(
        contact_point_evidence_counts(&admin, TENANT_A).await,
        after_value_change
    );

    let second_id = unique_id("contact-point-secondary-email");
    mutate(
        &mut grpc,
        &create,
        create_contact_point_payload(
            &create,
            &second_id,
            &party_a_id,
            contact_points::ContactPointKind::Email,
            "Ada@BÜCHER.Example",
            false,
        ),
        TENANT_A,
        "contact-point-process-create-secondary",
        true,
    )
    .await
    .expect("create secondary Contact Point");

    let third_id = unique_id("contact-point-third-email");
    mutate(
        &mut grpc,
        &create,
        create_contact_point_payload(
            &create,
            &third_id,
            &party_a_id,
            contact_points::ContactPointKind::Email,
            "verified@example.com",
            false,
        ),
        TENANT_A,
        "contact-point-process-create-third",
        true,
    )
    .await
    .expect("create third Contact Point");
    mutate(
        &mut grpc,
        &verify,
        verify_contact_point_payload(&verify, &third_id, 1, "verification-evidence-third"),
        TENANT_A,
        "contact-point-process-verify-third",
        true,
    )
    .await
    .expect("verify third Contact Point");

    let after_three = contact_point_evidence_counts(&admin, TENANT_A).await;
    assert_eq!(after_three.records, baseline.records + 3);

    let phone_filter = query(
        &mut grpc,
        &list,
        list_contact_points_payload(
            &list,
            10,
            "",
            Some(&party_a_id),
            Some(contact_points::ContactPointKind::Phone),
            Some(contact_points::ContactPointStatus::Active),
            Some(contact_points::ContactPointVerificationStatus::Unverified),
            Some(true),
        ),
        TENANT_A,
        true,
    )
    .await
    .expect("filter Contact Points by Party/kind/status/verification/preferred");
    let phone_filter = decode_list_contact_points(phone_filter);
    assert_eq!(phone_filter.contact_points.len(), 1);
    assert_eq!(
        contact_point_id_of(&phone_filter.contact_points[0]),
        primary_id
    );

    let verified_filter = query(
        &mut grpc,
        &list,
        list_contact_points_payload(
            &list,
            10,
            "",
            Some(&party_a_id),
            Some(contact_points::ContactPointKind::Email),
            Some(contact_points::ContactPointStatus::Active),
            Some(contact_points::ContactPointVerificationStatus::Verified),
            Some(false),
        ),
        TENANT_A,
        true,
    )
    .await
    .expect("filter verified Contact Points");
    let verified_filter = decode_list_contact_points(verified_filter);
    assert_eq!(verified_filter.contact_points.len(), 1);
    assert_eq!(
        contact_point_id_of(&verified_filter.contact_points[0]),
        third_id
    );

    let first_page = query(
        &mut grpc,
        &list,
        list_contact_points_payload(&list, 1, "", Some(&party_a_id), None, None, None, None),
        TENANT_A,
        true,
    )
    .await
    .expect("list first Contact Point page");
    let first_page = decode_list_contact_points(first_page);
    assert_eq!(first_page.contact_points.len(), 1);
    let next_page_token = first_page
        .page
        .as_ref()
        .expect("first Contact Point page info")
        .next_page_token
        .clone();
    assert!(!next_page_token.is_empty());

    let tampered_cursor = format!("{next_page_token}x");
    let tampered = query(
        &mut grpc,
        &list,
        list_contact_points_payload(
            &list,
            1,
            &tampered_cursor,
            Some(&party_a_id),
            None,
            None,
            None,
            None,
        ),
        TENANT_A,
        true,
    )
    .await
    .expect_err("tampered Contact Point cursor must be rejected");
    assert_eq!(tampered.code(), Code::InvalidArgument);

    let mut listed_ids = BTreeSet::new();
    listed_ids.insert(contact_point_id_of(&first_page.contact_points[0]).to_owned());
    let mut cursor = next_page_token;
    while !cursor.is_empty() {
        let page = query(
            &mut grpc,
            &list,
            list_contact_points_payload(
                &list,
                1,
                &cursor,
                Some(&party_a_id),
                None,
                None,
                None,
                None,
            ),
            TENANT_A,
            true,
        )
        .await
        .expect("list next Contact Point page");
        let page = decode_list_contact_points(page);
        assert_eq!(page.contact_points.len(), 1);
        listed_ids.insert(contact_point_id_of(&page.contact_points[0]).to_owned());
        cursor = page.page.expect("Contact Point page info").next_page_token;
    }
    assert_eq!(
        listed_ids,
        BTreeSet::from([primary_id.clone(), second_id.clone(), third_id.clone()])
    );

    let unauthenticated_query = query(
        &mut grpc,
        &get,
        get_contact_point_payload(&get, &primary_id),
        TENANT_A,
        false,
    )
    .await
    .expect_err("unauthenticated Contact Point query must fail");
    assert_eq!(unauthenticated_query.code(), Code::Unauthenticated);

    let cross_tenant_get = query(
        &mut grpc,
        &get,
        get_contact_point_payload(&get, &primary_id),
        TENANT_B,
        true,
    )
    .await
    .expect_err("tenant B must not discover tenant A Contact Point");
    assert_eq!(cross_tenant_get.code(), Code::NotFound);

    let cross_tenant_list = query(
        &mut grpc,
        &list,
        list_contact_points_payload(&list, 10, "", None, None, None, None, None),
        TENANT_B,
        true,
    )
    .await
    .expect("tenant B Contact Point list must not leak tenant A resources");
    assert!(
        decode_list_contact_points(cross_tenant_list)
            .contact_points
            .is_empty()
    );
    assert_eq!(
        contact_point_evidence_counts(&admin, TENANT_A).await,
        after_three
    );

    send_sigint(&child).await;
    let exit = timeout(Duration::from_secs(15), child.wait())
        .await
        .expect("crm-api must stop within graceful-shutdown budget")
        .expect("wait for Contact Point acceptance crm-api process");
    assert!(exit.success(), "crm-api exited unsuccessfully: {exit}");
}

async fn create_party(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: &CapabilityDefinition,
    tenant_id: &str,
    party_id: &str,
    kind: parties::PartyKind,
    display_name: &str,
    idempotency_key: &str,
) {
    let response = mutate(
        client,
        definition,
        payload(
            definition,
            parties::CreatePartyRequest {
                party_ref: Some(customer::PartyRef {
                    party_id: party_id.to_owned(),
                }),
                kind: kind as i32,
                display_name: display_name.to_owned(),
            },
        ),
        tenant_id,
        idempotency_key,
        true,
    )
    .await
    .expect("create Party prerequisite through production gateway");
    assert!(!response.replayed);
}

async fn mutate(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: &CapabilityDefinition,
    input: TypedPayload,
    tenant_id: &str,
    idempotency_key: &str,
    authenticated: bool,
) -> Result<crm_application_runtime::gateway_v1::MutateResponse, Status> {
    let mut request = Request::new(GatewayMutateRequest {
        owner_module_id: definition.owner_module_id.as_str().to_owned(),
        capability_id: definition.capability_id.as_str().to_owned(),
        capability_version: definition.capability_version.as_str().to_owned(),
        input: Some(wire_payload(input)),
        approval: None,
    });
    request.metadata_mut().insert(
        "x-tenant-id",
        tenant_id.parse().expect("valid tenant metadata"),
    );
    request.metadata_mut().insert(
        "idempotency-key",
        idempotency_key.parse().expect("valid idempotency metadata"),
    );
    if authenticated {
        request.metadata_mut().insert(
            "authorization",
            format!("Bearer {TOKEN}")
                .parse()
                .expect("valid authorization metadata"),
        );
    }
    client
        .mutate(request)
        .await
        .map(|response| response.into_inner())
}

async fn query(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: &CapabilityDefinition,
    input: TypedPayload,
    tenant_id: &str,
    authenticated: bool,
) -> Result<crm_application_runtime::gateway_v1::QueryResponse, Status> {
    let mut request = Request::new(GatewayQueryRequest {
        owner_module_id: definition.owner_module_id.as_str().to_owned(),
        capability_id: definition.capability_id.as_str().to_owned(),
        capability_version: definition.capability_version.as_str().to_owned(),
        input: Some(wire_payload(input)),
    });
    request.metadata_mut().insert(
        "x-tenant-id",
        tenant_id.parse().expect("valid tenant metadata"),
    );
    if authenticated {
        request.metadata_mut().insert(
            "authorization",
            format!("Bearer {TOKEN}")
                .parse()
                .expect("valid authorization metadata"),
        );
    }
    client
        .query(request)
        .await
        .map(|response| response.into_inner())
}

fn mutation_definition(capability_id: &str) -> CapabilityDefinition {
    application_mutation_definitions()
        .expect("valid application mutation definitions")
        .into_iter()
        .find(|definition| definition.capability_id.as_str() == capability_id)
        .unwrap_or_else(|| panic!("missing application mutation definition: {capability_id}"))
}

fn query_definition(capability_id: &str) -> CapabilityDefinition {
    application_query_definitions()
        .expect("valid application query definitions")
        .into_iter()
        .find(|definition| definition.capability_id.as_str() == capability_id)
        .unwrap_or_else(|| panic!("missing application query definition: {capability_id}"))
}

fn create_contact_point_payload(
    definition: &CapabilityDefinition,
    contact_point_id: &str,
    party_id: &str,
    kind: contact_points::ContactPointKind,
    value: &str,
    preferred: bool,
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
            kind: kind as i32,
            value: value.to_owned(),
            preferred,
            valid_from: None,
            valid_until: None,
        },
    )
}

fn update_contact_point_payload(
    definition: &CapabilityDefinition,
    contact_point_id: &str,
    expected_version: i64,
    value: &str,
    status: contact_points::ContactPointStatus,
    preferred: bool,
) -> TypedPayload {
    payload(
        definition,
        contact_points::UpdateContactPointRequest {
            contact_point_ref: Some(customer::ContactPointRef {
                contact_point_id: contact_point_id.to_owned(),
            }),
            expected_version,
            value: value.to_owned(),
            status: status as i32,
            preferred,
            valid_from: None,
            valid_until: None,
        },
    )
}

fn verify_contact_point_payload(
    definition: &CapabilityDefinition,
    contact_point_id: &str,
    expected_version: i64,
    evidence_ref: &str,
) -> TypedPayload {
    payload(
        definition,
        contact_points::VerifyContactPointRequest {
            contact_point_ref: Some(customer::ContactPointRef {
                contact_point_id: contact_point_id.to_owned(),
            }),
            expected_version,
            evidence_ref: evidence_ref.to_owned(),
        },
    )
}

fn get_contact_point_payload(
    definition: &CapabilityDefinition,
    contact_point_id: &str,
) -> TypedPayload {
    payload(
        definition,
        contact_points::GetContactPointRequest {
            contact_point_ref: Some(customer::ContactPointRef {
                contact_point_id: contact_point_id.to_owned(),
            }),
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn list_contact_points_payload(
    definition: &CapabilityDefinition,
    page_size: i32,
    page_token: &str,
    party_id: Option<&str>,
    kind: Option<contact_points::ContactPointKind>,
    status: Option<contact_points::ContactPointStatus>,
    verification_status: Option<contact_points::ContactPointVerificationStatus>,
    preferred: Option<bool>,
) -> TypedPayload {
    payload(
        definition,
        contact_points::ListContactPointsRequest {
            page: Some(core::PageRequest {
                page_size,
                page_token: page_token.to_owned(),
            }),
            party_ref: party_id.map(|party_id| customer::PartyRef {
                party_id: party_id.to_owned(),
            }),
            kind: kind.map(|value| value as i32),
            status: status.map(|value| value as i32),
            verification_status: verification_status.map(|value| value as i32),
            preferred,
            sort: contact_points::ContactPointSort::UpdatedAtDescending as i32,
        },
    )
}

fn decode_create_contact_point(bytes: &[u8]) -> contact_points::ContactPoint {
    contact_points::CreateContactPointResponse::decode(bytes)
        .expect("decode Contact Point create response")
        .contact_point
        .expect("created Contact Point exists")
}

fn decode_update_contact_point(bytes: &[u8]) -> contact_points::ContactPoint {
    contact_points::UpdateContactPointResponse::decode(bytes)
        .expect("decode Contact Point update response")
        .contact_point
        .expect("updated Contact Point exists")
}

fn decode_verify_contact_point(bytes: &[u8]) -> contact_points::ContactPoint {
    contact_points::VerifyContactPointResponse::decode(bytes)
        .expect("decode Contact Point verify response")
        .contact_point
        .expect("verified Contact Point exists")
}

fn decode_get_contact_point(
    response: crm_application_runtime::gateway_v1::QueryResponse,
) -> contact_points::ContactPoint {
    contact_points::GetContactPointResponse::decode(
        response
            .output
            .expect("Contact Point query output")
            .payload
            .as_slice(),
    )
    .expect("decode Contact Point query response")
    .contact_point
    .expect("queried Contact Point exists")
}

fn decode_list_contact_points(
    response: crm_application_runtime::gateway_v1::QueryResponse,
) -> contact_points::ListContactPointsResponse {
    contact_points::ListContactPointsResponse::decode(
        response
            .output
            .expect("Contact Point list output")
            .payload
            .as_slice(),
    )
    .expect("decode Contact Point list response")
}

fn contact_point_id_of(contact_point: &contact_points::ContactPoint) -> &str {
    contact_point
        .contact_point_ref
        .as_ref()
        .expect("Contact Point reference")
        .contact_point_id
        .as_str()
}

fn party_id_of(contact_point: &contact_points::ContactPoint) -> &str {
    contact_point
        .party_ref
        .as_ref()
        .expect("Contact Point Party reference")
        .party_id
        .as_str()
}

fn resource_version(contact_point: &contact_points::ContactPoint) -> i64 {
    contact_point
        .resource_version
        .as_ref()
        .expect("Contact Point resource version")
        .version
}

fn verification_status(
    contact_point: &contact_points::ContactPoint,
) -> contact_points::ContactPointVerificationStatus {
    contact_points::ContactPointVerificationStatus::try_from(
        contact_point
            .verification
            .as_ref()
            .expect("Contact Point verification")
            .status,
    )
    .expect("known Contact Point verification status")
}

fn payload<M: Message>(definition: &CapabilityDefinition, message: M) -> TypedPayload {
    let data_class = *definition
        .input_contract
        .allowed_data_classes
        .first()
        .expect("governed input contract must declare a data class");
    let payload = TypedPayload {
        owner: definition.input_contract.owner.clone(),
        schema_id: definition.input_contract.schema_id.clone(),
        schema_version: definition.input_contract.schema_version.clone(),
        descriptor_hash: definition.input_contract.descriptor_hash,
        data_class,
        encoding: PayloadEncoding::Protobuf,
        maximum_size_bytes: definition.input_contract.maximum_size_bytes,
        retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
        bytes: message.encode_to_vec(),
    };
    payload.validate().expect("valid governed input payload");
    payload
}

fn wire_payload(payload: TypedPayload) -> GatewayTypedPayload {
    GatewayTypedPayload {
        owner_module_id: payload.owner.as_str().to_owned(),
        schema_id: payload.schema_id.as_str().to_owned(),
        schema_version: payload.schema_version.as_str().to_owned(),
        descriptor_hash: payload.descriptor_hash.to_vec(),
        data_class: data_class_name(payload.data_class).to_owned(),
        encoding: "protobuf".to_owned(),
        maximum_size_bytes: payload.maximum_size_bytes,
        retention_policy_id: payload.retention_policy_id.as_str().to_owned(),
        payload: payload.bytes,
    }
}

fn data_class_name(data_class: DataClass) -> &'static str {
    match data_class {
        DataClass::Public => "public",
        DataClass::Internal => "internal",
        DataClass::Confidential => "confidential",
        DataClass::Restricted => "restricted",
        DataClass::Personal => "personal",
        DataClass::SensitivePersonal => "sensitive_personal",
        DataClass::Biometric => "biometric",
        DataClass::Financial => "financial",
        DataClass::Credential => "credential",
    }
}

async fn contact_point_evidence_counts(admin: &PgPool, tenant_id: &str) -> EvidenceCounts {
    let records = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.records WHERE tenant_id = $1 AND owner_module_id = 'crm.contact-points' AND record_type = 'contact-points.contact_point' AND deleted_at IS NULL",
    )
    .bind(tenant_id)
    .fetch_one(admin)
    .await
    .expect("count Contact Point records");
    let events = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.outbox_events WHERE tenant_id = $1 AND event_type IN ('contact-points.contact-point.created', 'contact-points.contact-point.updated', 'contact-points.contact-point.verified')",
    )
    .bind(tenant_id)
    .fetch_one(admin)
    .await
    .expect("count Contact Point outbox events");
    let audits =
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM crm.audit_records WHERE tenant_id = $1")
            .bind(tenant_id)
            .fetch_one(admin)
            .await
            .expect("count Contact Point audit evidence");
    let idempotency = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.idempotency_records WHERE tenant_id = $1",
    )
    .bind(tenant_id)
    .fetch_one(admin)
    .await
    .expect("count Contact Point idempotency evidence");
    let transactions = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.business_transactions WHERE tenant_id = $1",
    )
    .bind(tenant_id)
    .fetch_one(admin)
    .await
    .expect("count Contact Point business transactions");
    EvidenceCounts {
        records,
        events,
        audits,
        idempotency,
        transactions,
    }
}

async fn wait_until_ready(client: &reqwest::Client, child: &mut Child, http_addr: &str) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child.try_wait().expect("poll crm-api process") {
            panic!("crm-api exited before Contact Point acceptance readiness: {status}");
        }
        if let Ok(response) = client
            .get(format!("http://{http_addr}/readyz"))
            .send()
            .await
            && response.status().is_success()
        {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "Contact Point acceptance crm-api readiness timed out"
        );
        sleep(Duration::from_millis(200)).await;
    }
}

async fn connect_grpc(
    grpc_addr: &str,
) -> ApplicationGatewayServiceClient<tonic::transport::Channel> {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        match ApplicationGatewayServiceClient::connect(format!("http://{grpc_addr}")).await {
            Ok(client) => return client,
            Err(error) => {
                assert!(
                    Instant::now() < deadline,
                    "Contact Point acceptance gRPC listener timed out: {error}"
                );
                sleep(Duration::from_millis(200)).await;
            }
        }
    }
}

async fn send_sigint(child: &Child) {
    let pid = child.id().expect("running crm-api process has a PID");
    let status = Command::new("kill")
        .arg("-INT")
        .arg(pid.to_string())
        .status()
        .await
        .expect("send SIGINT to Contact Point acceptance crm-api");
    assert!(status.success(), "kill -INT failed: {status}");
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral Contact Point acceptance port")
        .local_addr()
        .expect("read ephemeral Contact Point acceptance port")
        .port()
}

fn unique_id(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after Unix epoch")
        .as_nanos();
    format!("{prefix}-{}-{nanos}", std::process::id())
}
