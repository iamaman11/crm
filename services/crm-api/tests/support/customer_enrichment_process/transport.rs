use crm_application_runtime::gateway_v1::{
    MutateRequest as GatewayMutateRequest, QueryRequest as GatewayQueryRequest,
    TypedPayload as GatewayTypedPayload,
    application_gateway_service_client::ApplicationGatewayServiceClient,
};
use crm_capability_runtime::CapabilityDefinition;
use crm_module_sdk::TypedPayload;
use reqwest::{Client as HttpClient, Response as HttpResponse};
use std::net::TcpListener;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::process::{Child, Command};
use tokio::time::{sleep, timeout};
use tonic::{Request, Status};

use super::{ACTOR, TENANT_A, TENANT_B, TOKEN, data_class_name};

pub fn spawn_crm_api(
    database_url: &str,
    http_addr: &str,
    grpc_addr: &str,
    bootstrap: bool,
    hidden_fields: Option<&str>,
) -> Child {
    spawn_crm_api_with_actor(
        database_url,
        http_addr,
        grpc_addr,
        bootstrap,
        hidden_fields,
        ACTOR,
    )
}

/// Spawns the same production process with an explicitly fixture-bound actor.
/// Multi-tenant process tests must choose an actor that exists in every tenant
/// granted to the bearer token so audit foreign keys remain authoritative.
pub fn spawn_crm_api_with_actor(
    database_url: &str,
    http_addr: &str,
    grpc_addr: &str,
    bootstrap: bool,
    hidden_fields: Option<&str>,
    actor_id: &str,
) -> Child {
    let mut command = Command::new(env!("CARGO_BIN_EXE_crm-api"));
    command
        .env("CRM_DATABASE_URL", database_url)
        .env("CRM_HTTP_BIND", http_addr)
        .env("CRM_GRPC_BIND", grpc_addr)
        .env("CRM_API_BEARER_TOKEN", TOKEN)
        .env("CRM_API_ACTOR_ID", actor_id)
        .env("CRM_API_TENANTS", format!("{TENANT_A},{TENANT_B}"))
        .env(
            "CRM_CURSOR_SIGNING_KEY",
            "customer-enrichment-process-cursor-key-0123456789abcdef",
        )
        .env(
            "CRM_APPROVAL_SIGNING_KEY",
            "customer-enrichment-process-approval-key-0123456789abcdef",
        )
        .env(
            "CRM_BOOTSTRAP_ALLOW_PHASE6",
            if bootstrap { "true" } else { "false" },
        )
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);
    if let Some(hidden_fields) = hidden_fields {
        command.env("CRM_QUERY_HIDDEN_FIELDS", hidden_fields);
    }
    command.spawn().expect("spawn crm-api process")
}

pub async fn wait_until_ready(
    client: &HttpClient,
    child: &mut Child,
    http_addr: &str,
    require_ready: bool,
) {
    let path = if require_ready { "readyz" } else { "healthz" };
    let deadline = Instant::now() + Duration::from_secs(45);
    loop {
        if let Some(status) = child.try_wait().expect("poll crm-api process") {
            panic!("crm-api exited before {path}: {status}");
        }
        if let Ok(response) = client
            .get(format!("http://{http_addr}/{path}"))
            .send()
            .await
            && response.status().is_success()
        {
            return;
        }
        assert!(Instant::now() < deadline, "crm-api {path} timed out");
        sleep(Duration::from_millis(200)).await;
    }
}

pub async fn connect_grpc(
    grpc_addr: &str,
) -> ApplicationGatewayServiceClient<tonic::transport::Channel> {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        match ApplicationGatewayServiceClient::connect(format!("http://{grpc_addr}")).await {
            Ok(client) => return client,
            Err(error) => {
                assert!(
                    Instant::now() < deadline,
                    "crm-api gRPC listener timed out: {error}"
                );
                sleep(Duration::from_millis(200)).await;
            }
        }
    }
}

pub async fn mutate(
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

pub async fn query(
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

pub async fn http_mutate(
    client: &HttpClient,
    http_addr: &str,
    definition: &CapabilityDefinition,
    input: &TypedPayload,
    tenant_id: &str,
    idempotency_key: &str,
    authenticated: bool,
) -> HttpResponse {
    let mut request = client
        .post(format!(
            "http://{http_addr}/v1/mutations/{}/{}/{}",
            definition.owner_module_id, definition.capability_id, definition.capability_version
        ))
        .header("x-tenant-id", tenant_id)
        .header("idempotency-key", idempotency_key)
        .json(input);
    if authenticated {
        request = request.bearer_auth(TOKEN);
    }
    request.send().await.expect("send HTTP mutation")
}

pub async fn stop_process(child: &mut Child) {
    let pid = child.id().expect("running crm-api process PID");
    let status = Command::new("kill")
        .arg("-INT")
        .arg(pid.to_string())
        .status()
        .await
        .expect("send SIGINT to crm-api");
    assert!(status.success(), "kill -INT failed: {status}");
    let exit = timeout(Duration::from_secs(20), child.wait())
        .await
        .expect("crm-api graceful shutdown timeout")
        .expect("wait for crm-api process");
    assert!(exit.success(), "crm-api exited unsuccessfully: {exit}");
}

pub fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("read ephemeral port")
        .port()
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
