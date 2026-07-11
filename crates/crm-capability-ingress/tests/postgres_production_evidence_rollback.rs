#![cfg(feature = "postgres-integration")]

use ::http::{HeaderMap, HeaderValue, StatusCode};
use crm_capability_adapters::{
    ApprovalStore, AuthorizationGrant, FixedWindowRateLimiter, LiveAuthorizationStore,
    LiveCapabilityAuthorizer, RateLimitPolicyStore, StoredApprovalVerifier,
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
use crm_proto_contracts::crm::{core::v1 as core, sales::v1 as sales};
use crm_sales_activities_capability_composition::{
    SalesActivitiesCapabilityPlannerRouter, capability_catalog, capability_definitions,
};
use prost::Message;
use sqlx::{PgPool, Row};
use std::collections::BTreeSet;
use std::sync::Arc;

const TENANT: &str = "tenant-a";
const ACTOR: &str = "actor-a";
const TOKEN: &str = "phase6g-fault-0123456789abcdef0123456789abcdef";
const NOW: i64 = 1_700_000_300_000_000_000;
const SALES_CREATE: &str = "sales.deal.create";

#[derive(Debug, Clone, Copy)]
enum EvidenceFault {
    OmitOutbox,
    OmitAudit,
    OmitIdempotency,
}

#[derive(Debug, Clone, Copy)]
struct CorruptingPlanner {
    fault: EvidenceFault,
}

impl TransactionalAggregatePlanner for CorruptingPlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        SalesActivitiesCapabilityPlannerRouter.target(definition, request)
    }

    fn plan(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
        current: Option<&RecordSnapshot>,
    ) -> Result<CapabilityBatchExecutionPlan, SdkError> {
        let mut plan = SalesActivitiesCapabilityPlannerRouter.plan(definition, request, current)?;
        match self.fault {
            EvidenceFault::OmitOutbox => plan.batch.events.clear(),
            EvidenceFault::OmitAudit => plan.batch.audits.clear(),
            EvidenceFault::OmitIdempotency => plan.batch.idempotency.scope.clear(),
        }
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
async fn omitted_production_evidence_rolls_back_every_transactional_side_effect() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping Phase 6G evidence rollback because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let store = PostgresDataStore::connect(&database_url, 4)
        .await
        .expect("connect Phase 6G rollback runtime store");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect Phase 6G rollback evidence reader");

    for (case, fault) in [
        ("outbox", EvidenceFault::OmitOutbox),
        ("audit", EvidenceFault::OmitAudit),
        ("idempotency", EvidenceFault::OmitIdempotency),
    ] {
        let before = evidence_counts(&admin).await;
        let definition = definition();
        let middleware = compose(store.clone(), fault);
        let deal_id = format!("phase6g-fault-{case}-deal");
        let response = middleware
            .handle(http_request(
                &definition,
                &format!("phase6g-fault-{case}-idem"),
                &format!("phase6g-fault-{case}"),
                payload(&definition, sales_create(&deal_id)),
            ))
            .await;

        assert_eq!(response.status, StatusCode::INTERNAL_SERVER_ERROR);
        let error = match &response.body {
            HttpCapabilityBody::Error(error) => error,
            HttpCapabilityBody::Success(_) => {
                panic!("corrupted production plan unexpectedly succeeded")
            }
        };
        assert_eq!(error.code, "CAPABILITY_EXECUTION_PLAN_INVALID");
        assert_eq!(
            error.safe_message,
            "The capability execution plan is invalid."
        );
        let safe = error.safe_message.to_lowercase();
        assert!(!safe.contains("sql"));
        assert!(!safe.contains("database"));
        assert_eq!(evidence_counts(&admin).await, before);
        assert_eq!(record_count(&admin, &deal_id).await, 0);
    }
}

fn compose(store: PostgresDataStore, fault: EvidenceFault) -> HttpCapabilityMiddleware {
    let definition = definition();
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
            policy_version: "phase6g-fault-policy-1".to_owned(),
            expires_at_unix_nanos: Some(NOW + 10_000_000_000_000),
        })
        .expect("valid rollback authorization grant");
    let executor =
        PostgresTransactionalAggregateExecutor::new(store, Arc::new(CorruptingPlanner { fault }));
    let gateway = Arc::new(CapabilityGateway::new(
        Arc::new(capability_catalog().expect("valid production capability catalog")),
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
        Arc::new(DeterministicRandom::from_bytes(vec![0x71; 1024])),
        TimeoutPolicy {
            default_millis: 5_000,
            maximum_millis: 10_000,
        },
    )
    .expect("valid rollback context resolver");
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
                authentication_id: "phase6g-fault-session".to_owned(),
                expires_at_unix_nanos: NOW + 10_000_000_000_000,
            },
        )
        .expect("issue Phase 6G rollback token");
    store
}

fn definition() -> CapabilityDefinition {
    capability_definitions()
        .expect("valid production definitions")
        .into_iter()
        .find(|definition| definition.capability_id.as_str() == SALES_CREATE)
        .expect("Sales create definition")
}

fn payload<M: Message>(definition: &CapabilityDefinition, message: M) -> TypedPayload {
    TypedPayload {
        owner: definition.input_contract.owner.clone(),
        schema_id: definition.input_contract.schema_id.clone(),
        schema_version: definition.input_contract.schema_version.clone(),
        descriptor_hash: definition.input_contract.descriptor_hash,
        data_class: DataClass::Confidential,
        encoding: PayloadEncoding::Protobuf,
        maximum_size_bytes: definition.input_contract.maximum_size_bytes,
        retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
        bytes: message.encode_to_vec(),
    }
}

fn sales_create(deal_id: &str) -> sales::CreateDealRequest {
    sales::CreateDealRequest {
        deal_id: deal_id.to_owned(),
        name: "Rollback proof".to_owned(),
        owner: Some(core::ActorOrTeamOwner {
            owner: Some(core::actor_or_team_owner::Owner::ActorId(ACTOR.to_owned())),
        }),
        account: None,
        primary_contact: None,
        stage: Some(sales::DealStage {
            pipeline_id: "pipeline.enterprise".to_owned(),
            stage_id: "qualification".to_owned(),
            ordinal: 1,
        }),
        amount: Some(core::ExactMoney {
            minor_units: "10000".to_owned(),
            currency_code: "USD".to_owned(),
        }),
        expected_close_date: None,
        probability_basis_points: 1_000,
    }
}

fn http_request(
    definition: &CapabilityDefinition,
    idempotency_key: &str,
    request_identity: &str,
    input: TypedPayload,
) -> HttpCapabilityRequest {
    let mut headers = HeaderMap::new();
    insert_header(&mut headers, "authorization", &format!("Bearer {TOKEN}"));
    insert_header(&mut headers, TENANT_HEADER, TENANT);
    insert_header(&mut headers, IDEMPOTENCY_KEY_HEADER, idempotency_key);
    insert_header(&mut headers, REQUEST_ID_HEADER, request_identity);
    insert_header(
        &mut headers,
        CORRELATION_ID_HEADER,
        &format!("corr-{request_identity}"),
    );
    insert_header(
        &mut headers,
        CAUSATION_ID_HEADER,
        &format!("cause-{request_identity}"),
    );
    insert_header(
        &mut headers,
        TRACE_ID_HEADER,
        &format!("trace-{request_identity}"),
    );
    insert_header(
        &mut headers,
        BUSINESS_TRANSACTION_HEADER,
        &format!("phase6g-fault-tx-{request_identity}"),
    );
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
        HeaderValue::from_str(value).expect("valid rollback header"),
    );
}

async fn evidence_counts(pool: &PgPool) -> EvidenceCounts {
    EvidenceCounts {
        records: scalar_count(
            pool,
            "SELECT count(*) FROM crm.records WHERE tenant_id = $1 AND record_id LIKE 'phase6g-fault-%'",
        )
        .await,
        outbox: scalar_count(
            pool,
            "SELECT count(*) FROM crm.outbox_events WHERE tenant_id = $1 AND aggregate_id LIKE 'phase6g-fault-%'",
        )
        .await,
        audits: scalar_count(
            pool,
            "SELECT count(*) FROM crm.audit_records WHERE tenant_id = $1 AND business_transaction_id LIKE 'phase6g-fault-tx-%'",
        )
        .await,
        idempotency: scalar_count(
            pool,
            "SELECT count(*) FROM crm.idempotency_records WHERE tenant_id = $1 AND idempotency_key LIKE 'phase6g-fault-%'",
        )
        .await,
        transactions: scalar_count(
            pool,
            "SELECT count(*) FROM crm.business_transactions WHERE tenant_id = $1 AND business_transaction_id LIKE 'phase6g-fault-tx-%'",
        )
        .await,
    }
}

async fn scalar_count(pool: &PgPool, query: &'static str) -> i64 {
    sqlx::query(query)
        .bind(TENANT)
        .fetch_one(pool)
        .await
        .expect("count rollback evidence")
        .try_get(0)
        .expect("valid rollback count")
}

async fn record_count(pool: &PgPool, record_id: &str) -> i64 {
    sqlx::query("SELECT count(*) FROM crm.records WHERE tenant_id = $1 AND record_id = $2")
        .bind(TENANT)
        .bind(record_id)
        .fetch_one(pool)
        .await
        .expect("count rollback record")
        .try_get(0)
        .expect("valid rollback record count")
}
