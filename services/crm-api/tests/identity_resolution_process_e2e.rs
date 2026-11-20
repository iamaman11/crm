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
    core::v1 as core, customer::v1 as customer, identity_resolution::v1 as identity,
    parties::v1 as parties,
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
const TOKEN: &str = "identity-resolution-process-bearer-token-0123456789abcdef";

const PARTY_CREATE: &str = "parties.party.create";
const PARTY_UPDATE: &str = "parties.party.update";
const PARTY_GET: &str = "parties.party.get";
const CANDIDATE_REGISTER: &str = "identity_resolution.candidate.register";
const CANDIDATE_REFRESH: &str = "identity_resolution.candidate.evidence.refresh";
const CANDIDATE_DISMISS: &str = "identity_resolution.candidate.dismiss";
const CANDIDATE_CONFIRM: &str = "identity_resolution.candidate.confirm_duplicate";
const CANDIDATE_GET: &str = "identity_resolution.candidate.get";
const CANDIDATE_LIST: &str = "identity_resolution.candidate.list_by_party";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EvidenceCounts {
    records: i64,
    relationships: i64,
    events: i64,
    audits: i64,
    idempotency: i64,
    transactions: i64,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn crm_api_process_proves_governed_identity_resolution_without_party_merge() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping Identity Resolution process acceptance because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect Identity Resolution process evidence reader");
    for fixture in [
        include_str!("../../../database/tests/0005_party_adapter.sql"),
        include_str!("../../../database/tests/0010_identity_resolution_adapter.sql"),
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
    let party_update = mutation_definition(PARTY_UPDATE);
    let party_get = query_definition(PARTY_GET);
    let register = mutation_definition(CANDIDATE_REGISTER);
    let refresh = mutation_definition(CANDIDATE_REFRESH);
    let dismiss = mutation_definition(CANDIDATE_DISMISS);
    let confirm = mutation_definition(CANDIDATE_CONFIRM);
    let candidate_get = query_definition(CANDIDATE_GET);
    let candidate_list = query_definition(CANDIDATE_LIST);

    let party_a = unique_id("identity-party-a");
    let party_b = unique_id("identity-party-b");
    let party_c = unique_id("identity-party-c");
    let party_cross_tenant = unique_id("identity-party-cross-tenant");

    create_party(
        &mut grpc,
        &party_create,
        TENANT_A,
        &party_a,
        "Identity Subject A",
        "identity-create-party-a",
    )
    .await;
    create_party(
        &mut grpc,
        &party_create,
        TENANT_A,
        &party_b,
        "Identity Subject B",
        "identity-create-party-b",
    )
    .await;
    create_party(
        &mut grpc,
        &party_create,
        TENANT_A,
        &party_c,
        "Identity Subject C",
        "identity-create-party-c",
    )
    .await;
    create_party(
        &mut grpc,
        &party_create,
        TENANT_B,
        &party_cross_tenant,
        "Identity Cross Tenant Subject",
        "identity-create-party-cross-tenant",
    )
    .await;

    let baseline = evidence_counts(&admin, TENANT_A).await;
    let missing_party = unique_id("identity-party-missing");
    let rejected_missing = mutate(
        &mut grpc,
        &register,
        register_payload(
            &register,
            &party_a,
            1,
            &missing_party,
            1,
            8_000,
            "identity-missing-party",
        ),
        TENANT_A,
        "identity-register-missing-party",
        true,
    )
    .await
    .expect_err("missing Party reference must be rejected");
    assert_eq!(rejected_missing.code(), Code::InvalidArgument);

    let rejected_cross_tenant = mutate(
        &mut grpc,
        &register,
        register_payload(
            &register,
            &party_a,
            1,
            &party_cross_tenant,
            1,
            8_000,
            "identity-cross-tenant-party",
        ),
        TENANT_A,
        "identity-register-cross-tenant-party",
        true,
    )
    .await
    .expect_err("cross-tenant Party reference must be rejected without disclosure");
    assert_eq!(rejected_cross_tenant.code(), Code::InvalidArgument);
    assert_eq!(rejected_cross_tenant.message(), rejected_missing.message());

    let rejected_stale_version = mutate(
        &mut grpc,
        &register,
        register_payload(
            &register,
            &party_a,
            2,
            &party_b,
            1,
            8_000,
            "identity-stale-party-version",
        ),
        TENANT_A,
        "identity-register-stale-party-version",
        true,
    )
    .await
    .expect_err("candidate registration must reject stale or speculative Party versions");
    assert_eq!(rejected_stale_version.code(), Code::Aborted);
    assert_eq!(evidence_counts(&admin, TENANT_A).await, baseline);

    let generated_ab = now_nanos();
    let register_ab_payload = register_payload_at(
        &register,
        &party_b,
        1,
        &party_a,
        1,
        9_100,
        "identity-ab",
        generated_ab,
    );
    let created_ab = mutate(
        &mut grpc,
        &register,
        register_ab_payload.clone(),
        TENANT_A,
        "identity-register-ab",
        true,
    )
    .await
    .expect("register canonical A/B duplicate candidate");
    assert!(!created_ab.replayed);
    let candidate_ab = decode_register(&created_ab);
    assert_eq!(candidate_version(&candidate_ab), 1);
    assert_eq!(
        candidate_ab.status,
        identity::DuplicateCandidateCaseStatus::Open as i32
    );
    let case_ab = case_id(&candidate_ab).to_owned();
    let left_ab = candidate_ab
        .left_party_ref
        .as_ref()
        .expect("canonical left Party")
        .party_id
        .as_str();
    let right_ab = candidate_ab
        .right_party_ref
        .as_ref()
        .expect("canonical right Party")
        .party_id
        .as_str();
    assert!(left_ab < right_ab, "candidate Party pair must be canonical");
    assert_eq!(
        [left_ab, right_ab]
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>(),
        [party_a.as_str(), party_b.as_str()]
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
    );

    let after_ab = evidence_counts(&admin, TENANT_A).await;
    assert_evidence_delta(after_ab, baseline, 1, 2, 1, 1);
    assert_eq!(
        relationship_count_for_case(&admin, TENANT_A, &case_ab).await,
        2
    );

    let replay_ab = mutate(
        &mut grpc,
        &register,
        register_ab_payload.clone(),
        TENANT_A,
        "identity-register-ab",
        true,
    )
    .await
    .expect("replay exact candidate registration");
    assert!(replay_ab.replayed);
    assert_eq!(evidence_counts(&admin, TENANT_A).await, after_ab);
    assert_eq!(
        relationship_count_for_case(&admin, TENANT_A, &case_ab).await,
        2
    );

    let conflicting_replay = mutate(
        &mut grpc,
        &register,
        register_payload_at(
            &register,
            &party_a,
            1,
            &party_b,
            1,
            7_500,
            "identity-ab-conflicting-replay",
            generated_ab,
        ),
        TENANT_A,
        "identity-register-ab",
        true,
    )
    .await
    .expect_err("conflicting candidate idempotency replay must fail");
    assert_eq!(conflicting_replay.code(), Code::Aborted);
    assert_eq!(evidence_counts(&admin, TENANT_A).await, after_ab);

    let reverse_duplicate = mutate(
        &mut grpc,
        &register,
        register_payload(
            &register,
            &party_a,
            1,
            &party_b,
            1,
            9_100,
            "identity-ab-reverse-duplicate",
        ),
        TENANT_A,
        "identity-register-ab-reverse-duplicate",
        true,
    )
    .await
    .expect_err("same unordered Party pair must not create a second candidate case");
    assert!(matches!(
        reverse_duplicate.code(),
        Code::AlreadyExists | Code::Aborted | Code::FailedPrecondition
    ));
    assert_eq!(evidence_counts(&admin, TENANT_A).await, after_ab);

    let created_ac = mutate(
        &mut grpc,
        &register,
        register_payload(&register, &party_a, 1, &party_c, 1, 8_700, "identity-ac"),
        TENANT_A,
        "identity-register-ac",
        true,
    )
    .await
    .expect("register second candidate sharing Party A");
    let candidate_ac = decode_register(&created_ac);
    let case_ac = case_id(&candidate_ac).to_owned();
    assert_ne!(case_ab, case_ac);
    let after_ac = evidence_counts(&admin, TENANT_A).await;
    assert_evidence_delta(after_ac, baseline, 2, 4, 2, 2);
    assert_eq!(
        relationship_count_for_case(&admin, TENANT_A, &case_ac).await,
        2
    );

    let got_ab = query(
        &mut grpc,
        &candidate_get,
        get_candidate_payload(&candidate_get, &case_ab),
        TENANT_A,
        true,
    )
    .await
    .expect("get authoritative candidate case");
    assert_eq!(case_id(&decode_get(got_ab)), case_ab);

    let first_page = query(
        &mut grpc,
        &candidate_list,
        list_candidates_payload(&candidate_list, &party_a, None, 1, ""),
        TENANT_A,
        true,
    )
    .await
    .expect("list first Party A candidate page");
    let first_page = decode_list(first_page);
    assert_eq!(first_page.candidate_cases.len(), 1);
    assert!(!first_page.next_cursor.is_empty());

    let tampered_cursor = query(
        &mut grpc,
        &candidate_list,
        list_candidates_payload(
            &candidate_list,
            &party_a,
            None,
            1,
            &format!("{}x", first_page.next_cursor),
        ),
        TENANT_A,
        true,
    )
    .await
    .expect_err("tampered signed candidate cursor must be rejected");
    assert_eq!(tampered_cursor.code(), Code::InvalidArgument);

    let second_page = query(
        &mut grpc,
        &candidate_list,
        list_candidates_payload(&candidate_list, &party_a, None, 1, &first_page.next_cursor),
        TENANT_A,
        true,
    )
    .await
    .expect("list second Party A candidate page");
    let second_page = decode_list(second_page);
    assert_eq!(second_page.candidate_cases.len(), 1);
    assert_ne!(
        case_id(&first_page.candidate_cases[0]),
        case_id(&second_page.candidate_cases[0])
    );

    for party in [&party_b, &party_c] {
        let listed = query(
            &mut grpc,
            &candidate_list,
            list_candidates_payload(&candidate_list, party, None, 10, ""),
            TENANT_A,
            true,
        )
        .await
        .expect("list candidate from the other Party endpoint");
        assert_eq!(decode_list(listed).candidate_cases.len(), 1);
    }

    let unauthenticated_get = query(
        &mut grpc,
        &candidate_get,
        get_candidate_payload(&candidate_get, &case_ab),
        TENANT_A,
        false,
    )
    .await
    .expect_err("unauthenticated candidate get must fail");
    assert_eq!(unauthenticated_get.code(), Code::Unauthenticated);

    let cross_tenant_get = query(
        &mut grpc,
        &candidate_get,
        get_candidate_payload(&candidate_get, &case_ab),
        TENANT_B,
        true,
    )
    .await
    .expect_err("tenant B must not discover tenant A candidate by case id");
    assert_eq!(cross_tenant_get.code(), Code::NotFound);
    let cross_tenant_list = query(
        &mut grpc,
        &candidate_list,
        list_candidates_payload(&candidate_list, &party_a, None, 10, ""),
        TENANT_B,
        true,
    )
    .await
    .expect("cross-tenant candidate list must be empty without disclosure");
    assert!(decode_list(cross_tenant_list).candidate_cases.is_empty());

    let updated_a = mutate(
        &mut grpc,
        &party_update,
        payload(
            &party_update,
            parties::UpdatePartyRequest {
                party_ref: Some(customer::PartyRef {
                    party_id: party_a.clone(),
                }),
                expected_version: 1,
                display_name: "Identity Subject A Updated".to_owned(),
            },
        ),
        TENANT_A,
        "identity-update-party-a-v2",
        true,
    )
    .await
    .expect("advance Party A authoritative version");
    assert_eq!(updated_a.affected_resources[0].version, Some(2));

    let stale_confirm = mutate(
        &mut grpc,
        &confirm,
        decision_payload(&confirm, &case_ab, 1, "review.confirmed"),
        TENANT_A,
        "identity-confirm-ab-stale-evidence",
        true,
    )
    .await
    .expect_err("terminal decision must reject evidence stale against live Party versions");
    assert_eq!(stale_confirm.code(), Code::Aborted);
    assert_eq!(identity_record_version(&admin, TENANT_A, &case_ab).await, 1);

    let refreshed_ab = mutate(
        &mut grpc,
        &refresh,
        refresh_payload(
            &refresh,
            &case_ab,
            1,
            &party_a,
            2,
            &party_b,
            1,
            9_300,
            "identity-ab-refreshed",
        ),
        TENANT_A,
        "identity-refresh-ab-v2",
        true,
    )
    .await
    .expect("refresh candidate evidence to current Party versions");
    let refreshed_ab = decode_refresh(&refreshed_ab);
    assert_eq!(candidate_version(&refreshed_ab), 2);

    let no_advance_refresh = mutate(
        &mut grpc,
        &refresh,
        refresh_payload(
            &refresh,
            &case_ab,
            2,
            &party_a,
            2,
            &party_b,
            1,
            9_400,
            "identity-ab-no-version-advance",
        ),
        TENANT_A,
        "identity-refresh-ab-no-version-advance",
        true,
    )
    .await
    .expect_err("evidence refresh without any source-version advance must fail");
    assert!(matches!(
        no_advance_refresh.code(),
        Code::InvalidArgument | Code::FailedPrecondition | Code::Aborted
    ));
    assert_eq!(identity_record_version(&admin, TENANT_A, &case_ab).await, 2);

    let confirmed_ab = mutate(
        &mut grpc,
        &confirm,
        decision_payload(&confirm, &case_ab, 2, "review.confirmed"),
        TENANT_A,
        "identity-confirm-ab-v3",
        true,
    )
    .await
    .expect("confirm duplicate after fresh evidence review");
    let confirmed_ab = decode_confirm(&confirmed_ab);
    assert_eq!(candidate_version(&confirmed_ab), 3);
    assert_eq!(
        confirmed_ab.status,
        identity::DuplicateCandidateCaseStatus::ConfirmedDuplicate as i32
    );

    let repeated_terminal = mutate(
        &mut grpc,
        &dismiss,
        decision_payload(&dismiss, &case_ab, 3, "review.reconsidered"),
        TENANT_A,
        "identity-dismiss-confirmed-ab",
        true,
    )
    .await
    .expect_err("terminal confirmed-duplicate case cannot transition to dismissed");
    assert!(matches!(
        repeated_terminal.code(),
        Code::InvalidArgument | Code::FailedPrecondition | Code::Aborted
    ));

    let refreshed_ac = mutate(
        &mut grpc,
        &refresh,
        refresh_payload(
            &refresh,
            &case_ac,
            1,
            &party_a,
            2,
            &party_c,
            1,
            8_900,
            "identity-ac-refreshed",
        ),
        TENANT_A,
        "identity-refresh-ac-v2",
        true,
    )
    .await
    .expect("refresh second candidate before dismissal");
    assert_eq!(candidate_version(&decode_refresh(&refreshed_ac)), 2);
    let dismissed_ac = mutate(
        &mut grpc,
        &dismiss,
        decision_payload(&dismiss, &case_ac, 2, "review.not_duplicate"),
        TENANT_A,
        "identity-dismiss-ac-v3",
        true,
    )
    .await
    .expect("dismiss reviewed non-duplicate candidate");
    let dismissed_ac = decode_dismiss(&dismissed_ac);
    assert_eq!(candidate_version(&dismissed_ac), 3);
    assert_eq!(
        dismissed_ac.status,
        identity::DuplicateCandidateCaseStatus::Dismissed as i32
    );

    let confirmed_filter = query(
        &mut grpc,
        &candidate_list,
        list_candidates_payload(
            &candidate_list,
            &party_a,
            Some(identity::DuplicateCandidateCaseStatus::ConfirmedDuplicate),
            10,
            "",
        ),
        TENANT_A,
        true,
    )
    .await
    .expect("filter Party A candidates by confirmed status");
    let confirmed_filter = decode_list(confirmed_filter);
    assert_eq!(confirmed_filter.candidate_cases.len(), 1);
    assert_eq!(case_id(&confirmed_filter.candidate_cases[0]), case_ab);

    let party_a_after = query(
        &mut grpc,
        &party_get,
        party_get_payload(&party_get, &party_a),
        TENANT_A,
        true,
    )
    .await
    .expect("Party A remains queryable after duplicate confirmation");
    let party_b_after = query(
        &mut grpc,
        &party_get,
        party_get_payload(&party_get, &party_b),
        TENANT_A,
        true,
    )
    .await
    .expect("Party B remains queryable after duplicate confirmation");
    assert_eq!(party_version(&decode_party(party_a_after)), 2);
    assert_eq!(party_version(&decode_party(party_b_after)), 1);
    assert_eq!(party_record_count(&admin, TENANT_A).await, 3);
    assert_eq!(
        relationship_count_for_case(&admin, TENANT_A, &case_ab).await,
        2
    );
    assert_eq!(
        relationship_count_for_case(&admin, TENANT_A, &case_ac).await,
        2
    );

    let final_counts = evidence_counts(&admin, TENANT_A).await;
    assert_evidence_delta(final_counts, baseline, 2, 4, 6, 7);

    send_sigint(&child).await;
    let exit = timeout(Duration::from_secs(15), child.wait())
        .await
        .expect("crm-api must stop within graceful-shutdown budget")
        .expect("wait for Identity Resolution acceptance crm-api process");
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

fn register_payload(
    definition: &CapabilityDefinition,
    first_party: &str,
    first_version: i64,
    second_party: &str,
    second_version: i64,
    score_basis_points: u32,
    evidence_suffix: &str,
) -> TypedPayload {
    register_payload_at(
        definition,
        first_party,
        first_version,
        second_party,
        second_version,
        score_basis_points,
        evidence_suffix,
        now_nanos(),
    )
}

#[allow(clippy::too_many_arguments)]
fn register_payload_at(
    definition: &CapabilityDefinition,
    first_party: &str,
    first_version: i64,
    second_party: &str,
    second_version: i64,
    score_basis_points: u32,
    evidence_suffix: &str,
    generated_at_unix_nanos: i64,
) -> TypedPayload {
    payload(
        definition,
        identity::RegisterDuplicateCandidateRequest {
            evidence: Some(match_evidence(
                first_party,
                first_version,
                second_party,
                second_version,
                score_basis_points,
                evidence_suffix,
                generated_at_unix_nanos,
            )),
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn refresh_payload(
    definition: &CapabilityDefinition,
    case_id: &str,
    expected_version: i64,
    first_party: &str,
    first_version: i64,
    second_party: &str,
    second_version: i64,
    score_basis_points: u32,
    evidence_suffix: &str,
) -> TypedPayload {
    payload(
        definition,
        identity::RefreshDuplicateCandidateEvidenceRequest {
            case_ref: Some(identity::DuplicateCandidateCaseRef {
                case_id: case_id.to_owned(),
            }),
            expected_version,
            evidence: Some(match_evidence(
                first_party,
                first_version,
                second_party,
                second_version,
                score_basis_points,
                evidence_suffix,
                now_nanos(),
            )),
        },
    )
}

fn decision_payload(
    definition: &CapabilityDefinition,
    case_id: &str,
    expected_version: i64,
    reason: &str,
) -> TypedPayload {
    match definition.capability_id.as_str() {
        CANDIDATE_DISMISS => payload(
            definition,
            identity::DismissDuplicateCandidateRequest {
                case_ref: Some(identity::DuplicateCandidateCaseRef {
                    case_id: case_id.to_owned(),
                }),
                expected_version,
                reason: reason.to_owned(),
            },
        ),
        CANDIDATE_CONFIRM => payload(
            definition,
            identity::ConfirmDuplicateCandidateRequest {
                case_ref: Some(identity::DuplicateCandidateCaseRef {
                    case_id: case_id.to_owned(),
                }),
                expected_version,
                reason: reason.to_owned(),
            },
        ),
        capability => panic!("unsupported decision capability {capability}"),
    }
}

fn match_evidence(
    first_party: &str,
    first_version: i64,
    second_party: &str,
    second_version: i64,
    score_basis_points: u32,
    evidence_suffix: &str,
    generated_at_unix_nanos: i64,
) -> identity::MatchEvidenceSnapshot {
    identity::MatchEvidenceSnapshot {
        first_party_ref: Some(customer::PartyRef {
            party_id: first_party.to_owned(),
        }),
        first_party_version: first_version,
        second_party_ref: Some(customer::PartyRef {
            party_id: second_party.to_owned(),
        }),
        second_party_version: second_version,
        matcher_profile: "deterministic.v1".to_owned(),
        score_basis_points,
        signals: vec![identity::MatchSignal {
            kind: "exact.email".to_owned(),
            source: "process.acceptance".to_owned(),
            evidence_ref: format!("evidence://identity-resolution/{evidence_suffix}"),
            contribution_basis_points: i32::try_from(score_basis_points)
                .expect("score fits signal contribution"),
        }],
        generated_at: Some(core::UnixTime {
            unix_nanos: generated_at_unix_nanos,
        }),
    }
}

fn get_candidate_payload(definition: &CapabilityDefinition, case_id: &str) -> TypedPayload {
    payload(
        definition,
        identity::GetDuplicateCandidateCaseRequest {
            case_ref: Some(identity::DuplicateCandidateCaseRef {
                case_id: case_id.to_owned(),
            }),
        },
    )
}

fn list_candidates_payload(
    definition: &CapabilityDefinition,
    party_id: &str,
    status: Option<identity::DuplicateCandidateCaseStatus>,
    page_size: i32,
    cursor: &str,
) -> TypedPayload {
    payload(
        definition,
        identity::ListDuplicateCandidateCasesByPartyRequest {
            party_ref: Some(customer::PartyRef {
                party_id: party_id.to_owned(),
            }),
            status: status
                .map(|value| value as i32)
                .unwrap_or(identity::DuplicateCandidateCaseStatus::Unspecified as i32),
            page_size,
            cursor: cursor.to_owned(),
        },
    )
}

fn party_get_payload(definition: &CapabilityDefinition, party_id: &str) -> TypedPayload {
    payload(
        definition,
        parties::GetPartyRequest {
            party_ref: Some(customer::PartyRef {
                party_id: party_id.to_owned(),
            }),
        },
    )
}

fn decode_register(
    response: &crm_application_runtime::gateway_v1::MutateResponse,
) -> identity::DuplicateCandidateCase {
    identity::RegisterDuplicateCandidateResponse::decode(
        response
            .output
            .as_ref()
            .expect("candidate register output")
            .payload
            .as_slice(),
    )
    .expect("decode candidate register response")
    .candidate_case
    .expect("registered candidate case")
}

fn decode_refresh(
    response: &crm_application_runtime::gateway_v1::MutateResponse,
) -> identity::DuplicateCandidateCase {
    identity::RefreshDuplicateCandidateEvidenceResponse::decode(
        response
            .output
            .as_ref()
            .expect("candidate refresh output")
            .payload
            .as_slice(),
    )
    .expect("decode candidate refresh response")
    .candidate_case
    .expect("refreshed candidate case")
}

fn decode_confirm(
    response: &crm_application_runtime::gateway_v1::MutateResponse,
) -> identity::DuplicateCandidateCase {
    identity::ConfirmDuplicateCandidateResponse::decode(
        response
            .output
            .as_ref()
            .expect("candidate confirm output")
            .payload
            .as_slice(),
    )
    .expect("decode candidate confirm response")
    .candidate_case
    .expect("confirmed candidate case")
}

fn decode_dismiss(
    response: &crm_application_runtime::gateway_v1::MutateResponse,
) -> identity::DuplicateCandidateCase {
    identity::DismissDuplicateCandidateResponse::decode(
        response
            .output
            .as_ref()
            .expect("candidate dismiss output")
            .payload
            .as_slice(),
    )
    .expect("decode candidate dismiss response")
    .candidate_case
    .expect("dismissed candidate case")
}

fn decode_get(
    response: crm_application_runtime::gateway_v1::QueryResponse,
) -> identity::DuplicateCandidateCase {
    identity::GetDuplicateCandidateCaseResponse::decode(
        response
            .output
            .expect("candidate get output")
            .payload
            .as_slice(),
    )
    .expect("decode candidate get response")
    .candidate_case
    .expect("queried candidate case")
}

fn decode_list(
    response: crm_application_runtime::gateway_v1::QueryResponse,
) -> identity::ListDuplicateCandidateCasesByPartyResponse {
    identity::ListDuplicateCandidateCasesByPartyResponse::decode(
        response
            .output
            .expect("candidate list output")
            .payload
            .as_slice(),
    )
    .expect("decode candidate list response")
}

fn decode_party(response: crm_application_runtime::gateway_v1::QueryResponse) -> parties::Party {
    parties::GetPartyResponse::decode(
        response
            .output
            .expect("Party get output")
            .payload
            .as_slice(),
    )
    .expect("decode Party get response")
    .party
    .expect("queried Party")
}

fn case_id(candidate: &identity::DuplicateCandidateCase) -> &str {
    candidate
        .case_ref
        .as_ref()
        .expect("candidate case ref")
        .case_id
        .as_str()
}

fn candidate_version(candidate: &identity::DuplicateCandidateCase) -> i64 {
    candidate
        .resource_version
        .as_ref()
        .expect("candidate resource version")
        .version
}

fn party_version(party: &parties::Party) -> i64 {
    party
        .resource_version
        .as_ref()
        .expect("Party resource version")
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

async fn evidence_counts(admin: &PgPool, tenant_id: &str) -> EvidenceCounts {
    let records = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.records WHERE tenant_id = $1 AND owner_module_id = 'crm.identity-resolution' AND record_type = 'identity_resolution.candidate_case' AND deleted_at IS NULL",
    )
    .bind(tenant_id)
    .fetch_one(admin)
    .await
    .expect("count Identity Resolution records");
    let relationships = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.relationships WHERE tenant_id = $1 AND owner_module_id = 'crm.identity-resolution' AND relationship_type = 'identity_resolution.candidate.party'",
    )
    .bind(tenant_id)
    .fetch_one(admin)
    .await
    .expect("count Identity Resolution Party access relationships");
    let events = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.outbox_events WHERE tenant_id = $1 AND event_type IN ('identity_resolution.candidate.registered', 'identity_resolution.candidate.evidence_refreshed', 'identity_resolution.candidate.dismissed', 'identity_resolution.candidate.confirmed_duplicate')",
    )
    .bind(tenant_id)
    .fetch_one(admin)
    .await
    .expect("count Identity Resolution outbox events");
    let audits =
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM crm.audit_records WHERE tenant_id = $1")
            .bind(tenant_id)
            .fetch_one(admin)
            .await
            .expect("count audit evidence");
    let idempotency = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.idempotency_records WHERE tenant_id = $1",
    )
    .bind(tenant_id)
    .fetch_one(admin)
    .await
    .expect("count idempotency evidence");
    let transactions = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.business_transactions WHERE tenant_id = $1",
    )
    .bind(tenant_id)
    .fetch_one(admin)
    .await
    .expect("count business transactions");
    EvidenceCounts {
        records,
        relationships,
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
    created_relationships: i64,
    identity_successful_mutations: i64,
    tenant_successful_mutations: i64,
) {
    assert_eq!(actual.records, baseline.records + created_records);
    assert_eq!(
        actual.relationships,
        baseline.relationships + created_relationships
    );
    assert_eq!(
        actual.events,
        baseline.events + identity_successful_mutations
    );
    assert_eq!(actual.audits, baseline.audits + tenant_successful_mutations);
    assert_eq!(
        actual.idempotency,
        baseline.idempotency + tenant_successful_mutations
    );
    assert_eq!(
        actual.transactions,
        baseline.transactions + tenant_successful_mutations
    );
}

async fn relationship_count_for_case(admin: &PgPool, tenant_id: &str, case_id: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.relationships WHERE tenant_id = $1 AND owner_module_id = 'crm.identity-resolution' AND relationship_type = 'identity_resolution.candidate.party' AND target_record_type = 'identity_resolution.candidate_case' AND target_record_id = $2",
    )
    .bind(tenant_id)
    .bind(case_id)
    .fetch_one(admin)
    .await
    .expect("count Party-to-candidate relationships for case")
}

async fn identity_record_version(admin: &PgPool, tenant_id: &str, case_id: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT version FROM crm.records WHERE tenant_id = $1 AND owner_module_id = 'crm.identity-resolution' AND record_type = 'identity_resolution.candidate_case' AND record_id = $2 AND deleted_at IS NULL",
    )
    .bind(tenant_id)
    .bind(case_id)
    .fetch_one(admin)
    .await
    .expect("read candidate record version")
}

async fn party_record_count(admin: &PgPool, tenant_id: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.records WHERE tenant_id = $1 AND owner_module_id = 'crm.parties' AND record_type = 'parties.party' AND deleted_at IS NULL",
    )
    .bind(tenant_id)
    .fetch_one(admin)
    .await
    .expect("count Party records")
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
            "identity-resolution-cursor-signing-key-0123456789abcdef",
        )
        .env(
            "CRM_APPROVAL_SIGNING_KEY",
            "identity-resolution-approval-signing-key-0123456789abcdef",
        )
        .env("CRM_BOOTSTRAP_ALLOW_PHASE6", "true")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn production crm-api process for Identity Resolution acceptance")
}

async fn wait_until_ready(client: &reqwest::Client, child: &mut Child, http_addr: &str) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child.try_wait().expect("poll crm-api process") {
            panic!("crm-api exited before Identity Resolution acceptance readiness: {status}");
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
            "Identity Resolution acceptance crm-api readiness timed out"
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
                    "Identity Resolution acceptance gRPC listener timed out: {error}"
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
        .expect("send SIGINT to Identity Resolution acceptance crm-api");
    assert!(status.success(), "kill -INT failed: {status}");
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral Identity Resolution acceptance port")
        .local_addr()
        .expect("read ephemeral Identity Resolution acceptance port")
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
