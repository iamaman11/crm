use super::data_quality_evaluation_fixture::{API_ACTOR, APPROVAL_KEY, TENANT, TOKEN};
use crm_application_runtime::gateway_v1::application_gateway_service_client::ApplicationGatewayServiceClient;
use std::net::TcpListener;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::process::{Child, Command};
use tokio::time::sleep;

pub struct RunningApi {
    pub child: Child,
    pub client: ApplicationGatewayServiceClient<tonic::transport::Channel>,
}

pub async fn start(database_url: &str) -> RunningApi {
    let (http_addr, grpc_addr) = free_addresses();
    let mut child = Command::new(env!("CARGO_BIN_EXE_crm-api"))
        .env("CRM_DATABASE_URL", database_url)
        .env("CRM_HTTP_BIND", &http_addr)
        .env("CRM_GRPC_BIND", &grpc_addr)
        .env("CRM_API_BEARER_TOKEN", TOKEN)
        .env("CRM_API_ACTOR_ID", API_ACTOR)
        .env("CRM_API_TENANTS", TENANT)
        .env(
            "CRM_CURSOR_SIGNING_KEY",
            "data-quality-evaluation-cursor-key-0123456789abcdef",
        )
        .env("CRM_APPROVAL_SIGNING_KEY", APPROVAL_KEY)
        .env("CRM_BOOTSTRAP_ALLOW_PHASE6", "true")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn crm-api for evaluation staging proof");
    let http = reqwest::Client::new();
    wait_until_ready(&http, &mut child, &http_addr).await;
    let client = connect_grpc(&grpc_addr).await;
    RunningApi { child, client }
}

pub async fn stop(api: &mut RunningApi) {
    let pid = api.child.id().expect("running evaluation crm-api PID");
    let status = Command::new("kill")
        .arg("-INT")
        .arg(pid.to_string())
        .status()
        .await
        .expect("send SIGINT to evaluation crm-api");
    assert!(status.success(), "kill -INT failed: {status}");
    let status = api.child.wait().await.expect("wait for evaluation crm-api");
    assert!(status.success(), "crm-api exited unsuccessfully: {status}");
}

async fn wait_until_ready(client: &reqwest::Client, child: &mut Child, http_addr: &str) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child.try_wait().expect("poll evaluation crm-api") {
            panic!("crm-api exited before readiness: {status}");
        }
        if let Ok(response) = client
            .get(format!("http://{http_addr}/readyz"))
            .send()
            .await
            && response.status().is_success()
        {
            return;
        }
        assert!(Instant::now() < deadline, "evaluation readiness timed out");
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
                    "evaluation gRPC listener timed out: {error}"
                );
                sleep(Duration::from_millis(200)).await;
            }
        }
    }
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
        .expect("bind ephemeral evaluation test port")
        .local_addr()
        .expect("read evaluation test port")
        .port()
}
