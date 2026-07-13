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
    consents::v1 as consents, contact_points::v1 as contact_points, core::v1 as core,
    customer::v1 as customer, parties::v1 as parties,
};
use prost::Message;
use sqlx::{Executor, PgPool};
use std::net::TcpListener;
use std::process::Stdio;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::process::{Child, Command};
use tokio::time::{sleep, timeout};
use tonic::{Code, Request, Status};

const TENANT_A: &str = "tenant-a";
const TENANT_B: &str = "tenant-b";
const ACTOR: &str = "actor-a";
const TOKEN: &str = "consent-process-bearer-token-0123456789abcdef0123456789abcdef";
const PARTY_CREATE: &str = "parties.party.create";
const CONTACT_POINT_CREATE: &str = "contact-points.contact-point.create";
const CONTACT_POINT_VERIFY: &str = "contact-points.contact-point.verify";
const CONSENT_CREATE: &str = "consents.authorization.create";
const CONSENT_WITHDRAW: &str = "consents.authorization.withdraw";
const CONSENT_GET: &str = "consents.authorization.get";
const CONSENT_LIST: &str = "consents.authorization.list";
const COMMUNICATION_AUTHORIZE: &str = "consents.communication.authorize";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EvidenceCounts {
    records: i64,
    events: i64,
    audits: i64,
    idempotency: i64,
    transactions: i64,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn crm_api_process_proves_authoritative_consent_and_communication_authorization() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping Consent process acceptance because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect Consent process evidence reader");
    for fixture in [
        include_str!("../../../database/tests/0005_party_adapter.sql"),
        include_str!("../../../database/tests/0007_contact_point_adapter.sql"),
        include_str!("../../../database/tests/0009_consent_adapter.sql"),
    ] {
        admin
            .execute(sqlx::raw_sql(fixture))
            .await
            .expect("publish production adapter registry fixture");
    }

    let http_port = free_port();
    let mut grpc_port = free_port();
    while grpc_port == http_port {
        grpc_port = free_port();
    }
    let http_addr = format!("127.0.0.1:{http_port}");
    let grpc_addr = format!("127.0.0.1:{grpc_port}");
    let mut child = spawn_crm_api(&database_url, &http_addr, &grpc_addr);
    let http = reqwest::Client::new();
    wait_until_ready(&http, &mut child, &http_addr).await;
    let mut grpc = connect_grpc(&grpc_addr).await;

    let party_create = mutation_definition(PARTY_CREATE);
    let contact_point_create = mutation_definition(CONTACT_POINT_CREATE);
    let contact_point_verify = mutation_definition(CONTACT_POINT_VERIFY);
    let consent_create = mutation_definition(CONSENT_CREATE);
    let consent_withdraw = mutation_definition(CONSENT_WITHDRAW);
    let consent_get = query_definition(CONSENT_GET);
    let consent_list = query_definition(CONSENT_LIST);
    let communication_authorize = query_definition(COMMUNICATION_AUTHORIZE);

    let party_a = unique_id("party-consent-a");
    let party_a_peer = unique_id("party-consent-a-peer");
    let party_b = unique_id("party-consent-b");
    create_party(
        &mut grpc,
        &party_create,
        TENANT_A,
        &party_a,
        "Consent Subject A",
        "consent-party-a",
    )
    .await;
    create_party(
        &mut grpc,
        &party_create,
        TENANT_A,
        &party_a_peer,
        "Consent Subject Peer",
        "consent-party-a-peer",
    )
    .await;
    create_party(
        &mut grpc,
        &party_create,
        TENANT_B,
        &party_b,
        "Consent Subject B",
        "consent-party-b",
    )
    .await;

    let email_a = unique_id("contact-point-consent-email-a");
    let phone_a = unique_id("contact-point-consent-phone-a");
    let email_peer = unique_id("contact-point-consent-email-peer");
    let email_b = unique_id("contact-point-consent-email-b");
    create_contact_point(
        &mut grpc,
        &contact_point_create,
        TENANT_A,
        &email_a,
        &party_a,
        contact_points::ContactPointKind::Email,
        "consent-a@example.com",
        true,
        "consent-contact-email-a",
    )
    .await;
    create_contact_point(
        &mut grpc,
        &contact_point_create,
        TENANT_A,
        &phone_a,
        &party_a,
        contact_points::ContactPointKind::Phone,
        "+37061234567",
        false,
        "consent-contact-phone-a",
    )
    .await;
    create_contact_point(
        &mut grpc,
        &contact_point_create,
        TENANT_A,
        &email_peer,
        &party_a_peer,
        contact_points::ContactPointKind::Email,
        "consent-peer@example.com",
        false,
        "consent-contact-email-peer",
    )
    .await;
    create_contact_point(
        &mut grpc,
        &contact_point_create,
        TENANT_B,
        &email_b,
        &party_b,
        contact_points::ContactPointKind::Email,
        "consent-b@example.com",
        false,
        "consent-contact-email-b",
    )
    .await;
    mutate(
        &mut grpc,
        &contact_point_verify,
        verify_contact_point_payload(&contact_point_verify, &email_a, 1, "evidence://contact-point/verified"),
        TENANT_A,
        "consent-contact-verify-email-a",
        true,
    )
    .await
    .expect("verify preferred Contact Point prerequisite");

    let baseline = consent_evidence_counts(&admin, TENANT_A).await;

    let contact_point_only = authorize(
        &mut grpc,
        &communication_authorize,
        TENANT_A,
        &party_a,
        "marketing.contact-point-only",
        consents::CommunicationChannel::Email,
        Some(&email_a),
        true,
    )
    .await
    .expect("evaluate verified/preferred Contact Point without Consent assertion");
    assert_denied(
        &contact_point_only,
        consents::CommunicationAuthorizationReason::NoApplicableGrant,
    );
    assert_eq!(consent_evidence_counts(&admin, TENANT_A).await, baseline);

    let rejection_effective = future_nanos(Duration::from_secs(30));
    let missing_party = mutate(
        &mut grpc,
        &consent_create,
        create_consent_payload(
            &consent_create,
            &unique_id("consent-missing-party"),
            &unique_id("party-missing"),
            None,
            "marketing.reference-check",
            consents::CommunicationChannel::Email,
            consents::ConsentEffect::Grant,
            rejection_effective,
            None,
        ),
        TENANT_A,
        "consent-missing-party",
        true,
    )
    .await
    .expect_err("missing Party must be rejected before Consent mutation");
    let cross_tenant_party = mutate(
        &mut grpc,
        &consent_create,
        create_consent_payload(
            &consent_create,
            &unique_id("consent-cross-tenant-party"),
            &party_b,
            None,
            "marketing.reference-check",
            consents::CommunicationChannel::Email,
            consents::ConsentEffect::Grant,
            rejection_effective,
            None,
        ),
        TENANT_A,
        "consent-cross-tenant-party",
        true,
    )
    .await
    .expect_err("cross-tenant Party must be rejected before Consent mutation");
    let missing_contact_point = mutate(
        &mut grpc,
        &consent_create,
        create_consent_payload(
            &consent_create,
            &unique_id("consent-missing-contact-point"),
            &party_a,
            Some(&unique_id("contact-point-missing")),
            "marketing.reference-check",
            consents::CommunicationChannel::Email,
            consents::ConsentEffect::Grant,
            rejection_effective,
            None,
        ),
        TENANT_A,
        "consent-missing-contact-point",
        true,
    )
    .await
    .expect_err("missing Contact Point must be rejected before Consent mutation");
    let cross_tenant_contact_point = mutate(
        &mut grpc,
        &consent_create,
        create_consent_payload(
            &consent_create,
            &unique_id("consent-cross-tenant-contact-point"),
            &party_a,
            Some(&email_b),
            "marketing.reference-check",
            consents::CommunicationChannel::Email,
            consents::ConsentEffect::Grant,
            rejection_effective,
            None,
        ),
        TENANT_A,
        "consent-cross-tenant-contact-point",
        true,
    )
    .await
    .expect_err("cross-tenant Contact Point must be rejected before Consent mutation");
    let wrong_owner_contact_point = mutate(
        &mut grpc,
        &consent_create,
        create_consent_payload(
            &consent_create,
            &unique_id("consent-wrong-owner-contact-point"),
            &party_a,
            Some(&email_peer),
            "marketing.reference-check",
            consents::CommunicationChannel::Email,
            consents::ConsentEffect::Grant,
            rejection_effective,
            None,
        ),
        TENANT_A,
        "consent-wrong-owner-contact-point",
        true,
    )
    .await
    .expect_err("Contact Point owned by another Party must be rejected");
    let channel_mismatch = mutate(
        &mut grpc,
        &consent_create,
        create_consent_payload(
            &consent_create,
            &unique_id("consent-channel-mismatch"),
            &party_a,
            Some(&phone_a),
            "marketing.reference-check",
            consents::CommunicationChannel::Email,
            consents::ConsentEffect::Grant,
            rejection_effective,
            None,
        ),
        TENANT_A,
        "consent-channel-mismatch",
        true,
    )
    .await
    .expect_err("deterministic channel mismatch must be rejected");
    for failure in [
        &missing_party,
        &cross_tenant_party,
        &missing_contact_point,
        &cross_tenant_contact_point,
        &wrong_owner_contact_point,
        &channel_mismatch,
    ] {
        assert_eq!(failure.code(), Code::InvalidArgument);
        assert_eq!(failure.message(), missing_party.message());
    }
    assert_eq!(consent_evidence_counts(&admin, TENANT_A).await, baseline);

    let unauthenticated = mutate(
        &mut grpc,
        &consent_create,
        create_consent_payload(
            &consent_create,
            &unique_id("consent-unauthenticated"),
            &party_a,
            None,
            "marketing.unauthenticated",
            consents::CommunicationChannel::Email,
            consents::ConsentEffect::Grant,
            rejection_effective,
            None,
        ),
        TENANT_A,
        "consent-unauthenticated",
        false,
    )
    .await
    .expect_err("unauthenticated Consent mutation must fail");
    assert_eq!(unauthenticated.code(), Code::Unauthenticated);
    assert_eq!(consent_evidence_counts(&admin, TENANT_A).await, baseline);

    let batch_effective = future_nanos(Duration::from_secs(4));
    let newsletter_id = unique_id("consent-newsletter-grant");
    let newsletter_payload = create_consent_payload(
        &consent_create,
        &newsletter_id,
        &party_a,
        None,
        "marketing.newsletter",
        consents::CommunicationChannel::Email,
        consents::ConsentEffect::Grant,
        batch_effective,
        None,
    );
    let newsletter_created = mutate(
        &mut grpc,
        &consent_create,
        newsletter_payload.clone(),
        TENANT_A,
        "consent-create-newsletter",
        true,
    )
    .await
    .expect("create party-wide newsletter grant");
    assert!(!newsletter_created.replayed);
    let newsletter = decode_create_consent(&newsletter_created);
    assert_eq!(authorization_id(&newsletter), newsletter_id);
    assert_eq!(resource_version(&newsletter), 1);

    let replay = mutate(
        &mut grpc,
        &consent_create,
        newsletter_payload,
        TENANT_A,
        "consent-create-newsletter",
        true,
    )
    .await
    .expect("replay exact Consent create");
    assert!(replay.replayed);
    let after_newsletter = consent_evidence_counts(&admin, TENANT_A).await;
    assert_evidence_delta(after_newsletter, baseline, 1, 1);

    let conflicting_replay = mutate(
        &mut grpc,
        &consent_create,
        create_consent_payload(
            &consent_create,
            &unique_id("consent-newsletter-conflict"),
            &party_a,
            None,
            "marketing.newsletter.other",
            consents::CommunicationChannel::Email,
            consents::ConsentEffect::Grant,
            batch_effective,
            None,
        ),
        TENANT_A,
        "consent-create-newsletter",
        true,
    )
    .await
    .expect_err("conflicting Consent idempotency replay must fail");
    assert_eq!(conflicting_replay.code(), Code::Aborted);
    assert_eq!(
        consent_evidence_counts(&admin, TENANT_A).await,
        after_newsletter
    );

    let scoped_id = unique_id("consent-scoped-grant");
    let denied_id = unique_id("consent-explicit-deny");
    let expiring_id = unique_id("consent-expiring-grant");
    let renewal_old_id = unique_id("consent-renewal-old");
    for (authorization_id, contact_point_id, purpose, effect, expires_at, key) in [
        (
            scoped_id.as_str(),
            Some(email_a.as_str()),
            "marketing.scoped",
            consents::ConsentEffect::Grant,
            None,
            "consent-create-scoped",
        ),
        (
            denied_id.as_str(),
            None,
            "marketing.denied",
            consents::ConsentEffect::Deny,
            None,
            "consent-create-deny",
        ),
        (
            expiring_id.as_str(),
            None,
            "marketing.expiring",
            consents::ConsentEffect::Grant,
            Some(batch_effective + 700_000_000),
            "consent-create-expiring",
        ),
        (
            renewal_old_id.as_str(),
            None,
            "marketing.renewal",
            consents::ConsentEffect::Grant,
            None,
            "consent-create-renewal-old",
        ),
    ] {
        mutate(
            &mut grpc,
            &consent_create,
            create_consent_payload(
                &consent_create,
                authorization_id,
                &party_a,
                contact_point_id,
                purpose,
                consents::CommunicationChannel::Email,
                effect,
                batch_effective,
                expires_at,
            ),
            TENANT_A,
            key,
            true,
        )
        .await
        .expect("create deterministic Consent assertion batch");
    }
    let after_initial_batch = consent_evidence_counts(&admin, TENANT_A).await;
    assert_evidence_delta(after_initial_batch, baseline, 5, 5);

    let future_newsletter = authorize(
        &mut grpc,
        &communication_authorize,
        TENANT_A,
        &party_a,
        "marketing.newsletter",
        consents::CommunicationChannel::Email,
        Some(&email_a),
        true,
    )
    .await
    .expect("future grant evaluation");
    assert_denied(
        &future_newsletter,
        consents::CommunicationAuthorizationReason::NoApplicableGrant,
    );

    sleep_until_after(batch_effective).await;

    assert_allowed(
        &authorize(
            &mut grpc,
            &communication_authorize,
            TENANT_A,
            &party_a,
            "marketing.newsletter",
            consents::CommunicationChannel::Email,
            Some(&email_a),
            true,
        )
        .await
        .expect("party-wide current grant authorizes exact purpose/channel"),
        &newsletter_id,
    );
    assert_denied(
        &authorize(
            &mut grpc,
            &communication_authorize,
            TENANT_A,
            &party_a,
            "marketing.unrelated",
            consents::CommunicationChannel::Email,
            Some(&email_a),
            true,
        )
        .await
        .expect("unrelated purpose evaluation"),
        consents::CommunicationAuthorizationReason::NoApplicableGrant,
    );
    assert_denied(
        &authorize(
            &mut grpc,
            &communication_authorize,
            TENANT_A,
            &party_a,
            "marketing.newsletter",
            consents::CommunicationChannel::Sms,
            Some(&phone_a),
            true,
        )
        .await
        .expect("unrelated channel evaluation"),
        consents::CommunicationAuthorizationReason::NoApplicableGrant,
    );

    assert_allowed(
        &authorize(
            &mut grpc,
            &communication_authorize,
            TENANT_A,
            &party_a,
            "marketing.scoped",
            consents::CommunicationChannel::Email,
            Some(&email_a),
            true,
        )
        .await
        .expect("exact Contact Point scope evaluation"),
        &scoped_id,
    );
    for contact_point in [None, Some(email_peer.as_str())] {
        assert_denied(
            &authorize(
                &mut grpc,
                &communication_authorize,
                TENANT_A,
                &party_a,
                "marketing.scoped",
                consents::CommunicationChannel::Email,
                contact_point,
                true,
            )
            .await
            .expect("nonmatching Contact Point scope evaluation"),
            consents::CommunicationAuthorizationReason::NoApplicableGrant,
        );
    }

    assert_denied(
        &authorize(
            &mut grpc,
            &communication_authorize,
            TENANT_A,
            &party_a,
            "marketing.denied",
            consents::CommunicationChannel::Email,
            Some(&email_a),
            true,
        )
        .await
        .expect("active deny evaluation"),
        consents::CommunicationAuthorizationReason::ActiveDeny,
    );
    assert_allowed(
        &authorize(
            &mut grpc,
            &communication_authorize,
            TENANT_A,
            &party_a,
            "marketing.expiring",
            consents::CommunicationChannel::Email,
            None,
            true,
        )
        .await
        .expect("unexpired grant evaluation"),
        &expiring_id,
    );
    sleep_until_after(batch_effective + 700_000_000).await;
    assert_denied(
        &authorize(
            &mut grpc,
            &communication_authorize,
            TENANT_A,
            &party_a,
            "marketing.expiring",
            consents::CommunicationChannel::Email,
            None,
            true,
        )
        .await
        .expect("expired grant evaluation"),
        consents::CommunicationAuthorizationReason::NoApplicableGrant,
    );

    assert_allowed(
        &authorize(
            &mut grpc,
            &communication_authorize,
            TENANT_A,
            &party_a,
            "marketing.renewal",
            consents::CommunicationChannel::Email,
            None,
            true,
        )
        .await
        .expect("old renewal grant evaluation"),
        &renewal_old_id,
    );
    let withdrawn = mutate(
        &mut grpc,
        &consent_withdraw,
        withdraw_consent_payload(&consent_withdraw, &renewal_old_id, 1),
        TENANT_A,
        "consent-withdraw-renewal-old",
        true,
    )
    .await
    .expect("withdraw active Consent grant");
    let withdrawn_authorization = decode_withdraw_consent(&withdrawn);
    assert_eq!(resource_version(&withdrawn_authorization), 2);
    assert_eq!(
        withdrawn_authorization.status,
        consents::ConsentAuthorizationStatus::Withdrawn as i32
    );
    let after_withdrawal = consent_evidence_counts(&admin, TENANT_A).await;
    assert_evidence_delta(after_withdrawal, baseline, 5, 6);

    let withdrawal_replay = mutate(
        &mut grpc,
        &consent_withdraw,
        withdraw_consent_payload(&consent_withdraw, &renewal_old_id, 1),
        TENANT_A,
        "consent-withdraw-renewal-old",
        true,
    )
    .await
    .expect("replay exact Consent withdrawal");
    assert!(withdrawal_replay.replayed);
    assert_eq!(
        consent_evidence_counts(&admin, TENANT_A).await,
        after_withdrawal
    );
    assert_denied(
        &authorize(
            &mut grpc,
            &communication_authorize,
            TENANT_A,
            &party_a,
            "marketing.renewal",
            consents::CommunicationChannel::Email,
            None,
            true,
        )
        .await
        .expect("withdrawn grant evaluation"),
        consents::CommunicationAuthorizationReason::Withdrawn,
    );

    let repeated_withdrawal = mutate(
        &mut grpc,
        &consent_withdraw,
        withdraw_consent_payload(&consent_withdraw, &renewal_old_id, 2),
        TENANT_A,
        "consent-withdraw-renewal-old-again",
        true,
    )
    .await
    .expect_err("irreversible withdrawal must reject a second transition");
    assert!(matches!(
        repeated_withdrawal.code(),
        Code::InvalidArgument | Code::FailedPrecondition | Code::Aborted
    ));
    assert_eq!(
        consent_evidence_counts(&admin, TENANT_A).await,
        after_withdrawal
    );

    let later_effective = future_nanos(Duration::from_secs(2));
    let renewal_new_id = unique_id("consent-renewal-new");
    let denied_later_grant_id = unique_id("consent-denied-later-grant");
    for (authorization_id, purpose, key) in [
        (
            renewal_new_id.as_str(),
            "marketing.renewal",
            "consent-create-renewal-new",
        ),
        (
            denied_later_grant_id.as_str(),
            "marketing.denied",
            "consent-create-denied-later-grant",
        ),
    ] {
        mutate(
            &mut grpc,
            &consent_create,
            create_consent_payload(
                &consent_create,
                authorization_id,
                &party_a,
                None,
                purpose,
                consents::CommunicationChannel::Email,
                consents::ConsentEffect::Grant,
                later_effective,
                None,
            ),
            TENANT_A,
            key,
            true,
        )
        .await
        .expect("create later explicit grant with new evidence");
    }
    let after_later_grants = consent_evidence_counts(&admin, TENANT_A).await;
    assert_evidence_delta(after_later_grants, baseline, 7, 8);
    assert_denied(
        &authorize(
            &mut grpc,
            &communication_authorize,
            TENANT_A,
            &party_a,
            "marketing.renewal",
            consents::CommunicationChannel::Email,
            None,
            true,
        )
        .await
        .expect("future renewed grant must not authorize early"),
        consents::CommunicationAuthorizationReason::Withdrawn,
    );
    sleep_until_after(later_effective).await;
    assert_allowed(
        &authorize(
            &mut grpc,
            &communication_authorize,
            TENANT_A,
            &party_a,
            "marketing.renewal",
            consents::CommunicationChannel::Email,
            None,
            true,
        )
        .await
        .expect("later grant supersedes older withdrawal"),
        &renewal_new_id,
    );
    assert_allowed(
        &authorize(
            &mut grpc,
            &communication_authorize,
            TENANT_A,
            &party_a,
            "marketing.denied",
            consents::CommunicationChannel::Email,
            None,
            true,
        )
        .await
        .expect("later grant supersedes older deny"),
        &denied_later_grant_id,
    );

    let queried = query(
        &mut grpc,
        &consent_get,
        get_consent_payload(&consent_get, &newsletter_id),
        TENANT_A,
        true,
    )
    .await
    .expect("get authoritative Consent record");
    assert_eq!(authorization_id(&decode_get_consent(queried)), newsletter_id);

    let filtered = query(
        &mut grpc,
        &consent_list,
        list_consents_payload(
            &consent_list,
            10,
            "",
            Some(&party_a),
            None,
            Some("marketing.newsletter"),
            Some(consents::CommunicationChannel::Email),
            Some(consents::ConsentEffect::Grant),
            Some(consents::ConsentAuthorizationStatus::Active),
        ),
        TENANT_A,
        true,
    )
    .await
    .expect("list Consent records with typed filters");
    let filtered = decode_list_consents(filtered);
    assert_eq!(filtered.authorizations.len(), 1);
    assert_eq!(authorization_id(&filtered.authorizations[0]), newsletter_id);

    let first_page = query(
        &mut grpc,
        &consent_list,
        list_consents_payload(
            &consent_list,
            1,
            "",
            Some(&party_a),
            None,
            None,
            None,
            None,
            None,
        ),
        TENANT_A,
        true,
    )
    .await
    .expect("list first Consent page");
    let first_page = decode_list_consents(first_page);
    assert_eq!(first_page.authorizations.len(), 1);
    assert!(!first_page.next_cursor.is_empty());
    let tampered = query(
        &mut grpc,
        &consent_list,
        list_consents_payload(
            &consent_list,
            1,
            &format!("{}x", first_page.next_cursor),
            Some(&party_a),
            None,
            None,
            None,
            None,
            None,
        ),
        TENANT_A,
        true,
    )
    .await
    .expect_err("tampered signed Consent cursor must be rejected");
    assert_eq!(tampered.code(), Code::InvalidArgument);

    let unauthenticated_query = query(
        &mut grpc,
        &consent_get,
        get_consent_payload(&consent_get, &newsletter_id),
        TENANT_A,
        false,
    )
    .await
    .expect_err("unauthenticated Consent query must fail");
    assert_eq!(unauthenticated_query.code(), Code::Unauthenticated);
    let cross_tenant_get = query(
        &mut grpc,
        &consent_get,
        get_consent_payload(&consent_get, &newsletter_id),
        TENANT_B,
        true,
    )
    .await
    .expect_err("tenant B must not discover tenant A Consent record");
    assert_eq!(cross_tenant_get.code(), Code::NotFound);
    let cross_tenant_list = query(
        &mut grpc,
        &consent_list,
        list_consents_payload(&consent_list, 10, "", None, None, None, None, None, None),
        TENANT_B,
        true,
    )
    .await
    .expect("tenant B Consent list must not leak tenant A records");
    assert!(decode_list_consents(cross_tenant_list).authorizations.is_empty());

    assert_eq!(
        consent_evidence_counts(&admin, TENANT_A).await,
        after_later_grants
    );

    send_sigint(&child).await;
    let exit = timeout(Duration::from_secs(15), child.wait())
        .await
        .expect("crm-api must stop within graceful-shutdown budget")
        .expect("wait for Consent acceptance crm-api process");
    assert!(exit.success(), "crm-api exited unsuccessfully: {exit}");
}

async fn create_party(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: &CapabilityDefinition,
    tenant_id: &str,
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
        tenant_id,
        idempotency_key,
        true,
    )
    .await
    .expect("create Party prerequisite through production gateway");
}

#[allow(clippy::too_many_arguments)]
async fn create_contact_point(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: &CapabilityDefinition,
    tenant_id: &str,
    contact_point_id: &str,
    party_id: &str,
    kind: contact_points::ContactPointKind,
    value: &str,
    preferred: bool,
    idempotency_key: &str,
) {
    mutate(
        client,
        definition,
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
        ),
        tenant_id,
        idempotency_key,
        true,
    )
    .await
    .expect("create Contact Point prerequisite through production gateway");
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

#[allow(clippy::too_many_arguments)]
fn create_consent_payload(
    definition: &CapabilityDefinition,
    authorization_id: &str,
    party_id: &str,
    contact_point_id: Option<&str>,
    purpose: &str,
    channel: consents::CommunicationChannel,
    effect: consents::ConsentEffect,
    effective_from_unix_nanos: i64,
    expires_at_unix_nanos: Option<i64>,
) -> TypedPayload {
    payload(
        definition,
        consents::CreateConsentAuthorizationRequest {
            authorization_ref: Some(consents::ConsentAuthorizationRef {
                authorization_id: authorization_id.to_owned(),
            }),
            party_ref: Some(customer::PartyRef {
                party_id: party_id.to_owned(),
            }),
            contact_point_ref: contact_point_id.map(|contact_point_id| customer::ContactPointRef {
                contact_point_id: contact_point_id.to_owned(),
            }),
            purpose: purpose.to_owned(),
            channel: channel as i32,
            effect: effect as i32,
            legal_basis: "consent".to_owned(),
            jurisdiction: "eu-lt".to_owned(),
            source: "process.acceptance".to_owned(),
            evidence_ref: format!("evidence://consent/{authorization_id}"),
            effective_from: Some(core::UnixTime {
                unix_nanos: effective_from_unix_nanos,
            }),
            expires_at: expires_at_unix_nanos.map(|unix_nanos| core::UnixTime { unix_nanos }),
        },
    )
}

fn withdraw_consent_payload(
    definition: &CapabilityDefinition,
    authorization_id: &str,
    expected_version: i64,
) -> TypedPayload {
    payload(
        definition,
        consents::WithdrawConsentAuthorizationRequest {
            authorization_ref: Some(consents::ConsentAuthorizationRef {
                authorization_id: authorization_id.to_owned(),
            }),
            expected_version,
        },
    )
}

fn get_consent_payload(definition: &CapabilityDefinition, authorization_id: &str) -> TypedPayload {
    payload(
        definition,
        consents::GetConsentAuthorizationRequest {
            authorization_ref: Some(consents::ConsentAuthorizationRef {
                authorization_id: authorization_id.to_owned(),
            }),
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn list_consents_payload(
    definition: &CapabilityDefinition,
    page_size: i32,
    cursor: &str,
    party_id: Option<&str>,
    contact_point_id: Option<&str>,
    purpose: Option<&str>,
    channel: Option<consents::CommunicationChannel>,
    effect: Option<consents::ConsentEffect>,
    status: Option<consents::ConsentAuthorizationStatus>,
) -> TypedPayload {
    payload(
        definition,
        consents::ListConsentAuthorizationsRequest {
            party_ref: party_id.map(|party_id| customer::PartyRef {
                party_id: party_id.to_owned(),
            }),
            contact_point_ref: contact_point_id.map(|contact_point_id| customer::ContactPointRef {
                contact_point_id: contact_point_id.to_owned(),
            }),
            purpose: purpose.map(str::to_owned),
            channel: channel
                .map(|value| value as i32)
                .unwrap_or(consents::CommunicationChannel::Unspecified as i32),
            effect: effect
                .map(|value| value as i32)
                .unwrap_or(consents::ConsentEffect::Unspecified as i32),
            status: status
                .map(|value| value as i32)
                .unwrap_or(consents::ConsentAuthorizationStatus::Unspecified as i32),
            page_size,
            cursor: cursor.to_owned(),
        },
    )
}

async fn authorize(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: &CapabilityDefinition,
    tenant_id: &str,
    party_id: &str,
    purpose: &str,
    channel: consents::CommunicationChannel,
    contact_point_id: Option<&str>,
    authenticated: bool,
) -> Result<consents::CommunicationAuthorizationDecision, Status> {
    let response = query(
        client,
        definition,
        payload(
            definition,
            consents::AuthorizeCommunicationRequest {
                party_ref: Some(customer::PartyRef {
                    party_id: party_id.to_owned(),
                }),
                purpose: purpose.to_owned(),
                channel: channel as i32,
                contact_point_ref: contact_point_id.map(|contact_point_id| {
                    customer::ContactPointRef {
                        contact_point_id: contact_point_id.to_owned(),
                    }
                }),
            },
        ),
        tenant_id,
        authenticated,
    )
    .await?;
    Ok(consents::AuthorizeCommunicationResponse::decode(
        response
            .output
            .expect("communication authorization output")
            .payload
            .as_slice(),
    )
    .expect("decode communication authorization response")
    .decision
    .expect("communication authorization decision"))
}

fn decode_create_consent(
    response: &crm_application_runtime::gateway_v1::MutateResponse,
) -> consents::ConsentAuthorization {
    consents::CreateConsentAuthorizationResponse::decode(
        response
            .output
            .as_ref()
            .expect("Consent create output")
            .payload
            .as_slice(),
    )
    .expect("decode Consent create response")
    .authorization
    .expect("created Consent authorization")
}

fn decode_withdraw_consent(
    response: &crm_application_runtime::gateway_v1::MutateResponse,
) -> consents::ConsentAuthorization {
    consents::WithdrawConsentAuthorizationResponse::decode(
        response
            .output
            .as_ref()
            .expect("Consent withdrawal output")
            .payload
            .as_slice(),
    )
    .expect("decode Consent withdrawal response")
    .authorization
    .expect("withdrawn Consent authorization")
}

fn decode_get_consent(
    response: crm_application_runtime::gateway_v1::QueryResponse,
) -> consents::ConsentAuthorization {
    consents::GetConsentAuthorizationResponse::decode(
        response
            .output
            .expect("Consent get output")
            .payload
            .as_slice(),
    )
    .expect("decode Consent get response")
    .authorization
    .expect("queried Consent authorization")
}

fn decode_list_consents(
    response: crm_application_runtime::gateway_v1::QueryResponse,
) -> consents::ListConsentAuthorizationsResponse {
    consents::ListConsentAuthorizationsResponse::decode(
        response
            .output
            .expect("Consent list output")
            .payload
            .as_slice(),
    )
    .expect("decode Consent list response")
}

fn assert_allowed(decision: &consents::CommunicationAuthorizationDecision, expected_id: &str) {
    assert!(decision.allowed);
    assert_eq!(
        consents::CommunicationAuthorizationReason::try_from(decision.reason)
            .expect("known authorization reason"),
        consents::CommunicationAuthorizationReason::ActiveGrant
    );
    assert!(
        decision
            .determining_authorizations
            .iter()
            .any(|reference| reference.authorization_id == expected_id)
    );
}

fn assert_denied(
    decision: &consents::CommunicationAuthorizationDecision,
    expected_reason: consents::CommunicationAuthorizationReason,
) {
    assert!(!decision.allowed);
    assert_eq!(
        consents::CommunicationAuthorizationReason::try_from(decision.reason)
            .expect("known authorization reason"),
        expected_reason
    );
}

fn authorization_id(authorization: &consents::ConsentAuthorization) -> &str {
    authorization
        .authorization_ref
        .as_ref()
        .expect("Consent authorization reference")
        .authorization_id
        .as_str()
}

fn resource_version(authorization: &consents::ConsentAuthorization) -> i64 {
    authorization
        .resource_version
        .as_ref()
        .expect("Consent resource version")
        .version
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

async fn consent_evidence_counts(admin: &PgPool, tenant_id: &str) -> EvidenceCounts {
    let records = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.records WHERE tenant_id = $1 AND owner_module_id = 'crm.consents' AND record_type = 'consents.authorization' AND deleted_at IS NULL",
    )
    .bind(tenant_id)
    .fetch_one(admin)
    .await
    .expect("count Consent records");
    let events = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.outbox_events WHERE tenant_id = $1 AND event_type IN ('consents.authorization.created', 'consents.authorization.withdrawn')",
    )
    .bind(tenant_id)
    .fetch_one(admin)
    .await
    .expect("count Consent outbox events");
    let audits =
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM crm.audit_records WHERE tenant_id = $1")
            .bind(tenant_id)
            .fetch_one(admin)
            .await
            .expect("count Consent audit evidence");
    let idempotency = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.idempotency_records WHERE tenant_id = $1",
    )
    .bind(tenant_id)
    .fetch_one(admin)
    .await
    .expect("count Consent idempotency evidence");
    let transactions = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.business_transactions WHERE tenant_id = $1",
    )
    .bind(tenant_id)
    .fetch_one(admin)
    .await
    .expect("count Consent business transactions");
    EvidenceCounts {
        records,
        events,
        audits,
        idempotency,
        transactions,
    }
}

fn assert_evidence_delta(
    actual: EvidenceCounts,
    baseline: EvidenceCounts,
    created_records: i64,
    successful_mutations: i64,
) {
    assert_eq!(actual.records, baseline.records + created_records);
    assert_eq!(actual.events, baseline.events + successful_mutations);
    assert_eq!(actual.audits, baseline.audits + successful_mutations);
    assert_eq!(actual.idempotency, baseline.idempotency + successful_mutations);
    assert_eq!(actual.transactions, baseline.transactions + successful_mutations);
}

fn spawn_crm_api(database_url: &str, http_addr: &str, grpc_addr: &str) -> Child {
    Command::new(env!("CARGO_BIN_EXE_crm-api"))
        .env("CRM_DATABASE_URL", database_url)
        .env("CRM_HTTP_BIND", http_addr)
        .env("CRM_GRPC_BIND", grpc_addr)
        .env("CRM_API_BEARER_TOKEN", TOKEN)
        .env("CRM_API_ACTOR_ID", ACTOR)
        .env("CRM_API_TENANTS", format!("{TENANT_A},{TENANT_B}"))
        .env(
            "CRM_CURSOR_SIGNING_KEY",
            "consent-process-cursor-signing-key-0123456789abcdef",
        )
        .env(
            "CRM_APPROVAL_SIGNING_KEY",
            "consent-process-approval-signing-key-0123456789abcdef",
        )
        .env("CRM_BOOTSTRAP_ALLOW_PHASE6", "true")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn production crm-api process for Consent acceptance")
}

async fn wait_until_ready(client: &reqwest::Client, child: &mut Child, http_addr: &str) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child.try_wait().expect("poll crm-api process") {
            panic!("crm-api exited before Consent acceptance readiness: {status}");
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
            "Consent acceptance crm-api readiness timed out"
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
                    "Consent acceptance gRPC listener timed out: {error}"
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
        .expect("send SIGINT to Consent acceptance crm-api");
    assert!(status.success(), "kill -INT failed: {status}");
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral Consent acceptance port")
        .local_addr()
        .expect("read ephemeral Consent acceptance port")
        .port()
}

fn unique_id(prefix: &str) -> String {
    format!("{prefix}-{}-{}", std::process::id(), now_nanos())
}

fn now_nanos() -> i64 {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after Unix epoch")
            .as_nanos(),
    )
    .expect("current Unix nanos fit i64")
}

fn future_nanos(offset: Duration) -> i64 {
    now_nanos()
        + i64::try_from(offset.as_nanos()).expect("test time offset nanoseconds fit i64")
}

async fn sleep_until_after(target_unix_nanos: i64) {
    let remaining = target_unix_nanos.saturating_sub(now_nanos());
    if remaining > 0 {
        sleep(Duration::from_nanos(
            u64::try_from(remaining).expect("positive remaining nanoseconds fit u64"),
        ))
        .await;
    }
    sleep(Duration::from_millis(50)).await;
}
