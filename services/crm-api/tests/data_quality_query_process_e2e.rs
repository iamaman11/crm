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
const TOKEN: &str = "data-quality-query-bearer-token-0123456789abcdef0123456789abcdef";
const APPROVAL_KEY: &str = "data-quality-query-approval-key-0123456789abcdef";
const PUBLISH_RULE_SET: &str = "data_quality.party.rule_set.publish";
const PUBLISH_PROFILE: &str = "data_quality.party.completeness_profile.publish";
const GET_RULE_SET: &str = "data_quality.party.rule_set.get";
const GET_PROFILE: &str = "data_quality.party.completeness_profile.get";
const MODULE_ID: &str = "crm.data-quality";
const PROFILE_RECORD_TYPE: &str = "data_quality.party_completeness_profile_version";

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn crm_api_process_discloses_only_tenant_bound_data_quality_definitions() {
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
        .expect("publish Data Quality rule-set adapter registry fixture");
    admin
        .execute(sqlx::raw_sql(include_str!(
            "../../../database/tests/0015_data_quality_completeness_profile_adapter.sql"
        )))
        .await
        .expect("publish Data Quality completeness-profile adapter registry fixture");

    let http_port = free_port();
    let mut grpc_port = free_port();
    while grpc_port == http_port {
        grpc_port = free_port();
    }
    let http_addr = format!("127.0.0.1:{http_port}");
    let grpc_addr = format!("127.0.0.1:{grpc_port}");
    let http = reqwest::Client::new();

    let publish_rule_set_definition = mutation_definition(PUBLISH_RULE_SET);
    let publish_profile_definition = mutation_definition(PUBLISH_PROFILE);
    let get_rule_set_definition = query_definition(GET_RULE_SET);
    let get_profile_definition = query_definition(GET_PROFILE);
    let mut child = spawn_crm_api(&database_url, &http_addr, &grpc_addr);
    wait_until_ready(&http, &mut child, &http_addr).await;
    let mut grpc = connect_grpc(&grpc_addr).await;

    let tenant_a_rule_set = publish_rule_set(
        &mut grpc,
        &publish_rule_set_definition,
        TENANT_A,
        "display_name.query_process_a",
        17,
    )
    .await;
    let tenant_a_rule_set_ref = rule_set_ref(&tenant_a_rule_set);
    let disclosed_rule_set = get_rule_set(
        &mut grpc,
        &get_rule_set_definition,
        tenant_a_rule_set_ref.clone(),
        TENANT_A,
    )
    .await
    .expect("query tenant A Party rule-set through authorized production gateway");
    assert_disclosed_rule_set(
        &disclosed_rule_set,
        &tenant_a_rule_set_ref,
        "display_name.query_process_a",
    );

    let tenant_a_profile = publish_profile(
        &mut grpc,
        &publish_profile_definition,
        TENANT_A,
        tenant_a_rule_set_ref,
        "display_name.query_process_a",
    )
    .await;
    let tenant_a_profile_ref = profile_ref(&tenant_a_profile);
    let disclosed_profile = get_profile(
        &mut grpc,
        &get_profile_definition,
        tenant_a_profile_ref.clone(),
        TENANT_A,
    )
    .await
    .expect("query tenant A completeness profile through authorized production gateway");
    assert_disclosed_profile(
        &disclosed_profile,
        &tenant_a_profile_ref,
        "display_name.query_process_a",
    );

    let tenant_b_rule_set = publish_rule_set(
        &mut grpc,
        &publish_rule_set_definition,
        TENANT_B,
        "display_name.query_process_b",
        19,
    )
    .await;
    let tenant_b_rule_set_ref = rule_set_ref(&tenant_b_rule_set);
    let tenant_b_profile = publish_profile(
        &mut grpc,
        &publish_profile_definition,
        TENANT_B,
        tenant_b_rule_set_ref.clone(),
        "display_name.query_process_b",
    )
    .await;
    let tenant_b_profile_ref = profile_ref(&tenant_b_profile);

    assert_force_rls_profile_boundary(
        &admin,
        &database_url,
        &tenant_b_profile_ref.completeness_profile_version_id,
    )
    .await;

    let disclosed_b = get_profile(
        &mut grpc,
        &get_profile_definition,
        tenant_b_profile_ref.clone(),
        TENANT_B,
    )
    .await
    .expect("query tenant B completeness profile through production gateway");
    assert_disclosed_profile(
        &disclosed_b,
        &tenant_b_profile_ref,
        "display_name.query_process_b",
    );

    let cross_tenant_rule_set = get_rule_set(
        &mut grpc,
        &get_rule_set_definition,
        tenant_b_rule_set_ref,
        TENANT_A,
    )
    .await
    .expect_err("tenant-authorized actor must not read another tenant's rule-set record");
    assert_eq!(cross_tenant_rule_set.code(), Code::NotFound);

    let cross_tenant_profile = get_profile(
        &mut grpc,
        &get_profile_definition,
        tenant_b_profile_ref,
        TENANT_A,
    )
    .await
    .expect_err("tenant-authorized actor must not read another tenant's profile record");
    assert_eq!(cross_tenant_profile.code(), Code::NotFound);

    let unavailable_rule_set = get_rule_set(
        &mut grpc,
        &get_rule_set_definition,
        data_quality::PartyRuleSetVersionRef {
            rule_set_version_id: "dq-party-rule-set-missing".to_owned(),
        },
        TENANT_A,
    )
    .await
    .expect_err("unavailable Data Quality rule-set version must fail closed");
    assert_eq!(unavailable_rule_set.code(), Code::NotFound);
    assert_eq!(
        cross_tenant_rule_set.message(),
        unavailable_rule_set.message()
    );

    let unavailable_profile = get_profile(
        &mut grpc,
        &get_profile_definition,
        data_quality::PartyCompletenessProfileVersionRef {
            completeness_profile_version_id: "dq-party-completeness-profile-missing".to_owned(),
        },
        TENANT_A,
    )
    .await
    .expect_err("unavailable completeness-profile version must fail closed");
    assert_eq!(unavailable_profile.code(), Code::NotFound);
    assert_eq!(
        cross_tenant_profile.message(),
        unavailable_profile.message()
    );

    send_sigint(&child).await;
    let status = child
        .wait()
        .await
        .expect("wait for Data Quality query acceptance crm-api");
    assert!(status.success(), "crm-api exited unsuccessfully: {status}");
}

async fn assert_force_rls_profile_boundary(
    admin: &PgPool,
    application_database_url: &str,
    record_id: &str,
) {
    let durable_count = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.records WHERE tenant_id = $1 AND owner_module_id = $2 AND record_type = $3 AND record_id = $4 AND deleted_at IS NULL",
    )
    .bind(TENANT_B)
    .bind(MODULE_ID)
    .bind(PROFILE_RECORD_TYPE)
    .bind(record_id)
    .fetch_one(admin)
    .await
    .expect("confirm tenant B completeness-profile record exists as administrator");
    assert_eq!(durable_count, 1);

    let application = PgPool::connect(application_database_url)
        .await
        .expect("connect application role for Data Quality RLS proof");
    let current_user = sqlx::query_scalar::<_, String>("SELECT current_user")
        .fetch_one(&application)
        .await
        .expect("read Data Quality RLS proof database role");
    assert_eq!(current_user, "crm_app_test");

    let tenant_a_visible =
        app_role_profile_count(&application, TENANT_A, TENANT_B, record_id).await;
    assert_eq!(
        tenant_a_visible, 0,
        "FORCE RLS must hide tenant B Data Quality records under tenant A context"
    );

    let tenant_b_visible =
        app_role_profile_count(&application, TENANT_B, TENANT_B, record_id).await;
    assert_eq!(
        tenant_b_visible, 1,
        "application role must see the same record under its owning tenant context"
    );
}

async fn app_role_profile_count(
    application: &PgPool,
    context_tenant: &str,
    row_tenant: &str,
    record_id: &str,
) -> i64 {
    let mut transaction = application
        .begin()
        .await
        .expect("begin Data Quality application-role RLS transaction");
    sqlx::query(
        "SELECT set_config('app.tenant_id', $1, true), set_config('app.actor_id', $2, true), set_config('app.request_id', $3, true), set_config('app.capability_id', $4, true), set_config('app.capability_version', '1.0.0', true), set_config('app.business_transaction_id', $5, true)",
    )
    .bind(context_tenant)
    .bind(ACTOR)
    .bind(unique_id("data-quality-rls-request"))
    .bind(GET_PROFILE)
    .bind(unique_id("data-quality-rls-transaction"))
    .execute(&mut *transaction)
    .await
    .expect("set transaction-local Data Quality RLS context");

    let count = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.records WHERE tenant_id = $1 AND owner_module_id = $2 AND record_type = $3 AND record_id = $4 AND deleted_at IS NULL",
    )
    .bind(row_tenant)
    .bind(MODULE_ID)
    .bind(PROFILE_RECORD_TYPE)
    .bind(record_id)
    .fetch_one(&mut *transaction)
    .await
    .expect("read Data Quality record through application-role RLS boundary");
    transaction
        .rollback()
        .await
        .expect("rollback Data Quality RLS proof transaction");
    count
}

async fn publish_rule_set(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: &CapabilityDefinition,
    tenant_id: &str,
    rule_key: &str,
    minimum_utf8_bytes: u32,
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
                                    minimum_utf8_bytes,
                                },
                            ),
                        ),
                        title: format!("Query process display name threshold for {tenant_id}"),
                        remediation_guidance: "Use a meaningful customer display name.".to_owned(),
                    }],
                }),
            },
        ),
        tenant_id,
        &unique_id("data-quality-query-rule-set-publish"),
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

async fn publish_profile(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: &CapabilityDefinition,
    tenant_id: &str,
    rule_set_version_ref: data_quality::PartyRuleSetVersionRef,
    rule_key: &str,
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
                    rule_set_version_ref: Some(rule_set_version_ref),
                    components: vec![data_quality::PartyCompletenessComponent {
                        component_key: "name.minimum".to_owned(),
                        rule_key: rule_key.to_owned(),
                        weight_basis_points: 10_000,
                    }],
                }),
            },
        ),
        tenant_id,
        &unique_id("data-quality-query-profile-publish"),
    )
    .await
    .expect("publish query acceptance Party completeness profile");
    data_quality::PublishPartyCompletenessProfileVersionResponse::decode(
        response
            .output
            .expect("Party completeness-profile publication output")
            .payload
            .as_slice(),
    )
    .expect("decode Party completeness-profile publication response")
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
        response
            .output
            .expect("Party completeness-profile query output")
            .payload
            .as_slice(),
    )
    .map_err(|error| {
        Status::internal(format!(
            "decode Party completeness-profile query response: {error}"
        ))
    })
}

fn assert_disclosed_rule_set(
    response: &data_quality::GetPartyRuleSetVersionResponse,
    expected_ref: &data_quality::PartyRuleSetVersionRef,
    expected_rule_key: &str,
) {
    let version = response
        .rule_set_version
        .as_ref()
        .expect("disclosed Party rule-set version");
    assert_eq!(version.rule_set_version_ref.as_ref(), Some(expected_ref));
    let definition = version
        .definition
        .as_ref()
        .expect("bootstrap visibility discloses rule-set definition");
    assert_eq!(definition.rules.len(), 1);
    assert_eq!(definition.rules[0].rule_key, expected_rule_key);
}

fn assert_disclosed_profile(
    response: &data_quality::GetPartyCompletenessProfileVersionResponse,
    expected_ref: &data_quality::PartyCompletenessProfileVersionRef,
    expected_rule_key: &str,
) {
    let version = response
        .completeness_profile_version
        .as_ref()
        .expect("disclosed Party completeness-profile version");
    assert_eq!(
        version.completeness_profile_version_ref.as_ref(),
        Some(expected_ref)
    );
    let definition = version
        .definition
        .as_ref()
        .expect("bootstrap visibility discloses completeness-profile definition");
    assert_eq!(definition.components.len(), 1);
    assert_eq!(definition.components[0].rule_key, expected_rule_key);
    assert_eq!(definition.components[0].weight_basis_points, 10_000);
}

fn rule_set_ref(
    response: &data_quality::PublishPartyRuleSetVersionResponse,
) -> data_quality::PartyRuleSetVersionRef {
    response
        .rule_set_version
        .as_ref()
        .and_then(|version| version.rule_set_version_ref.clone())
        .expect("published Party rule-set version ref")
}

fn profile_ref(
    response: &data_quality::PublishPartyCompletenessProfileVersionResponse,
) -> data_quality::PartyCompletenessProfileVersionRef {
    response
        .completeness_profile_version
        .as_ref()
        .and_then(|version| version.completeness_profile_version_ref.clone())
        .expect("published Party completeness-profile version ref")
}

async fn mutate(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: &CapabilityDefinition,
    input: TypedPayload,
    tenant_id: &str,
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
        .insert("x-tenant-id", tenant_id.parse().unwrap());
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
        .env("CRM_API_TENANTS", format!("{TENANT_A},{TENANT_B}"))
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

fn unique_id(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after Unix epoch")
        .as_nanos();
    format!("{prefix}-{}-{nanos}", std::process::id())
}
