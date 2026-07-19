#[path = "support/customer_enrichment_suggestion_get.rs"]
mod support;

use crm_application_runtime::{
    PRODUCTION_REVIEW_POLICY_VERSION, PostgresModuleActivation, ProductionCompositionDependencies,
    build_production_composition,
};
use crm_capability_adapters::{
    LiveAuthorizationStore, LiveCapabilityAuthorizer, LiveQueryVisibilityAuthorizer,
    LiveQueryVisibilityStore,
};
use crm_capability_ingress::{
    AccessTokenGrant, AccessTokenStore, BearerTokenAuthenticator, CapabilityIngress,
    ExecutionContextResolver, HttpCapabilityBody, HttpCapabilityMiddleware, HttpQueryMiddleware,
    QueryContextResolver, QueryIngress, TimeoutPolicy,
};
use crm_capability_runtime::{
    ApprovalEvidence, CapabilityApprovalVerifier, CapabilityDefinition, CapabilityGateway,
    CapabilityRateLimiter, CapabilityRequest, RateLimitDecision,
};
use crm_core_data::PostgresDataStore;
use crm_customer_enrichment::Suggestion;
use crm_customer_enrichment_capability_adapter::MODULE_ID;
use crm_customer_enrichment_review_adapter::{
    REJECT_SUGGESTION_CAPABILITY, reject_suggestion_capability_definition,
};
use crm_customer_enrichment_suggestion_query_adapter::get_suggestion_capability_definition;
use crm_module_sdk::testing::{DeterministicRandom, FixedClock};
use crm_module_sdk::{Clock, PortFuture, SdkError};
use crm_parties_query_adapter::{
    GET_CAPABILITY as PARTY_GET_CAPABILITY, query_capability_definition as party_query_definition,
};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;
use crm_query_runtime::QueryGateway;
use http::StatusCode;
use prost::Message;
use sqlx::PgPool;
use std::collections::BTreeSet;
use std::sync::Arc;
use support::*;

#[tokio::test(flavor = "current_thread")]
async fn production_suggestion_rejection_is_policy_bound_atomic_and_replay_safe() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping production suggestion rejection because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let store = PostgresDataStore::connect(&database_url, 6)
        .await
        .expect("connect production review store");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect production evidence reader");
    let suggestion = suggestion();
    seed_suggestion(&store, &suggestion)
        .await
        .expect("seed governed Party and suggestion evidence");
    activate_customer_enrichment(&store).await;

    let reject_definition =
        reject_suggestion_capability_definition().expect("valid rejection definition");
    let get_definition =
        get_suggestion_capability_definition().expect("valid suggestion get definition");
    let party_definition =
        party_query_definition(PARTY_GET_CAPABILITY).expect("valid Party get definition");
    let clock: Arc<dyn Clock> = Arc::new(FixedClock::new(NOW));
    let authorization_store = LiveAuthorizationStore::default();
    let reject_authorization = authorization_grant(&reject_definition);
    let get_authorization = authorization_grant(&get_definition);
    let party_authorization = authorization_grant(&party_definition);
    for grant in [
        reject_authorization.clone(),
        get_authorization,
        party_authorization.clone(),
    ] {
        authorization_store
            .upsert(grant)
            .expect("grant production review authorization");
    }

    let visibility_store = LiveQueryVisibilityStore::default();
    let party_policy_visibility = visibility_grant(
        &party_definition,
        PARTY_RECORD_TYPE,
        BTreeSet::from(["display_name".to_owned()]),
    );
    for grant in [
        party_policy_visibility.clone(),
        visibility_grant(&get_definition, PARTY_RECORD_TYPE, BTreeSet::new()),
        visibility_grant(&get_definition, SUGGESTION_RECORD_TYPE, suggestion_fields()),
        visibility_grant(&get_definition, REVIEW_RECORD_TYPE, review_fields()),
    ] {
        visibility_store
            .upsert(grant)
            .expect("grant production review visibility");
    }

    let live_authorizer = Arc::new(LiveCapabilityAuthorizer::new(
        authorization_store.clone(),
        Arc::clone(&clock),
    ));
    let composition = build_production_composition(ProductionCompositionDependencies {
        store: store.clone(),
        activation: Arc::new(PostgresModuleActivation::new(store)),
        capability_authorizer: live_authorizer.clone(),
        query_authorizer: live_authorizer,
        visibility_authorizer: Arc::new(LiveQueryVisibilityAuthorizer::new(
            visibility_store.clone(),
            Arc::clone(&clock),
        )),
        cursor_key: [0x6d; 32],
    })
    .expect("assemble production review composition");
    assert!(composition.mutation_definitions().iter().any(|candidate| {
        candidate.capability_id.as_str() == REJECT_SUGGESTION_CAPABILITY
            && candidate.capability_version.as_str() == "1.0.0"
            && candidate.owner_module_id.as_str() == MODULE_ID
            && candidate.mutation
    }));

    let capability_gateway = Arc::new(CapabilityGateway::new(
        composition.mutation_registry(),
        composition.mutation_validator(),
        Arc::new(AllowRateLimiter),
        Arc::new(NoopApprovalVerifier),
        Arc::new(LiveCapabilityAuthorizer::new(
            authorization_store.clone(),
            Arc::clone(&clock),
        )),
        composition.mutation_executor(),
        Arc::clone(&clock),
    ));
    let query_gateway = Arc::new(QueryGateway::new(
        composition.query_registry(),
        composition.query_validator(),
        Arc::new(LiveCapabilityAuthorizer::new(
            authorization_store.clone(),
            Arc::clone(&clock),
        )),
        composition.query_executor(),
    ));
    let token_store = AccessTokenStore::default();
    token_store
        .issue(
            access_token().as_bytes(),
            AccessTokenGrant {
                actor_id: actor(),
                tenant_ids: BTreeSet::from([tenant(TENANT)]),
                authentication_id: "suggestion-production-review-session".to_owned(),
                expires_at_unix_nanos: NOW + 10_000_000_000_000,
            },
        )
        .expect("issue production review token");
    let mutation_http = HttpCapabilityMiddleware::new(CapabilityIngress::new(
        Arc::new(BearerTokenAuthenticator::new(
            token_store.clone(),
            Arc::clone(&clock),
        )),
        ExecutionContextResolver::new(
            Arc::clone(&clock),
            Arc::new(DeterministicRandom::from_bytes(0_u8..=127)),
            TimeoutPolicy {
                default_millis: 5_000,
                maximum_millis: 30_000,
            },
        )
        .expect("valid mutation context resolver"),
        capability_gateway,
    ));
    let query_http = HttpQueryMiddleware::new(QueryIngress::new(
        Arc::new(BearerTokenAuthenticator::new(token_store, Arc::clone(&clock))),
        QueryContextResolver::new(
            Arc::clone(&clock),
            Arc::new(DeterministicRandom::from_bytes(128_u8..=255)),
            TimeoutPolicy {
                default_millis: 5_000,
                maximum_millis: 30_000,
            },
        )
        .expect("valid query context resolver"),
        query_gateway,
    ));
    let baseline = evidence_counts(&admin).await;

    authorization_store
        .revoke(
            &party_authorization.tenant_id,
            &party_authorization.actor_id,
            &party_authorization.policy_id,
        )
        .expect("revoke Party review authorization");
    let policy_denied = execute_mutation(
        &mutation_http,
        &reject_definition,
        &reject_request(&suggestion, 7),
        TENANT,
        "suggestion-production-reject-policy-denied",
    )
    .await;
    assert_eq!(policy_denied.status, StatusCode::FORBIDDEN);
    assert_mutation_error_code(
        policy_denied.body,
        "CUSTOMER_ENRICHMENT_SUGGESTION_REVIEW_POLICY_DENIED",
    );
    authorization_store
        .upsert(party_authorization)
        .expect("restore Party review authorization");

    visibility_store
        .revoke(&party_policy_visibility)
        .expect("revoke Party review visibility");
    let hidden_party = execute_mutation(
        &mutation_http,
        &reject_definition,
        &reject_request(&suggestion, 7),
        TENANT,
        "suggestion-production-reject-hidden-party",
    )
    .await;
    assert_eq!(hidden_party.status, StatusCode::FORBIDDEN);
    assert_mutation_error_code(
        hidden_party.body,
        "CUSTOMER_ENRICHMENT_SUGGESTION_REVIEW_POLICY_DENIED",
    );
    visibility_store
        .upsert(party_policy_visibility)
        .expect("restore Party review visibility");

    let stale = execute_mutation(
        &mutation_http,
        &reject_definition,
        &reject_request(&suggestion, 6),
        TENANT,
        "suggestion-production-reject-stale",
    )
    .await;
    assert_eq!(stale.status, StatusCode::CONFLICT);
    assert_mutation_error_code(stale.body, "CUSTOMER_ENRICHMENT_REVIEW_CONFLICT");

    let cross_tenant = execute_mutation(
        &mutation_http,
        &reject_definition,
        &reject_request(&suggestion, 7),
        OTHER_TENANT,
        "suggestion-production-reject-cross-tenant",
    )
    .await;
    assert_eq!(cross_tenant.status, StatusCode::FORBIDDEN);
    assert_mutation_error_code(cross_tenant.body, "AUTHENTICATION_TENANT_FORBIDDEN");

    for (status, key) in [
        ("suspended", "suggestion-production-reject-suspended"),
        (
            "uninstalling",
            "suggestion-production-reject-uninstalling",
        ),
    ] {
        set_installation_status(&admin, status).await;
        let inactive = execute_mutation(
            &mutation_http,
            &reject_definition,
            &reject_request(&suggestion, 7),
            TENANT,
            key,
        )
        .await;
        assert_eq!(inactive.status, StatusCode::CONFLICT);
        assert_mutation_error_code(inactive.body, "MODULE_NOT_ACTIVE");
    }
    set_installation_status(&admin, "active").await;
    assert_eq!(evidence_counts(&admin).await, baseline);

    let success = execute_mutation(
        &mutation_http,
        &reject_definition,
        &reject_request(&suggestion, 7),
        TENANT,
        "suggestion-production-reject-success",
    )
    .await;
    assert_eq!(success.status, StatusCode::OK);
    let response = wire::RejectSuggestionResponse::decode(
        success_mutation_payload(success.body).bytes.as_slice(),
    )
    .expect("decode production rejection response");
    let review = response.review_decision.expect("persisted review decision");
    assert_eq!(
        review.kind,
        wire::SuggestionReviewDecisionKind::Rejected as i32
    );
    assert_eq!(review.policy_version, PRODUCTION_REVIEW_POLICY_VERSION);
    assert_eq!(review.safe_reason_code, "incorrect_provider_value");
    assert_eq!(
        response
            .suggestion
            .expect("rejected suggestion")
            .lifecycle_status,
        wire::SuggestionLifecycleStatus::Rejected as i32
    );
    let after_success = evidence_counts(&admin).await;
    assert_ne!(after_success, baseline);

    let replay = execute_mutation(
        &mutation_http,
        &reject_definition,
        &reject_request(&suggestion, 7),
        TENANT,
        "suggestion-production-reject-success",
    )
    .await;
    assert_eq!(replay.status, StatusCode::OK);
    match replay.body {
        HttpCapabilityBody::Success(result) => assert!(result.replayed),
        HttpCapabilityBody::Error(error) => panic!("expected replay success, got {error:?}"),
    }
    assert_eq!(evidence_counts(&admin).await, after_success);

    let read_after_review = execute_get(&query_http, &get_definition, &suggestion, TENANT).await;
    assert_eq!(read_after_review.status, StatusCode::OK);
    let read_after_review = wire::GetSuggestionResponse::decode(
        success_payload(read_after_review.body).bytes.as_slice(),
    )
    .expect("decode reviewed suggestion query response");
    assert_eq!(
        read_after_review
            .suggestion
            .expect("reviewed suggestion")
            .lifecycle_status,
        wire::SuggestionLifecycleStatus::Rejected as i32
    );
    assert_eq!(
        read_after_review
            .latest_review_decision
            .expect("latest rejection")
            .kind,
        wire::SuggestionReviewDecisionKind::Rejected as i32
    );
    assert_eq!(evidence_counts(&admin).await, after_success);

    authorization_store
        .revoke(
            &reject_authorization.tenant_id,
            &reject_authorization.actor_id,
            &reject_authorization.policy_id,
        )
        .expect("revoke top-level rejection authorization");
    let top_level_denied = execute_mutation(
        &mutation_http,
        &reject_definition,
        &reject_request(&suggestion, 7),
        TENANT,
        "suggestion-production-reject-top-level-denied",
    )
    .await;
    assert_eq!(top_level_denied.status, StatusCode::FORBIDDEN);
    assert_mutation_error_code(top_level_denied.body, "CAPABILITY_PERMISSION_DENIED");
    assert_eq!(evidence_counts(&admin).await, after_success);
}

fn reject_request(suggestion: &Suggestion, expected_version: i64) -> wire::RejectSuggestionRequest {
    wire::RejectSuggestionRequest {
        suggestion_ref: Some(wire::SuggestionRef {
            suggestion_id: suggestion.suggestion_id().as_str().to_owned(),
        }),
        expected_party_resource_version: expected_version,
        expected_proposed_value_digest: suggestion.proposed_value_digest().to_vec(),
        policy_version: PRODUCTION_REVIEW_POLICY_VERSION.to_owned(),
        safe_reason_code: "incorrect_provider_value".to_owned(),
    }
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
                decision_id: "suggestion-review-rate-allowed".to_owned(),
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
