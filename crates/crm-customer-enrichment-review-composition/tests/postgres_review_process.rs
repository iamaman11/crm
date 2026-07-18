mod support;

use crm_capability_ingress::semantic_input_hash;
use crm_capability_plan_support as plan_support;
use crm_core_data::PostgresDataStore;
use crm_customer_enrichment::{
    ApprovalRequirement, ReviewDecisionKind, SuggestionReviewPolicyDecision,
    SuggestionReviewPolicyPort, SuggestionReviewPolicyRequest,
};
use crm_customer_enrichment_capability_adapter::MODULE_ID;
use crm_customer_enrichment_review_composition::PostgresCustomerEnrichmentSuggestionReviewExecutor;
use crm_customer_enrichment_suggestion_query_adapter::{
    CustomerEnrichmentSuggestionQueryAdapter, GET_SUGGESTION_REQUEST_SCHEMA,
    LIST_SUGGESTIONS_BY_PARTY_REQUEST_SCHEMA, get_suggestion_capability_definition,
    list_suggestions_by_party_capability_definition,
};
use crm_module_sdk::{
    ActorId, CapabilityId, CapabilityVersion, CorrelationId, DataClass, ModuleId, PortFuture,
    RecordRef, RequestId, SchemaVersion, SdkError, TenantId, TraceId,
};
use crm_proto_contracts::crm::{customer::v1::PartyRef, customer_enrichment::v1 as wire};
use crm_query_runtime::{
    CursorCodec, QueryExecutionContext, QueryExecutor, QueryRequest, QuerySemanticValidator,
    QueryVisibilityAuthorizer, QueryVisibilityDecision,
};
use prost::Message;
use sqlx::PgPool;
use std::collections::BTreeSet;
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

#[derive(Debug)]
struct ProcessVisibility {
    hide_party: bool,
}

impl QueryVisibilityAuthorizer for ProcessVisibility {
    fn authorize_visibility<'a>(
        &'a self,
        _request: &'a QueryRequest,
        resource: &'a RecordRef,
    ) -> PortFuture<'a, Result<QueryVisibilityDecision, SdkError>> {
        Box::pin(async move {
            if self.hide_party && resource.record_type.as_str() == "parties.party" {
                return Ok(QueryVisibilityDecision::denied(
                    "visibility-party-hidden",
                    "visibility-v1",
                ));
            }
            let allowed_fields = match resource.record_type.as_str() {
                "customer_enrichment.suggestion" => suggestion_fields(),
                "customer_enrichment.review_decision" => review_fields(),
                _ => BTreeSet::new(),
            };
            Ok(QueryVisibilityDecision {
                resource_visible: true,
                allowed_fields,
                decision_id: format!("visibility-{}", resource.record_id.as_str()),
                policy_version: "visibility-v1".to_owned(),
            })
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn postgres_review_and_permission_aware_queries_are_replay_safe() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping PostgreSQL review process because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let store = PostgresDataStore::connect(&database_url, 6)
        .await
        .expect("connect review store");
    let query_store = store.clone();
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect review evidence reader");

    let suggestion = suggestion();
    seed_suggestion(&store, &suggestion)
        .await
        .expect("seed immutable suggestion");
    let request = accept_request(&suggestion);
    let executor =
        PostgresCustomerEnrichmentSuggestionReviewExecutor::new(store, Arc::new(ExactAllowPolicy));

    let first = executor
        .execute(request.clone())
        .await
        .expect("commit policy-bound review");
    assert!(!first.replayed);
    let first_output =
        wire::AcceptSuggestionResponse::decode(first.output.as_ref().unwrap().bytes.as_slice())
            .unwrap();
    let accepted_suggestion = first_output.suggestion.unwrap();
    assert_eq!(
        accepted_suggestion.lifecycle_status,
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
    let second_output =
        wire::AcceptSuggestionResponse::decode(second.output.as_ref().unwrap().bytes.as_slice())
            .unwrap();
    assert_eq!(
        second_output.review_decision.unwrap().review_decision_ref,
        first_decision.review_decision_ref
    );

    let visible_queries = CustomerEnrichmentSuggestionQueryAdapter::new(
        query_store.clone(),
        CursorCodec::new([91; 32]),
        Arc::new(ProcessVisibility { hide_party: false }),
    );
    let get_definition = get_suggestion_capability_definition().unwrap();
    let get_request = query_request(
        &get_definition.capability_id,
        GET_SUGGESTION_REQUEST_SCHEMA,
        &wire::GetSuggestionRequest {
            suggestion_ref: accepted_suggestion.suggestion_ref.clone(),
        },
        "review-get-request",
    );
    visible_queries
        .validate(&get_definition, &get_request)
        .await
        .unwrap();
    let get_result = visible_queries
        .execute(&get_definition, get_request.clone())
        .await
        .unwrap();
    let get_output =
        wire::GetSuggestionResponse::decode(get_result.output.bytes.as_slice()).unwrap();
    assert_eq!(
        get_output.suggestion.unwrap().lifecycle_status,
        wire::SuggestionLifecycleStatus::Accepted as i32
    );
    assert_eq!(
        get_output
            .latest_review_decision
            .unwrap()
            .review_decision_ref,
        first_decision.review_decision_ref
    );
    assert!(get_output.latest_application_attempt.is_none());

    let list_definition = list_suggestions_by_party_capability_definition().unwrap();
    let list_request = query_request(
        &list_definition.capability_id,
        LIST_SUGGESTIONS_BY_PARTY_REQUEST_SCHEMA,
        &wire::ListSuggestionsByPartyRequest {
            party_ref: Some(PartyRef {
                party_id: "party-review-1".to_owned(),
            }),
            provider_profile_version_ref: accepted_suggestion.provider_profile_version_ref.clone(),
            status: Some(wire::SuggestionLifecycleStatus::Accepted as i32),
            page_size: 10,
            cursor: String::new(),
        },
        "review-list-request",
    );
    visible_queries
        .validate(&list_definition, &list_request)
        .await
        .unwrap();
    let list_result = visible_queries
        .execute(&list_definition, list_request.clone())
        .await
        .unwrap();
    let list_output =
        wire::ListSuggestionsByPartyResponse::decode(list_result.output.bytes.as_slice()).unwrap();
    assert_eq!(list_output.suggestions.len(), 1);
    assert!(list_output.next_cursor.is_empty());

    let hidden_queries = CustomerEnrichmentSuggestionQueryAdapter::new(
        query_store,
        CursorCodec::new([92; 32]),
        Arc::new(ProcessVisibility { hide_party: true }),
    );
    let hidden_get = hidden_queries
        .execute(&get_definition, get_request)
        .await
        .unwrap_err();
    assert_eq!(hidden_get.code, "CUSTOMER_ENRICHMENT_SUGGESTION_NOT_FOUND");
    let hidden_list = hidden_queries
        .execute(&list_definition, list_request)
        .await
        .unwrap();
    let hidden_list_output =
        wire::ListSuggestionsByPartyResponse::decode(hidden_list.output.bytes.as_slice()).unwrap();
    assert!(hidden_list_output.suggestions.is_empty());
    assert!(hidden_list_output.next_cursor.is_empty());

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

fn query_request<M: Message>(
    capability_id: &CapabilityId,
    schema: &'static str,
    message: &M,
    request_id: &str,
) -> QueryRequest {
    let input =
        plan_support::protobuf_payload(MODULE_ID, schema, DataClass::Personal, message).unwrap();
    QueryRequest {
        owner_module_id: ModuleId::try_new(MODULE_ID).unwrap(),
        context: QueryExecutionContext {
            tenant_id: TenantId::try_new("tenant-a").unwrap(),
            actor_id: ActorId::try_new("reviewer-a").unwrap(),
            request_id: RequestId::try_new(request_id).unwrap(),
            correlation_id: CorrelationId::try_new(format!("correlation-{request_id}")).unwrap(),
            trace_id: TraceId::try_new(format!("trace-{request_id}")).unwrap(),
            capability_id: capability_id.clone(),
            capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
            request_started_at_unix_nanos: 50_000_000,
        },
        input_hash: semantic_input_hash(&input),
        input,
    }
}

fn suggestion_fields() -> BTreeSet<String> {
    [
        "enrichment_request_ref",
        "provider_response_receipt_ref",
        "provider_profile_version_ref",
        "mapping_version_ref",
        "target",
        "proposed_value",
        "proposed_value_digest",
        "observed_at_unix_ms",
        "retrieved_at_unix_ms",
        "effective_at_unix_ms",
        "fresh_until_unix_ms",
        "expires_at_unix_ms",
        "confidence_basis_points",
        "policy_evidence",
        "evidence_references",
        "lifecycle_status",
        "superseded_by_suggestion_ref",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn review_fields() -> BTreeSet<String> {
    [
        "suggestion_ref",
        "target_party_resource_version",
        "proposed_value_digest",
        "reviewed_by_actor_id",
        "kind",
        "policy_version",
        "safe_reason_code",
        "approval_evidence_reference",
        "decided_at_unix_ms",
        "expires_at_unix_ms",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

async fn scalar(pool: &PgPool, query: &'static str) -> i64 {
    sqlx::query_scalar::<_, i64>(query)
        .fetch_one(pool)
        .await
        .expect("query review evidence")
}
