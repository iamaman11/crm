use crm_capability_plan_support as support;
use crm_capability_runtime::{
    CapabilityDefinition, CapabilityRequest, TransactionalCapabilityExecutor,
};
use crm_core_data::{PostgresDataStore, PostgresTransactionalAggregateExecutor};
use crm_customer_privacy::{CANONICAL_SCOPE_REGISTRY_VERSION, OwnerScopeRegistry};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, CorrelationId,
    DataClass, ExecutionContext, IdempotencyKey, ModuleExecutionContext, ModuleId, PayloadEncoding,
    RequestId, RetentionPolicyId, SchemaId, SchemaVersion, TenantId, TraceId, TypedPayload,
};
use crm_parties_capability_adapter::{
    CREATE_CAPABILITY as CREATE_PARTY_CAPABILITY, CREATE_REQUEST_SCHEMA as CREATE_PARTY_SCHEMA,
    PartyCapabilityPlanner, capability_definition as party_definition,
};
use crm_parties_privacy_scope_adapter::{
    CAPABILITY_ID, CAPABILITY_VERSION, CONTRACT_SCHEMA_VERSION, INPUT_MAXIMUM_BYTES,
    INPUT_RETENTION_POLICY_ID, INPUT_SCHEMA_ID, PartiesPrivacyScopeQueryAdapter,
    parties_privacy_scope_definition,
};
use crm_proto_contracts::{
    crm::{customer::v1 as customer, customer_privacy::v1 as privacy, parties::v1 as parties},
    message_descriptor_hash,
};
use crm_query_runtime::{QueryExecutionContext, QueryExecutor, QueryRequest};
use prost::Message;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::sync::Arc;

const TENANT_A: &str = "tenant-a";
const TENANT_B: &str = "tenant-b";
const ACTOR: &str = "privacy-worker";

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn parties_scope_is_tenant_bound_strict_and_side_effect_free() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping Parties privacy scope PostgreSQL proof without DATABASE_URL");
        return;
    };
    let admin_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");

    let store = PostgresDataStore::connect(&database_url, 8)
        .await
        .expect("connect Parties privacy scope runtime store");
    let admin = PgPool::connect(&admin_url)
        .await
        .expect("connect Parties privacy scope evidence reader");
    let party_executor: Arc<dyn TransactionalCapabilityExecutor> =
        Arc::new(PostgresTransactionalAggregateExecutor::new(
            store.clone(),
            Arc::new(PartyCapabilityPlanner),
        ));
    let party_definition = party_definition(CREATE_PARTY_CAPABILITY).unwrap();

    create_party(
        &party_executor,
        &party_definition,
        TENANT_A,
        "party-scope",
        11,
    )
    .await;
    create_party(
        &party_executor,
        &party_definition,
        TENANT_A,
        "party-malformed",
        12,
    )
    .await;

    let adapter = PartiesPrivacyScopeQueryAdapter::new(store);
    let definition = parties_privacy_scope_definition().unwrap();

    let before_success = write_surface_counts(&admin).await;
    let result = adapter
        .execute(
            &definition,
            scope_request(TENANT_A, "party-scope", 1, "success"),
        )
        .await
        .expect("read authoritative Parties scope");
    assert_eq!(write_surface_counts(&admin).await, before_success);

    let response =
        privacy::PartiesPrivacyScopeContributionResponse::decode(result.output.bytes.as_slice())
            .expect("decode Parties scope response");
    let contribution = response.contribution.expect("scope contribution envelope");
    assert_eq!(contribution.owner_module_id, crm_parties::MODULE_ID);
    assert_eq!(contribution.capability_id, CAPABILITY_ID);
    assert_eq!(contribution.resources.len(), 1);
    assert_eq!(contribution.resources[0].resource_id, "party-scope");
    assert_eq!(contribution.resources[0].resource_version, 1);
    assert!(
        !result
            .output
            .bytes
            .windows(b"Scope Subject party-scope".len())
            .any(|window| window == b"Scope Subject party-scope")
    );

    let stale_before = write_surface_counts(&admin).await;
    let stale = adapter
        .execute(
            &definition,
            scope_request(TENANT_A, "party-scope", 2, "stale-generation"),
        )
        .await
        .expect_err("stale topology generation must fail closed");
    assert_eq!(stale.code, "PARTIES_PRIVACY_SCOPE_LINEAGE_INVALID");
    assert!(stale.retryable);
    assert_eq!(write_surface_counts(&admin).await, stale_before);

    let cross_tenant_before = write_surface_counts(&admin).await;
    let cross_tenant = adapter
        .execute(
            &definition,
            scope_request(TENANT_B, "party-scope", 1, "cross-tenant"),
        )
        .await
        .expect_err("cross-tenant Party scope must be concealed");
    assert_eq!(cross_tenant.code, "PARTIES_PRIVACY_SCOPE_LINEAGE_INVALID");
    assert_eq!(write_surface_counts(&admin).await, cross_tenant_before);

    sqlx::query(
        r#"
        UPDATE crm.records
        SET schema_id = 'crm.parties.party.state.invalid'
        WHERE tenant_id = $1
          AND owner_module_id = $2
          AND record_type = $3
          AND record_id = $4
        "#,
    )
    .bind(TENANT_A)
    .bind(crm_parties::MODULE_ID)
    .bind(crm_parties_capability_adapter::RECORD_TYPE)
    .bind("party-malformed")
    .execute(&admin)
    .await
    .expect("corrupt isolated Party metadata fixture");

    let malformed_before = write_surface_counts(&admin).await;
    let malformed = adapter
        .execute(
            &definition,
            scope_request(TENANT_A, "party-malformed", 1, "malformed"),
        )
        .await
        .expect_err("malformed Party state must fail closed");
    assert_eq!(malformed.code, "PARTIES_PRIVACY_SCOPE_STORED_STATE_INVALID");
    assert_eq!(write_surface_counts(&admin).await, malformed_before);
}

async fn create_party(
    executor: &Arc<dyn TransactionalCapabilityExecutor>,
    definition: &CapabilityDefinition,
    tenant: &str,
    party_id: &str,
    seed: u8,
) {
    executor
        .execute(
            definition,
            capability_request(
                tenant,
                &format!("party-{party_id}"),
                500_000_000 + i64::from(seed),
                seed,
                &parties::CreatePartyRequest {
                    party_ref: Some(customer::PartyRef {
                        party_id: party_id.to_owned(),
                    }),
                    kind: parties::PartyKind::Person as i32,
                    display_name: format!("Scope Subject {party_id}"),
                },
            ),
        )
        .await
        .expect("create authoritative Party fixture");
}

fn capability_request<M: Message>(
    tenant: &str,
    identity: &str,
    started_at: i64,
    hash: u8,
    command: &M,
) -> CapabilityRequest {
    CapabilityRequest {
        context: ModuleExecutionContext {
            module_id: ModuleId::try_new(crm_parties::MODULE_ID).unwrap(),
            execution: ExecutionContext {
                tenant_id: TenantId::try_new(tenant).unwrap(),
                actor_id: ActorId::try_new("party-fixture").unwrap(),
                request_id: RequestId::try_new(format!("request-{identity}")).unwrap(),
                correlation_id: CorrelationId::try_new(format!("correlation-{identity}")).unwrap(),
                causation_id: CausationId::try_new(format!("causation-{identity}")).unwrap(),
                trace_id: TraceId::try_new(format!("trace-{identity}")).unwrap(),
                capability_id: CapabilityId::try_new(CREATE_PARTY_CAPABILITY).unwrap(),
                capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                idempotency_key: IdempotencyKey::try_new(format!("{identity}-key")).unwrap(),
                business_transaction_id: BusinessTransactionId::try_new(format!("{identity}-tx"))
                    .unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                request_started_at_unix_nanos: started_at,
            },
        },
        input: support::protobuf_payload(
            crm_parties::MODULE_ID,
            CREATE_PARTY_SCHEMA,
            DataClass::Personal,
            command,
        )
        .unwrap(),
        input_hash: [hash; 32],
        approval: None,
    }
}

fn scope_request(tenant: &str, party_id: &str, generation: u64, identity: &str) -> QueryRequest {
    let registry = OwnerScopeRegistry::canonical_v1().unwrap();
    let wire = privacy::PartiesPrivacyScopeContributionRequest {
        contribution: Some(privacy::PrivacyScopeContributionRequestEnvelope {
            lineage: Some(privacy::PrivacyScopeContributionLineage {
                privacy_case_id: format!("privacy-case-{identity}"),
                tenant_id: tenant.to_owned(),
                canonical_party_ref: Some(customer::PartyRef {
                    party_id: party_id.to_owned(),
                }),
                identity_resolution_generation: generation,
                registry_version: CANONICAL_SCOPE_REGISTRY_VERSION.to_owned(),
                registry_digest_sha256: registry.digest().to_vec(),
                purpose_code: "PRIVACY_ERASURE_SCOPE".to_owned(),
                effective_request_at_unix_ms: 1_000,
            }),
            page_size: 0,
            cursor: String::new(),
        }),
    };
    let bytes = wire.encode_to_vec();
    let input_hash: [u8; 32] = Sha256::digest(&bytes).into();

    QueryRequest {
        owner_module_id: ModuleId::try_new(crm_parties::MODULE_ID).unwrap(),
        context: QueryExecutionContext {
            tenant_id: TenantId::try_new(tenant).unwrap(),
            actor_id: ActorId::try_new(ACTOR).unwrap(),
            request_id: RequestId::try_new(format!("request-{identity}")).unwrap(),
            correlation_id: CorrelationId::try_new(format!("correlation-{identity}")).unwrap(),
            trace_id: TraceId::try_new(format!("trace-{identity}")).unwrap(),
            capability_id: CapabilityId::try_new(CAPABILITY_ID).unwrap(),
            capability_version: CapabilityVersion::try_new(CAPABILITY_VERSION).unwrap(),
            schema_version: SchemaVersion::try_new(CONTRACT_SCHEMA_VERSION).unwrap(),
            request_started_at_unix_nanos: 2_000_000_000,
        },
        input: TypedPayload {
            owner: ModuleId::try_new(crm_parties::MODULE_ID).unwrap(),
            schema_id: SchemaId::try_new(INPUT_SCHEMA_ID).unwrap(),
            schema_version: SchemaVersion::try_new(CONTRACT_SCHEMA_VERSION).unwrap(),
            descriptor_hash: message_descriptor_hash(INPUT_SCHEMA_ID),
            data_class: DataClass::Confidential,
            encoding: PayloadEncoding::Protobuf,
            maximum_size_bytes: INPUT_MAXIMUM_BYTES,
            retention_policy_id: RetentionPolicyId::try_new(INPUT_RETENTION_POLICY_ID).unwrap(),
            bytes,
        },
        input_hash,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WriteSurfaceCounts {
    records: i64,
    business_transactions: i64,
    idempotency_records: i64,
    outbox_events: i64,
    outbox_delivery: i64,
    audit_heads: i64,
    audit_records: i64,
}

async fn write_surface_counts(pool: &PgPool) -> WriteSurfaceCounts {
    WriteSurfaceCounts {
        records: sqlx::query_scalar("SELECT count(*)::bigint FROM crm.records")
            .fetch_one(pool)
            .await
            .expect("count CRM records"),
        business_transactions: sqlx::query_scalar(
            "SELECT count(*)::bigint FROM crm.business_transactions",
        )
        .fetch_one(pool)
        .await
        .expect("count CRM business transactions"),
        idempotency_records: sqlx::query_scalar(
            "SELECT count(*)::bigint FROM crm.idempotency_records",
        )
        .fetch_one(pool)
        .await
        .expect("count CRM idempotency records"),
        outbox_events: sqlx::query_scalar("SELECT count(*)::bigint FROM crm.outbox_events")
            .fetch_one(pool)
            .await
            .expect("count CRM outbox events"),
        outbox_delivery: sqlx::query_scalar("SELECT count(*)::bigint FROM crm.outbox_delivery")
            .fetch_one(pool)
            .await
            .expect("count CRM outbox delivery rows"),
        audit_heads: sqlx::query_scalar("SELECT count(*)::bigint FROM crm.audit_heads")
            .fetch_one(pool)
            .await
            .expect("count CRM audit heads"),
        audit_records: sqlx::query_scalar("SELECT count(*)::bigint FROM crm.audit_records")
            .fetch_one(pool)
            .await
            .expect("count CRM audit records"),
    }
}
