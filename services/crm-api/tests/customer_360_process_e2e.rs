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
use crm_customer_360_composition::Customer360ProjectionWorker;
use crm_module_sdk::{DataClass, PayloadEncoding, RetentionPolicyId, TenantId, TypedPayload};
use crm_proto_contracts::crm::{
    accounts::v1 as accounts, contact_points::v1 as contact_points, core::v1 as core,
    customer::v1 as customer, customer_360::v1 as customer_360, parties::v1 as parties,
    party_relationships::v1 as relationships,
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
const TOKEN: &str = "customer-360-process-bearer-token-0123456789abcdef0123456789abcdef";
const PARTY_CREATE: &str = "parties.party.create";
const ACCOUNT_CREATE: &str = "accounts.account.create";
const ACCOUNT_UPDATE: &str = "accounts.account.update";
const CONTACT_POINT_CREATE: &str = "contact-points.contact-point.create";
const CONTACT_POINT_UPDATE: &str = "contact-points.contact-point.update";
const CONTACT_POINT_VERIFY: &str = "contact-points.contact-point.verify";
const RELATIONSHIP_CREATE: &str = "party-relationships.party-relationship.create";
const RELATIONSHIP_UPDATE: &str = "party-relationships.party-relationship.update";
const CUSTOMER_360_GET: &str = "customer-360.customer.get";

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn crm_api_process_composes_customer_360_converges_owner_updates_and_rebuilds_identically() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping Customer 360 process acceptance because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect Customer 360 process fixture publisher");
    for fixture in [
        include_str!("../../../database/tests/0005_party_adapter.sql"),
        include_str!("../../../database/tests/0006_account_adapter.sql"),
        include_str!("../../../database/tests/0007_contact_point_adapter.sql"),
        include_str!("../../../database/tests/0008_party_relationship_adapter.sql"),
    ] {
        admin
            .execute(sqlx::raw_sql(fixture))
            .await
            .expect("publish customer-master owner adapter fixture");
    }

    let run_id = unique_id("customer-360");
    let root_party_id = format!("party-root-{run_id}");
    let peer_party_id = format!("party-peer-{run_id}");
    let account_id = format!("account-{run_id}");
    let contact_point_id = format!("contact-point-{run_id}");
    let relationship_id = format!("relationship-{run_id}");

    let (mut child, http_addr, grpc_addr) = spawn_crm_api(&database_url);
    let http = reqwest::Client::new();
    wait_until_ready(&http, &mut child, &http_addr).await;
    let mut grpc = connect_grpc(&grpc_addr).await;

    let party_create = mutation_definition(PARTY_CREATE);
    let account_create = mutation_definition(ACCOUNT_CREATE);
    let account_update = mutation_definition(ACCOUNT_UPDATE);
    let contact_point_create = mutation_definition(CONTACT_POINT_CREATE);
    let contact_point_update = mutation_definition(CONTACT_POINT_UPDATE);
    let contact_point_verify = mutation_definition(CONTACT_POINT_VERIFY);
    let relationship_create = mutation_definition(RELATIONSHIP_CREATE);
    let relationship_update = mutation_definition(RELATIONSHIP_UPDATE);
    let customer_360_get = query_definition(CUSTOMER_360_GET);

    create_party(
        &mut grpc,
        &party_create,
        &root_party_id,
        parties::PartyKind::Person,
        "Ada Customer",
        &format!("c360-root-{run_id}"),
    )
    .await;
    create_party(
        &mut grpc,
        &party_create,
        &peer_party_id,
        parties::PartyKind::Person,
        "Grace Peer",
        &format!("c360-peer-{run_id}"),
    )
    .await;

    mutate(
        &mut grpc,
        &account_create,
        payload(
            &account_create,
            accounts::CreateAccountRequest {
                account_ref: Some(customer::AccountRef {
                    account_id: account_id.clone(),
                }),
                name: "Shared Customer Account".to_owned(),
                party_associations: vec![
                    association(&root_party_id, accounts::AccountPartyRole::Primary),
                    association(&peer_party_id, accounts::AccountPartyRole::Member),
                ],
            },
        ),
        TENANT_A,
        &format!("c360-account-create-{run_id}"),
        true,
    )
    .await
    .expect("create Account through real crm-api");

    mutate(
        &mut grpc,
        &contact_point_create,
        payload(
            &contact_point_create,
            contact_points::CreateContactPointRequest {
                contact_point_ref: Some(customer::ContactPointRef {
                    contact_point_id: contact_point_id.clone(),
                }),
                party_ref: Some(customer::PartyRef {
                    party_id: root_party_id.clone(),
                }),
                kind: contact_points::ContactPointKind::Email as i32,
                value: "Ada.Customer@Example.com".to_owned(),
                preferred: true,
                valid_from: None,
                valid_until: None,
            },
        ),
        TENANT_A,
        &format!("c360-contact-create-{run_id}"),
        true,
    )
    .await
    .expect("create Contact Point through real crm-api");

    mutate(
        &mut grpc,
        &contact_point_verify,
        payload(
            &contact_point_verify,
            contact_points::VerifyContactPointRequest {
                contact_point_ref: Some(customer::ContactPointRef {
                    contact_point_id: contact_point_id.clone(),
                }),
                expected_version: 1,
                evidence_ref: format!("c360-evidence-{run_id}"),
            },
        ),
        TENANT_A,
        &format!("c360-contact-verify-{run_id}"),
        true,
    )
    .await
    .expect("verify Contact Point through real crm-api");

    mutate(
        &mut grpc,
        &relationship_create,
        payload(
            &relationship_create,
            relationships::CreatePartyRelationshipRequest {
                party_relationship_ref: Some(customer::PartyRelationshipRef {
                    party_relationship_id: relationship_id.clone(),
                }),
                from_party_ref: Some(customer::PartyRef {
                    party_id: root_party_id.clone(),
                }),
                to_party_ref: Some(customer::PartyRef {
                    party_id: peer_party_id.clone(),
                }),
                relationship_type: Some(relationships::PartyRelationshipType {
                    code: "household".to_owned(),
                    directionality: relationships::PartyRelationshipDirectionality::Reciprocal
                        as i32,
                    from_role: "household_member".to_owned(),
                    to_role: "household_member".to_owned(),
                }),
                valid_from: None,
                valid_until: None,
            },
        ),
        TENANT_A,
        &format!("c360-relationship-create-{run_id}"),
        true,
    )
    .await
    .expect("create Party Relationship through real crm-api");

    let initial = wait_for_customer_360(
        &mut grpc,
        &customer_360_get,
        TENANT_A,
        &root_party_id,
        |view| {
            view.accounts.len() == 1
                && view.contact_points.len() == 1
                && contact_point_verification_status(&view.contact_points[0])
                    == contact_points::ContactPointVerificationStatus::Verified as i32
                && view.party_relationships.len() == 1
        },
    )
    .await;
    assert_eq!(party_display_name(&initial), "Ada Customer");
    assert_eq!(account_id_of(&initial.accounts[0]), account_id);
    assert_eq!(
        contact_point_id_of(&initial.contact_points[0]),
        contact_point_id
    );
    assert_eq!(
        relationship_id_of(&initial.party_relationships[0]),
        relationship_id
    );

    assert_eq!(
        initial
            .party
            .as_ref()
            .and_then(|section| section.party.as_ref())
            .expect("Customer 360 Party section")
            .kind,
        parties::PartyKind::Unspecified as i32,
        "Customer 360 Party kind must be field-redacted by the bootstrap policy"
    );
    assert!(
        initial.accounts[0]
            .account
            .as_ref()
            .expect("Customer 360 Account")
            .party_associations
            .is_empty(),
        "Customer 360 Account associations must be field-redacted after root selection"
    );
    assert!(
        initial.contact_points[0]
            .contact_point
            .as_ref()
            .expect("Customer 360 Contact Point")
            .display_value
            .is_empty(),
        "Customer 360 Contact Point display value must be field-redacted"
    );
    assert!(
        initial.party_relationships[0]
            .party_relationship
            .as_ref()
            .expect("Customer 360 Party Relationship")
            .relationship_type
            .is_none(),
        "Customer 360 Party Relationship type must be field-redacted"
    );

    assert!(
        initial
            .freshness
            .as_ref()
            .expect("Customer 360 freshness")
            .applied_event_count
            >= 6
    );

    let unauthenticated = query(
        &mut grpc,
        &customer_360_get,
        customer_360_payload(&customer_360_get, &root_party_id),
        TENANT_A,
        false,
    )
    .await
    .expect_err("unauthenticated Customer 360 query must fail");
    assert_eq!(unauthenticated.code(), Code::Unauthenticated);

    let cross_tenant = query(
        &mut grpc,
        &customer_360_get,
        customer_360_payload(&customer_360_get, &root_party_id),
        TENANT_B,
        true,
    )
    .await
    .expect_err("tenant B must not discover tenant A Customer 360");
    assert_eq!(cross_tenant.code(), Code::NotFound);

    mutate(
        &mut grpc,
        &account_update,
        payload(
            &account_update,
            accounts::UpdateAccountRequest {
                account_ref: Some(customer::AccountRef {
                    account_id: account_id.clone(),
                }),
                expected_version: 1,
                name: "Peer-only Customer Account".to_owned(),
                status: accounts::AccountStatus::Active as i32,
                party_associations: vec![association(
                    &peer_party_id,
                    accounts::AccountPartyRole::Primary,
                )],
            },
        ),
        TENANT_A,
        &format!("c360-account-update-{run_id}"),
        true,
    )
    .await
    .expect("update Account root membership through real crm-api");

    mutate(
        &mut grpc,
        &contact_point_update,
        payload(
            &contact_point_update,
            contact_points::UpdateContactPointRequest {
                contact_point_ref: Some(customer::ContactPointRef {
                    contact_point_id: contact_point_id.clone(),
                }),
                expected_version: 2,
                value: "ada.updated@example.com".to_owned(),
                status: contact_points::ContactPointStatus::Active as i32,
                preferred: true,
                valid_from: None,
                valid_until: None,
            },
        ),
        TENANT_A,
        &format!("c360-contact-update-{run_id}"),
        true,
    )
    .await
    .expect("update Contact Point through real crm-api");

    mutate(
        &mut grpc,
        &relationship_update,
        payload(
            &relationship_update,
            relationships::UpdatePartyRelationshipRequest {
                party_relationship_ref: Some(customer::PartyRelationshipRef {
                    party_relationship_id: relationship_id.clone(),
                }),
                expected_version: 1,
                status: relationships::PartyRelationshipStatus::Inactive as i32,
                valid_from: None,
                valid_until: Some(core::UnixTime {
                    unix_nanos: now_unix_nanos(),
                }),
            },
        ),
        TENANT_A,
        &format!("c360-relationship-update-{run_id}"),
        true,
    )
    .await
    .expect("update Party Relationship through real crm-api");

    let converged = wait_for_customer_360(
        &mut grpc,
        &customer_360_get,
        TENANT_A,
        &root_party_id,
        |view| {
            view.accounts.is_empty()
                && view.contact_points.len() == 1
                && contact_point_normalized_value(&view.contact_points[0])
                    == "ada.updated@example.com"
                && contact_point_verification_status(&view.contact_points[0])
                    == contact_points::ContactPointVerificationStatus::Unverified as i32
                && view.party_relationships.len() == 1
                && relationship_status(&view.party_relationships[0])
                    == relationships::PartyRelationshipStatus::Inactive as i32
                && relationship_valid_until(&view.party_relationships[0]).is_some()
        },
    )
    .await;
    assert_eq!(
        source_version(
            converged.contact_points[0]
                .source
                .as_ref()
                .expect("Contact Point source lineage")
        ),
        3
    );
    assert_eq!(
        source_version(
            converged.party_relationships[0]
                .source
                .as_ref()
                .expect("Party Relationship source lineage")
        ),
        2
    );
    assert_eq!(
        converged
            .freshness
            .as_ref()
            .expect("converged Customer 360 freshness")
            .applied_event_count,
        initial
            .freshness
            .as_ref()
            .expect("initial Customer 360 freshness")
            .applied_event_count
            + 3
    );

    let peer_view = wait_for_customer_360(
        &mut grpc,
        &customer_360_get,
        TENANT_A,
        &peer_party_id,
        |view| {
            view.accounts.len() == 1
                && account_id_of(&view.accounts[0]) == account_id
                && view.party_relationships.len() == 1
        },
    )
    .await;
    assert_eq!(account_id_of(&peer_view.accounts[0]), account_id);
    assert_eq!(
        source_version(
            peer_view.accounts[0]
                .source
                .as_ref()
                .expect("Account source lineage")
        ),
        2
    );

    let authoritative_before_rebuild = authoritative_evidence(&admin, TENANT_A).await;
    stop_crm_api(&mut child).await;

    let store = PostgresDataStore::connect(&database_url, 5)
        .await
        .expect("connect Customer 360 rebuild store");
    let rebuilt_event_count = Customer360ProjectionWorker::new(store)
        .expect("construct Customer 360 rebuild worker")
        .rebuild(TenantId::try_new(TENANT_A).expect("valid tenant id"), 200)
        .await
        .expect("rebuild Customer 360 from immutable owner event history");
    assert!(rebuilt_event_count >= 9);
    assert_eq!(
        authoritative_evidence(&admin, TENANT_A).await,
        authoritative_before_rebuild,
        "Customer 360 rebuild must not mutate authoritative records, outbox or audit evidence"
    );

    let (mut child, http_addr, grpc_addr) = spawn_crm_api(&database_url);
    wait_until_ready(&http, &mut child, &http_addr).await;
    let mut grpc = connect_grpc(&grpc_addr).await;
    let rebuilt = wait_for_customer_360(
        &mut grpc,
        &customer_360_get,
        TENANT_A,
        &root_party_id,
        |view| {
            view.accounts.is_empty()
                && view.contact_points.len() == 1
                && view.party_relationships.len() == 1
        },
    )
    .await;
    assert_eq!(rebuilt, converged);

    stop_crm_api(&mut child).await;
}

fn spawn_crm_api(database_url: &str) -> (Child, String, String) {
    let http_port = free_port();
    let mut grpc_port = free_port();
    while grpc_port == http_port {
        grpc_port = free_port();
    }
    let http_addr = format!("127.0.0.1:{http_port}");
    let grpc_addr = format!("127.0.0.1:{grpc_port}");
    let child = Command::new(env!("CARGO_BIN_EXE_crm-api"))
        .env("CRM_DATABASE_URL", database_url)
        .env("CRM_HTTP_BIND", &http_addr)
        .env("CRM_GRPC_BIND", &grpc_addr)
        .env("CRM_API_BEARER_TOKEN", TOKEN)
        .env("CRM_API_ACTOR_ID", ACTOR)
        .env("CRM_API_TENANTS", format!("{TENANT_A},{TENANT_B}"))
        .env(
            "CRM_CURSOR_SIGNING_KEY",
            "customer-360-process-cursor-signing-key-0123456789abcdef",
        )
        .env(
            "CRM_APPROVAL_SIGNING_KEY",
            "customer-360-process-approval-signing-key-0123456789abcdef",
        )
        .env("CRM_BOOTSTRAP_ALLOW_PHASE6", "true")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn production crm-api process for Customer 360 acceptance");
    (child, http_addr, grpc_addr)
}

async fn create_party(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: &CapabilityDefinition,
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
        TENANT_A,
        idempotency_key,
        true,
    )
    .await
    .expect("create Party prerequisite through production gateway");
}

fn association(
    party_id: &str,
    role: accounts::AccountPartyRole,
) -> accounts::AccountPartyAssociation {
    accounts::AccountPartyAssociation {
        party_ref: Some(customer::PartyRef {
            party_id: party_id.to_owned(),
        }),
        role: role as i32,
    }
}

fn customer_360_payload(definition: &CapabilityDefinition, party_id: &str) -> TypedPayload {
    payload(
        definition,
        customer_360::GetCustomer360Request {
            party_ref: Some(customer::PartyRef {
                party_id: party_id.to_owned(),
            }),
        },
    )
}

async fn wait_for_customer_360<F>(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: &CapabilityDefinition,
    tenant_id: &str,
    party_id: &str,
    predicate: F,
) -> customer_360::Customer360
where
    F: Fn(&customer_360::Customer360) -> bool,
{
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        match query(
            client,
            definition,
            customer_360_payload(definition, party_id),
            tenant_id,
            true,
        )
        .await
        {
            Ok(response) => {
                let view = decode_customer_360(response);
                if predicate(&view) {
                    return view;
                }
            }
            Err(status) if status.code() == Code::NotFound => {}
            Err(status) => panic!("Customer 360 query failed while awaiting convergence: {status}"),
        }
        assert!(
            Instant::now() < deadline,
            "Customer 360 projection did not converge before deadline"
        );
        sleep(Duration::from_millis(200)).await;
    }
}

fn decode_customer_360(
    response: crm_application_runtime::gateway_v1::QueryResponse,
) -> customer_360::Customer360 {
    customer_360::GetCustomer360Response::decode(
        response
            .output
            .expect("Customer 360 query output")
            .payload
            .as_slice(),
    )
    .expect("decode Customer 360 response")
    .customer_360
    .expect("Customer 360 view exists")
}

fn party_display_name(view: &customer_360::Customer360) -> &str {
    view.party
        .as_ref()
        .and_then(|section| section.party.as_ref())
        .expect("Customer 360 Party section")
        .display_name
        .as_str()
}

fn account_id_of(section: &customer_360::Customer360AccountSection) -> &str {
    section
        .account
        .as_ref()
        .and_then(|account| account.account_ref.as_ref())
        .expect("Customer 360 Account reference")
        .account_id
        .as_str()
}

fn contact_point_id_of(section: &customer_360::Customer360ContactPointSection) -> &str {
    section
        .contact_point
        .as_ref()
        .and_then(|contact_point| contact_point.contact_point_ref.as_ref())
        .expect("Customer 360 Contact Point reference")
        .contact_point_id
        .as_str()
}

fn contact_point_verification_status(
    section: &customer_360::Customer360ContactPointSection,
) -> i32 {
    section
        .contact_point
        .as_ref()
        .and_then(|contact_point| contact_point.verification.as_ref())
        .expect("Customer 360 Contact Point verification")
        .status
}

fn contact_point_normalized_value(section: &customer_360::Customer360ContactPointSection) -> &str {
    section
        .contact_point
        .as_ref()
        .expect("Customer 360 Contact Point")
        .normalized_value
        .as_str()
}

fn relationship_id_of(section: &customer_360::Customer360PartyRelationshipSection) -> &str {
    section
        .party_relationship
        .as_ref()
        .and_then(|relationship| relationship.party_relationship_ref.as_ref())
        .expect("Customer 360 Party Relationship reference")
        .party_relationship_id
        .as_str()
}

fn relationship_status(section: &customer_360::Customer360PartyRelationshipSection) -> i32 {
    section
        .party_relationship
        .as_ref()
        .expect("Customer 360 Party Relationship")
        .status
}

fn relationship_valid_until(
    section: &customer_360::Customer360PartyRelationshipSection,
) -> Option<i64> {
    section
        .party_relationship
        .as_ref()
        .expect("Customer 360 Party Relationship")
        .valid_until
        .as_ref()
        .map(|value| value.unix_nanos)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AuthoritativeEvidence {
    records: i64,
    outbox_events: i64,
    audit_records: i64,
}

async fn authoritative_evidence(admin: &PgPool, tenant_id: &str) -> AuthoritativeEvidence {
    let records = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.records WHERE tenant_id = $1 AND deleted_at IS NULL",
    )
    .bind(tenant_id)
    .fetch_one(admin)
    .await
    .expect("count authoritative records");
    let outbox_events =
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM crm.outbox_events WHERE tenant_id = $1")
            .bind(tenant_id)
            .fetch_one(admin)
            .await
            .expect("count authoritative outbox events");
    let audit_records =
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM crm.audit_records WHERE tenant_id = $1")
            .bind(tenant_id)
            .fetch_one(admin)
            .await
            .expect("count authoritative audit records");
    AuthoritativeEvidence {
        records,
        outbox_events,
        audit_records,
    }
}

fn source_version(source: &customer_360::Customer360SourceLineage) -> i64 {
    source.source_version
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

async fn wait_until_ready(client: &reqwest::Client, child: &mut Child, http_addr: &str) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child.try_wait().expect("poll crm-api process") {
            panic!("crm-api exited before Customer 360 acceptance readiness: {status}");
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
            "Customer 360 acceptance crm-api readiness timed out"
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
                    "Customer 360 acceptance gRPC listener timed out: {error}"
                );
                sleep(Duration::from_millis(200)).await;
            }
        }
    }
}

async fn stop_crm_api(child: &mut Child) {
    let pid = child.id().expect("running crm-api process has a PID");
    let status = Command::new("kill")
        .arg("-INT")
        .arg(pid.to_string())
        .status()
        .await
        .expect("send SIGINT to Customer 360 acceptance crm-api");
    assert!(status.success(), "kill -INT failed: {status}");
    let exit = timeout(Duration::from_secs(15), child.wait())
        .await
        .expect("crm-api must stop within graceful-shutdown budget")
        .expect("wait for Customer 360 acceptance crm-api process");
    assert!(exit.success(), "crm-api exited unsuccessfully: {exit}");
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral Customer 360 acceptance port")
        .local_addr()
        .expect("read ephemeral Customer 360 acceptance port")
        .port()
}

fn now_unix_nanos() -> i64 {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after Unix epoch")
            .as_nanos(),
    )
    .expect("current Unix nanoseconds fit i64")
}

fn unique_id(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after Unix epoch")
        .as_nanos();
    format!("{prefix}-{}-{nanos}", std::process::id())
}
