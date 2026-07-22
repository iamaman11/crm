use crm_capability_plan_support as plan_support;
use crm_capability_runtime::{CapabilityRequest, TransactionalCapabilityExecutor};
use crm_core_data::{PostgresDataStore, PostgresTransactionalAggregateExecutor};
use crm_customer_privacy::{MODULE_ID, PRIVACY_CASE_RECORD_TYPE, PrivacyCaseStatus};
use crm_customer_privacy_capability_adapter::{
    CREATE_PRIVACY_CASE_CAPABILITY, CREATE_PRIVACY_CASE_REQUEST_SCHEMA,
    capability_definition as create_definition, deterministic_privacy_case_id,
    privacy_case_ref_from_id,
};
use crm_customer_privacy_capability_composition::{
    postgres_case_create_executor, postgres_case_submit_executor,
    postgres_case_subject_verify_executor,
};
use crm_customer_privacy_persistence_adapter::privacy_case_from_snapshot;
use crm_customer_privacy_subject_capability_adapter::{
    VERIFY_PRIVACY_CASE_SUBJECT_CAPABILITY, VERIFY_PRIVACY_CASE_SUBJECT_REQUEST_SCHEMA,
    capability_definition as verify_definition,
};
use crm_customer_privacy_submit_capability_adapter::{
    SUBMIT_PRIVACY_CASE_CAPABILITY, SUBMIT_PRIVACY_CASE_REQUEST_SCHEMA,
    capability_definition as submit_definition,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, CorrelationId,
    DataClass, ExecutionContext, IdempotencyKey, ModuleExecutionContext, RecordId, RequestId,
    SchemaVersion, TenantId, TraceId,
};
use crm_parties_capability_adapter::{
    CREATE_CAPABILITY as CREATE_PARTY_CAPABILITY, CREATE_REQUEST_SCHEMA as CREATE_PARTY_REQUEST_SCHEMA,
    PartyCapabilityPlanner, capability_definition as party_definition,
};
use crm_proto_contracts::crm::{
    customer::v1 as customer_wire, customer_privacy::v1 as wire, parties::v1 as parties_wire,
};
use prost::Message;
use sqlx::{PgPool, Postgres, Row, Transaction};
use std::sync::Arc;

const TENANT_A: &str = "tenant-a";
const TENANT_B: &str = "tenant-b";
const ACTOR: &str = "privacy-officer";
const VERIFY_SCOPE: &str = "capability:customer_privacy.case.subject.verify:1.0.0";

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_subject_verification_is_authoritative_atomic_and_replay_safe() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!(
            "skipping Customer Privacy subject-verification process proof because DATABASE_URL is absent"
        );
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let store = PostgresDataStore::connect(&database_url, 8)
        .await
        .expect("connect Customer Privacy subject runtime store");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect Customer Privacy subject evidence reader");

    let party_executor: Arc<dyn TransactionalCapabilityExecutor> = Arc::new(
        PostgresTransactionalAggregateExecutor::new(
            store.clone(),
            Arc::new(PartyCapabilityPlanner),
        ),
    );
    let create_executor = postgres_case_create_executor(store.clone());
    let submit_executor = postgres_case_submit_executor(store.clone());
    let verify_executor = postgres_case_subject_verify_executor(store.clone());
    let party_definition =
        party_definition(CREATE_PARTY_CAPABILITY).expect("construct Party create definition");
    let create_definition = create_definition().expect("construct case-create definition");
    let submit_definition = submit_definition().expect("construct case-submit definition");
    let verify_definition = verify_definition().expect("construct subject-verify definition");

    for (party_id, hash_byte) in [
        ("party-submitted", 31),
        ("party-canonical", 32),
        ("party-other", 33),
    ] {
        create_party(
            &party_executor,
            &party_definition,
            TENANT_A,
            party_id,
            hash_byte,
        )
        .await;
    }

    let case_id = create_and_submit(
        &create_executor,
        &create_definition,
        &submit_executor,
        &submit_definition,
        "subject-success",
        40,
    )
    .await;
    let verify = verify_request(
        TENANT_A,
        &case_id,
        2,
        "party-submitted",
        "party-submitted",
        1,
        "subject-success-verify",
        "privacy-subject-success-key",
        "privacy-subject-success-tx",
        3_000_000_000,
        41,
    );
    let first = verify_executor
        .execute(&verify_definition, verify.clone())
        .await
        .expect("verify subject at authoritative self-root");
    assert!(!first.replayed);
    let first_case = decode_verify(first.output.as_ref().expect("subject verify output"));
    assert_eq!(
        first_case.status,
        wire::PrivacyCaseStatus::SubjectVerified as i32
    );
    assert_eq!(first_case.version, 3);
    assert_verified_record(&store, &verify, &case_id, "party-submitted", 1).await;
    assert_verify_evidence(
        &admin,
        TENANT_A,
        &case_id,
        "privacy-subject-success-key",
        "privacy-subject-success-tx",
    )
    .await;

    let replay = verify_executor
        .execute(&verify_definition, verify.clone())
        .await
        .expect("exact subject verification replay");
    assert!(replay.replayed);
    assert_eq!(
        decode_verify(replay.output.as_ref().expect("replayed verify output")),
        first_case
    );
    assert_verify_evidence(
        &admin,
        TENANT_A,
        &case_id,
        "privacy-subject-success-key",
        "privacy-subject-success-tx",
    )
    .await;

    let mut conflicting = verify.clone();
    conflicting.input_hash = [42; 32];
    let conflict = verify_executor
        .execute(&verify_definition, conflicting)
        .await
        .expect_err("incompatible subject verification replay must conflict");
    assert_eq!(conflict.code, "CAPABILITY_IDEMPOTENCY_KEY_REUSED");
    assert!(!conflict.retryable);
    assert_verify_evidence(
        &admin,
        TENANT_A,
        &case_id,
        "privacy-subject-success-key",
        "privacy-subject-success-tx",
    )
    .await;

    let stale_version_case = create_and_submit(
        &create_executor,
        &create_definition,
        &submit_executor,
        &submit_definition,
        "subject-stale-version",
        50,
    )
    .await;
    assert_failed_verify(
        &verify_executor,
        &verify_definition,
        &admin,
        verify_request(
            TENANT_A,
            &stale_version_case,
            3,
            "party-submitted",
            "party-submitted",
            1,
            "subject-stale-version-verify",
            "privacy-subject-stale-version-key",
            "privacy-subject-stale-version-tx",
            6_000_000_000,
            51,
        ),
        "CUSTOMER_PRIVACY_VERSION_CONFLICT",
        true,
        2,
    )
    .await;

    let draft_case = create_case(
        &create_executor,
        &create_definition,
        "subject-wrong-state",
        60,
    )
    .await;
    assert_failed_verify(
        &verify_executor,
        &verify_definition,
        &admin,
        verify_request(
            TENANT_A,
            &draft_case,
            1,
            "party-submitted",
            "party-submitted",
            1,
            "subject-wrong-state-verify",
            "privacy-subject-wrong-state-key",
            "privacy-subject-wrong-state-tx",
            8_000_000_000,
            61,
        ),
        "CUSTOMER_PRIVACY_INVALID_TRANSITION",
        false,
        1,
    )
    .await;

    for (identity, submitted, canonical, expected_code, hash_byte) in [
        (
            "subject-missing-submitted",
            "party-missing",
            "party-submitted",
            "CUSTOMER_PRIVACY_SUBJECT_REFERENCE_UNAVAILABLE",
            70,
        ),
        (
            "subject-missing-canonical",
            "party-submitted",
            "party-missing",
            "CUSTOMER_PRIVACY_SUBJECT_REFERENCE_UNAVAILABLE",
            71,
        ),
        (
            "subject-invalid-canonical",
            "party-submitted",
            "party-canonical",
            "CUSTOMER_PRIVACY_SUBJECT_CANONICAL_REFERENCE_INVALID",
            72,
        ),
    ] {
        let case_id = create_and_submit(
            &create_executor,
            &create_definition,
            &submit_executor,
            &submit_definition,
            identity,
            hash_byte,
        )
        .await;
        assert_failed_verify(
            &verify_executor,
            &verify_definition,
            &admin,
            verify_request(
                TENANT_A,
                &case_id,
                2,
                submitted,
                canonical,
                1,
                &format!("{identity}-verify"),
                &format!("privacy-{identity}-key"),
                &format!("privacy-{identity}-tx"),
                10_000_000_000 + i64::from(hash_byte),
                hash_byte + 1,
            ),
            expected_code,
            false,
            2,
        )
        .await;
    }

    let stale_generation_case = create_and_submit(
        &create_executor,
        &create_definition,
        &submit_executor,
        &submit_definition,
        "subject-stale-generation",
        80,
    )
    .await;
    assert_failed_verify(
        &verify_executor,
        &verify_definition,
        &admin,
        verify_request(
            TENANT_A,
            &stale_generation_case,
            2,
            "party-submitted",
            "party-submitted",
            2,
            "subject-stale-generation-verify",
            "privacy-subject-stale-generation-key",
            "privacy-subject-stale-generation-tx",
            12_000_000_000,
            81,
        ),
        "CUSTOMER_PRIVACY_SUBJECT_GENERATION_STALE",
        true,
        2,
    )
    .await;

    let cross_tenant_case = create_and_submit(
        &create_executor,
        &create_definition,
        &submit_executor,
        &submit_definition,
        "subject-cross-tenant",
        90,
    )
    .await;
    assert_failed_verify(
        &verify_executor,
        &verify_definition,
        &admin,
        verify_request(
            TENANT_B,
            &cross_tenant_case,
            2,
            "party-submitted",
            "party-submitted",
            1,
            "subject-cross-tenant-verify",
            "privacy-subject-cross-tenant-key",
            "privacy-subject-cross-tenant-tx",
            14_000_000_000,
            91,
        ),
        "CUSTOMER_PRIVACY_SUBJECT_REFERENCE_UNAVAILABLE",
        false,
        2,
    )
    .await;

    let malformed_case = create_and_submit(
        &create_executor,
        &create_definition,
        &submit_executor,
        &submit_definition,
        "subject-malformed",
        100,
    )
    .await;
    corrupt_record_payload(&admin, TENANT_A, &malformed_case).await;
    assert_failed_verify(
        &verify_executor,
        &verify_definition,
        &admin,
        verify_request(
            TENANT_A,
            &malformed_case,
            2,
            "party-submitted",
            "party-submitted",
            1,
            "subject-malformed-verify",
            "privacy-subject-malformed-key",
            "privacy-subject-malformed-tx",
            16_000_000_000,
            101,
        ),
        "CUSTOMER_PRIVACY_CASE_INVALID",
        false,
        2,
    )
    .await;

    let contended_case = create_and_submit(
        &create_executor,
        &create_definition,
        &submit_executor,
        &submit_definition,
        "subject-lock-contention",
        110,
    )
    .await;
    let mut lock_holder = admin.begin().await.expect("start subject lock holder");
    bind_context(
        &mut lock_holder,
        TENANT_A,
        "subject-lock-holder",
        "privacy-subject-lock-holder-tx",
    )
    .await;
    sqlx::query("SELECT crm.lock_customer_subject($1, $2)")
        .bind(TENANT_A)
        .bind("party-submitted")
        .execute(&mut *lock_holder)
        .await
        .expect("hold exact shared subject lock");
    assert_failed_verify(
        &verify_executor,
        &verify_definition,
        &admin,
        verify_request(
            TENANT_A,
            &contended_case,
            2,
            "party-submitted",
            "party-submitted",
            1,
            "subject-lock-contention-verify",
            "privacy-subject-lock-contention-key",
            "privacy-subject-lock-contention-tx",
            18_000_000_000,
            111,
        ),
        "CUSTOMER_PRIVACY_SUBJECT_LOCK_UNAVAILABLE",
        true,
        2,
    )
    .await;
    lock_holder.rollback().await.expect("release subject lock");
}

async fn create_party(
    executor: &Arc<dyn TransactionalCapabilityExecutor>,
    definition: &crm_capability_runtime::CapabilityDefinition,
    tenant: &str,
    party_id: &str,
    hash_byte: u8,
) {
    let command = parties_wire::CreatePartyRequest {
        party_ref: Some(customer_wire::PartyRef {
            party_id: party_id.to_owned(),
        }),
        kind: parties_wire::PartyKind::Individual as i32,
        display_name: format!("Subject fixture {party_id}"),
    };
    executor
        .execute(
            definition,
            capability_request(
                tenant,
                &format!("party-{party_id}"),
                &format!("party-{party_id}-key"),
                &format!("party-{party_id}-tx"),
                500_000_000 + i64::from(hash_byte),
                hash_byte,
                "crm.parties",
                CREATE_PARTY_CAPABILITY,
                CREATE_PARTY_REQUEST_SCHEMA,
                DataClass::Personal,
                &command,
            ),
        )
        .await
        .expect("create authoritative Party fixture");
}

async fn create_and_submit(
    create_executor: &Arc<dyn TransactionalCapabilityExecutor>,
    create_definition: &crm_capability_runtime::CapabilityDefinition,
    submit_executor: &Arc<dyn TransactionalCapabilityExecutor>,
    submit_definition: &crm_capability_runtime::CapabilityDefinition,
    identity: &str,
    hash_byte: u8,
) -> RecordId {
    let case_id = create_case(create_executor, create_definition, identity, hash_byte).await;
    submit_executor
        .execute(
            submit_definition,
            submit_request(
                TENANT_A,
                &case_id,
                1,
                &format!("{identity}-submit"),
                &format!("privacy-{identity}-submit-key"),
                &format!("privacy-{identity}-submit-tx"),
                2_000_000_000 + i64::from(hash_byte),
                hash_byte + 1,
            ),
        )
        .await
        .expect("submit privacy case fixture");
    case_id
}

async fn create_case(
    executor: &Arc<dyn TransactionalCapabilityExecutor>,
    definition: &crm_capability_runtime::CapabilityDefinition,
    identity: &str,
    hash_byte: u8,
) -> RecordId {
    let request = create_request(
        TENANT_A,
        &format!("{identity}-create"),
        &format!("privacy-{identity}-create-key"),
        &format!("privacy-{identity}-create-tx"),
        1_000_000_000 + i64::from(hash_byte),
        hash_byte,
    );
    let case_id =
        deterministic_privacy_case_id(TENANT_A, request.context.execution.idempotency_key.as_str())
            .expect("derive deterministic privacy case id");
    executor
        .execute(definition, request)
        .await
        .expect("create privacy case fixture");
    case_id
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
        MODULE_ID,
        CREATE_PRIVACY_CASE_CAPABILITY,
        CREATE_PRIVACY_CASE_REQUEST_SCHEMA,
        DataClass::Confidential,
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
        MODULE_ID,
        SUBMIT_PRIVACY_CASE_CAPABILITY,
        SUBMIT_PRIVACY_CASE_REQUEST_SCHEMA,
        DataClass::Confidential,
        &wire::SubmitPrivacyCaseRequest {
            privacy_case_ref: Some(wire::PrivacyCaseRef {
                privacy_case_id: case_id.as_str().to_owned(),
            }),
            expected_version,
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn verify_request(
    tenant: &str,
    case_id: &RecordId,
    expected_version: i64,
    submitted_party_id: &str,
    canonical_party_id: &str,
    generation: u64,
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
        MODULE_ID,
        VERIFY_PRIVACY_CASE_SUBJECT_CAPABILITY,
        VERIFY_PRIVACY_CASE_SUBJECT_REQUEST_SCHEMA,
        DataClass::Confidential,
        &wire::VerifyPrivacyCaseSubjectRequest {
            privacy_case_ref: Some(wire::PrivacyCaseRef {
                privacy_case_id: case_id.as_str().to_owned(),
            }),
            expected_version,
            submitted_party_ref: Some(customer_wire::PartyRef {
                party_id: submitted_party_id.to_owned(),
            }),
            canonical_party_ref: Some(customer_wire::PartyRef {
                party_id: canonical_party_id.to_owned(),
            }),
            identity_resolution_generation: generation,
            verification_method: wire::SubjectVerificationMethod::VerifiedDocument as i32,
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
    module_id: &str,
    capability_id: &str,
    request_schema: &str,
    data_class: DataClass,
    command: &M,
) -> CapabilityRequest {
    CapabilityRequest {
        context: ModuleExecutionContext {
            module_id: crm_module_sdk::ModuleId::try_new(module_id).unwrap(),
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
            module_id,
            request_schema,
            data_class,
            command,
        )
        .unwrap(),
        input_hash: [hash_byte; 32],
        approval: None,
    }
}

fn decode_verify(payload: &crm_module_sdk::TypedPayload) -> wire::PrivacyCase {
    wire::VerifyPrivacyCaseSubjectResponse::decode(payload.bytes.as_slice())
        .expect("decode exact VerifyPrivacyCaseSubjectResponse")
        .privacy_case
        .expect("subject verification response contains privacy case")
}

async fn assert_verified_record(
    store: &PostgresDataStore,
    request: &CapabilityRequest,
    case_id: &RecordId,
    canonical_party_id: &str,
    generation: u64,
) {
    let snapshot = store
        .get_record(
            &request.context,
            &privacy_case_ref_from_id(case_id).unwrap(),
        )
        .await
        .expect("read subject-verified privacy case")
        .expect("subject-verified privacy case exists");
    assert_eq!(snapshot.version, 3);
    let privacy_case =
        privacy_case_from_snapshot(&snapshot).expect("strictly rehydrate subject-verified case");
    assert_eq!(privacy_case.status(), PrivacyCaseStatus::SubjectVerified);
    assert_eq!(privacy_case.version(), 3);
    let binding = privacy_case.subject_binding().expect("subject binding persisted");
    assert_eq!(binding.submitted_party_id.as_str(), canonical_party_id);
    assert_eq!(binding.canonical_party_id.as_str(), canonical_party_id);
    assert_eq!(binding.identity_resolution_generation, generation);
}

async fn assert_failed_verify(
    executor: &Arc<dyn TransactionalCapabilityExecutor>,
    definition: &crm_capability_runtime::CapabilityDefinition,
    admin: &PgPool,
    request: CapabilityRequest,
    expected_code: &str,
    retryable: bool,
    expected_version: i64,
) {
    let tenant = request.context.execution.tenant_id.as_str().to_owned();
    let case_id = wire::VerifyPrivacyCaseSubjectRequest::decode(request.input.bytes.as_slice())
        .expect("decode failed verify fixture")
        .privacy_case_ref
        .expect("failed verify fixture contains case")
        .privacy_case_id;
    let idempotency_key = request.context.execution.idempotency_key.as_str().to_owned();
    let transaction_id = request
        .context
        .execution
        .business_transaction_id
        .as_str()
        .to_owned();
    let error = executor
        .execute(definition, request)
        .await
        .expect_err("subject verification scenario must fail closed");
    assert_eq!(error.code, expected_code);
    assert_eq!(error.retryable, retryable);
    for forbidden in [
        "crm.records",
        "crm.relationships",
        "identity_resolution_topology_generations",
        "SELECT",
        "sqlx",
        "password",
        "raw_secret",
    ] {
        assert!(!error.safe_message.contains(forbidden));
    }
    if tenant == TENANT_A {
        assert_record_version(admin, &tenant, &case_id, expected_version).await;
    }
    assert_no_verify_evidence(admin, &tenant, &idempotency_key, &transaction_id).await;
}

async fn assert_verify_evidence(
    admin: &PgPool,
    tenant: &str,
    case_id: &RecordId,
    idempotency_key: &str,
    transaction_id: &str,
) {
    assert_record_version(admin, tenant, case_id.as_str(), 3).await;
    for (sql, expected) in [
        (
            "SELECT count(*) FROM crm.outbox_events WHERE tenant_id = $1 AND business_transaction_id = $2 AND event_type = 'customer_privacy.case.subject_verified'",
            1,
        ),
        (
            "SELECT count(*) FROM crm.audit_records WHERE tenant_id = $1 AND business_transaction_id = $2 AND capability_id = 'customer_privacy.case.subject.verify'",
            1,
        ),
        (
            "SELECT count(*) FROM crm.business_transactions WHERE tenant_id = $1 AND business_transaction_id = $2",
            1,
        ),
    ] {
        assert_eq!(count_rows(admin, sql, tenant, transaction_id).await, expected);
    }
    let idempotency_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM crm.idempotency_records WHERE tenant_id = $1 AND idempotency_scope = $2 AND idempotency_key = $3 AND status = 'completed'",
    )
    .bind(tenant)
    .bind(VERIFY_SCOPE)
    .bind(idempotency_key)
    .fetch_one(admin)
    .await
    .expect("count completed subject-verification idempotency");
    assert_eq!(idempotency_count, 1);
    let marker = sqlx::query(
        "SELECT expected_outbox_events, expected_audit_records, expected_idempotency_records FROM crm.business_transactions WHERE tenant_id = $1 AND business_transaction_id = $2",
    )
    .bind(tenant)
    .bind(transaction_id)
    .fetch_one(admin)
    .await
    .expect("read subject-verification business transaction marker");
    assert_eq!(marker.get::<i32, _>("expected_outbox_events"), 1);
    assert_eq!(marker.get::<i32, _>("expected_audit_records"), 1);
    assert_eq!(marker.get::<i32, _>("expected_idempotency_records"), 1);
}

async fn assert_no_verify_evidence(
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
        assert_eq!(count_rows(admin, sql, tenant, transaction_id).await, 0);
    }
    let idempotency_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM crm.idempotency_records WHERE tenant_id = $1 AND idempotency_scope = $2 AND idempotency_key = $3",
    )
    .bind(tenant)
    .bind(VERIFY_SCOPE)
    .bind(idempotency_key)
    .fetch_one(admin)
    .await
    .expect("count rolled-back subject-verification idempotency");
    assert_eq!(idempotency_count, 0);
}

async fn assert_record_version(
    admin: &PgPool,
    tenant: &str,
    case_id: &str,
    expected_version: i64,
) {
    let actual: i64 = sqlx::query_scalar(
        "SELECT version FROM crm.records WHERE tenant_id = $1 AND record_type = $2 AND record_id = $3",
    )
    .bind(tenant)
    .bind(PRIVACY_CASE_RECORD_TYPE)
    .bind(case_id)
    .fetch_one(admin)
    .await
    .expect("read privacy-case version");
    assert_eq!(actual, expected_version);
}

async fn count_rows(
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
        .expect("count subject-verification evidence")
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
    .expect("read malformed subject fixture transaction context");
    let mut transaction = admin.begin().await.expect("start malformed update");
    bind_context(
        &mut transaction,
        tenant,
        "subject-malformed-fixture",
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
    .expect("corrupt subject fixture payload");
    transaction.commit().await.expect("commit malformed fixture");
}

async fn bind_context(
    transaction: &mut Transaction<'_, Postgres>,
    tenant: &str,
    request_id: &str,
    business_transaction_id: &str,
) {
    for (name, value) in [
        ("app.tenant_id", tenant),
        ("app.actor_id", "customer-privacy-subject-fixture"),
        ("app.request_id", request_id),
        (
            "app.capability_id",
            "customer_privacy.case.subject.verify.fixture",
        ),
        ("app.capability_version", "1.0.0"),
        ("app.business_transaction_id", business_transaction_id),
    ] {
        sqlx::query("SELECT set_config($1, $2, true)")
            .bind(name)
            .bind(value)
            .execute(&mut **transaction)
            .await
            .expect("bind governed subject fixture context");
    }
}
