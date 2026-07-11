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
use crm_proto_contracts::crm::{core::v1 as core, sales::v1 as sales};
use crm_sales_activities_capability_composition::{
    SalesActivitiesCapabilityPlannerRouter, SalesActivitiesLinkDeliveryOutcome,
    SalesActivitiesLinkEventProcessor, SalesActivitiesLinkEventProcessorConfig, capability_catalog,
    capability_definitions,
};
use crm_sales_activities_link::MODULE_ID as LINK_MODULE_ID;
use prost::Message;
use sqlx::PgPool;
use std::sync::Arc;

const TENANT: &str = "tenant-a";
const OTHER_TENANT: &str = "tenant-b";
const ACTOR: &str = "actor-a";
const DEAL_ID: &str = "phase6i-link-deal";
const NOW: i64 = 1_700_000_700_000_000_000;
const SALES_CREATE: &str = "sales.deal.create";
const SALES_ADVANCE: &str = "sales.deal.advance_stage";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EvidenceCounts {
    records: i64,
    outbox: i64,
    audits: i64,
    idempotency: i64,
    transactions: i64,
}

#[tokio::test(flavor = "current_thread")]
async fn production_link_delivery_is_idempotent_tenant_bound_and_lifecycle_gated() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping Phase 6I PostgreSQL acceptance because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let store = PostgresDataStore::connect(&database_url, 6)
        .await
        .expect("connect Phase 6I runtime store");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect Phase 6I evidence reader");
    provision_link_module(&admin).await;

    let gateway = production_gateway(store.clone());
    let source_client = GatewayCapabilityClient::new(Arc::clone(&gateway));
    create_deal(&source_client).await;
    advance_deal(&source_client, 1, "proposal", 2, "phase6i-sales-advance-1").await;
    let first_event_id = stage_changed_event_id(&admin, 2).await;
    let baseline = evidence_counts(&admin).await;

    let processor = SalesActivitiesLinkEventProcessor::new(
        store.clone(),
        Arc::clone(&gateway),
        SalesActivitiesLinkEventProcessorConfig {
            worker_id: "phase6i-link-worker".to_owned(),
            worker_actor_id: ActorId::try_new(ACTOR).unwrap(),
            lease_duration_nanos: 10_000_000_000,
            retry_delay_nanos: 1_000_000_000,
        },
    )
    .expect("valid Phase 6I processor configuration");

    let first = processor
        .process(
            TenantId::try_new(TENANT).unwrap(),
            crm_module_sdk::EventId::try_new(first_event_id.clone()).unwrap(),
            NOW + 100,
        )
        .await
        .expect("first link delivery must succeed");
    let task_id = match first {
        SalesActivitiesLinkDeliveryOutcome::Applied { affected_resources } => {
            assert_eq!(affected_resources.len(), 1);
            assert_eq!(affected_resources[0].resource_type, "activities.task");
            affected_resources[0].resource_id.clone()
        }
        other => panic!("expected applied link delivery, got {other:?}"),
    };
    assert_task_exists(&admin, &task_id).await;
    let after_first = evidence_counts(&admin).await;
    assert_eq!(after_first.records, baseline.records + 1);
    assert_eq!(after_first.outbox, baseline.outbox + 1);
    assert_eq!(after_first.audits, baseline.audits + 1);
    assert_eq!(after_first.idempotency, baseline.idempotency + 1);
    assert_eq!(after_first.transactions, baseline.transactions + 1);

    let duplicate = processor
        .process(
            TenantId::try_new(TENANT).unwrap(),
            crm_module_sdk::EventId::try_new(first_event_id.clone()).unwrap(),
            NOW + 200,
        )
        .await
        .expect("completed duplicate must be safe");
    assert_eq!(duplicate, SalesActivitiesLinkDeliveryOutcome::NotReady);
    assert_eq!(evidence_counts(&admin).await, after_first);

    delete_rebuildable_delivery_ledger(&admin, &first_event_id).await;
    let rebuilt = processor
        .process(
            TenantId::try_new(TENANT).unwrap(),
            crm_module_sdk::EventId::try_new(first_event_id.clone()).unwrap(),
            NOW + 300,
        )
        .await
        .expect("recreated delivery ledger must replay target idempotently");
    match rebuilt {
        SalesActivitiesLinkDeliveryOutcome::Applied { affected_resources } => {
            assert_eq!(affected_resources.len(), 1);
            assert_eq!(affected_resources[0].resource_id, task_id);
        }
        other => panic!("expected idempotent applied replay, got {other:?}"),
    }
    assert_eq!(evidence_counts(&admin).await, after_first);

    set_link_installation_status(&admin, "suspended").await;
    advance_deal(
        &source_client,
        2,
        "negotiation",
        3,
        "phase6i-sales-advance-2",
    )
    .await;
    let second_event_id = stage_changed_event_id(&admin, 3).await;
    let after_independent_sales = evidence_counts(&admin).await;
    let suspended = processor
        .process(
            TenantId::try_new(TENANT).unwrap(),
            crm_module_sdk::EventId::try_new(second_event_id.clone()).unwrap(),
            NOW + 400,
        )
        .await
        .expect("suspended link must be a safe no-op");
    assert_eq!(
        suspended,
        SalesActivitiesLinkDeliveryOutcome::InactiveConsumer
    );
    assert_eq!(evidence_counts(&admin).await, after_independent_sales);

    remove_link_installation(&admin).await;
    let missing_installation = processor
        .process(
            TenantId::try_new(TENANT).unwrap(),
            crm_module_sdk::EventId::try_new(second_event_id.clone()).unwrap(),
            NOW + 500,
        )
        .await
        .expect("missing link installation must remain a safe no-op");
    assert_eq!(
        missing_installation,
        SalesActivitiesLinkDeliveryOutcome::InactiveConsumer
    );
    assert_eq!(evidence_counts(&admin).await, after_independent_sales);

    let foreign_tenant = processor
        .process(
            TenantId::try_new(OTHER_TENANT).unwrap(),
            crm_module_sdk::EventId::try_new(first_event_id).unwrap(),
            NOW + 600,
        )
        .await
        .expect("cross-tenant source event lookup must remain non-disclosing");
    assert_eq!(
        foreign_tenant,
        SalesActivitiesLinkDeliveryOutcome::MissingSourceEvent
    );
    assert_eq!(evidence_counts(&admin).await, after_independent_sales);
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
                policy_version: "phase6i-policy-1".to_owned(),
                expires_at_unix_nanos: Some(NOW + 10_000_000_000_000),
            })
            .expect("valid Phase 6I authorization grant");
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
                decision_id: "phase6i-rate-allow".to_owned(),
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

async fn create_deal(client: &GatewayCapabilityClient) {
    let definition = definition(SALES_CREATE);
    client
        .invoke(
            &caller_context("phase6i-sales-create"),
            CapabilityInvocation {
                capability_id: definition.capability_id.clone(),
                capability_version: definition.capability_version.clone(),
                input: payload(
                    &definition,
                    sales::CreateDealRequest {
                        deal_id: DEAL_ID.to_owned(),
                        name: "Phase 6I linked deal".to_owned(),
                        owner: Some(actor_owner()),
                        account: None,
                        primary_contact: None,
                        stage: Some(sales::DealStage {
                            pipeline_id: "pipeline.enterprise".to_owned(),
                            stage_id: "qualification".to_owned(),
                            ordinal: 1,
                        }),
                        amount: Some(core::ExactMoney {
                            minor_units: "100000".to_owned(),
                            currency_code: "USD".to_owned(),
                        }),
                        expected_close_date: Some(core::CalendarDate {
                            year: 2027,
                            month: 12,
                            day: 31,
                        }),
                        probability_basis_points: 2_500,
                    },
                ),
            },
        )
        .await
        .expect("create Phase 6I source deal through production gateway");
}

async fn advance_deal(
    client: &GatewayCapabilityClient,
    expected_version: i64,
    stage_id: &str,
    ordinal: u32,
    identity: &str,
) {
    let definition = definition(SALES_ADVANCE);
    client
        .invoke(
            &caller_context(identity),
            CapabilityInvocation {
                capability_id: definition.capability_id.clone(),
                capability_version: definition.capability_version.clone(),
                input: payload(
                    &definition,
                    sales::AdvanceStageRequest {
                        deal_id: DEAL_ID.to_owned(),
                        expected_version,
                        target_stage: Some(sales::DealStage {
                            pipeline_id: "pipeline.enterprise".to_owned(),
                            stage_id: stage_id.to_owned(),
                            ordinal,
                        }),
                        target_status: sales::DealStatus::Open as i32,
                        close_reason_code: None,
                        policy: Some(sales::StageTransitionPolicy {
                            allow_regression: false,
                            allow_skip: false,
                        }),
                    },
                ),
            },
        )
        .await
        .expect("advance Phase 6I source deal through production gateway");
}

fn caller_context(identity: &str) -> ModuleExecutionContext {
    ModuleExecutionContext {
        module_id: ModuleId::try_new("crm.phase6i-acceptance").unwrap(),
        execution: ExecutionContext {
            tenant_id: TenantId::try_new(TENANT).unwrap(),
            actor_id: ActorId::try_new(ACTOR).unwrap(),
            request_id: RequestId::try_new(format!("request-{identity}")).unwrap(),
            correlation_id: CorrelationId::try_new(format!("correlation-{identity}")).unwrap(),
            causation_id: CausationId::try_new(format!("causation-{identity}")).unwrap(),
            trace_id: TraceId::try_new(format!("trace-{identity}")).unwrap(),
            capability_id: CapabilityId::try_new("phase6i.acceptance.invoke").unwrap(),
            capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
            idempotency_key: IdempotencyKey::try_new(format!("idem-{identity}")).unwrap(),
            business_transaction_id: BusinessTransactionId::try_new(format!("tx-{identity}"))
                .unwrap(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
            request_started_at_unix_nanos: NOW,
        },
    }
}

fn definition(capability_id: &str) -> crm_capability_runtime::CapabilityDefinition {
    capability_definitions()
        .expect("valid production definitions")
        .into_iter()
        .find(|definition| definition.capability_id.as_str() == capability_id)
        .unwrap_or_else(|| panic!("missing production capability definition: {capability_id}"))
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

async fn stage_changed_event_id(admin: &PgPool, aggregate_version: i64) -> String {
    sqlx::query_scalar::<_, String>(
        r#"
        SELECT event_id
        FROM crm.outbox_events
        WHERE tenant_id = $1
          AND aggregate_type = 'sales.deal'
          AND aggregate_id = $2
          AND aggregate_version = $3
          AND event_type = 'sales.deal.stage_changed'
        ORDER BY occurred_at DESC
        LIMIT 1
        "#,
    )
    .bind(TENANT)
    .bind(DEAL_ID)
    .bind(aggregate_version)
    .fetch_one(admin)
    .await
    .expect("read Sales stage-changed source event")
}

async fn assert_task_exists(admin: &PgPool, task_id: &str) {
    let count: i64 = sqlx::query_scalar(
        r#"
        SELECT count(*)
        FROM crm.records
        WHERE tenant_id = $1
          AND record_type = 'activities.task'
          AND record_id = $2
          AND deleted_at IS NULL
        "#,
    )
    .bind(TENANT)
    .bind(task_id)
    .fetch_one(admin)
    .await
    .expect("count linked task");
    assert_eq!(count, 1);
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
        .expect("count Phase 6I evidence")
}

async fn provision_link_module(admin: &PgPool) {
    sqlx::query(
        r#"
        INSERT INTO crm.module_versions (
          module_id, version, canonicalization_profile, manifest_sha256,
          normalized_manifest_json, published_at, publisher_id
        )
        VALUES ($1, '1.0.0', 'crm.cjson/v1', $2, '{}'::jsonb, clock_timestamp(), 'phase6i-test')
        ON CONFLICT (module_id, version) DO NOTHING
        "#,
    )
    .bind(LINK_MODULE_ID)
    .bind(vec![0x69_u8; 32])
    .execute(admin)
    .await
    .expect("provision link module version");
    set_link_installation_status(admin, "active").await;
}

async fn set_link_installation_status(admin: &PgPool, status: &str) {
    let mut transaction = admin
        .begin()
        .await
        .expect("begin link installation fixture");
    bind_fixture_context(&mut transaction).await;
    sqlx::query(
        r#"
        INSERT INTO crm.module_installations (
          tenant_id, install_id, module_id, current_version, status,
          generation, grant_set_digest, last_business_transaction_id
        )
        VALUES ($1, 'phase6i-link-installation', $2, '1.0.0', $3, 1, $4, 'tx-bootstrap-a')
        ON CONFLICT (tenant_id, module_id)
        DO UPDATE SET
          status = EXCLUDED.status,
          generation = crm.module_installations.generation + 1,
          last_business_transaction_id = EXCLUDED.last_business_transaction_id,
          updated_at = clock_timestamp()
        "#,
    )
    .bind(TENANT)
    .bind(LINK_MODULE_ID)
    .bind(status)
    .bind(vec![0x6a_u8; 32])
    .execute(&mut *transaction)
    .await
    .expect("upsert link installation");
    transaction
        .commit()
        .await
        .expect("commit link installation fixture");
}

async fn remove_link_installation(admin: &PgPool) {
    let mut transaction = admin.begin().await.expect("begin link removal fixture");
    bind_fixture_context(&mut transaction).await;
    sqlx::query("DELETE FROM crm.module_installations WHERE tenant_id = $1 AND module_id = $2")
        .bind(TENANT)
        .bind(LINK_MODULE_ID)
        .execute(&mut *transaction)
        .await
        .expect("remove link installation");
    transaction
        .commit()
        .await
        .expect("commit link removal fixture");
}

async fn bind_fixture_context(transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>) {
    sqlx::query(
        r#"
        SELECT
          set_config('app.tenant_id', $1, true),
          set_config('app.actor_id', $2, true),
          set_config('app.request_id', 'phase6i-link-fixture', true),
          set_config('app.capability_id', 'test.record.mutate', true),
          set_config('app.capability_version', '1.0.0', true),
          set_config('app.business_transaction_id', 'tx-bootstrap-a', true)
        "#,
    )
    .bind(TENANT)
    .bind(ACTOR)
    .execute(&mut **transaction)
    .await
    .expect("bind link fixture context");
}

async fn delete_rebuildable_delivery_ledger(admin: &PgPool, event_id: &str) {
    sqlx::query(
        "DELETE FROM crm.event_deliveries WHERE tenant_id = $1 AND consumer_module_id = $2 AND event_id = $3",
    )
    .bind(TENANT)
    .bind(LINK_MODULE_ID)
    .bind(event_id)
    .execute(admin)
    .await
    .expect("delete rebuildable delivery ledger state");
}
