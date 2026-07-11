#![cfg(feature = "postgres-integration")]

use crm_activities_capability_adapter::{
    ActivitiesTaskCapabilityPlanner, COMPLETE_CAPABILITY, CREATE_CAPABILITY, MODULE_ID,
    capability_definition,
};
use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityRequest, TransactionalCapabilityExecutor};
use crm_core_data::{PostgresDataStore, PostgresTransactionalAggregateExecutor};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CausationId, CorrelationId, DataClass, ExecutionContext,
    IdempotencyKey, ModuleExecutionContext, ModuleId, PayloadEncoding, RequestId,
    RetentionPolicyId, SchemaId, SchemaVersion, TenantId, TraceId, TypedPayload,
};
use crm_proto_contracts::crm::{activities::v1 as wire, core::v1 as core};
use prost::Message;
use sqlx::{PgPool, Row};
use std::sync::Arc;

const TENANT: &str = "tenant-a";
const ACTOR: &str = "actor-a";

#[tokio::test(flavor = "current_thread")]
async fn completed_task_noop_commits_audit_and_idempotency_without_outbox() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping Activities PostgreSQL acceptance because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let store = PostgresDataStore::connect(&database_url, 4)
        .await
        .expect("connect Activities runtime store");
    let executor = PostgresTransactionalAggregateExecutor::new(
        store.clone(),
        Arc::new(ActivitiesTaskCapabilityPlanner),
    );

    let create_definition = capability_definition(CREATE_CAPABILITY).unwrap();
    let create = request(
        &create_definition,
        "tx-activities-create",
        "idem-activities-create",
        1_700_000_000_000_001_000,
        wire::CreateTaskRequest {
            task_id: "task-postgres-noop".to_owned(),
            subject: "Prove audited no-op".to_owned(),
            description: None,
            owner: Some(core::ActorOrTeamOwner {
                owner: Some(core::actor_or_team_owner::Owner::ActorId(ACTOR.to_owned())),
            }),
            related_resources: Vec::new(),
            priority: wire::TaskPriority::Normal as i32,
            due_at: None,
            reminder_at: None,
        },
        "crm.activities.v1.CreateTaskRequest",
        [0x41; 32],
    );
    let created = executor.execute(&create_definition, create).await.unwrap();
    assert!(!created.replayed);
    assert_eq!(created.affected_resources[0].version, Some(1));

    let complete_definition = capability_definition(COMPLETE_CAPABILITY).unwrap();
    let complete = request(
        &complete_definition,
        "tx-activities-complete",
        "idem-activities-complete",
        1_700_000_000_000_002_000,
        wire::CompleteTaskRequest {
            task_id: "task-postgres-noop".to_owned(),
            expected_version: 1,
        },
        "crm.activities.v1.CompleteTaskRequest",
        [0x42; 32],
    );
    let completed = executor
        .execute(&complete_definition, complete)
        .await
        .unwrap();
    assert_eq!(completed.affected_resources[0].version, Some(2));
    let completed_output =
        wire::CompleteTaskResponse::decode(completed.output.as_ref().unwrap().bytes.as_slice())
            .unwrap();
    assert!(completed_output.changed);

    let noop_request = request(
        &complete_definition,
        "tx-activities-complete-noop",
        "idem-activities-complete-noop",
        1_700_000_000_000_003_000,
        wire::CompleteTaskRequest {
            task_id: "task-postgres-noop".to_owned(),
            expected_version: 2,
        },
        "crm.activities.v1.CompleteTaskRequest",
        [0x43; 32],
    );
    let noop = executor
        .execute(&complete_definition, noop_request.clone())
        .await
        .unwrap();
    assert!(!noop.replayed);
    assert!(noop.affected_resources.is_empty());
    let noop_output =
        wire::CompleteTaskResponse::decode(noop.output.as_ref().unwrap().bytes.as_slice()).unwrap();
    assert!(!noop_output.changed);
    assert_eq!(noop_output.task.unwrap().version, 2);

    let replay = executor
        .execute(&complete_definition, noop_request)
        .await
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.output, noop.output);
    assert!(replay.affected_resources.is_empty());

    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect PostgreSQL admin evidence reader");
    assert_eq!(
        count(
            &admin,
            "crm.records",
            "last_business_transaction_id",
            "tx-activities-complete-noop"
        )
        .await,
        0
    );
    assert_eq!(
        count(
            &admin,
            "crm.outbox_events",
            "business_transaction_id",
            "tx-activities-complete-noop"
        )
        .await,
        0
    );
    assert_eq!(
        count(
            &admin,
            "crm.audit_records",
            "business_transaction_id",
            "tx-activities-complete-noop"
        )
        .await,
        1
    );
    assert_eq!(
        count(
            &admin,
            "crm.idempotency_records",
            "business_transaction_id",
            "tx-activities-complete-noop"
        )
        .await,
        1
    );
    assert_eq!(
        count(
            &admin,
            "crm.business_transactions",
            "business_transaction_id",
            "tx-activities-complete-noop"
        )
        .await,
        1
    );

    let row = sqlx::query(
        "SELECT version FROM crm.records WHERE tenant_id = $1 AND record_type = 'activities.task' AND record_id = 'task-postgres-noop'",
    )
    .bind(TENANT)
    .fetch_one(&admin)
    .await
    .unwrap();
    assert_eq!(row.try_get::<i64, _>("version").unwrap(), 2);
}

async fn count(pool: &PgPool, table: &str, column: &str, transaction_id: &str) -> i64 {
    let query =
        format!("SELECT count(*) AS count FROM {table} WHERE tenant_id = $1 AND {column} = $2");
    sqlx::query(&query)
        .bind(TENANT)
        .bind(transaction_id)
        .fetch_one(pool)
        .await
        .unwrap()
        .try_get("count")
        .unwrap()
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
                correlation_id: CorrelationId::try_new("correlation-activities-noop").unwrap(),
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
