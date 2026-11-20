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
use crm_core_data::PostgresDataStore;
use crm_module_sdk::{DataClass, PayloadEncoding, RetentionPolicyId, TenantId, TypedPayload};
use crm_party_relationships_projection::{
    HierarchyAdjacencyDocument, PARTY_RELATIONSHIP_HIERARCHY_PROJECTION_ID,
    PARTY_RELATIONSHIP_HIERARCHY_RESOURCE_TYPE, PartyRelationshipHierarchyProjectionWorker,
    traverse_projected_hierarchy,
};
use crm_proto_contracts::crm::{
    core::v1 as core, customer::v1 as customer, parties::v1 as parties,
    party_relationships::v1 as relationships,
};
use prost::Message;
use serde_json::Value;
use sqlx::{Executor, PgPool};
use std::collections::{BTreeMap, BTreeSet};
use std::net::TcpListener;
use std::process::Stdio;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::process::{Child, Command};
use tokio::time::{sleep, timeout};
use tonic::{Code, Request, Status};

const TENANT_A: &str = "tenant-a";
const TENANT_B: &str = "tenant-b";
const ACTOR: &str = "actor-a";
const TOKEN: &str = "party-relationship-process-bearer-token-0123456789abcdef0123456789abcdef";
const PARTY_CREATE: &str = "parties.party.create";
const RELATIONSHIP_CREATE: &str = "party-relationships.party-relationship.create";
const RELATIONSHIP_UPDATE: &str = "party-relationships.party-relationship.update";
const RELATIONSHIP_GET: &str = "party-relationships.party-relationship.get";
const RELATIONSHIP_LIST: &str = "party-relationships.party-relationship.list";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EvidenceCounts {
    records: i64,
    events: i64,
    audits: i64,
    idempotency: i64,
    transactions: i64,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn crm_api_process_proves_party_relationship_lifecycle_and_hierarchy_rebuild() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping Party Relationship process acceptance because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect Party Relationship process evidence reader");
    admin
        .execute(sqlx::raw_sql(include_str!(
            "../../../database/tests/0005_party_adapter.sql"
        )))
        .await
        .expect("publish Party module/capability registry fixture");
    admin
        .execute(sqlx::raw_sql(include_str!(
            "../../../database/tests/0008_party_relationship_adapter.sql"
        )))
        .await
        .expect("publish Party Relationship module/capability registry fixture");

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
    let employer_id = unique_id("party-z-employer");
    let employee_id = unique_id("party-a-employee");
    let household_peer_id = unique_id("party-m-household-peer");
    let tenant_b_party_id = unique_id("party-tenant-b");
    create_party(
        &mut grpc,
        &party_create,
        TENANT_A,
        &employer_id,
        parties::PartyKind::Organization,
        "Relationship Employer",
        "party-relationship-create-employer",
    )
    .await;
    create_party(
        &mut grpc,
        &party_create,
        TENANT_A,
        &employee_id,
        parties::PartyKind::Person,
        "Relationship Employee",
        "party-relationship-create-employee",
    )
    .await;
    create_party(
        &mut grpc,
        &party_create,
        TENANT_A,
        &household_peer_id,
        parties::PartyKind::Person,
        "Relationship Household Peer",
        "party-relationship-create-household-peer",
    )
    .await;
    create_party(
        &mut grpc,
        &party_create,
        TENANT_B,
        &tenant_b_party_id,
        parties::PartyKind::Person,
        "Tenant B Relationship Party",
        "party-relationship-create-tenant-b-party",
    )
    .await;

    let create = mutation_definition(RELATIONSHIP_CREATE);
    let update = mutation_definition(RELATIONSHIP_UPDATE);
    let get = query_definition(RELATIONSHIP_GET);
    let list = query_definition(RELATIONSHIP_LIST);
    let baseline = relationship_evidence_counts(&admin, TENANT_A).await;

    assert_rejected_reference_has_no_side_effects(
        &mut grpc,
        &create,
        &admin,
        baseline,
        &unique_id("party-missing"),
        &employee_id,
        "party-relationship-missing-party",
    )
    .await;
    let missing = mutate(
        &mut grpc,
        &create,
        create_relationship_payload(
            &create,
            &unique_id("relationship-missing-message"),
            &unique_id("party-missing-message"),
            &employee_id,
            employment_type(),
        ),
        TENANT_A,
        "party-relationship-missing-message",
        true,
    )
    .await
    .expect_err("missing Party endpoint must be rejected");
    let cross_tenant = mutate(
        &mut grpc,
        &create,
        create_relationship_payload(
            &create,
            &unique_id("relationship-cross-tenant"),
            &tenant_b_party_id,
            &employee_id,
            employment_type(),
        ),
        TENANT_A,
        "party-relationship-cross-tenant-party",
        true,
    )
    .await
    .expect_err("cross-tenant Party endpoint must be rejected");
    assert_eq!(missing.code(), Code::InvalidArgument);
    assert_eq!(cross_tenant.code(), Code::InvalidArgument);
    assert_eq!(cross_tenant.message(), missing.message());
    assert_eq!(
        relationship_evidence_counts(&admin, TENANT_A).await,
        baseline
    );

    let unauthenticated = mutate(
        &mut grpc,
        &create,
        create_relationship_payload(
            &create,
            &unique_id("relationship-unauthenticated"),
            &employer_id,
            &employee_id,
            employment_type(),
        ),
        TENANT_A,
        "party-relationship-unauthenticated",
        false,
    )
    .await
    .expect_err("unauthenticated Party Relationship mutation must fail");
    assert_eq!(unauthenticated.code(), Code::Unauthenticated);
    assert_eq!(
        relationship_evidence_counts(&admin, TENANT_A).await,
        baseline
    );

    let employment_id = unique_id("relationship-employment");
    let employment_payload = create_relationship_payload(
        &create,
        &employment_id,
        &employer_id,
        &employee_id,
        employment_type(),
    );
    let created = mutate(
        &mut grpc,
        &create,
        employment_payload.clone(),
        TENANT_A,
        "party-relationship-create-employment",
        true,
    )
    .await
    .expect("create directional Party Relationship");
    assert!(!created.replayed);
    let created_output = created.output.expect("Party Relationship create output");
    let employment = decode_create_relationship(&created_output.payload);
    assert_relationship(
        &employment,
        &employment_id,
        &employer_id,
        &employee_id,
        "employment",
        relationships::PartyRelationshipDirectionality::Directional,
        relationships::PartyRelationshipStatus::Active,
        1,
    );
    let after_employment = relationship_evidence_counts(&admin, TENANT_A).await;
    assert_evidence_delta(after_employment, baseline, 1, 1);

    let replay = mutate(
        &mut grpc,
        &create,
        employment_payload.clone(),
        TENANT_A,
        "party-relationship-create-employment",
        true,
    )
    .await
    .expect("replay Party Relationship create");
    assert!(replay.replayed);
    assert_eq!(
        replay
            .output
            .expect("Party Relationship replay output")
            .payload,
        created_output.payload
    );
    assert_eq!(
        relationship_evidence_counts(&admin, TENANT_A).await,
        after_employment
    );

    let idempotency_conflict = mutate(
        &mut grpc,
        &create,
        create_relationship_payload(
            &create,
            &unique_id("relationship-conflicting-replay"),
            &employer_id,
            &household_peer_id,
            employment_type(),
        ),
        TENANT_A,
        "party-relationship-create-employment",
        true,
    )
    .await
    .expect_err("conflicting Party Relationship replay must fail");
    assert_eq!(idempotency_conflict.code(), Code::Aborted);
    assert_eq!(
        relationship_evidence_counts(&admin, TENANT_A).await,
        after_employment
    );

    let same_id_duplicate = mutate(
        &mut grpc,
        &create,
        employment_payload,
        TENANT_A,
        "party-relationship-create-employment-new-key",
        true,
    )
    .await
    .expect_err("same Party Relationship id cannot be created twice");
    assert!(matches!(
        same_id_duplicate.code(),
        Code::AlreadyExists | Code::Aborted
    ));
    assert_eq!(
        relationship_evidence_counts(&admin, TENANT_A).await,
        after_employment
    );

    let queried = query(
        &mut grpc,
        &get,
        get_relationship_payload(&get, &employment_id),
        TENANT_A,
        true,
    )
    .await
    .expect("get Party Relationship");
    assert_eq!(decode_get_relationship(queried), employment);

    let household_id = unique_id("relationship-household");
    let household_created = mutate(
        &mut grpc,
        &create,
        create_relationship_payload(
            &create,
            &household_id,
            &employer_id,
            &household_peer_id,
            household_type(),
        ),
        TENANT_A,
        "party-relationship-create-household",
        true,
    )
    .await
    .expect("create reciprocal Party Relationship");
    let household = decode_create_relationship(
        &household_created
            .output
            .expect("reciprocal Party Relationship output")
            .payload,
    );
    let canonical_from = std::cmp::min(employer_id.as_str(), household_peer_id.as_str());
    let canonical_to = std::cmp::max(employer_id.as_str(), household_peer_id.as_str());
    assert_relationship(
        &household,
        &household_id,
        canonical_from,
        canonical_to,
        "household",
        relationships::PartyRelationshipDirectionality::Reciprocal,
        relationships::PartyRelationshipStatus::Active,
        1,
    );
    let after_household = relationship_evidence_counts(&admin, TENANT_A).await;
    assert_evidence_delta(after_household, after_employment, 1, 1);

    let filtered = query(
        &mut grpc,
        &list,
        list_relationships_payload(
            &list,
            10,
            "",
            Some(&household_peer_id),
            Some("household"),
            Some(relationships::PartyRelationshipDirectionality::Reciprocal),
            Some(relationships::PartyRelationshipStatus::Active),
        ),
        TENANT_A,
        true,
    )
    .await
    .expect("filter reciprocal household Party Relationships");
    let filtered = decode_list_relationships(filtered);
    assert_eq!(filtered.party_relationships.len(), 1);
    assert_eq!(
        relationship_id(&filtered.party_relationships[0]),
        household_id
    );

    let first_page = query(
        &mut grpc,
        &list,
        list_relationships_payload(&list, 1, "", None, None, None, None),
        TENANT_A,
        true,
    )
    .await
    .expect("list first Party Relationship page");
    let first_page = decode_list_relationships(first_page);
    assert_eq!(first_page.party_relationships.len(), 1);
    let page_token = first_page
        .page
        .as_ref()
        .expect("first Party Relationship page info")
        .next_page_token
        .clone();
    assert!(!page_token.is_empty());
    let tampered = query(
        &mut grpc,
        &list,
        list_relationships_payload(&list, 1, &format!("{page_token}x"), None, None, None, None),
        TENANT_A,
        true,
    )
    .await
    .expect_err("tampered Party Relationship cursor must fail");
    assert_eq!(tampered.code(), Code::InvalidArgument);
    let second_page = query(
        &mut grpc,
        &list,
        list_relationships_payload(&list, 1, &page_token, None, None, None, None),
        TENANT_A,
        true,
    )
    .await
    .expect("list second Party Relationship page");
    let second_page = decode_list_relationships(second_page);
    assert_eq!(second_page.party_relationships.len(), 1);
    assert_eq!(
        BTreeSet::from([
            relationship_id(&first_page.party_relationships[0]).to_owned(),
            relationship_id(&second_page.party_relationships[0]).to_owned(),
        ]),
        BTreeSet::from([employment_id.clone(), household_id.clone()])
    );

    let valid_until = now_unix_nanos();
    let update_payload = update_relationship_payload(
        &update,
        &employment_id,
        1,
        relationships::PartyRelationshipStatus::Inactive,
        Some(valid_until),
    );
    let updated = mutate(
        &mut grpc,
        &update,
        update_payload.clone(),
        TENANT_A,
        "party-relationship-update-employment-v1",
        true,
    )
    .await
    .expect("deactivate and bound directional Party Relationship");
    let updated_output = updated.output.expect("Party Relationship update output");
    let updated_relationship = decode_update_relationship(&updated_output.payload);
    assert_eq!(
        relationship_status(&updated_relationship),
        relationships::PartyRelationshipStatus::Inactive
    );
    assert_eq!(resource_version(&updated_relationship), 2);
    assert_eq!(
        updated_relationship
            .valid_until
            .as_ref()
            .expect("updated validity end")
            .unix_nanos,
        valid_until
    );
    let after_update = relationship_evidence_counts(&admin, TENANT_A).await;
    assert_evidence_delta(after_update, after_household, 0, 1);

    let update_replay = mutate(
        &mut grpc,
        &update,
        update_payload,
        TENANT_A,
        "party-relationship-update-employment-v1",
        true,
    )
    .await
    .expect("replay Party Relationship update");
    assert!(update_replay.replayed);
    assert_eq!(
        update_replay
            .output
            .expect("Party Relationship update replay output")
            .payload,
        updated_output.payload
    );
    assert_eq!(
        relationship_evidence_counts(&admin, TENANT_A).await,
        after_update
    );

    let stale = mutate(
        &mut grpc,
        &update,
        update_relationship_payload(
            &update,
            &employment_id,
            1,
            relationships::PartyRelationshipStatus::Active,
            None,
        ),
        TENANT_A,
        "party-relationship-stale-update",
        true,
    )
    .await
    .expect_err("stale Party Relationship version must fail");
    assert_eq!(stale.code(), Code::Aborted);
    let no_op = mutate(
        &mut grpc,
        &update,
        update_relationship_payload(
            &update,
            &employment_id,
            2,
            relationships::PartyRelationshipStatus::Inactive,
            Some(valid_until),
        ),
        TENANT_A,
        "party-relationship-no-op-update",
        true,
    )
    .await
    .expect_err("semantic no-op Party Relationship update must fail");
    assert_eq!(no_op.code(), Code::InvalidArgument);
    assert_eq!(
        relationship_evidence_counts(&admin, TENANT_A).await,
        after_update
    );

    assert_query_non_disclosure(&mut grpc, &get, &list, &employment_id).await;
    prove_hierarchy_rebuild(
        &admin,
        &database_url,
        &employment_id,
        &employee_id,
        &household,
        &household_id,
        valid_until,
    )
    .await;

    send_sigint(&child).await;
    let exit = timeout(Duration::from_secs(15), child.wait())
        .await
        .expect("crm-api must stop within graceful-shutdown budget")
        .expect("wait for Party Relationship acceptance crm-api process");
    assert!(exit.success(), "crm-api exited unsuccessfully: {exit}");
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
            "party-relationship-process-cursor-signing-key-0123456789abcdef",
        )
        .env(
            "CRM_APPROVAL_SIGNING_KEY",
            "party-relationship-process-approval-signing-key-0123456789abcdef",
        )
        .env("CRM_BOOTSTRAP_ALLOW_PHASE6", "true")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn production crm-api process for Party Relationship acceptance")
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
    mutate(
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
    .expect("create prerequisite Party");
}

async fn assert_rejected_reference_has_no_side_effects(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: &CapabilityDefinition,
    admin: &PgPool,
    baseline: EvidenceCounts,
    from_party_id: &str,
    to_party_id: &str,
    idempotency_key: &str,
) {
    let error = mutate(
        client,
        definition,
        create_relationship_payload(
            definition,
            &unique_id("relationship-rejected-reference"),
            from_party_id,
            to_party_id,
            employment_type(),
        ),
        TENANT_A,
        idempotency_key,
        true,
    )
    .await
    .expect_err("unavailable Party endpoint must fail");
    assert_eq!(error.code(), Code::InvalidArgument);
    assert_eq!(
        relationship_evidence_counts(admin, TENANT_A).await,
        baseline
    );
}

async fn assert_query_non_disclosure(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    get: &CapabilityDefinition,
    list: &CapabilityDefinition,
    relationship_id: &str,
) {
    let unauthenticated = query(
        client,
        get,
        get_relationship_payload(get, relationship_id),
        TENANT_A,
        false,
    )
    .await
    .expect_err("unauthenticated Party Relationship query must fail");
    assert_eq!(unauthenticated.code(), Code::Unauthenticated);
    let cross_tenant_get = query(
        client,
        get,
        get_relationship_payload(get, relationship_id),
        TENANT_B,
        true,
    )
    .await
    .expect_err("tenant B must not discover tenant A Party Relationship");
    assert_eq!(cross_tenant_get.code(), Code::NotFound);
    let cross_tenant_list = query(
        client,
        list,
        list_relationships_payload(list, 10, "", None, None, None, None),
        TENANT_B,
        true,
    )
    .await
    .expect("tenant B list must not leak tenant A Party Relationships");
    assert!(
        decode_list_relationships(cross_tenant_list)
            .party_relationships
            .is_empty()
    );
}

async fn prove_hierarchy_rebuild(
    admin: &PgPool,
    database_url: &str,
    employment_id: &str,
    employee_id: &str,
    household: &relationships::PartyRelationship,
    household_id: &str,
    valid_until: i64,
) {
    let store = PostgresDataStore::connect(database_url, 4)
        .await
        .expect("connect Party Relationship hierarchy projection store");
    let worker = PartyRelationshipHierarchyProjectionWorker::new(store)
        .expect("construct Party Relationship hierarchy projection worker");
    let tenant = TenantId::try_new(TENANT_A).expect("valid Party Relationship tenant");
    drain_hierarchy_projection(&worker, tenant.clone()).await;
    let before = hierarchy_documents(admin, TENANT_A).await;
    assert_eq!(before.len(), 4);
    let employment_edges = before
        .iter()
        .filter(|document| document.relationship_id == employment_id)
        .collect::<Vec<_>>();
    assert_eq!(employment_edges.len(), 2);
    assert!(employment_edges.iter().all(|document| {
        document.status == "inactive"
            && document.version == 2
            && document.valid_until_unix_nanos == Some(valid_until)
    }));
    let household_edges = before
        .iter()
        .filter(|document| document.relationship_id == household_id)
        .collect::<Vec<_>>();
    assert_eq!(household_edges.len(), 2);
    assert!(
        household_edges
            .iter()
            .all(|document| document.status == "active" && document.version == 1)
    );
    assert_eq!(
        traverse_projected_hierarchy(&before, from_party_id(household), 1, now_unix_nanos()),
        BTreeMap::from([
            (from_party_id(household).to_owned(), 0),
            (to_party_id(household).to_owned(), 1),
        ])
    );
    assert_eq!(
        traverse_projected_hierarchy(&before, employee_id, 1, now_unix_nanos()),
        BTreeMap::from([(employee_id.to_owned(), 0)])
    );
    let rebuilt_events = worker
        .rebuild(tenant, 100)
        .await
        .expect("rebuild Party Relationship hierarchy projection");
    assert_eq!(rebuilt_events, 3);
    assert_eq!(hierarchy_documents(admin, TENANT_A).await, before);
    let checkpoint_count: i64 = sqlx::query_scalar(
        "SELECT applied_event_count FROM crm.projection_checkpoints WHERE tenant_id = $1 AND projection_id = $2",
    )
    .bind(TENANT_A)
    .bind(PARTY_RELATIONSHIP_HIERARCHY_PROJECTION_ID)
    .fetch_one(admin)
    .await
    .expect("read Party Relationship hierarchy projection checkpoint");
    assert_eq!(checkpoint_count, 3);
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

fn employment_type() -> relationships::PartyRelationshipType {
    relationships::PartyRelationshipType {
        code: "employment".to_owned(),
        directionality: relationships::PartyRelationshipDirectionality::Directional as i32,
        from_role: "employer".to_owned(),
        to_role: "employee".to_owned(),
    }
}

fn household_type() -> relationships::PartyRelationshipType {
    relationships::PartyRelationshipType {
        code: "household".to_owned(),
        directionality: relationships::PartyRelationshipDirectionality::Reciprocal as i32,
        from_role: "household_member".to_owned(),
        to_role: "household_member".to_owned(),
    }
}

fn create_relationship_payload(
    definition: &CapabilityDefinition,
    relationship_id: &str,
    from_party_id: &str,
    to_party_id: &str,
    relationship_type: relationships::PartyRelationshipType,
) -> TypedPayload {
    payload(
        definition,
        relationships::CreatePartyRelationshipRequest {
            party_relationship_ref: Some(customer::PartyRelationshipRef {
                party_relationship_id: relationship_id.to_owned(),
            }),
            from_party_ref: Some(customer::PartyRef {
                party_id: from_party_id.to_owned(),
            }),
            to_party_ref: Some(customer::PartyRef {
                party_id: to_party_id.to_owned(),
            }),
            relationship_type: Some(relationship_type),
            valid_from: None,
            valid_until: None,
        },
    )
}

fn update_relationship_payload(
    definition: &CapabilityDefinition,
    relationship_id: &str,
    expected_version: i64,
    status: relationships::PartyRelationshipStatus,
    valid_until: Option<i64>,
) -> TypedPayload {
    payload(
        definition,
        relationships::UpdatePartyRelationshipRequest {
            party_relationship_ref: Some(customer::PartyRelationshipRef {
                party_relationship_id: relationship_id.to_owned(),
            }),
            expected_version,
            status: status as i32,
            valid_from: None,
            valid_until: valid_until.map(|unix_nanos| core::UnixTime { unix_nanos }),
        },
    )
}

fn get_relationship_payload(
    definition: &CapabilityDefinition,
    relationship_id: &str,
) -> TypedPayload {
    payload(
        definition,
        relationships::GetPartyRelationshipRequest {
            party_relationship_ref: Some(customer::PartyRelationshipRef {
                party_relationship_id: relationship_id.to_owned(),
            }),
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn list_relationships_payload(
    definition: &CapabilityDefinition,
    page_size: i32,
    page_token: &str,
    party_id: Option<&str>,
    type_code: Option<&str>,
    directionality: Option<relationships::PartyRelationshipDirectionality>,
    status: Option<relationships::PartyRelationshipStatus>,
) -> TypedPayload {
    payload(
        definition,
        relationships::ListPartyRelationshipsRequest {
            page: Some(core::PageRequest {
                page_size,
                page_token: page_token.to_owned(),
            }),
            party_ref: party_id.map(|party_id| customer::PartyRef {
                party_id: party_id.to_owned(),
            }),
            relationship_type_code: type_code.map(str::to_owned),
            directionality: directionality.map(|value| value as i32),
            status: status.map(|value| value as i32),
            sort: relationships::PartyRelationshipSort::UpdatedAtDescending as i32,
        },
    )
}

fn decode_create_relationship(bytes: &[u8]) -> relationships::PartyRelationship {
    relationships::CreatePartyRelationshipResponse::decode(bytes)
        .expect("decode Party Relationship create response")
        .party_relationship
        .expect("created Party Relationship exists")
}

fn decode_update_relationship(bytes: &[u8]) -> relationships::PartyRelationship {
    relationships::UpdatePartyRelationshipResponse::decode(bytes)
        .expect("decode Party Relationship update response")
        .party_relationship
        .expect("updated Party Relationship exists")
}

fn decode_get_relationship(
    response: crm_application_runtime::gateway_v1::QueryResponse,
) -> relationships::PartyRelationship {
    relationships::GetPartyRelationshipResponse::decode(
        response
            .output
            .expect("Party Relationship query output")
            .payload
            .as_slice(),
    )
    .expect("decode Party Relationship get response")
    .party_relationship
    .expect("queried Party Relationship exists")
}

fn decode_list_relationships(
    response: crm_application_runtime::gateway_v1::QueryResponse,
) -> relationships::ListPartyRelationshipsResponse {
    relationships::ListPartyRelationshipsResponse::decode(
        response
            .output
            .expect("Party Relationship list output")
            .payload
            .as_slice(),
    )
    .expect("decode Party Relationship list response")
}

#[allow(clippy::too_many_arguments)]
fn assert_relationship(
    relationship: &relationships::PartyRelationship,
    expected_id: &str,
    expected_from: &str,
    expected_to: &str,
    expected_type: &str,
    expected_directionality: relationships::PartyRelationshipDirectionality,
    expected_status: relationships::PartyRelationshipStatus,
    expected_version: i64,
) {
    assert_eq!(relationship_id(relationship), expected_id);
    assert_eq!(from_party_id(relationship), expected_from);
    assert_eq!(to_party_id(relationship), expected_to);
    assert_eq!(relationship_type_code(relationship), expected_type);
    assert_eq!(
        relationship_directionality(relationship),
        expected_directionality
    );
    assert_eq!(relationship_status(relationship), expected_status);
    assert_eq!(resource_version(relationship), expected_version);
}

fn relationship_id(relationship: &relationships::PartyRelationship) -> &str {
    relationship
        .party_relationship_ref
        .as_ref()
        .expect("Party Relationship reference")
        .party_relationship_id
        .as_str()
}

fn from_party_id(relationship: &relationships::PartyRelationship) -> &str {
    relationship
        .from_party_ref
        .as_ref()
        .expect("from Party reference")
        .party_id
        .as_str()
}

fn to_party_id(relationship: &relationships::PartyRelationship) -> &str {
    relationship
        .to_party_ref
        .as_ref()
        .expect("to Party reference")
        .party_id
        .as_str()
}

fn relationship_type_code(relationship: &relationships::PartyRelationship) -> &str {
    relationship
        .relationship_type
        .as_ref()
        .expect("Party Relationship type")
        .code
        .as_str()
}

fn relationship_directionality(
    relationship: &relationships::PartyRelationship,
) -> relationships::PartyRelationshipDirectionality {
    relationships::PartyRelationshipDirectionality::try_from(
        relationship
            .relationship_type
            .as_ref()
            .expect("Party Relationship type")
            .directionality,
    )
    .expect("known Party Relationship directionality")
}

fn relationship_status(
    relationship: &relationships::PartyRelationship,
) -> relationships::PartyRelationshipStatus {
    relationships::PartyRelationshipStatus::try_from(relationship.status)
        .expect("known Party Relationship status")
}

fn resource_version(relationship: &relationships::PartyRelationship) -> i64 {
    relationship
        .resource_version
        .as_ref()
        .expect("Party Relationship resource version")
        .version
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

fn assert_evidence_delta(
    current: EvidenceCounts,
    previous: EvidenceCounts,
    record_delta: i64,
    mutation_delta: i64,
) {
    assert_eq!(current.records, previous.records + record_delta);
    assert_eq!(current.events, previous.events + mutation_delta);
    assert_eq!(current.audits, previous.audits + mutation_delta);
    assert_eq!(current.idempotency, previous.idempotency + mutation_delta);
    assert_eq!(current.transactions, previous.transactions + mutation_delta);
}

async fn relationship_evidence_counts(admin: &PgPool, tenant_id: &str) -> EvidenceCounts {
    let records = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.records WHERE tenant_id = $1 AND owner_module_id = 'crm.party-relationships' AND record_type = 'party-relationships.party_relationship' AND deleted_at IS NULL",
    )
    .bind(tenant_id)
    .fetch_one(admin)
    .await
    .expect("count Party Relationship records");
    let events = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.outbox_events WHERE tenant_id = $1 AND event_type IN ('party-relationships.party-relationship.created', 'party-relationships.party-relationship.updated')",
    )
    .bind(tenant_id)
    .fetch_one(admin)
    .await
    .expect("count Party Relationship outbox events");
    let audits =
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM crm.audit_records WHERE tenant_id = $1")
            .bind(tenant_id)
            .fetch_one(admin)
            .await
            .expect("count Party Relationship audit evidence");
    let idempotency = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.idempotency_records WHERE tenant_id = $1",
    )
    .bind(tenant_id)
    .fetch_one(admin)
    .await
    .expect("count Party Relationship idempotency evidence");
    let transactions = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.business_transactions WHERE tenant_id = $1",
    )
    .bind(tenant_id)
    .fetch_one(admin)
    .await
    .expect("count Party Relationship business transactions");
    EvidenceCounts {
        records,
        events,
        audits,
        idempotency,
        transactions,
    }
}

async fn drain_hierarchy_projection(
    worker: &PartyRelationshipHierarchyProjectionWorker,
    tenant_id: TenantId,
) {
    loop {
        let result = worker
            .run_batch(tenant_id.clone(), 100)
            .await
            .expect("run Party Relationship hierarchy projection batch");
        if !result.has_more {
            return;
        }
    }
}

async fn hierarchy_documents(admin: &PgPool, tenant_id: &str) -> Vec<HierarchyAdjacencyDocument> {
    let values = sqlx::query_scalar::<_, Value>(
        "SELECT document FROM crm.projection_documents WHERE tenant_id = $1 AND projection_id = $2 AND resource_type = $3 ORDER BY resource_id",
    )
    .bind(tenant_id)
    .bind(PARTY_RELATIONSHIP_HIERARCHY_PROJECTION_ID)
    .bind(PARTY_RELATIONSHIP_HIERARCHY_RESOURCE_TYPE)
    .fetch_all(admin)
    .await
    .expect("read Party Relationship hierarchy projection documents");
    values
        .into_iter()
        .map(|value| {
            HierarchyAdjacencyDocument::from_json(&value)
                .expect("decode Party Relationship hierarchy projection document")
        })
        .collect()
}

async fn wait_until_ready(client: &reqwest::Client, child: &mut Child, http_addr: &str) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child.try_wait().expect("poll crm-api process") {
            panic!("crm-api exited before Party Relationship acceptance readiness: {status}");
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
            "Party Relationship acceptance crm-api readiness timed out"
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
                    "Party Relationship acceptance gRPC listener timed out: {error}"
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
        .expect("send SIGINT to Party Relationship acceptance crm-api");
    assert!(status.success(), "kill -INT failed: {status}");
}

fn unique_id(prefix: &str) -> String {
    format!("{prefix}-{}", now_unix_nanos())
}

fn now_unix_nanos() -> i64 {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos(),
    )
    .expect("current Unix nanos fit i64")
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral Party Relationship acceptance port")
        .local_addr()
        .expect("read ephemeral Party Relationship acceptance port")
        .port()
}
