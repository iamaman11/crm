#![cfg(feature = "postgres-integration")]

use crm_capability_adapters::{
    ApprovalStore, AuthorizationGrant, FixedWindowRateLimiter, LiveAuthorizationStore,
    LiveCapabilityAuthorizer, LiveQueryVisibilityAuthorizer, LiveQueryVisibilityStore,
    QueryVisibilityGrant, RateLimitPolicyStore, StoredApprovalVerifier,
};
use crm_capability_ingress::{
    AccessTokenGrant, AccessTokenStore, BearerTokenAuthenticator, ERROR_CODE_METADATA,
    GrpcQueryMessage, GrpcQueryMiddleware, HttpQueryBody, HttpQueryMiddleware, HttpQueryRequest,
    QueryContextResolver, QueryIngress, TENANT_HEADER, TimeoutPolicy,
};
use crm_capability_runtime::{
    CapabilityDefinition, CapabilityGateway, CapabilityRequest, CapabilitySemanticValidator,
};
use crm_core_data::{PostgresDataStore, PostgresTransactionalAggregateExecutor};
use crm_module_sdk::testing::{DeterministicRandom, FixedClock};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CausationId, Clock, CorrelationId, DataClass, ExecutionContext,
    IdempotencyKey, ModuleExecutionContext, PayloadEncoding, PortFuture, RecordType, RequestId,
    RetentionPolicyId, SchemaVersion, SdkError, TenantId, TraceId, TypedPayload,
};
use crm_proto_contracts::crm::{
    activities::v1 as activities, core::v1 as core, sales::v1 as sales,
};
use crm_query_runtime::{CursorCodec, QueryGateway};
use crm_sales_activities_capability_composition::{
    SalesActivitiesCapabilityPlannerRouter, capability_catalog, capability_definitions,
    query_capability_catalog, query_capability_definitions,
};
use crm_sales_activities_query_adapter::{
    ACTIVITIES_GET_CAPABILITY, ACTIVITIES_RECORD_TYPE, SALES_GET_CAPABILITY, SALES_RECORD_TYPE,
    SalesActivitiesQueryAdapter,
};
use http::{HeaderMap, HeaderValue, StatusCode};
use prost::Message;
use sqlx::PgPool;
use std::collections::BTreeSet;
use std::sync::Arc;
use tonic::{Code, Request};

const TENANT: &str = "tenant-a";
const OTHER_TENANT: &str = "tenant-b";
const ACTOR: &str = "actor-a";
const NOW: i64 = 1_700_000_400_000_000_000;
const TOKEN: &str = "phase6h-query-ingress-token-000001";
const SALES_CREATE: &str = "sales.deal.create";
const TASK_CREATE: &str = "activities.task.create";
const DEAL_ID: &str = "phase6h-ingress-deal-1";
const TASK_ID: &str = "phase6h-ingress-task-1";

#[derive(Debug)]
struct AcceptMutationSemantics;

impl CapabilitySemanticValidator for AcceptMutationSemantics {
    fn validate<'a>(
        &'a self,
        _definition: &'a CapabilityDefinition,
        _request: &'a CapabilityRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async { Ok(()) })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EvidenceCounts {
    records: i64,
    outbox: i64,
    audits: i64,
    idempotency: i64,
    transactions: i64,
}

#[tokio::test(flavor = "current_thread")]
async fn authenticated_http_and_grpc_queries_need_no_mutation_identity_and_are_side_effect_free() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping query ingress PostgreSQL acceptance because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");

    let mutation_store = PostgresDataStore::connect(&database_url, 4)
        .await
        .expect("connect mutation store");
    let query_store = PostgresDataStore::connect(&database_url, 4)
        .await
        .expect("connect query store");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect evidence reader");
    let clock: Arc<dyn Clock> = Arc::new(FixedClock::new(NOW));

    let authorization_store = LiveAuthorizationStore::default();
    for capability_id in [SALES_CREATE, TASK_CREATE] {
        let definition = mutation_definition(capability_id);
        authorization_store
            .upsert(authorization_grant(&definition))
            .expect("valid mutation authorization grant");
    }
    let sales_get = query_definition(SALES_GET_CAPABILITY);
    let task_get = query_definition(ACTIVITIES_GET_CAPABILITY);
    let sales_get_grant = authorization_grant(&sales_get);
    let task_get_grant = authorization_grant(&task_get);
    authorization_store
        .upsert(sales_get_grant)
        .expect("valid Sales query authorization grant");
    authorization_store
        .upsert(task_get_grant.clone())
        .expect("valid Task query authorization grant");

    let mutation_gateway = CapabilityGateway::new(
        Arc::new(capability_catalog().expect("valid mutation catalog")),
        Arc::new(AcceptMutationSemantics),
        Arc::new(FixedWindowRateLimiter::new(
            RateLimitPolicyStore::default(),
            Arc::clone(&clock),
        )),
        Arc::new(StoredApprovalVerifier::new(ApprovalStore::default())),
        Arc::new(LiveCapabilityAuthorizer::new(
            authorization_store.clone(),
            Arc::clone(&clock),
        )),
        Arc::new(PostgresTransactionalAggregateExecutor::new(
            mutation_store,
            Arc::new(SalesActivitiesCapabilityPlannerRouter),
        )),
        Arc::clone(&clock),
    );

    let sales_create = mutation_definition(SALES_CREATE);
    mutation_gateway
        .execute(mutation_request(
            &sales_create,
            "create-deal",
            1,
            sales::CreateDealRequest {
                deal_id: DEAL_ID.to_owned(),
                name: "Phase 6H transport Deal".to_owned(),
                owner: Some(actor_owner()),
                account: None,
                primary_contact: None,
                stage: Some(sales::DealStage {
                    pipeline_id: "pipeline.phase6h-transport".to_owned(),
                    stage_id: "qualification".to_owned(),
                    ordinal: 1,
                }),
                amount: Some(core::ExactMoney {
                    minor_units: "250000".to_owned(),
                    currency_code: "USD".to_owned(),
                }),
                expected_close_date: Some(core::CalendarDate {
                    year: 2027,
                    month: 12,
                    day: 31,
                }),
                probability_basis_points: 2_500,
            },
        ))
        .await
        .expect("create Deal through mutation gateway");

    let task_create = mutation_definition(TASK_CREATE);
    mutation_gateway
        .execute(mutation_request(
            &task_create,
            "create-task",
            2,
            activities::CreateTaskRequest {
                task_id: TASK_ID.to_owned(),
                subject: "Phase 6H transport Task".to_owned(),
                description: Some("HTTP/gRPC query ingress acceptance".to_owned()),
                owner: Some(actor_owner()),
                related_resources: vec![core::ResourceRef {
                    tenant_id: TENANT.to_owned(),
                    resource_type: SALES_RECORD_TYPE.to_owned(),
                    resource_id: DEAL_ID.to_owned(),
                    version: Some(1),
                }],
                priority: activities::TaskPriority::High as i32,
                due_at: Some(core::UnixTime {
                    unix_nanos: NOW + 100_000_000_000,
                }),
                reminder_at: None,
            },
        ))
        .await
        .expect("create Task through mutation gateway");

    let baseline = evidence_counts(&admin).await;

    let visibility_store = LiveQueryVisibilityStore::default();
    for definition in [&sales_get, &task_get] {
        visibility_store
            .upsert(QueryVisibilityGrant {
                tenant_id: TenantId::try_new(TENANT).unwrap(),
                actor_id: ActorId::try_new(ACTOR).unwrap(),
                capability_id: definition.capability_id.clone(),
                capability_version: definition.capability_version.clone(),
                owner_module_id: definition.owner_module_id.clone(),
                record_type: RecordType::try_new(
                    if definition.owner_module_id.as_str() == "crm.sales" {
                        SALES_RECORD_TYPE
                    } else {
                        ACTIVITIES_RECORD_TYPE
                    },
                )
                .unwrap(),
                record_id: None,
                allowed_fields: if definition.owner_module_id.as_str() == "crm.sales" {
                    sales_fields()
                } else {
                    task_fields()
                },
                policy_version: "phase6h-transport-visibility-1".to_owned(),
                expires_at_unix_nanos: Some(NOW + 10_000_000_000_000),
            })
            .expect("valid query visibility grant");
    }

    let query_adapter = Arc::new(
        SalesActivitiesQueryAdapter::new(
            query_store,
            CursorCodec::new([0x7b; 32]).expect("valid cursor signing key"),
            Arc::new(LiveQueryVisibilityAuthorizer::new(
                visibility_store,
                Arc::clone(&clock),
            )),
        )
        .expect("valid query adapter"),
    );
    let query_gateway = Arc::new(QueryGateway::new(
        Arc::new(query_capability_catalog().expect("valid query catalog")),
        query_adapter.clone(),
        Arc::new(LiveCapabilityAuthorizer::new(
            authorization_store.clone(),
            Arc::clone(&clock),
        )),
        query_adapter,
    ));

    let token_store = AccessTokenStore::default();
    token_store
        .issue(
            TOKEN.as_bytes(),
            AccessTokenGrant {
                actor_id: ActorId::try_new(ACTOR).unwrap(),
                tenant_ids: BTreeSet::from([TenantId::try_new(TENANT).unwrap()]),
                authentication_id: "phase6h-query-session".to_owned(),
                expires_at_unix_nanos: NOW + 10_000_000_000_000,
            },
        )
        .expect("issue query bearer token");
    let authenticator = Arc::new(BearerTokenAuthenticator::new(
        token_store,
        Arc::clone(&clock),
    ));
    let context_resolver = QueryContextResolver::new(
        Arc::clone(&clock),
        Arc::new(DeterministicRandom::from_bytes(0_u8..=127)),
        TimeoutPolicy {
            default_millis: 5_000,
            maximum_millis: 30_000,
        },
    )
    .expect("valid query context resolver");
    let ingress = QueryIngress::new(authenticator, context_resolver, query_gateway);
    let http = HttpQueryMiddleware::new(ingress.clone());
    let grpc = GrpcQueryMiddleware::new(ingress);

    let mut http_headers = HeaderMap::new();
    http_headers.insert(
        "authorization",
        HeaderValue::from_str(&format!("Bearer {TOKEN}")).unwrap(),
    );
    http_headers.insert(TENANT_HEADER, HeaderValue::from_static(TENANT));
    let http_response = http
        .handle(HttpQueryRequest {
            headers: http_headers,
            route: route(&sales_get),
            input: query_payload(
                &sales_get,
                sales::GetDealRequest {
                    deal_id: DEAL_ID.to_owned(),
                },
            ),
        })
        .await;
    assert_eq!(http_response.status, StatusCode::OK);
    assert!(http_response.headers.contains_key("x-request-id"));
    assert!(http_response.headers.contains_key("x-correlation-id"));
    assert!(http_response.headers.contains_key("x-trace-id"));
    let deal_result = match http_response.body {
        HttpQueryBody::Success(result) => result,
        HttpQueryBody::Error(error) => panic!("HTTP Deal query failed: {error:?}"),
    };
    let deal_response = sales::GetDealResponse::decode(deal_result.output.bytes.as_slice())
        .expect("decode HTTP Deal response");
    assert_eq!(
        deal_response.deal.expect("HTTP Deal").name,
        "Phase 6H transport Deal"
    );

    let missing_auth = http
        .handle(HttpQueryRequest {
            headers: HeaderMap::from_iter([(
                TENANT_HEADER.parse().unwrap(),
                HeaderValue::from_static(TENANT),
            )]),
            route: route(&sales_get),
            input: query_payload(
                &sales_get,
                sales::GetDealRequest {
                    deal_id: DEAL_ID.to_owned(),
                },
            ),
        })
        .await;
    assert_eq!(missing_auth.status, StatusCode::UNAUTHORIZED);

    let mut forbidden_headers = HeaderMap::new();
    forbidden_headers.insert(
        "authorization",
        HeaderValue::from_str(&format!("Bearer {TOKEN}")).unwrap(),
    );
    forbidden_headers.insert(TENANT_HEADER, HeaderValue::from_static(OTHER_TENANT));
    let forbidden = http
        .handle(HttpQueryRequest {
            headers: forbidden_headers,
            route: route(&sales_get),
            input: query_payload(
                &sales_get,
                sales::GetDealRequest {
                    deal_id: DEAL_ID.to_owned(),
                },
            ),
        })
        .await;
    assert_eq!(forbidden.status, StatusCode::FORBIDDEN);

    let mut grpc_request = Request::new(GrpcQueryMessage {
        route: route(&task_get),
        input: query_payload(
            &task_get,
            activities::GetTaskRequest {
                task_id: TASK_ID.to_owned(),
            },
        ),
    });
    grpc_request
        .metadata_mut()
        .insert("authorization", format!("Bearer {TOKEN}").parse().unwrap());
    grpc_request
        .metadata_mut()
        .insert(TENANT_HEADER, TENANT.parse().unwrap());
    let grpc_response = grpc
        .handle(grpc_request)
        .await
        .expect("gRPC Task query without mutation-only metadata");
    assert!(grpc_response.metadata().contains_key("x-request-id"));
    assert!(grpc_response.metadata().contains_key("x-correlation-id"));
    assert!(grpc_response.metadata().contains_key("x-trace-id"));
    let task_response =
        activities::GetTaskResponse::decode(grpc_response.into_inner().output.bytes.as_slice())
            .expect("decode gRPC Task response");
    assert_eq!(
        task_response.task.expect("gRPC Task").subject,
        "Phase 6H transport Task"
    );

    assert!(
        authorization_store
            .revoke(
                &task_get_grant.tenant_id,
                &task_get_grant.actor_id,
                &task_get_grant.policy_id,
            )
            .expect("revoke Task query authorization")
    );
    let mut revoked_request = Request::new(GrpcQueryMessage {
        route: route(&task_get),
        input: query_payload(
            &task_get,
            activities::GetTaskRequest {
                task_id: TASK_ID.to_owned(),
            },
        ),
    });
    revoked_request
        .metadata_mut()
        .insert("authorization", format!("Bearer {TOKEN}").parse().unwrap());
    revoked_request
        .metadata_mut()
        .insert(TENANT_HEADER, TENANT.parse().unwrap());
    let revoked = grpc
        .handle(revoked_request)
        .await
        .expect_err("live query authorization revocation must deny gRPC request");
    assert_eq!(revoked.code(), Code::PermissionDenied);
    assert_eq!(
        revoked
            .metadata()
            .get(ERROR_CODE_METADATA)
            .unwrap()
            .to_str()
            .unwrap(),
        "QUERY_PERMISSION_DENIED"
    );

    assert_eq!(evidence_counts(&admin).await, baseline);
}

fn authorization_grant(definition: &CapabilityDefinition) -> AuthorizationGrant {
    AuthorizationGrant {
        tenant_id: TenantId::try_new(TENANT).unwrap(),
        actor_id: ActorId::try_new(ACTOR).unwrap(),
        policy_id: definition.authorization_policy_id.clone(),
        capability_id: definition.capability_id.clone(),
        capability_version: definition.capability_version.clone(),
        owner_module_id: definition.owner_module_id.clone(),
        policy_version: "phase6h-query-transport-policy-1".to_owned(),
        expires_at_unix_nanos: Some(NOW + 10_000_000_000_000),
    }
}

fn mutation_definition(capability_id: &str) -> CapabilityDefinition {
    capability_definitions()
        .expect("valid mutation definitions")
        .into_iter()
        .find(|definition| definition.capability_id.as_str() == capability_id)
        .unwrap_or_else(|| panic!("missing mutation definition: {capability_id}"))
}

fn query_definition(capability_id: &str) -> CapabilityDefinition {
    query_capability_definitions()
        .expect("valid query definitions")
        .into_iter()
        .find(|definition| definition.capability_id.as_str() == capability_id)
        .unwrap_or_else(|| panic!("missing query definition: {capability_id}"))
}

fn mutation_request<M: Message>(
    definition: &CapabilityDefinition,
    identity: &str,
    hash_byte: u8,
    message: M,
) -> CapabilityRequest {
    CapabilityRequest {
        context: ModuleExecutionContext {
            module_id: definition.owner_module_id.clone(),
            execution: ExecutionContext {
                tenant_id: TenantId::try_new(TENANT).unwrap(),
                actor_id: ActorId::try_new(ACTOR).unwrap(),
                request_id: RequestId::try_new(format!("phase6h-ingress-{identity}")).unwrap(),
                correlation_id: CorrelationId::try_new(format!("phase6h-ingress-corr-{identity}"))
                    .unwrap(),
                causation_id: CausationId::try_new(format!("phase6h-ingress-cause-{identity}"))
                    .unwrap(),
                trace_id: TraceId::try_new(format!("phase6h-ingress-trace-{identity}")).unwrap(),
                capability_id: definition.capability_id.clone(),
                capability_version: definition.capability_version.clone(),
                idempotency_key: IdempotencyKey::try_new(format!(
                    "phase6h-ingress-idem-{identity}"
                ))
                .unwrap(),
                business_transaction_id: BusinessTransactionId::try_new(format!(
                    "phase6h-ingress-tx-{identity}"
                ))
                .unwrap(),
                schema_version: definition.input_contract.schema_version.clone(),
                request_started_at_unix_nanos: NOW + i64::from(hash_byte),
            },
        },
        input: query_payload(definition, message),
        input_hash: [hash_byte; 32],
        approval: None,
    }
}

fn query_payload<M: Message>(definition: &CapabilityDefinition, message: M) -> TypedPayload {
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
    payload.validate().expect("valid governed payload");
    payload
}

fn route(definition: &CapabilityDefinition) -> crm_capability_ingress::CapabilityRoute {
    crm_capability_ingress::CapabilityRoute {
        owner_module_id: definition.owner_module_id.clone(),
        capability_id: definition.capability_id.clone(),
        capability_version: definition.capability_version.clone(),
        schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
    }
}

fn actor_owner() -> core::ActorOrTeamOwner {
    core::ActorOrTeamOwner {
        owner: Some(core::actor_or_team_owner::Owner::ActorId(ACTOR.to_owned())),
    }
}

fn sales_fields() -> BTreeSet<String> {
    [
        "name",
        "stage",
        "amount",
        "owner",
        "expected_close_date",
        "probability_basis_points",
        "status",
        "created_at",
        "updated_at",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn task_fields() -> BTreeSet<String> {
    [
        "subject",
        "description",
        "owner",
        "related_resources",
        "priority",
        "status",
        "due_at",
        "created_at",
        "updated_at",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

async fn evidence_counts(pool: &PgPool) -> EvidenceCounts {
    EvidenceCounts {
        records: scalar_count(pool, "SELECT count(*) FROM crm.records").await,
        outbox: scalar_count(pool, "SELECT count(*) FROM crm.outbox_events").await,
        audits: scalar_count(pool, "SELECT count(*) FROM crm.audit_records").await,
        idempotency: scalar_count(pool, "SELECT count(*) FROM crm.idempotency_records").await,
        transactions: scalar_count(pool, "SELECT count(*) FROM crm.business_transactions").await,
    }
}

async fn scalar_count(pool: &PgPool, statement: &'static str) -> i64 {
    sqlx::query_scalar::<_, i64>(statement)
        .fetch_one(pool)
        .await
        .expect("read evidence count")
}
