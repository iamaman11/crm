#[path = "support/customer_enrichment_suggestion_get.rs"]
mod support;

use crm_application_runtime::{
    CustomerEnrichmentApplicationWorkerDependencies, PostgresModuleActivation,
    ProductionCompositionDependencies, build_customer_enrichment_application_worker,
    build_production_composition,
};
use crm_capability_adapters::{
    AuthorizationGrant, GatewayCapabilityClient, LiveAuthorizationStore, LiveCapabilityAuthorizer,
    LiveQueryVisibilityAuthorizer, LiveQueryVisibilityStore, QueryVisibilityGrant,
};
use crm_capability_ingress::semantic_input_hash;
use crm_capability_plan_support as plan_support;
use crm_capability_runtime::{
    ApprovalEvidence, CapabilityApprovalVerifier, CapabilityDefinition, CapabilityGateway,
    CapabilityRateLimiter, CapabilityRequest, RateLimitDecision,
};
use crm_core_data::{
    AuditIntent, IdempotencyEvidence, PostgresDataStore, RecordCreatePlan, RecordGetQuery,
};
use crm_customer_enrichment::{
    APPLICATION_ATTEMPT_RECORD_TYPE, ApprovalRequirement, ReviewDecision, ReviewDecisionKind,
};
use crm_customer_enrichment_application_composition::{
    PARTY_DISPLAY_NAME_APPLICATION_PROJECTION_ID, PARTY_DISPLAY_NAME_APPLICATION_WORKER_ACTOR_ID,
};
use crm_customer_enrichment_capability_adapter::MODULE_ID;
use crm_customer_enrichment_review_adapter::{
    SUGGESTION_REVIEWED_EVENT_SCHEMA, SUGGESTION_REVIEWED_EVENT_TYPE,
    review_decision_persisted_payload, review_decision_record_ref, review_decision_to_wire,
    suggestion_to_wire,
};
use crm_module_sdk::testing::FixedClock;
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, Clock,
    CorrelationId, DataClass, DomainEvent, EventType, ExecutionContext, IdempotencyKey,
    ModuleExecutionContext, ModuleId, PortFuture, RecordId, RecordType, RequestId, SchemaVersion,
    SdkError, TraceId,
};
use crm_parties_capability_adapter::{
    MODULE_ID as PARTIES_MODULE_ID, RECORD_TYPE as PARTY_RECORD_TYPE,
    UPDATE_CAPABILITY as PARTY_UPDATE_CAPABILITY, party_from_snapshot,
};
use crm_parties_query_adapter::{
    GET_CAPABILITY as PARTY_GET_CAPABILITY, query_capability_definition as party_query_definition,
};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;
use crm_query_runtime::QueryVisibilityAuthorizer;
use sqlx::PgPool;
use std::collections::BTreeSet;
use std::sync::Arc;
use support::*;

const CURSOR_KEY: [u8; 32] = [0x7a; 32];
const REVIEW_SEED_CAPABILITY: &str = "customer_enrichment.application.worker.seed";

#[tokio::test(flavor = "current_thread")]
async fn production_application_worker_uses_governed_party_gateway_and_recovers_after_grant() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping production application worker because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let store = PostgresDataStore::connect(&database_url, 6)
        .await
        .expect("connect production application store");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect production application evidence reader");
    let suggestion = suggestion();
    seed_suggestion(&store, &suggestion)
        .await
        .expect("seed Party and suggestion");
    activate_modules(&store).await;
    let review = accepted_review(&suggestion);
    seed_review(&store, &suggestion, &review)
        .await
        .expect("seed accepted review event");

    let clock: Arc<dyn Clock> = Arc::new(FixedClock::new(NOW + 2_000_000));
    let authorization_store = LiveAuthorizationStore::default();
    let visibility_store = LiveQueryVisibilityStore::default();
    let authorizer = Arc::new(LiveCapabilityAuthorizer::new(
        authorization_store.clone(),
        Arc::clone(&clock),
    ));
    let visibility_authorizer: Arc<dyn QueryVisibilityAuthorizer> = Arc::new(
        LiveQueryVisibilityAuthorizer::new(visibility_store.clone(), Arc::clone(&clock)),
    );
    let composition = build_production_composition(ProductionCompositionDependencies {
        store: store.clone(),
        activation: Arc::new(PostgresModuleActivation::new(store.clone())),
        capability_authorizer: authorizer.clone(),
        query_authorizer: authorizer.clone(),
        visibility_authorizer: visibility_authorizer.clone(),
        cursor_key: CURSOR_KEY,
    })
    .expect("assemble production composition");
    let party_update_definition = composition
        .mutation_definitions()
        .iter()
        .find(|definition| {
            definition.owner_module_id.as_str() == PARTIES_MODULE_ID
                && definition.capability_id.as_str() == PARTY_UPDATE_CAPABILITY
        })
        .cloned()
        .expect("Party update route in production composition");
    let party_get_definition =
        party_query_definition(PARTY_GET_CAPABILITY).expect("Party get definition");
    let gateway = Arc::new(CapabilityGateway::new(
        composition.mutation_registry(),
        composition.mutation_validator(),
        Arc::new(AllowRateLimiter),
        Arc::new(NoopApprovalVerifier),
        authorizer.clone(),
        composition.mutation_executor(),
        Arc::clone(&clock),
    ));
    let worker_actor = ActorId::try_new(PARTY_DISPLAY_NAME_APPLICATION_WORKER_ACTOR_ID)
        .expect("canonical application worker actor");
    let worker = build_customer_enrichment_application_worker(
        CustomerEnrichmentApplicationWorkerDependencies {
            store: store.clone(),
            capabilities: Arc::new(GatewayCapabilityClient::new(gateway)),
            query_authorizer: authorizer.clone(),
            visibility_authorizer: visibility_authorizer.clone(),
            clock: Arc::clone(&clock),
            cursor_key: CURSOR_KEY,
            actor_id: worker_actor.clone(),
        },
    )
    .expect("assemble production application worker");

    let denied = worker
        .run_cycle(tenant(TENANT), NOW + 2_000_000)
        .await
        .expect_err("missing worker Party grant must fail closed");
    assert_eq!(
        denied.code,
        "CUSTOMER_ENRICHMENT_APPLICATION_PARTY_PERMISSION_DENIED"
    );
    assert_eq!(application_attempt_version(&admin).await, 1);
    assert_eq!(party_version(&store, &suggestion).await, 7);

    store
        .reset_projection(
            &tenant(TENANT),
            PARTY_DISPLAY_NAME_APPLICATION_PROJECTION_ID,
        )
        .await
        .expect("reset repaired application projection");
    authorization_store
        .upsert(worker_authorization_grant(
            &party_update_definition,
            &worker_actor,
        ))
        .expect("grant worker Party update");
    authorization_store
        .upsert(worker_authorization_grant(
            &party_get_definition,
            &worker_actor,
        ))
        .expect("grant worker Party get");
    visibility_store
        .upsert(worker_party_visibility(
            &party_get_definition,
            &worker_actor,
        ))
        .expect("grant worker Party visibility");

    let applied = worker
        .run_cycle(tenant(TENANT), NOW + 3_000_000)
        .await
        .expect("apply accepted suggestion through production owner gateway");
    assert_eq!(applied.reviewed_events, 1);
    assert_eq!(applied.accepted_events, 1);
    assert_eq!(applied.replayed_attempts, 1);
    assert_eq!(application_attempt_version(&admin).await, 2);
    let party = party_state(&store, &suggestion).await;
    assert_eq!(party.0, 8);
    assert_eq!(party.1, suggestion.proposed_value());

    let evidence_after_application = evidence_counts(&admin).await;
    let replay = worker
        .run_cycle(tenant(TENANT), NOW + 4_000_000)
        .await
        .expect("completed application replay");
    assert_eq!(replay.reviewed_events, 0);
    assert_eq!(evidence_counts(&admin).await, evidence_after_application);

    let cross_tenant = worker
        .run_cycle(tenant(OTHER_TENANT), NOW + 5_000_000)
        .await
        .expect("cross-tenant worker scan");
    assert_eq!(cross_tenant.reviewed_events, 0);
    assert_eq!(party_version(&store, &suggestion).await, 8);
}

async fn activate_modules(store: &PostgresDataStore) {
    store
        .bootstrap_activate_published_modules(
            &BTreeSet::from([tenant(TENANT)]),
            &BTreeSet::from([MODULE_ID.to_owned(), PARTIES_MODULE_ID.to_owned()]),
        )
        .await
        .expect("activate Customer Enrichment and Parties");
}

fn accepted_review(suggestion: &crm_customer_enrichment::Suggestion) -> ReviewDecision {
    ReviewDecision::decide(
        suggestion,
        actor(),
        ReviewDecisionKind::Accepted,
        "review-policy-v1",
        "reviewed_accepted",
        ApprovalRequirement::Required,
        Some("approval-production-worker-1".to_owned()),
        u64::try_from(NOW / 1_000_000).expect("nonnegative review time"),
        Some(u64::try_from(NOW / 1_000_000 + 50_000).expect("review expiry")),
    )
    .expect("valid accepted review")
}

async fn seed_review(
    store: &PostgresDataStore,
    suggestion: &crm_customer_enrichment::Suggestion,
    review: &ReviewDecision,
) -> Result<(), SdkError> {
    let reference = review_decision_record_ref(review)?;
    let event_payload = plan_support::protobuf_payload(
        MODULE_ID,
        SUGGESTION_REVIEWED_EVENT_SCHEMA,
        DataClass::Personal,
        &wire::SuggestionReviewedEvent {
            suggestion: Some(suggestion_to_wire(
                suggestion,
                Some(review),
                u64::try_from(NOW / 1_000_000).map_err(test_configuration_error)?,
            )?),
            review_decision: Some(review_decision_to_wire(review)?),
        },
    )?;
    store
        .create_record(&RecordCreatePlan {
            context: review_seed_context()?,
            record: reference.clone(),
            record_payload: review_decision_persisted_payload(review)?,
            event_id: "application-worker-review-event".to_owned(),
            event: DomainEvent {
                event_type: EventType::try_new(SUGGESTION_REVIEWED_EVENT_TYPE)
                    .map_err(test_configuration_error)?,
                aggregate: reference,
                expected_aggregate_version: None,
                deduplication_key: "application-worker-review-event".to_owned(),
                payload: event_payload.clone(),
            },
            idempotency: IdempotencyEvidence {
                scope: format!("{REVIEW_SEED_CAPABILITY}@1.0.0"),
                key: "application-worker-review-idempotency".to_owned(),
                request_hash: semantic_input_hash(&event_payload),
                expires_at_unix_nanos: NOW + 86_400_000_000_000,
            },
            audit: AuditIntent {
                audit_record_id: "application-worker-review-audit".to_owned(),
                canonicalization_profile: "crm.cjson/v1".to_owned(),
                canonical_envelope: b"{\"seed\":\"application_worker_review\"}".to_vec(),
                occurred_at_unix_nanos: NOW,
            },
        })
        .await
        .map_err(|error| test_configuration_error(error.to_string()))?;
    Ok(())
}

fn review_seed_context() -> Result<ModuleExecutionContext, SdkError> {
    Ok(ModuleExecutionContext {
        module_id: ModuleId::try_new(MODULE_ID).map_err(test_configuration_error)?,
        execution: ExecutionContext {
            tenant_id: tenant(TENANT),
            actor_id: actor(),
            request_id: RequestId::try_new("application-worker-review-request")
                .map_err(test_configuration_error)?,
            correlation_id: CorrelationId::try_new("application-worker-review-correlation")
                .map_err(test_configuration_error)?,
            causation_id: CausationId::try_new("application-worker-review-causation")
                .map_err(test_configuration_error)?,
            trace_id: TraceId::try_new("application-worker-review-trace")
                .map_err(test_configuration_error)?,
            capability_id: CapabilityId::try_new(REVIEW_SEED_CAPABILITY)
                .map_err(test_configuration_error)?,
            capability_version: CapabilityVersion::try_new("1.0.0")
                .map_err(test_configuration_error)?,
            idempotency_key: IdempotencyKey::try_new("application-worker-review-idempotency")
                .map_err(test_configuration_error)?,
            business_transaction_id: BusinessTransactionId::try_new(
                "application-worker-review-transaction",
            )
            .map_err(test_configuration_error)?,
            schema_version: SchemaVersion::try_new("1.0.0").map_err(test_configuration_error)?,
            request_started_at_unix_nanos: NOW,
        },
    })
}

fn worker_authorization_grant(
    definition: &CapabilityDefinition,
    actor_id: &ActorId,
) -> AuthorizationGrant {
    AuthorizationGrant {
        tenant_id: tenant(TENANT),
        actor_id: actor_id.clone(),
        policy_id: definition.authorization_policy_id.clone(),
        capability_id: definition.capability_id.clone(),
        capability_version: definition.capability_version.clone(),
        owner_module_id: definition.owner_module_id.clone(),
        policy_version: "application-worker-auth-v1".to_owned(),
        expires_at_unix_nanos: Some(NOW + 10_000_000_000_000),
    }
}

fn worker_party_visibility(
    definition: &CapabilityDefinition,
    actor_id: &ActorId,
) -> QueryVisibilityGrant {
    QueryVisibilityGrant {
        tenant_id: tenant(TENANT),
        actor_id: actor_id.clone(),
        capability_id: definition.capability_id.clone(),
        capability_version: definition.capability_version.clone(),
        owner_module_id: definition.owner_module_id.clone(),
        record_type: RecordType::try_new(PARTY_RECORD_TYPE).expect("Party record type"),
        record_id: None,
        allowed_fields: BTreeSet::from(["display_name".to_owned()]),
        policy_version: "application-worker-visibility-v1".to_owned(),
        expires_at_unix_nanos: Some(NOW + 10_000_000_000_000),
    }
}

async fn application_attempt_version(admin: &PgPool) -> i64 {
    sqlx::query_scalar(
        "SELECT version::bigint FROM crm.records WHERE tenant_id = $1 AND record_type = $2",
    )
    .bind(TENANT)
    .bind(APPLICATION_ATTEMPT_RECORD_TYPE)
    .fetch_one(admin)
    .await
    .expect("application attempt version")
}

async fn party_version(
    store: &PostgresDataStore,
    suggestion: &crm_customer_enrichment::Suggestion,
) -> i64 {
    party_state(store, suggestion).await.0
}

async fn party_state(
    store: &PostgresDataStore,
    suggestion: &crm_customer_enrichment::Suggestion,
) -> (i64, String) {
    let party_id = RecordId::try_new(suggestion.target().resource_id.as_str())
        .expect("canonical suggestion Party id");
    let snapshot = store
        .get_record_for_query(&RecordGetQuery {
            tenant_id: tenant(TENANT),
            owner_module_id: ModuleId::try_new(PARTIES_MODULE_ID).expect("Parties module id"),
            record_type: RecordType::try_new(PARTY_RECORD_TYPE).expect("Party record type"),
            record_id: party_id,
        })
        .await
        .expect("read Party state")
        .expect("Party exists");
    let version = snapshot.version;
    let party = party_from_snapshot(&snapshot).expect("decode Party state");
    (version, party.display_name().to_owned())
}

fn test_configuration_error(error: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_APPLICATION_WORKER_TEST_CONFIGURATION_INVALID",
        crm_module_sdk::ErrorCategory::Internal,
        false,
        "The production application-worker test fixture is invalid.",
    )
    .with_internal_reference(error.to_string())
}

#[derive(Debug, Clone, Copy)]
struct AllowRateLimiter;

impl CapabilityRateLimiter for AllowRateLimiter {
    fn check<'a>(
        &'a self,
        _definition: &'a CapabilityDefinition,
        _request: &'a CapabilityRequest,
    ) -> PortFuture<'a, Result<RateLimitDecision, SdkError>> {
        Box::pin(async {
            Ok(RateLimitDecision {
                allowed: true,
                decision_id: "application-worker-rate-allowed".to_owned(),
                retry_after_millis: None,
            })
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct NoopApprovalVerifier;

impl CapabilityApprovalVerifier for NoopApprovalVerifier {
    fn verify<'a>(
        &'a self,
        _definition: &'a CapabilityDefinition,
        _request: &'a CapabilityRequest,
        _approval: &'a ApprovalEvidence,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async { Ok(()) })
    }
}
