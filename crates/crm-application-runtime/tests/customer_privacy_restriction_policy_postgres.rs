use crm_capability_runtime::CapabilityRequest;
use crm_core_data::{CustomerSubjectOperationClass, TransactionalCustomerSubjectPolicyPort};
use crm_customer_privacy_production::{
    CUSTOMER_PRIVACY_MODULE_ID, PostgresCustomerPrivacySubjectPolicy, ProcessingRestriction,
    RestrictionScope, processing_restriction_persisted_payload,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, CorrelationId,
    DataClass, ExecutionContext, IdempotencyKey, ModuleExecutionContext, ModuleId, PayloadEncoding,
    RecordId, RequestId, RetentionPolicyId, SchemaId, SchemaVersion, TenantId, TraceId, TypedPayload,
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
        ACTIVE_RESTRICTION,
        TENANT_A,
        ACTIVE_SUBJECT,
        RestrictionScope::Processing,
        1_000_000_000,
        false,
    )
    .await;
    insert_restriction(
        &admin,
        FUTURE_RESTRICTION,
        TENANT_A,
        FUTURE_SUBJECT,
        RestrictionScope::ProcessingAndCommunication,
        3_000_000_000,
        false,
    )
    .await;
    insert_restriction(
        &admin,
        MALFORMED_RESTRICTION,
        TENANT_A,
        MALFORMED_SUBJECT,
        RestrictionScope::Processing,
        1_000_000_000,
        true,
    )
    .await;

    let policy = PostgresCustomerPrivacySubjectPolicy;

    let mut locked = app.begin().await.expect("begin first subject transaction");
    let locked_request = request(TENANT_A, "locked", DECISION_AT);
    bind_context(&mut locked, &locked_request.context)
        .await
        .expect("bind first subject transaction");
    policy
        .lock_and_enforce(
            &mut locked,
            &locked_request,
            &RecordId::try_new(LOCKED_SUBJECT).unwrap(),
            CustomerSubjectOperationClass::Processing,
        )
        .await
        .expect("subject without a directive is allowed");

    let mut competing = app.begin().await.expect("begin competing subject transaction");
    bind_context(&mut competing, &locked_request.context)
        .await
        .expect("bind competing subject transaction");
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
    locked.commit().await.expect("commit first subject transaction");

    let processing_error = decide(
        &app,
        &policy,
        TENANT_A,
        ACTIVE_SUBJECT,
        CustomerSubjectOperationClass::Processing,
    )
    .await
    .expect_err("active processing directive must deny processing");
    assert_eq!(
        processing_error.code,
        "CUSTOMER_PRIVACY_RESTRICTION_ACTIVE"
    );
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
    let mut transaction = app.begin().await.expect("begin policy transaction");
    bind_context(&mut transaction, &request.context)
        .await
        .expect("bind policy transaction");
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

#[allow(clippy::too_many_arguments)]
async fn insert_restriction(
    admin: &PgPool,
    restriction_id: &str,
    tenant: &str,
    subject: &str,
    scope: RestrictionScope,
    effective_from_unix_nanos: i64,
    corrupt_descriptor: bool,
) {
    let restriction = ProcessingRestriction::place(
        RecordId::try_new(restriction_id).unwrap(),
        TenantId::try_new(tenant).unwrap(),
        RecordId::try_new(subject).unwrap(),
        scope,
        SchemaVersion::try_new("privacy-policy/1").unwrap(),
        ActorId::try_new(ACTOR_A).unwrap(),
        1_000_000_000,
        effective_from_unix_nanos,
        None,
    )
    .expect("construct valid restriction fixture");
    let mut payload = processing_restriction_persisted_payload(&restriction)
        .expect("encode valid restriction fixture");
    if corrupt_descriptor {
        payload.descriptor_hash = [99; 32];
    }
    insert_payload(admin, restriction_id, tenant, payload).await;
}

async fn insert_payload(admin: &PgPool, restriction_id: &str, tenant: &str, payload: TypedPayload) {
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
    .bind(tenant)
    .bind(restriction_id)
    .bind(payload.owner.as_str())
    .bind(payload.schema_id.as_str())
    .bind(payload.schema_version.as_str())
    .bind(payload.descriptor_hash.as_slice())
    .bind(maximum_size)
    .bind(payload.retention_policy_id.as_str())
    .bind(payload.bytes)
    .bind(format!("tx-{restriction_id}"))
    .execute(admin)
    .await
    .expect("insert restriction fixture");
}

async fn cleanup(admin: &PgPool) {
    sqlx::query(
        r#"
        DELETE FROM crm.records
        WHERE owner_module_id = $1
          AND record_type = 'customer-privacy.restriction'
          AND record_id IN ($2, $3, $4)
        "#,
    )
    .bind(CUSTOMER_PRIVACY_MODULE_ID)
    .bind(ACTIVE_RESTRICTION)
    .bind(MALFORMED_RESTRICTION)
    .bind(FUTURE_RESTRICTION)
    .execute(admin)
    .await
    .expect("clean restriction policy fixtures");
}

fn request(tenant: &str, identity: &str, started_at_unix_nanos: i64) -> CapabilityRequest {
    CapabilityRequest {
        context: ModuleExecutionContext {
            module_id: ModuleId::try_new("crm.parties").unwrap(),
            execution: ExecutionContext {
                tenant_id: TenantId::try_new(tenant).unwrap(),
                actor_id: ActorId::try_new(ACTOR_A).unwrap(),
                request_id: RequestId::try_new(format!("restriction-request-{identity}")).unwrap(),
                correlation_id: CorrelationId::try_new(format!("restriction-correlation-{identity}"))
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
