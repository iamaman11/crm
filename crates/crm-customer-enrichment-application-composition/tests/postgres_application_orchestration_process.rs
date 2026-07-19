use crm_capability_ingress::semantic_input_hash;
use crm_capability_plan_support as support;
use crm_capability_runtime::CapabilityRequest;
use crm_core_data::{AuditIntent, IdempotencyEvidence, PostgresDataStore, RecordCreatePlan};
use crm_customer_enrichment::{
    ApplicationAttempt, ApprovalRequirement, EnrichmentPolicyDecision, EnrichmentPolicyPort,
    EnrichmentPolicyRequest, EnrichmentRequest, EnrichmentRequestDraft, MappingDraft,
    MappingNormalization, MappingVersion, PartyDisplayNameApplicationPort,
    PartyDisplayNameApplicationRequest, PartyDisplayNameApplicationResult, PolicyEvaluationPhase,
    ProviderProfileDraft, ProviderProfileVersion, ProviderResponseClass, ProviderResponseReceipt,
    ProviderResponseReceiptDraft, RawPayloadPolicy, RequestPolicyEvidence, ReviewDecision,
    ReviewDecisionKind, Suggestion, SuggestionDraft, TargetField, TargetSnapshot,
};
use crm_customer_enrichment_application_adapter::{
    APPLY_PARTY_DISPLAY_NAME_REQUEST_SCHEMA, apply_party_display_name_capability_definition,
};
use crm_customer_enrichment_application_composition::{
    CustomerEnrichmentPartyApplicationOrchestrator,
    PostgresCustomerEnrichmentApplicationAttemptExecutor,
};
use crm_customer_enrichment_capability_adapter::MODULE_ID;
use crm_customer_enrichment_review_adapter::{
    review_decision_persisted_payload, review_decision_record_ref, review_decision_to_wire,
    suggestion_persisted_payload, suggestion_record_ref, suggestion_to_wire,
};
use crm_module_sdk::testing::FixedClock;
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, CorrelationId,
    DataClass, DomainEvent, ErrorCategory, EventType, ExecutionContext, IdempotencyKey,
    ModuleExecutionContext, PortFuture, RecordId, RecordRef, RequestId, SchemaVersion, SdkError,
    TenantId, TraceId, TypedPayload,
};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;
use prost::Message;
use sqlx::PgPool;
use std::sync::{Arc, Mutex};

const TENANT: &str = "tenant-application-orchestration-a";
const ACTOR: &str = "application-orchestrator-a";
const SEED: &str = "customer_enrichment.application_orchestration.seed";

#[derive(Debug, Default)]
struct AllowedPolicy(Mutex<Vec<EnrichmentPolicyRequest>>);

impl EnrichmentPolicyPort for AllowedPolicy {
    fn evaluate<'a>(
        &'a self,
        request: EnrichmentPolicyRequest,
    ) -> PortFuture<'a, Result<EnrichmentPolicyDecision, SdkError>> {
        Box::pin(async move {
            self.0.lock().unwrap().push(request);
            Ok(EnrichmentPolicyDecision::Allowed {
                decision_id: "application-policy-decision-1".into(),
                policy_version: "owner-application-policy-v1".into(),
            })
        })
    }
}

#[derive(Debug, Default)]
struct ReplayOwner(Mutex<Vec<PartyDisplayNameApplicationRequest>>);

impl PartyDisplayNameApplicationPort for ReplayOwner {
    fn apply<'a>(
        &'a self,
        request: PartyDisplayNameApplicationRequest,
    ) -> PortFuture<'a, Result<PartyDisplayNameApplicationResult, SdkError>> {
        Box::pin(async move {
            let mut calls = self.0.lock().unwrap();
            if calls.first().is_some_and(|first| {
                first.application_attempt_id != request.application_attempt_id
                    || first.target_idempotency_key != request.target_idempotency_key
                    || first.party_id != request.party_id
                    || first.reviewed_display_name != request.reviewed_display_name
            }) {
                return Err(SdkError::new(
                    "CUSTOMER_ENRICHMENT_TEST_OWNER_REPLAY_CONFLICT",
                    ErrorCategory::Conflict,
                    false,
                    "Replay changed owner application lineage.",
                ));
            }
            calls.push(request.clone());
            Ok(PartyDisplayNameApplicationResult::Applied {
                business_transaction_id: request.application_attempt_id.as_str().into(),
                resulting_party_resource_version: request.expected_party_resource_version + 1,
            })
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn recovers_owner_success_and_skips_completed_replay() {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        return;
    };
    let admin_url = std::env::var("ADMIN_DATABASE_URL").unwrap();
    let store = PostgresDataStore::connect(&url, 6).await.unwrap();
    let admin = PgPool::connect(&admin_url).await.unwrap();
    let (suggestion, review) = evidence();
    seed(
        &store,
        suggestion_record_ref(suggestion.suggestion_id().as_str()).unwrap(),
        suggestion_persisted_payload(&suggestion).unwrap(),
        "customer_enrichment.suggestion.materialized",
        "crm.customer_enrichment.v1.SuggestionMaterializedEvent",
        wire::SuggestionMaterializedEvent {
            suggestion: Some(suggestion_to_wire(&suggestion, None, 30).unwrap()),
        },
        "suggestion",
        30_000_000,
    )
    .await;
    seed(
        &store,
        review_decision_record_ref(&review).unwrap(),
        review_decision_persisted_payload(&review).unwrap(),
        "customer_enrichment.suggestion.reviewed",
        "crm.customer_enrichment.v1.SuggestionReviewedEvent",
        wire::SuggestionReviewedEvent {
            suggestion: Some(suggestion_to_wire(&suggestion, Some(&review), 40).unwrap()),
            review_decision: Some(review_decision_to_wire(&review).unwrap()),
        },
        "review",
        40_000_000,
    )
    .await;

    let apply = apply_request(&suggestion, &review);
    let pending = PostgresCustomerEnrichmentApplicationAttemptExecutor::new(store.clone())
        .execute(apply.clone())
        .await
        .unwrap();
    assert!(!pending.replayed);
    let planned = ApplicationAttempt::plan(
        TenantId::try_new(TENANT).unwrap(),
        &suggestion,
        &review,
        0,
        50,
    )
    .unwrap();
    let policy = Arc::new(AllowedPolicy::default());
    let owner = Arc::new(ReplayOwner::default());
    owner
        .apply(owner_request(&suggestion, &review, &planned))
        .await
        .unwrap();

    let orchestrator = CustomerEnrichmentPartyApplicationOrchestrator::postgres(
        store,
        policy.clone(),
        owner.clone(),
        Arc::new(FixedClock::new(60_000_000)),
    )
    .unwrap();
    let recovered = orchestrator.execute(apply.clone()).await.unwrap();
    assert!(recovered.attempt_replayed && recovered.policy_evaluated && recovered.owner_invoked);
    let recorded = recovered.application_attempt.recorded_outcome.unwrap();
    assert_eq!(recorded.recorded_at_unix_ms, 60);
    assert!(matches!(
        recorded.outcome.unwrap().result,
        Some(wire::application_outcome::Result::Succeeded(ref value))
            if value.resulting_party_resource_version == 8
    ));

    let replay = orchestrator.execute(apply).await.unwrap();
    assert!(replay.attempt_replayed && replay.outcome_replayed);
    assert!(!replay.policy_evaluated && !replay.owner_invoked);
    assert_eq!(policy.0.lock().unwrap().len(), 1);
    assert_eq!(
        policy.0.lock().unwrap()[0].phase,
        PolicyEvaluationPhase::OwnerApplication
    );
    let calls = owner.0.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert_eq!(
        calls[0].target_idempotency_key,
        calls[1].target_idempotency_key
    );
    drop(calls);
    assert_eq!(
        scalar(
            &admin,
            "SELECT version::bigint FROM crm.records WHERE tenant_id = 'tenant-application-orchestration-a' AND record_type = 'customer_enrichment.application_attempt'",
        )
        .await,
        2
    );
    assert_eq!(
        scalar(
            &admin,
            "SELECT count(*)::bigint FROM crm.outbox_events WHERE tenant_id = 'tenant-application-orchestration-a' AND event_type = 'customer_enrichment.suggestion.application_recorded'",
        )
        .await,
        2
    );
}

fn owner_request(
    suggestion: &Suggestion,
    review: &ReviewDecision,
    attempt: &ApplicationAttempt,
) -> PartyDisplayNameApplicationRequest {
    PartyDisplayNameApplicationRequest {
        tenant_id: TenantId::try_new(TENANT).unwrap(),
        actor_id: ActorId::try_new(ACTOR).unwrap(),
        suggestion_id: suggestion.suggestion_id().clone(),
        review_decision_id: review.decision_id().clone(),
        application_attempt_id: attempt.attempt_id().clone(),
        party_id: RecordId::try_new("party-application-orchestration-1").unwrap(),
        expected_party_resource_version: 7,
        reviewed_display_name: suggestion.proposed_value().into(),
        target_idempotency_key: attempt.target_idempotency_key().as_str().into(),
        final_authorization_decision_id: "application-policy-decision-1".into(),
    }
}

fn apply_request(suggestion: &Suggestion, review: &ReviewDecision) -> CapabilityRequest {
    let definition = apply_party_display_name_capability_definition().unwrap();
    let input = support::protobuf_payload(
        MODULE_ID,
        APPLY_PARTY_DISPLAY_NAME_REQUEST_SCHEMA,
        DataClass::Personal,
        &wire::ApplyPartyDisplayNameSuggestionRequest {
            suggestion_ref: Some(wire::SuggestionRef {
                suggestion_id: suggestion.suggestion_id().as_str().into(),
            }),
            review_decision_ref: Some(wire::ReviewDecisionRef {
                review_decision_id: review.decision_id().as_str().into(),
            }),
            expected_party_resource_version: 7,
            application_generation: 0,
        },
    )
    .unwrap();
    CapabilityRequest {
        input_hash: semantic_input_hash(&input),
        context: context("apply", definition.capability_id.as_str(), 50_000_000),
        input,
        approval: None,
    }
}

#[allow(clippy::too_many_arguments)]
async fn seed<M: Message>(
    store: &PostgresDataStore,
    reference: RecordRef,
    payload: TypedPayload,
    event_type: &str,
    schema: &str,
    event: M,
    key: &str,
    at: i64,
) {
    let event_payload =
        support::protobuf_payload(MODULE_ID, schema, DataClass::Personal, &event).unwrap();
    store
        .create_record(&RecordCreatePlan {
            context: context(key, SEED, at),
            record: reference.clone(),
            record_payload: payload,
            event_id: format!("{key}-event"),
            event: DomainEvent {
                event_type: EventType::try_new(event_type).unwrap(),
                aggregate: reference,
                expected_aggregate_version: None,
                deduplication_key: format!("{key}-event"),
                payload: event_payload.clone(),
            },
            idempotency: IdempotencyEvidence {
                scope: format!("{SEED}@1.0.0"),
                key: format!("{key}-idempotency"),
                request_hash: semantic_input_hash(&event_payload),
                expires_at_unix_nanos: at + 86_400_000_000_000,
            },
            audit: AuditIntent {
                audit_record_id: format!("{key}-audit"),
                canonicalization_profile: "crm.cjson/v1".into(),
                canonical_envelope: format!("{{\"seed\":\"{key}\"}}").into_bytes(),
                occurred_at_unix_nanos: at,
            },
        })
        .await
        .unwrap();
}

fn context(key: &str, capability: &str, at: i64) -> ModuleExecutionContext {
    ModuleExecutionContext {
        module_id: crm_module_sdk::ModuleId::try_new(MODULE_ID).unwrap(),
        execution: ExecutionContext {
            tenant_id: TenantId::try_new(TENANT).unwrap(),
            actor_id: ActorId::try_new(ACTOR).unwrap(),
            request_id: RequestId::try_new(format!("{key}-request")).unwrap(),
            correlation_id: CorrelationId::try_new(format!("{key}-correlation")).unwrap(),
            causation_id: CausationId::try_new(format!("{key}-causation")).unwrap(),
            trace_id: TraceId::try_new(format!("{key}-trace")).unwrap(),
            capability_id: CapabilityId::try_new(capability).unwrap(),
            capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
            idempotency_key: IdempotencyKey::try_new(format!("{key}-idempotency")).unwrap(),
            business_transaction_id: BusinessTransactionId::try_new(format!("{key}-transaction"))
                .unwrap(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
            request_started_at_unix_nanos: at,
        },
    }
}

fn evidence() -> (Suggestion, ReviewDecision) {
    let profile = ProviderProfileVersion::publish(ProviderProfileDraft {
        provider_key: "orchestration-registry".into(),
        adapter_kind: "orchestration-http-v1".into(),
        adapter_contract_version: "1.0.0".into(),
        supported_target_fields: vec![TargetField::PartyDisplayName],
        purpose_codes: vec!["customer_profile_enrichment".into()],
        license_id: "Orchestration licence".into(),
        permitted_use_class: "customer_master_application".into(),
        residency_region: "eu".into(),
        retention_days: 30,
        raw_payload_policy: RawPayloadPolicy::GovernedProtectedEvidence,
        credential_handle_aliases: vec!["orchestration_primary".into()],
        effective_at_unix_ms: 1,
        expires_at_unix_ms: Some(5_000),
    })
    .unwrap();
    let mapping = MappingVersion::publish(MappingDraft {
        mapping_key: "orchestration_party_display_name".into(),
        provider_profile_version_id: profile.version_id().clone(),
        provider_response_field_path: "organization.legal_name".into(),
        target_field: TargetField::PartyDisplayName,
        normalization: MappingNormalization::CanonicalPartyDisplayNameV1,
        maximum_suggestions_per_response: 1,
        confidence_required: true,
    })
    .unwrap();
    let mut request = EnrichmentRequest::create(EnrichmentRequestDraft {
        tenant_id: TenantId::try_new(TENANT).unwrap(),
        requested_by: ActorId::try_new("orchestration-worker").unwrap(),
        idempotency_key: IdempotencyKey::try_new("orchestration-domain-request").unwrap(),
        target: TargetSnapshot::try_new(
            "party-application-orchestration-1",
            7,
            TargetField::PartyDisplayName,
        )
        .unwrap(),
        provider_profile_version_id: profile.version_id().clone(),
        mapping_version_id: mapping.version_id().clone(),
        requested_fields: vec![TargetField::PartyDisplayName],
        policy_evidence: RequestPolicyEvidence::try_new(
            "customer_profile_enrichment",
            "legitimate_interest",
            Some("consent-orchestration-1".into()),
            "request-policy-v1",
        )
        .unwrap(),
        created_at_unix_ms: 1,
        deadline_at_unix_ms: 1_000,
        expires_at_unix_ms: 2_000,
    })
    .unwrap();
    request.queue(10).unwrap();
    request.mark_dispatched(10).unwrap();
    let receipt = ProviderResponseReceipt::record(ProviderResponseReceiptDraft {
        request_id: request.request_id().clone(),
        provider_profile_version_id: profile.version_id().clone(),
        mapping_version_id: mapping.version_id().clone(),
        replay_key: "orchestration-provider-replay-1".into(),
        provider_correlation_id: None,
        response_class: ProviderResponseClass::Success,
        canonical_response_digest: [83; 32],
        provider_observed_at_unix_ms: Some(20),
        retrieved_at_unix_ms: 30,
        metered_units: 1,
        protected_evidence_reference: Some("orchestration-evidence-1".into()),
    })
    .unwrap();
    let suggestion = Suggestion::materialize(SuggestionDraft {
        request_id: request.request_id().clone(),
        response_receipt_id: receipt.receipt_id().clone(),
        provider_profile_version_id: profile.version_id().clone(),
        mapping_version_id: mapping.version_id().clone(),
        target: request.target().clone(),
        proposed_value: "Applied Orchestration Company".into(),
        observed_at_unix_ms: Some(20),
        retrieved_at_unix_ms: 30,
        effective_at_unix_ms: 20,
        fresh_until_unix_ms: 1_000,
        expires_at_unix_ms: 1_500,
        confidence_basis_points: Some(9_200),
        purpose_code: "customer_profile_enrichment".into(),
        legal_basis_code: "legitimate_interest".into(),
        license_id: "Orchestration licence".into(),
        permitted_use_class: "customer_master_application".into(),
        residency_region: "eu".into(),
        retention_days: 30,
        consent_evidence_reference: Some("consent-orchestration-1".into()),
        evidence_references: vec!["orchestration-evidence-1".into()],
    })
    .unwrap();
    let review = ReviewDecision::decide(
        &suggestion,
        ActorId::try_new(ACTOR).unwrap(),
        ReviewDecisionKind::Accepted,
        "review-policy-v1",
        "reviewed_accepted",
        ApprovalRequirement::Required,
        Some("approval-orchestration-1".into()),
        40,
        Some(1_000),
    )
    .unwrap();
    (suggestion, review)
}

async fn scalar(pool: &PgPool, query: &'static str) -> i64 {
    sqlx::query_scalar(query).fetch_one(pool).await.unwrap()
}
