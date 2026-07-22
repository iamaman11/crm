use crm_capability_plan_support as plan_support;
use crm_capability_runtime::CapabilityRequest;
use crm_core_data::{PostgresDataStore, TransactionalAggregatePlanner};
use crm_customer_privacy::MODULE_ID;
use crm_customer_privacy_capability_adapter::{
    CREATE_PRIVACY_CASE_CAPABILITY, CREATE_PRIVACY_CASE_REQUEST_SCHEMA,
    CustomerPrivacyCaseCreateCapabilityPlanner, capability_definition,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, CorrelationId,
    DataClass, ExecutionContext, IdempotencyKey, ModuleExecutionContext, RequestId, SchemaVersion,
    TenantId, TraceId,
};
use crm_proto_contracts::crm::customer_privacy::v1 as wire;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn expose_raw_create_batch_error_for_submit_fixture() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        return;
    };
    let store = PostgresDataStore::connect(&database_url, 4)
        .await
        .expect("connect diagnostic store");
    let request = CapabilityRequest {
        context: ModuleExecutionContext {
            module_id: crm_module_sdk::ModuleId::try_new(MODULE_ID).unwrap(),
            execution: ExecutionContext {
                tenant_id: TenantId::try_new("tenant-a").unwrap(),
                actor_id: ActorId::try_new("privacy-officer").unwrap(),
                request_id: RequestId::try_new("request-submit-proof-create").unwrap(),
                correlation_id: CorrelationId::try_new("correlation-submit-proof-create").unwrap(),
                causation_id: CausationId::try_new("causation-submit-proof-create").unwrap(),
                trace_id: TraceId::try_new("trace-submit-proof-create").unwrap(),
                capability_id: CapabilityId::try_new(CREATE_PRIVACY_CASE_CAPABILITY).unwrap(),
                capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                idempotency_key: IdempotencyKey::try_new("privacy-submit-create-key").unwrap(),
                business_transaction_id: BusinessTransactionId::try_new("privacy-submit-create-tx")
                    .unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                request_started_at_unix_nanos: 1_000_000_000,
            },
        },
        input: plan_support::protobuf_payload(
            MODULE_ID,
            CREATE_PRIVACY_CASE_REQUEST_SCHEMA,
            DataClass::Confidential,
            &wire::CreatePrivacyCaseRequest {
                kind: wire::PrivacyCaseKind::Erasure as i32,
                policy_version: "privacy-policy/1".to_owned(),
                previous_privacy_case_ref: None,
            },
        )
        .unwrap(),
        input_hash: [3; 32],
        approval: None,
    };
    let definition = capability_definition().unwrap();
    let plan = CustomerPrivacyCaseCreateCapabilityPlanner
        .plan(&definition, &request, None)
        .expect("plan diagnostic create");
    store
        .execute_batch(&plan.batch)
        .await
        .expect("raw create batch must expose its PostgreSQL error");
}
