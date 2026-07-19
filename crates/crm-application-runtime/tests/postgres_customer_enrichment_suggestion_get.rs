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
    GET_SUGGESTION_CAPABILITY, LIST_SUGGESTIONS_BY_PARTY_CAPABILITY,
    get_suggestion_capability_definition, list_suggestions_by_party_capability_definition,
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
async fn production_suggestion_queries_are_activation_gated_permission_aware_and_side_effect_free()
{
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping production suggestion queries because DATABASE_URL is absent");
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

    let get_definition =
        get_suggestion_capability_definition().expect("valid suggestion get definition");
    let list_definition = list_suggestions_by_party_capability_definition()
        .expect("valid suggestion list definition");
    let clock: Arc<dyn Clock> = Arc::new(FixedClock::new(NOW));
    let authorization_store = LiveAuthorizationStore::default();
    let get_authorization = authorization_grant(&get_definition);
    let list_authorization = authorization_grant(&list_definition);
    for grant in [get_authorization.clone(), list_authorization.clone()] {
        authorization_store
            .upsert(grant)
            .expect("grant suggestion query authorization");
    }

    let visibility_store = LiveQueryVisibilityStore::default();
    let get_party_visibility =
        visibility_grant(&get_definition, PARTY_RECORD_TYPE, BTreeSet::new());
    let get_suggestion_visibility =
        visibility_grant(&get_definition, SUGGESTION_RECORD_TYPE, suggestion_fields());
    let list_party_visibility =
        visibility_grant(&list_definition, PARTY_RECORD_TYPE, BTreeSet::new());
    let list_suggestion_visibility = visibility_grant(
        &list_definition,
        SUGGESTION_RECORD_TYPE,
        suggestion_fields(),
    );
    for grant in [
        get_party_visibility.clone(),
        get_suggestion_visibility.clone(),
        visibility_grant(&get_definition, REVIEW_RECORD_TYPE, review_fields()),
        list_party_visibility.clone(),
        list_suggestion_visibility.clone(),
        visibility_grant(&list_definition, REVIEW_RECORD_TYPE, review_fields()),
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
    for capability_id in [
        GET_SUGGESTION_CAPABILITY,
        LIST_SUGGESTIONS_BY_PARTY_CAPABILITY,
    ] {
        assert!(composition.query_definitions().iter().any(|candidate| {
            candidate.capability_id.as_str() == capability_id
                && candidate.capability_version.as_str() == "1.0.0"
                && candidate.owner_module_id.as_str() == MODULE_ID
                && !candidate.mutation
        }));
    }

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

    let get_success = execute_get(&http, &get_definition, &suggestion, TENANT).await;
    assert_eq!(get_success.status, StatusCode::OK);
    let get_response = wire::GetSuggestionResponse::decode(
        success_payload(get_success.body).bytes.as_slice(),
    )
    .expect("decode production suggestion response");
    let public = get_response.suggestion.expect("production suggestion");
    assert_eq!(public.proposed_value, "Production Company");
    assert_eq!(
        public.lifecycle_status,
        wire::SuggestionLifecycleStatus::Proposed as i32
    );
    assert!(get_response.latest_review_decision.is_none());
    assert!(get_response.latest_application_attempt.is_none());

    let list_success = execute_query(
        &http,
        &list_definition,
        &list_request(&suggestion, 25, ""),
        TENANT,
    )
    .await;
    assert_eq!(list_success.status, StatusCode::OK);
    let list_response = wire::ListSuggestionsByPartyResponse::decode(
        success_payload(list_success.body).bytes.as_slice(),
    )
    .expect("decode production suggestion list response");
    assert_eq!(list_response.suggestions.len(), 1);
    assert_eq!(
        list_response.suggestions[0]
            .suggestion_ref
            .as_ref()
            .expect("listed suggestion reference")
            .suggestion_id,
        suggestion.suggestion_id().as_str()
    );
    assert!(list_response.next_cursor.is_empty());

    visibility_store
        .revoke(&list_suggestion_visibility)
        .expect("revoke list suggestion visibility");
    let list_redacted_visibility = visibility_grant(
        &list_definition,
        SUGGESTION_RECORD_TYPE,
        BTreeSet::from(["lifecycle_status".to_owned()]),
    );
    visibility_store
        .upsert(list_redacted_visibility.clone())
        .expect("grant redacted list visibility");
    let list_redacted = execute_query(
        &http,
        &list_definition,
        &list_request(&suggestion, 25, ""),
        TENANT,
    )
    .await;
    assert_eq!(list_redacted.status, StatusCode::OK);
    let list_redacted = wire::ListSuggestionsByPartyResponse::decode(
        success_payload(list_redacted.body).bytes.as_slice(),
    )
    .expect("decode redacted suggestion list response");
    assert_eq!(list_redacted.suggestions.len(), 1);
    assert!(list_redacted.suggestions[0].proposed_value.is_empty());
    assert_eq!(
        list_redacted.suggestions[0].lifecycle_status,
        wire::SuggestionLifecycleStatus::Proposed as i32
    );
    visibility_store
        .revoke(&list_redacted_visibility)
        .expect("remove redacted list visibility");
    visibility_store
        .upsert(list_suggestion_visibility.clone())
        .expect("restore full list visibility");

    visibility_store
        .revoke(&list_party_visibility)
        .expect("revoke list Party visibility");
    let hidden_party = execute_query(
        &http,
        &list_definition,
        &list_request(&suggestion, 25, ""),
        TENANT,
    )
    .await;
    assert_eq!(hidden_party.status, StatusCode::OK);
    let hidden_party = wire::ListSuggestionsByPartyResponse::decode(
        success_payload(hidden_party.body).bytes.as_slice(),
    )
    .expect("decode hidden Party list response");
    assert!(hidden_party.suggestions.is_empty());
    assert!(hidden_party.next_cursor.is_empty());
    visibility_store
        .upsert(list_party_visibility.clone())
        .expect("restore list Party visibility");

    let tampered_cursor = execute_query(
        &http,
        &list_definition,
        &list_request(&suggestion, 25, "tampered-cursor"),
        TENANT,
    )
    .await;
    assert_eq!(tampered_cursor.status, StatusCode::BAD_REQUEST);
    assert_error_code(
        tampered_cursor.body,
        "CUSTOMER_ENRICHMENT_SUGGESTION_LIST_CURSOR_INVALID",
    );

    authorization_store
        .revoke(
            &list_authorization.tenant_id,
            &list_authorization.actor_id,
            &list_authorization.policy_id,
        )
        .expect("revoke list authorization");
    let list_denied = execute_query(
        &http,
        &list_definition,
        &list_request(&suggestion, 25, ""),
        TENANT,
    )
    .await;
    assert_eq!(list_denied.status, StatusCode::FORBIDDEN);
    assert_error_code(list_denied.body, "QUERY_PERMISSION_DENIED");
    authorization_store
        .upsert(list_authorization)
        .expect("restore list authorization");

    let list_cross_tenant = execute_query(
        &http,
        &list_definition,
        &list_request(&suggestion, 25, ""),
        OTHER_TENANT,
    )
    .await;
    assert_eq!(list_cross_tenant.status, StatusCode::FORBIDDEN);
    assert_error_code(
        list_cross_tenant.body,
        "AUTHENTICATION_TENANT_FORBIDDEN",
    );

    for status in ["suspended", "uninstalling"] {
        set_installation_status(&admin, status).await;
        let inactive = execute_query(
            &http,
            &list_definition,
            &list_request(&suggestion, 25, ""),
            TENANT,
        )
        .await;
        assert_eq!(inactive.status, StatusCode::CONFLICT);
        assert_error_code(inactive.body, "MODULE_NOT_ACTIVE");
    }
    set_installation_status(&admin, "active").await;

    visibility_store
        .revoke(&get_suggestion_visibility)
        .expect("revoke get suggestion visibility");
    let hidden_get = execute_get(&http, &get_definition, &suggestion, TENANT).await;
    assert_eq!(hidden_get.status, StatusCode::NOT_FOUND);
    assert_error_code(
        hidden_get.body,
        "CUSTOMER_ENRICHMENT_SUGGESTION_NOT_FOUND",
    );
    visibility_store
        .upsert(get_suggestion_visibility)
        .expect("restore get suggestion visibility");

    visibility_store
        .revoke(&get_party_visibility)
        .expect("revoke get Party visibility");
    let hidden_get_party = execute_get(&http, &get_definition, &suggestion, TENANT).await;
    assert_eq!(hidden_get_party.status, StatusCode::NOT_FOUND);
    assert_error_code(
        hidden_get_party.body,
        "CUSTOMER_ENRICHMENT_SUGGESTION_NOT_FOUND",
    );
    visibility_store
        .upsert(get_party_visibility)
        .expect("restore get Party visibility");

    authorization_store
        .revoke(
            &get_authorization.tenant_id,
            &get_authorization.actor_id,
            &get_authorization.policy_id,
        )
        .expect("revoke get authorization");
    let get_denied = execute_get(&http, &get_definition, &suggestion, TENANT).await;
    assert_eq!(get_denied.status, StatusCode::FORBIDDEN);
    assert_error_code(get_denied.body, "QUERY_PERMISSION_DENIED");
    authorization_store
        .upsert(get_authorization)
        .expect("restore get authorization");

    assert_eq!(evidence_counts(&admin).await, baseline);
}
