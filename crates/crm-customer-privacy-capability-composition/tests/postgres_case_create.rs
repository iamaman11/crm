use crm_capability_plan_support as plan_support;
use crm_capability_runtime::CapabilityRequest;
use crm_core_data::PostgresDataStore;
use crm_customer_privacy::{
    MODULE_ID, PRIVACY_CASE_RECORD_TYPE, PRIVACY_CASE_STATE_MAXIMUM_BYTES,
    PRIVACY_CASE_STATE_RETENTION_POLICY_ID, PRIVACY_CASE_STATE_SCHEMA_ID,
    PRIVACY_CASE_STATE_SCHEMA_VERSION, PrivacyCase, PrivacyCaseKind,
    privacy_case_state_descriptor_hash,
};
use crm_customer_privacy_capability_adapter::{
    CREATE_PRIVACY_CASE_CAPABILITY, CREATE_PRIVACY_CASE_REQUEST_SCHEMA, capability_definition,
    deterministic_privacy_case_id, privacy_case_ref_from_id,
};
use crm_customer_privacy_capability_composition::postgres_case_create_executor;
use crm_customer_privacy_persistence_adapter::privacy_case_persisted_payload;
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, CorrelationId,
    DataClass, ExecutionContext, IdempotencyKey, ModuleExecutionContext, RecordId, RequestId,
    SchemaVersion, TenantId, TraceId,
};
use crm_proto_contracts::crm::customer_privacy::v1 as wire;
use prost::Message;
use sqlx::{PgPool, Postgres, Row, Transaction};

const TENANT_A: &str = "tenant-a";
const TENANT_B: &str = "tenant-b";
const ACTOR_A: &str = "actor-a";
const ACTOR_B: &str = "actor-b";
const IDEMPOTENCY_SCOPE: &str = "capability:customer_privacy.case.create:1.0.0";

#[derive(Debug, Clone, Copy)]
struct RequestSpec<'a> {
    tenant: &'a str,
    actor: &'a str,
    identity: &'a str,
    transaction: &'a str,
    idempotency_key: &'a str,
    previous_case_id: Option<&'a str>,
    kind: wire::PrivacyCaseKind,
    started_at_unix_nanos: i64,
    hash_byte: u8,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn postgres_case_create_is_atomic_replay_safe_and_tenant_isolated() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!(
            "skipping Customer Privacy case-create process proof because DATABASE_URL is absent"
        );
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let store = PostgresDataStore::connect(&database_url, 6)
        .await
        .expect("connect Customer Privacy runtime store");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect Customer Privacy evidence reader");
    let executor = postgres_case_create_executor(store.clone());
    let definition = capability_definition().expect("construct exact case-create definition");

    let root_request = request(RequestSpec {
        tenant: TENANT_A,
        actor: ACTOR_A,
        identity: "privacy-create-root",
        transaction: "privacy-tx-root",
        idempotency_key: "privacy-idempotency-root",
        previous_case_id: None,
        kind: wire::PrivacyCaseKind::Erasure,
        started_at_unix_nanos: 1_000_000_000,
        hash_byte: 7,
    });
    let root_id = deterministic_privacy_case_id(
        TENANT_A,
        root_request.context.execution.idempotency_key.as_str(),
    )
    .unwrap();

    let first = executor
        .execute(&definition, root_request.clone())
        .await
        .expect("commit root privacy case");
    assert!(!first.replayed);
    let first_case = decode_output(first.output.as_ref().expect("case-create output"));
    assert_eq!(
        first_case
            .privacy_case_ref
            .as_ref()
            .expect("public case reference")
            .privacy_case_id,
        root_id.as_str()
    );
    assert_eq!(first_case.status, wire::PrivacyCaseStatus::Draft as i32);
    assert_eq!(first_case.version, 1);
    assert_record_metadata(&admin, TENANT_A, &root_id).await;
    assert_committed_evidence(
        &admin,
        TENANT_A,
        &root_id,
        "privacy-idempotency-root",
        "privacy-tx-root",
    )
    .await;

    let replay = executor
        .execute(&definition, root_request.clone())
        .await
        .expect("replay exact root privacy case");
    assert!(replay.replayed);
    assert_eq!(
        decode_output(replay.output.as_ref().expect("replayed output")),
        first_case
    );
    assert_committed_evidence(
        &admin,
        TENANT_A,
        &root_id,
        "privacy-idempotency-root",
        "privacy-tx-root",
    )
    .await;

    let mut conflicting_replay = root_request.clone();
    conflicting_replay.input_hash = [8; 32];
    let conflict = executor
        .execute(&definition, conflicting_replay)
        .await
        .expect_err("incompatible request hash must conflict");
    assert_eq!(conflict.code, "CAPABILITY_IDEMPOTENCY_KEY_REUSED");
    assert!(!conflict.retryable);
    assert_committed_evidence(
        &admin,
        TENANT_A,
        &root_id,
        "privacy-idempotency-root",
        "privacy-tx-root",
    )
    .await;

    make_predecessor_terminal(&admin, TENANT_A, &root_id).await;
    let successor_request = request(RequestSpec {
        tenant: TENANT_A,
        actor: ACTOR_A,
        identity: "privacy-create-successor",
        transaction: "privacy-tx-successor",
        idempotency_key: "privacy-idempotency-successor",
        previous_case_id: Some(root_id.as_str()),
        kind: wire::PrivacyCaseKind::Access,
        started_at_unix_nanos: 2_000_000_000,
        hash_byte: 9,
    });
    let successor_id = deterministic_privacy_case_id(
        TENANT_A,
        successor_request.context.execution.idempotency_key.as_str(),
    )
    .unwrap();
    let successor = executor
        .execute(&definition, successor_request)
        .await
        .expect("terminal predecessor permits successor");
    let successor_case = decode_output(successor.output.as_ref().expect("successor output"));
    assert_eq!(
        successor_case
            .previous_privacy_case_ref
            .expect("successor lineage")
            .privacy_case_id,
        root_id.as_str()
    );
    assert_eq!(
        successor_case
            .privacy_case_ref
            .expect("successor reference")
            .privacy_case_id,
        successor_id.as_str()
    );
    assert_committed_evidence(
        &admin,
        TENANT_A,
        &successor_id,
        "privacy-idempotency-successor",
        "privacy-tx-successor",
    )
    .await;

    let tenant_b_request = request(RequestSpec {
        tenant: TENANT_B,
        actor: ACTOR_B,
        identity: "privacy-create-tenant-b",
        transaction: "privacy-tx-tenant-b",
        idempotency_key: "privacy-idempotency-root",
        previous_case_id: None,
        kind: wire::PrivacyCaseKind::RestrictProcessing,
        started_at_unix_nanos: 3_000_000_000,
        hash_byte: 11,
    });
    let tenant_b_id = deterministic_privacy_case_id(
        TENANT_B,
        tenant_b_request.context.execution.idempotency_key.as_str(),
    )
    .unwrap();
    assert_ne!(root_id, tenant_b_id);
    executor
        .execute(&definition, tenant_b_request.clone())
        .await
        .expect("tenant B creates independent case");
    assert!(
        store
            .get_record(
                &tenant_b_request.context,
                &privacy_case_ref_from_id(&root_id).unwrap(),
            )
            .await
            .expect("governed cross-tenant read")
            .is_none()
    );

    let nonterminal_request = request(RequestSpec {
        tenant: TENANT_B,
        actor: ACTOR_B,
        identity: "privacy-create-nonterminal",
        transaction: "privacy-tx-nonterminal",
        idempotency_key: "privacy-idempotency-nonterminal",
        previous_case_id: Some(tenant_b_id.as_str()),
        kind: wire::PrivacyCaseKind::Access,
        started_at_unix_nanos: 3_100_000_000,
        hash_byte: 13,
    });
    let nonterminal_id = deterministic_privacy_case_id(
        TENANT_B,
        nonterminal_request.context.execution.idempotency_key.as_str(),
    )
    .unwrap();
    let nonterminal = executor
        .execute(&definition, nonterminal_request)
        .await
        .expect_err("nonterminal predecessor must fail closed");
    assert_eq!(
        nonterminal.code,
        "CUSTOMER_PRIVACY_PREVIOUS_CASE_NOT_TERMINAL"
    );
    assert_no_evidence(
        &admin,
        TENANT_B,
        &nonterminal_id,
        "privacy-idempotency-nonterminal",
        "privacy-tx-nonterminal",
    )
    .await;

    let concealed_request = request(RequestSpec {
        tenant: TENANT_B,
        actor: ACTOR_B,
        identity: "privacy-create-cross-tenant",
        transaction: "privacy-tx-cross-tenant",
        idempotency_key: "privacy-idempotency-cross-tenant",
        previous_case_id: Some(root_id.as_str()),
        kind: wire::PrivacyCaseKind::Access,
        started_at_unix_nanos: 3_200_000_000,
        hash_byte: 15,
    });
    let concealed_id = deterministic_privacy_case_id(
        TENANT_B,
        concealed_request.context.execution.idempotency_key.as_str(),
    )
    .unwrap();
    let concealed = executor
        .execute(&definition, concealed_request)
        .await
        .expect_err("cross-tenant predecessor must be concealed");
    assert_eq!(concealed.code, "CUSTOMER_PRIVACY_PREVIOUS_CASE_NOT_FOUND");
    assert_no_evidence(
        &admin,
        TENANT_B,
        &concealed_id,
        "privacy-idempotency-cross-tenant",
        "privacy-tx-cross-tenant",
    )
    .await;

    governed_replace_payload(
        &admin,
        TENANT_A,
        &root_id,
        None,
        b"{\"raw_secret\":\"must-not-leak\"}".to_vec(),
        "privacy-fixture-corrupt",
    )
    .await;
    let malformed_request = request(RequestSpec {
        tenant: TENANT_A,
        actor: ACTOR_A,
        identity: "privacy-create-malformed",
        transaction: "privacy-tx-malformed",
        idempotency_key: "privacy-idempotency-malformed",
        previous_case_id: Some(root_id.as_str()),
        kind: wire::PrivacyCaseKind::Access,
        started_at_unix_nanos: 4_000_000_000,
        hash_byte: 17,
    });
    let malformed_id = deterministic_privacy_case_id(
        TENANT_A,
        malformed_request.context.execution.idempotency_key.as_str(),
    )
    .unwrap();
    let malformed = executor
        .execute(&definition, malformed_request)
        .await
        .expect_err("malformed predecessor must fail closed");
    assert_eq!(malformed.code, "CUSTOMER_PRIVACY_PREVIOUS_CASE_INVALID");
    assert_eq!(
        malformed.safe_message,
        "The previous privacy case could not be loaded safely."
    );
    for forbidden in [
        "raw_secret",
        "must-not-leak",
        "crm.records",
        "SELECT",
        "sqlx",
    ] {
        assert!(!malformed.safe_message.contains(forbidden));
    }
    assert_no_evidence(
        &admin,
        TENANT_A,
        &malformed_id,
        "privacy-idempotency-malformed",
        "privacy-tx-malformed",
    )
    .await;
}

fn request(spec: RequestSpec<'_>) -> CapabilityRequest {
    let command = wire::CreatePrivacyCaseRequest {
        kind: spec.kind as i32,
        policy_version: "privacy-policy/1".to_owned(),
        previous_privacy_case_ref: spec.previous_case_id.map(|privacy_case_id| {
            wire::PrivacyCaseRef {
                privacy_case_id: privacy_case_id.to_owned(),
            }
        }),
    };
    CapabilityRequest {
        context: ModuleExecutionContext {
            module_id: crm_module_sdk::ModuleId::try_new(MODULE_ID).unwrap(),
            execution: ExecutionContext {
                tenant_id: TenantId::try_new(spec.tenant).unwrap(),
                actor_id: ActorId::try_new(spec.actor).unwrap(),
                request_id: RequestId::try_new(format!("request-{}", spec.identity)).unwrap(),
                correlation_id: CorrelationId::try_new(format!(
                    "correlation-{}",
                    spec.identity
                ))
                .unwrap(),
                causation_id: CausationId::try_new(format!("causation-{}", spec.identity))
                    .unwrap(),
                trace_id: TraceId::try_new(format!("trace-{}", spec.identity)).unwrap(),
                capability_id: CapabilityId::try_new(CREATE_PRIVACY_CASE_CAPABILITY).unwrap(),
                capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                idempotency_key: IdempotencyKey::try_new(spec.idempotency_key).unwrap(),
                business_transaction_id: BusinessTransactionId::try_new(spec.transaction).unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                request_started_at_unix_nanos: spec.started_at_unix_nanos,
            },
        },
        input: plan_support::protobuf_payload(
            MODULE_ID,
            CREATE_PRIVACY_CASE_REQUEST_SCHEMA,
            DataClass::Confidential,
            &command,
        )
        .unwrap(),
        input_hash: [spec.hash_byte; 32],
        approval: None,
    }
}

fn decode_output(payload: &crm_module_sdk::TypedPayload) -> wire::PrivacyCase {
    wire::CreatePrivacyCaseResponse::decode(payload.bytes.as_slice())
        .expect("decode exact CreatePrivacyCaseResponse")
        .privacy_case
        .expect("response contains privacy case")
}

async fn assert_record_metadata(admin: &PgPool, tenant: &str, case_id: &RecordId) {
    let row = sqlx::query(
        r#"
        SELECT version, owner_module_id, schema_id, schema_version, descriptor_hash,
               data_class, payload_encoding, maximum_payload_size, retention_policy_id
        FROM crm.records
        WHERE tenant_id = $1 AND record_type = $2 AND record_id = $3
        "#,
    )
    .bind(tenant)
    .bind(PRIVACY_CASE_RECORD_TYPE)
    .bind(case_id.as_str())
    .fetch_one(admin)
    .await
    .expect("read persisted privacy-case metadata");

    assert_eq!(row.get::<i64, _>("version"), 1);
    assert_eq!(row.get::<String, _>("owner_module_id"), MODULE_ID);
    assert_eq!(row.get::<String, _>("schema_id"), PRIVACY_CASE_STATE_SCHEMA_ID);
    assert_eq!(
        row.get::<String, _>("schema_version"),
        PRIVACY_CASE_STATE_SCHEMA_VERSION
    );
    assert_eq!(
        row.get::<Vec<u8>, _>("descriptor_hash"),
        privacy_case_state_descriptor_hash()
    );
    assert_eq!(row.get::<String, _>("data_class"), "confidential");
    assert_eq!(row.get::<String, _>("payload_encoding"), "json");
    assert_eq!(
        row.get::<i64, _>("maximum_payload_size"),
        i64::try_from(PRIVACY_CASE_STATE_MAXIMUM_BYTES).unwrap()
    );
    assert_eq!(
        row.get::<String, _>("retention_policy_id"),
        PRIVACY_CASE_STATE_RETENTION_POLICY_ID
    );
}

async fn assert_committed_evidence(
    admin: &PgPool,
    tenant: &str,
    case_id: &RecordId,
    idempotency_key: &str,
    transaction_id: &str,
) {
    assert_eq!(record_count(admin, tenant, case_id).await, 1);
    assert_eq!(
        evidence_count(
            admin,
            "SELECT count(*) FROM crm.outbox_events WHERE tenant_id = $1 AND business_transaction_id = $2 AND event_type = 'customer_privacy.case.created'",
            tenant,
            transaction_id,
        )
        .await,
        1
    );
    assert_eq!(
        evidence_count(
            admin,
            "SELECT count(*) FROM crm.audit_records WHERE tenant_id = $1 AND business_transaction_id = $2 AND capability_id = 'customer_privacy.case.create'",
            tenant,
            transaction_id,
        )
        .await,
        1
    );
    let idempotency_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM crm.idempotency_records WHERE tenant_id = $1 AND idempotency_scope = $2 AND idempotency_key = $3 AND status = 'completed'",
    )
    .bind(tenant)
    .bind(IDEMPOTENCY_SCOPE)
    .bind(idempotency_key)
    .fetch_one(admin)
    .await
    .expect("count idempotency evidence");
    assert_eq!(idempotency_count, 1);

    let transaction = sqlx::query(
        "SELECT expected_outbox_events, expected_audit_records, expected_idempotency_records FROM crm.business_transactions WHERE tenant_id = $1 AND business_transaction_id = $2",
    )
    .bind(tenant)
    .bind(transaction_id)
    .fetch_one(admin)
    .await
    .expect("read business transaction marker");
    assert_eq!(transaction.get::<i32, _>("expected_outbox_events"), 1);
    assert_eq!(transaction.get::<i32, _>("expected_audit_records"), 1);
    assert_eq!(transaction.get::<i32, _>("expected_idempotency_records"), 1);
}

async fn assert_no_evidence(
    admin: &PgPool,
    tenant: &str,
    case_id: &RecordId,
    idempotency_key: &str,
    transaction_id: &str,
) {
    assert_eq!(record_count(admin, tenant, case_id).await, 0);
    for sql in [
        "SELECT count(*) FROM crm.outbox_events WHERE tenant_id = $1 AND business_transaction_id = $2",
        "SELECT count(*) FROM crm.audit_records WHERE tenant_id = $1 AND business_transaction_id = $2",
        "SELECT count(*) FROM crm.business_transactions WHERE tenant_id = $1 AND business_transaction_id = $2",
    ] {
        assert_eq!(evidence_count(admin, sql, tenant, transaction_id).await, 0);
    }
    let idempotency_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM crm.idempotency_records WHERE tenant_id = $1 AND idempotency_scope = $2 AND idempotency_key = $3",
    )
    .bind(tenant)
    .bind(IDEMPOTENCY_SCOPE)
    .bind(idempotency_key)
    .fetch_one(admin)
    .await
    .expect("count rolled-back idempotency claim");
    assert_eq!(idempotency_count, 0);
}

async fn record_count(admin: &PgPool, tenant: &str, case_id: &RecordId) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM crm.records WHERE tenant_id = $1 AND record_type = $2 AND record_id = $3",
    )
    .bind(tenant)
    .bind(PRIVACY_CASE_RECORD_TYPE)
    .bind(case_id.as_str())
    .fetch_one(admin)
    .await
    .expect("count privacy cases")
}

async fn evidence_count(
    admin: &PgPool,
    sql: &'static str,
    tenant: &str,
    transaction_id: &str,
) -> i64 {
    sqlx::query_scalar(sql)
        .bind(tenant)
        .bind(transaction_id)
        .fetch_one(admin)
        .await
        .expect("count transaction evidence")
}

async fn make_predecessor_terminal(admin: &PgPool, tenant: &str, case_id: &RecordId) {
    let mut predecessor = PrivacyCase::new(
        case_id.clone(),
        TenantId::try_new(tenant).unwrap(),
        PrivacyCaseKind::Erasure,
        SchemaVersion::try_new("privacy-policy/1").unwrap(),
        1_000_000_000,
        None,
    )
    .unwrap();
    predecessor.cancel(1, 1_100_000_000).unwrap();
    let payload = privacy_case_persisted_payload(&predecessor).unwrap();
    governed_replace_payload(
        admin,
        tenant,
        case_id,
        Some(i64::try_from(predecessor.version()).unwrap()),
        payload.bytes,
        "privacy-fixture-terminal",
    )
    .await;
}

async fn governed_replace_payload(
    admin: &PgPool,
    tenant: &str,
    case_id: &RecordId,
    version: Option<i64>,
    payload_bytes: Vec<u8>,
    request_id: &str,
) {
    let business_transaction_id: String = sqlx::query_scalar(
        "SELECT last_business_transaction_id FROM crm.records WHERE tenant_id = $1 AND record_type = $2 AND record_id = $3",
    )
    .bind(tenant)
    .bind(PRIVACY_CASE_RECORD_TYPE)
    .bind(case_id.as_str())
    .fetch_one(admin)
    .await
    .expect("read predecessor transaction context");

    let mut transaction = admin.begin().await.expect("start governed fixture update");
    bind_write_context(
        &mut transaction,
        tenant,
        request_id,
        &business_transaction_id,
    )
    .await;
    let affected = if let Some(version) = version {
        sqlx::query(
            "UPDATE crm.records SET version = $4, payload_bytes = $5, updated_at = clock_timestamp() WHERE tenant_id = $1 AND record_type = $2 AND record_id = $3",
        )
        .bind(tenant)
        .bind(PRIVACY_CASE_RECORD_TYPE)
        .bind(case_id.as_str())
        .bind(version)
        .bind(payload_bytes)
        .execute(&mut *transaction)
        .await
        .expect("update canonical terminal predecessor")
        .rows_affected()
    } else {
        sqlx::query(
            "UPDATE crm.records SET payload_bytes = $4, updated_at = clock_timestamp() WHERE tenant_id = $1 AND record_type = $2 AND record_id = $3",
        )
        .bind(tenant)
        .bind(PRIVACY_CASE_RECORD_TYPE)
        .bind(case_id.as_str())
        .bind(payload_bytes)
        .execute(&mut *transaction)
        .await
        .expect("corrupt predecessor fixture")
        .rows_affected()
    };
    assert_eq!(affected, 1);
    transaction
        .commit()
        .await
        .expect("commit governed fixture update");
}

async fn bind_write_context(
    transaction: &mut Transaction<'_, Postgres>,
    tenant: &str,
    request_id: &str,
    business_transaction_id: &str,
) {
    for (name, value) in [
        ("app.tenant_id", tenant),
        ("app.actor_id", "customer-privacy-process-fixture"),
        ("app.request_id", request_id),
        ("app.capability_id", "customer_privacy.case.fixture"),
        ("app.capability_version", "1.0.0"),
        ("app.business_transaction_id", business_transaction_id),
    ] {
        sqlx::query("SELECT set_config($1, $2, true)")
            .bind(name)
            .bind(value)
            .execute(&mut **transaction)
            .await
            .expect("bind governed fixture execution context");
    }
}
