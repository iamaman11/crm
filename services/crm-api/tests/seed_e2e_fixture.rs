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
    core::v1 as core, customer::v1 as customer, customer_privacy::v1 as privacy,
    parties::v1 as parties, sales::v1 as sales, search::v1 as search,
};
use crm_sales_activities_capability_composition::{
    DEAL_TIMELINE_PROJECTION_ID, DEAL_TIMELINE_RESOURCE_TYPE, TASK_STATUS_PROJECTION_ID,
    TASK_STATUS_RESOURCE_TYPE,
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
const SEARCH_GLOBAL: &str = "search.global.query";
const LINK_MODULE_ID: &str = "crm.sales-activities-link";
const PARTIES_MODULE_ID: &str = "crm.parties";
const PRIVACY_MODULE_ID: &str = "crm.customer-privacy";
const PARTY_CREATE: &str = "parties.party.create";
const PRIVACY_CREATE: &str = "customer_privacy.case.create";
const PRIVACY_SUBMIT: &str = "customer_privacy.case.submit";
const PRIVACY_VERIFY: &str = "customer_privacy.case.subject.verify";
const PRIVACY_LIST: &str = "customer_privacy.case.list";
const PRIVACY_PARTY_ID: &str = "privacy-product-plane-party";

struct ChildGuard {
    child: tokio::process::Child,
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Ok(None) = self.child.try_wait() {
            let _ = self.child.start_kill();
        }
    }
}

#[tokio::test]
async fn seed_e2e_fixture_records() {
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(val) => val,
        Err(_) => {
            println!("Skipping seed_e2e_fixture_records: DATABASE_URL not set");
            return;
        }
    };
    let admin_database_url = match std::env::var("ADMIN_DATABASE_URL") {
        Ok(val) => val,
        Err(_) => {
            println!("Skipping seed_e2e_fixture_records: ADMIN_DATABASE_URL not set");
            return;
        }
    };
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect admin database");

    provision_link_module(&admin).await;
    provision_module(
        &admin,
        PARTIES_MODULE_ID,
        "0.4.0",
        "phase20a-parties-installation",
        0x70,
    )
    .await;
    provision_module(
        &admin,
        PRIVACY_MODULE_ID,
        "0.3.0",
        "phase20a-privacy-installation",
        0x71,
    )
    .await;

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
        .env(
            "CRM_CURSOR_SIGNING_KEY",
            "phase6l-process-cursor-signing-key-0123456789abcdef",
        )
        .env(
            "CRM_APPROVAL_SIGNING_KEY",
            "phase6l-process-approval-signing-key-0123456789abcdef",
        )
        .spawn()
        .expect("spawn crm-api process");

    let mut child_guard = ChildGuard { child };

    let http = reqwest::Client::new();
    wait_until_ready(&http, &child_guard.child, &http_addr).await;
    let mut grpc = connect_grpc(&grpc_addr).await;
    let privacy_case_id = seed_customer_privacy_fixture(&mut grpc).await;
    println!("Seeded Customer Privacy case {privacy_case_id} for {PRIVACY_PARTY_ID}");

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

    let search_definition = query_definition(SEARCH_GLOBAL);
    let search_payload = wire_payload(payload(
        &search_definition,
        search::SearchRequest {
            text: "Phase 6L process deal".to_owned(),
            resource_types: vec!["sales.deal".to_owned()],
            page_size: 25,
            cursor: String::new(),
        },
    ));
    wait_for_search_hit(
        &mut grpc,
        &search_definition,
        search_payload,
        DEAL_ID,
    )
    .await;

    println!("Stopping crm-api spawned for seeding...");
    let pid = child_guard.child.id().expect("running crm-api has a PID");
    let kill_status = Command::new("kill")
        .arg("-INT")
        .arg(pid.to_string())
        .status()
        .await
        .expect("send SIGINT to crm-api");
    assert!(kill_status.success(), "kill -INT failed");
    child_guard.child.wait().await.ok();

    println!("Seeding completed successfully!");
}

async fn seed_customer_privacy_fixture(
    grpc: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
) -> String {
    let party_definition = mutation_definition(PARTY_CREATE);
    grpc_mutation(
        grpc,
        &party_definition,
        payload(
            &party_definition,
            parties::CreatePartyRequest {
                party_ref: Some(customer::PartyRef {
                    party_id: PRIVACY_PARTY_ID.to_owned(),
                }),
                kind: parties::PartyKind::Person as i32,
                display_name: "Privacy product-plane fixture".to_owned(),
            },
        ),
        "phase20a-privacy-party-create",
    )
    .await;

    let create_definition = mutation_definition(PRIVACY_CREATE);
    let created = grpc_mutation(
        grpc,
        &create_definition,
        payload(
            &create_definition,
            privacy::CreatePrivacyCaseRequest {
                kind: privacy::PrivacyCaseKind::Erasure as i32,
                policy_version: "privacy-policy/1".to_owned(),
                previous_privacy_case_ref: None,
            },
        ),
        "phase20a-privacy-case-create",
    )
    .await;
    let case_id = privacy::CreatePrivacyCaseResponse::decode(
        created
            .output
            .as_ref()
            .expect("Customer Privacy create output")
            .payload
            .as_slice(),
    )
    .expect("decode Customer Privacy create output")
    .privacy_case
    .and_then(|privacy_case| privacy_case.privacy_case_ref)
    .expect("created Customer Privacy case reference")
    .privacy_case_id;

    let submit_definition = mutation_definition(PRIVACY_SUBMIT);
    grpc_mutation(
        grpc,
        &submit_definition,
        payload(
            &submit_definition,
            privacy::SubmitPrivacyCaseRequest {
                privacy_case_ref: Some(privacy::PrivacyCaseRef {
                    privacy_case_id: case_id.clone(),
                }),
                expected_version: 1,
            },
        ),
        "phase20a-privacy-case-submit",
    )
    .await;

    let verify_definition = mutation_definition(PRIVACY_VERIFY);
    grpc_mutation(
        grpc,
        &verify_definition,
        payload(
            &verify_definition,
            privacy::VerifyPrivacyCaseSubjectRequest {
                privacy_case_ref: Some(privacy::PrivacyCaseRef {
                    privacy_case_id: case_id.clone(),
                }),
                expected_version: 2,
                submitted_party_ref: Some(customer::PartyRef {
                    party_id: PRIVACY_PARTY_ID.to_owned(),
                }),
                canonical_party_ref: Some(customer::PartyRef {
                    party_id: PRIVACY_PARTY_ID.to_owned(),
                }),
                identity_resolution_generation: 1,
                verification_method: privacy::SubjectVerificationMethod::VerifiedDocument as i32,
            },
        ),
        "phase20a-privacy-case-verify",
    )
    .await;

    let list_definition = query_definition(PRIVACY_LIST);
    let mut request = Request::new(GatewayQueryRequest {
        owner_module_id: list_definition.owner_module_id.as_str().to_owned(),
        capability_id: list_definition.capability_id.as_str().to_owned(),
        capability_version: list_definition.capability_version.as_str().to_owned(),
        input: Some(wire_payload(payload(
            &list_definition,
            privacy::ListPrivacyCasesRequest {
                canonical_party_ref: Some(customer::PartyRef {
                    party_id: PRIVACY_PARTY_ID.to_owned(),
                }),
                page_size: 25,
                cursor: String::new(),
                kind: None,
                status: None,
            },
        ))),
    });
    request
        .metadata_mut()
        .insert("authorization", format!("Bearer {TOKEN}").parse().unwrap());
    request
        .metadata_mut()
        .insert("x-tenant-id", TENANT.parse().unwrap());
    let response = grpc
        .query(request)
        .await
        .expect("list seeded Customer Privacy cases")
        .into_inner();
    let listed = privacy::ListPrivacyCasesResponse::decode(
        response
            .output
            .expect("Customer Privacy list output")
            .payload
            .as_slice(),
    )
    .expect("decode Customer Privacy list output");
    assert_eq!(listed.privacy_cases.len(), 1);
    let listed_case = &listed.privacy_cases[0];
    assert_eq!(
        listed_case
            .privacy_case_ref
            .as_ref()
            .expect("listed case reference")
            .privacy_case_id,
        case_id
    );
    assert_eq!(
        listed_case.status,
        privacy::PrivacyCaseStatus::SubjectVerified as i32
    );
    assert_eq!(listed_case.version, 3);
    case_id
}

async fn grpc_mutation(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: &CapabilityDefinition,
    input: TypedPayload,
    idempotency_key: &str,
) -> crm_application_runtime::gateway_v1::MutateResponse {
    let mut request = Request::new(GatewayMutateRequest {
        owner_module_id: definition.owner_module_id.as_str().to_owned(),
        capability_id: definition.capability_id.as_str().to_owned(),
        capability_version: definition.capability_version.as_str().to_owned(),
        input: Some(wire_payload(input)),
        approval: None,
    });
    request
        .metadata_mut()
        .insert("authorization", format!("Bearer {TOKEN}").parse().unwrap());
    request
        .metadata_mut()
        .insert("x-tenant-id", TENANT.parse().unwrap());
    request
        .metadata_mut()
        .insert("idempotency-key", idempotency_key.parse().unwrap());
    client
        .mutate(request)
        .await
        .unwrap_or_else(|error| {
            panic!(
                "governed mutation {} failed: {error}",
                definition.capability_id
            )
        })
        .into_inner()
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

async fn wait_until_ready(
    client: &reqwest::Client,
    _child: &tokio::process::Child,
    http_addr: &str,
) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Ok(response) = client
            .get(format!("http://{http_addr}/readyz"))
            .send()
            .await
            && response.status() == StatusCode::OK
        {
            return;
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

async fn wait_for_search_hit(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: &CapabilityDefinition,
    input: GatewayTypedPayload,
    resource_id: &str,
) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let mut request = Request::new(GatewayQueryRequest {
            owner_module_id: definition.owner_module_id.as_str().to_owned(),
            capability_id: definition.capability_id.as_str().to_owned(),
            capability_version: definition.capability_version.as_str().to_owned(),
            input: Some(input.clone()),
        });
        request
            .metadata_mut()
            .insert("authorization", format!("Bearer {TOKEN}").parse().unwrap());
        request
            .metadata_mut()
            .insert("x-tenant-id", TENANT.parse().unwrap());
        let response = client
            .query(request)
            .await
            .expect("query search while waiting for projection")
            .into_inner();
        let output = response.output.expect("search output");
        let page = search::SearchResponse::decode(output.payload.as_slice())
            .expect("decode search response");
        if page.hits.iter().any(|hit| hit.resource_id == resource_id) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "search projection timeout for {resource_id}"
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
            Err(_) => {
                assert!(Instant::now() < deadline, "gRPC timeout");
                sleep(Duration::from_millis(200)).await;
            }
        }
    }
}

fn mutation_definition(capability_id: &str) -> CapabilityDefinition {
    application_mutation_definitions()
        .expect("valid production mutation definitions")
        .into_iter()
        .find(|definition| definition.capability_id.as_str() == capability_id)
        .unwrap_or_else(|| panic!("missing production mutation definition {capability_id}"))
}

fn query_definition(capability_id: &str) -> CapabilityDefinition {
    application_query_definitions()
        .expect("valid production query definitions")
        .into_iter()
        .find(|definition| definition.capability_id.as_str() == capability_id)
        .unwrap_or_else(|| panic!("missing production query definition {capability_id}"))
}

fn payload<M: Message>(definition: &CapabilityDefinition, message: M) -> TypedPayload {
    let data_class = *definition
        .input_contract
        .allowed_data_classes
        .first()
        .expect("input contract data class");
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
    payload.validate().unwrap();
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
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
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

async fn provision_module(
    admin: &PgPool,
    module_id: &str,
    version: &str,
    install_id: &str,
    digest_byte: u8,
) {
    sqlx::query(
        r#"
        INSERT INTO crm.module_versions (
          module_id, version, canonicalization_profile, manifest_sha256,
          normalized_manifest_json, published_at, publisher_id
        )
        VALUES ($1, $2, 'crm.cjson/v1', $3, '{}'::jsonb, clock_timestamp(), 'phase20a-test')
        ON CONFLICT (module_id, version) DO NOTHING
        "#,
    )
    .bind(module_id)
    .bind(version)
    .bind(vec![digest_byte; 32])
    .execute(admin)
    .await
    .unwrap();

    let mut transaction = admin.begin().await.unwrap();
    sqlx::query(
        r#"
        SELECT
          set_config('app.tenant_id', $1, true),
          set_config('app.actor_id', $2, true),
          set_config('app.request_id', 'phase20a-module-fixture', true),
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
        VALUES ($1, $2, $3, $4, 'active', 1, $5, 'tx-bootstrap-a')
        ON CONFLICT (tenant_id, module_id)
        DO UPDATE SET
          current_version = EXCLUDED.current_version,
          status = 'active',
          generation = crm.module_installations.generation + 1,
          grant_set_digest = EXCLUDED.grant_set_digest,
          last_business_transaction_id = EXCLUDED.last_business_transaction_id,
          updated_at = clock_timestamp()
        "#,
    )
    .bind(TENANT)
    .bind(install_id)
    .bind(module_id)
    .bind(version)
    .bind(vec![digest_byte.wrapping_add(1); 32])
    .execute(&mut *transaction)
    .await
    .unwrap();
    transaction.commit().await.unwrap();
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
