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
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::process::{Child, Command};
use tokio::time::{sleep, timeout};
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn crm_api_process_serves_http_grpc_workers_and_graceful_shutdown() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping Phase 6L process acceptance because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect Phase 6L admin evidence reader");
    provision_link_module(&admin).await;

    let baseline_tasks = task_count(&admin).await;
    let baseline_task_status_documents = task_status_document_count(&admin).await;
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
        .env("CRM_API_TENANTS", TENANT)
        .env(
            "CRM_CURSOR_SIGNING_KEY",
            "phase6l-cursor-signing-key-0123456789abcdef",
        )
        .env(
            "CRM_APPROVAL_SIGNING_KEY",
            "phase6l-approval-signing-key-0123456789abcdef",
        )
        .env("CRM_BOOTSTRAP_ALLOW_PHASE6", "true")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn production crm-api process");

    let http = reqwest::Client::new();
    wait_until_ready(&http, &mut child, &http_addr).await;

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

    let unauthorized = http
        .post(mutation_url(&http_addr, &create_definition))
        .header("x-tenant-id", TENANT)
        .header("idempotency-key", "phase6l-unauthorized-create")
        .json(&create_payload)
        .send()
        .await
        .expect("send unauthenticated process mutation");
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

    let created = authenticated_mutation(
        &http,
        &http_addr,
        &create_definition,
        &create_payload,
        "phase6l-create",
    )
    .await;
    assert_eq!(created, StatusCode::OK);

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

    let query_definition = query_definition(SALES_GET);
    let query_payload = payload(
        &query_definition,
        sales::GetDealRequest {
            deal_id: DEAL_ID.to_owned(),
        },
    );
    let mut grpc = connect_grpc(&grpc_addr).await;
    let mut request = Request::new(GatewayQueryRequest {
        owner_module_id: query_definition.owner_module_id.as_str().to_owned(),
        capability_id: query_definition.capability_id.as_str().to_owned(),
        capability_version: query_definition.capability_version.as_str().to_owned(),
        input: Some(wire_payload(query_payload)),
    });
    request.metadata_mut().insert(
        "authorization",
        format!("Bearer {TOKEN}")
            .parse()
            .expect("valid authorization metadata"),
    );
    request.metadata_mut().insert(
        "x-tenant-id",
        TENANT.parse().expect("valid tenant metadata"),
    );
    let response = grpc
        .query(request)
        .await
        .expect("query production crm-api over gRPC")
        .into_inner();
    let output = response.output.expect("gRPC query output payload");
    let deal = sales::GetDealResponse::decode(output.payload.as_slice())
        .expect("decode production Deal query response")
        .deal
        .expect("queried Deal exists");
    assert_eq!(deal.deal_id, DEAL_ID);
    assert_eq!(deal.name, "Phase 6L process deal");
    assert_eq!(
        deal.stage_details.expect("Deal stage details").stage_id,
        "proposal"
    );

    let search_definition = crate::query_definition(SEARCH_GLOBAL);
    let search_payload = wire_payload(payload(
        &search_definition,
        search::SearchRequest {
            text: "Phase 6L process deal".to_owned(),
            resource_types: vec!["sales.deal".to_owned()],
            page_size: 25,
            cursor: String::new(),
        },
    ));
    let search_deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let mut request = Request::new(GatewayQueryRequest {
            owner_module_id: search_definition.owner_module_id.as_str().to_owned(),
            capability_id: search_definition.capability_id.as_str().to_owned(),
            capability_version: search_definition.capability_version.as_str().to_owned(),
            input: Some(search_payload.clone()),
        });
        request.metadata_mut().insert(
            "authorization",
            format!("Bearer {TOKEN}")
                .parse()
                .expect("valid search authorization metadata"),
        );
        request.metadata_mut().insert(
            "x-tenant-id",
            TENANT.parse().expect("valid search tenant metadata"),
        );
        let response = grpc
            .query(request)
            .await
            .expect("query governed search through production gRPC gateway")
            .into_inner();
        let output = response.output.expect("gRPC search output payload");
        let page = search::SearchResponse::decode(output.payload.as_slice())
            .expect("decode production search response");
        if let Some(hit) = page.hits.iter().find(|hit| hit.resource_id == DEAL_ID) {
            assert_eq!(hit.owner_module_id, "crm.sales");
            assert_eq!(hit.resource_type, "sales.deal");
            assert_eq!(hit.fields.len(), 1);
            assert_eq!(
                hit.fields.get("name").map(String::as_str),
                Some("Phase 6L process deal")
            );
            assert_eq!(hit.matched_fields, vec!["name"]);
            break;
        }
        assert!(
            Instant::now() < search_deadline,
            "production search did not expose the indexed Deal before the acceptance deadline"
        );
        sleep(Duration::from_millis(250)).await;
    }

    send_sigint(&child).await;
    let exit = timeout(Duration::from_secs(15), child.wait())
        .await
        .expect("crm-api must stop within graceful-shutdown budget")
        .expect("wait for crm-api process");
    assert!(exit.success(), "crm-api exited unsuccessfully: {exit}");
}

async fn authenticated_mutation(
    client: &reqwest::Client,
    http_addr: &str,
    definition: &CapabilityDefinition,
    input: &TypedPayload,
    idempotency_key: &str,
) -> StatusCode {
    client
        .post(mutation_url(http_addr, definition))
        .bearer_auth(TOKEN)
        .header("x-tenant-id", TENANT)
        .header("idempotency-key", idempotency_key)
        .json(input)
        .send()
        .await
        .expect("send authenticated process mutation")
        .status()
}

fn mutation_url(http_addr: &str, definition: &CapabilityDefinition) -> String {
    format!(
        "http://{http_addr}/v1/mutations/{}/{}/{}",
        definition.owner_module_id, definition.capability_id, definition.capability_version
    )
}

async fn wait_until_ready(client: &reqwest::Client, child: &mut Child, http_addr: &str) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child.try_wait().expect("poll crm-api process") {
            panic!("crm-api exited before readiness: {status}");
        }
        if let Ok(response) = client
            .get(format!("http://{http_addr}/readyz"))
            .send()
            .await
            && response.status() == StatusCode::OK
        {
            return;
        }
        assert!(Instant::now() < deadline, "crm-api readiness timed out");
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
        assert!(
            Instant::now() < deadline,
            "background runtime did not materialize link/projections: tasks={tasks}, timeline={timeline_entries}, task_status={task_status_documents}"
        );
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
            Err(error) => {
                assert!(
                    Instant::now() < deadline,
                    "gRPC listener timed out: {error}"
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
        .expect("send SIGINT to crm-api");
    assert!(status.success(), "kill -INT failed: {status}");
}

fn mutation_definition(capability_id: &str) -> CapabilityDefinition {
    capability_definitions()
        .expect("valid production mutation definitions")
        .into_iter()
        .find(|definition| definition.capability_id.as_str() == capability_id)
        .unwrap_or_else(|| panic!("missing mutation definition: {capability_id}"))
}

fn query_definition(capability_id: &str) -> CapabilityDefinition {
    query_capability_definitions()
        .expect("valid production query definitions")
        .into_iter()
        .find(|definition| definition.capability_id.as_str() == capability_id)
        .unwrap_or_else(|| panic!("missing query definition: {capability_id}"))
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

fn actor_owner() -> core::ActorOrTeamOwner {
    core::ActorOrTeamOwner {
        owner: Some(core::actor_or_team_owner::Owner::ActorId(ACTOR.to_owned())),
    }
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral test port")
        .local_addr()
        .expect("read ephemeral test port")
        .port()
}

async fn task_count(admin: &PgPool) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.records WHERE tenant_id = $1 AND record_type = 'activities.task' AND deleted_at IS NULL",
    )
    .bind(TENANT)
    .fetch_one(admin)
    .await
    .expect("count Activities tasks")
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
    .expect("count Deal timeline projection entries")
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
    .expect("count Task status projection documents")
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
    .expect("provision link module version");

    let mut transaction = admin
        .begin()
        .await
        .expect("begin link installation fixture");
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
    .expect("bind link installation fixture context");
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
    .expect("activate link module installation");
    transaction
        .commit()
        .await
        .expect("commit link module installation");
}
