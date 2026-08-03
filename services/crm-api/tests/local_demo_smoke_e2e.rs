mod support;

use crm_application_runtime::gateway_v1::application_gateway_service_client::ApplicationGatewayServiceClient;
use crm_proto_contracts::crm::{customer::v1 as customer, parties::v1 as parties};
use prost::Message;
use reqwest::Client as HttpClient;
use support::customer_enrichment_process::*;
use tonic::Code;

const DATASET_VERSION: &str = "crm.local-demo.dataset/v1";
const PARTY_CREATE: &str = "parties.party.create";
const PARTY_GET: &str = "parties.party.get";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn deterministic_local_demo_seed_or_smoke() {
    let Ok(mode) = std::env::var("CRM_LOCAL_DEMO_MODE") else {
        eprintln!("skipping local demo process test without CRM_LOCAL_DEMO_MODE");
        return;
    };
    assert!(matches!(mode.as_str(), "seed" | "smoke"));
    assert_eq!(
        std::env::var("CRM_LOCAL_DEMO_DATASET_VERSION").unwrap(),
        DATASET_VERSION
    );
    let database_url = std::env::var("DATABASE_URL").expect("local demo DATABASE_URL");
    let _admin_database_url =
        std::env::var("ADMIN_DATABASE_URL").expect("local demo ADMIN_DATABASE_URL");
    let party_id = std::env::var("CRM_LOCAL_DEMO_PARTY_ID").expect("local demo Party ID");
    let display_name =
        std::env::var("CRM_LOCAL_DEMO_PARTY_DISPLAY_NAME").expect("local demo Party name");
    let idempotency_key =
        std::env::var("CRM_LOCAL_DEMO_IDEMPOTENCY_KEY").expect("local demo idempotency key");

    let http_port = free_port();
    let grpc_port = free_port();
    let http_addr = format!("127.0.0.1:{http_port}");
    let grpc_addr = format!("127.0.0.1:{grpc_port}");
    let http = HttpClient::new();
    let mut process = spawn_crm_api(
        &database_url,
        &http_addr,
        &grpc_addr,
        mode == "seed",
        None,
    );
    wait_until_ready(&http, &mut process, &http_addr, true).await;
    let mut gateway = connect_grpc(&grpc_addr).await;

    if mode == "seed" {
        create_demo_party(
            &mut gateway,
            &party_id,
            &display_name,
            &idempotency_key,
        )
        .await;
    }

    let party = get_demo_party(&mut gateway, &party_id, TENANT_A, true)
        .await
        .expect("authenticated local demo Party query");
    assert_eq!(
        party
            .party_ref
            .as_ref()
            .expect("local demo Party reference")
            .party_id,
        party_id
    );
    assert_eq!(party.display_name, display_name);
    assert_eq!(party.kind, parties::PartyKind::Organization as i32);

    if mode == "smoke" {
        let unauthenticated = get_demo_party(&mut gateway, &party_id, TENANT_A, false)
            .await
            .expect_err("local demo query without token must fail");
        assert!(matches!(
            unauthenticated.code(),
            Code::Unauthenticated | Code::PermissionDenied
        ));

        let cross_tenant = get_demo_party(&mut gateway, &party_id, TENANT_B, true)
            .await
            .expect_err("tenant B must not observe tenant A demo data");
        assert_eq!(cross_tenant.code(), Code::NotFound);
    }

    stop_process(&mut process).await;
}

async fn create_demo_party(
    gateway: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    party_id: &str,
    display_name: &str,
    idempotency_key: &str,
) {
    let definition = mutation_definition(PARTY_CREATE);
    let input = payload(
        &definition,
        parties::CreatePartyRequest {
            party_ref: Some(customer::PartyRef {
                party_id: party_id.to_owned(),
            }),
            kind: parties::PartyKind::Organization as i32,
            display_name: display_name.to_owned(),
        },
    );
    mutate(
        gateway,
        &definition,
        input,
        TENANT_A,
        idempotency_key,
        true,
    )
    .await
    .expect("create or idempotently replay local demo Party");
}

async fn get_demo_party(
    gateway: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    party_id: &str,
    tenant_id: &str,
    authenticated: bool,
) -> Result<parties::Party, tonic::Status> {
    let definition = query_definition(PARTY_GET);
    let input = payload(
        &definition,
        parties::GetPartyRequest {
            party_ref: Some(customer::PartyRef {
                party_id: party_id.to_owned(),
            }),
        },
    );
    let response = query(gateway, &definition, input, tenant_id, authenticated).await?;
    let output = response.output.expect("local demo Party query output");
    let decoded = parties::GetPartyResponse::decode(output.payload.as_slice())
        .expect("decode local demo Party query response");
    Ok(decoded.party.expect("queried local demo Party"))
}
