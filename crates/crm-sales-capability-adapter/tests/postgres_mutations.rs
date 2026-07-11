#![cfg(feature = "postgres-integration")]

use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityRequest, TransactionalCapabilityExecutor};
use crm_core_data::{PostgresDataStore, PostgresTransactionalAggregateExecutor};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CausationId, CorrelationId, DataClass, ExecutionContext,
    IdempotencyKey, ModuleExecutionContext, ModuleId, PayloadEncoding, RequestId,
    RetentionPolicyId, SchemaId, SchemaVersion, TenantId, TraceId, TypedPayload,
};
use crm_proto_contracts::crm::{core::v1 as core, sales::v1 as wire};
use crm_sales_capability_adapter::{
    ADVANCE_CAPABILITY, CREATE_CAPABILITY, MODULE_ID, SalesDealCapabilityPlanner,
    UPDATE_CAPABILITY, capability_definition,
};
use prost::Message;
use sqlx::{PgPool, Row};
use std::sync::Arc;

const TENANT: &str = "tenant-a";
const ACTOR: &str = "actor-a";
const EXACT_MINOR_UNITS: &str = "125000000000000000000";

#[tokio::test(flavor = "current_thread")]
async fn deal_mutations_persist_versions_events_and_original_replay_output() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping Sales PostgreSQL acceptance because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let store = PostgresDataStore::connect(&database_url, 4)
        .await
        .expect("connect Sales runtime store");
    let executor = PostgresTransactionalAggregateExecutor::new(
        store.clone(),
        Arc::new(SalesDealCapabilityPlanner),
    );

    let create_definition = capability_definition(CREATE_CAPABILITY).unwrap();
    let create_request = request(
        &create_definition,
        "tx-sales-create",
        "idem-sales-create",
        1_700_000_000_000_011_000,
        wire::CreateDealRequest {
            deal_id: "deal-postgres".to_owned(),
            name: "Enterprise renewal".to_owned(),
            owner: Some(core::ActorOrTeamOwner {
                owner: Some(core::actor_or_team_owner::Owner::ActorId(ACTOR.to_owned())),
            }),
            account: None,
            primary_contact: None,
            stage: Some(wire::DealStage {
                pipeline_id: "pipeline.enterprise".to_owned(),
                stage_id: "qualification".to_owned(),
                ordinal: 1,
            }),
            amount: Some(core::ExactMoney {
                minor_units: EXACT_MINOR_UNITS.to_owned(),
                currency_code: "USD".to_owned(),
            }),
            expected_close_date: Some(core::CalendarDate {
                year: 2027,
                month: 12,
                day: 31,
            }),
            probability_basis_points: 2_500,
        },
        "crm.sales.v1.CreateDealRequest",
        [0x51; 32],
    );
    let created = executor
        .execute(&create_definition, create_request.clone())
        .await
        .unwrap();
    assert_eq!(created.affected_resources[0].version, Some(1));
    let created_output =
        wire::CreateDealResponse::decode(created.output.as_ref().unwrap().bytes.as_slice())
            .unwrap();
    let original_deal = created_output.deal.unwrap();
    assert_eq!(original_deal.version, 1);
    assert_eq!(original_deal.amount.unwrap().minor_units, EXACT_MINOR_UNITS);

    let update_definition = capability_definition(UPDATE_CAPABILITY).unwrap();
    let updated = executor
        .execute(
            &update_definition,
            request(
                &update_definition,
                "tx-sales-update",
                "idem-sales-update",
                1_700_000_000_000_012_000,
                wire::UpdateDealRequest {
                    deal_id: "deal-postgres".to_owned(),
                    expected_version: 1,
                    name: Some(core::StringPatch {
                        operation: Some(core::string_patch::Operation::Set(
                            "Enterprise renewal 2027".to_owned(),
                        )),
                    }),
                    owner: None,
                    account: None,
                    primary_contact: None,
                    amount: None,
                    expected_close_date: None,
                    probability_basis_points: Some(core::UInt32Patch {
                        operation: Some(core::u_int32_patch::Operation::Set(4_000)),
                    }),
                },
                "crm.sales.v1.UpdateDealRequest",
                [0x52; 32],
            ),
        )
        .await
        .unwrap();
    assert_eq!(updated.affected_resources[0].version, Some(2));

    let advance_definition = capability_definition(ADVANCE_CAPABILITY).unwrap();
    let advanced = executor
        .execute(
            &advance_definition,
            request(
                &advance_definition,
                "tx-sales-advance",
                "idem-sales-advance",
                1_700_000_000_000_013_000,
                wire::AdvanceStageRequest {
                    deal_id: "deal-postgres".to_owned(),
                    expected_version: 2,
                    target_stage: Some(wire::DealStage {
                        pipeline_id: "pipeline.enterprise".to_owned(),
                        stage_id: "proposal".to_owned(),
                        ordinal: 2,
                    }),
                    target_status: wire::DealStatus::Open as i32,
                    close_reason_code: None,
                    policy: Some(wire::StageTransitionPolicy {
                        allow_regression: false,
                        allow_skip: false,
                    }),
                },
                "crm.sales.v1.AdvanceStageRequest",
                [0x53; 32],
            ),
        )
        .await
        .unwrap();
    assert_eq!(advanced.affected_resources[0].version, Some(3));

    let replay = executor
        .execute(&create_definition, create_request)
        .await
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.affected_resources[0].version, Some(1));
    let replay_output =
        wire::CreateDealResponse::decode(replay.output.as_ref().unwrap().bytes.as_slice()).unwrap();
    let replay_deal = replay_output.deal.unwrap();
    assert_eq!(replay_deal.version, 1);
    assert_eq!(replay_deal.amount.unwrap().minor_units, EXACT_MINOR_UNITS);

    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect PostgreSQL admin evidence reader");
    let row = sqlx::query(
        "SELECT version FROM crm.records WHERE tenant_id = $1 AND record_type = 'sales.deal' AND record_id = 'deal-postgres'",
    )
    .bind(TENANT)
    .fetch_one(&admin)
    .await
    .unwrap();
    assert_eq!(row.try_get::<i64, _>("version").unwrap(), 3);

    let event_count: i64 = sqlx::query(
        "SELECT count(*) AS count FROM crm.outbox_events WHERE tenant_id = $1 AND aggregate_type = 'sales.deal' AND aggregate_id = 'deal-postgres'",
    )
    .bind(TENANT)
    .fetch_one(&admin)
    .await
    .unwrap()
    .try_get("count")
    .unwrap();
    assert_eq!(event_count, 3);
}

fn request<M: Message>(
    definition: &crm_capability_runtime::CapabilityDefinition,
    transaction_id: &str,
    idempotency_key: &str,
    started_at: i64,
    message: M,
    schema_id: &str,
    input_hash: [u8; 32],
) -> CapabilityRequest {
    CapabilityRequest {
        context: ModuleExecutionContext {
            module_id: ModuleId::try_new(MODULE_ID).unwrap(),
            execution: ExecutionContext {
                tenant_id: TenantId::try_new(TENANT).unwrap(),
                actor_id: ActorId::try_new(ACTOR).unwrap(),
                request_id: RequestId::try_new(format!("request-{transaction_id}")).unwrap(),
                correlation_id: CorrelationId::try_new("correlation-sales-postgres").unwrap(),
                causation_id: CausationId::try_new(format!("causation-{transaction_id}")).unwrap(),
                trace_id: TraceId::try_new(format!("trace-{transaction_id}")).unwrap(),
                capability_id: definition.capability_id.clone(),
                capability_version: definition.capability_version.clone(),
                idempotency_key: IdempotencyKey::try_new(idempotency_key).unwrap(),
                business_transaction_id: BusinessTransactionId::try_new(transaction_id).unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                request_started_at_unix_nanos: started_at,
            },
        },
        input: TypedPayload {
            owner: ModuleId::try_new(MODULE_ID).unwrap(),
            schema_id: SchemaId::try_new(schema_id).unwrap(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
            descriptor_hash: support::message_descriptor_hash(schema_id),
            data_class: DataClass::Confidential,
            encoding: PayloadEncoding::Protobuf,
            maximum_size_bytes: support::MAX_PROTOBUF_BYTES,
            retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
            bytes: message.encode_to_vec(),
        },
        input_hash,
        approval: None,
    }
}
