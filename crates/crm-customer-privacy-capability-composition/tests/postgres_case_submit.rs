use crm_capability_plan_support as plan_support;
use crm_capability_runtime::CapabilityRequest;
use crm_core_data::PostgresDataStore;
use crm_customer_privacy::{MODULE_ID, PRIVACY_CASE_RECORD_TYPE};
use crm_customer_privacy_capability_adapter::{
    CREATE_PRIVACY_CASE_CAPABILITY, CREATE_PRIVACY_CASE_REQUEST_SCHEMA,
    capability_definition as create_definition, deterministic_privacy_case_id,
    privacy_case_ref_from_id,
};
use crm_customer_privacy_capability_composition::{
    postgres_case_create_executor, postgres_case_submit_executor,
};
use crm_customer_privacy_persistence_adapter::privacy_case_from_snapshot;
use crm_customer_privacy_submit_capability_adapter::{
    SUBMIT_PRIVACY_CASE_CAPABILITY, SUBMIT_PRIVACY_CASE_REQUEST_SCHEMA,
    capability_definition as submit_definition,
};
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
const ACTOR: &str = "privacy-officer";
const SUBMIT_SCOPE: &str = "capability:customer_privacy.case.submit:1.0.0";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn postgres_case_submit_is_atomic_replay_safe_and_fail_closed() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!(
            "skipping Customer Privacy case-submit process proof because DATABASE_URL is absent"
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
    let create_executor = postgres_case_create_executor(store.clone());
    let submit_executor = postgres_case_submit_executor(store.clone());
    let create_definition = create_definition().expect("construct case-create definition");
    let submit_definition = submit_definition().expect("construct case-submit definition");

    let create = create_request(
        TENANT_A,
        "submit-proof-create",
        "privacy-submit-create-key",
        "privacy-submit-create-tx",
        1_000_000_000,
        3,
    );
    let case_id =
        deterministic_privacy_case_id(TENANT_A, create.context.execution.idempotency_key.as_str())
            .unwrap();
    create_executor
        .execute(&create_definition, create)
        .await
        .expect("create draft case for submit proof");

    let submit = submit_request(
        TENANT_A,
        &case_id,
        1,
        "submit-proof-success",
        "privacy-submit-key",
        "privacy-submit-tx",
        2_000_000_000,
        7,
    );
    let first = submit_executor
        .execute(&submit_definition, submit.clone())
        .await
        .expect("submit draft privacy case");
    assert!(!first.replayed);
    let first_case = decode_submit(first.output.as_ref().expect("submit output"));
    assert_eq!(first_case.status, wire::PrivacyCaseStatus::Submitted as i32);
    assert_eq!(first_case.version, 2);
    assert_eq!(first_case.updated_at_unix_ms, 2_000);
    assert_submitted_record(&store, &submit, &case_id).await;
    assert_submit_evidence(
        &admin,
        TENANT_A,
        &case_id,
        "privacy-submit-key",
        "privacy-submit-tx",
    )
    .await;

    let replay = submit_executor
        .execute(&submit_definition, submit.clone())
        .await
        .expect("exact submit replay");
    assert!(replay.replayed);
    assert_eq!(
        decode_submit(replay.output.as_ref().expect("replayed submit output")),
        first_case
    );
    assert_submitted_record(&store, &submit, &case_id).await;
    assert_submit_evidence(
        &admin,
        TENANT_A,
        &case_id,
        "privacy-submit-key",
        "privacy-submit-tx",
    )
    .await;

    let mut conflicting = submit.clone();
    conflicting.input_hash = [8; 32];
    let conflict = submit_executor
        .execute(&submit_definition, conflicting)
        .await
        .expect_err("incompatible submit replay must conflict");
    assert_eq!(conflict.code, "CAPABILITY_IDEMPOTENCY_KEY_REUSED");
    assert!(!conflict.retryable);
    assert_submit_evidence(
        &admin,
        TENANT_A,
        &case_id,
        "privacy-submit-key",
        "privacy-submit-tx",
    )
    .await;

    let stale_id = create_draft(
        &create_executor,
        &create_definition,
        TENANT_A,
        "submit-proof-stale-create",
        "privacy-submit-stale-create-key",
        "privacy-submit-stale-create-tx",
        3_000_000_000,
    )
    .await;
    let stale = submit_request(
        TENANT_A,
        &stale_id,
        2,
        "submit-proof-stale",
        "privacy-submit-stale-key",
        "privacy-submit-stale-tx",
        4_000_000_000,
        11,
    );
    let stale_error = submit_executor
        .execute(&submit_definition, stale)
        .await
        .expect_err("stale expected version must fail closed");
    assert_eq!(stale_error.code, "CUSTOMER_PRIVACY_VERSION_CONFLICT");
    assert!(stale_error.retryable);
    assert_record_version(&admin, TENANT_A, &stale_id, 1).await;
    assert_no_submit_evidence(
        &admin,
        TENANT_A,
        "privacy-submit-stale-key",
        "privacy-submit-stale-tx",
    )
    .await;

    let wrong_state = submit_request(
        TENANT_A,
        &case_id,
        2,
        "submit-proof-wrong-state",
        "privacy-submit-wrong-state-key",
        "privacy-submit-wrong-state-tx",
        5_000_000_000,
        13,
    );
    let wrong_state_error = submit_executor
        .execute(&submit_definition, wrong_state)
        .await
        .expect_err("submitted case cannot be submitted again under a new key");
    assert_eq!(
        wrong_state_error.code,
        "CUSTOMER_PRIVACY_INVALID_TRANSITION"
    );
    assert_record_version(&admin, TENANT_A, &case_id, 2).await;
    assert_no_submit_evidence(
        &admin,
        TENANT_A,
        "privacy-submit-wrong-state-key",
        "privacy-submit-wrong-state-tx",
    )
    .await;

    let cross_tenant = submit_request(
        TENANT_B,
        &case_id,
        1,
        "submit-proof-cross-tenant",
        "privacy-submit-cross-tenant-key",
        "privacy-submit-cross-tenant-tx",
        6_000_000_000,
        17,
    );
    let concealed = submit_executor
        .execute(&submit_definition, cross_tenant)
        .await
        .expect_err("cross-tenant case must be concealed");
    assert!(!concealed.retryable);
    assert_no_submit_evidence(
        &admin,
        TENANT_B,
        "privacy-submit-cross-tenant-key",
        "privacy-submit-cross-tenant-tx",
    )
    .await;

    let malformed_id = create_draft(
        &create_executor,
        &create_definition,
        TENANT_A,
        "submit-proof-malformed-create",
        "privacy-submit-malformed-create-key",
        "privacy-submit-malformed-create-tx",
        7_000_000_000,
    )
    .await;
    corrupt_record_payload(&admin, TENANT_A, &malformed_id).await;
    let malformed = submit_request(
        TENANT_A,
        &malformed_id,
        1,
        "submit-proof-malformed",
        "privacy-submit-malformed-key",
        "privacy-submit-malformed-tx",
        8_000_000_000,
        19,
    );
    let malformed_error = submit_executor
        .execute(&submit_definition, malformed)
        .await
        .expect_err("malformed canonical case must fail closed");
    assert_eq!(malformed_error.code, "CUSTOMER_PRIVACY_CASE_INVALID");
    assert_eq!(
        malformed_error.safe_message,
        "The privacy case could not be loaded safely."
    );
    for forbidden in [
        "raw_secret",
        "must-not-leak",
        "crm.records",
        "sqlx",
        "SELECT",
    ] {
        assert!(!malformed_error.safe_message.contains(forbidden));
    }
    assert_record_version(&admin, TENANT_A, &malformed_id, 1).await;
    assert_no_submit_evidence(
        &admin,
        TENANT_A,
        "privacy-submit-malformed-key",
        "privacy-submit-malformed-tx",
    )
    .await;
}

async fn create_draft(
    executor: &std::sync::Arc<dyn crm_capability_runtime::TransactionalCapabilityExecutor>,
    definition: &crm_capability_runtime::CapabilityDefinition,
    tenant: &str,
    identity: &str,
    idempotency_key: &str,
    transaction: &str,
    started_at: i64,
) -> RecordId {
    let request = create_request(
        tenant,
        identity,
        idempotency_key,
        transaction,
        started_at,
        5,
    );
    let id =
        deterministic_privacy_case_id(tenant, request.context.execution.idempotency_key.as_str())
            .unwrap();
    executor
        .execute(definition, request)
        .await
        .expect("create draft case");
    id
}

fn create_request(
    tenant: &str,
    identity: &str,
    idempotency_key: &str,
    transaction: &str,
    started_at: i64,
    hash_byte: u8,
) -> CapabilityRequest {
    capability_request(
        tenant,
        identity,
        idempotency_key,
        transaction,
        started_at,
        hash_byte,
        CREATE_PRIVACY_CASE_CAPABILITY,
        CREATE_PRIVACY_CASE_REQUEST_SCHEMA,
        &wire::CreatePrivacyCaseRequest {
            kind: wire::PrivacyCaseKind::Erasure as i32,
            policy_version: "privacy-policy/1".to_owned(),
            previous_privacy_case_ref: None,
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn submit_request(
    tenant: &str,
    case_id: &RecordId,
    expected_version: i64,
    identity: &str,
    idempotency_key: &str,
    transaction: &str,
    started_at: i64,
    hash_byte: u8,
) -> CapabilityRequest {
    capability_request(
        tenant,
        identity,
        idempotency_key,
        transaction,
        started_at,
        hash_byte,
        SUBMIT_PRIVACY_CASE_CAPABILITY,
        SUBMIT_PRIVACY_CASE_REQUEST_SCHEMA,
        &wire::SubmitPrivacyCaseRequest {
            privacy_case_ref: Some(wire::PrivacyCaseRef {
                privacy_case_id: case_id.as_str().to_owned(),
            }),
            expected_version,
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn capability_request<M: Message>(
    tenant: &str,
    identity: &str,
    idempotency_key: &str,
    transaction: &str,
    started_at: i64,
    hash_byte: u8,
    capability_id: &str,
    request_schema: &str,
    command: &M,
) -> CapabilityRequest {
    CapabilityRequest {
        context: ModuleExecutionContext {
            module_id: crm_module_sdk::ModuleId::try_new(MODULE_ID).unwrap(),
            execution: ExecutionContext {
                tenant_id: TenantId::try_new(tenant).unwrap(),
                actor_id: ActorId::try_new(ACTOR).unwrap(),
                request_id: RequestId::try_new(format!("request-{identity}")).unwrap(),
                correlation_id: CorrelationId::try_new(format!("correlation-{identity}")).unwrap(),
                causation_id: CausationId::try_new(format!("causation-{identity}")).unwrap(),
                trace_id: TraceId::try_new(format!("trace-{identity}")).unwrap(),
                capability_id: CapabilityId::try_new(capability_id).unwrap(),
                capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                idempotency_key: IdempotencyKey::try_new(idempotency_key).unwrap(),
                business_transaction_id: BusinessTransactionId::try_new(transaction).unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                request_started_at_unix_nanos: started_at,
            },
        },
        input: plan_support::protobuf_payload(
            MODULE_ID,
            request_schema,
            DataClass::Confidential,
            command,
        )
        .unwrap(),
        input_hash: [hash_byte; 32],
        approval: None,
    }
}

fn decode_submit(payload: &crm_module_sdk::TypedPayload) -> wire::PrivacyCase {
    wire::SubmitPrivacyCaseResponse::decode(payload.bytes.as_slice())
        .expect("decode exact SubmitPrivacyCaseResponse")
        .privacy_case
        .expect("submit response contains privacy case")
}

async fn assert_submitted_record(
    store: &PostgresDataStore,
    request: &CapabilityRequest,
    case_id: &RecordId,
) {
    let snapshot = store
        .get_record(
            &request.context,
            &privacy_case_ref_from_id(case_id).unwrap(),
        )
        .await
        .expect("read submitted case")
        .expect("submitted case exists");
    assert_eq!(snapshot.version, 2);
    let case = privacy_case_from_snapshot(&snapshot).expect("strictly rehydrate submitted case");
    assert_eq!(
        case.status(),
        crm_customer_privacy::PrivacyCaseStatus::Submitted
    );
    assert_eq!(case.version(), 2);
}

async fn assert_submit_evidence(
    admin: &PgPool,
    tenant: &str,
    case_id: &RecordId,
    idempotency_key: &str,
    transaction_id: &str,
) {
    assert_record_version(admin, tenant, case_id, 2).await;
    assert_eq!(
        count_transaction_rows(
            admin,
            "SELECT count(*) FROM crm.outbox_events WHERE tenant_id = $1 AND business_transaction_id = $2 AND event_type = 'customer_privacy.case.status_changed'",
            tenant,
            transaction_id,
        )
        .await,
        1
    );
    assert_eq!(
        count_transaction_rows(
            admin,
            "SELECT count(*) FROM crm.audit_records WHERE tenant_id = $1 AND business_transaction_id = $2 AND capability_id = 'customer_privacy.case.submit'",
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
    .bind(SUBMIT_SCOPE)
    .bind(idempotency_key)
    .fetch_one(admin)
    .await
    .expect("count completed submit idempotency");
    assert_eq!(idempotency_count, 1);

    let marker = sqlx::query(
        "SELECT expected_outbox_events, expected_audit_records, expected_idempotency_records FROM crm.business_transactions WHERE tenant_id = $1 AND business_transaction_id = $2",
    )
    .bind(tenant)
    .bind(transaction_id)
    .fetch_one(admin)
    .await
    .expect("read submit business transaction marker");
    assert_eq!(marker.get::<i32, _>("expected_outbox_events"), 1);
    assert_eq!(marker.get::<i32, _>("expected_audit_records"), 1);
    assert_eq!(marker.get::<i32, _>("expected_idempotency_records"), 1);
}

async fn assert_no_submit_evidence(
    admin: &PgPool,
    tenant: &str,
    idempotency_key: &str,
    transaction_id: &str,
) {
    for sql in [
        "SELECT count(*) FROM crm.outbox_events WHERE tenant_id = $1 AND business_transaction_id = $2",
        "SELECT count(*) FROM crm.audit_records WHERE tenant_id = $1 AND business_transaction_id = $2",
        "SELECT count(*) FROM crm.business_transactions WHERE tenant_id = $1 AND business_transaction_id = $2",
    ] {
        assert_eq!(
            count_transaction_rows(admin, sql, tenant, transaction_id).await,
            0
        );
    }
    let idempotency_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM crm.idempotency_records WHERE tenant_id = $1 AND idempotency_scope = $2 AND idempotency_key = $3",
    )
    .bind(tenant)
    .bind(SUBMIT_SCOPE)
    .bind(idempotency_key)
    .fetch_one(admin)
    .await
    .expect("count rolled-back submit idempotency");
    assert_eq!(idempotency_count, 0);
}

async fn assert_record_version(admin: &PgPool, tenant: &str, case_id: &RecordId, version: i64) {
    let actual: i64 = sqlx::query_scalar(
        "SELECT version FROM crm.records WHERE tenant_id = $1 AND record_type = $2 AND record_id = $3",
    )
    .bind(tenant)
    .bind(PRIVACY_CASE_RECORD_TYPE)
    .bind(case_id.as_str())
    .fetch_one(admin)
    .await
    .expect("read privacy-case version");
    assert_eq!(actual, version);
}

async fn count_transaction_rows(
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

async fn corrupt_record_payload(admin: &PgPool, tenant: &str, case_id: &RecordId) {
    let business_transaction_id: String = sqlx::query_scalar(
        "SELECT last_business_transaction_id FROM crm.records WHERE tenant_id = $1 AND record_type = $2 AND record_id = $3",
    )
    .bind(tenant)
    .bind(PRIVACY_CASE_RECORD_TYPE)
    .bind(case_id.as_str())
    .fetch_one(admin)
    .await
    .expect("read malformed fixture transaction context");
    let mut transaction = admin
        .begin()
        .await
        .expect("start governed malformed update");
    bind_write_context(
        &mut transaction,
        tenant,
        "privacy-submit-malformed-fixture",
        &business_transaction_id,
    )
    .await;
    sqlx::query(
        "UPDATE crm.records SET payload_bytes = $4, updated_at = clock_timestamp() WHERE tenant_id = $1 AND record_type = $2 AND record_id = $3",
    )
    .bind(tenant)
    .bind(PRIVACY_CASE_RECORD_TYPE)
    .bind(case_id.as_str())
    .bind(b"{\"raw_secret\":\"must-not-leak\"}".as_slice())
    .execute(&mut *transaction)
    .await
    .expect("corrupt submit fixture payload");
    transaction
        .commit()
        .await
        .expect("commit malformed submit fixture");
}

async fn bind_write_context(
    transaction: &mut Transaction<'_, Postgres>,
    tenant: &str,
    request_id: &str,
    business_transaction_id: &str,
) {
    for (name, value) in [
        ("app.tenant_id", tenant),
        ("app.actor_id", "customer-privacy-submit-fixture"),
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
            .expect("bind governed submit fixture context");
    }
}
