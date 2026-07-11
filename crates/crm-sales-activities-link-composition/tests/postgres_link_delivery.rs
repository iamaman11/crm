#![cfg(feature = "postgres-integration")]

use crm_capability_adapters::{
    ApprovalStore, AuthorizationGrant, FixedWindowRateLimiter, GatewayCapabilityClient,
    LiveAuthorizationStore, LiveCapabilityAuthorizer, RateLimitPolicyStore, StoredApprovalVerifier,
};
use crm_capability_ingress::semantic_input_hash;
use crm_capability_runtime::{
    CapabilityDefinition, CapabilityGateway, CapabilityRequest, CapabilitySemanticValidator,
};
use crm_core_data::{
    PostgresDataStore, PostgresEventDeliveryReader, PostgresModuleRuntimeStore,
    PostgresTransactionalAggregateExecutor,
};
use crm_core_events::{
    EventDeliveryDisposition, EventDeliveryLookup, EventDeliveryReader, EventDeliveryRuntime,
};
use crm_module_sdk::testing::FixedClock;
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CausationId, Clock, CorrelationId, DataClass, DeliveryId,
    ErrorCategory, ExecutionContext, IdempotencyKey, ModuleExecutionContext, ModuleId,
    PayloadEncoding, PortFuture, RetentionPolicyId, SdkError, TenantId, TraceId, TypedPayload,
};
use crm_proto_contracts::crm::{core::v1 as core, sales::v1 as sales};
use crm_sales_activities_capability_composition::{
    SalesActivitiesCapabilityPlannerRouter, capability_catalog, capability_definitions,
};
use crm_sales_activities_link::{MODULE_ID, SOURCE_EVENT_TYPE, TARGET_CAPABILITY_ID};
use crm_sales_activities_link_composition::SalesActivitiesLinkEventHandler;
use prost::Message;
use sqlx::{PgPool, Row};
use std::sync::Arc;

const TENANT: &str = "tenant-a";
const OTHER_TENANT: &str = "tenant-b";
const ACTOR: &str = "actor-a";
const NOW: i64 = 1_700_000_600_000_000_000;
const DEAL_ID: &str = "phase6i-link-deal";
const DELIVERY_ID: &str = "phase6i-delivery-1";

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct DeliveryEvidence {
    task_records: i64,
    task_outbox: i64,
    target_idempotency: i64,
    target_transactions: i64,
    target_audits: i64,
    link_receipts: i64,
}

#[tokio::test(flavor = "current_thread")]
async fn production_link_delivery_is_lifecycle_gated_tenant_safe_and_exactly_once() {
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
        .expect("connect Phase 6I admin reader");
    let clock = Arc::new(FixedClock::new(NOW));
    let clock_port: Arc<dyn Clock> = clock.clone();
    let authorization_store = LiveAuthorizationStore::default();
    grant_all_production_capabilities(&authorization_store);
    let gateway = production_gateway(
        store.clone(),
        authorization_store.clone(),
        Arc::clone(&clock_port),
    );

    execute_capability(
        &gateway,
        definition("sales.deal.create"),
        sales_create_payload(),
        "phase6i-sales-create",
    )
    .await;
    execute_capability(
        &gateway,
        definition("sales.deal.advance_stage"),
        sales_advance_payload(),
        "phase6i-sales-advance",
    )
    .await;

    let source_event_id = source_event_id(&admin).await;
    assert_source_event_lineage(&admin, &source_event_id).await;
    install_link_module(&admin, "phase6i-sales-advance-tx").await;

    let module_runtime = Arc::new(PostgresModuleRuntimeStore::new(store.clone()));
    let event_reader = Arc::new(PostgresEventDeliveryReader::new(store));
    let capability_client = Arc::new(GatewayCapabilityClient::new(Arc::clone(&gateway)));
    let handler = Arc::new(SalesActivitiesLinkEventHandler::new(
        capability_client,
        module_runtime.clone(),
        Arc::clone(&clock_port),
        ActorId::try_new(ACTOR).unwrap(),
    ));
    let runtime = EventDeliveryRuntime::new(event_reader.clone(), module_runtime.clone(), handler);
    let lookup = delivery_lookup(TENANT, &source_event_id, DELIVERY_ID);

    let cross_tenant = EventDeliveryReader::load(
        event_reader.as_ref(),
        &delivery_lookup(OTHER_TENANT, &source_event_id, "phase6i-cross-tenant"),
    )
    .await
    .expect("cross-tenant event read must be a safe query");
    assert!(cross_tenant.is_none());

    let target_definition = definition(TARGET_CAPABILITY_ID);
    assert!(
        authorization_store
            .revoke(
                &TenantId::try_new(TENANT).unwrap(),
                &ActorId::try_new(ACTOR).unwrap(),
                &target_definition.authorization_policy_id,
            )
            .unwrap()
    );
    let denied = runtime
        .deliver(lookup.clone())
        .await
        .expect_err("live target authorization revocation must deny link delivery");
    assert_eq!(denied.code, "CAPABILITY_PERMISSION_DENIED");
    assert_eq!(delivery_evidence(&admin).await, DeliveryEvidence::default());

    authorization_store
        .upsert(grant_for(&target_definition))
        .expect("restore Activities create grant");
    let first = runtime
        .deliver(lookup.clone())
        .await
        .expect("active link delivery must succeed");
    assert_eq!(first.disposition, EventDeliveryDisposition::Applied);
    assert!(!first.replayed);
    assert_eq!(first.affected_resources.len(), 1);

    let after_first = delivery_evidence(&admin).await;
    assert_eq!(after_first.task_records, 1);
    assert_eq!(after_first.task_outbox, 1);
    assert_eq!(after_first.target_idempotency, 1);
    assert_eq!(after_first.target_transactions, 1);
    assert_eq!(after_first.target_audits, 1);
    assert_eq!(after_first.link_receipts, 1);

    let duplicate = runtime
        .deliver(lookup.clone())
        .await
        .expect("duplicate delivery must resolve from durable receipt");
    assert_eq!(duplicate.disposition, EventDeliveryDisposition::Applied);
    assert!(duplicate.replayed);
    assert_eq!(delivery_evidence(&admin).await, after_first);

    let link_transaction_id = target_transaction_id(&admin).await;
    set_link_status(&admin, "suspended", &link_transaction_id).await;
    let suspended = runtime
        .deliver(delivery_lookup(
            TENANT,
            &source_event_id,
            "phase6i-delivery-suspended",
        ))
        .await
        .expect("suspended link must be skipped safely");
    assert_eq!(
        suspended.disposition,
        EventDeliveryDisposition::SkippedInactive
    );
    assert_eq!(delivery_evidence(&admin).await, after_first);

    execute_capability(
        &gateway,
        definition("sales.deal.update"),
        sales_update_payload(),
        "phase6i-independent-sales",
    )
    .await;
    execute_capability(
        &gateway,
        definition("activities.task.create"),
        independent_task_payload(),
        "phase6i-independent-task",
    )
    .await;
    assert_eq!(record_count(&admin, "phase6i-independent-task").await, 1);

    delete_link_installation(&admin, &link_transaction_id).await;
    let missing = runtime
        .deliver(delivery_lookup(
            TENANT,
            &source_event_id,
            "phase6i-delivery-missing",
        ))
        .await
        .expect("uninstalled link must be skipped safely");
    assert_eq!(
        missing.disposition,
        EventDeliveryDisposition::SkippedMissing
    );
    assert_eq!(module_state_count(&admin).await, 0);
}

fn production_gateway(
    store: PostgresDataStore,
    authorization_store: LiveAuthorizationStore,
    clock: Arc<dyn Clock>,
) -> Arc<CapabilityGateway> {
    Arc::new(CapabilityGateway::new(
        Arc::new(capability_catalog().expect("valid production capability catalog")),
        Arc::new(SemanticHashValidator),
        Arc::new(FixedWindowRateLimiter::new(
            RateLimitPolicyStore::default(),
            Arc::clone(&clock),
        )),
        Arc::new(StoredApprovalVerifier::new(ApprovalStore::default())),
        Arc::new(LiveCapabilityAuthorizer::new(
            authorization_store,
            Arc::clone(&clock),
        )),
        Arc::new(PostgresTransactionalAggregateExecutor::new(
            store,
            Arc::new(SalesActivitiesCapabilityPlannerRouter),
        )),
        clock,
    ))
}

fn grant_all_production_capabilities(store: &LiveAuthorizationStore) {
    for definition in capability_definitions().expect("valid production capability definitions") {
        store
            .upsert(grant_for(&definition))
            .expect("grant production capability");
    }
}

fn grant_for(definition: &CapabilityDefinition) -> AuthorizationGrant {
    AuthorizationGrant {
        tenant_id: TenantId::try_new(TENANT).unwrap(),
        actor_id: ActorId::try_new(ACTOR).unwrap(),
        policy_id: definition.authorization_policy_id.clone(),
        capability_id: definition.capability_id.clone(),
        capability_version: definition.capability_version.clone(),
        owner_module_id: definition.owner_module_id.clone(),
        policy_version: "phase6i-policy-1".to_owned(),
        expires_at_unix_nanos: Some(NOW + 10_000_000_000_000),
    }
}

async fn execute_capability(
    gateway: &CapabilityGateway,
    definition: CapabilityDefinition,
    input: TypedPayload,
    identity_prefix: &str,
) -> crm_capability_runtime::CapabilityExecutionResult {
    let request = CapabilityRequest {
        context: ModuleExecutionContext {
            module_id: definition.owner_module_id.clone(),
            execution: ExecutionContext {
                tenant_id: TenantId::try_new(TENANT).unwrap(),
                actor_id: ActorId::try_new(ACTOR).unwrap(),
                request_id: crm_module_sdk::RequestId::try_new(format!(
                    "{identity_prefix}-request"
                ))
                .unwrap(),
                correlation_id: CorrelationId::try_new(format!("{identity_prefix}-correlation"))
                    .unwrap(),
                causation_id: CausationId::try_new(format!("{identity_prefix}-causation")).unwrap(),
                trace_id: TraceId::try_new(format!("{identity_prefix}-trace")).unwrap(),
                capability_id: definition.capability_id.clone(),
                capability_version: definition.capability_version.clone(),
                idempotency_key: IdempotencyKey::try_new(format!("{identity_prefix}-idempotency"))
                    .unwrap(),
                business_transaction_id: BusinessTransactionId::try_new(format!(
                    "{identity_prefix}-tx"
                ))
                .unwrap(),
                schema_version: definition.input_contract.schema_version.clone(),
                request_started_at_unix_nanos: NOW,
            },
        },
        input_hash: semantic_input_hash(&input),
        input,
        approval: None,
    };
    gateway
        .execute(request)
        .await
        .unwrap_or_else(|error| panic!("production capability execution failed: {error}"))
}

fn definition(capability_id: &str) -> CapabilityDefinition {
    capability_definitions()
        .expect("valid production definitions")
        .into_iter()
        .find(|definition| definition.capability_id.as_str() == capability_id)
        .unwrap_or_else(|| panic!("missing production capability definition: {capability_id}"))
}

fn protobuf_payload<M: Message>(definition: &CapabilityDefinition, message: M) -> TypedPayload {
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

fn sales_create_payload() -> TypedPayload {
    let definition = definition("sales.deal.create");
    protobuf_payload(
        &definition,
        sales::CreateDealRequest {
            deal_id: DEAL_ID.to_owned(),
            name: "Phase 6I link proof".to_owned(),
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
    )
}

fn sales_advance_payload() -> TypedPayload {
    let definition = definition("sales.deal.advance_stage");
    protobuf_payload(
        &definition,
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
}

fn sales_update_payload() -> TypedPayload {
    let definition = definition("sales.deal.update");
    protobuf_payload(
        &definition,
        sales::UpdateDealRequest {
            deal_id: DEAL_ID.to_owned(),
            expected_version: 2,
            name: Some(core::StringPatch {
                operation: Some(core::string_patch::Operation::Set(
                    "Phase 6I independent Sales update".to_owned(),
                )),
            }),
            owner: None,
            account: None,
            primary_contact: None,
            amount: None,
            expected_close_date: None,
            probability_basis_points: None,
        },
    )
}

fn independent_task_payload() -> TypedPayload {
    let definition = definition("activities.task.create");
    protobuf_payload(
        &definition,
        crm_proto_contracts::crm::activities::v1::CreateTaskRequest {
            task_id: "phase6i-independent-task".to_owned(),
            subject: "Independent Activities proof".to_owned(),
            description: None,
            owner: Some(actor_owner()),
            related_resources: vec![],
            priority: crm_proto_contracts::crm::activities::v1::TaskPriority::Normal as i32,
            due_at: None,
            reminder_at: None,
        },
    )
}

fn actor_owner() -> core::ActorOrTeamOwner {
    core::ActorOrTeamOwner {
        owner: Some(core::actor_or_team_owner::Owner::ActorId(ACTOR.to_owned())),
    }
}

fn delivery_lookup(tenant: &str, event_id: &str, delivery_id: &str) -> EventDeliveryLookup {
    EventDeliveryLookup {
        tenant_id: TenantId::try_new(tenant).unwrap(),
        consumer_module_id: ModuleId::try_new(MODULE_ID).unwrap(),
        event_id: crm_module_sdk::EventId::try_new(event_id).unwrap(),
        delivery_id: DeliveryId::try_new(delivery_id).unwrap(),
    }
}

async fn source_event_id(pool: &PgPool) -> String {
    sqlx::query(
        "SELECT event_id FROM crm.outbox_events WHERE tenant_id = $1 AND aggregate_id = $2 AND event_type = $3",
    )
    .bind(TENANT)
    .bind(DEAL_ID)
    .bind(SOURCE_EVENT_TYPE)
    .fetch_one(pool)
    .await
    .expect("load Sales stage-changed event")
    .try_get("event_id")
    .expect("valid event id")
}

async fn assert_source_event_lineage(pool: &PgPool, event_id: &str) {
    let row = sqlx::query(
        r#"
        SELECT source_module_id, source_actor_id, correlation_id, trace_id, event_version
        FROM crm.outbox_events
        WHERE tenant_id = $1 AND event_id = $2
        "#,
    )
    .bind(TENANT)
    .bind(event_id)
    .fetch_one(pool)
    .await
    .expect("load source event lineage");
    assert_eq!(
        row.try_get::<String, _>("source_module_id").unwrap(),
        "crm.sales"
    );
    assert_eq!(row.try_get::<String, _>("source_actor_id").unwrap(), ACTOR);
    assert_eq!(
        row.try_get::<String, _>("correlation_id").unwrap(),
        "phase6i-sales-advance-correlation"
    );
    assert_eq!(
        row.try_get::<String, _>("trace_id").unwrap(),
        "phase6i-sales-advance-trace"
    );
    assert_eq!(row.try_get::<String, _>("event_version").unwrap(), "1.0.0");
}

async fn install_link_module(pool: &PgPool, source_transaction_id: &str) {
    sqlx::query(
        r#"
        INSERT INTO crm.module_versions (
          module_id, version, canonicalization_profile, manifest_sha256,
          normalized_manifest_json, published_at, publisher_id
        )
        VALUES ($1, '0.1.0', 'crm.cjson/v1', $2, '{}'::jsonb, clock_timestamp(), 'platform')
        ON CONFLICT (module_id, version) DO NOTHING
        "#,
    )
    .bind(MODULE_ID)
    .bind(vec![0x6a_u8; 32])
    .execute(pool)
    .await
    .expect("publish link module version");

    let mut transaction = pool.begin().await.expect("begin link installation");
    bind_write_context(&mut transaction, source_transaction_id).await;
    sqlx::query(
        r#"
        INSERT INTO crm.module_installations (
          tenant_id, install_id, module_id, current_version, status,
          generation, grant_set_digest, last_business_transaction_id
        )
        VALUES ($1, 'phase6i-link-install', $2, '0.1.0', 'active', 1, $3, $4)
        "#,
    )
    .bind(TENANT)
    .bind(MODULE_ID)
    .bind(vec![0x61_u8; 32])
    .bind(source_transaction_id)
    .execute(&mut *transaction)
    .await
    .expect("install active link module");
    transaction
        .commit()
        .await
        .expect("commit link installation");
}

async fn set_link_status(pool: &PgPool, status: &str, transaction_id: &str) {
    let mut transaction = pool.begin().await.expect("begin link status update");
    bind_write_context(&mut transaction, transaction_id).await;
    sqlx::query(
        "UPDATE crm.module_installations SET status = $3, last_business_transaction_id = $4, updated_at = clock_timestamp() WHERE tenant_id = $1 AND module_id = $2",
    )
    .bind(TENANT)
    .bind(MODULE_ID)
    .bind(status)
    .bind(transaction_id)
    .execute(&mut *transaction)
    .await
    .expect("update link status");
    transaction
        .commit()
        .await
        .expect("commit link status update");
}

async fn delete_link_installation(pool: &PgPool, transaction_id: &str) {
    let mut transaction = pool.begin().await.expect("begin link uninstall");
    bind_write_context(&mut transaction, transaction_id).await;
    sqlx::query("DELETE FROM crm.module_installations WHERE tenant_id = $1 AND module_id = $2")
        .bind(TENANT)
        .bind(MODULE_ID)
        .execute(&mut *transaction)
        .await
        .expect("delete link installation");
    transaction.commit().await.expect("commit link uninstall");
}

async fn bind_write_context(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    business_transaction_id: &str,
) {
    sqlx::query(
        r#"
        SELECT
          set_config('app.tenant_id', $1, true),
          set_config('app.actor_id', $2, true),
          set_config('app.request_id', $3, true),
          set_config('app.capability_id', $4, true),
          set_config('app.capability_version', '1.0.0', true),
          set_config('app.business_transaction_id', $5, true)
        "#,
    )
    .bind(TENANT)
    .bind(ACTOR)
    .bind("phase6i-link-lifecycle-request")
    .bind("sales.deal.advance_stage")
    .bind(business_transaction_id)
    .execute(&mut **transaction)
    .await
    .expect("bind lifecycle write context");
}

async fn target_transaction_id(pool: &PgPool) -> String {
    sqlx::query(
        "SELECT business_transaction_id FROM crm.idempotency_records WHERE tenant_id = $1 AND idempotency_scope = $2 AND idempotency_key = $3",
    )
    .bind(TENANT)
    .bind("activities.task.create@1.0.0")
    .bind(DELIVERY_ID)
    .fetch_one(pool)
    .await
    .expect("load target transaction id")
    .try_get("business_transaction_id")
    .expect("valid target transaction id")
}

async fn delivery_evidence(pool: &PgPool) -> DeliveryEvidence {
    DeliveryEvidence {
        task_records: record_count(pool, DELIVERY_ID).await,
        task_outbox: count_with_one(
            pool,
            "SELECT count(*) FROM crm.outbox_events WHERE tenant_id = $1 AND aggregate_id = $2",
            DELIVERY_ID,
        )
        .await,
        target_idempotency: count_with_one(
            pool,
            "SELECT count(*) FROM crm.idempotency_records WHERE tenant_id = $1 AND idempotency_key = $2",
            DELIVERY_ID,
        )
        .await,
        target_transactions: sqlx::query(
            r#"
            SELECT count(*)
            FROM crm.business_transactions AS tx
            JOIN crm.idempotency_records AS idem
              ON idem.tenant_id = tx.tenant_id
             AND idem.business_transaction_id = tx.business_transaction_id
            WHERE idem.tenant_id = $1 AND idem.idempotency_key = $2
            "#,
        )
        .bind(TENANT)
        .bind(DELIVERY_ID)
        .fetch_one(pool)
        .await
        .unwrap()
        .try_get(0)
        .unwrap(),
        target_audits: sqlx::query(
            r#"
            SELECT count(*)
            FROM crm.audit_records AS audit
            JOIN crm.idempotency_records AS idem
              ON idem.tenant_id = audit.tenant_id
             AND idem.business_transaction_id = audit.business_transaction_id
            WHERE idem.tenant_id = $1 AND idem.idempotency_key = $2
            "#,
        )
        .bind(TENANT)
        .bind(DELIVERY_ID)
        .fetch_one(pool)
        .await
        .unwrap()
        .try_get(0)
        .unwrap(),
        link_receipts: module_state_count(pool).await,
    }
}

async fn module_state_count(pool: &PgPool) -> i64 {
    count_with_one(
        pool,
        "SELECT count(*) FROM crm.module_state WHERE tenant_id = $1 AND module_id = $2",
        MODULE_ID,
    )
    .await
}

async fn record_count(pool: &PgPool, record_id: &str) -> i64 {
    count_with_one(
        pool,
        "SELECT count(*) FROM crm.records WHERE tenant_id = $1 AND record_id = $2",
        record_id,
    )
    .await
}

async fn count_with_one(pool: &PgPool, query: &'static str, value: &str) -> i64 {
    sqlx::query(query)
        .bind(TENANT)
        .bind(value)
        .fetch_one(pool)
        .await
        .expect("count Phase 6I evidence")
        .try_get(0)
        .expect("valid Phase 6I evidence count")
}
