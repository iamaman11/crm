use crm_capability_plan_support as plan_support;
use crm_capability_runtime::{CapabilityRequest, TransactionalCapabilityExecutor};
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
use sqlx::{PgPool, Row};

const TENANT_A: &str = "tenant-a";
const TENANT_B: &str = "tenant-b";
const ACTOR_A: &str = "actor-a";
const ACTOR_B: &str = "actor-b";
const IDEMPOTENCY_SCOPE: &str = "capability:customer_privacy.case.create:1.0.0";

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

    let root_request = request(
        TENANT_A,
        ACTOR_A,
        "privacy-create-root",
        "privacy-tx-root",
        "privacy-idempotency-root",
        None,
        wire::PrivacyCaseKind::Erasure,
        "privacy-policy/1",
        1_000_000_000,
        7,
    );
    let expected_root_id = deterministic_privacy_case_id(
        TENANT_A,
        root_request.context.execution.idempotency_key.as_str(),
    )
    .unwrap();

    let first = executor
        .execute(&definition, root_request.clone())
        .await
        .expect("commit root privacy case");
    assert!(!first.replayed);
    let first_case = decode_output(&first.output.expect("case-create output"));
    assert_eq!(
        first_case
            .privacy_case_ref
            .as_ref()
            .expect("public case reference")
            .privacy_case_id,
        expected_root_id.as_str()
    );
    assert_eq!(first_case.status, wire::PrivacyCaseStatus::Draft as i32);
    assert_eq!(first_case.version, 1);
    assert_record_metadata(&admin, TENANT_A, &expected_root_id).await;
    assert_single_atomic_evidence(
        &admin,
        TENANT_A,
        &expected_root_id,
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
        decode_output(&replay.output.expect("replayed case-create output")),
        first_case
    );
    assert_single_atomic_evidence(
        &admin,
        TENANT_A,
        &expected_root_id,
        "privacy-idempotency-root",
        "privacy-tx-root",
    )
    .await;

    let mut conflicting_replay = root_request.clone();
    conflicting_replay.input_hash = [8; 32];
    let conflict = executor
        .execute(&definition, conflicting_replay)
        .await
        .expect_err("incompatible request hash must conflict before a second write");
    assert_eq!(conflict.code, "CAPABILITY_IDEMPOTENCY_KEY_REUSED");
    assert!(!conflict.retryable);
    assert_single_atomic_evidence(
        &admin,
        TENANT_A,
        &expected_root_id,
        "privacy-idempotency-root",
        "privacy-tx-root",
    )
    .await;

    make_predecessor_terminal(&admin, TENANT_A, &expected_root_id).await;
    let successor_request = request(
        TENANT_A,
        ACTOR_A,
        "privacy-create-successor",
        "privacy-tx-successor",
        "privacy-idempotency-successor",
        Some(expected_root_id.as_str()),
        wire::PrivacyCaseKind::Access,
        "privacy-policy/1",
        2_000_000_000,
        9,
    );
    let expected_successor_id = deterministic_privacy_case_id(
        TENANT_A,
        successor_request.context.execution.idempotency_key.as_str(),
    )
    .unwrap();
    let successor = executor
        .execute(&definition, successor_request)
        .await
        .expect("terminal predecessor permits a separate successor");
    let successor_case = decode_output(&successor.output.expect("successor output"));
    assert_eq!(
        successor_case
            .previous_privacy_case_ref
            .expect("successor lineage")
            .privacy_case_id,
        expected_root_id.as_str()
    );
    assert_eq!(
        successor_case
            .privacy_case_ref
            .expect("successor reference")
            .privacy_case_id,
        expected_successor_id.as_str()
    );
    assert_single_atomic_evidence(
        &admin,
        TENANT_A,
        &expected_successor_id,
        "privacy-idempotency-successor",
        "privacy-tx-successor",
    )
    .await;

    let tenant_b_root = request(
        TENANT_B,
        ACTOR_B,
        "privacy-create-tenant-b",
        "privacy-tx-tenant-b",
        "privacy-idempotency-root",
        None,
        wire::PrivacyCaseKind::RestrictProcessing,
        "privacy-policy/1",
        3_000_000_000,
        11,
    );
    let expected_tenant_b_id = deterministic_privacy_case_id(
        TENANT_B,
        tenant_b_root.context.execution.idempotency_key.as_str(),
    )
    .unwrap();
    assert_ne!(expected_root_id, expected_tenant_b_id);
    executor
        .execute(&definition, tenant_b_root.clone())
        .await
        .expect("tenant B creates an independent deterministic case");
    assert!(
        store
            .get_record(
                &tenant_b_root.context,
                &privacy_case_ref_from_id(&expected_root_id).unwrap(),
            )
            .await
            .expect("governed cross-tenant read")
            .is_none(),
        "tenant B must not observe tenant A's privacy case"
    );

    let nonterminal_successor = request(
        TENANT_B,
        ACTOR_B,
        "privacy-create-nonterminal",
        "privacy-tx-nonterminal",
        "privacy-idempotency-nonterminal",
        Some(expected_tenant_b_id.as_str()),
        wire::PrivacyCaseKind::Access,
        "privacy-policy/1",
        3_100_000_000,
        13,
    );
    let nonterminal_successor_id = deterministic_privacy_case_id(
        TENANT_B,
        nonterminal_successor
            .context
            .execution
            .idempotency_key
            .as_str(),
    )
    .unwrap();
    let nonterminal = executor
        .execute(&definition, nonterminal_successor)
        .await
        .expect_err("nonterminal predecessor must fail closed");
    assert_eq!(
        nonterminal.code,
        "CUSTOMER_PRIVACY_PREVIOUS_CASE_NOT_TERMINAL"
    );
    assert_no_attempt_evidence(
        &admin,
        TENANT_B,
        &nonterminal_successor_id,
        "privacy-idempotency-nonterminal",
        "privacy-tx-nonterminal",
    )
    .await;

    let cross_tenant_successor = request(
        TENANT_B,
        ACTOR_B,
        "privacy-create-cross-tenant",
        "privacy-tx-cross-tenant",
        "privacy-idempotency-cross-tenant",
        Some(expected_root_id.as_str()),
        wire::PrivacyCaseKind::Access,
        "privacy-policy/1",
        3_200_000_000,
        15,
    );
    let cross_tenant_successor_id = deterministic_privacy_case_id(
        TENANT_B,
        cross_tenant_successor
            .context
            .execution
            .idempotency_key
            .as_str(),
    )
    .unwrap();
    let concealed = executor
        .execute(&definition, cross_tenant_successor)
        .await
        .expect_err("cross-tenant predecessor must be concealed");
    assert_eq!(concealed.code, "CUSTOMER_PRIVACY_PREVIOUS_CASE_NOT_FOUND");
    assert_no_attempt_evidence(
        &admin,
        TENANT_B,
        &cross_tenant_successor_id,
        "privacy-idempotency-cross-tenant",
        "privacy-tx-cross-tenant",
    )
    .await;

    sqlx::query(
        r#"
        UPDATE crm.records
        SET payload_bytes = $3
        WHERE tenant_id = $1
          AND record_type = $2
          AND record_id = $4
        "#,
    )
    .bind(TENANT_A)
    .bind(PRIVACY_CASE_RECORD_TYPE)
    .bind(b"{\"raw_secret\":\"must-not-leak\"}".as_slice())
    .bind(expected_root_id.as_str())
    .execute(&admin)
    .await
    .expect("corrupt predecessor fixture");

    let malformed_successor = request(
        TENANT_A,
        ACTOR_A,
        "privacy-create-malformed",
        "privacy-tx-malformed",
        "privacy-idempotency-malformed",
        Some(expected_root_id.as_str()),
        wire::PrivacyCaseKind::Access,
        "privacy-policy/1",
        4_000_000_000,
        17,
    );
    let malformed_successor_id = deterministic_privacy_case_id(
        TENANT_A,
        malformed_successor
            .context
            .execution
            .idempotency_key
            .as_str(),
    )
    .unwrap();
    let malformed = executor
        .execute(&definition, malformed_successor)
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
    assert_no_attempt_evidence(
        &admin,
        TENANT_A,
        &malformed_successor_id,
        "privacy-idempotency-malformed",
        "privacy-tx-malformed",
    )
    .await;
}

fn request(
    tenant: &str,
    actor: &str,
    identity: &str,
    transaction: &str,
    idempotency_key: &str,
    previous_case_id: Option<&str>,
    kind: wire::PrivacyCaseKind,
    policy_version: &str,
    started_at_unix_nanos: i64,
    hash_byte: u8,
) -> CapabilityRequest {
    let command = wire::CreatePrivacyCaseRequest {
        kind: kind as i32,
        policy_version: policy_version.to_owned(),
        previous_privacy_case_ref: previous_case_id.map(|privacy_case_id| wire::PrivacyCaseRef {
            privacy_case_id: privacy_case_id.to_owned(),
        }),
    };
    CapabilityRequest {
        context: ModuleExecutionContext {
            module_id: crm_module_sdk::ModuleId::try_new(MODULE_ID).unwrap(),
            execution: ExecutionContext {
                tenant_id: TenantId::try_new(tenant).unwrap(),
                actor_id: ActorId::try_new(actor).unwrap(),
                request_id: RequestId::try_new(format!("request-{identity}")).unwrap(),
                correlation_id: CorrelationId::try_new(format!("correlation-{identity}")).unwrap(),
                causation_id: CausationId::try_new(format!("causation-{identity}")).unwrap(),
                trace_id: TraceId::try_new(format!("trace-{identity}")).unwrap(),
                capability_id: CapabilityId::try_new(CREATE_PRIVACY_CASE_CAPABILITY).unwrap(),
                capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                idempotency_key: IdempotencyKey::try_new(idempotency_key).unwrap(),
                business_transaction_id: BusinessTransactionId::try_new(transaction).unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                request_started_at_unix_nanos: started_at_unix_nanos,
            },
        },
        input: plan_support::protobuf_payload(
            MODULE_ID,
            CREATE_PRIVACY_CASE_REQUEST_SCHEMA,
            DataClass::Confidential,
            &command,
        )
        .unwrap(),
        input_hash: [hash_byte; 32],
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
        SELECT
          version,
          owner_module_id,
          schema_id,
          schema_version,
          descriptor_hash,
          data_class,
          payload_encoding,
          maximum_payload_size,
          retention_policy_id
        FROM crm.records
        WHERE tenant_id = $1
          AND record_type = $2
          AND record_id = $3
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
    assert_eq!(
        row.get::<String, _>("schema_id"),
        PRIVACY_CASE_STATE_SCHEMA_ID
    );
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

async fn assert_single_atomic_evidence(
    admin: &PgPool,
    tenant: &str,
    case_id: &RecordId,
    idempotency_key: &str,
    transaction_id: &str,
) {
    assert_eq!(record_count(admin, tenant, case_id).await, 1);
    assert_eq!(
        scalar_count(
            admin,
            "SELECT count(*) FROM crm.outbox_events WHERE tenant_id = $1 AND business_transaction_id = $2 AND event_type = 'customer_privacy.case.created'",
            tenant,
            transaction_id,
        )
        .await,
        1
    );
    assert_eq!(
        scalar_count(
            admin,
            "SELECT count(*) FROM crm.audit_records WHERE tenant_id = $1 AND business_transaction_id = $2 AND capability_id = 'customer_privacy.case.create'",
            tenant,
            transaction_id,
        )
        .await,
        1
    );
    let idempotency_count: i64 = sqlx::query_scalar(
        r#"
        SELECT count(*)
        FROM crm.idempotency_records
        WHERE tenant_id = $1
          AND idempotency_scope = $2
          AND idempotency_key = $3
          AND status = 'completed'
        "#,
    )
    .bind(tenant)
    .bind(IDEMPOTENCY_SCOPE)
    .bind(idempotency_key)
    .fetch_one(admin)
    .await
    .expect("count idempotency evidence");
    assert_eq!(idempotency_count, 1);

    let transaction = sqlx::query(
        r#"
        SELECT expected_outbox_events, expected_audit_records, expected_idempotency_records
        FROM crm.business_transactions
        WHERE tenant_id = $1 AND business_transaction_id = $2
        "#,
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

async fn assert_no_attempt_evidence(
    admin: &PgPool,
    tenant: &str,
    case_id: &RecordId,
    idempotency_key: &str,
    transaction_id: &str,
) {
    assert_eq!(record_count(admin, tenant, case_id).await, 0);
    assert_eq!(
        scalar_count(
            admin,
            "SELECT count(*) FROM crm.outbox_events WHERE tenant_id = $1 AND business_transaction_id = $2",
            tenant,
            transaction_id,
        )
        .await,
        0
    );
    assert_eq!(
        scalar_count(
            admin,
            "SELECT count(*) FROM crm.audit_records WHERE tenant_id = $1 AND business_transaction_id = $2",
            tenant,
            transaction_id,
        )
        .await,
        0
    );
    let idempotency_count: i64 = sqlx::query_scalar(
        r#"
        SELECT count(*)
        FROM crm.idempotency_records
        WHERE tenant_id = $1
          AND idempotency_scope = $2
          AND idempotency_key = $3
        "#,
    )
    .bind(tenant)
    .bind(IDEMPOTENCY_SCOPE)
    .bind(idempotency_key)
    .fetch_one(admin)
    .await
    .expect("count rolled-back idempotency claim");
    assert_eq!(idempotency_count, 0);
    assert_eq!(
        scalar_count(
            admin,
            "SELECT count(*) FROM crm.business_transactions WHERE tenant_id = $1 AND business_transaction_id = $2",
            tenant,
            transaction_id,
        )
        .await,
        0
    );
}

async fn record_count(admin: &PgPool, tenant: &str, case_id: &RecordId) -> i64 {
    sqlx::query_scalar(
        r#"
        SELECT count(*)
        FROM crm.records
        WHERE tenant_id = $1
          AND record_type = $2
          AND record_id = $3
        "#,
    )
    .bind(tenant)
    .bind(PRIVACY_CASE_RECORD_TYPE)
    .bind(case_id.as_str())
    .fetch_one(admin)
    .await
    .expect("count privacy cases")
}

async fn scalar_count(admin: &PgPool, sql: &str, tenant: &str, transaction_id: &str) -> i64 {
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
    let affected = sqlx::query(
        r#"
        UPDATE crm.records
        SET version = $4, payload_bytes = $5
        WHERE tenant_id = $1
          AND record_type = $2
          AND record_id = $3
        "#,
    )
    .bind(tenant)
    .bind(PRIVACY_CASE_RECORD_TYPE)
    .bind(case_id.as_str())
    .bind(i64::try_from(predecessor.version()).unwrap())
    .bind(payload.bytes)
    .execute(admin)
    .await
    .expect("seed canonical terminal predecessor")
    .rows_affected();
    assert_eq!(affected, 1);
}
