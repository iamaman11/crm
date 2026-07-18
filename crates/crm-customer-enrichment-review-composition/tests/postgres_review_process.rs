mod support;

use crm_customer_enrichment::{
    ApprovalRequirement, ReviewDecisionKind, SuggestionReviewPolicyDecision,
    SuggestionReviewPolicyPort, SuggestionReviewPolicyRequest,
};
use crm_customer_enrichment_review_composition::PostgresCustomerEnrichmentSuggestionReviewExecutor;
use crm_core_data::PostgresDataStore;
use crm_module_sdk::{PortFuture, SdkError};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;
use prost::Message;
use sqlx::PgPool;
use std::sync::Arc;
use support::{accept_request, seed_suggestion, suggestion};

#[derive(Debug)]
struct ExactAllowPolicy;

impl SuggestionReviewPolicyPort for ExactAllowPolicy {
    fn evaluate<'a>(
        &'a self,
        request: SuggestionReviewPolicyRequest,
    ) -> PortFuture<'a, Result<SuggestionReviewPolicyDecision, SdkError>> {
        Box::pin(async move {
            assert_eq!(request.tenant_id.as_str(), "tenant-a");
            assert_eq!(request.actor_id.as_str(), "reviewer-a");
            assert_eq!(request.party_id.as_str(), "party-review-1");
            assert_eq!(request.party_resource_version, 7);
            assert_eq!(request.decision_kind, ReviewDecisionKind::Accepted);
            assert_eq!(request.purpose_code, "customer_profile_enrichment");
            assert_eq!(request.legal_basis_code, "legitimate_interest");
            assert_eq!(
                request.consent_evidence_reference.as_deref(),
                Some("consent-review-1")
            );
            assert_eq!(request.evaluated_at_unix_ms, 40);
            Ok(SuggestionReviewPolicyDecision::Allowed {
                decision_id: "review-policy-decision-1".to_owned(),
                policy_version: "review-policy-v1".to_owned(),
                acceptance_approval_requirement: ApprovalRequirement::Required,
            })
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn postgres_review_is_policy_bound_atomic_and_replay_safe() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping PostgreSQL review process because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let store = PostgresDataStore::connect(&database_url, 6)
        .await
        .expect("connect review store");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect review evidence reader");

    let suggestion = suggestion();
    seed_suggestion(&store, &suggestion)
        .await
        .expect("seed immutable suggestion");
    let request = accept_request(&suggestion);
    let executor = PostgresCustomerEnrichmentSuggestionReviewExecutor::new(
        store,
        Arc::new(ExactAllowPolicy),
    );

    let first = executor
        .execute(request.clone())
        .await
        .expect("commit policy-bound review");
    assert!(!first.replayed);
    let first_output = wire::AcceptSuggestionResponse::decode(
        first.output.as_ref().unwrap().bytes.as_slice(),
    )
    .unwrap();
    assert_eq!(
        first_output.suggestion.unwrap().lifecycle_status,
        wire::SuggestionLifecycleStatus::Accepted as i32
    );
    let first_decision = first_output.review_decision.unwrap();
    assert_eq!(
        first_decision.kind,
        wire::SuggestionReviewDecisionKind::Accepted as i32
    );
    assert_eq!(first_decision.policy_version, "review-policy-v1");
    assert_eq!(
        first_decision.approval_evidence_reference.as_deref(),
        Some("approval-review-1")
    );

    let second = executor
        .execute(request)
        .await
        .expect("replay exact policy-bound review");
    assert!(second.replayed);
    let second_output = wire::AcceptSuggestionResponse::decode(
        second.output.as_ref().unwrap().bytes.as_slice(),
    )
    .unwrap();
    assert_eq!(
        second_output.review_decision.unwrap().review_decision_ref,
        first_decision.review_decision_ref
    );

    assert_eq!(
        scalar(
            &admin,
            "SELECT count(*)::bigint FROM crm.records WHERE tenant_id = 'tenant-a' AND owner_module_id = 'crm.customer-enrichment'",
        )
        .await,
        2
    );
    assert_eq!(
        scalar(
            &admin,
            "SELECT count(*)::bigint FROM crm.records WHERE tenant_id = 'tenant-a' AND record_type = 'customer_enrichment.review_decision'",
        )
        .await,
        1
    );
    assert_eq!(
        scalar(
            &admin,
            "SELECT count(*)::bigint FROM crm.outbox_events WHERE tenant_id = 'tenant-a' AND event_type LIKE 'customer_enrichment.%'",
        )
        .await,
        2
    );
    assert_eq!(
        scalar(
            &admin,
            "SELECT count(*)::bigint FROM crm.audit_records WHERE tenant_id = 'tenant-a' AND capability_id LIKE 'customer_enrichment.%'",
        )
        .await,
        2
    );
    assert_eq!(
        scalar(
            &admin,
            "SELECT count(*)::bigint FROM crm.idempotency_records WHERE tenant_id = 'tenant-a' AND (idempotency_scope = 'customer_enrichment.review.seed@1.0.0' OR idempotency_scope = 'capability:customer_enrichment.suggestion.accept:1.0.0')",
        )
        .await,
        2
    );
    assert_eq!(
        scalar(
            &admin,
            "SELECT count(*)::bigint FROM crm.business_transactions WHERE tenant_id = 'tenant-a' AND capability_id IN ('customer_enrichment.review.seed', 'customer_enrichment.suggestion.accept')",
        )
        .await,
        2
    );
}

async fn scalar(pool: &PgPool, query: &'static str) -> i64 {
    sqlx::query_scalar::<_, i64>(query)
        .fetch_one(pool)
        .await
        .expect("query review evidence")
}
