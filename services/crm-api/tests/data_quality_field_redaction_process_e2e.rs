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
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::process::{Child, Command};
use tokio::time::sleep;
use tonic::{Code, Request, Status};

const TENANT_A: &str = "tenant-a";
const TENANT_B: &str = "tenant-b";
const ACTOR: &str = "actor-a";
const TOKEN: &str = "data-quality-redaction-token-0123456789abcdef0123456789abcdef";
const APPROVAL_KEY: &str = "data-quality-redaction-approval-key-0123456789abcdef";
const PUBLISH_RULE_SET: &str = "data_quality.party.rule_set.publish";
const PUBLISH_PROFILE: &str = "data_quality.party.completeness_profile.publish";
const GET_RULE_SET: &str = "data_quality.party.rule_set.get";
const GET_PROFILE: &str = "data_quality.party.completeness_profile.get";
const HIDDEN_FIELDS: &str = "data_quality.party.rule_set.get|crm.data-quality|data_quality.party_rule_set_version|definition,data_quality.party.completeness_profile.get|crm.data-quality|data_quality.party_rule_set_version|definition";

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn crm_api_restart_applies_live_data_quality_field_redaction() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping Data Quality field-redaction process proof because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect Data Quality redaction fixture publisher");
    admin
        .execute(sqlx::raw_sql(include_str!(
            "../../../database/tests/0014_data_quality_adapter.sql"
        )))
        .await
        .expect("publish Data Quality rule-set registry fixture");
    admin
        .execute(sqlx::raw_sql(include_str!(
            "../../../database/tests/0015_data_quality_completeness_profile_adapter.sql"
        )))
        .await
        .expect("publish Data Quality completeness-profile registry fixture");

    let publish_rule_set_definition = mutation_definition(PUBLISH_RULE_SET);
    let publish_profile_definition = mutation_definition(PUBLISH_PROFILE);
    let get_rule_set_definition = query_definition(GET_RULE_SET);
    let get_profile_definition = query_definition(GET_PROFILE);

    let (http_addr, grpc_addr) = free_addresses();
    let http = reqwest::Client::new();
    let mut visible_child = spawn_crm_api(&database_url, &http_addr, &grpc_addr, None);
    wait_until_ready(&http, &mut visible_child, &http_addr).await;
    let mut visible_grpc = connect_grpc(&grpc_addr).await;

    let rule_set = publish_rule_set(
        &mut visible_grpc,
        &publish_rule_set_definition,
        "display_name.redaction_process",
    )
    .await;
    let rule_set_ref = rule_set
        .rule_set_version
        .as_ref()
        .and_then(|version| version.rule_set_version_ref.clone())
        .expect("published redaction rule-set ref");
    let profile = publish_profile(
        &mut visible_grpc,
        &publish_profile_definition,
        rule_set_ref.clone(),
    )
    .await;
    let profile_ref = profile
        .completeness_profile_version
        .as_ref()
        .and_then(|version| version.completeness_profile_version_ref.clone())
        .expect("published redaction profile ref");

    let visible_rule_set = get_rule_set(
        &mut visible_grpc,
        &get_rule_set_definition,
        rule_set_ref.clone(),
        TENANT_A,
    )
    .await
    .expect("query visible rule-set definition before restart");
    assert!(
        visible_rule_set
            .rule_set_version
            .as_ref()
            .and_then(|version| version.definition.as_ref())
            .is_some()
    );
    let visible_profile = get_profile(
        &mut visible_grpc,
        &get_profile_definition,
        profile_ref.clone(),
        TENANT_A,
    )
    .await
    .expect("query visible profile definition before restart");
    assert!(
        visible_profile
            .completeness_profile_version
            .as_ref()
            .and_then(|version| version.definition.as_ref())
            .is_some()
    );

    stop(&mut visible_child).await;
    drop(visible_grpc);

    let (redacted_http_addr, redacted_grpc_addr) = free_addresses();
    let mut redacted_child = spawn_crm_api(
        &database_url,
        &redacted_http_addr,
        &redacted_grpc_addr,
        Some(HIDDEN_FIELDS),
    );
    wait_until_ready(&http, &mut redacted_child, &redacted_http_addr).await;
    let mut redacted_grpc = connect_grpc(&redacted_grpc_addr).await;

    let redacted_rule_set = get_rule_set(
        &mut redacted_grpc,
        &get_rule_set_definition,
        rule_set_ref.clone(),
        TENANT_A,
    )
    .await
    .expect("query resource-visible redacted rule set after restart");
    let redacted_rule_set_version = redacted_rule_set
        .rule_set_version
        .expect("redacted rule-set resource remains visible");
    assert_eq!(
        redacted_rule_set_version.rule_set_version_ref.as_ref(),
        Some(&rule_set_ref)
    );
    assert!(redacted_rule_set_version.definition.is_none());

    let redacted_profile = get_profile(
        &mut redacted_grpc,
        &get_profile_definition,
        profile_ref.clone(),
        TENANT_A,
    )
    .await
    .expect("query resource-visible redacted profile after restart");
    let redacted_profile_version = redacted_profile
        .completeness_profile_version
        .expect("redacted completeness-profile resource remains visible");
    assert_eq!(
        redacted_profile_version
            .completeness_profile_version_ref
            .as_ref(),
        Some(&profile_ref)
    );
    assert!(redacted_profile_version.definition.is_none());

    let cross_tenant = get_profile(
        &mut redacted_grpc,
        &get_profile_definition,
        profile_ref,
        TENANT_B,
    )
    .await
    .expect_err("redaction policy must not weaken tenant nondisclosure");
    let missing = get_profile(
        &mut redacted_grpc,
        &get_profile_definition,
        data_quality::PartyCompletenessProfileVersionRef {
            completeness_profile_version_id: "dq-party-completeness-profile-missing".to_owned(),
        },
        TENANT_B,
    )
    .await
    .expect_err("missing profile must remain nondisclosing");
    assert_eq!(cross_tenant.code(), Code::NotFound);
    assert_eq!(missing.code(), Code::NotFound);
    assert_eq!(cross_tenant.message(), missing.message());

    stop(&mut redacted_child).await;
}

async fn publish_rule_set(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: &CapabilityDefinition,
    rule_key: &str,
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
                        rule_key: rule_key.to_owned(),
                        severity: data_quality::QualitySeverity::Warning as i32,
                        evaluator: Some(
                            data_quality::party_quality_rule::Evaluator::DisplayNameMinUtf8Bytes(
                                data_quality::PartyDisplayNameMinUtf8BytesEvaluator {
                                    minimum_utf8_bytes: 11,
                                },
                            ),
                        ),
                        title: "Redaction process display name threshold".to_owned(),
                        remediation_guidance: "Use a meaningful customer display name.".to_owned(),
                    }],
                }),
            },
        ),
        &unique_id("data-quality-redaction-rule-set"),
    )
    .await
    .expect("publish redaction process rule set");
    data_quality::PublishPartyRuleSetVersionResponse::decode(
        response
            .output
            .expect("redaction rule-set publication output")
            .payload
            .as_slice(),
    )
    .expect("decode redaction rule-set publication")
}

async fn publish_profile(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: &CapabilityDefinition,
    rule_set_ref: data_quality::PartyRuleSetVersionRef,
) -> data_quality::PublishPartyCompletenessProfileVersionResponse {
    let response = mutate(
        client,
        definition,
        payload(
            definition,
            data_quality::PublishPartyCompletenessProfileVersionRequest {
                definition: Some(data_quality::PartyCompletenessProfileDefinition {
                    completeness_semantic_version:
                        data_quality::PartyCompletenessSemanticVersion::V1 as i32,
                    rule_set_version_ref: Some(rule_set_ref),
                    components: vec![data_quality::PartyCompletenessComponent {
                        component_key: "name.minimum".to_owned(),
                        rule_key: "display_name.redaction_process".to_owned(),
                        weight_basis_points: 10_000,
                    }],
                }),
            },
        ),
        &unique_id("data-quality-redaction-profile"),
    )
    .await
    .expect("publish redaction process profile");
    data_quality::PublishPartyCompletenessProfileVersionResponse::decode(
        response
            .output
            .expect("redaction profile publication output")
            .payload
            .as_slice(),
    )
    .expect("decode redaction profile publication")
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
        response.output.expect("rule-set query output").payload.as_slice(),
    )
    .map_err(|error| Status::internal(format!("decode rule-set response: {error}")))
}

async fn get_profile(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: &CapabilityDefinition,
    version_ref: data_quality::PartyCompletenessProfileVersionRef,
    tenant_id: &str,
) -> Result<data_quality::GetPartyCompletenessProfileVersionResponse, Status> {
    let response = query(
        client,
        definition,
        payload(
            definition,
            data_quality::GetPartyCompletenessProfileVersionRequest {
                completeness_profile_version_ref: Some(version_ref),
            },
        ),
        tenant_id,
    )
    .await?;
    data_quality::GetPartyCompletenessProfileVersionResponse::decode(
        response.output.expect("profile query output").payload.as_slice(),
    )
    .map_err(|error| Status::internal(format!("decode profile response: {error}")))
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
        .expect("valid mutation definitions")
        .into_iter()
        .find(|definition| definition.capability_id.as_str() == capability_id)
        .unwrap_or_else(|| panic!("missing mutation definition: {capability_id}"))
}

fn query_definition(capability_id: &str) -> CapabilityDefinition {
    application_query_definitions()
        .expect("valid query definitions")
        .into_iter()
        .find(|definition| definition.capability_id.as_str() == capability_id)
        .unwrap_or_else(|| panic!("missing query definition: {capability_id}"))
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

fn spawn_crm_api(
    database_url: &str,
    http_addr: &str,
    grpc_addr: &str,
    hidden_fields: Option<&str>,
) -> Child {
    let mut command = Command::new(env!("CARGO_BIN_EXE_crm-api"));
    command
        .env("CRM_DATABASE_URL", database_url)
        .env("CRM_HTTP_BIND", http_addr)
        .env("CRM_GRPC_BIND", grpc_addr)
        .env("CRM_API_BEARER_TOKEN", TOKEN)
        .env("CRM_API_ACTOR_ID", ACTOR)
        .env("CRM_API_TENANTS", format!("{TENANT_A},{TENANT_B}"))
        .env(
            "CRM_CURSOR_SIGNING_KEY",
            "data-quality-redaction-cursor-key-0123456789abcdef",
        )
        .env("CRM_APPROVAL_SIGNING_KEY", APPROVAL_KEY)
        .env("CRM_BOOTSTRAP_ALLOW_PHASE6", "true")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);
    if let Some(hidden_fields) = hidden_fields {
        command.env("CRM_QUERY_HIDDEN_FIELDS", hidden_fields);
    }
    command
        .spawn()
        .expect("spawn crm-api for Data Quality redaction proof")
}

async fn wait_until_ready(client: &reqwest::Client, child: &mut Child, http_addr: &str) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child.try_wait().expect("poll redaction crm-api") {
            panic!("crm-api exited before redaction readiness: {status}");
        }
        if let Ok(response) = client
            .get(format!("http://{http_addr}/readyz"))
            .send()
            .await
            && response.status().is_success()
        {
            return;
        }
        assert!(Instant::now() < deadline, "redaction readiness timed out");
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
                    "redaction gRPC listener timed out: {error}"
                );
                sleep(Duration::from_millis(200)).await;
            }
        }
    }
}

async fn stop(child: &mut Child) {
    let pid = child.id().expect("running redaction crm-api has a PID");
    let status = Command::new("kill")
        .arg("-INT")
        .arg(pid.to_string())
        .status()
        .await
        .expect("send SIGINT to redaction crm-api");
    assert!(status.success(), "kill -INT failed: {status}");
    let status = child.wait().await.expect("wait for redaction crm-api");
    assert!(status.success(), "redaction crm-api exited unsuccessfully: {status}");
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
        .expect("bind ephemeral redaction test port")
        .local_addr()
        .expect("read ephemeral redaction test address")
        .port()
}

fn unique_id(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after epoch")
        .as_nanos();
    format!("{prefix}-{nanos}")
}
