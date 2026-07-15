#![cfg(unix)]

use crm_application_runtime::{
    application_mutation_definitions,
    gateway_v1::{
        MutateRequest as GatewayMutateRequest, TypedPayload as GatewayTypedPayload,
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
use tonic::{Request, Status};

const TENANT: &str = "tenant-a";
const ACTOR: &str = "actor-a";
const TOKEN: &str = "data-quality-process-bearer-token-0123456789abcdef0123456789abcdef";
const APPROVAL_KEY: &str = "data-quality-process-approval-key-0123456789abcdef";
const PUBLISH_RULE_SET: &str = "data_quality.party.rule_set.publish";
const MODULE_ID: &str = "crm.data-quality";
const RULE_SET_RECORD_TYPE: &str = "data_quality.party_rule_set_version";
const RULE_SET_PUBLISHED_EVENT_TYPE: &str = "data_quality.party.rule_set.published";

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn crm_api_process_publishes_content_addressed_party_rule_sets_idempotently() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping data-quality process acceptance because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect data-quality process evidence reader");
    admin
        .execute(sqlx::raw_sql(include_str!(
            "../../../database/tests/0014_data_quality_adapter.sql"
        )))
        .await
        .expect("publish data-quality production adapter registry fixture");

    let http_port = free_port();
    let mut grpc_port = free_port();
    while grpc_port == http_port {
        grpc_port = free_port();
    }
    let http_addr = format!("127.0.0.1:{http_port}");
    let grpc_addr = format!("127.0.0.1:{grpc_port}");
    let http = reqwest::Client::new();

    let definition = mutation_definition(PUBLISH_RULE_SET);
    let mut child = spawn_crm_api(&database_url, &http_addr, &grpc_addr);
    wait_until_ready(&http, &mut child, &http_addr).await;
    let mut grpc = connect_grpc(&grpc_addr).await;

    let replay_key = unique_id("data-quality-rule-set-publication");
    let first = publish_rule_set(
        &mut grpc,
        &definition,
        canonical_equivalent_definition(4, true),
        &replay_key,
    )
    .await
    .expect("publish Party rule-set through production gateway");
    let first_version_id = rule_set_version_id(&first);
    assert!(first_version_id.starts_with("dq-party-rule-set-"));
    assert_canonical_first_definition(&first);

    assert_eq!(rule_set_record_count(&admin).await, 1);
    assert_eq!(rule_set_record_version(&admin, &first_version_id).await, 1);
    assert_eq!(published_event_count(&admin).await, 1);
    assert_eq!(publication_audit_count(&admin).await, 1);

    let replay = publish_rule_set(
        &mut grpc,
        &definition,
        canonical_equivalent_definition(4, true),
        &replay_key,
    )
    .await
    .expect("replay Party rule-set publication through production gateway");
    assert_eq!(replay, first);
    assert_eq!(rule_set_record_count(&admin).await, 1);
    assert_eq!(published_event_count(&admin).await, 1);
    assert_eq!(publication_audit_count(&admin).await, 1);

    let duplicate_content = publish_rule_set(
        &mut grpc,
        &definition,
        canonical_equivalent_definition(4, false),
        &unique_id("data-quality-same-content-new-key"),
    )
    .await;
    assert!(
        duplicate_content.is_err(),
        "a new idempotency identity must not silently upsert an existing content-addressed version"
    );
    assert_eq!(rule_set_record_count(&admin).await, 1);
    assert_eq!(published_event_count(&admin).await, 1);
    assert_eq!(publication_audit_count(&admin).await, 1);

    let changed = publish_rule_set(
        &mut grpc,
        &definition,
        canonical_equivalent_definition(5, false),
        &unique_id("data-quality-changed-content"),
    )
    .await
    .expect("publish changed Party rule-set content through production gateway");
    let changed_version_id = rule_set_version_id(&changed);
    assert_ne!(changed_version_id, first_version_id);
    assert_eq!(rule_set_record_count(&admin).await, 2);
    assert_eq!(rule_set_record_version(&admin, &changed_version_id).await, 1);
    assert_eq!(published_event_count(&admin).await, 2);
    assert_eq!(publication_audit_count(&admin).await, 2);

    send_sigint(&child).await;
    let status = child
        .wait()
        .await
        .expect("wait for data-quality acceptance crm-api");
    assert!(status.success(), "crm-api exited unsuccessfully: {status}");
}

async fn publish_rule_set(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: &CapabilityDefinition,
    rule_set_definition: data_quality::PartyRuleSetDefinition,
    idempotency_key: &str,
) -> Result<data_quality::PublishPartyRuleSetVersionResponse, Status> {
    let response = mutate(
        client,
        definition,
        payload(
            definition,
            data_quality::PublishPartyRuleSetVersionRequest {
                definition: Some(rule_set_definition),
            },
        ),
        idempotency_key,
    )
    .await?;
    data_quality::PublishPartyRuleSetVersionResponse::decode(
        response.output.expect("rule-set publication output").payload.as_slice(),
    )
    .map_err(|error| Status::internal(format!("decode Party rule-set response: {error}")))
}

fn canonical_equivalent_definition(
    minimum_utf8_bytes: u32,
    noncanonical_caller_order: bool,
) -> data_quality::PartyRuleSetDefinition {
    let minimum = data_quality::PartyQualityRule {
        rule_key: "display_name.minimum".to_owned(),
        severity: data_quality::QualitySeverity::Warning as i32,
        evaluator: Some(
            data_quality::party_quality_rule::Evaluator::DisplayNameMinUtf8Bytes(
                data_quality::PartyDisplayNameMinUtf8BytesEvaluator { minimum_utf8_bytes },
            ),
        ),
        title: "Display name length".to_owned(),
        remediation_guidance: "Replace the display name with a meaningful customer name."
            .to_owned(),
    };
    let placeholder = data_quality::PartyQualityRule {
        rule_key: "display_name.placeholder".to_owned(),
        severity: data_quality::QualitySeverity::Error as i32,
        evaluator: Some(
            data_quality::party_quality_rule::Evaluator::DisplayNamePlaceholderExactAsciiCasefold(
                data_quality::PartyDisplayNamePlaceholderExactAsciiCasefoldEvaluator {
                    placeholder_tokens: if noncanonical_caller_order {
                        vec![" UNKNOWN ".to_owned(), "N/A".to_owned()]
                    } else {
                        vec!["n/a".to_owned(), "unknown".to_owned()]
                    },
                },
            ),
        ),
        title: "Placeholder display name".to_owned(),
        remediation_guidance: "Replace the placeholder with the real customer name.".to_owned(),
    };
    data_quality::PartyRuleSetDefinition {
        evaluator_semantic_version: data_quality::PartyQualityEvaluatorSemanticVersion::V1 as i32,
        rules: if noncanonical_caller_order {
            vec![placeholder, minimum]
        } else {
            vec![minimum, placeholder]
        },
    }
}

fn assert_canonical_first_definition(response: &data_quality::PublishPartyRuleSetVersionResponse) {
    let definition = response
        .rule_set_version
        .as_ref()
        .and_then(|version| version.definition.as_ref())
        .expect("canonical published Party rule-set definition");
    assert_eq!(definition.rules.len(), 2);
    assert_eq!(definition.rules[0].rule_key, "display_name.minimum");
    assert_eq!(definition.rules[1].rule_key, "display_name.placeholder");
    match definition.rules[1].evaluator.as_ref().unwrap() {
        data_quality::party_quality_rule::Evaluator::DisplayNamePlaceholderExactAsciiCasefold(
            parameters,
        ) => assert_eq!(parameters.placeholder_tokens, ["n/a", "unknown"]),
        other => panic!("unexpected canonical evaluator: {other:?}"),
    }
}

fn rule_set_version_id(response: &data_quality::PublishPartyRuleSetVersionResponse) -> String {
    response
        .rule_set_version
        .as_ref()
        .and_then(|version| version.rule_set_version_ref.as_ref())
        .expect("published Party rule-set version ref")
        .rule_set_version_id
        .clone()
}

async fn rule_set_record_count(admin: &PgPool) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.records WHERE tenant_id = $1 AND owner_module_id = $2 AND record_type = $3 AND deleted_at IS NULL",
    )
    .bind(TENANT)
    .bind(MODULE_ID)
    .bind(RULE_SET_RECORD_TYPE)
    .fetch_one(admin)
    .await
    .expect("count durable Data Quality rule-set records")
}

async fn rule_set_record_version(admin: &PgPool, record_id: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT version FROM crm.records WHERE tenant_id = $1 AND owner_module_id = $2 AND record_type = $3 AND record_id = $4 AND deleted_at IS NULL",
    )
    .bind(TENANT)
    .bind(MODULE_ID)
    .bind(RULE_SET_RECORD_TYPE)
    .bind(record_id)
    .fetch_one(admin)
    .await
    .expect("read durable Data Quality rule-set record version")
}

async fn published_event_count(admin: &PgPool) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.outbox_events WHERE tenant_id = $1 AND event_type = $2",
    )
    .bind(TENANT)
    .bind(RULE_SET_PUBLISHED_EVENT_TYPE)
    .fetch_one(admin)
    .await
    .expect("count Data Quality rule-set published events")
}

async fn publication_audit_count(admin: &PgPool) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.audit_records WHERE tenant_id = $1 AND capability_id = $2",
    )
    .bind(TENANT)
    .bind(PUBLISH_RULE_SET)
    .fetch_one(admin)
    .await
    .expect("count Data Quality rule-set publication audits")
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
        .insert("x-tenant-id", TENANT.parse().unwrap());
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
        .expect("valid application mutation definitions")
        .into_iter()
        .find(|definition| definition.capability_id.as_str() == capability_id)
        .unwrap_or_else(|| panic!("missing application mutation definition: {capability_id}"))
}

fn payload<M: Message>(definition: &CapabilityDefinition, message: M) -> TypedPayload {
    let data_class = *definition
        .input_contract
        .allowed_data_classes
        .first()
        .expect("capability input data class");
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
            "data-quality-process-cursor-key-0123456789abcdef",
        )
        .env("CRM_APPROVAL_SIGNING_KEY", APPROVAL_KEY)
        .env("CRM_BOOTSTRAP_ALLOW_PHASE6", "true")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn production crm-api process for Data Quality acceptance")
}

async fn wait_until_ready(client: &reqwest::Client, child: &mut Child, http_addr: &str) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child.try_wait().expect("poll Data Quality acceptance crm-api") {
            panic!("crm-api exited before Data Quality acceptance readiness: {status}");
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
            "Data Quality acceptance crm-api readiness timed out"
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
                    "Data Quality acceptance gRPC listener timed out: {error}"
                );
                sleep(Duration::from_millis(200)).await;
            }
        }
    }
}

async fn send_sigint(child: &Child) {
    let pid = child
        .id()
        .expect("running Data Quality acceptance crm-api has a PID");
    let status = Command::new("kill")
        .arg("-INT")
        .arg(pid.to_string())
        .status()
        .await
        .expect("send SIGINT to Data Quality acceptance crm-api");
    assert!(status.success(), "kill -INT failed: {status}");
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral Data Quality acceptance port")
        .local_addr()
        .expect("read ephemeral Data Quality acceptance port")
        .port()
}

fn unique_id(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after Unix epoch")
        .as_nanos();
    format!("{prefix}-{}-{nanos}", std::process::id())
}
