use crm_capability_runtime::CapabilityRequest;
use crm_core_data::{CustomerSubjectOperationClass, TransactionalCustomerSubjectPolicyPort};
use crm_customer_privacy_production::{
    CUSTOMER_PRIVACY_MODULE_ID, PostgresCustomerPrivacySubjectPolicy, ProcessingRestriction,
    RestrictionScope, processing_restriction_persisted_payload,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, CorrelationId,
    DataClass, ExecutionContext, IdempotencyKey, ModuleExecutionContext, ModuleId, PayloadEncoding,
    RecordId, RequestId, RetentionPolicyId, SchemaId, SchemaVersion, TenantId, TraceId,
    TypedPayload,
};
use sqlx::{PgPool, Postgres, Transaction, postgres::PgPoolOptions};

const TENANT_A: &str = "tenant-a";
const TENANT_B: &str = "tenant-b";
const ACTOR_A: &str = "actor-a";
const DECISION_AT: i64 = 2_000_000_000;
const ACTIVE_SUBJECT: &str = "restriction-policy-active-party";
const LOCKED_SUBJECT: &str = "restriction-policy-locked-party";
const MALFORMED_SUBJECT: &str = "restriction-policy-malformed-party";
const FUTURE_SUBJECT: &str = "restriction-policy-future-party";
const ACTIVE_RESTRICTION: &str = "restriction-policy-active";
const MALFORMED_RESTRICTION: &str = "restriction-policy-malformed";
const FUTURE_RESTRICTION: &str = "restriction-policy-future";

struct RestrictionFixture<'a> {
    restriction_id: &'a str,
    subject: &'a str,
    scope: RestrictionScope,
    effective_from_unix_nanos: i64,
    corrupt_descriptor: bool,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn final_restriction_decision_is_live_tenant_bound_strict_and_lock_safe() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping final restriction policy proof because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let app = PgPoolOptions::new()
        .max_connections(6)
        .connect(&database_url)
        .await
        .expect("connect Customer Privacy app-role pool");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect Customer Privacy evidence pool");
    cleanup(&admin).await;

    insert_restriction(
        &admin,
        RestrictionFixture {
            restriction_id: ACTIVE_RESTRICTION,
            subject: ACTIVE_SUBJECT,
            scope: RestrictionScope::Processing,
            effective_from_unix_nanos: 1_000_000_000,
            corrupt_descriptor: false,
        },
    )
    .await;
    insert_restriction(
        &admin,
        RestrictionFixture {
            restriction_id: FUTURE_RESTRICTION,
            subject: FUTURE_SUBJECT,
            scope: RestrictionScope::ProcessingAndCommunication,
            effective_from_unix_nanos: 3_000_000_000,
            corrupt_descriptor: false,
        },
    )
    .await;
    insert_restriction(
        &admin,
        RestrictionFixture {
            restriction_id: MALFORMED_RESTRICTION,
            subject: MALFORMED_SUBJECT,
            scope: RestrictionScope::Processing,
            effective_from_unix_nanos: 1_000_000_000,
            corrupt_descriptor: true,
        },
    )
    .await;

    let policy = PostgresCustomerPrivacySubjectPolicy;

    let locked_request = request(TENANT_A, "locked", DECISION_AT);
    let mut locked = begin_bound(&app, &locked_request.context).await;
    policy
        .lock_and_enforce(
            &mut locked,
            &locked_request,
            &RecordId::try_new(LOCKED_SUBJECT).unwrap(),
            CustomerSubjectOperationClass::Processing,
        )
        .await
        .expect("subject without a directive is allowed");

    let mut competing = begin_bound(&app, &locked_request.context).await;
    let lock_error = sqlx::query("SELECT crm.lock_customer_subject($1, $2)")
        .bind(TENANT_A)
        .bind(LOCKED_SUBJECT)
        .execute(&mut *competing)
        .await
        .expect_err("the final decision must retain the shared lock until transaction end");
    assert!(
        lock_error
            .as_database_error()
            .and_then(|error| error.code())
            .is_some_and(|code| code.as_ref() == "55P03")
    );
    competing
        .rollback()
        .await
        .expect("roll back competing transaction");
    locked
        .commit()
        .await
        .expect("commit first subject transaction");

    let processing_error = decide(
        &app,
        &policy,
        TENANT_A,
        ACTIVE_SUBJECT,
        CustomerSubjectOperationClass::Processing,
    )
    .await
    .expect_err("active processing directive must deny processing");
    assert_eq!(processing_error.code, "CUSTOMER_PRIVACY_RESTRICTION_ACTIVE");
    assert!(!processing_error.retryable);

    decide(
        &app,
        &policy,
        TENANT_A,
        ACTIVE_SUBJECT,
        CustomerSubjectOperationClass::Communication,
    )
    .await
    .expect("processing-only directive must not invent a communication denial");

    decide(
        &app,
        &policy,
        TENANT_B,
        ACTIVE_SUBJECT,
        CustomerSubjectOperationClass::Processing,
    )
    .await
    .expect("tenant B cannot observe tenant A restriction state through FORCE RLS");

    decide(
        &app,
        &policy,
        TENANT_A,
        FUTURE_SUBJECT,
        CustomerSubjectOperationClass::Processing,
    )
    .await
    .expect("future directive is not active at the immutable request timestamp");

    let malformed_error = decide(
        &app,
        &policy,
        TENANT_A,
        MALFORMED_SUBJECT,
        CustomerSubjectOperationClass::Processing,
    )
    .await
    .expect_err("matching malformed state must fail closed");
    assert_eq!(
        malformed_error.code,
        "CUSTOMER_PRIVACY_RESTRICTION_STATE_INVALID"
    );
    assert!(!malformed_error.retryable);

    cleanup(&admin).await;
}

async fn decide(
    app: &PgPool,
    policy: &PostgresCustomerPrivacySubjectPolicy,
    tenant: &str,
    subject: &str,
    operation_class: CustomerSubjectOperationClass,
) -> Result<(), crm_module_sdk::SdkError> {
    let request = request(tenant, subject, DECISION_AT);
    let mut transaction = begin_bound(app, &request.context).await;
    let result = policy
        .lock_and_enforce(
            &mut transaction,
            &request,
            &RecordId::try_new(subject).unwrap(),
            operation_class,
        )
        .await;
    transaction
        .rollback()
        .await
        .expect("roll back read-only policy proof");
    result
}

async fn begin_bound<'a>(
    pool: &'a PgPool,
    context: &ModuleExecutionContext,
) -> Transaction<'a, Postgres> {
    let mut transaction = pool.begin().await.expect("begin bound transaction");
    bind_context(&mut transaction, context)
        .await
        .expect("bind transaction-local execution context");
    transaction
}

async fn bind_context(
    transaction: &mut Transaction<'_, Postgres>,
    context: &ModuleExecutionContext,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        SELECT
          set_config('app.tenant_id', $1, true),
          set_config('app.actor_id', $2, true),
          set_config('app.request_id', $3, true),
          set_config('app.capability_id', $4, true),
          set_config('app.capability_version', $5, true),
          set_config('app.business_transaction_id', $6, true)
        "#,
    )
    .bind(context.execution.tenant_id.as_str())
    .bind(context.execution.actor_id.as_str())
    .bind(context.execution.request_id.as_str())
    .bind(context.execution.capability_id.as_str())
    .bind(context.execution.capability_version.as_str())
    .bind(context.execution.business_transaction_id.as_str())
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn disable_fixture_triggers(transaction: &mut Transaction<'_, Postgres>) {
    sqlx::query("SET LOCAL session_replication_role = replica")
        .execute(&mut **transaction)
        .await
        .expect("disable trigger-backed evidence verification for admin fixture seeding");
}

async fn insert_restriction(admin: &PgPool, fixture: RestrictionFixture<'_>) {
    let restriction = ProcessingRestriction::place(
        RecordId::try_new(fixture.restriction_id).unwrap(),
        TenantId::try_new(TENANT_A).unwrap(),
        RecordId::try_new(fixture.subject).unwrap(),
        fixture.scope,
        SchemaVersion::try_new("privacy-policy/1").unwrap(),
        ActorId::try_new(ACTOR_A).unwrap(),
        1_000_000_000,
        fixture.effective_from_unix_nanos,
        None,
    )
    .expect("construct valid restriction fixture");
    let mut payload = processing_restriction_persisted_payload(&restriction)
        .expect("encode valid restriction fixture");
    if fixture.corrupt_descriptor {
        payload.descriptor_hash = [99; 32];
    }

    let fixture_request = fixture_request(fixture.restriction_id);
    let mut transaction = begin_bound(admin, &fixture_request.context).await;
    disable_fixture_triggers(&mut transaction).await;
    insert_business_transaction(&mut transaction, &fixture_request.context).await;
    insert_payload(
        &mut transaction,
        fixture.restriction_id,
        &fixture_request.context.execution.business_transaction_id,
        payload,
    )
    .await;
    transaction
        .commit()
        .await
        .expect("commit restriction fixture");
}

async fn insert_business_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    context: &ModuleExecutionContext,
) {
    sqlx::query(
        r#"
        INSERT INTO crm.business_transactions (
          tenant_id,
          business_transaction_id,
          actor_id,
          request_id,
          capability_id,
          capability_version,
          expected_outbox_events,
          expected_audit_records,
          expected_idempotency_records
        )
        VALUES ($1, $2, $3, $4, $5, $6, 1, 1, 1)
        "#,
    )
    .bind(context.execution.tenant_id.as_str())
    .bind(context.execution.business_transaction_id.as_str())
    .bind(context.execution.actor_id.as_str())
    .bind(context.execution.request_id.as_str())
    .bind(context.execution.capability_id.as_str())
    .bind(context.execution.capability_version.as_str())
    .execute(&mut **transaction)
    .await
    .expect("insert restriction fixture business transaction");
}

async fn insert_payload(
    transaction: &mut Transaction<'_, Postgres>,
    restriction_id: &str,
    business_transaction_id: &BusinessTransactionId,
    payload: TypedPayload,
) {
    let maximum_size = i64::try_from(payload.maximum_size_bytes).unwrap();
    sqlx::query(
        r#"
        INSERT INTO crm.records (
          tenant_id,
          record_type,
          record_id,
          version,
          owner_module_id,
          schema_id,
          schema_version,
          descriptor_hash,
          data_class,
          payload_encoding,
          maximum_payload_size,
          retention_policy_id,
          payload_bytes,
          last_business_transaction_id
        )
        VALUES ($1, 'customer-privacy.restriction', $2, 1, $3, $4, $5, $6,
                'personal', 'json', $7, $8, $9, $10)
        "#,
    )
    .bind(TENANT_A)
    .bind(restriction_id)
    .bind(payload.owner.as_str())
    .bind(payload.schema_id.as_str())
    .bind(payload.schema_version.as_str())
    .bind(payload.descriptor_hash.as_slice())
    .bind(maximum_size)
    .bind(payload.retention_policy_id.as_str())
    .bind(payload.bytes)
    .bind(business_transaction_id.as_str())
    .execute(&mut **transaction)
    .await
    .expect("insert restriction fixture");
}

async fn cleanup(admin: &PgPool) {
    let cleanup_request = request(TENANT_A, "cleanup", DECISION_AT);
    let mut transaction = begin_bound(admin, &cleanup_request.context).await;
    disable_fixture_triggers(&mut transaction).await;
    sqlx::query(
        r#"
        DELETE FROM crm.records
        WHERE tenant_id = $1
          AND owner_module_id = $2
          AND record_type = 'customer-privacy.restriction'
          AND record_id IN ($3, $4, $5)
        "#,
    )
    .bind(TENANT_A)
    .bind(CUSTOMER_PRIVACY_MODULE_ID)
    .bind(ACTIVE_RESTRICTION)
    .bind(MALFORMED_RESTRICTION)
    .bind(FUTURE_RESTRICTION)
    .execute(&mut *transaction)
    .await
    .expect("clean restriction policy fixtures");
    sqlx::query(
        r#"
        DELETE FROM crm.business_transactions
        WHERE tenant_id = $1
          AND business_transaction_id IN ($2, $3, $4)
        "#,
    )
    .bind(TENANT_A)
    .bind(format!("restriction-tx-{ACTIVE_RESTRICTION}"))
    .bind(format!("restriction-tx-{MALFORMED_RESTRICTION}"))
    .bind(format!("restriction-tx-{FUTURE_RESTRICTION}"))
    .execute(&mut *transaction)
    .await
    .expect("clean restriction policy fixture transactions");
    transaction
        .commit()
        .await
        .expect("commit restriction fixture cleanup");
}

fn fixture_request(identity: &str) -> CapabilityRequest {
    let mut fixture = request(TENANT_A, identity, DECISION_AT);
    fixture.context.module_id = ModuleId::try_new("crm.test").unwrap();
    fixture.context.execution.capability_id = CapabilityId::try_new("test.record.mutate").unwrap();
    fixture.input.owner = ModuleId::try_new("crm.test").unwrap();
    fixture
}

fn request(tenant: &str, identity: &str, started_at_unix_nanos: i64) -> CapabilityRequest {
    CapabilityRequest {
        context: ModuleExecutionContext {
            module_id: ModuleId::try_new("crm.parties").unwrap(),
            execution: ExecutionContext {
                tenant_id: TenantId::try_new(tenant).unwrap(),
                actor_id: ActorId::try_new(ACTOR_A).unwrap(),
                request_id: RequestId::try_new(format!("restriction-request-{identity}")).unwrap(),
                correlation_id: CorrelationId::try_new(format!(
                    "restriction-correlation-{identity}"
                ))
                .unwrap(),
                causation_id: CausationId::try_new(format!("restriction-causation-{identity}"))
                    .unwrap(),
                trace_id: TraceId::try_new(format!("restriction-trace-{identity}")).unwrap(),
                capability_id: CapabilityId::try_new("parties.party.update").unwrap(),
                capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                idempotency_key: IdempotencyKey::try_new(format!("restriction-key-{identity}"))
                    .unwrap(),
                business_transaction_id: BusinessTransactionId::try_new(format!(
                    "restriction-tx-{identity}"
                ))
                .unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                request_started_at_unix_nanos: started_at_unix_nanos,
            },
        },
        input: TypedPayload {
            owner: ModuleId::try_new("crm.parties").unwrap(),
            schema_id: SchemaId::try_new("crm.parties.v1.UpdatePartyRequest").unwrap(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
            descriptor_hash: [7; 32],
            data_class: DataClass::Personal,
            encoding: PayloadEncoding::Protobuf,
            maximum_size_bytes: 64,
            retention_policy_id: RetentionPolicyId::try_new("crm.parties.request").unwrap(),
            bytes: vec![1],
        },
        input_hash: [8; 32],
        approval: None,
    }
}
