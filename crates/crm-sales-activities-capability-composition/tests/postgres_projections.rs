#![cfg(feature = "postgres-integration")]

use crm_capability_adapters::{
    AuthorizationGrant, GatewayCapabilityClient, LiveAuthorizationStore, LiveCapabilityAuthorizer,
};
use crm_capability_runtime::testing::{
    FixedApprovalVerifier, FixedRateLimiter, FixedSemanticValidator, call_log,
};
use crm_capability_runtime::{CapabilityGateway, RateLimitDecision};
use crm_core_data::{PostgresDataStore, PostgresTransactionalAggregateExecutor};
use crm_module_sdk::testing::FixedClock;
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityClient, CapabilityId, CapabilityInvocation,
    CapabilityVersion, CausationId, CorrelationId, DataClass, ExecutionContext, IdempotencyKey,
    ModuleExecutionContext, ModuleId, PayloadEncoding, RequestId, RetentionPolicyId, SchemaVersion,
    TenantId, TraceId, TypedPayload,
};
use crm_proto_contracts::crm::{
    activities::v1 as activities, core::v1 as core, sales::v1 as sales,
};
use crm_sales_activities_capability_composition::{
    DEAL_TIMELINE_PROJECTION_ID, DEAL_TIMELINE_RESOURCE_TYPE, Phase6ProjectionWorker,
    SalesActivitiesCapabilityPlannerRouter, TASK_STATUS_PROJECTION_ID, TASK_STATUS_RESOURCE_TYPE,
    capability_catalog, capability_definitions,
};
use prost::Message;
use sqlx::PgPool;
use std::sync::Arc;

const TENANT: &str = "tenant-a";
const OTHER_TENANT: &str = "tenant-b";
const ACTOR: &str = "actor-a";
const DEAL_ID: &str = "phase6j-projection-deal";
const TASK_ID: &str = "phase6j-projection-task";
const NOW: i64 = 1_700_000_800_000_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EvidenceCounts {
    records: i64,
    outbox: i64,
    audits: i64,
    idempotency: i64,
    transactions: i64,
}

#[tokio::test(flavor = "current_thread")]
async fn projections_rebuild_from_immutable_events_without_authoritative_side_effects() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping Phase 6J PostgreSQL acceptance because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let store = PostgresDataStore::connect(&database_url, 6)
        .await
        .expect("connect Phase 6J runtime store");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect Phase 6J evidence reader");
    let gateway = production_gateway(store.clone());
    let client = GatewayCapabilityClient::new(gateway);

    invoke(
        &client,
        "sales.deal.create",
        "phase6j-deal-create",
        sales::CreateDealRequest {
            deal_id: DEAL_ID.to_owned(),
            name: "Phase 6J projected deal".to_owned(),
            owner: Some(actor_owner()),
            account: None,
            primary_contact: None,
            stage: Some(sales::DealStage {
                pipeline_id: "pipeline.enterprise".to_owned(),
                stage_id: "qualification".to_owned(),
                ordinal: 1,
            }),
            amount: Some(core::ExactMoney {
                minor_units: "500000".to_owned(),
                currency_code: "USD".to_owned(),
            }),
            expected_close_date: Some(core::CalendarDate {
                year: 2027,
                month: 12,
                day: 31,
            }),
            probability_basis_points: 2_500,
        },
    )
    .await;
    invoke(
        &client,
        "sales.deal.advance_stage",
        "phase6j-deal-advance",
        sales::AdvanceStageRequest {
            deal_id: DEAL_ID.to_owned(),
            expected_version: 1,
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
        },
    )
    .await;
    invoke(
        &client,
        "activities.task.create",
        "phase6j-task-create",
        activities::CreateTaskRequest {
            task_id: TASK_ID.to_owned(),
            subject: "Phase 6J projected task".to_owned(),
            description: None,
            owner: Some(actor_owner()),
            related_resources: Vec::new(),
            priority: activities::TaskPriority::Normal as i32,
            due_at: None,
            reminder_at: None,
        },
    )
    .await;
    invoke(
        &client,
        "activities.task.complete",
        "phase6j-task-complete",
        activities::CompleteTaskRequest {
            task_id: TASK_ID.to_owned(),
            expected_version: 1,
        },
    )
    .await;

    let authoritative_before = evidence_counts(&admin).await;
    let worker = Phase6ProjectionWorker::new(store.clone());
    let deal_applied = worker
        .rebuild(
            TenantId::try_new(TENANT).unwrap(),
            DEAL_TIMELINE_PROJECTION_ID,
            1,
        )
        .await
        .expect("rebuild Deal timeline projection");
    let task_applied = worker
        .rebuild(
            TenantId::try_new(TENANT).unwrap(),
            TASK_STATUS_PROJECTION_ID,
            1,
        )
        .await
        .expect("rebuild Task status projection");
    assert!(deal_applied >= 2);
    assert!(task_applied >= 2);
    assert_eq!(evidence_counts(&admin).await, authoritative_before);

    let deal_docs = store
        .projection_documents(
            &TenantId::try_new(TENANT).unwrap(),
            DEAL_TIMELINE_PROJECTION_ID,
            DEAL_TIMELINE_RESOURCE_TYPE,
        )
        .await
        .expect("read Deal timeline projection");
    let task_docs = store
        .projection_documents(
            &TenantId::try_new(TENANT).unwrap(),
            TASK_STATUS_PROJECTION_ID,
            TASK_STATUS_RESOURCE_TYPE,
        )
        .await
        .expect("read Task status projection");
    let projected_deal_entries = deal_docs
        .iter()
        .filter(|document| document["deal_id"].as_str() == Some(DEAL_ID))
        .count();
    assert_eq!(projected_deal_entries, 2);
    let task_document = task_docs
        .iter()
        .find(|document| document["task_id"].as_str() == Some(TASK_ID))
        .expect("projected Task status document exists");
    assert_eq!(task_document["status"], "completed");
    assert_eq!(task_document["version"], 2);

    let deal_checkpoint = store
        .projection_checkpoint(
            &TenantId::try_new(TENANT).unwrap(),
            DEAL_TIMELINE_PROJECTION_ID,
        )
        .await
        .expect("read Deal projection checkpoint")
        .expect("Deal projection checkpoint exists");
    let task_checkpoint = store
        .projection_checkpoint(
            &TenantId::try_new(TENANT).unwrap(),
            TASK_STATUS_PROJECTION_ID,
        )
        .await
        .expect("read Task projection checkpoint")
        .expect("Task projection checkpoint exists");
    assert_eq!(deal_checkpoint.applied_event_count, deal_applied);
    assert_eq!(task_checkpoint.applied_event_count, task_applied);

    let deal_idle = worker
        .run_batch(
            TenantId::try_new(TENANT).unwrap(),
            DEAL_TIMELINE_PROJECTION_ID,
            10,
        )
        .await
        .expect("Deal projection resumes from checkpoint");
    let task_idle = worker
        .run_batch(
            TenantId::try_new(TENANT).unwrap(),
            TASK_STATUS_PROJECTION_ID,
            10,
        )
        .await
        .expect("Task projection resumes from checkpoint");
    assert_eq!(deal_idle.events_seen, 0);
    assert_eq!(task_idle.events_seen, 0);
    assert_eq!(evidence_counts(&admin).await, authoritative_before);

    assert!(
        store
            .projection_documents(
                &TenantId::try_new(OTHER_TENANT).unwrap(),
                DEAL_TIMELINE_PROJECTION_ID,
                DEAL_TIMELINE_RESOURCE_TYPE,
            )
            .await
            .expect("cross-tenant Deal projection read is non-disclosing")
            .is_empty()
    );
    assert!(
        store
            .projection_checkpoint(
                &TenantId::try_new(OTHER_TENANT).unwrap(),
                TASK_STATUS_PROJECTION_ID,
            )
            .await
            .expect("cross-tenant checkpoint read is non-disclosing")
            .is_none()
    );

    worker
        .rebuild(
            TenantId::try_new(TENANT).unwrap(),
            DEAL_TIMELINE_PROJECTION_ID,
            1,
        )
        .await
        .expect("rebuild Deal projection from zero again");
    worker
        .rebuild(
            TenantId::try_new(TENANT).unwrap(),
            TASK_STATUS_PROJECTION_ID,
            1,
        )
        .await
        .expect("rebuild Task projection from zero again");
    let rebuilt_deal_docs = store
        .projection_documents(
            &TenantId::try_new(TENANT).unwrap(),
            DEAL_TIMELINE_PROJECTION_ID,
            DEAL_TIMELINE_RESOURCE_TYPE,
        )
        .await
        .expect("read rebuilt Deal projection");
    let rebuilt_task_docs = store
        .projection_documents(
            &TenantId::try_new(TENANT).unwrap(),
            TASK_STATUS_PROJECTION_ID,
            TASK_STATUS_RESOURCE_TYPE,
        )
        .await
        .expect("read rebuilt Task projection");
    assert_eq!(rebuilt_deal_docs, deal_docs);
    assert_eq!(rebuilt_task_docs, task_docs);
    assert_eq!(evidence_counts(&admin).await, authoritative_before);
}

fn production_gateway(store: PostgresDataStore) -> Arc<CapabilityGateway> {
    let clock = Arc::new(FixedClock::new(NOW));
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
                policy_version: "phase6j-policy-1".to_owned(),
                expires_at_unix_nanos: Some(NOW + 10_000_000_000_000),
            })
            .expect("valid Phase 6J authorization grant");
    }
    let calls = call_log();
    Arc::new(CapabilityGateway::new(
        Arc::new(capability_catalog().expect("valid production capability catalog")),
        Arc::new(FixedSemanticValidator {
            error: None,
            calls: calls.clone(),
        }),
        Arc::new(FixedRateLimiter {
            decision: RateLimitDecision {
                allowed: true,
                decision_id: "phase6j-rate-allow".to_owned(),
                retry_after_millis: None,
            },
            error: None,
            calls: calls.clone(),
        }),
        Arc::new(FixedApprovalVerifier {
            error: None,
            calls: calls.clone(),
        }),
        Arc::new(LiveCapabilityAuthorizer::new(
            authorization_store,
            clock.clone(),
        )),
        Arc::new(PostgresTransactionalAggregateExecutor::new(
            store,
            Arc::new(SalesActivitiesCapabilityPlannerRouter),
        )),
        clock,
    ))
}

async fn invoke<M: Message>(
    client: &GatewayCapabilityClient,
    capability_id: &str,
    identity: &str,
    message: M,
) {
    let definition = capability_definitions()
        .expect("valid production definitions")
        .into_iter()
        .find(|definition| definition.capability_id.as_str() == capability_id)
        .unwrap_or_else(|| panic!("missing capability definition: {capability_id}"));
    client
        .invoke(
            &caller_context(identity),
            CapabilityInvocation {
                capability_id: definition.capability_id.clone(),
                capability_version: definition.capability_version.clone(),
                input: payload(&definition, message),
            },
        )
        .await
        .unwrap_or_else(|error| panic!("production capability failed: {error}"));
}

fn caller_context(identity: &str) -> ModuleExecutionContext {
    ModuleExecutionContext {
        module_id: ModuleId::try_new("crm.phase6j-acceptance").unwrap(),
        execution: ExecutionContext {
            tenant_id: TenantId::try_new(TENANT).unwrap(),
            actor_id: ActorId::try_new(ACTOR).unwrap(),
            request_id: RequestId::try_new(format!("request-{identity}")).unwrap(),
            correlation_id: CorrelationId::try_new(format!("correlation-{identity}")).unwrap(),
            causation_id: CausationId::try_new(format!("causation-{identity}")).unwrap(),
            trace_id: TraceId::try_new(format!("trace-{identity}")).unwrap(),
            capability_id: CapabilityId::try_new("phase6j.acceptance.invoke").unwrap(),
            capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
            idempotency_key: IdempotencyKey::try_new(format!("idem-{identity}")).unwrap(),
            business_transaction_id: BusinessTransactionId::try_new(format!("tx-{identity}"))
                .unwrap(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
            request_started_at_unix_nanos: NOW,
        },
    }
}

fn payload<M: Message>(
    definition: &crm_capability_runtime::CapabilityDefinition,
    message: M,
) -> TypedPayload {
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

fn actor_owner() -> core::ActorOrTeamOwner {
    core::ActorOrTeamOwner {
        owner: Some(core::actor_or_team_owner::Owner::ActorId(ACTOR.to_owned())),
    }
}

async fn evidence_counts(admin: &PgPool) -> EvidenceCounts {
    EvidenceCounts {
        records: count(admin, "crm.records").await,
        outbox: count(admin, "crm.outbox_events").await,
        audits: count(admin, "crm.audit_records").await,
        idempotency: count(admin, "crm.idempotency_records").await,
        transactions: count(admin, "crm.business_transactions").await,
    }
}

async fn count(admin: &PgPool, table: &str) -> i64 {
    let sql = match table {
        "crm.records" => "SELECT count(*) FROM crm.records WHERE tenant_id = $1",
        "crm.outbox_events" => "SELECT count(*) FROM crm.outbox_events WHERE tenant_id = $1",
        "crm.audit_records" => "SELECT count(*) FROM crm.audit_records WHERE tenant_id = $1",
        "crm.idempotency_records" => {
            "SELECT count(*) FROM crm.idempotency_records WHERE tenant_id = $1"
        }
        "crm.business_transactions" => {
            "SELECT count(*) FROM crm.business_transactions WHERE tenant_id = $1"
        }
        _ => panic!("unsupported evidence table: {table}"),
    };
    sqlx::query_scalar(sql)
        .bind(TENANT)
        .fetch_one(admin)
        .await
        .expect("count Phase 6J evidence")
}
