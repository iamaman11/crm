#![cfg(unix)]

use crm_application_runtime::{
    SystemClock, application_mutation_definitions,
    gateway_v1::{
        MutateRequest as GatewayMutateRequest, TypedPayload as GatewayTypedPayload,
        application_gateway_service_client::ApplicationGatewayServiceClient,
    },
};
use crm_capability_adapters::{
    AuthorizationGrant, LiveAuthorizationStore, LiveCapabilityAuthorizer,
    LiveQueryVisibilityAuthorizer, LiveQueryVisibilityStore, QueryVisibilityGrant,
};
use crm_capability_runtime::CapabilityDefinition;
use crm_core_data::PostgresDataStore;
use crm_data_quality_source_composition::{
    GovernedPartyQualitySource, PartyQualitySource, PartyQualitySourceKind,
    PartyQualitySourceRequest,
};
use crm_module_sdk::{
    ActorId, Clock, DataClass, ErrorCategory, ModuleId, PayloadEncoding, RecordId, RecordType,
    RetentionPolicyId, TenantId, TypedPayload,
};
use crm_parties_query_adapter::{
    GET_CAPABILITY as PARTY_GET_CAPABILITY, PartyQueryAdapter, query_capability_definition,
};
use crm_proto_contracts::crm::{customer::v1 as customer, parties::v1 as parties};
use crm_query_runtime::{CursorCodec, QueryAuthorizer};
use prost::Message;
use std::collections::BTreeSet;
use std::net::TcpListener;
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::process::{Child, Command};
use tokio::time::sleep;
use tonic::{Request, Status};

const TENANT_A: &str = "tenant-a";
const TENANT_B: &str = "tenant-b";
const API_ACTOR: &str = "actor-a";
const SOURCE_ACTOR: &str = "data-quality-source-worker";
const TOKEN: &str = "data-quality-source-token-0123456789abcdef0123456789abcdef";
const APPROVAL_KEY: &str = "data-quality-source-approval-key-0123456789abcdef";
const PARTY_CREATE: &str = "parties.party.create";

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn governed_party_quality_source_authorizes_and_minimizes_live_party_data() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping governed Party quality source proof because DATABASE_URL is absent");
        return;
    };

    let (http_addr, grpc_addr) = free_addresses();
    let http = reqwest::Client::new();
    let mut child = spawn_crm_api(&database_url, &http_addr, &grpc_addr);
    wait_until_ready(&http, &mut child, &http_addr).await;
    let mut grpc = connect_grpc(&grpc_addr).await;

    let party_id = unique_id("party-data-quality-source");
    let created = create_party(&mut grpc, &party_id).await;
    let expected_version = created
        .resource_version
        .as_ref()
        .expect("created Party resource version")
        .version;
    assert!(expected_version > 0);

    let full_source = build_source(
        &database_url,
        TENANT_A,
        true,
        BTreeSet::from(["kind".to_owned(), "display_name".to_owned()]),
    )
    .await;
    let snapshot = full_source
        .get(source_request(TENANT_A, &party_id, "source-visible"))
        .await
        .expect("read visible governed Party quality snapshot");
    assert_eq!(snapshot.party_id.as_str(), party_id);
    assert_eq!(snapshot.kind, PartyQualitySourceKind::Person);
    assert_eq!(snapshot.display_name, "Ada Lovelace");
    assert_eq!(snapshot.resource_version, expected_version);

    let hidden_source = build_source(
        &database_url,
        TENANT_A,
        true,
        BTreeSet::from(["kind".to_owned()]),
    )
    .await;
    let hidden = hidden_source
        .get(source_request(TENANT_A, &party_id, "source-hidden-field"))
        .await
        .expect_err("hidden display_name must prevent source disclosure");
    assert_eq!(hidden.category, ErrorCategory::NotFound);

    let cross_tenant_source = build_source(
        &database_url,
        TENANT_B,
        true,
        BTreeSet::from(["kind".to_owned(), "display_name".to_owned()]),
    )
    .await;
    let cross_tenant = cross_tenant_source
        .get(source_request(TENANT_B, &party_id, "source-cross-tenant"))
        .await
        .expect_err("tenant B must not discover tenant A Party evidence");
    assert_eq!(cross_tenant.category, ErrorCategory::NotFound);
    assert_eq!(cross_tenant.code, hidden.code);
    assert_eq!(cross_tenant.safe_message, hidden.safe_message);

    let denied_source = build_source(
        &database_url,
        TENANT_A,
        false,
        BTreeSet::from(["kind".to_owned(), "display_name".to_owned()]),
    )
    .await;
    let denied = denied_source
        .get(source_request(TENANT_A, &party_id, "source-denied"))
        .await
        .expect_err("missing top-level Party GET grant must deny the source read");
    assert_eq!(denied.category, ErrorCategory::Authorization);
    assert_eq!(denied.code, "DATA_QUALITY_PARTY_SOURCE_PERMISSION_DENIED");

    stop(&mut child).await;
}

async fn build_source(
    database_url: &str,
    tenant: &str,
    authorize: bool,
    allowed_fields: BTreeSet<String>,
) -> GovernedPartyQualitySource {
    let store = PostgresDataStore::connect(database_url, 4)
        .await
        .expect("connect governed Party quality source store");
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let now = clock.now_unix_nanos();
    let expires_at = now
        .checked_add(60_000_000_000)
        .expect("bounded grant expiry");
    let tenant_id = TenantId::try_new(tenant).unwrap();
    let actor_id = ActorId::try_new(SOURCE_ACTOR).unwrap();
    let definition =
        query_capability_definition(PARTY_GET_CAPABILITY).expect("valid Party GET definition");

    let authorization_store = LiveAuthorizationStore::default();
    if authorize {
        authorization_store
            .upsert(AuthorizationGrant {
                tenant_id: tenant_id.clone(),
                actor_id: actor_id.clone(),
                policy_id: definition.authorization_policy_id.clone(),
                capability_id: definition.capability_id.clone(),
                capability_version: definition.capability_version.clone(),
                owner_module_id: definition.owner_module_id.clone(),
                policy_version: "data-quality-party-source-auth/v1".to_owned(),
                expires_at_unix_nanos: Some(expires_at),
            })
            .expect("grant top-level Party GET authorization");
    }
    let authorizer: Arc<dyn QueryAuthorizer> = Arc::new(LiveCapabilityAuthorizer::new(
        authorization_store,
        Arc::clone(&clock),
    ));

    let visibility_store = LiveQueryVisibilityStore::default();
    visibility_store
        .upsert(QueryVisibilityGrant {
            tenant_id,
            actor_id,
            capability_id: definition.capability_id,
            capability_version: definition.capability_version,
            owner_module_id: ModuleId::try_new("crm.parties").unwrap(),
            record_type: RecordType::try_new("parties.party").unwrap(),
            record_id: None,
            allowed_fields,
            policy_version: "data-quality-party-source-visibility/v1".to_owned(),
            expires_at_unix_nanos: Some(expires_at),
        })
        .expect("grant Party resource and field visibility");
    let visibility = Arc::new(LiveQueryVisibilityAuthorizer::new(
        visibility_store,
        Arc::clone(&clock),
    ));
    let adapter = Arc::new(
        PartyQueryAdapter::new(
            store,
            CursorCodec::new([7; 32]).expect("valid source cursor key"),
            visibility,
        )
        .expect("construct Party query adapter"),
    );
    GovernedPartyQualitySource::new(adapter, authorizer)
}

fn source_request<'a>(
    tenant: &'a str,
    party_id: &'a str,
    request_identity: &'a str,
) -> PartyQualitySourceRequest<'a> {
    PartyQualitySourceRequest {
        tenant_id: Box::leak(Box::new(TenantId::try_new(tenant).unwrap())),
        actor_id: Box::leak(Box::new(ActorId::try_new(SOURCE_ACTOR).unwrap())),
        request_identity,
        party_id: Box::leak(Box::new(RecordId::try_new(party_id).unwrap())),
        request_started_at_unix_nanos: SystemClock.now_unix_nanos(),
    }
}

async fn create_party(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    party_id: &str,
) -> parties::Party {
    let definition = mutation_definition(PARTY_CREATE);
    let response = mutate(
        client,
        &definition,
        payload(
            &definition,
            parties::CreatePartyRequest {
                party_ref: Some(customer::PartyRef {
                    party_id: party_id.to_owned(),
                }),
                kind: parties::PartyKind::Person as i32,
                display_name: "Ada Lovelace".to_owned(),
            },
        ),
        &unique_id("create-party-for-data-quality-source"),
    )
    .await
    .expect("create Party through governed crm-api mutation");
    parties::CreatePartyResponse::decode(
        response
            .output
            .expect("Party create output")
            .payload
            .as_slice(),
    )
    .expect("decode Party create response")
    .party
    .expect("created Party")
}

async fn mutate(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: &CapabilityDefinition,
    input: TypedPayload,
    idempotency_key: &str,
) -> Result<crm_application_runtime::gateway_v1::MutateResponse, Status> {
    let mut request = Request::new(GatewayMutateRequest {
        owner_module_id: definition.owner_module_id.as_str().to_owned(),
        capability_id: definition.capability_id.as_str().to_owned(),
        capability_version: definition.capability_version.as_str().to_owned(),
        input: Some(wire_payload(input)),
        approval: None,
    });
    request
        .metadata_mut()
        .insert("x-tenant-id", TENANT_A.parse().unwrap());
    request
        .metadata_mut()
        .insert("idempotency-key", idempotency_key.parse().unwrap());
    request
        .metadata_mut()
        .insert("authorization", format!("Bearer {TOKEN}").parse().unwrap());
    client
        .mutate(request)
        .await
        .map(|response| response.into_inner())
}

fn mutation_definition(capability_id: &str) -> CapabilityDefinition {
    application_mutation_definitions()
        .expect("valid mutation definitions")
        .into_iter()
        .find(|definition| definition.capability_id.as_str() == capability_id)
        .unwrap_or_else(|| panic!("missing mutation definition: {capability_id}"))
}

fn payload<M: Message>(definition: &CapabilityDefinition, message: M) -> TypedPayload {
    let payload = TypedPayload {
        owner: definition.input_contract.owner.clone(),
        schema_id: definition.input_contract.schema_id.clone(),
        schema_version: definition.input_contract.schema_version.clone(),
        descriptor_hash: definition.input_contract.descriptor_hash,
        data_class: *definition
            .input_contract
            .allowed_data_classes
            .first()
            .expect("input data class"),
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

fn spawn_crm_api(database_url: &str, http_addr: &str, grpc_addr: &str) -> Child {
    Command::new(env!("CARGO_BIN_EXE_crm-api"))
        .env("CRM_DATABASE_URL", database_url)
        .env("CRM_HTTP_BIND", http_addr)
        .env("CRM_GRPC_BIND", grpc_addr)
        .env("CRM_API_BEARER_TOKEN", TOKEN)
        .env("CRM_API_ACTOR_ID", API_ACTOR)
        .env("CRM_API_TENANTS", format!("{TENANT_A},{TENANT_B}"))
        .env(
            "CRM_CURSOR_SIGNING_KEY",
            "data-quality-source-cursor-key-0123456789abcdef",
        )
        .env("CRM_APPROVAL_SIGNING_KEY", APPROVAL_KEY)
        .env("CRM_BOOTSTRAP_ALLOW_PHASE6", "true")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn crm-api for governed Party source proof")
}

async fn wait_until_ready(client: &reqwest::Client, child: &mut Child, http_addr: &str) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child.try_wait().expect("poll Party source crm-api") {
            panic!("crm-api exited before Party source readiness: {status}");
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
            "Party source readiness timed out"
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
                    "Party source gRPC listener timed out: {error}"
                );
                sleep(Duration::from_millis(200)).await;
            }
        }
    }
}

async fn stop(child: &mut Child) {
    let pid = child.id().expect("running Party source crm-api has a PID");
    let status = Command::new("kill")
        .arg("-INT")
        .arg(pid.to_string())
        .status()
        .await
        .expect("send SIGINT to Party source crm-api");
    assert!(status.success(), "kill -INT failed: {status}");
    let status = child.wait().await.expect("wait for Party source crm-api");
    assert!(
        status.success(),
        "Party source crm-api exited unsuccessfully: {status}"
    );
}

fn free_addresses() -> (String, String) {
    let http_port = free_port();
    let mut grpc_port = free_port();
    while grpc_port == http_port {
        grpc_port = free_port();
    }
    (
        format!("127.0.0.1:{http_port}"),
        format!("127.0.0.1:{grpc_port}"),
    )
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral Party source test port")
        .local_addr()
        .expect("read ephemeral Party source test address")
        .port()
}

fn unique_id(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after epoch")
        .as_nanos();
    format!("{prefix}-{nanos}")
}
