use ::http::{HeaderMap, HeaderValue, StatusCode};
use crm_capability_adapters::{
    ApprovalStore, AuthorizationGrant, CapabilityCatalog, FixedWindowPolicy,
    FixedWindowRateLimiter, LiveAuthorizationStore, LiveCapabilityAuthorizer, RateLimitPolicyStore,
    StoredApprovalVerifier,
};
use crm_capability_ingress::{
    AccessTokenGrant, AccessTokenStore, BUSINESS_TRANSACTION_HEADER, BearerTokenAuthenticator,
    CAUSATION_ID_HEADER, CORRELATION_ID_HEADER, CapabilityIngress, CapabilityRoute,
    ExecutionContextResolver, HttpCapabilityBody, HttpCapabilityMiddleware, HttpCapabilityRequest,
    IDEMPOTENCY_KEY_HEADER, REQUEST_ID_HEADER, TENANT_HEADER, TIMEOUT_HEADER, TRACE_ID_HEADER,
    TimeoutPolicy, semantic_input_hash,
};
use crm_capability_runtime::{
    AuthorizationDecision, CapabilityAuthorizer, CapabilityDefinition, CapabilityGateway,
    CapabilityRequest, CapabilityRisk, CapabilitySemanticValidator, PayloadContract,
};
use crm_core_data::{
    AuditEvidence, BatchError, BatchMutationPlan, BatchMutationResult, BatchMutationRuntime,
    CapabilityBatchExecutionPlan, CapabilityBatchPlanner, EventEvidence, FaultInjection,
    IdempotencyEvidence, PostgresDataStore, PostgresTransactionalCapabilityExecutor,
    RecordMutation, capability_idempotency_scope,
};
use crm_module_sdk::testing::{DeterministicRandom, FixedClock};
use crm_module_sdk::{
    ActorId, CapabilityId, CapabilityVersion, Clock, DataClass, DomainEvent, ErrorCategory,
    EventType, ModuleId, PayloadEncoding, PortFuture, RecordId, RecordRef, RecordType,
    RetentionPolicyId, SchemaId, SchemaVersion, SdkError, TenantId, TypedPayload,
};
use sqlx::{Postgres, Row, Transaction};
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

const TENANT: &str = "tenant-a";
const OTHER_TENANT: &str = "tenant-b";
const ACTOR: &str = "actor-a";
const MODULE: &str = "crm.test";
const CAPABILITY: &str = "test.record.mutate";
const CAPABILITY_VERSION: &str = "1.0.0";
const AUTHORIZATION_POLICY: &str = "test.gateway.write";
const RATE_LIMIT_POLICY: &str = "test.gateway.fixed-window";
const TOKEN: &str = "0123456789abcdef0123456789abcdef";
const NOW: i64 = 1_700_000_100_000_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EvidenceCounts {
    records: i64,
    outbox: i64,
    audits: i64,
    idempotency: i64,
    transactions: i64,
}

impl EvidenceCounts {
    fn assert_incremented_once(self, previous: Self) {
        assert_eq!(self.records, previous.records + 1);
        assert_eq!(self.outbox, previous.outbox + 1);
        assert_eq!(self.audits, previous.audits + 1);
        assert_eq!(self.idempotency, previous.idempotency + 1);
        assert_eq!(self.transactions, previous.transactions + 1);
    }
}

#[derive(Debug)]
struct SemanticHashValidator;

impl CapabilitySemanticValidator for SemanticHashValidator {
    fn validate<'a>(
        &'a self,
        _definition: &'a CapabilityDefinition,
        request: &'a CapabilityRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            if request.input_hash != semantic_input_hash(&request.input) {
                return Err(SdkError::new(
                    "CAPABILITY_INPUT_HASH_INVALID",
                    ErrorCategory::InvalidArgument,
                    false,
                    "The capability input hash is invalid.",
                ));
            }
            Ok(())
        })
    }
}

#[derive(Clone)]
struct RecordingAuthorizer {
    inner: LiveCapabilityAuthorizer,
    calls: Arc<Mutex<Vec<&'static str>>>,
}

impl CapabilityAuthorizer for RecordingAuthorizer {
    fn authorize<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a CapabilityRequest,
    ) -> PortFuture<'a, Result<AuthorizationDecision, SdkError>> {
        Box::pin(async move {
            let decision = self.inner.authorize(definition, request).await;
            self.calls
                .lock()
                .expect("call-order mutex poisoned")
                .push("authorize");
            decision
        })
    }
}

#[derive(Clone)]
struct RecordingBatchRuntime {
    store: PostgresDataStore,
    fault: FaultInjection,
    calls: Arc<Mutex<Vec<&'static str>>>,
}

impl BatchMutationRuntime for RecordingBatchRuntime {
    fn execute_batch<'a>(
        &'a self,
        plan: &'a BatchMutationPlan,
    ) -> PortFuture<'a, Result<BatchMutationResult, BatchError>> {
        Box::pin(async move {
            self.calls
                .lock()
                .expect("call-order mutex poisoned")
                .push("batch");
            self.store.execute_batch_with_fault(plan, self.fault).await
        })
    }
}

#[derive(Debug, Clone)]
struct CreateRecordPlanner {
    record_id: String,
    audit_sequence: i64,
    previous_hash: [u8; 32],
    record_hash: [u8; 32],
}

impl CapabilityBatchPlanner for CreateRecordPlanner {
    fn plan(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<CapabilityBatchExecutionPlan, SdkError> {
        let record = RecordRef {
            record_type: RecordType::try_new("test.gateway_record")
                .expect("static record type must be valid"),
            record_id: RecordId::try_new(self.record_id.clone())
                .expect("test record ID must be valid"),
        };
        let occurred_at = request.context.execution.request_started_at_unix_nanos;
        Ok(CapabilityBatchExecutionPlan {
            batch: BatchMutationPlan {
                context: request.context.clone(),
                records: vec![RecordMutation::Create {
                    reference: record.clone(),
                    payload: request.input.clone(),
                }],
                relationships: Vec::new(),
                events: vec![EventEvidence {
                    event_id: format!("event-{}", self.record_id),
                    event: DomainEvent {
                        event_type: EventType::try_new("test.gateway_record.created")
                            .expect("static event type must be valid"),
                        aggregate: record,
                        expected_aggregate_version: None,
                        deduplication_key: format!("dedupe-{}", self.record_id),
                        payload: request.input.clone(),
                    },
                    aggregate_version: 1,
                    event_sequence: 1,
                    occurred_at_unix_nanos: occurred_at + 1,
                }],
                idempotency: IdempotencyEvidence {
                    scope: capability_idempotency_scope(definition),
                    key: request.context.execution.idempotency_key.to_string(),
                    request_hash: request.input_hash,
                    expires_at_unix_nanos: occurred_at + 1_000_000_000_000,
                },
                audits: vec![AuditEvidence {
                    audit_sequence: self.audit_sequence,
                    audit_record_id: format!("audit-{}", self.record_id),
                    canonicalization_profile: "crm.cjson/v1".to_owned(),
                    previous_hash: self.previous_hash,
                    record_hash: self.record_hash,
                    canonical_envelope: format!(r#"{{"record_id":"{}"}}"#, self.record_id)
                        .into_bytes(),
                    occurred_at_unix_nanos: occurred_at + 2,
                }],
            },
            output: None,
        })
    }
}

struct Composition {
    middleware: HttpCapabilityMiddleware,
    authorization_store: LiveAuthorizationStore,
    calls: Arc<Mutex<Vec<&'static str>>>,
}

#[tokio::test(flavor = "current_thread")]
async fn public_gateway_proves_no_bypass_replay_live_authorization_and_atomic_rollback() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping gateway PostgreSQL scenario because DATABASE_URL is not configured");
        return;
    };
    let store = PostgresDataStore::connect(&database_url, 4)
        .await
        .expect("connect to PostgreSQL");
    let initial_counts = evidence_counts(&store).await;
    let initial_head = audit_head(&store).await;
    let suffix = initial_head.0.to_string();
    let success_record_id = format!("e2e-gateway-success-{suffix}");
    let success_planner = CreateRecordPlanner {
        record_id: success_record_id,
        audit_sequence: initial_head.0,
        previous_hash: initial_head.1,
        record_hash: [0xa1; 32],
    };
    let clock = Arc::new(FixedClock::new(NOW));
    let token_store = token_store();
    let composition = compose(
        store.clone(),
        success_planner,
        FaultInjection::None,
        Arc::clone(&clock),
        token_store.clone(),
    );

    let invalid_credentials = composition
        .middleware
        .handle(http_request(
            OTHER_TENANT,
            &format!("e2e-gateway-invalid-auth-{suffix}"),
            &format!("e2e-gateway-invalid-auth-{suffix}"),
            input_payload(0x41),
            "Bearer invalid-invalid-invalid-invalid",
        ))
        .await;
    assert_eq!(invalid_credentials.status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        error_code(&invalid_credentials.body),
        "AUTHENTICATION_INVALID"
    );
    assert_call_order(&composition.calls, &[]);
    assert_eq!(evidence_counts(&store).await, initial_counts);

    let cross_tenant = composition
        .middleware
        .handle(http_request(
            OTHER_TENANT,
            &format!("e2e-gateway-cross-tenant-{suffix}"),
            &format!("e2e-gateway-cross-tenant-{suffix}"),
            input_payload(0x41),
            &format!("Bearer {TOKEN}"),
        ))
        .await;
    assert_eq!(cross_tenant.status, StatusCode::FORBIDDEN);
    assert_eq!(error_code(&cross_tenant.body), "TENANT_FORBIDDEN");
    assert_call_order(&composition.calls, &[]);
    assert_eq!(evidence_counts(&store).await, initial_counts);

    let success_idempotency = format!("e2e-gateway-success-idem-{suffix}");
    let first = composition
        .middleware
        .handle(http_request(
            TENANT,
            &success_idempotency,
            &format!("e2e-gateway-success-first-{suffix}"),
            input_payload(0x41),
            &format!("Bearer {TOKEN}"),
        ))
        .await;
    assert_eq!(first.status, StatusCode::OK);
    assert!(!success_result(&first.body).replayed);
    assert_call_order(&composition.calls, &["authorize", "batch"]);
    let after_first = evidence_counts(&store).await;
    after_first.assert_incremented_once(initial_counts);

    clear_calls(&composition.calls);
    let replay = composition
        .middleware
        .handle(http_request(
            TENANT,
            &success_idempotency,
            &format!("e2e-gateway-success-replay-{suffix}"),
            input_payload(0x41),
            &format!("Bearer {TOKEN}"),
        ))
        .await;
    assert_eq!(replay.status, StatusCode::OK);
    assert!(success_result(&replay.body).replayed);
    assert_eq!(success_result(&replay.body), success_result(&first.body));
    assert_call_order(&composition.calls, &["authorize", "batch"]);
    assert_eq!(evidence_counts(&store).await, after_first);

    clear_calls(&composition.calls);
    let reused_key = composition
        .middleware
        .handle(http_request(
            TENANT,
            &success_idempotency,
            &format!("e2e-gateway-success-reused-{suffix}"),
            input_payload(0x42),
            &format!("Bearer {TOKEN}"),
        ))
        .await;
    assert_eq!(reused_key.status, StatusCode::CONFLICT);
    assert_eq!(
        error_code(&reused_key.body),
        "CAPABILITY_IDEMPOTENCY_KEY_REUSED"
    );
    assert_call_order(&composition.calls, &["authorize", "batch"]);
    assert_eq!(evidence_counts(&store).await, after_first);

    composition
        .authorization_store
        .revoke(
            &TenantId::try_new(TENANT).unwrap(),
            &ActorId::try_new(ACTOR).unwrap(),
            AUTHORIZATION_POLICY,
        )
        .expect("revoke live authorization grant");
    clear_calls(&composition.calls);
    let denied = composition
        .middleware
        .handle(http_request(
            TENANT,
            &format!("e2e-gateway-revoked-idem-{suffix}"),
            &format!("e2e-gateway-revoked-{suffix}"),
            input_payload(0x41),
            &format!("Bearer {TOKEN}"),
        ))
        .await;
    assert_eq!(denied.status, StatusCode::FORBIDDEN);
    assert_eq!(error_code(&denied.body), "CAPABILITY_PERMISSION_DENIED");
    assert_call_order(&composition.calls, &["authorize"]);
    assert_eq!(evidence_counts(&store).await, after_first);

    let current_head = audit_head(&store).await;
    composition
        .authorization_store
        .upsert(authorization_grant())
        .expect("restore live authorization grant");
    let rollback_record_id = format!("e2e-gateway-rollback-{suffix}");
    let rollback = compose(
        store.clone(),
        CreateRecordPlanner {
            record_id: rollback_record_id,
            audit_sequence: current_head.0,
            previous_hash: current_head.1,
            record_hash: [0xb1; 32],
        },
        FaultInjection::OmitOutbox,
        clock,
        token_store,
    );
    let before_rollback = evidence_counts(&store).await;
    let rollback_response = rollback
        .middleware
        .handle(http_request(
            TENANT,
            &format!("e2e-gateway-rollback-idem-{suffix}"),
            &format!("e2e-gateway-rollback-{suffix}"),
            input_payload(0x51),
            &format!("Bearer {TOKEN}"),
        ))
        .await;
    assert_eq!(rollback_response.status, StatusCode::SERVICE_UNAVAILABLE);
    let rollback_error = transport_error(&rollback_response.body);
    assert_eq!(rollback_error.code, "CAPABILITY_STORAGE_UNAVAILABLE");
    assert_eq!(
        rollback_error.safe_message,
        "The capability could not be persisted at this time."
    );
    assert!(!rollback_error.safe_message.to_lowercase().contains("sql"));
    assert!(
        !rollback_error
            .safe_message
            .to_lowercase()
            .contains("database")
    );
    assert_call_order(&rollback.calls, &["authorize", "batch"]);
    assert_eq!(evidence_counts(&store).await, before_rollback);
    assert_eq!(audit_head(&store).await, current_head);
}

fn compose(
    store: PostgresDataStore,
    planner: CreateRecordPlanner,
    fault: FaultInjection,
    clock: Arc<FixedClock>,
    token_store: AccessTokenStore,
) -> Composition {
    let definition = capability_definition();
    let clock_port: Arc<dyn Clock> = clock.clone();
    let catalog = CapabilityCatalog::new([definition.clone()]).expect("valid capability catalog");
    let rate_policies = RateLimitPolicyStore::default();
    rate_policies
        .upsert(FixedWindowPolicy {
            policy_id: RATE_LIMIT_POLICY.to_owned(),
            maximum_requests: 100,
            window_nanos: 60_000_000_000,
        })
        .expect("valid rate policy");
    let authorization_store = LiveAuthorizationStore::default();
    authorization_store
        .upsert(authorization_grant())
        .expect("valid authorization grant");
    let calls = Arc::new(Mutex::new(Vec::new()));
    let authorizer = RecordingAuthorizer {
        inner: LiveCapabilityAuthorizer::new(authorization_store.clone(), Arc::clone(&clock_port)),
        calls: Arc::clone(&calls),
    };
    let runtime = RecordingBatchRuntime {
        store,
        fault,
        calls: Arc::clone(&calls),
    };
    let executor =
        PostgresTransactionalCapabilityExecutor::from_runtime(Arc::new(runtime), Arc::new(planner));
    let gateway = Arc::new(CapabilityGateway::new(
        Arc::new(catalog),
        Arc::new(SemanticHashValidator),
        Arc::new(FixedWindowRateLimiter::new(
            rate_policies,
            Arc::clone(&clock_port),
        )),
        Arc::new(StoredApprovalVerifier::new(ApprovalStore::default())),
        Arc::new(authorizer),
        Arc::new(executor),
        Arc::clone(&clock_port),
    ));
    let resolver = ExecutionContextResolver::new(
        Arc::clone(&clock_port),
        Arc::new(DeterministicRandom::from_bytes(vec![0x7f; 512])),
        TimeoutPolicy {
            default_millis: 2_000,
            maximum_millis: 5_000,
        },
    )
    .expect("valid context resolver");
    let authenticator = BearerTokenAuthenticator::new(token_store, Arc::clone(&clock_port));
    Composition {
        middleware: HttpCapabilityMiddleware::new(CapabilityIngress::new(
            Arc::new(authenticator),
            resolver,
            gateway,
        )),
        authorization_store,
        calls,
    }
}

fn token_store() -> AccessTokenStore {
    let store = AccessTokenStore::default();
    store
        .issue(
            TOKEN.as_bytes(),
            AccessTokenGrant {
                actor_id: ActorId::try_new(ACTOR).unwrap(),
                tenant_ids: BTreeSet::from([TenantId::try_new(TENANT).unwrap()]),
                authentication_id: "e2e-gateway-session".to_owned(),
                expires_at_unix_nanos: NOW + 10_000_000_000_000,
            },
        )
        .expect("issue E2E access token");
    store
}

fn authorization_grant() -> AuthorizationGrant {
    AuthorizationGrant {
        tenant_id: TenantId::try_new(TENANT).unwrap(),
        actor_id: ActorId::try_new(ACTOR).unwrap(),
        policy_id: AUTHORIZATION_POLICY.to_owned(),
        capability_id: CapabilityId::try_new(CAPABILITY).unwrap(),
        capability_version: CapabilityVersion::try_new(CAPABILITY_VERSION).unwrap(),
        owner_module_id: ModuleId::try_new(MODULE).unwrap(),
        policy_version: "e2e-policy-1".to_owned(),
        expires_at_unix_nanos: Some(NOW + 10_000_000_000_000),
    }
}

fn capability_definition() -> CapabilityDefinition {
    CapabilityDefinition {
        capability_id: CapabilityId::try_new(CAPABILITY).unwrap(),
        capability_version: CapabilityVersion::try_new(CAPABILITY_VERSION).unwrap(),
        owner_module_id: ModuleId::try_new(MODULE).unwrap(),
        input_contract: PayloadContract {
            owner: ModuleId::try_new(MODULE).unwrap(),
            schema_id: SchemaId::try_new("test.gateway.input").unwrap(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
            descriptor_hash: [0x31; 32],
            allowed_data_classes: vec![DataClass::Internal],
            allowed_encodings: vec![PayloadEncoding::Protobuf],
            maximum_size_bytes: 1024,
        },
        output_contract: None,
        risk: CapabilityRisk::High,
        mutation: true,
        requires_idempotency: true,
        requires_approval: false,
        authorization_policy_id: AUTHORIZATION_POLICY.to_owned(),
        rate_limit_policy_id: Some(RATE_LIMIT_POLICY.to_owned()),
    }
}

fn input_payload(value: u8) -> TypedPayload {
    TypedPayload {
        owner: ModuleId::try_new(MODULE).unwrap(),
        schema_id: SchemaId::try_new("test.gateway.input").unwrap(),
        schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
        descriptor_hash: [0x31; 32],
        data_class: DataClass::Internal,
        encoding: PayloadEncoding::Protobuf,
        maximum_size_bytes: 1024,
        retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
        bytes: vec![value],
    }
}

fn http_request(
    tenant: &str,
    idempotency_key: &str,
    request_identity: &str,
    input: TypedPayload,
    authorization: &str,
) -> HttpCapabilityRequest {
    let mut headers = HeaderMap::new();
    insert_header(&mut headers, "authorization", authorization);
    insert_header(&mut headers, TENANT_HEADER, tenant);
    insert_header(&mut headers, IDEMPOTENCY_KEY_HEADER, idempotency_key);
    insert_header(&mut headers, REQUEST_ID_HEADER, request_identity);
    insert_header(
        &mut headers,
        CORRELATION_ID_HEADER,
        &format!("correlation-{request_identity}"),
    );
    insert_header(
        &mut headers,
        CAUSATION_ID_HEADER,
        &format!("causation-{request_identity}"),
    );
    insert_header(
        &mut headers,
        TRACE_ID_HEADER,
        &format!("trace-{request_identity}"),
    );
    insert_header(
        &mut headers,
        BUSINESS_TRANSACTION_HEADER,
        &format!("transaction-{request_identity}"),
    );
    insert_header(&mut headers, TIMEOUT_HEADER, "2000");
    HttpCapabilityRequest {
        headers,
        route: CapabilityRoute {
            owner_module_id: ModuleId::try_new(MODULE).unwrap(),
            capability_id: CapabilityId::try_new(CAPABILITY).unwrap(),
            capability_version: CapabilityVersion::try_new(CAPABILITY_VERSION).unwrap(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
        },
        input,
        approval: None,
    }
}

fn insert_header(headers: &mut HeaderMap, name: &'static str, value: &str) {
    headers.insert(
        name,
        HeaderValue::from_str(value).expect("test header must be valid"),
    );
}

fn success_result(body: &HttpCapabilityBody) -> &crm_capability_runtime::CapabilityExecutionResult {
    match body {
        HttpCapabilityBody::Success(result) => result,
        HttpCapabilityBody::Error(error) => panic!("expected success, received {}", error.code),
    }
}

fn transport_error(body: &HttpCapabilityBody) -> &crm_capability_ingress::SafeTransportError {
    match body {
        HttpCapabilityBody::Error(error) => error,
        HttpCapabilityBody::Success(_) => panic!("expected transport error"),
    }
}

fn error_code(body: &HttpCapabilityBody) -> &str {
    transport_error(body).code.as_str()
}

fn clear_calls(calls: &Arc<Mutex<Vec<&'static str>>>) {
    calls.lock().expect("call-order mutex poisoned").clear();
}

fn assert_call_order(calls: &Arc<Mutex<Vec<&'static str>>>, expected: &[&'static str]) {
    assert_eq!(
        calls.lock().expect("call-order mutex poisoned").as_slice(),
        expected
    );
}

async fn bind_context(transaction: &mut Transaction<'_, Postgres>) {
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
    .bind(TENANT)
    .bind(ACTOR)
    .bind("e2e-gateway-inspection-request")
    .bind(CAPABILITY)
    .bind(CAPABILITY_VERSION)
    .bind("e2e-gateway-inspection-transaction")
    .execute(&mut **transaction)
    .await
    .expect("bind inspection context");
}

async fn evidence_counts(store: &PostgresDataStore) -> EvidenceCounts {
    let mut transaction = store.pool().begin().await.expect("begin inspection");
    bind_context(&mut transaction).await;
    let records = count_prefix(
        &mut transaction,
        "SELECT count(*) FROM crm.records WHERE tenant_id = $1 AND record_id LIKE 'e2e-gateway-%'",
    )
    .await;
    let outbox = count_prefix(
        &mut transaction,
        "SELECT count(*) FROM crm.outbox_events WHERE tenant_id = $1 AND event_id LIKE 'event-e2e-gateway-%'",
    )
    .await;
    let audits = count_prefix(
        &mut transaction,
        "SELECT count(*) FROM crm.audit_records WHERE tenant_id = $1 AND audit_record_id LIKE 'audit-e2e-gateway-%'",
    )
    .await;
    let idempotency = count_prefix(
        &mut transaction,
        "SELECT count(*) FROM crm.idempotency_records WHERE tenant_id = $1 AND idempotency_key LIKE 'e2e-gateway-%'",
    )
    .await;
    let transactions = count_prefix(
        &mut transaction,
        "SELECT count(*) FROM crm.business_transactions WHERE tenant_id = $1 AND business_transaction_id LIKE 'transaction-e2e-gateway-%'",
    )
    .await;
    transaction.commit().await.expect("commit inspection");
    EvidenceCounts {
        records,
        outbox,
        audits,
        idempotency,
        transactions,
    }
}

async fn count_prefix(transaction: &mut Transaction<'_, Postgres>, query: &'static str) -> i64 {
    sqlx::query_scalar(query)
        .bind(TENANT)
        .fetch_one(&mut **transaction)
        .await
        .expect("count E2E evidence")
}

async fn audit_head(store: &PostgresDataStore) -> (i64, [u8; 32]) {
    let mut transaction = store.pool().begin().await.expect("begin audit inspection");
    bind_context(&mut transaction).await;
    let row =
        sqlx::query("SELECT next_sequence, last_hash FROM crm.audit_heads WHERE tenant_id = $1")
            .bind(TENANT)
            .fetch_one(&mut *transaction)
            .await
            .expect("read tenant audit head");
    transaction.commit().await.expect("commit audit inspection");
    let next_sequence: i64 = row.try_get("next_sequence").expect("valid sequence");
    let hash: Vec<u8> = row.try_get("last_hash").expect("valid audit hash");
    (
        next_sequence,
        hash.try_into().expect("audit hash must be 32 bytes"),
    )
}
