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
use crm_proto_contracts::crm::data_quality::v1 as data_quality;
use prost::Message;
use sqlx::{Executor, PgPool};
use std::net::TcpListener;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::process::{Child, Command};
use tokio::time::sleep;
use tonic::{Code, Request, Status};

const TENANT: &str = "tenant-a";
const ACTOR: &str = "actor-a";
const TOKEN: &str = "data-quality-query-bearer-token-0123456789abcdef0123456789abcdef";
const APPROVAL_KEY: &str = "data-quality-query-approval-key-0123456789abcdef";
const PUBLISH_RULE_SET: &str = "data_quality.party.rule_set.publish";
const GET_RULE_SET: &str = "data_quality.party.rule_set.get";

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn crm_api_process_discloses_only_authorized_party_rule_set_versions() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping data-quality query process acceptance because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect Data Quality query process fixture publisher");
    admin
        .execute(sqlx::raw_sql(include_str!(
            "../../../database/tests/0014_data_quality_adapter.sql"
        )))
        .await
        .expect("publish Data Quality adapter registry fixture");

    let http_port = free_port();
    let mut grpc_port = free_port();
    while grpc_port == http_port {
        grpc_port = free_port();
    }
    let http_addr = format!("127.0.0.1:{http_port}");
    let grpc_addr = format!("127.0.0.1:{grpc_port}");
    let http = reqwest::Client::new();

    let publish_definition = mutation_definition(PUBLISH_RULE_SET);
    let get_definition = query_definition(GET_RULE_SET);
    let mut child = spawn_crm_api(&database_url, &http_addr, &grpc_addr);
    wait_until_ready(&http, &mut child, &http_addr).await;
    let mut grpc = connect_grpc(&grpc_addr).await;

    let published = publish_rule_set(&mut grpc, &publish_definition).await;
    let version_ref = published
        .rule_set_version
        .as_ref()
        .and_then(|version| version.rule_set_version_ref.clone())
        .expect("published Party rule-set version reference");

    let disclosed = get_rule_set(&mut grpc, &get_definition, version_ref.clone(), TENANT)
        .await
        .expect("query Party rule-set through authorized production gateway");
    let disclosed_version = disclosed
        .rule_set_version
        .expect("disclosed Party rule-set version");
    assert_eq!(
        disclosed_version.rule_set_version_ref.as_ref(),
        Some(&version_ref)
    );
    let definition = disclosed_version
        .definition
        .expect("bootstrap visibility discloses rule-set definition");
    assert_eq!(definition.rules.len(), 1);
    assert_eq!(definition.rules[0].rule_key, "display_name.query_process_minimum");

    let cross_tenant = get_rule_set(&mut grpc, &get_definition, version_ref, "tenant-b")
        .await
        .expect_err("cross-tenant Data Quality query must be rejected");
    assert_eq!(cross_tenant.code(), Code::PermissionDenied);

    let unavailable = get_rule_set(
        &mut grpc,
        &get_definition,
        data_quality::PartyRuleSetVersionRef {
            rule_set_version_id: "dq-party-rule-set-missing".to_owned(),
        },
        TENANT,
    )
    .await
    .expect_err("unavailable Data Quality version must fail closed");
    assert_eq!(unavailable.code(), Code::NotFound);

    send_sigint(&child).await;
    let status = child
        .wait()
        .await
        .expect("wait for Data Quality query acceptance crm-api");
    assert!(status.success(), "crm-api exited unsuccessfully: {status}");
}

async fn publish_rule_set(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: &CapabilityDefinition,
) -> data_quality::PublishPartyRuleSetVersionResponse {
    let response = mutate(
        client,
        definition,
        payload(
            definition,
            data_quality::PublishPartyRuleSetVersionRequest {
                definition: Some(data_quality::PartyRuleSetDefinition {
                    evaluator_semantic_version:
                        data_quality::PartyQualityEvaluatorSemanticVersion::V1 as i32,
                    rules: vec![data_quality::PartyQualityRule {
                        rule_key: "display_name.query_process_minimum".to_owned(),
                        severity: data_quality::QualitySeverity::Warning as i32,
                        evaluator: Some(
                            data_quality::party_quality_rule::Evaluator::DisplayNameMinUtf8Bytes(
                                data_quality::PartyDisplayNameMinUtf8BytesEvaluator {
                                    minimum_utf8_bytes: 17,
                                },
                            ),
                        ),
                        title: "Query process display name threshold".to_owned(),
                        remediation_guidance: "Use a meaningful customer display name.".to_owned(),
                    }],
                }),
            },
        ),
    )
    .await
    .expect("publish query acceptance Party rule-set");
    data_quality::PublishPartyRuleSetVersionResponse::decode(
        response
            .output
            .expect("Party rule-set publication output")
            .payload
            .as_slice(),
    )
    .expect("decode Party rule-set publication response")
}

async fn get_rule_set(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: &CapabilityDefinition,
    version_ref: data_quality::PartyRuleSetVersionRef,
    tenant_id: &str,
) -> Result<data_quality::GetPartyRuleSetVersionResponse, Status> {
    let response = query(
        client,
        definition,
        payload(
            definition,
            data_quality::GetPartyRuleSetVersionRequest {
                rule_set_version_ref: Some(version_ref),
            },
        ),
        tenant_id,
    )
    .await?;
    data_quality::GetPartyRuleSetVersionResponse::decode(
        response
            .output
            .expect("Party rule-set query output")
            .payload
            .as_slice(),
    )
    .map_err(|error| Status::internal(format!("decode Party rule-set query response: {error}")))
}

async fn mutate(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: &CapabilityDefinition,
    input: TypedPayload,
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
        .insert("x-tenant-id", TENANT.parse().unwrap());
    request
        .metadata_mut()
        .insert("idempotency-key", "data-quality-query-process-publish".parse().unwrap());
    request
        .metadata_mut()
        .insert("authorization", format!("Bearer {TOKEN}").parse().unwrap());
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
) -> Result<crm_application_runtime::gateway_v1::QueryResponse, Status> {
    let mut request = Request::new(GatewayQueryRequest {
        owner_module_id: definition.owner_module_id.as_str().to_owned(),
        capability_id: definition.capability_id.as_str().to_owned(),
        capability_version: definition.capability_version.as_str().to_owned(),
        input: Some(wire_payload(input)),
    });
    request
        .metadata_mut()
        .insert("x-tenant-id", tenant_id.parse().unwrap());
    request
        .metadata_mut()
        .insert("authorization", format!("Bearer {TOKEN}").parse().unwrap());
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
    let payload = TypedPayload {
        owner: definition.input_contract.owner.clone(),
        schema_id: definition.input_contract.schema_id.clone(),
        schema_version: definition.input_contract.schema_version.clone(),
        descriptor_hash: definition.input_contract.descriptor_hash,
        data_class: *definition
            .input_contract
            .allowed_data_classes
            .first()
            .expect("capability input data class"),
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
        .env("CRM_API_ACTOR_ID", ACTOR)
        .env("CRM_API_TENANTS", TENANT)
        .env(
            "CRM_CURSOR_SIGNING_KEY",
            "data-quality-query-cursor-key-0123456789abcdef",
        )
        .env("CRM_APPROVAL_SIGNING_KEY", APPROVAL_KEY)
        .env("CRM_BOOTSTRAP_ALLOW_PHASE6", "true")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn production crm-api process for Data Quality query acceptance")
}

async fn wait_until_ready(client: &reqwest::Client, child: &mut Child, http_addr: &str) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child
            .try_wait()
            .expect("poll Data Quality query acceptance crm-api")
        {
            panic!("crm-api exited before Data Quality query acceptance readiness: {status}");
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
            "Data Quality query acceptance crm-api readiness timed out"
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
                    "Data Quality query acceptance gRPC listener timed out: {error}"
                );
                sleep(Duration::from_millis(200)).await;
            }
        }
    }
}

async fn send_sigint(child: &Child) {
    let pid = child
        .id()
        .expect("running Data Quality query acceptance crm-api has a PID");
    let status = Command::new("kill")
        .arg("-INT")
        .arg(pid.to_string())
        .status()
        .await
        .expect("send SIGINT to Data Quality query acceptance crm-api");
    assert!(status.success(), "kill -INT failed: {status}");
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral Data Quality query acceptance port")
        .local_addr()
        .expect("read ephemeral Data Quality query acceptance port")
        .port()
}
