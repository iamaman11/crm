#[path = "support/customer_enrichment_suggestion_get.rs"]
mod support;

use crm_application_runtime::{
    PostgresModuleActivation, ProductionCompositionDependencies, build_production_composition,
};
use crm_capability_adapters::{
    LiveAuthorizationStore, LiveCapabilityAuthorizer, LiveQueryVisibilityAuthorizer,
    LiveQueryVisibilityStore,
};
use crm_capability_ingress::{
    AccessTokenGrant, AccessTokenStore, BearerTokenAuthenticator, HttpQueryMiddleware,
    QueryContextResolver, QueryIngress, TimeoutPolicy,
};
use crm_core_data::PostgresDataStore;
use crm_customer_enrichment_capability_adapter::MODULE_ID;
use crm_customer_enrichment_suggestion_query_adapter::{
    GET_SUGGESTION_CAPABILITY, get_suggestion_capability_definition,
};
use crm_module_sdk::Clock;
use crm_module_sdk::testing::{DeterministicRandom, FixedClock};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;
use crm_query_runtime::QueryGateway;
use http::StatusCode;
use prost::Message;
use sqlx::PgPool;
use std::collections::BTreeSet;
use std::sync::Arc;
use support::*;

#[tokio::test(flavor = "current_thread")]
async fn production_suggestion_get_is_activation_gated_permission_aware_and_side_effect_free() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping production suggestion query because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let store = PostgresDataStore::connect(&database_url, 6)
        .await
        .expect("connect production query store");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect production evidence reader");
    let suggestion = suggestion();
    seed_suggestion(&store, &suggestion)
        .await
        .expect("seed strict suggestion evidence");
    activate_customer_enrichment(&store).await;

    let definition =
        get_suggestion_capability_definition().expect("valid suggestion get definition");
    let clock: Arc<dyn Clock> = Arc::new(FixedClock::new(NOW));
    let authorization_store = LiveAuthorizationStore::default();
    let authorization_grant = authorization_grant(&definition);
    authorization_store
        .upsert(authorization_grant.clone())
        .expect("grant suggestion query authorization");

    let visibility_store = LiveQueryVisibilityStore::default();
    let party_visibility = visibility_grant(&definition, PARTY_RECORD_TYPE, BTreeSet::new());
    let suggestion_visibility =
        visibility_grant(&definition, SUGGESTION_RECORD_TYPE, suggestion_fields());
    for grant in [
        party_visibility.clone(),
        suggestion_visibility.clone(),
        visibility_grant(&definition, REVIEW_RECORD_TYPE, review_fields()),
    ] {
        visibility_store
            .upsert(grant)
            .expect("grant suggestion visibility");
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
    .expect("assemble exact production composition");
    assert!(composition.query_definitions().iter().any(|candidate| {
        candidate.capability_id.as_str() == GET_SUGGESTION_CAPABILITY
            && candidate.capability_version.as_str() == "1.0.0"
            && candidate.owner_module_id.as_str() == MODULE_ID
            && !candidate.mutation
    }));

    let gateway = Arc::new(QueryGateway::new(
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
                authentication_id: "suggestion-production-session".to_owned(),
                expires_at_unix_nanos: NOW + 10_000_000_000_000,
            },
        )
        .expect("issue suggestion query access token");
    let http = HttpQueryMiddleware::new(QueryIngress::new(
        Arc::new(BearerTokenAuthenticator::new(
            token_store,
            Arc::clone(&clock),
        )),
        QueryContextResolver::new(
            Arc::clone(&clock),
            Arc::new(DeterministicRandom::from_bytes(0_u8..=127)),
            TimeoutPolicy {
                default_millis: 5_000,
                maximum_millis: 30_000,
            },
        )
        .expect("valid query context resolver"),
        gateway,
    ));
    let baseline = evidence_counts(&admin).await;

    let success = execute_get(&http, &definition, &suggestion, TENANT).await;
    assert_eq!(success.status, StatusCode::OK);
    let response =
        wire::GetSuggestionResponse::decode(success_payload(success.body).bytes.as_slice())
            .expect("decode production suggestion response");
    let public = response.suggestion.expect("production suggestion");
    assert_eq!(public.proposed_value, "Production Company");
    assert_eq!(
        public.lifecycle_status,
        wire::SuggestionLifecycleStatus::Proposed as i32
    );
    assert!(response.latest_review_decision.is_none());
    assert!(response.latest_application_attempt.is_none());

    visibility_store
        .revoke(&suggestion_visibility)
        .expect("revoke suggestion visibility");
    let hidden = execute_get(&http, &definition, &suggestion, TENANT).await;
    assert_eq!(hidden.status, StatusCode::NOT_FOUND);
    assert_error_code(hidden.body, "CUSTOMER_ENRICHMENT_SUGGESTION_NOT_FOUND");

    let redacted_visibility = visibility_grant(
        &definition,
        SUGGESTION_RECORD_TYPE,
        BTreeSet::from(["lifecycle_status".to_owned()]),
    );
    visibility_store
        .upsert(redacted_visibility.clone())
        .expect("grant redacted suggestion visibility");
    let redacted = execute_get(&http, &definition, &suggestion, TENANT).await;
    assert_eq!(redacted.status, StatusCode::OK);
    let redacted =
        wire::GetSuggestionResponse::decode(success_payload(redacted.body).bytes.as_slice())
            .expect("decode redacted suggestion response")
            .suggestion
            .expect("redacted suggestion");
    assert!(redacted.proposed_value.is_empty());
    assert_eq!(
        redacted.lifecycle_status,
        wire::SuggestionLifecycleStatus::Proposed as i32
    );
    visibility_store
        .revoke(&redacted_visibility)
        .expect("remove redacted visibility");
    visibility_store
        .upsert(suggestion_visibility)
        .expect("restore full suggestion visibility");

    visibility_store
        .revoke(&party_visibility)
        .expect("revoke Party visibility");
    let hidden_party = execute_get(&http, &definition, &suggestion, TENANT).await;
    assert_eq!(hidden_party.status, StatusCode::NOT_FOUND);
    assert_error_code(
        hidden_party.body,
        "CUSTOMER_ENRICHMENT_SUGGESTION_NOT_FOUND",
    );
    visibility_store
        .upsert(party_visibility)
        .expect("restore Party visibility");

    authorization_store
        .revoke(
            &authorization_grant.tenant_id,
            &authorization_grant.actor_id,
            &authorization_grant.policy_id,
        )
        .expect("revoke query authorization");
    let denied = execute_get(&http, &definition, &suggestion, TENANT).await;
    assert_eq!(denied.status, StatusCode::FORBIDDEN);
    assert_error_code(denied.body, "QUERY_PERMISSION_DENIED");
    authorization_store
        .upsert(authorization_grant)
        .expect("restore query authorization");

    let cross_tenant = execute_get(&http, &definition, &suggestion, OTHER_TENANT).await;
    assert_eq!(cross_tenant.status, StatusCode::FORBIDDEN);
    assert_error_code(cross_tenant.body, "AUTHENTICATION_TENANT_FORBIDDEN");

    for status in ["suspended", "uninstalling"] {
        set_installation_status(&admin, status).await;
        let inactive = execute_get(&http, &definition, &suggestion, TENANT).await;
        assert_eq!(inactive.status, StatusCode::CONFLICT);
        assert_error_code(inactive.body, "MODULE_NOT_ACTIVE");
    }
    set_installation_status(&admin, "active").await;
    assert_eq!(evidence_counts(&admin).await, baseline);
}
