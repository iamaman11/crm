#![cfg(feature = "postgres-integration")]

use ::http::{HeaderMap, HeaderValue, StatusCode};
use crm_capability_adapters::{
    ApprovalStore, AuthorizationGrant, FixedWindowRateLimiter, LiveAuthorizationStore,
    LiveCapabilityAuthorizer, RateLimitPolicyStore, StoredApprovalVerifier,
};
use crm_capability_ingress::{
    AccessTokenGrant, AccessTokenStore, BUSINESS_TRANSACTION_HEADER, BearerTokenAuthenticator,
    CAUSATION_ID_HEADER, CORRELATION_ID_HEADER, CapabilityIngress, CapabilityRoute,
    ERROR_CODE_METADATA, ExecutionContextResolver, GrpcCapabilityMessage, GrpcCapabilityMiddleware,
    HttpCapabilityBody, HttpCapabilityMiddleware, HttpCapabilityRequest, IDEMPOTENCY_KEY_HEADER,
    REQUEST_ID_HEADER, TENANT_HEADER, TIMEOUT_HEADER, TRACE_ID_HEADER, TimeoutPolicy,
    semantic_input_hash,
};
use crm_capability_runtime::{
    AuthorizationDecision, CapabilityAuthorizer, CapabilityDefinition, CapabilityGateway,
    CapabilityRequest, CapabilitySemanticValidator, TransactionalCapabilityExecutor,
};
use crm_core_data::{PostgresDataStore, PostgresTransactionalAggregateExecutor};
use crm_module_sdk::testing::{DeterministicRandom, FixedClock};
use crm_module_sdk::{
    ActorId, Clock, DataClass, ErrorCategory, PayloadEncoding, PortFuture, RetentionPolicyId,
    SdkError, TenantId, TypedPayload,
};
use crm_proto_contracts::crm::{
    activities::v1 as activities, core::v1 as core, sales::v1 as sales,
};
use crm_sales_activities_capability_composition::{
    SalesActivitiesCapabilityPlannerRouter, capability_catalog, capability_definitions,
};
use prost::Message;
use sqlx::{PgPool, Row};
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use tonic::metadata::MetadataValue;
use tonic::{Code, Request};

const TENANT: &str = "tenant-a";
const OTHER_TENANT: &str = "tenant-b";
const ACTOR: &str = "actor-a";
const TOKEN: &str = "phase6g-0123456789abcdef0123456789abcdef";
const NOW: i64 = 1_700_000_200_000_000_000;

const SALES_CREATE: &str = "sales.deal.create";
const SALES_UPDATE: &str = "sales.deal.update";
const SALES_ADVANCE: &str = "sales.deal.advance_stage";
const TASK_CREATE: &str = "activities.task.create";
const TASK_UPDATE: &str = "activities.task.update";
const TASK_COMPLETE: &str = "activities.task.complete";
const TASK_REMINDER: &str = "activities.task.schedule_reminder";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EvidenceCounts {
    records: i64,
    outbox: i64,
    audits: i64,
    idempotency: i64,
    transactions: i64,
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
                .expect("call mutex poisoned")
                .push("authorize");
            decision
        })
    }
}

#[derive(Clone)]
struct RecordingExecutor {
    inner: PostgresTransactionalAggregateExecutor,
    calls: Arc<Mutex<Vec<&'static str>>>,
}

impl TransactionalCapabilityExecutor for RecordingExecutor {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: CapabilityRequest,
    ) -> PortFuture<'a, Result<crm_capability_runtime::CapabilityExecutionResult, SdkError>> {
        Box::pin(async move {
            self.calls
                .lock()
                .expect("call mutex poisoned")
                .push("execute");
            self.inner.execute(definition, request).await
        })
    }
}

struct Composition {
    http: HttpCapabilityMiddleware,
    grpc: GrpcCapabilityMiddleware,
    authorization_store: LiveAuthorizationStore,
    calls: Arc<Mutex<Vec<&'static str>>>,
}

#[tokio::test(flavor = "current_thread")]
async fn production_sales_and_activities_mutations_use_the_authenticated_public_postgres_path() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping Phase 6G PostgreSQL acceptance because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let store = PostgresDataStore::connect(&database_url, 4)
        .await
        .expect("connect Phase 6G runtime store");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect Phase 6G evidence reader");
    let composition = compose(store);
    let baseline = evidence_counts(&admin).await;

    let deal_id = "phase6g-deal";
    let task_id = "phase6g-task";

    clear_calls(&composition.calls);
    let invalid_auth = composition
        .http
        .handle(http_request(
            &definition(SALES_CREATE),
            TENANT,
            "phase6g-idem-invalid-auth",
            "phase6g-invalid-auth",
            payload(
                &definition(SALES_CREATE),
                sales_create(deal_id, "Invalid auth"),
            ),
            "Bearer invalid-invalid-invalid-invalid",
        ))
        .await;
    assert_eq!(invalid_auth.status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        http_error_code(&invalid_auth.body),
        "AUTHENTICATION_INVALID"
    );
    assert_calls(&composition.calls, &[]);
    assert_eq!(evidence_counts(&admin).await, baseline);

    clear_calls(&composition.calls);
    let tenant_denied = composition
        .grpc
        .handle(grpc_request(
            &definition(SALES_CREATE),
            OTHER_TENANT,
            "phase6g-idem-tenant-denied",
            "phase6g-tenant-denied",
            payload(
                &definition(SALES_CREATE),
                sales_create(deal_id, "Tenant denied"),
            ),
            &format!("Bearer {TOKEN}"),
        ))
        .await
        .unwrap_err();
    assert_eq!(tenant_denied.code(), Code::PermissionDenied);
    assert_eq!(grpc_error_code(&tenant_denied), "TENANT_FORBIDDEN");
    assert_calls(&composition.calls, &[]);
    assert_eq!(evidence_counts(&admin).await, baseline);

    clear_calls(&composition.calls);
    let cross_tenant_task = activities::CreateTaskRequest {
        task_id: "phase6g-cross-tenant-task".to_owned(),
        subject: "Reject cross-tenant relation".to_owned(),
        description: None,
        owner: Some(actor_owner()),
        related_resources: vec![core::ResourceRef {
            tenant_id: OTHER_TENANT.to_owned(),
            resource_type: "sales.deal".to_owned(),
            resource_id: deal_id.to_owned(),
            version: None,
        }],
        priority: activities::TaskPriority::Normal as i32,
        due_at: None,
        reminder_at: None,
    };
    let cross_tenant = composition
        .http
        .handle(http_request(
            &definition(TASK_CREATE),
            TENANT,
            "phase6g-idem-cross-tenant",
            "phase6g-cross-tenant",
            payload(&definition(TASK_CREATE), cross_tenant_task),
            &format!("Bearer {TOKEN}"),
        ))
        .await;
    assert_eq!(cross_tenant.status, StatusCode::BAD_REQUEST);
    assert_calls(&composition.calls, &["authorize", "execute"]);
    assert_eq!(evidence_counts(&admin).await, baseline);

    let sales_create_definition = definition(SALES_CREATE);
    let create_message = sales_create(deal_id, "Enterprise renewal");
    clear_calls(&composition.calls);
    let create = composition
        .http
        .handle(http_request(
            &sales_create_definition,
            TENANT,
            "phase6g-idem-sales-create",
            "phase6g-sales-create",
            payload(&sales_create_definition, create_message.clone()),
            &format!("Bearer {TOKEN}"),
        ))
        .await;
    assert_eq!(create.status, StatusCode::OK);
    assert_calls(&composition.calls, &["authorize", "execute"]);
    let created_result = http_success(&create.body).clone();
    assert!(!created_result.replayed);
    let created_deal =
        sales::CreateDealResponse::decode(created_result.output.as_ref().unwrap().bytes.as_slice())
            .unwrap()
            .deal
            .unwrap();
    assert_eq!(created_deal.version, 1);
    let after_sales_create = evidence_counts(&admin).await;
    assert_mutation_delta(after_sales_create, baseline, 1, 1);

    clear_calls(&composition.calls);
    let replay = composition
        .http
        .handle(http_request(
            &sales_create_definition,
            TENANT,
            "phase6g-idem-sales-create",
            "phase6g-sales-create-replay",
            payload(&sales_create_definition, create_message.clone()),
            &format!("Bearer {TOKEN}"),
        ))
        .await;
    assert_eq!(replay.status, StatusCode::OK);
    assert_calls(&composition.calls, &["authorize", "execute"]);
    let replay_result = http_success(&replay.body);
    assert!(replay_result.replayed);
    assert_eq!(replay_result.output, created_result.output);
    assert_eq!(
        replay_result.affected_resources,
        created_result.affected_resources
    );
    assert_eq!(evidence_counts(&admin).await, after_sales_create);

    clear_calls(&composition.calls);
    let idempotency_conflict = composition
        .http
        .handle(http_request(
            &sales_create_definition,
            TENANT,
            "phase6g-idem-sales-create",
            "phase6g-sales-create-conflict",
            payload(
                &sales_create_definition,
                sales_create(deal_id, "Different semantic input"),
            ),
            &format!("Bearer {TOKEN}"),
        ))
        .await;
    assert_eq!(idempotency_conflict.status, StatusCode::CONFLICT);
    assert_eq!(
        http_error_code(&idempotency_conflict.body),
        "CAPABILITY_IDEMPOTENCY_KEY_REUSED"
    );
    assert_eq!(evidence_counts(&admin).await, after_sales_create);

    let sales_update_definition = definition(SALES_UPDATE);
    let update = composition
        .http
        .handle(http_request(
            &sales_update_definition,
            TENANT,
            "phase6g-idem-sales-update",
            "phase6g-sales-update",
            payload(
                &sales_update_definition,
                sales_update(deal_id, 1, "Enterprise renewal 2027"),
            ),
            &format!("Bearer {TOKEN}"),
        ))
        .await;
    assert_eq!(update.status, StatusCode::OK);
    let after_sales_update = evidence_counts(&admin).await;
    assert_mutation_delta(after_sales_update, after_sales_create, 0, 1);

    let stale = composition
        .http
        .handle(http_request(
            &sales_update_definition,
            TENANT,
            "phase6g-idem-sales-stale",
            "phase6g-sales-stale",
            payload(
                &sales_update_definition,
                sales_update(deal_id, 1, "Stale write"),
            ),
            &format!("Bearer {TOKEN}"),
        ))
        .await;
    assert_eq!(stale.status, StatusCode::CONFLICT);
    assert_eq!(evidence_counts(&admin).await, after_sales_update);

    let sales_advance_definition = definition(SALES_ADVANCE);
    let advance = composition
        .http
        .handle(http_request(
            &sales_advance_definition,
            TENANT,
            "phase6g-idem-sales-advance",
            "phase6g-sales-advance",
            payload(&sales_advance_definition, sales_advance(deal_id, 2)),
            &format!("Bearer {TOKEN}"),
        ))
        .await;
    assert_eq!(advance.status, StatusCode::OK);
    let after_sales = evidence_counts(&admin).await;
    assert_mutation_delta(after_sales, after_sales_update, 0, 1);

    let task_create_definition = definition(TASK_CREATE);
    let task_create_result = composition
        .grpc
        .handle(grpc_request(
            &task_create_definition,
            TENANT,
            "phase6g-idem-task-create",
            "phase6g-task-create",
            payload(&task_create_definition, task_create(task_id, deal_id)),
            &format!("Bearer {TOKEN}"),
        ))
        .await
        .unwrap()
        .into_inner();
    let created_task = activities::CreateTaskResponse::decode(
        task_create_result.output.as_ref().unwrap().bytes.as_slice(),
    )
    .unwrap()
    .task
    .unwrap();
    assert_eq!(created_task.version, 1);
    let after_task_create = evidence_counts(&admin).await;
    assert_mutation_delta(after_task_create, after_sales, 1, 1);

    let task_update_definition = definition(TASK_UPDATE);
    let task_update = composition
        .http
        .handle(http_request(
            &task_update_definition,
            TENANT,
            "phase6g-idem-task-update",
            "phase6g-task-update",
            payload(&task_update_definition, task_update(task_id, 1)),
            &format!("Bearer {TOKEN}"),
        ))
        .await;
    assert_eq!(task_update.status, StatusCode::OK);
    let after_task_update = evidence_counts(&admin).await;
    assert_mutation_delta(after_task_update, after_task_create, 0, 1);

    let reminder_definition = definition(TASK_REMINDER);
    let reminder = composition
        .http
        .handle(http_request(
            &reminder_definition,
            TENANT,
            "phase6g-idem-task-reminder",
            "phase6g-task-reminder",
            payload(&reminder_definition, schedule_reminder(task_id, 2)),
            &format!("Bearer {TOKEN}"),
        ))
        .await;
    assert_eq!(reminder.status, StatusCode::OK);
    let after_reminder = evidence_counts(&admin).await;
    assert_mutation_delta(after_reminder, after_task_update, 0, 1);

    let complete_definition = definition(TASK_COMPLETE);
    let complete = composition
        .http
        .handle(http_request(
            &complete_definition,
            TENANT,
            "phase6g-idem-task-complete",
            "phase6g-task-complete",
            payload(&complete_definition, complete_task(task_id, 3)),
            &format!("Bearer {TOKEN}"),
        ))
        .await;
    assert_eq!(complete.status, StatusCode::OK);
    let completed = activities::CompleteTaskResponse::decode(
        http_success(&complete.body)
            .output
            .as_ref()
            .unwrap()
            .bytes
            .as_slice(),
    )
    .unwrap();
    assert!(completed.changed);
    assert_eq!(completed.task.unwrap().version, 4);
    let after_complete = evidence_counts(&admin).await;
    assert_mutation_delta(after_complete, after_reminder, 0, 1);

    let noop = composition
        .grpc
        .handle(grpc_request(
            &complete_definition,
            TENANT,
            "phase6g-idem-task-complete-noop",
            "phase6g-task-complete-noop",
            payload(&complete_definition, complete_task(task_id, 4)),
            &format!("Bearer {TOKEN}"),
        ))
        .await
        .unwrap()
        .into_inner();
    let noop_output =
        activities::CompleteTaskResponse::decode(noop.output.as_ref().unwrap().bytes.as_slice())
            .unwrap();
    assert!(!noop_output.changed);
    assert_eq!(noop_output.task.unwrap().version, 4);
    let after_noop = evidence_counts(&admin).await;
    assert_noop_delta(after_noop, after_complete);

    composition
        .authorization_store
        .revoke(
            &TenantId::try_new(TENANT).unwrap(),
            &ActorId::try_new(ACTOR).unwrap(),
            TASK_REMINDER,
        )
        .expect("revoke production reminder permission");

    clear_calls(&composition.calls);
    let denied_http = composition
        .http
        .handle(http_request(
            &reminder_definition,
            TENANT,
            "phase6g-idem-reminder-denied-http",
            "phase6g-reminder-denied-http",
            payload(&reminder_definition, schedule_reminder(task_id, 4)),
            &format!("Bearer {TOKEN}"),
        ))
        .await;
    assert_eq!(denied_http.status, StatusCode::FORBIDDEN);
    assert_eq!(
        http_error_code(&denied_http.body),
        "CAPABILITY_PERMISSION_DENIED"
    );
    assert_calls(&composition.calls, &["authorize"]);
    assert_eq!(evidence_counts(&admin).await, after_noop);

    clear_calls(&composition.calls);
    let denied_grpc = composition
        .grpc
        .handle(grpc_request(
            &reminder_definition,
            TENANT,
            "phase6g-idem-reminder-denied-grpc",
            "phase6g-reminder-denied-grpc",
            payload(&reminder_definition, schedule_reminder(task_id, 4)),
            &format!("Bearer {TOKEN}"),
        ))
        .await
        .unwrap_err();
    assert_eq!(denied_grpc.code(), Code::PermissionDenied);
    assert_eq!(
        grpc_error_code(&denied_grpc),
        "CAPABILITY_PERMISSION_DENIED"
    );
    assert_calls(&composition.calls, &["authorize"]);
    assert_eq!(evidence_counts(&admin).await, after_noop);

    assert_eq!(after_noop.records, baseline.records + 2);
    assert_eq!(after_noop.outbox, baseline.outbox + 7);
    assert_eq!(after_noop.audits, baseline.audits + 8);
    assert_eq!(after_noop.idempotency, baseline.idempotency + 8);
    assert_eq!(after_noop.transactions, baseline.transactions + 8);
}

fn compose(store: PostgresDataStore) -> Composition {
    let clock = Arc::new(FixedClock::new(NOW));
    let clock_port: Arc<dyn Clock> = clock.clone();
    let token_store = token_store();
    let authorization_store = LiveAuthorizationStore::default();
    for definition in capability_definitions().expect("valid production capability definitions") {
        authorization_store
            .upsert(AuthorizationGrant {
                tenant_id: TenantId::try_new(TENANT).unwrap(),
                actor_id: ActorId::try_new(ACTOR).unwrap(),
                policy_id: definition.authorization_policy_id.clone(),
                capability_id: definition.capability_id.clone(),
                capability_version: definition.capability_version.clone(),
                owner_module_id: definition.owner_module_id.clone(),
                policy_version: "phase6g-policy-1".to_owned(),
                expires_at_unix_nanos: Some(NOW + 10_000_000_000_000),
            })
            .expect("valid production authorization grant");
    }

    let calls = Arc::new(Mutex::new(Vec::new()));
    let authorizer = RecordingAuthorizer {
        inner: LiveCapabilityAuthorizer::new(authorization_store.clone(), Arc::clone(&clock_port)),
        calls: Arc::clone(&calls),
    };
    let executor = RecordingExecutor {
        inner: PostgresTransactionalAggregateExecutor::new(
            store,
            Arc::new(SalesActivitiesCapabilityPlannerRouter),
        ),
        calls: Arc::clone(&calls),
    };
    let gateway = Arc::new(CapabilityGateway::new(
        Arc::new(capability_catalog().expect("valid production capability catalog")),
        Arc::new(SemanticHashValidator),
        Arc::new(FixedWindowRateLimiter::new(
            RateLimitPolicyStore::default(),
            Arc::clone(&clock_port),
        )),
        Arc::new(StoredApprovalVerifier::new(ApprovalStore::default())),
        Arc::new(authorizer),
        Arc::new(executor),
        Arc::clone(&clock_port),
    ));
    let resolver = ExecutionContextResolver::new(
        Arc::clone(&clock_port),
        Arc::new(DeterministicRandom::from_bytes(vec![0x61; 2048])),
        TimeoutPolicy {
            default_millis: 5_000,
            maximum_millis: 10_000,
        },
    )
    .expect("valid Phase 6G context resolver");
    let authenticator = BearerTokenAuthenticator::new(token_store, clock_port);
    let ingress = CapabilityIngress::new(Arc::new(authenticator), resolver, gateway);

    Composition {
        http: HttpCapabilityMiddleware::new(ingress.clone()),
        grpc: GrpcCapabilityMiddleware::new(ingress),
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
                authentication_id: "phase6g-session".to_owned(),
                expires_at_unix_nanos: NOW + 10_000_000_000_000,
            },
        )
        .expect("issue Phase 6G access token");
    store
}

fn definition(capability_id: &str) -> CapabilityDefinition {
    capability_definitions()
        .expect("valid production definitions")
        .into_iter()
        .find(|definition| definition.capability_id.as_str() == capability_id)
        .unwrap_or_else(|| panic!("missing production capability definition: {capability_id}"))
}

fn payload<M: Message>(definition: &CapabilityDefinition, message: M) -> TypedPayload {
    let payload = TypedPayload {
        owner: definition.input_contract.owner.clone(),
        schema_id: definition.input_contract.schema_id.clone(),
        schema_version: definition.input_contract.schema_version.clone(),
        descriptor_hash: definition.input_contract.descriptor_hash,
        data_class: DataClass::Confidential,
        encoding: PayloadEncoding::Protobuf,
        maximum_size_bytes: definition.input_contract.maximum_size_bytes,
        retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
        bytes: message.encode_to_vec(),
    };
    payload.validate().expect("valid production input payload");
    payload
}

fn route(definition: &CapabilityDefinition) -> CapabilityRoute {
    CapabilityRoute {
        owner_module_id: definition.owner_module_id.clone(),
        capability_id: definition.capability_id.clone(),
        capability_version: definition.capability_version.clone(),
        schema_version: definition.input_contract.schema_version.clone(),
    }
}

fn http_request(
    definition: &CapabilityDefinition,
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
        &format!("phase6g-correlation-{request_identity}"),
    );
    insert_header(
        &mut headers,
        CAUSATION_ID_HEADER,
        &format!("phase6g-causation-{request_identity}"),
    );
    insert_header(
        &mut headers,
        TRACE_ID_HEADER,
        &format!("phase6g-trace-{request_identity}"),
    );
    insert_header(
        &mut headers,
        BUSINESS_TRANSACTION_HEADER,
        &format!("phase6g-tx-{request_identity}"),
    );
    insert_header(&mut headers, TIMEOUT_HEADER, "5000");
    HttpCapabilityRequest {
        headers,
        route: route(definition),
        input,
        approval: None,
    }
}

fn grpc_request(
    definition: &CapabilityDefinition,
    tenant: &str,
    idempotency_key: &str,
    request_identity: &str,
    input: TypedPayload,
    authorization: &str,
) -> Request<GrpcCapabilityMessage> {
    let mut request = Request::new(GrpcCapabilityMessage {
        route: route(definition),
        input,
        approval: None,
    });
    insert_metadata(request.metadata_mut(), "authorization", authorization);
    insert_metadata(request.metadata_mut(), TENANT_HEADER, tenant);
    insert_metadata(
        request.metadata_mut(),
        IDEMPOTENCY_KEY_HEADER,
        idempotency_key,
    );
    insert_metadata(request.metadata_mut(), REQUEST_ID_HEADER, request_identity);
    insert_metadata(
        request.metadata_mut(),
        CORRELATION_ID_HEADER,
        &format!("phase6g-correlation-{request_identity}"),
    );
    insert_metadata(
        request.metadata_mut(),
        CAUSATION_ID_HEADER,
        &format!("phase6g-causation-{request_identity}"),
    );
    insert_metadata(
        request.metadata_mut(),
        TRACE_ID_HEADER,
        &format!("phase6g-trace-{request_identity}"),
    );
    insert_metadata(
        request.metadata_mut(),
        BUSINESS_TRANSACTION_HEADER,
        &format!("phase6g-tx-{request_identity}"),
    );
    insert_metadata(request.metadata_mut(), TIMEOUT_HEADER, "5000");
    request
}

fn insert_header(headers: &mut HeaderMap, name: &'static str, value: &str) {
    headers.insert(
        name,
        HeaderValue::from_str(value).expect("valid Phase 6G HTTP header"),
    );
}

fn insert_metadata(metadata: &mut tonic::metadata::MetadataMap, name: &'static str, value: &str) {
    metadata.insert(
        name,
        MetadataValue::try_from(value).expect("valid Phase 6G gRPC metadata"),
    );
}

fn http_success(body: &HttpCapabilityBody) -> &crm_capability_runtime::CapabilityExecutionResult {
    match body {
        HttpCapabilityBody::Success(result) => result,
        HttpCapabilityBody::Error(error) => panic!("expected success, received {}", error.code),
    }
}

fn http_error_code(body: &HttpCapabilityBody) -> &str {
    match body {
        HttpCapabilityBody::Error(error) => error.code.as_str(),
        HttpCapabilityBody::Success(_) => panic!("expected transport error"),
    }
}

fn grpc_error_code(status: &tonic::Status) -> &str {
    status
        .metadata()
        .get(ERROR_CODE_METADATA)
        .expect("typed gRPC error code")
        .to_str()
        .expect("ASCII gRPC error code")
}

fn clear_calls(calls: &Arc<Mutex<Vec<&'static str>>>) {
    calls.lock().expect("call mutex poisoned").clear();
}

fn assert_calls(calls: &Arc<Mutex<Vec<&'static str>>>, expected: &[&'static str]) {
    assert_eq!(
        calls.lock().expect("call mutex poisoned").as_slice(),
        expected
    );
}

fn actor_owner() -> core::ActorOrTeamOwner {
    core::ActorOrTeamOwner {
        owner: Some(core::actor_or_team_owner::Owner::ActorId(ACTOR.to_owned())),
    }
}

fn sales_create(deal_id: &str, name: &str) -> sales::CreateDealRequest {
    sales::CreateDealRequest {
        deal_id: deal_id.to_owned(),
        name: name.to_owned(),
        owner: Some(actor_owner()),
        account: None,
        primary_contact: None,
        stage: Some(sales::DealStage {
            pipeline_id: "pipeline.enterprise".to_owned(),
            stage_id: "qualification".to_owned(),
            ordinal: 1,
        }),
        amount: Some(core::ExactMoney {
            minor_units: "125000000000000000000".to_owned(),
            currency_code: "USD".to_owned(),
        }),
        expected_close_date: Some(core::CalendarDate {
            year: 2027,
            month: 12,
            day: 31,
        }),
        probability_basis_points: 2_500,
    }
}

fn sales_update(deal_id: &str, expected_version: i64, name: &str) -> sales::UpdateDealRequest {
    sales::UpdateDealRequest {
        deal_id: deal_id.to_owned(),
        expected_version,
        name: Some(core::StringPatch {
            operation: Some(core::string_patch::Operation::Set(name.to_owned())),
        }),
        owner: None,
        account: None,
        primary_contact: None,
        amount: None,
        expected_close_date: None,
        probability_basis_points: Some(core::UInt32Patch {
            operation: Some(core::u_int32_patch::Operation::Set(4_000)),
        }),
    }
}

fn sales_advance(deal_id: &str, expected_version: i64) -> sales::AdvanceStageRequest {
    sales::AdvanceStageRequest {
        deal_id: deal_id.to_owned(),
        expected_version,
        target_stage: Some(sales::DealStage {
            pipeline_id: "pipeline.enterprise".to_owned(),
            stage_id: "proposal".to_owned(),
            ordinal: 2,
        }),
        target_status: sales::DealStatus::Open as i32,
        close_reason_code: None,
        policy: Some(sales::StageTransitionPolicy {
            allow_regression: false,
            allow_skip: false,
        }),
    }
}

fn task_create(task_id: &str, deal_id: &str) -> activities::CreateTaskRequest {
    activities::CreateTaskRequest {
        task_id: task_id.to_owned(),
        subject: "Prepare proposal".to_owned(),
        description: Some("Production Phase 6G task".to_owned()),
        owner: Some(actor_owner()),
        related_resources: vec![core::ResourceRef {
            tenant_id: TENANT.to_owned(),
            resource_type: "sales.deal".to_owned(),
            resource_id: deal_id.to_owned(),
            version: Some(3),
        }],
        priority: activities::TaskPriority::Normal as i32,
        due_at: Some(core::UnixTime {
            unix_nanos: NOW + 100_000_000_000,
        }),
        reminder_at: None,
    }
}

fn task_update(task_id: &str, expected_version: i64) -> activities::UpdateTaskRequest {
    activities::UpdateTaskRequest {
        task_id: task_id.to_owned(),
        expected_version,
        subject: Some(core::StringPatch {
            operation: Some(core::string_patch::Operation::Set(
                "Prepare final proposal".to_owned(),
            )),
        }),
        description: None,
        owner: None,
        priority: Some(activities::TaskPriorityPatch {
            operation: Some(activities::task_priority_patch::Operation::Set(
                activities::TaskPriority::High as i32,
            )),
        }),
        due_at: None,
    }
}

fn schedule_reminder(task_id: &str, expected_version: i64) -> activities::ScheduleReminderRequest {
    activities::ScheduleReminderRequest {
        task_id: task_id.to_owned(),
        expected_version,
        reminder_at: Some(core::UnixTime {
            unix_nanos: NOW + 50_000_000_000,
        }),
    }
}

fn complete_task(task_id: &str, expected_version: i64) -> activities::CompleteTaskRequest {
    activities::CompleteTaskRequest {
        task_id: task_id.to_owned(),
        expected_version,
    }
}

async fn evidence_counts(pool: &PgPool) -> EvidenceCounts {
    EvidenceCounts {
        records: scalar_count(
            pool,
            "SELECT count(*) FROM crm.records WHERE tenant_id = $1 AND record_id LIKE 'phase6g-%'",
        )
        .await,
        outbox: scalar_count(
            pool,
            "SELECT count(*) FROM crm.outbox_events WHERE tenant_id = $1 AND aggregate_id LIKE 'phase6g-%'",
        )
        .await,
        audits: scalar_count(
            pool,
            "SELECT count(*) FROM crm.audit_records WHERE tenant_id = $1 AND business_transaction_id LIKE 'phase6g-tx-%'",
        )
        .await,
        idempotency: scalar_count(
            pool,
            "SELECT count(*) FROM crm.idempotency_records WHERE tenant_id = $1 AND idempotency_key LIKE 'phase6g-idem-%'",
        )
        .await,
        transactions: scalar_count(
            pool,
            "SELECT count(*) FROM crm.business_transactions WHERE tenant_id = $1 AND business_transaction_id LIKE 'phase6g-tx-%'",
        )
        .await,
    }
}

async fn scalar_count(pool: &PgPool, query: &'static str) -> i64 {
    sqlx::query(query)
        .bind(TENANT)
        .fetch_one(pool)
        .await
        .expect("count Phase 6G evidence")
        .try_get(0)
        .expect("valid Phase 6G count")
}

fn assert_mutation_delta(
    current: EvidenceCounts,
    previous: EvidenceCounts,
    record_delta: i64,
    transaction_delta: i64,
) {
    assert_eq!(current.records, previous.records + record_delta);
    assert_eq!(current.outbox, previous.outbox + transaction_delta);
    assert_eq!(current.audits, previous.audits + transaction_delta);
    assert_eq!(
        current.idempotency,
        previous.idempotency + transaction_delta
    );
    assert_eq!(
        current.transactions,
        previous.transactions + transaction_delta
    );
}

fn assert_noop_delta(current: EvidenceCounts, previous: EvidenceCounts) {
    assert_eq!(current.records, previous.records);
    assert_eq!(current.outbox, previous.outbox);
    assert_eq!(current.audits, previous.audits + 1);
    assert_eq!(current.idempotency, previous.idempotency + 1);
    assert_eq!(current.transactions, previous.transactions + 1);
}
