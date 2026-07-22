use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest, TransactionalCapabilityExecutor};
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
    CREATE_CAPABILITY as CREATE_PARTY_CAPABILITY, CREATE_REQUEST_SCHEMA as CREATE_PARTY_SCHEMA,
    PartyCapabilityPlanner, capability_definition as party_definition,
};
use crm_proto_contracts::crm::{
    customer::v1 as customer_wire, customer_privacy::v1 as wire, parties::v1 as parties_wire,
};
use prost::Message;
use sqlx::{PgPool, Postgres, Transaction};
use std::sync::Arc;

const TENANT_A: &str = "tenant-a";
const TENANT_B: &str = "tenant-b";
const ACTOR: &str = "privacy-officer";
const VERIFY_SCOPE: &str = "capability:customer_privacy.case.subject.verify:1.0.0";

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn subject_verify_is_atomic_replay_safe_and_authoritative() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping subject verification PostgreSQL proof without DATABASE_URL");
        return;
    };
    let admin_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let store = PostgresDataStore::connect(&database_url, 8)
        .await
        .expect("connect subject verification runtime store");
    let admin = PgPool::connect(&admin_url)
        .await
        .expect("connect subject verification evidence reader");

    let party_executor: Arc<dyn TransactionalCapabilityExecutor> = Arc::new(
        PostgresTransactionalAggregateExecutor::new(
            store.clone(),
            Arc::new(PartyCapabilityPlanner),
        ),
    );
    let create_executor = postgres_case_create_executor(store.clone());
    let submit_executor = postgres_case_submit_executor(store.clone());
    let verify_executor = postgres_case_subject_verify_executor(store.clone());
    let party_definition = party_definition(CREATE_PARTY_CAPABILITY).unwrap();
    let create_definition = create_definition().unwrap();
    let submit_definition = submit_definition().unwrap();
    let verify_definition = verify_definition().unwrap();

    for (party, hash) in [
        ("party-submitted", 11),
        ("party-canonical", 12),
        ("party-other", 13),
    ] {
        create_party(&party_executor, &party_definition, party, hash).await;
    }

    let case_id = submitted_case(
        &create_executor,
        &create_definition,
        &submit_executor,
        &submit_definition,
        "success",
        20,
    )
    .await;
    let verify = verify_request(
        TENANT_A,
        &case_id,
        2,
        "party-submitted",
        "party-submitted",
        1,
        "success",
        21,
    );
    let first = verify_executor
        .execute(&verify_definition, verify.clone())
        .await
        .expect("verify subject at authoritative self-root");
    assert!(!first.replayed);
    let first_case = decode_verify(first.output.as_ref().unwrap());
    assert_eq!(
        first_case.status,
        wire::PrivacyCaseStatus::SubjectVerified as i32
    );
    assert_eq!(first_case.version, 3);
    assert_verified_record(&store, &verify, &case_id).await;
    assert_evidence(&admin, TENANT_A, "success", &case_id, 1).await;

    let replay = verify_executor
        .execute(&verify_definition, verify.clone())
        .await
        .expect("exact verify replay");
    assert!(replay.replayed);
    assert_eq!(decode_verify(replay.output.as_ref().unwrap()), first_case);
    assert_evidence(&admin, TENANT_A, "success", &case_id, 1).await;

    let mut conflicting = verify.clone();
    conflicting.input_hash = [22; 32];
    let conflict = verify_executor
        .execute(&verify_definition, conflicting)
        .await
        .expect_err("conflicting verify replay must fail");
    assert_eq!(conflict.code, "CAPABILITY_IDEMPOTENCY_KEY_REUSED");
    assert!(!conflict.retryable);
    assert_evidence(&admin, TENANT_A, "success", &case_id, 1).await;

    for scenario in [
        FailureScenario {
            name: "missing-submitted",
            tenant: TENANT_A,
            expected_version: 2,
            submitted: "party-missing",
            canonical: "party-submitted",
            generation: 1,
            expected_code: "CUSTOMER_PRIVACY_SUBJECT_REFERENCE_UNAVAILABLE",
            retryable: false,
        },
        FailureScenario {
            name: "missing-canonical",
            tenant: TENANT_A,
            expected_version: 2,
            submitted: "party-submitted",
            canonical: "party-missing",
            generation: 1,
            expected_code: "CUSTOMER_PRIVACY_SUBJECT_REFERENCE_UNAVAILABLE",
            retryable: false,
        },
        FailureScenario {
            name: "false-canonical",
            tenant: TENANT_A,
            expected_version: 2,
            submitted: "party-submitted",
            canonical: "party-canonical",
            generation: 1,
            expected_code: "CUSTOMER_PRIVACY_SUBJECT_CANONICAL_REFERENCE_INVALID",
            retryable: false,
        },
        FailureScenario {
            name: "stale-generation",
            tenant: TENANT_A,
            expected_version: 2,
            submitted: "party-submitted",
            canonical: "party-submitted",
            generation: 2,
            expected_code: "CUSTOMER_PRIVACY_SUBJECT_GENERATION_STALE",
            retryable: true,
        },
        FailureScenario {
            name: "cross-tenant",
            tenant: TENANT_B,
            expected_version: 2,
            submitted: "party-submitted",
            canonical: "party-submitted",
            generation: 1,
            expected_code: "CUSTOMER_PRIVACY_SUBJECT_REFERENCE_UNAVAILABLE",
            retryable: false,
        },
    ] {
        let case_id = submitted_case(
            &create_executor,
            &create_definition,
            &submit_executor,
            &submit_definition,
            scenario.name,
            30 + scenario.name.len() as u8,
        )
        .await;
        let request = verify_request(
            scenario.tenant,
            &case_id,
            scenario.expected_version,
            scenario.submitted,
            scenario.canonical,
            scenario.generation,
            scenario.name,
            80 + scenario.name.len() as u8,
        );
        assert_failure(
            &verify_executor,
            &verify_definition,
            &admin,
            request,
            scenario.expected_code,
            scenario.retryable,
            if scenario.tenant == TENANT_A {
                Some((&case_id, 2))
            } else {
                None
            },
        )
        .await;
    }

    let stale_case = submitted_case(
        &create_executor,
        &create_definition,
        &submit_executor,
        &submit_definition,
        "stale-version",
        50,
    )
    .await;
    assert_failure(
        &verify_executor,
        &verify_definition,
        &admin,
        verify_request(
            TENANT_A,
            &stale_case,
            3,
            "party-submitted",
            "party-submitted",
            1,
            "stale-version",
            51,
        ),
        "CUSTOMER_PRIVACY_VERSION_CONFLICT",
        true,
        Some((&stale_case, 2)),
    )
    .await;

    let draft_case = create_case(&create_executor, &create_definition, "wrong-state", 60).await;
    assert_failure(
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
            "wrong-state",
            61,
        ),
        "CUSTOMER_PRIVACY_INVALID_TRANSITION",
        false,
        Some((&draft_case, 1)),
    )
    .await;

    let malformed_case = submitted_case(
        &create_executor,
        &create_definition,
        &submit_executor,
        &submit_definition,
        "malformed",
        70,
    )
    .await;
    corrupt_payload(&admin, &malformed_case).await;
    assert_failure(
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
            "malformed",
            71,
        ),
        "CUSTOMER_PRIVACY_CASE_INVALID",
        false,
        Some((&malformed_case, 2)),
    )
    .await;

    let contended_case = submitted_case(
        &create_executor,
        &create_definition,
        &submit_executor,
        &submit_definition,
        "lock-contention",
        90,
    )
    .await;
    let mut lock_holder = admin.begin().await.unwrap();
    bind_context(&mut lock_holder, "subject-lock-holder", "subject-lock-holder-tx").await;
    sqlx::query("SELECT crm.lock_customer_subject($1, $2)")
        .bind(TENANT_A)
        .bind("party-submitted")
        .execute(&mut *lock_holder)
        .await
        .unwrap();
    assert_failure(
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
            "lock-contention",
            91,
        ),
        "CUSTOMER_PRIVACY_SUBJECT_LOCK_UNAVAILABLE",
        true,
        Some((&contended_case, 2)),
    )
    .await;
    lock_holder.rollback().await.unwrap();
}

struct FailureScenario {
    name: &'static str,
    tenant: &'static str,
    expected_version: i64,
    submitted: &'static str,
    canonical: &'static str,
    generation: u64,
    expected_code: &'static str,
    retryable: bool,
}

async fn create_party(
    executor: &Arc<dyn TransactionalCapabilityExecutor>,
    definition: &CapabilityDefinition,
    party_id: &str,
    hash: u8,
) {
    executor
        .execute(
            definition,
            request(
                TENANT_A,
                "crm.parties",
                CREATE_PARTY_CAPABILITY,
                CREATE_PARTY_SCHEMA,
                DataClass::Personal,
                &format!("party-{party_id}"),
                hash,
                &parties_wire::CreatePartyRequest {
                    party_ref: Some(customer_wire::PartyRef {
                        party_id: party_id.to_owned(),
                    }),
                    kind: parties_wire::PartyKind::Person as i32,
                    display_name: format!("Subject fixture {party_id}"),
                },
            ),
        )
        .await
        .unwrap();
}

async fn create_case(
    executor: &Arc<dyn TransactionalCapabilityExecutor>,
    definition: &CapabilityDefinition,
    name: &str,
    hash: u8,
) -> RecordId {
    let request = request(
        TENANT_A,
        MODULE_ID,
        CREATE_PRIVACY_CASE_CAPABILITY,
        CREATE_PRIVACY_CASE_REQUEST_SCHEMA,
        DataClass::Confidential,
        &format!("{name}-create"),
        hash,
        &wire::CreatePrivacyCaseRequest {
            kind: wire::PrivacyCaseKind::Erasure as i32,
            policy_version: "privacy-policy/1".to_owned(),
            previous_privacy_case_ref: None,
        },
    );
    let case_id =
        deterministic_privacy_case_id(TENANT_A, request.context.execution.idempotency_key.as_str())
            .unwrap();
    executor.execute(definition, request).await.unwrap();
    case_id
}

async fn submitted_case(
    create_executor: &Arc<dyn TransactionalCapabilityExecutor>,
    create_definition: &CapabilityDefinition,
    submit_executor: &Arc<dyn TransactionalCapabilityExecutor>,
    submit_definition: &CapabilityDefinition,
    name: &str,
    hash: u8,
) -> RecordId {
    let case_id = create_case(create_executor, create_definition, name, hash).await;
    submit_executor
        .execute(
            submit_definition,
            request(
                TENANT_A,
                MODULE_ID,
                SUBMIT_PRIVACY_CASE_CAPABILITY,
                SUBMIT_PRIVACY_CASE_REQUEST_SCHEMA,
                DataClass::Confidential,
                &format!("{name}-submit"),
                hash + 1,
                &wire::SubmitPrivacyCaseRequest {
                    privacy_case_ref: Some(wire::PrivacyCaseRef {
                        privacy_case_id: case_id.as_str().to_owned(),
                    }),
                    expected_version: 1,
                },
            ),
        )
        .await
        .unwrap();
    case_id
}

#[allow(clippy::too_many_arguments)]
fn verify_request(
    tenant: &str,
    case_id: &RecordId,
    expected_version: i64,
    submitted: &str,
    canonical: &str,
    generation: u64,
    name: &str,
    hash: u8,
) -> CapabilityRequest {
    request(
        tenant,
        MODULE_ID,
        VERIFY_PRIVACY_CASE_SUBJECT_CAPABILITY,
        VERIFY_PRIVACY_CASE_SUBJECT_REQUEST_SCHEMA,
        DataClass::Confidential,
        &format!("{name}-verify"),
        hash,
        &wire::VerifyPrivacyCaseSubjectRequest {
            privacy_case_ref: Some(wire::PrivacyCaseRef {
                privacy_case_id: case_id.as_str().to_owned(),
            }),
            expected_version,
            submitted_party_ref: Some(customer_wire::PartyRef {
                party_id: submitted.to_owned(),
            }),
            canonical_party_ref: Some(customer_wire::PartyRef {
                party_id: canonical.to_owned(),
            }),
            identity_resolution_generation: generation,
            verification_method: wire::SubjectVerificationMethod::VerifiedDocument as i32,
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn request<M: Message>(
    tenant: &str,
    module: &str,
    capability: &str,
    schema: &str,
    data_class: DataClass,
    identity: &str,
    hash: u8,
    command: &M,
) -> CapabilityRequest {
    CapabilityRequest {
        context: ModuleExecutionContext {
            module_id: crm_module_sdk::ModuleId::try_new(module).unwrap(),
            execution: ExecutionContext {
                tenant_id: TenantId::try_new(tenant).unwrap(),
                actor_id: ActorId::try_new(ACTOR).unwrap(),
                request_id: RequestId::try_new(format!("request-{identity}")).unwrap(),
                correlation_id: CorrelationId::try_new(format!("correlation-{identity}")).unwrap(),
                causation_id: CausationId::try_new(format!("causation-{identity}")).unwrap(),
                trace_id: TraceId::try_new(format!("trace-{identity}")).unwrap(),
                capability_id: CapabilityId::try_new(capability).unwrap(),
                capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                idempotency_key: IdempotencyKey::try_new(format!("{identity}-key")).unwrap(),
                business_transaction_id: BusinessTransactionId::try_new(format!(
                    "{identity}-tx"
                ))
                .unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                request_started_at_unix_nanos: 1_000_000_000 + i64::from(hash),
            },
        },
        input: support::protobuf_payload(module, schema, data_class, command).unwrap(),
        input_hash: [hash; 32],
        approval: None,
    }
}

fn decode_verify(payload: &crm_module_sdk::TypedPayload) -> wire::PrivacyCase {
    wire::VerifyPrivacyCaseSubjectResponse::decode(payload.bytes.as_slice())
        .unwrap()
        .privacy_case
        .unwrap()
}

async fn assert_verified_record(
    store: &PostgresDataStore,
    verify: &CapabilityRequest,
    case_id: &RecordId,
) {
    let snapshot = store
        .get_record(
            &verify.context,
            &privacy_case_ref_from_id(case_id).unwrap(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(snapshot.version, 3);
    let privacy_case = privacy_case_from_snapshot(&snapshot).unwrap();
    assert_eq!(privacy_case.status(), PrivacyCaseStatus::SubjectVerified);
    let binding = privacy_case.subject_binding().unwrap();
    assert_eq!(binding.submitted_party_id.as_str(), "party-submitted");
    assert_eq!(binding.canonical_party_id.as_str(), "party-submitted");
    assert_eq!(binding.identity_resolution_generation, 1);
}

async fn assert_failure(
    executor: &Arc<dyn TransactionalCapabilityExecutor>,
    definition: &CapabilityDefinition,
    admin: &PgPool,
    request: CapabilityRequest,
    code: &str,
    retryable: bool,
    version: Option<(&RecordId, i64)>,
) {
    let tenant = request.context.execution.tenant_id.as_str().to_owned();
    let idempotency_key = request.context.execution.idempotency_key.as_str().to_owned();
    let transaction_id = request
        .context
        .execution
        .business_transaction_id
        .as_str()
        .to_owned();
    let error = executor.execute(definition, request).await.unwrap_err();
    assert_eq!(error.code, code);
    assert_eq!(error.retryable, retryable);
    for forbidden in ["crm.records", "SELECT", "sqlx", "password", "raw_secret"] {
        assert!(!error.safe_message.contains(forbidden));
    }
    if let Some((case_id, expected)) = version {
        assert_eq!(record_version(admin, TENANT_A, case_id).await, expected);
    }
    assert_eq!(
        count_for_transaction(
            admin,
            "SELECT count(*) FROM crm.business_transactions WHERE tenant_id = $1 AND business_transaction_id = $2",
            &tenant,
            &transaction_id,
        )
        .await,
        0
    );
    let idempotency_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM crm.idempotency_records WHERE tenant_id = $1 AND idempotency_scope = $2 AND idempotency_key = $3",
    )
    .bind(&tenant)
    .bind(VERIFY_SCOPE)
    .bind(&idempotency_key)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(idempotency_count, 0);
}

async fn assert_evidence(
    admin: &PgPool,
    tenant: &str,
    name: &str,
    case_id: &RecordId,
    expected: i64,
) {
    assert_eq!(record_version(admin, tenant, case_id).await, 3);
    let transaction = format!("{name}-verify-tx");
    for sql in [
        "SELECT count(*) FROM crm.outbox_events WHERE tenant_id = $1 AND business_transaction_id = $2 AND event_type = 'customer_privacy.case.subject_verified'",
        "SELECT count(*) FROM crm.audit_records WHERE tenant_id = $1 AND business_transaction_id = $2 AND capability_id = 'customer_privacy.case.subject.verify'",
        "SELECT count(*) FROM crm.business_transactions WHERE tenant_id = $1 AND business_transaction_id = $2",
    ] {
        assert_eq!(count_for_transaction(admin, sql, tenant, &transaction).await, expected);
    }
    let idempotency: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM crm.idempotency_records WHERE tenant_id = $1 AND idempotency_scope = $2 AND idempotency_key = $3 AND status = 'completed'",
    )
    .bind(tenant)
    .bind(VERIFY_SCOPE)
    .bind(format!("{name}-verify-key"))
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(idempotency, expected);
}

async fn record_version(admin: &PgPool, tenant: &str, case_id: &RecordId) -> i64 {
    sqlx::query_scalar(
        "SELECT version FROM crm.records WHERE tenant_id = $1 AND record_type = $2 AND record_id = $3",
    )
    .bind(tenant)
    .bind(PRIVACY_CASE_RECORD_TYPE)
    .bind(case_id.as_str())
    .fetch_one(admin)
    .await
    .unwrap()
}

async fn count_for_transaction(
    admin: &PgPool,
    sql: &'static str,
    tenant: &str,
    transaction: &str,
) -> i64 {
    sqlx::query_scalar(sql)
        .bind(tenant)
        .bind(transaction)
        .fetch_one(admin)
        .await
        .unwrap()
}

async fn corrupt_payload(admin: &PgPool, case_id: &RecordId) {
    let transaction_id: String = sqlx::query_scalar(
        "SELECT last_business_transaction_id FROM crm.records WHERE tenant_id = $1 AND record_type = $2 AND record_id = $3",
    )
    .bind(TENANT_A)
    .bind(PRIVACY_CASE_RECORD_TYPE)
    .bind(case_id.as_str())
    .fetch_one(admin)
    .await
    .unwrap();
    let mut transaction = admin.begin().await.unwrap();
    bind_context(&mut transaction, "malformed-fixture", &transaction_id).await;
    sqlx::query(
        "UPDATE crm.records SET payload_bytes = $4 WHERE tenant_id = $1 AND record_type = $2 AND record_id = $3",
    )
    .bind(TENANT_A)
    .bind(PRIVACY_CASE_RECORD_TYPE)
    .bind(case_id.as_str())
    .bind(b"{\"raw_secret\":\"must-not-leak\"}".as_slice())
    .execute(&mut *transaction)
    .await
    .unwrap();
    transaction.commit().await.unwrap();
}

async fn bind_context(
    transaction: &mut Transaction<'_, Postgres>,
    request_id: &str,
    business_transaction_id: &str,
) {
    for (name, value) in [
        ("app.tenant_id", TENANT_A),
        ("app.actor_id", "customer-privacy-subject-fixture"),
        ("app.request_id", request_id),
        ("app.capability_id", "customer_privacy.case.subject.verify.fixture"),
        ("app.capability_version", "1.0.0"),
        ("app.business_transaction_id", business_transaction_id),
    ] {
        sqlx::query("SELECT set_config($1, $2, true)")
            .bind(name)
            .bind(value)
            .execute(&mut **transaction)
            .await
            .unwrap();
    }
}
