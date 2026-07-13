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
use crm_proto_contracts::crm::{customer::v1 as customer, parties::v1 as parties};
use prost::Message;
use sqlx::{Executor, PgPool};
use std::net::TcpListener;
use std::process::Stdio;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::process::{Child, Command};
use tokio::time::{sleep, timeout};
use tonic::{Code, Request, Status};

const TENANT_A: &str = "party-process-a";
const TENANT_B: &str = "party-process-b";
const ACTOR: &str = "party-process-actor";
const TOKEN: &str = "party-process-bearer-token-0123456789abcdef0123456789abcdef";
const PARTY_CREATE: &str = "parties.party.create";
const PARTY_GET: &str = "parties.party.get";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EvidenceCounts {
    records: i64,
    events: i64,
    audits: i64,
    idempotency: i64,
    transactions: i64,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn crm_api_process_serves_governed_party_create_get_replay_conflict_and_tenant_isolation() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping Party process acceptance because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect Party process evidence reader");
    admin
        .execute(sqlx::raw_sql(include_str!(
            "../../../database/tests/0005_party_adapter.sql"
        )))
        .await
        .expect("publish Party module/capability registry fixture");

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
            "party-process-cursor-signing-key-0123456789abcdef",
        )
        .env(
            "CRM_APPROVAL_SIGNING_KEY",
            "party-process-approval-signing-key-0123456789abcdef",
        )
        .env("CRM_BOOTSTRAP_ALLOW_PHASE6", "true")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn production crm-api process for Party acceptance");

    let http = reqwest::Client::new();
    wait_until_ready(&http, &mut child, &http_addr).await;
    let mut grpc = connect_grpc(&grpc_addr).await;

    let baseline = evidence_counts(&admin, TENANT_A).await;
    let party_id = unique_party_id();
    let create_definition = mutation_definition(PARTY_CREATE);
    let create_payload = payload(
        &create_definition,
        parties::CreatePartyRequest {
            party_ref: Some(customer::PartyRef {
                party_id: party_id.clone(),
            }),
            kind: parties::PartyKind::Person as i32,
            display_name: "  Ada   Lovelace  ".to_owned(),
        },
    );

    let unauthorized = mutate(
        &mut grpc,
        &create_definition,
        create_payload.clone(),
        TENANT_A,
        "party-process-create",
        false,
    )
    .await
    .expect_err("unauthenticated Party mutation must fail");
    assert_eq!(unauthorized.code(), Code::Unauthenticated);
    assert_eq!(evidence_counts(&admin, TENANT_A).await, baseline);

    let created = mutate(
        &mut grpc,
        &create_definition,
        create_payload.clone(),
        TENANT_A,
        "party-process-create",
        true,
    )
    .await
    .expect("create Party through production gRPC gateway");
    assert!(!created.replayed);
    assert_eq!(created.affected_resources.len(), 1);
    assert_eq!(created.affected_resources[0].resource_type, "parties.party");
    assert_eq!(created.affected_resources[0].resource_id, party_id);
    assert_eq!(created.affected_resources[0].version, Some(1));
    let created_output = created.output.expect("Party create output");
    let created_party = parties::CreatePartyResponse::decode(created_output.payload.as_slice())
        .expect("decode Party create response")
        .party
        .expect("created Party exists");
    assert_eq!(
        created_party
            .party_ref
            .as_ref()
            .expect("created Party reference")
            .party_id,
        party_id
    );
    assert_eq!(created_party.kind, parties::PartyKind::Person as i32);
    assert_eq!(created_party.display_name, "Ada Lovelace");
    assert_eq!(
        created_party
            .resource_version
            .as_ref()
            .expect("created Party version")
            .version,
        1
    );

    let after_create = evidence_counts(&admin, TENANT_A).await;
    assert_eq!(after_create.records, baseline.records + 1);
    assert_eq!(after_create.events, baseline.events + 1);
    assert_eq!(after_create.audits, baseline.audits + 1);
    assert_eq!(after_create.idempotency, baseline.idempotency + 1);
    assert_eq!(after_create.transactions, baseline.transactions + 1);

    let replay = mutate(
        &mut grpc,
        &create_definition,
        create_payload.clone(),
        TENANT_A,
        "party-process-create",
        true,
    )
    .await
    .expect("replay Party create through production gateway");
    assert!(replay.replayed);
    assert_eq!(
        replay.output.expect("replay output").payload,
        created_output.payload
    );
    assert_eq!(evidence_counts(&admin, TENANT_A).await, after_create);

    let conflicting_payload = payload(
        &create_definition,
        parties::CreatePartyRequest {
            party_ref: Some(customer::PartyRef {
                party_id: party_id.clone(),
            }),
            kind: parties::PartyKind::Person as i32,
            display_name: "Augusta Ada King".to_owned(),
        },
    );
    let conflict = mutate(
        &mut grpc,
        &create_definition,
        conflicting_payload,
        TENANT_A,
        "party-process-create",
        true,
    )
    .await
    .expect_err("conflicting Party idempotency replay must fail");
    assert_eq!(conflict.code(), Code::Aborted);
    assert_eq!(evidence_counts(&admin, TENANT_A).await, after_create);

    let query_definition = query_definition(PARTY_GET);
    let get_payload = payload(
        &query_definition,
        parties::GetPartyRequest {
            party_ref: Some(customer::PartyRef {
                party_id: party_id.clone(),
            }),
        },
    );

    let queried = query(
        &mut grpc,
        &query_definition,
        get_payload.clone(),
        TENANT_A,
        true,
    )
    .await
    .expect("query Party through production gRPC gateway");
    let queried_party = parties::GetPartyResponse::decode(
        queried
            .output
            .expect("Party query output")
            .payload
            .as_slice(),
    )
    .expect("decode Party query response")
    .party
    .expect("queried Party exists");
    assert_eq!(
        queried_party
            .party_ref
            .as_ref()
            .expect("queried Party reference")
            .party_id,
        party_id
    );
    assert_eq!(queried_party.kind, parties::PartyKind::Person as i32);
    assert_eq!(queried_party.display_name, "Ada Lovelace");
    assert_eq!(evidence_counts(&admin, TENANT_A).await, after_create);

    let unauthorized_query = query(
        &mut grpc,
        &query_definition,
        get_payload.clone(),
        TENANT_A,
        false,
    )
    .await
    .expect_err("unauthenticated Party query must fail");
    assert_eq!(unauthorized_query.code(), Code::Unauthenticated);

    let cross_tenant = query(&mut grpc, &query_definition, get_payload, TENANT_B, true)
        .await
        .expect_err("tenant B must not discover tenant A Party");
    assert_eq!(cross_tenant.code(), Code::NotFound);
    assert_eq!(evidence_counts(&admin, TENANT_A).await, after_create);

    send_sigint(&child).await;
    let exit = timeout(Duration::from_secs(15), child.wait())
        .await
        .expect("crm-api must stop within graceful-shutdown budget")
        .expect("wait for Party acceptance crm-api process");
    assert!(exit.success(), "crm-api exited unsuccessfully: {exit}");
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

async fn evidence_counts(admin: &PgPool, tenant_id: &str) -> EvidenceCounts {
    let records = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.records WHERE tenant_id = $1 AND owner_module_id = 'crm.parties' AND record_type = 'parties.party' AND deleted_at IS NULL",
    )
    .bind(tenant_id)
    .fetch_one(admin)
    .await
    .expect("count Party records");
    let events = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.outbox_events WHERE tenant_id = $1 AND event_type = 'parties.party.created'",
    )
    .bind(tenant_id)
    .fetch_one(admin)
    .await
    .expect("count Party outbox events");
    let audits =
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM crm.audit_records WHERE tenant_id = $1")
            .bind(tenant_id)
            .fetch_one(admin)
            .await
            .expect("count Party tenant audit evidence");
    let idempotency = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.idempotency_records WHERE tenant_id = $1",
    )
    .bind(tenant_id)
    .fetch_one(admin)
    .await
    .expect("count Party tenant idempotency evidence");
    let transactions = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.business_transactions WHERE tenant_id = $1",
    )
    .bind(tenant_id)
    .fetch_one(admin)
    .await
    .expect("count Party tenant business transactions");
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
            panic!("crm-api exited before Party acceptance readiness: {status}");
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
            "Party acceptance crm-api readiness timed out"
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
                    "Party acceptance gRPC listener timed out: {error}"
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
        .expect("send SIGINT to Party acceptance crm-api");
    assert!(status.success(), "kill -INT failed: {status}");
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral Party acceptance port")
        .local_addr()
        .expect("read ephemeral Party acceptance port")
        .port()
}

fn unique_party_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after Unix epoch")
        .as_nanos();
    format!("party-process-{}-{nanos}", std::process::id())
}
