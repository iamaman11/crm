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
const TOKEN: &str = "data-quality-profile-process-bearer-token-0123456789abcdef0123456789abcdef";
const APPROVAL_KEY: &str = "data-quality-profile-process-approval-key-0123456789abcdef";
const PUBLISH_RULE_SET: &str = "data_quality.party.rule_set.publish";
const PUBLISH_PROFILE: &str = "data_quality.party.completeness_profile.publish";
const MODULE_ID: &str = "crm.data-quality";
const PROFILE_RECORD_TYPE: &str = "data_quality.party_completeness_profile_version";
const PROFILE_PUBLISHED_EVENT_TYPE: &str = "data_quality.party.completeness_profile.published";

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn crm_api_process_publishes_party_completeness_profiles_with_exact_binding_and_replay() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!(
            "skipping data-quality completeness-profile process acceptance because DATABASE_URL is absent"
        );
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect completeness-profile process evidence reader");
    admin
        .execute(sqlx::raw_sql(include_str!(
            "../../../database/tests/0014_data_quality_adapter.sql"
        )))
        .await
        .expect("publish Data Quality production adapter registry fixture");

    let http_port = free_port();
    let mut grpc_port = free_port();
    while grpc_port == http_port {
        grpc_port = free_port();
    }
    let http_addr = format!("127.0.0.1:{http_port}");
    let grpc_addr = format!("127.0.0.1:{grpc_port}");
    let http = reqwest::Client::new();

    let rule_set_definition = mutation_definition(PUBLISH_RULE_SET);
    let profile_definition = mutation_definition(PUBLISH_PROFILE);
    let mut child = spawn_crm_api(&database_url, &http_addr, &grpc_addr);
    wait_until_ready(&http, &mut child, &http_addr).await;
    let mut grpc = connect_grpc(&grpc_addr).await;

    let rule_set = publish_rule_set(
        &mut grpc,
        &rule_set_definition,
        unique_rule_set_definition(),
        &unique_id("data-quality-profile-rule-set"),
    )
    .await
    .expect("publish profile acceptance Party rule-set");
    let rule_set_version_id = rule_set_version_id(&rule_set);

    let replay_key = unique_id("data-quality-profile-publication");
    let first = publish_profile(
        &mut grpc,
        &profile_definition,
        profile_input(&rule_set_version_id, 4_000, true),
        &replay_key,
    )
    .await
    .expect("publish Party completeness profile through production gateway");
    let first_version_id = profile_version_id(&first);
    assert!(first_version_id.starts_with("dq-party-completeness-profile-"));
    assert_canonical_profile(&first, &rule_set_version_id, 4_000, 6_000);

    assert_eq!(profile_record_count(&admin).await, 1);
    assert_eq!(profile_record_version(&admin, &first_version_id).await, 1);
    assert_eq!(profile_event_count(&admin).await, 1);
    assert_eq!(profile_audit_count(&admin).await, 1);

    let replay = publish_profile(
        &mut grpc,
        &profile_definition,
        profile_input(&rule_set_version_id, 4_000, true),
        &replay_key,
    )
    .await
    .expect("replay Party completeness-profile publication");
    assert_eq!(replay, first);
    assert_profile_side_effect_counts(&admin, 1).await;

    let changed_same_key = publish_profile(
        &mut grpc,
        &profile_definition,
        profile_input(&rule_set_version_id, 5_000, true),
        &replay_key,
    )
    .await;
    assert!(
        changed_same_key.is_err(),
        "the same idempotency key must reject changed completeness-profile content"
    );
    assert_profile_side_effect_counts(&admin, 1).await;

    let duplicate_content = publish_profile(
        &mut grpc,
        &profile_definition,
        profile_input(&rule_set_version_id, 4_000, false),
        &unique_id("data-quality-profile-same-content-new-key"),
    )
    .await;
    assert!(
        duplicate_content.is_err(),
        "a new idempotency identity must not silently upsert an existing content-addressed profile"
    );
    assert_profile_side_effect_counts(&admin, 1).await;

    let unavailable_reference = publish_profile(
        &mut grpc,
        &profile_definition,
        profile_input("dq-party-rule-set-unavailable", 4_000, false),
        &unique_id("data-quality-profile-unavailable-rule-set"),
    )
    .await;
    assert!(
        unavailable_reference.is_err(),
        "an unavailable rule-set reference must fail before profile side effects"
    );
    assert_profile_side_effect_counts(&admin, 1).await;

    let changed = publish_profile(
        &mut grpc,
        &profile_definition,
        profile_input(&rule_set_version_id, 5_000, false),
        &unique_id("data-quality-profile-changed-content"),
    )
    .await
    .expect("publish changed Party completeness-profile content");
    let changed_version_id = profile_version_id(&changed);
    assert_ne!(changed_version_id, first_version_id);
    assert_canonical_profile(&changed, &rule_set_version_id, 5_000, 5_000);
    assert_eq!(profile_record_count(&admin).await, 2);
    assert_eq!(profile_record_version(&admin, &changed_version_id).await, 1);
    assert_eq!(profile_event_count(&admin).await, 2);
    assert_eq!(profile_audit_count(&admin).await, 2);

    send_sigint(&child).await;
    let status = child
        .wait()
        .await
        .expect("wait for completeness-profile acceptance crm-api");
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
        response
            .output
            .expect("profile acceptance rule-set publication output")
            .payload
            .as_slice(),
    )
    .map_err(|error| Status::internal(format!("decode Party rule-set response: {error}")))
}

async fn publish_profile(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: &CapabilityDefinition,
    profile: data_quality::PartyCompletenessProfileDefinition,
    idempotency_key: &str,
) -> Result<data_quality::PublishPartyCompletenessProfileVersionResponse, Status> {
    let response = mutate(
        client,
        definition,
        payload(
            definition,
            data_quality::PublishPartyCompletenessProfileVersionRequest {
                definition: Some(profile),
            },
        ),
        idempotency_key,
    )
    .await?;
    data_quality::PublishPartyCompletenessProfileVersionResponse::decode(
        response
            .output
            .expect("completeness-profile publication output")
            .payload
            .as_slice(),
    )
    .map_err(|error| {
        Status::internal(format!(
            "decode Party completeness-profile response: {error}"
        ))
    })
}

fn unique_rule_set_definition() -> data_quality::PartyRuleSetDefinition {
    data_quality::PartyRuleSetDefinition {
        evaluator_semantic_version: data_quality::PartyQualityEvaluatorSemanticVersion::V1 as i32,
        rules: vec![
            data_quality::PartyQualityRule {
                rule_key: "display_name.profile_process_minimum".to_owned(),
                severity: data_quality::QualitySeverity::Warning as i32,
                evaluator: Some(
                    data_quality::party_quality_rule::Evaluator::DisplayNameMinUtf8Bytes(
                        data_quality::PartyDisplayNameMinUtf8BytesEvaluator {
                            minimum_utf8_bytes: 7,
                        },
                    ),
                ),
                title: "Profile process display name length".to_owned(),
                remediation_guidance: "Use a meaningful profile-process display name.".to_owned(),
            },
            data_quality::PartyQualityRule {
                rule_key: "display_name.profile_process_placeholder".to_owned(),
                severity: data_quality::QualitySeverity::Error as i32,
                evaluator: Some(
                    data_quality::party_quality_rule::Evaluator::DisplayNamePlaceholderExactAsciiCasefold(
                        data_quality::PartyDisplayNamePlaceholderExactAsciiCasefoldEvaluator {
                            placeholder_tokens: vec!["profile unknown".to_owned()],
                        },
                    ),
                ),
                title: "Profile process placeholder".to_owned(),
                remediation_guidance: "Replace the profile-process placeholder.".to_owned(),
            },
        ],
    }
}

fn profile_input(
    rule_set_version_id: &str,
    minimum_weight: u32,
    noncanonical_caller_order: bool,
) -> data_quality::PartyCompletenessProfileDefinition {
    let minimum = data_quality::PartyCompletenessComponent {
        component_key: "name.minimum".to_owned(),
        rule_key: "display_name.profile_process_minimum".to_owned(),
        weight_basis_points: minimum_weight,
    };
    let placeholder = data_quality::PartyCompletenessComponent {
        component_key: "name.placeholder".to_owned(),
        rule_key: "display_name.profile_process_placeholder".to_owned(),
        weight_basis_points: 10_000 - minimum_weight,
    };
    data_quality::PartyCompletenessProfileDefinition {
        completeness_semantic_version: data_quality::PartyCompletenessSemanticVersion::V1 as i32,
        rule_set_version_ref: Some(data_quality::PartyRuleSetVersionRef {
            rule_set_version_id: rule_set_version_id.to_owned(),
        }),
        components: if noncanonical_caller_order {
            vec![placeholder, minimum]
        } else {
            vec![minimum, placeholder]
        },
    }
}

fn assert_canonical_profile(
    response: &data_quality::PublishPartyCompletenessProfileVersionResponse,
    expected_rule_set_version_id: &str,
    expected_minimum_weight: u32,
    expected_placeholder_weight: u32,
) {
    let definition = response
        .completeness_profile_version
        .as_ref()
        .and_then(|version| version.definition.as_ref())
        .expect("canonical published Party completeness-profile definition");
    assert_eq!(
        definition
            .rule_set_version_ref
            .as_ref()
            .expect("profile rule-set reference")
            .rule_set_version_id,
        expected_rule_set_version_id
    );
    assert_eq!(definition.components.len(), 2);
    assert_eq!(definition.components[0].component_key, "name.minimum");
    assert_eq!(
        definition.components[0].weight_basis_points,
        expected_minimum_weight
    );
    assert_eq!(definition.components[1].component_key, "name.placeholder");
    assert_eq!(
        definition.components[1].weight_basis_points,
        expected_placeholder_weight
    );
    assert_eq!(
        definition
            .components
            .iter()
            .map(|component| component.weight_basis_points)
            .sum::<u32>(),
        10_000
    );
}

fn rule_set_version_id(response: &data_quality::PublishPartyRuleSetVersionResponse) -> String {
    response
        .rule_set_version
        .as_ref()
        .and_then(|version| version.rule_set_version_ref.as_ref())
        .expect("published profile acceptance Party rule-set version ref")
        .rule_set_version_id
        .clone()
}

fn profile_version_id(
    response: &data_quality::PublishPartyCompletenessProfileVersionResponse,
) -> String {
    response
        .completeness_profile_version
        .as_ref()
        .and_then(|version| version.completeness_profile_version_ref.as_ref())
        .expect("published Party completeness-profile version ref")
        .completeness_profile_version_id
        .clone()
}

async fn profile_record_count(admin: &PgPool) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.records WHERE tenant_id = $1 AND owner_module_id = $2 AND record_type = $3 AND deleted_at IS NULL",
    )
    .bind(TENANT)
    .bind(MODULE_ID)
    .bind(PROFILE_RECORD_TYPE)
    .fetch_one(admin)
    .await
    .expect("count durable Party completeness-profile records")
}

async fn profile_record_version(admin: &PgPool, record_id: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT version FROM crm.records WHERE tenant_id = $1 AND owner_module_id = $2 AND record_type = $3 AND record_id = $4 AND deleted_at IS NULL",
    )
    .bind(TENANT)
    .bind(MODULE_ID)
    .bind(PROFILE_RECORD_TYPE)
    .bind(record_id)
    .fetch_one(admin)
    .await
    .expect("read durable Party completeness-profile record version")
}

async fn profile_event_count(admin: &PgPool) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.outbox_events WHERE tenant_id = $1 AND event_type = $2",
    )
    .bind(TENANT)
    .bind(PROFILE_PUBLISHED_EVENT_TYPE)
    .fetch_one(admin)
    .await
    .expect("count Party completeness-profile published events")
}

async fn profile_audit_count(admin: &PgPool) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.audit_records WHERE tenant_id = $1 AND capability_id = $2",
    )
    .bind(TENANT)
    .bind(PUBLISH_PROFILE)
    .fetch_one(admin)
    .await
    .expect("count Party completeness-profile publication audits")
}

async fn assert_profile_side_effect_counts(admin: &PgPool, expected: i64) {
    assert_eq!(profile_record_count(admin).await, expected);
    assert_eq!(profile_event_count(admin).await, expected);
    assert_eq!(profile_audit_count(admin).await, expected);
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
            "data-quality-profile-process-cursor-key-0123456789abcdef",
        )
        .env("CRM_APPROVAL_SIGNING_KEY", APPROVAL_KEY)
        .env("CRM_BOOTSTRAP_ALLOW_PHASE6", "true")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn production crm-api process for completeness-profile acceptance")
}

async fn wait_until_ready(client: &reqwest::Client, child: &mut Child, http_addr: &str) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child
            .try_wait()
            .expect("poll completeness-profile acceptance crm-api")
        {
            panic!("crm-api exited before completeness-profile readiness: {status}");
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
            "completeness-profile acceptance crm-api readiness timed out"
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
                    "completeness-profile gRPC listener timed out: {error}"
                );
                sleep(Duration::from_millis(200)).await;
            }
        }
    }
}

async fn send_sigint(child: &Child) {
    let pid = child
        .id()
        .expect("running completeness-profile acceptance crm-api has a PID");
    let status = Command::new("kill")
        .arg("-INT")
        .arg(pid.to_string())
        .status()
        .await
        .expect("send SIGINT to completeness-profile acceptance crm-api");
    assert!(status.success(), "kill -INT failed: {status}");
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral completeness-profile acceptance port")
        .local_addr()
        .expect("read ephemeral completeness-profile acceptance port")
        .port()
}

fn unique_id(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after Unix epoch")
        .as_nanos();
    format!("{prefix}-{}-{nanos}", std::process::id())
}
