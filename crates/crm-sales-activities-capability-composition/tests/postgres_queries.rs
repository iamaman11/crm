#![cfg(feature = "postgres-integration")]

use crm_capability_adapters::{
    ApprovalStore, AuthorizationGrant, FixedWindowRateLimiter, LiveAuthorizationStore,
    LiveCapabilityAuthorizer, LiveQueryVisibilityAuthorizer, LiveQueryVisibilityStore,
    QueryVisibilityGrant, RateLimitPolicyStore, StoredApprovalVerifier,
};
use crm_capability_runtime::{
    CapabilityDefinition, CapabilityGateway, CapabilityRequest, CapabilitySemanticValidator,
};
use crm_core_data::{PostgresDataStore, PostgresTransactionalAggregateExecutor};
use crm_module_sdk::testing::FixedClock;
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, Clock,
    CorrelationId, DataClass, ExecutionContext, IdempotencyKey, ModuleExecutionContext, ModuleId,
    PayloadEncoding, PortFuture, RecordId, RecordType, RequestId, RetentionPolicyId, SchemaVersion,
    SdkError, TenantId, TraceId, TypedPayload,
};
use crm_proto_contracts::crm::{
    activities::v1 as activities, core::v1 as core, sales::v1 as sales,
};
use crm_query_runtime::{
    CursorCodec, QueryExecutionContext, QueryGateway, QueryGatewayError, QueryRequest,
};
use crm_sales_activities_capability_composition::{
    SalesActivitiesCapabilityPlannerRouter, capability_catalog, capability_definitions,
    query_capability_catalog, query_capability_definitions,
};
use crm_sales_activities_query_adapter::{
    ACTIVITIES_GET_CAPABILITY, ACTIVITIES_LIST_CAPABILITY, ACTIVITIES_RECORD_TYPE,
    SALES_GET_CAPABILITY, SALES_LIST_CAPABILITY, SALES_RECORD_TYPE, SalesActivitiesQueryAdapter,
};
use prost::Message;
use sqlx::PgPool;
use std::collections::BTreeSet;
use std::sync::Arc;

const TENANT: &str = "tenant-a";
const OTHER_TENANT: &str = "tenant-b";
const ACTOR: &str = "actor-a";
const NO_VISIBILITY_ACTOR: &str = "actor-query-no-visibility";
const NOW: i64 = 1_700_000_300_000_000_000;
const SALES_CREATE: &str = "sales.deal.create";
const TASK_CREATE: &str = "activities.task.create";
const QUERY_PIPELINE: &str = "pipeline.phase6h-query";

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
async fn production_queries_are_permission_bound_cursor_safe_and_side_effect_free() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping Phase 6H PostgreSQL query acceptance because DATABASE_URL is absent");
        return;
    };
    let admin_database_url =
        std::env::var("ADMIN_DATABASE_URL").expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");

    let mutation_store = PostgresDataStore::connect(&database_url, 4)
        .await
        .expect("connect Phase 6H mutation store");
    let query_store = PostgresDataStore::connect(&database_url, 4)
        .await
        .expect("connect Phase 6H query store");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect Phase 6H evidence reader");
    let clock: Arc<dyn Clock> = Arc::new(FixedClock::new(NOW));

    let authorization_store = LiveAuthorizationStore::default();
    for definition in capability_definitions().expect("valid mutation definitions") {
        authorization_store
            .upsert(authorization_grant(&definition, TENANT, ACTOR))
            .expect("valid mutation authorization grant");
    }
    let query_definitions = query_capability_definitions().expect("valid query definitions");
    for definition in &query_definitions {
        authorization_store
            .upsert(authorization_grant(definition, TENANT, ACTOR))
            .expect("valid query authorization grant");
    }

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

    let deal_ids = [
        "phase6h-query-deal-1",
        "phase6h-query-deal-2",
        "phase6h-query-deal-3",
    ];
    for (index, deal_id) in deal_ids.iter().enumerate() {
        let definition = mutation_definition(SALES_CREATE);
        mutation_gateway
            .execute(mutation_request(
                &definition,
                &format!("deal-create-{index}"),
                u8::try_from(index + 1).unwrap(),
                sales_create(deal_id, &format!("Phase 6H deal {index}")),
            ))
            .await
            .expect("create production Deal through mutation gateway");
    }

    let task_ids = ["phase6h-query-task-1", "phase6h-query-task-2"];
    for (index, task_id) in task_ids.iter().enumerate() {
        let definition = mutation_definition(TASK_CREATE);
        mutation_gateway
            .execute(mutation_request(
                &definition,
                &format!("task-create-{index}"),
                u8::try_from(index + 20).unwrap(),
                task_create(task_id, deal_ids[0]),
            ))
            .await
            .expect("create production Task through mutation gateway");
    }

    let read_baseline = evidence_counts(&admin).await;

    let visibility_store = LiveQueryVisibilityStore::default();
    let mut visibility_grants = Vec::new();
    for definition in &query_definitions {
        let grant = visibility_grant(
            definition,
            TENANT,
            ACTOR,
            None,
            if definition.owner_module_id.as_str() == "crm.sales" {
                sales_fields()
            } else {
                task_fields()
            },
        );
        visibility_store
            .upsert(grant.clone())
            .expect("valid type-wide visibility grant");
        visibility_grants.push(grant);
    }

    let sales_get_definition = query_definition(SALES_GET_CAPABILITY);
    let masked_exact_grant = visibility_grant(
        &sales_get_definition,
        TENANT,
        ACTOR,
        Some(deal_ids[0]),
        BTreeSet::from(["name".to_owned()]),
    );
    visibility_store
        .upsert(masked_exact_grant.clone())
        .expect("valid exact field-mask grant");

    let visibility_authorizer = Arc::new(LiveQueryVisibilityAuthorizer::new(
        visibility_store.clone(),
        Arc::clone(&clock),
    ));
    let query_adapter = Arc::new(
        SalesActivitiesQueryAdapter::new(
            query_store,
            CursorCodec::new([0x6a; 32]).expect("valid cursor signing key"),
            visibility_authorizer,
        )
        .expect("valid production query adapter"),
    );
    let query_gateway = QueryGateway::new(
        Arc::new(query_capability_catalog().expect("valid query catalog")),
        query_adapter.clone(),
        Arc::new(LiveCapabilityAuthorizer::new(
            authorization_store.clone(),
            Arc::clone(&clock),
        )),
        query_adapter,
    );

    let masked = execute_query::<_, sales::GetDealResponse>(
        &query_gateway,
        &sales_get_definition,
        TENANT,
        ACTOR,
        "get-masked-deal",
        60,
        sales::GetDealRequest {
            deal_id: deal_ids[0].to_owned(),
        },
    )
    .await;
    let masked_deal = masked.deal.expect("masked Deal response");
    assert_eq!(masked_deal.name, "Phase 6H deal 0");
    assert!(masked_deal.amount.is_none());
    assert!(masked_deal.owner.is_none());
    assert!(masked_deal.stage_details.is_none());

    visibility_store
        .revoke(&masked_exact_grant)
        .expect("revoke exact field-mask grant");
    let full = execute_query::<_, sales::GetDealResponse>(
        &query_gateway,
        &sales_get_definition,
        TENANT,
        ACTOR,
        "get-full-deal",
        61,
        sales::GetDealRequest {
            deal_id: deal_ids[0].to_owned(),
        },
    )
    .await;
    let full_deal = full.deal.expect("full Deal response");
    assert!(full_deal.amount.is_some());
    assert!(full_deal.owner.is_some());
    assert!(full_deal.stage_details.is_some());

    authorization_store
        .upsert(authorization_grant(
            &sales_get_definition,
            TENANT,
            NO_VISIBILITY_ACTOR,
        ))
        .expect("grant capability without resource visibility");
    let hidden = query_gateway
        .execute(query_request(
            &sales_get_definition,
            TENANT,
            NO_VISIBILITY_ACTOR,
            "get-hidden-deal",
            62,
            sales::GetDealRequest {
                deal_id: deal_ids[0].to_owned(),
            },
        ))
        .await
        .expect_err("resource without visibility must be non-disclosing");
    assert_execution_code(hidden, "QUERY_RESOURCE_NOT_FOUND");

    authorization_store
        .upsert(authorization_grant(
            &sales_get_definition,
            OTHER_TENANT,
            ACTOR,
        ))
        .expect("grant cross-tenant query capability");
    visibility_store
        .upsert(visibility_grant(
            &sales_get_definition,
            OTHER_TENANT,
            ACTOR,
            None,
            sales_fields(),
        ))
        .expect("grant cross-tenant resource visibility");
    let cross_tenant = query_gateway
        .execute(query_request(
            &sales_get_definition,
            OTHER_TENANT,
            ACTOR,
            "get-cross-tenant-deal",
            63,
            sales::GetDealRequest {
                deal_id: deal_ids[0].to_owned(),
            },
        ))
        .await
        .expect_err("FORCE RLS must hide another tenant's Deal");
    assert_execution_code(cross_tenant, "QUERY_RESOURCE_NOT_FOUND");

    let sales_list_definition = query_definition(SALES_LIST_CAPABILITY);
    let first_page = execute_query::<_, sales::ListDealsResponse>(
        &query_gateway,
        &sales_list_definition,
        TENANT,
        ACTOR,
        "list-deals-page-1",
        70,
        list_deals(2, ""),
    )
    .await;
    assert_eq!(first_page.deals.len(), 2);
    let page_token = first_page
        .page
        .expect("first Deal page info")
        .next_page_token;
    assert!(!page_token.is_empty());
    let first_ids = first_page
        .deals
        .iter()
        .map(|deal| deal.deal_id.clone())
        .collect::<BTreeSet<_>>();

    let changed_size = query_gateway
        .execute(query_request(
            &sales_list_definition,
            TENANT,
            ACTOR,
            "list-deals-binding-mismatch",
            71,
            list_deals(1, &page_token),
        ))
        .await
        .expect_err("cursor must be bound to effective page size");
    assert_semantic_code(changed_size, "QUERY_CURSOR_BINDING_MISMATCH");

    let tampered = query_gateway
        .execute(query_request(
            &sales_list_definition,
            TENANT,
            ACTOR,
            "list-deals-tampered-cursor",
            72,
            list_deals(2, &tamper_token(&page_token)),
        ))
        .await
        .expect_err("tampered cursor must be rejected");
    assert_cursor_rejected(tampered);

    let list_auth_grant = authorization_grant(&sales_list_definition, TENANT, ACTOR);
    assert!(
        authorization_store
            .revoke(
                &list_auth_grant.tenant_id,
                &list_auth_grant.actor_id,
                &list_auth_grant.policy_id,
            )
            .expect("revoke live query capability grant")
    );
    let revoked_page = query_gateway
        .execute(query_request(
            &sales_list_definition,
            TENANT,
            ACTOR,
            "list-deals-revoked-page-2",
            73,
            list_deals(2, &page_token),
        ))
        .await
        .expect_err("live revocation must deny the next page");
    assert!(matches!(revoked_page, QueryGatewayError::PermissionDenied { .. }));
    authorization_store
        .upsert(list_auth_grant)
        .expect("restore live query capability grant");

    let second_page = execute_query::<_, sales::ListDealsResponse>(
        &query_gateway,
        &sales_list_definition,
        TENANT,
        ACTOR,
        "list-deals-page-2",
        74,
        list_deals(2, &page_token),
    )
    .await;
    assert_eq!(second_page.deals.len(), 1);
    assert!(
        second_page
            .page
            .expect("second Deal page info")
            .next_page_token
            .is_empty()
    );
    let second_ids = second_page
        .deals
        .iter()
        .map(|deal| deal.deal_id.clone())
        .collect::<BTreeSet<_>>();
    assert!(first_ids.is_disjoint(&second_ids));
    let all_ids = first_ids.union(&second_ids).cloned().collect::<BTreeSet<_>>();
    assert_eq!(all_ids, deal_ids.into_iter().map(str::to_owned).collect());

    let activities_get_definition = query_definition(ACTIVITIES_GET_CAPABILITY);
    let task = execute_query::<_, activities::GetTaskResponse>(
        &query_gateway,
        &activities_get_definition,
        TENANT,
        ACTOR,
        "get-task",
        80,
        activities::GetTaskRequest {
            task_id: task_ids[0].to_owned(),
        },
    )
    .await
    .task
    .expect("Task response");
    assert_eq!(task.subject, "Phase 6H query task");
    assert_eq!(task.related_resources.len(), 1);

    let activities_list_definition = query_definition(ACTIVITIES_LIST_CAPABILITY);
    let first_task_page = execute_query::<_, activities::ListTasksResponse>(
        &query_gateway,
        &activities_list_definition,
        TENANT,
        ACTOR,
        "list-tasks-page-1",
        81,
        list_tasks(1, "", deal_ids[0]),
    )
    .await;
    assert_eq!(first_task_page.tasks.len(), 1);
    let task_cursor = first_task_page
        .page
        .expect("first Task page info")
        .next_page_token;
    assert!(!task_cursor.is_empty());
    let second_task_page = execute_query::<_, activities::ListTasksResponse>(
        &query_gateway,
        &activities_list_definition,
        TENANT,
        ACTOR,
        "list-tasks-page-2",
        82,
        list_tasks(1, &task_cursor, deal_ids[0]),
    )
    .await;
    assert_eq!(second_task_page.tasks.len(), 1);
    assert!(
        second_task_page
            .page
            .expect("second Task page info")
            .next_page_token
            .is_empty()
    );

    assert_eq!(evidence_counts(&admin).await, read_baseline);

    let _ = visibility_grants;
}

fn authorization_grant(
    definition: &CapabilityDefinition,
    tenant: &str,
    actor: &str,
) -> AuthorizationGrant {
    AuthorizationGrant {
        tenant_id: TenantId::try_new(tenant).unwrap(),
        actor_id: ActorId::try_new(actor).unwrap(),
        policy_id: definition.authorization_policy_id.clone(),
        capability_id: definition.capability_id.clone(),
        capability_version: definition.capability_version.clone(),
        owner_module_id: definition.owner_module_id.clone(),
        policy_version: "phase6h-query-policy-1".to_owned(),
        expires_at_unix_nanos: Some(NOW + 10_000_000_000_000),
    }
}

fn visibility_grant(
    definition: &CapabilityDefinition,
    tenant: &str,
    actor: &str,
    record_id: Option<&str>,
    allowed_fields: BTreeSet<String>,
) -> QueryVisibilityGrant {
    QueryVisibilityGrant {
        tenant_id: TenantId::try_new(tenant).unwrap(),
        actor_id: ActorId::try_new(actor).unwrap(),
        capability_id: definition.capability_id.clone(),
        capability_version: definition.capability_version.clone(),
        owner_module_id: definition.owner_module_id.clone(),
        record_type: RecordType::try_new(if definition.owner_module_id.as_str() == "crm.sales" {
            SALES_RECORD_TYPE
        } else {
            ACTIVITIES_RECORD_TYPE
        })
        .unwrap(),
        record_id: record_id.map(|value| RecordId::try_new(value).unwrap()),
        allowed_fields,
        policy_version: "phase6h-visibility-1".to_owned(),
        expires_at_unix_nanos: Some(NOW + 10_000_000_000_000),
    }
}

fn mutation_definition(capability_id: &str) -> CapabilityDefinition {
    capability_definitions()
        .expect("valid mutation definitions")
        .into_iter()
        .find(|definition| definition.capability_id.as_str() == capability_id)
        .unwrap_or_else(|| panic!("missing mutation capability definition: {capability_id}"))
}

fn query_definition(capability_id: &str) -> CapabilityDefinition {
    query_capability_definitions()
        .expect("valid query definitions")
        .into_iter()
        .find(|definition| definition.capability_id.as_str() == capability_id)
        .unwrap_or_else(|| panic!("missing query capability definition: {capability_id}"))
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
                request_id: RequestId::try_new(format!("phase6h-query-{identity}")).unwrap(),
                correlation_id: CorrelationId::try_new(format!("phase6h-query-corr-{identity}"))
                    .unwrap(),
                causation_id: CausationId::try_new(format!("phase6h-query-cause-{identity}"))
                    .unwrap(),
                trace_id: TraceId::try_new(format!("phase6h-query-trace-{identity}")).unwrap(),
                capability_id: definition.capability_id.clone(),
                capability_version: definition.capability_version.clone(),
                idempotency_key: IdempotencyKey::try_new(format!("phase6h-query-idem-{identity}"))
                    .unwrap(),
                business_transaction_id: BusinessTransactionId::try_new(format!(
                    "phase6h-query-tx-{identity}"
                ))
                .unwrap(),
                schema_version: definition.input_contract.schema_version.clone(),
                request_started_at_unix_nanos: NOW + i64::from(hash_byte),
            },
        },
        input: payload(definition, message),
        input_hash: [hash_byte; 32],
        approval: None,
    }
}

fn query_request<M: Message>(
    definition: &CapabilityDefinition,
    tenant: &str,
    actor: &str,
    identity: &str,
    hash_byte: u8,
    message: M,
) -> QueryRequest {
    QueryRequest {
        owner_module_id: definition.owner_module_id.clone(),
        context: QueryExecutionContext {
            tenant_id: TenantId::try_new(tenant).unwrap(),
            actor_id: ActorId::try_new(actor).unwrap(),
            request_id: RequestId::try_new(format!("phase6h-{identity}")).unwrap(),
            correlation_id: CorrelationId::try_new(format!("phase6h-corr-{identity}")).unwrap(),
            trace_id: TraceId::try_new(format!("phase6h-trace-{identity}")).unwrap(),
            capability_id: definition.capability_id.clone(),
            capability_version: definition.capability_version.clone(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
            request_started_at_unix_nanos: NOW + i64::from(hash_byte),
        },
        input: payload(definition, message),
        input_hash: [hash_byte; 32],
    }
}

async fn execute_query<M, R>(
    gateway: &QueryGateway,
    definition: &CapabilityDefinition,
    tenant: &str,
    actor: &str,
    identity: &str,
    hash_byte: u8,
    message: M,
) -> R
where
    M: Message,
    R: Message + Default,
{
    let result = gateway
        .execute(query_request(
            definition,
            tenant,
            actor,
            identity,
            hash_byte,
            message,
        ))
        .await
        .unwrap_or_else(|error| panic!("query {identity} failed: {error}"));
    R::decode(result.output.bytes.as_slice()).expect("decode governed query response")
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
    payload.validate().expect("valid production query payload");
    payload
}

fn sales_create(deal_id: &str, name: &str) -> sales::CreateDealRequest {
    sales::CreateDealRequest {
        deal_id: deal_id.to_owned(),
        name: name.to_owned(),
        owner: Some(actor_owner()),
        account: None,
        primary_contact: None,
        stage: Some(sales::DealStage {
            pipeline_id: QUERY_PIPELINE.to_owned(),
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

fn task_create(task_id: &str, deal_id: &str) -> activities::CreateTaskRequest {
    activities::CreateTaskRequest {
        task_id: task_id.to_owned(),
        subject: "Phase 6H query task".to_owned(),
        description: Some("Read-path PostgreSQL acceptance".to_owned()),
        owner: Some(actor_owner()),
        related_resources: vec![core::ResourceRef {
            tenant_id: TENANT.to_owned(),
            resource_type: SALES_RECORD_TYPE.to_owned(),
            resource_id: deal_id.to_owned(),
            version: Some(1),
        }],
        priority: activities::TaskPriority::High as i32,
        due_at: Some(core::UnixTime {
            unix_nanos: NOW + 100_000_000_000,
        }),
        reminder_at: None,
    }
}

fn list_deals(page_size: i32, page_token: &str) -> sales::ListDealsRequest {
    sales::ListDealsRequest {
        page: Some(core::PageRequest {
            page_size,
            page_token: page_token.to_owned(),
        }),
        owner: None,
        pipeline_id: Some(QUERY_PIPELINE.to_owned()),
        status: None,
        sort: sales::DealSort::UpdatedAtDescending as i32,
    }
}

fn list_tasks(page_size: i32, page_token: &str, deal_id: &str) -> activities::ListTasksRequest {
    activities::ListTasksRequest {
        page: Some(core::PageRequest {
            page_size,
            page_token: page_token.to_owned(),
        }),
        owner: None,
        status: None,
        related_resource: Some(core::ResourceRef {
            tenant_id: TENANT.to_owned(),
            resource_type: SALES_RECORD_TYPE.to_owned(),
            resource_id: deal_id.to_owned(),
            version: Some(1),
        }),
        sort: activities::TaskSort::UpdatedAtDescending as i32,
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
        "account",
        "primary_contact",
        "expected_close_date",
        "probability_basis_points",
        "status",
        "close_outcome",
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
        "reminder_at",
        "completed_at",
        "created_at",
        "updated_at",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn assert_execution_code(error: QueryGatewayError, expected: &str) {
    match error {
        QueryGatewayError::Execution(error) => assert_eq!(error.code, expected),
        other => panic!("expected query execution error {expected}, got {other:?}"),
    }
}

fn assert_semantic_code(error: QueryGatewayError, expected: &str) {
    match error {
        QueryGatewayError::SemanticValidation(error) => assert_eq!(error.code, expected),
        other => panic!("expected query semantic error {expected}, got {other:?}"),
    }
}

fn assert_cursor_rejected(error: QueryGatewayError) {
    match error {
        QueryGatewayError::SemanticValidation(error) => assert!(
            matches!(
                error.code.as_str(),
                "QUERY_CURSOR_TAMPERED" | "QUERY_CURSOR_INVALID"
            ),
            "unexpected cursor error code: {}",
            error.code
        ),
        other => panic!("expected cursor semantic error, got {other:?}"),
    }
}

fn tamper_token(token: &str) -> String {
    let mut bytes = token.as_bytes().to_vec();
    let index = bytes.len() / 2;
    bytes[index] = if bytes[index] == b'A' { b'B' } else { b'A' };
    String::from_utf8(bytes).expect("cursor token is URL-safe ASCII")
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
        .expect("read Phase 6H evidence count")
}
