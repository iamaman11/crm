#![cfg(unix)]

#[path = "support/customer_enrichment_process/mod.rs"]
mod support;

use crm_application_runtime::gateway_v1::{
    MutateResponse, application_gateway_service_client::ApplicationGatewayServiceClient,
};
use crm_capability_runtime::CapabilityDefinition;
use crm_customer_privacy::{
    ACTION_PLAN_RECORD_TYPE, ACTION_PLAN_STATE_MAXIMUM_BYTES,
    ACTION_PLAN_STATE_RETENTION_POLICY_ID, ACTION_PLAN_STATE_SCHEMA_ID,
    ACTION_PLAN_STATE_SCHEMA_VERSION, ActionPlanningPolicy, ContributionCompletenessProof,
    DISCOVERY_SCOPE_SNAPSHOT_STATE_MAXIMUM_BYTES,
    DISCOVERY_SCOPE_SNAPSHOT_STATE_RETENTION_POLICY_ID, DISCOVERY_SCOPE_SNAPSHOT_STATE_SCHEMA_ID,
    DISCOVERY_SCOPE_SNAPSHOT_STATE_SCHEMA_VERSION, DiscoveryOwnerScopeContribution,
    DiscoveryScopeSnapshot, MODULE_ID as PRIVACY_MODULE, OwnerScopeContribution,
    OwnerScopeRegistry, PRIVACY_CASE_RECORD_TYPE, PRIVACY_CASE_STATE_MAXIMUM_BYTES,
    PRIVACY_CASE_STATE_RETENTION_POLICY_ID, PRIVACY_CASE_STATE_SCHEMA_ID,
    PRIVACY_CASE_STATE_SCHEMA_VERSION, PrivacyActionPlan, PrivacyCase, PrivacyCaseKind,
    PrivacyCaseStatus, SCOPE_SNAPSHOT_RECORD_TYPE, ScopeDiscoveryLineage, ScopeResource,
    SubjectVerificationMethod, action_plan_state_descriptor_hash, decode_privacy_case_state,
    discovery_scope_snapshot_state_descriptor_hash, encode_action_plan_state,
    encode_discovery_scope_snapshot_state, encode_privacy_case_state,
    privacy_case_state_descriptor_hash,
};
use crm_module_sdk::{
    ActorId, DataClass, ModuleId, PayloadEncoding, RecordId, RetentionPolicyId, SchemaId,
    SchemaVersion, TenantId, TypedPayload,
};
use crm_proto_contracts::crm::customer_privacy::v1 as wire;
use prost::Message;
use reqwest::Client as HttpClient;
use serde_json::Value;
use sqlx::{PgPool, Postgres, Row, Transaction};
use tonic::{Code, Status};

use support::{
    ACTOR, TENANT_A, TENANT_B, TENANT_OUTSIDE_TOKEN, connect_grpc, free_port, http_mutate,
    mutate, mutation_definition, payload, spawn_crm_api, stop_process, wait_until_ready,
};

const APPROVE_CASE: &str = "customer_privacy.case.approve";
const APPROVE_SCOPE: &str = "capability:customer_privacy.case.approve:1.0.0";
const CANONICAL_PARTY: &str = "privacy-approval-canonical-party";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ApprovalEvidenceCounts {
    events: i64,
    audits: i64,
    idempotency: i64,
    transactions: i64,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn customer_privacy_case_approval_real_process_is_atomic_and_fail_closed() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping Customer Privacy approval process test because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect approval evidence reader");

    let approve_definition = mutation_definition(APPROVE_CASE);
    assert_eq!(approve_definition.owner_module_id.as_str(), PRIVACY_MODULE);

    let http_addr = format!("127.0.0.1:{}", free_port());
    let grpc_addr = format!("127.0.0.1:{}", free_port());
    let http = HttpClient::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("build approval HTTP client");
    let mut process = spawn_crm_api(&database_url, &http_addr, &grpc_addr, true, None);
    wait_until_ready(&http, &mut process, &http_addr, true).await;
    let mut grpc = connect_grpc(&grpc_addr).await;

    let success = seed_awaiting_approval(&admin, "success", false).await;
    let success_payload = approval_payload(&approve_definition, &success.case_id, 6);

    let unauthenticated = http_mutate(
        &http,
        &http_addr,
        &approve_definition,
        &success_payload,
        TENANT_A,
        "privacy-approval-unauthenticated",
        false,
    )
    .await;
    assert_eq!(unauthenticated.status(), reqwest::StatusCode::UNAUTHORIZED);
    let unauthenticated_body: Value = unauthenticated
        .json()
        .await
        .expect("decode unauthenticated approval response");
    assert_eq!(
        unauthenticated_body,
        serde_json::json!({"error": "request_failed"})
    );
    assert_safe_text(&unauthenticated_body.to_string());
    assert_case_version(&admin, TENANT_A, &success.case_id, 6).await;

    let outside_token = mutate(
        &mut grpc,
        &approve_definition,
        success_payload.clone(),
        TENANT_OUTSIDE_TOKEN,
        "privacy-approval-outside-token",
        true,
    )
    .await
    .expect_err("tenant outside bearer grant must be denied before approval");
    assert_safe_status(&outside_token, Code::PermissionDenied, "TENANT_FORBIDDEN", false);
    assert_case_version(&admin, TENANT_A, &success.case_id, 6).await;

    let success_key = "privacy-approval-success";
    let first = mutate(
        &mut grpc,
        &approve_definition,
        success_payload,
        TENANT_A,
        success_key,
        true,
    )
    .await
    .expect("approve AwaitingApproval case through generic gRPC ingress");
    let first_case = decode_approval(&first);
    assert_eq!(first_case.status, wire::PrivacyCaseStatus::Planned as i32);
    assert_eq!(first_case.version, 7);
    let approval = first_case
        .approval
        .as_ref()
        .expect("approval response contains immutable approval evidence");
    assert_eq!(approval.approved_by_actor_id, ACTOR);
    assert!(approval.approved_at_unix_ms > 0);
    assert_persisted_approval(&admin, &success.case_id).await;

    let committed = approval_evidence(&admin, TENANT_A, &success.case_id, success_key).await;
    assert_eq!(
        committed,
        ApprovalEvidenceCounts {
            events: 1,
            audits: 1,
            idempotency: 1,
            transactions: 1,
        }
    );

    let replay = mutate(
        &mut grpc,
        &approve_definition,
        approval_payload(&approve_definition, &success.case_id, 6),
        TENANT_A,
        success_key,
        true,
    )
    .await
    .expect("exact approval replay returns committed output");
    assert_eq!(decode_approval(&replay), first_case);
    assert_eq!(
        approval_evidence(&admin, TENANT_A, &success.case_id, success_key).await,
        committed
    );

    let conflicting_replay = mutate(
        &mut grpc,
        &approve_definition,
        approval_payload(&approve_definition, &success.case_id, 7),
        TENANT_A,
        success_key,
        true,
    )
    .await
    .expect_err("incompatible approval replay must conflict");
    assert_safe_status(
        &conflicting_replay,
        Code::Aborted,
        "CAPABILITY_IDEMPOTENCY_KEY_REUSED",
        false,
    );
    assert_case_version(&admin, TENANT_A, &success.case_id, 7).await;

    let already_planned = mutate(
        &mut grpc,
        &approve_definition,
        approval_payload(&approve_definition, &success.case_id, 7),
        TENANT_A,
        "privacy-approval-already-planned",
        true,
    )
    .await
    .expect_err("Planned case must not be approved under a new key");
    assert_safe_status(
        &already_planned,
        Code::Aborted,
        "CUSTOMER_PRIVACY_APPROVAL_CONFLICT",
        false,
    );

    let cross_tenant = mutate(
        &mut grpc,
        &approve_definition,
        approval_payload(&approve_definition, &success.case_id, 7),
        TENANT_B,
        "privacy-approval-cross-tenant",
        true,
    )
    .await
    .expect_err("cross-tenant approval target must be concealed");
    assert_safe_status(
        &cross_tenant,
        Code::NotFound,
        "CUSTOMER_PRIVACY_CASE_NOT_FOUND",
        false,
    );

    let stale = seed_awaiting_approval(&admin, "stale", false).await;
    let stale_error = mutate(
        &mut grpc,
        &approve_definition,
        approval_payload(&approve_definition, &stale.case_id, 5),
        TENANT_A,
        "privacy-approval-stale",
        true,
    )
    .await
    .expect_err("stale approval version must conflict");
    assert_safe_status(
        &stale_error,
        Code::Aborted,
        "CUSTOMER_PRIVACY_VERSION_CONFLICT",
        true,
    );
    assert_case_version(&admin, TENANT_A, &stale.case_id, 6).await;

    let corrupt = seed_awaiting_approval(&admin, "corrupt-link", true).await;
    let corrupt_error = mutate(
        &mut grpc,
        &approve_definition,
        approval_payload(&approve_definition, &corrupt.case_id, 6),
        TENANT_A,
        "privacy-approval-corrupt-link",
        true,
    )
    .await
    .expect_err("conflicting immutable planning link must fail closed");
    assert_safe_status(
        &corrupt_error,
        Code::Internal,
        "CUSTOMER_PRIVACY_APPROVAL_EVIDENCE_INVALID",
        false,
    );
    assert_case_version(&admin, TENANT_A, &corrupt.case_id, 6).await;
    assert_eq!(
        approval_evidence(
            &admin,
            TENANT_A,
            &corrupt.case_id,
            "privacy-approval-corrupt-link"
        )
        .await,
        ApprovalEvidenceCounts {
            events: 0,
            audits: 0,
            idempotency: 0,
            transactions: 0,
        }
    );

    let inactive = seed_awaiting_approval(&admin, "inactive", false).await;
    set_privacy_module_status(&admin, "suspended").await;
    let inactive_error = mutate(
        &mut grpc,
        &approve_definition,
        approval_payload(&approve_definition, &inactive.case_id, 6),
        TENANT_A,
        "privacy-approval-inactive",
        true,
    )
    .await
    .expect_err("inactive Customer Privacy module must reject approval");
    assert_safe_status(&inactive_error, Code::Aborted, "MODULE_NOT_ACTIVE", false);
    assert_case_version(&admin, TENANT_A, &inactive.case_id, 6).await;
    set_privacy_module_status(&admin, "active").await;
    stop_process(&mut process).await;

    let denied_http_addr = format!("127.0.0.1:{}", free_port());
    let denied_grpc_addr = format!("127.0.0.1:{}", free_port());
    let mut denied_process = spawn_crm_api(
        &database_url,
        &denied_http_addr,
        &denied_grpc_addr,
        false,
        None,
    );
    wait_until_ready(
        &http,
        &mut denied_process,
        &denied_http_addr,
        false,
    )
    .await;
    let mut denied_grpc: ApplicationGatewayServiceClient<tonic::transport::Channel> =
        connect_grpc(&denied_grpc_addr).await;
    let denied = mutate(
        &mut denied_grpc,
        &approve_definition,
        approval_payload(&approve_definition, &inactive.case_id, 6),
        TENANT_A,
        "privacy-approval-no-grant",
        true,
    )
    .await
    .expect_err("missing live capability grant must deny approval");
    assert_safe_status(
        &denied,
        Code::PermissionDenied,
        "CAPABILITY_PERMISSION_DENIED",
        false,
    );
    assert_case_version(&admin, TENANT_A, &inactive.case_id, 6).await;
    stop_process(&mut denied_process).await;
}

#[derive(Debug)]
struct ApprovalFixture {
    case_id: String,
}

async fn seed_awaiting_approval(
    pool: &PgPool,
    suffix: &str,
    corrupt_planning_link: bool,
) -> ApprovalFixture {
    let tenant = TenantId::try_new(TENANT_A).unwrap();
    let case_id = RecordId::try_new(format!("privacy-approval-case-{suffix}")).unwrap();
    let canonical_party = RecordId::try_new(CANONICAL_PARTY).unwrap();
    let policy_version = SchemaVersion::try_new("privacy-policy/1").unwrap();

    let mut privacy_case = PrivacyCase::new(
        case_id.clone(),
        tenant.clone(),
        PrivacyCaseKind::Erasure,
        policy_version.clone(),
        1_000_000_000,
        None,
    )
    .unwrap();
    privacy_case.submit(1, 2_000_000_000).unwrap();
    privacy_case
        .verify_subject(
            2,
            canonical_party.clone(),
            canonical_party.clone(),
            1,
            SubjectVerificationMethod::VerifiedDocument,
            ActorId::try_new("privacy-approval-fixture-verifier").unwrap(),
            3_000_000_000,
        )
        .unwrap();
    privacy_case.begin_scoping(3, 4_000_000_000).unwrap();

    let registry = OwnerScopeRegistry::canonical_v1().unwrap();
    let lineage = ScopeDiscoveryLineage::new(
        case_id.clone(),
        tenant.clone(),
        canonical_party.clone(),
        1,
        registry.registry_version().clone(),
        *registry.digest(),
        "ERASURE_REQUEST",
        5_000,
    )
    .unwrap();
    let contributions = registry
        .contracts()
        .iter()
        .enumerate()
        .map(|(index, contract)| {
            let terminal_digest = [u8::try_from(index + 1).unwrap(); 32];
            let completeness =
                ContributionCompletenessProof::new(true, 1, 0, 0, terminal_digest).unwrap();
            let contribution = OwnerScopeContribution::new(
                contract.clone(),
                tenant.clone(),
                canonical_party.clone(),
                1,
                Vec::<ScopeResource>::new(),
                completeness,
            )
            .unwrap();
            DiscoveryOwnerScopeContribution::new(lineage.clone(), contribution).unwrap()
        })
        .collect::<Vec<_>>();
    let snapshot = DiscoveryScopeSnapshot::finalize(
        lineage,
        registry,
        5_000_000_000,
        contributions,
    )
    .unwrap();
    privacy_case
        .record_scope(4, snapshot.snapshot_id().clone(), 5_000_000_000)
        .unwrap();

    let plan = PrivacyActionPlan::build(
        &snapshot,
        privacy_case.version(),
        privacy_case.kind(),
        ActionPlanningPolicy::new(policy_version, "EU", true, false).unwrap(),
        6_000_000_000,
    )
    .unwrap();
    privacy_case
        .record_plan(
            5,
            plan.plan_id().clone(),
            true,
            6_000_000_000,
        )
        .unwrap();
    assert_eq!(privacy_case.status(), PrivacyCaseStatus::AwaitingApproval);
    assert_eq!(privacy_case.version(), 6);

    let case_payload = governed_json_payload(
        PRIVACY_CASE_STATE_SCHEMA_ID,
        PRIVACY_CASE_STATE_SCHEMA_VERSION,
        privacy_case_state_descriptor_hash(),
        PRIVACY_CASE_STATE_MAXIMUM_BYTES,
        PRIVACY_CASE_STATE_RETENTION_POLICY_ID,
        encode_privacy_case_state(&privacy_case).unwrap(),
    );
    let snapshot_payload = governed_json_payload(
        DISCOVERY_SCOPE_SNAPSHOT_STATE_SCHEMA_ID,
        DISCOVERY_SCOPE_SNAPSHOT_STATE_SCHEMA_VERSION,
        discovery_scope_snapshot_state_descriptor_hash(),
        DISCOVERY_SCOPE_SNAPSHOT_STATE_MAXIMUM_BYTES,
        DISCOVERY_SCOPE_SNAPSHOT_STATE_RETENTION_POLICY_ID,
        encode_discovery_scope_snapshot_state(&snapshot).unwrap(),
    );
    let plan_payload = governed_json_payload(
        ACTION_PLAN_STATE_SCHEMA_ID,
        ACTION_PLAN_STATE_SCHEMA_VERSION,
        action_plan_state_descriptor_hash(),
        ACTION_PLAN_STATE_MAXIMUM_BYTES,
        ACTION_PLAN_STATE_RETENTION_POLICY_ID,
        encode_action_plan_state(&plan).unwrap(),
    );

    let transaction_id: String = sqlx::query_scalar(
        "SELECT last_business_transaction_id FROM crm.module_installations WHERE tenant_id = $1 AND module_id = $2",
    )
    .bind(TENANT_A)
    .bind(PRIVACY_MODULE)
    .fetch_one(pool)
    .await
    .expect("read Customer Privacy installation transaction");
    let mut transaction = pool.begin().await.expect("start approval fixture transaction");
    bind_fixture_context(&mut transaction, &transaction_id, suffix).await;
    insert_record(
        &mut transaction,
        PRIVACY_CASE_RECORD_TYPE,
        case_id.as_str(),
        6,
        &case_payload,
        &transaction_id,
    )
    .await;
    insert_record(
        &mut transaction,
        SCOPE_SNAPSHOT_RECORD_TYPE,
        snapshot.snapshot_id().as_str(),
        1,
        &snapshot_payload,
        &transaction_id,
    )
    .await;
    insert_record(
        &mut transaction,
        ACTION_PLAN_RECORD_TYPE,
        plan.plan_id().as_str(),
        1,
        &plan_payload,
        &transaction_id,
    )
    .await;
    let link_digest = if corrupt_planning_link {
        [0xee; 32]
    } else {
        *plan.digest()
    };
    sqlx::query(
        r#"
        INSERT INTO crm.customer_privacy_action_plans (
          tenant_id, privacy_case_id, source_case_version, resulting_case_version,
          scope_snapshot_id, plan_id, plan_digest, approval_required, planned_at
        ) VALUES (
          $1,$2,5,6,$3,$4,$5,true,
          TIMESTAMPTZ 'epoch' + 6000000 * INTERVAL '1 microsecond'
        )
        "#,
    )
    .bind(TENANT_A)
    .bind(case_id.as_str())
    .bind(snapshot.snapshot_id().as_str())
    .bind(plan.plan_id().as_str())
    .bind(link_digest.as_slice())
    .execute(&mut *transaction)
    .await
    .expect("insert immutable approval planning link");
    transaction
        .commit()
        .await
        .expect("commit canonical approval fixture");

    ApprovalFixture {
        case_id: case_id.as_str().to_owned(),
    }
}

fn governed_json_payload(
    schema_id: &str,
    schema_version: &str,
    descriptor_hash: [u8; 32],
    maximum_size_bytes: u64,
    retention_policy_id: &str,
    bytes: Vec<u8>,
) -> TypedPayload {
    let payload = TypedPayload {
        owner: ModuleId::try_new(PRIVACY_MODULE).unwrap(),
        schema_id: SchemaId::try_new(schema_id).unwrap(),
        schema_version: SchemaVersion::try_new(schema_version).unwrap(),
        descriptor_hash,
        data_class: DataClass::Confidential,
        encoding: PayloadEncoding::Json,
        maximum_size_bytes,
        retention_policy_id: RetentionPolicyId::try_new(retention_policy_id).unwrap(),
        bytes,
    };
    payload.validate().expect("valid canonical fixture payload");
    payload
}

async fn insert_record(
    transaction: &mut Transaction<'_, Postgres>,
    record_type: &str,
    record_id: &str,
    version: i64,
    payload: &TypedPayload,
    business_transaction_id: &str,
) {
    sqlx::query(
        r#"
        INSERT INTO crm.records (
          tenant_id, record_type, record_id, version, owner_module_id,
          schema_id, schema_version, descriptor_hash, data_class,
          payload_encoding, maximum_payload_size, retention_policy_id,
          payload_bytes, last_business_transaction_id
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,'confidential','json',$9,$10,$11,$12)
        "#,
    )
    .bind(TENANT_A)
    .bind(record_type)
    .bind(record_id)
    .bind(version)
    .bind(payload.owner.as_str())
    .bind(payload.schema_id.as_str())
    .bind(payload.schema_version.as_str())
    .bind(payload.descriptor_hash.as_slice())
    .bind(i64::try_from(payload.maximum_size_bytes).unwrap())
    .bind(payload.retention_policy_id.as_str())
    .bind(payload.bytes.as_slice())
    .bind(business_transaction_id)
    .execute(&mut **transaction)
    .await
    .expect("insert canonical Customer Privacy approval record");
}

async fn bind_fixture_context(
    transaction: &mut Transaction<'_, Postgres>,
    business_transaction_id: &str,
    suffix: &str,
) {
    let request_id = format!("privacy-approval-fixture-{suffix}");
    for (name, value) in [
        ("app.tenant_id", TENANT_A),
        ("app.actor_id", "privacy-approval-fixture"),
        ("app.request_id", request_id.as_str()),
        ("app.capability_id", "customer_privacy.approval.fixture"),
        ("app.capability_version", "1.0.0"),
        ("app.business_transaction_id", business_transaction_id),
    ] {
        sqlx::query("SELECT set_config($1, $2, true)")
            .bind(name)
            .bind(value)
            .execute(&mut **transaction)
            .await
            .expect("bind approval fixture context");
    }
}

fn approval_payload(
    definition: &CapabilityDefinition,
    case_id: &str,
    expected_version: i64,
) -> TypedPayload {
    payload(
        definition,
        wire::ApprovePrivacyCaseRequest {
            privacy_case_ref: Some(wire::PrivacyCaseRef {
                privacy_case_id: case_id.to_owned(),
            }),
            expected_version,
        },
    )
}

fn decode_approval(response: &MutateResponse) -> wire::PrivacyCase {
    wire::ApprovePrivacyCaseResponse::decode(
        response
            .output
            .as_ref()
            .expect("approval output")
            .payload
            .as_slice(),
    )
    .expect("decode approval response")
    .privacy_case
    .expect("approval response case")
}

async fn assert_persisted_approval(pool: &PgPool, case_id: &str) {
    let row = sqlx::query(
        "SELECT version, payload_bytes FROM crm.records WHERE tenant_id = $1 AND owner_module_id = $2 AND record_type = $3 AND record_id = $4",
    )
    .bind(TENANT_A)
    .bind(PRIVACY_MODULE)
    .bind(PRIVACY_CASE_RECORD_TYPE)
    .bind(case_id)
    .fetch_one(pool)
    .await
    .expect("read approved case record");
    assert_eq!(row.get::<i64, _>("version"), 7);
    let bytes: Vec<u8> = row.get("payload_bytes");
    let privacy_case = decode_privacy_case_state(&bytes).expect("strictly rehydrate approved case");
    assert_eq!(privacy_case.status(), PrivacyCaseStatus::Planned);
    assert_eq!(privacy_case.version(), 7);
    let approval = privacy_case
        .approval()
        .expect("persisted case contains approval evidence");
    assert_eq!(approval.approved_by.as_str(), ACTOR);
    assert!(approval.approved_at_unix_nanos > 0);
}

async fn approval_evidence(
    pool: &PgPool,
    tenant: &str,
    case_id: &str,
    idempotency_key: &str,
) -> ApprovalEvidenceCounts {
    ApprovalEvidenceCounts {
        events: sqlx::query_scalar(
            "SELECT count(*) FROM crm.outbox_events WHERE tenant_id = $1 AND aggregate_type = $2 AND aggregate_id = $3 AND event_type = 'customer_privacy.case.status_changed' AND aggregate_version = 7",
        )
        .bind(tenant)
        .bind(PRIVACY_CASE_RECORD_TYPE)
        .bind(case_id)
        .fetch_one(pool)
        .await
        .expect("count approval events"),
        audits: sqlx::query_scalar(
            "SELECT count(*) FROM crm.audit_records a JOIN crm.idempotency_records i ON i.tenant_id = a.tenant_id AND i.business_transaction_id = a.business_transaction_id WHERE i.tenant_id = $1 AND i.idempotency_scope = $2 AND i.idempotency_key = $3 AND a.capability_id = $4",
        )
        .bind(tenant)
        .bind(APPROVE_SCOPE)
        .bind(idempotency_key)
        .bind(APPROVE_CASE)
        .fetch_one(pool)
        .await
        .expect("count approval audits"),
        idempotency: sqlx::query_scalar(
            "SELECT count(*) FROM crm.idempotency_records WHERE tenant_id = $1 AND idempotency_scope = $2 AND idempotency_key = $3 AND status = 'completed'",
        )
        .bind(tenant)
        .bind(APPROVE_SCOPE)
        .bind(idempotency_key)
        .fetch_one(pool)
        .await
        .expect("count approval idempotency evidence"),
        transactions: sqlx::query_scalar(
            "SELECT count(*) FROM crm.business_transactions bt JOIN crm.idempotency_records i ON i.tenant_id = bt.tenant_id AND i.business_transaction_id = bt.business_transaction_id WHERE i.tenant_id = $1 AND i.idempotency_scope = $2 AND i.idempotency_key = $3 AND bt.capability_id = $4",
        )
        .bind(tenant)
        .bind(APPROVE_SCOPE)
        .bind(idempotency_key)
        .bind(APPROVE_CASE)
        .fetch_one(pool)
        .await
        .expect("count approval business transactions"),
    }
}

async fn assert_case_version(pool: &PgPool, tenant: &str, case_id: &str, version: i64) {
    let actual: i64 = sqlx::query_scalar(
        "SELECT version FROM crm.records WHERE tenant_id = $1 AND owner_module_id = $2 AND record_type = $3 AND record_id = $4",
    )
    .bind(tenant)
    .bind(PRIVACY_MODULE)
    .bind(PRIVACY_CASE_RECORD_TYPE)
    .bind(case_id)
    .fetch_one(pool)
    .await
    .expect("read approval case version");
    assert_eq!(actual, version);
}

async fn set_privacy_module_status(pool: &PgPool, status: &str) {
    let transaction_id: String = sqlx::query_scalar(
        "SELECT last_business_transaction_id FROM crm.module_installations WHERE tenant_id = $1 AND module_id = $2",
    )
    .bind(TENANT_A)
    .bind(PRIVACY_MODULE)
    .fetch_one(pool)
    .await
    .expect("read Customer Privacy installation");
    let mut transaction = pool.begin().await.expect("start activation update");
    for (name, value) in [
        ("app.tenant_id", TENANT_A),
        ("app.actor_id", "customer-privacy-approval-process-admin"),
        ("app.request_id", "customer-privacy-approval-process-activation"),
        ("app.capability_id", "customer_privacy.process.activation"),
        ("app.capability_version", "1.0.0"),
        ("app.business_transaction_id", transaction_id.as_str()),
    ] {
        sqlx::query("SELECT set_config($1, $2, true)")
            .bind(name)
            .bind(value)
            .execute(&mut *transaction)
            .await
            .expect("bind approval activation context");
    }
    sqlx::query(
        "UPDATE crm.module_installations SET status = $1, updated_at = clock_timestamp() WHERE tenant_id = $2 AND module_id = $3",
    )
    .bind(status)
    .bind(TENANT_A)
    .bind(PRIVACY_MODULE)
    .execute(&mut *transaction)
    .await
    .expect("update Customer Privacy activation state");
    transaction
        .commit()
        .await
        .expect("commit approval activation update");
}

fn assert_safe_status(
    status: &Status,
    expected_code: Code,
    expected_error_code: &str,
    expected_retryable: bool,
) {
    assert_eq!(status.code(), expected_code);
    assert_eq!(
        status
            .metadata()
            .get("x-error-code")
            .expect("typed gRPC error code")
            .to_str()
            .expect("ASCII error code"),
        expected_error_code
    );
    assert_eq!(
        status
            .metadata()
            .get("x-error-retryable")
            .expect("retryability metadata")
            .to_str()
            .expect("ASCII retryability metadata"),
        if expected_retryable { "true" } else { "false" }
    );
    assert_safe_text(status.message());
    assert_safe_text(&format!("{:?}", status.metadata()));
}

fn assert_safe_text(value: &str) {
    for forbidden in [
        CANONICAL_PARTY,
        "privacy-action-plan-",
        "privacy-discovery-scope-",
        "payload_bytes",
        "plan_digest",
        "scope_snapshot_id",
    ] {
        assert!(
            !value.contains(forbidden),
            "safe error leaked protected approval detail: {forbidden}"
        );
    }
}
