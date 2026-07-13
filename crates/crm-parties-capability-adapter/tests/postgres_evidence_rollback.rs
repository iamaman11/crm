#![cfg(feature = "postgres-integration")]

use crm_capability_adapters::{
    ApprovalStore, AuthorizationGrant, CapabilityCatalog, FixedWindowRateLimiter,
    LiveAuthorizationStore, LiveCapabilityAuthorizer, RateLimitPolicyStore, StoredApprovalVerifier,
};
use crm_capability_ingress::{
    AccessTokenGrant, AccessTokenStore, BUSINESS_TRANSACTION_HEADER, BearerTokenAuthenticator,
    CAUSATION_ID_HEADER, CORRELATION_ID_HEADER, CapabilityIngress, CapabilityRoute,
    ExecutionContextResolver, HttpCapabilityBody, HttpCapabilityMiddleware, HttpCapabilityRequest,
    IDEMPOTENCY_KEY_HEADER, REQUEST_ID_HEADER, TENANT_HEADER, TIMEOUT_HEADER, TRACE_ID_HEADER,
    TimeoutPolicy, semantic_input_hash,
};
use crm_capability_runtime::{
    CapabilityDefinition, CapabilityGateway, CapabilityRequest, CapabilitySemanticValidator,
};
use crm_core_data::{
    AggregateTarget, CapabilityBatchExecutionPlan, PostgresDataStore,
    PostgresTransactionalAggregateExecutor, TransactionalAggregatePlanner,
};
use crm_module_sdk::testing::{DeterministicRandom, FixedClock};
use crm_module_sdk::{
    ActorId, Clock, DataClass, ErrorCategory, PayloadEncoding, PortFuture, RecordSnapshot,
    RetentionPolicyId, SdkError, TenantId, TypedPayload,
};
use crm_parties_capability_adapter::{
    CREATE_CAPABILITY, PartyCapabilityPlanner, capability_definition,
};
use crm_proto_contracts::crm::{customer::v1 as customer, parties::v1 as parties};
use http::{HeaderMap, HeaderValue, StatusCode};
use prost::Message;
use sqlx::PgPool;
use std::collections::BTreeSet;
use std::sync::Arc;

const TENANT: &str = "party-evidence-rollback";
const ACTOR: &str = "party-evidence-actor";
const TOKEN: &str = "party-evidence-token-0123456789abcdef0123456789abcdef";
const PARTY_ID: &str = "party-evidence-rollback-record";
const IDEMPOTENCY_KEY: &str = "party-evidence-rollback-idem";
const TRANSACTION_ID: &str = "party-evidence-rollback-tx";
const NOW: i64 = 1_800_000_000_000_000_000;

#[derive(Debug, Clone, Copy)]
struct MissingAuditPlanner;

impl TransactionalAggregatePlanner for MissingAuditPlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        PartyCapabilityPlanner.target(definition, request)
    }

    fn plan(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
        current: Option<&RecordSnapshot>,
    ) -> Result<CapabilityBatchExecutionPlan, SdkError> {
        let mut plan = PartyCapabilityPlanner.plan(definition, request, current)?;
        plan.batch.audits.clear();
        Ok(plan)
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EvidenceCounts {
    records: i64,
    outbox: i64,
    audits: i64,
    idempotency: i64,
    transactions: i64,
}

#[tokio::test(flavor = "current_thread")]
async fn missing_party_audit_evidence_rolls_back_every_transactional_side_effect() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping Party evidence rollback because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let store = PostgresDataStore::connect(&database_url, 4)
        .await
        .expect("connect Party rollback runtime store");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect Party rollback evidence reader");
    let before = evidence_counts(&admin).await;

    let definition = capability_definition(CREATE_CAPABILITY).unwrap();
    let middleware = compose(store, definition.clone());
    let response = middleware
        .handle(http_request(
            &definition,
            payload(
                &definition,
                parties::CreatePartyRequest {
                    party_ref: Some(customer::PartyRef {
                        party_id: PARTY_ID.to_owned(),
                    }),
                    kind: parties::PartyKind::Organization as i32,
                    display_name: "Rollback Holdings".to_owned(),
                },
            ),
        ))
        .await;

    assert_eq!(response.status, StatusCode::INTERNAL_SERVER_ERROR);
    let error = match response.body {
        HttpCapabilityBody::Error(error) => error,
        HttpCapabilityBody::Success(_) => panic!("corrupted Party plan unexpectedly succeeded"),
    };
    assert_eq!(error.code, "CAPABILITY_EXECUTION_PLAN_INVALID");
    assert_eq!(
        error.safe_message,
        "The capability execution plan is invalid."
    );
    assert_eq!(evidence_counts(&admin).await, before);
    assert_eq!(party_record_count(&admin).await, 0);
}

fn compose(store: PostgresDataStore, definition: CapabilityDefinition) -> HttpCapabilityMiddleware {
    let clock = Arc::new(FixedClock::new(NOW));
    let clock_port: Arc<dyn Clock> = clock.clone();
    let authorization_store = LiveAuthorizationStore::default();
    authorization_store
        .upsert(AuthorizationGrant {
            tenant_id: TenantId::try_new(TENANT).unwrap(),
            actor_id: ActorId::try_new(ACTOR).unwrap(),
            policy_id: definition.authorization_policy_id.clone(),
            capability_id: definition.capability_id.clone(),
            capability_version: definition.capability_version.clone(),
            owner_module_id: definition.owner_module_id.clone(),
            policy_version: "party-evidence-policy-1".to_owned(),
            expires_at_unix_nanos: Some(NOW + 10_000_000_000_000),
        })
        .expect("valid Party rollback authorization grant");
    let executor =
        PostgresTransactionalAggregateExecutor::new(store, Arc::new(MissingAuditPlanner));
    let catalog = CapabilityCatalog::new(vec![definition]).expect("valid Party capability catalog");
    let gateway = Arc::new(CapabilityGateway::new(
        Arc::new(catalog),
        Arc::new(SemanticHashValidator),
        Arc::new(FixedWindowRateLimiter::new(
            RateLimitPolicyStore::default(),
            Arc::clone(&clock_port),
        )),
        Arc::new(StoredApprovalVerifier::new(ApprovalStore::default())),
        Arc::new(LiveCapabilityAuthorizer::new(
            authorization_store,
            Arc::clone(&clock_port),
        )),
        Arc::new(executor),
        Arc::clone(&clock_port),
    ));
    let resolver = ExecutionContextResolver::new(
        Arc::clone(&clock_port),
        Arc::new(DeterministicRandom::from_bytes(vec![0x81; 1024])),
        TimeoutPolicy {
            default_millis: 5_000,
            maximum_millis: 10_000,
        },
    )
    .expect("valid Party rollback context resolver");
    let authenticator = BearerTokenAuthenticator::new(token_store(), clock_port);
    HttpCapabilityMiddleware::new(CapabilityIngress::new(
        Arc::new(authenticator),
        resolver,
        gateway,
    ))
}

fn token_store() -> AccessTokenStore {
    let store = AccessTokenStore::default();
    store
        .issue(
            TOKEN.as_bytes(),
            AccessTokenGrant {
                actor_id: ActorId::try_new(ACTOR).unwrap(),
                tenant_ids: BTreeSet::from([TenantId::try_new(TENANT).unwrap()]),
                authentication_id: "party-evidence-session".to_owned(),
                expires_at_unix_nanos: NOW + 10_000_000_000_000,
            },
        )
        .expect("issue Party rollback token");
    store
}

fn payload<M: Message>(definition: &CapabilityDefinition, message: M) -> TypedPayload {
    TypedPayload {
        owner: definition.input_contract.owner.clone(),
        schema_id: definition.input_contract.schema_id.clone(),
        schema_version: definition.input_contract.schema_version.clone(),
        descriptor_hash: definition.input_contract.descriptor_hash,
        data_class: DataClass::Personal,
        encoding: PayloadEncoding::Protobuf,
        maximum_size_bytes: definition.input_contract.maximum_size_bytes,
        retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
        bytes: message.encode_to_vec(),
    }
}

fn http_request(definition: &CapabilityDefinition, input: TypedPayload) -> HttpCapabilityRequest {
    let mut headers = HeaderMap::new();
    insert_header(&mut headers, "authorization", &format!("Bearer {TOKEN}"));
    insert_header(&mut headers, TENANT_HEADER, TENANT);
    insert_header(&mut headers, IDEMPOTENCY_KEY_HEADER, IDEMPOTENCY_KEY);
    insert_header(&mut headers, REQUEST_ID_HEADER, "party-evidence-request");
    insert_header(
        &mut headers,
        CORRELATION_ID_HEADER,
        "party-evidence-correlation",
    );
    insert_header(
        &mut headers,
        CAUSATION_ID_HEADER,
        "party-evidence-causation",
    );
    insert_header(&mut headers, TRACE_ID_HEADER, "party-evidence-trace");
    insert_header(&mut headers, BUSINESS_TRANSACTION_HEADER, TRANSACTION_ID);
    insert_header(&mut headers, TIMEOUT_HEADER, "5000");
    HttpCapabilityRequest {
        headers,
        route: CapabilityRoute {
            owner_module_id: definition.owner_module_id.clone(),
            capability_id: definition.capability_id.clone(),
            capability_version: definition.capability_version.clone(),
            schema_version: definition.input_contract.schema_version.clone(),
        },
        input,
        approval: None,
    }
}

fn insert_header(headers: &mut HeaderMap, name: &'static str, value: &str) {
    headers.insert(
        name,
        HeaderValue::from_str(value).expect("valid Party rollback header"),
    );
}

async fn evidence_counts(pool: &PgPool) -> EvidenceCounts {
    EvidenceCounts {
        records: scalar_count(
            pool,
            "SELECT count(*) FROM crm.records WHERE tenant_id = $1 AND record_id = 'party-evidence-rollback-record'",
        )
        .await,
        outbox: scalar_count(
            pool,
            "SELECT count(*) FROM crm.outbox_events WHERE tenant_id = $1 AND aggregate_id = 'party-evidence-rollback-record'",
        )
        .await,
        audits: scalar_count(
            pool,
            "SELECT count(*) FROM crm.audit_records WHERE tenant_id = $1 AND business_transaction_id = 'party-evidence-rollback-tx'",
        )
        .await,
        idempotency: scalar_count(
            pool,
            "SELECT count(*) FROM crm.idempotency_records WHERE tenant_id = $1 AND idempotency_key = 'party-evidence-rollback-idem'",
        )
        .await,
        transactions: scalar_count(
            pool,
            "SELECT count(*) FROM crm.business_transactions WHERE tenant_id = $1 AND business_transaction_id = 'party-evidence-rollback-tx'",
        )
        .await,
    }
}

async fn scalar_count(pool: &PgPool, query: &'static str) -> i64 {
    sqlx::query_scalar::<_, i64>(query)
        .bind(TENANT)
        .fetch_one(pool)
        .await
        .expect("read Party rollback evidence count")
}

async fn party_record_count(pool: &PgPool) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.records WHERE tenant_id = $1 AND owner_module_id = 'crm.parties' AND record_type = 'parties.party' AND record_id = $2",
    )
    .bind(TENANT)
    .bind(PARTY_ID)
    .fetch_one(pool)
    .await
    .expect("count rolled-back Party record")
}
