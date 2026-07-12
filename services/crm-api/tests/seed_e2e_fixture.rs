#![cfg(unix)]

use crm_application_runtime::gateway_v1::{
    QueryRequest as GatewayQueryRequest, TypedPayload as GatewayTypedPayload,
    application_gateway_service_client::ApplicationGatewayServiceClient,
};
use crm_capability_runtime::CapabilityDefinition;
use crm_module_sdk::{DataClass, PayloadEncoding, RetentionPolicyId, TypedPayload};
use crm_proto_contracts::crm::{core::v1 as core, sales::v1 as sales, search::v1 as search};
use crm_sales_activities_capability_composition::{
    DEAL_TIMELINE_PROJECTION_ID, DEAL_TIMELINE_RESOURCE_TYPE, TASK_STATUS_PROJECTION_ID,
    TASK_STATUS_RESOURCE_TYPE, capability_definitions, query_capability_definitions,
};
use prost::Message;
use reqwest::StatusCode;
use sqlx::PgPool;
use std::net::TcpListener;
use std::time::{Duration, Instant};
use tokio::process::Command;
use tokio::time::sleep;
use tonic::Request;

const TENANT: &str = "tenant-a";
const ACTOR: &str = "actor-a";
const TOKEN: &str = "phase6l-process-bearer-token-0123456789abcdef0123456789abcdef";
const DEAL_ID: &str = "phase6l-process-deal";
const SALES_CREATE: &str = "sales.deal.create";
const SALES_ADVANCE: &str = "sales.deal.advance_stage";
const SALES_GET: &str = "sales.deal.get";
const SEARCH_GLOBAL: &str = "search.global.query";
const LINK_MODULE_ID: &str = "crm.sales-activities-link";

#[tokio::test]
async fn seed_e2e_fixture_records() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        panic!("DATABASE_URL is required for seed_e2e_fixture_records");
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL is required");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect admin database");

    provision_link_module(&admin).await;

    let baseline_tasks = task_count(&admin).await;
    let baseline_task_status_documents = task_status_document_count(&admin).await;
    let http_port = free_port();
    let grpc_port = free_port();
    let http_addr = format!("127.0.0.1:{http_port}");
    let grpc_addr = format!("127.0.0.1:{grpc_port}");

    println!("Starting crm-api for seeding on HTTP={http_addr}, gRPC={grpc_addr}");
    let child = Command::new(env!("CARGO_BIN_EXE_crm-api"))
        .env("CRM_DATABASE_URL", &database_url)
        .env("CRM_HTTP_BIND", &http_addr)
        .env("CRM_GRPC_BIND", &grpc_addr)
        .env("CRM_API_BEARER_TOKEN", TOKEN)
        .env("CRM_API_ACTOR_ID", ACTOR)
        .env("CRM_API_TENANTS", TENANT)
        .env("CRM_BOOTSTRAP_ALLOW_PHASE6", "true")
        .env("CRM_CURSOR_SIGNING_KEY", "phase6l-process-cursor-signing-key-0123456789abcdef")
        .env("CRM_APPROVAL_SIGNING_KEY", "phase6l-process-approval-signing-key-0123456789abcdef")
        .spawn()
        .expect("spawn crm-api process");

    let http = reqwest::Client::new();
    wait_until_ready(&http, &child, &http_addr).await;

    // Trigger sales.deal.create mutation
    let create_definition = mutation_definition(SALES_CREATE);
    let create_payload = payload(
        &create_definition,
        sales::CreateDealRequest {
            deal_id: DEAL_ID.to_owned(),
            name: "Phase 6L process deal".to_owned(),
            owner: Some(actor_owner()),
            account: None,
            primary_contact: None,
            stage: Some(sales::DealStage {
                pipeline_id: "pipeline.phase6l".to_owned(),
                stage_id: "qualification".to_owned(),
                ordinal: 1,
            }),
            amount: Some(core::ExactMoney {
                minor_units: "250000".to_owned(),
                currency_code: "USD".to_owned(),
            }),
            expected_close_date: Some(core::CalendarDate {
                year: 2027,
                month: 12,
                day: 31,
            }),
            probability_basis_points: 3_500,
        },
    );
    let created = authenticated_mutation(
        &http,
        &http_addr,
        &create_definition,
        &create_payload,
        "phase6l-create",
    )
    .await;
    assert_eq!(created, StatusCode::OK);

    // Trigger sales.deal.advance_stage mutation
    let advance_definition = mutation_definition(SALES_ADVANCE);
    let advance_payload = payload(
        &advance_definition,
        sales::AdvanceStageRequest {
            deal_id: DEAL_ID.to_owned(),
            expected_version: 1,
            target_stage: Some(sales::DealStage {
                pipeline_id: "pipeline.phase6l".to_owned(),
                stage_id: "proposal".to_owned(),
                ordinal: 2,
            }),
            target_status: sales::DealStatus::Open as i32,
            close_reason_code: None,
            policy: Some(sales::StageTransitionPolicy {
                allow_regression: false,
                allow_skip: false,
            }),
        },
    );
    let advanced = authenticated_mutation(
        &http,
        &http_addr,
        &advance_definition,
        &advance_payload,
        "phase6l-advance",
    )
    .await;
    assert_eq!(advanced, StatusCode::OK);

    wait_for_background_effects(&admin, baseline_tasks, baseline_task_status_documents).await;

    // Verify search matches
    let query_definition = query_definition(SEARCH_GLOBAL);
    let search_payload = wire_payload(payload(
        &query_definition,
        search::SearchRequest {
            text: "Phase 6L process deal".to_owned(),
            resource_types: vec!["sales.deal".to_owned()],
            page_size: 25,
            cursor: String::new(),
        },
    ));
    let mut grpc = connect_grpc(&grpc_addr).await;
    let mut request = Request::new(GatewayQueryRequest {
        owner_module_id: query_definition.owner_module_id.as_str().to_owned(),
        capability_id: query_definition.capability_id.as_str().to_owned(),
        capability_version: query_definition.capability_version.as_str().to_owned(),
        input: Some(search_payload.clone()),
    });
    request.metadata_mut().insert(
        "authorization",
        format!("Bearer {TOKEN}").parse().unwrap(),
    );
    request.metadata_mut().insert(
        "x-tenant-id",
        TENANT.parse().unwrap(),
    );
    let response = grpc.query(request).await.expect("query search").into_inner();
    let output = response.output.expect("output");
    let page = search::SearchResponse::decode(output.payload.as_slice()).expect("decode response");
    assert!(page.hits.iter().any(|hit| hit.resource_id == DEAL_ID));

    println!("Seeding completed successfully!");
}

async fn authenticated_mutation(
    client: &reqwest::Client,
    http_addr: &str,
    definition: &CapabilityDefinition,
    input: &TypedPayload,
    idempotency_key: &str,
) -> StatusCode {
    client
        .post(format!(
            "http://{http_addr}/v1/mutations/{}/{}/{}",
            definition.owner_module_id, definition.capability_id, definition.capability_version
        ))
        .bearer_auth(TOKEN)
        .header("x-tenant-id", TENANT)
        .header("idempotency-key", idempotency_key)
        .json(input)
        .send()
        .await
        .expect("send mutation")
        .status()
}

async fn wait_until_ready(client: &reqwest::Client, _child: &tokio::process::Child, http_addr: &str) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Ok(response) = client.get(format!("http://{http_addr}/readyz")).send().await {
            if response.status() == StatusCode::OK {
                return;
            }
        }
        assert!(Instant::now() < deadline, "readiness timeout");
        sleep(Duration::from_millis(200)).await;
    }
}

async fn wait_for_background_effects(
    admin: &PgPool,
    baseline_tasks: i64,
    baseline_task_status_documents: i64,
) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let tasks = task_count(admin).await;
        let timeline_entries = deal_timeline_entry_count(admin).await;
        let task_status_documents = task_status_document_count(admin).await;
        if tasks > baseline_tasks
            && timeline_entries >= 2
            && task_status_documents > baseline_task_status_documents
        {
            return;
        }
        assert!(Instant::now() < deadline, "background effects timeout");
        sleep(Duration::from_millis(250)).await;
    }
}

async fn connect_grpc(
    grpc_addr: &str,
) -> ApplicationGatewayServiceClient<tonic::transport::Channel> {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        match ApplicationGatewayServiceClient::connect(format!("http://{grpc_addr}")).await {
            Ok(client) => return client,
            Err(_) => {
                assert!(Instant::now() < deadline, "gRPC timeout");
                sleep(Duration::from_millis(200)).await;
            }
        }
    }
}

fn mutation_definition(capability_id: &str) -> CapabilityDefinition {
    capability_definitions()
        .unwrap()
        .into_iter()
        .find(|d| d.capability_id.as_str() == capability_id)
        .unwrap()
}

fn query_definition(capability_id: &str) -> CapabilityDefinition {
    query_capability_definitions()
        .unwrap()
        .into_iter()
        .find(|d| d.capability_id.as_str() == capability_id)
        .unwrap()
}

fn payload<M: Message>(definition: &CapabilityDefinition, message: M) -> TypedPayload {
    let payload = TypedPayload {
        owner: definition.input_contract.owner.clone(),
        schema_id: definition.input_contract.schema_id.clone(),
        schema_version: definition.input_contract.schema_version.clone(),
        descriptor_hash: definition.input_contract.descriptor_hash,
        data_class: DataClass::Confidential,
        encoding: PayloadEncoding::Protobuf,
        maximum_size_bytes: definition.input_contract.maximum_size_bytes,
        retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
        bytes: message.encode_to_vec(),
    };
    payload.validate().unwrap();
    payload
}

fn wire_payload(payload: TypedPayload) -> GatewayTypedPayload {
    GatewayTypedPayload {
        owner_module_id: payload.owner.as_str().to_owned(),
        schema_id: payload.schema_id.as_str().to_owned(),
        schema_version: payload.schema_version.as_str().to_owned(),
        descriptor_hash: payload.descriptor_hash.to_vec(),
        data_class: "confidential".to_owned(),
        encoding: "protobuf".to_owned(),
        maximum_size_bytes: payload.maximum_size_bytes,
        retention_policy_id: payload.retention_policy_id.as_str().to_owned(),
        payload: payload.bytes,
    }
}

fn actor_owner() -> core::ActorOrTeamOwner {
    core::ActorOrTeamOwner {
        owner: Some(core::actor_or_team_owner::Owner::ActorId(ACTOR.to_owned())),
    }
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

async fn task_count(admin: &PgPool) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.records WHERE tenant_id = $1 AND record_type = 'activities.task' AND deleted_at IS NULL",
    )
    .bind(TENANT)
    .fetch_one(admin)
    .await
    .unwrap()
}

async fn deal_timeline_entry_count(admin: &PgPool) -> i64 {
    sqlx::query_scalar::<_, i64>(
        r#"
        SELECT count(*)
        FROM crm.projection_documents
        WHERE tenant_id = $1
          AND projection_id = $2
          AND resource_type = $3
          AND document ->> 'deal_id' = $4
        "#,
    )
    .bind(TENANT)
    .bind(DEAL_TIMELINE_PROJECTION_ID)
    .bind(DEAL_TIMELINE_RESOURCE_TYPE)
    .bind(DEAL_ID)
    .fetch_one(admin)
    .await
    .unwrap()
}

async fn task_status_document_count(admin: &PgPool) -> i64 {
    sqlx::query_scalar::<_, i64>(
        r#"
        SELECT count(*)
        FROM crm.projection_documents
        WHERE tenant_id = $1
          AND projection_id = $2
          AND resource_type = $3
        "#,
    )
    .bind(TENANT)
    .bind(TASK_STATUS_PROJECTION_ID)
    .bind(TASK_STATUS_RESOURCE_TYPE)
    .fetch_one(admin)
    .await
    .unwrap()
}

async fn provision_link_module(admin: &PgPool) {
    sqlx::query(
        r#"
        INSERT INTO crm.module_versions (
          module_id, version, canonicalization_profile, manifest_sha256,
          normalized_manifest_json, published_at, publisher_id
        )
        VALUES ($1, '1.0.0', 'crm.cjson/v1', $2, '{}'::jsonb, clock_timestamp(), 'phase6l-test')
        ON CONFLICT (module_id, version) DO NOTHING
        "#,
    )
    .bind(LINK_MODULE_ID)
    .bind(vec![0x6c_u8; 32])
    .execute(admin)
    .await
    .unwrap();

    let mut transaction = admin.begin().await.unwrap();
    sqlx::query(
        r#"
        SELECT
          set_config('app.tenant_id', $1, true),
          set_config('app.actor_id', $2, true),
          set_config('app.request_id', 'phase6l-link-fixture', true),
          set_config('app.capability_id', 'test.record.mutate', true),
          set_config('app.capability_version', '1.0.0', true),
          set_config('app.business_transaction_id', 'tx-bootstrap-a', true)
        "#,
    )
    .bind(TENANT)
    .bind(ACTOR)
    .execute(&mut *transaction)
    .await
    .unwrap();

    sqlx::query(
        r#"
        INSERT INTO crm.module_installations (
          tenant_id, install_id, module_id, current_version, status,
          generation, grant_set_digest, last_business_transaction_id
        )
        VALUES ($1, 'phase6l-link-installation', $2, '1.0.0', 'active', 1, $3, 'tx-bootstrap-a')
        ON CONFLICT (tenant_id, module_id)
        DO UPDATE SET
          status = 'active',
          generation = crm.module_installations.generation + 1,
          last_business_transaction_id = EXCLUDED.last_business_transaction_id,
          updated_at = clock_timestamp()
        "#,
    )
    .bind(TENANT)
    .bind(LINK_MODULE_ID)
    .bind(vec![0x6d_u8; 32])
    .execute(&mut *transaction)
    .await
    .unwrap();

    transaction.commit().await.unwrap();
}
