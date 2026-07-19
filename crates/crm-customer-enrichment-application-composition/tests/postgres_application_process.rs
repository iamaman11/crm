use crm_capability_ingress::semantic_input_hash;
use crm_capability_plan_support as support;
use crm_capability_runtime::CapabilityRequest;
use crm_core_data::{AuditIntent, IdempotencyEvidence, PostgresDataStore, RecordCreatePlan};
use crm_customer_enrichment::{
    ApprovalRequirement, EnrichmentRequest, EnrichmentRequestDraft, MappingDraft,
    MappingNormalization, MappingVersion, ProviderProfileDraft, ProviderProfileVersion,
    ProviderResponseClass, ProviderResponseReceipt, ProviderResponseReceiptDraft, RawPayloadPolicy,
    RequestPolicyEvidence, ReviewDecision, ReviewDecisionKind, Suggestion, SuggestionDraft,
    TargetField, TargetSnapshot,
};
use crm_customer_enrichment_application_adapter::{
    APPLY_PARTY_DISPLAY_NAME_REQUEST_SCHEMA, RECORD_APPLICATION_OUTCOME_REQUEST_SCHEMA,
    apply_party_display_name_capability_definition, record_application_outcome_capability_definition,
};
use crm_customer_enrichment_application_composition::{
    PostgresCustomerEnrichmentApplicationAttemptExecutor,
    PostgresCustomerEnrichmentApplicationOutcomeExecutor,
};
use crm_customer_enrichment_capability_adapter::MODULE_ID;
use crm_customer_enrichment_review_adapter::{
    review_decision_persisted_payload, review_decision_record_ref, review_decision_to_wire,
    suggestion_persisted_payload, suggestion_record_ref, suggestion_to_wire,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, CorrelationId,
    DataClass, DomainEvent, EventType, ExecutionContext, IdempotencyKey, ModuleExecutionContext,
    RecordRef, RequestId, SchemaVersion, TenantId, TraceId, TypedPayload,
};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;
use prost::Message;
use sqlx::PgPool;

const TENANT_ID: &str = "tenant-application-a";
const ACTOR_ID: &str = "application-reviewer-a";
const SEED_CAPABILITY: &str = "customer_enrichment.application.seed";
const SUGGESTION_MATERIALIZED_EVENT_TYPE: &str = "customer_enrichment.suggestion.materialized";
const SUGGESTION_MATERIALIZED_EVENT_SCHEMA: &str =
    "crm.customer_enrichment.v1.SuggestionMaterializedEvent";
const SUGGESTION_REVIEWED_EVENT_TYPE: &str = "customer_enrichment.suggestion.reviewed";
const SUGGESTION_REVIEWED_EVENT_SCHEMA: &str =
    "crm.customer_enrichment.v1.SuggestionReviewedEvent";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn postgres_application_attempt_outcome_replay_and_conflict_are_deterministic() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping PostgreSQL application process because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let store = PostgresDataStore::connect(&database_url, 6)
        .await
        .expect("connect application store");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect application evidence reader");

    let suggestion = suggestion();
    let review = accepted_review(&suggestion);
    seed_suggestion(&store, &suggestion)
        .await
        .expect("seed immutable suggestion");
    seed_review(&store, &suggestion, &review)
        .await
        .expect("seed immutable accepted review");

    let attempt_executor = PostgresCustomerEnrichmentApplicationAttemptExecutor::new(store.clone());
    let apply = apply_request(
        &suggestion,
        &review,
        "application-request-1",
        "application-idempotency-1",
        "application-tx-1",
    );
    let first_apply = attempt_executor
        .execute(apply.clone())
        .await
        .expect("persist pending application attempt");
    assert!(!first_apply.replayed);
    let first_apply_output = wire::ApplyPartyDisplayNameSuggestionResponse::decode(
        first_apply.output.as_ref().unwrap().bytes.as_slice(),
    )
    .unwrap();
    let pending = first_apply_output.application_attempt.unwrap();
    let attempt_id = pending
        .application_attempt_ref
        .as_ref()
        .unwrap()
        .application_attempt_id
        .clone();
    assert_eq!(pending.owner_capability_id, "parties.party.update");
    assert_eq!(pending.owner_capability_version, "1.0.0");
    assert_eq!(pending.application_generation, 0);
    assert!(pending.target_idempotency_key.starts_with("customer-enrichment-apply-"));
    assert!(pending.recorded_outcome.is_none());

    let replayed_apply = attempt_executor
        .execute(apply)
        .await
        .expect("replay exact pending application attempt");
    assert!(replayed_apply.replayed);
    let replayed_apply_output = wire::ApplyPartyDisplayNameSuggestionResponse::decode(
        replayed_apply.output.as_ref().unwrap().bytes.as_slice(),
    )
    .unwrap();
    assert_eq!(
        replayed_apply_output
            .application_attempt
            .unwrap()
            .application_attempt_ref
            .unwrap()
            .application_attempt_id,
        attempt_id
    );

    let outcome_executor = PostgresCustomerEnrichmentApplicationOutcomeExecutor::new(store);
    let outcome = outcome_request(
        &attempt_id,
        8,
        60,
        "application-outcome-request-1",
        "application-outcome-idempotency-1",
        "application-outcome-tx-1",
    );
    let first_outcome = outcome_executor
        .execute(outcome.clone())
        .await
        .expect("append exact successful outcome");
    assert!(!first_outcome.replayed);
    let first_outcome_output = wire::RecordApplicationOutcomeResponse::decode(
        first_outcome.output.as_ref().unwrap().bytes.as_slice(),
    )
    .unwrap();
    let completed = first_outcome_output.application_attempt.unwrap();
    let recorded = completed.recorded_outcome.unwrap();
    assert_eq!(recorded.recorded_at_unix_ms, 60);
    match recorded.outcome.unwrap().result.unwrap() {
        wire::application_outcome::Result::Succeeded(success) => {
            assert_eq!(success.business_transaction_id, "party-update-tx-1");
            assert_eq!(success.resulting_party_resource_version, 8);
        }
        other => panic!("expected successful application outcome, got {other:?}"),
    }

    let exact_outcome_replay = outcome_executor
        .execute(outcome)
        .await
        .expect("replay exact outcome idempotency");
    assert!(exact_outcome_replay.replayed);

    let semantic_duplicate = outcome_request(
        &attempt_id,
        8,
        60,
        "application-outcome-request-2",
        "application-outcome-idempotency-2",
        "application-outcome-tx-2",
    );
    let duplicate_result = outcome_executor
        .execute(semantic_duplicate)
        .await
        .expect("audit semantic duplicate without rewriting evidence");
    assert!(!duplicate_result.replayed);

    let conflicting = outcome_request(
        &attempt_id,
        9,
        61,
        "application-outcome-request-3",
        "application-outcome-idempotency-3",
        "application-outcome-tx-3",
    );
    let conflict = outcome_executor.execute(conflicting).await.unwrap_err();
    assert_eq!(
        conflict.code,
        "CUSTOMER_ENRICHMENT_APPLICATION_OUTCOME_CONFLICT"
    );

    assert_eq!(
        scalar(
            &admin,
            "SELECT count(*)::bigint FROM crm.records WHERE tenant_id = 'tenant-application-a' AND owner_module_id = 'crm.customer-enrichment'",
        )
        .await,
        3
    );
    assert_eq!(
        scalar(
            &admin,
            "SELECT version::bigint FROM crm.records WHERE tenant_id = 'tenant-application-a' AND record_type = 'customer_enrichment.application_attempt'",
        )
        .await,
        2
    );
    assert_eq!(
        scalar(
            &admin,
            "SELECT count(*)::bigint FROM crm.outbox_events WHERE tenant_id = 'tenant-application-a' AND event_type LIKE 'customer_enrichment.%'",
        )
        .await,
        4
    );
    assert_eq!(
        scalar(
            &admin,
            "SELECT count(*)::bigint FROM crm.outbox_events WHERE tenant_id = 'tenant-application-a' AND event_type = 'customer_enrichment.suggestion.application_recorded'",
        )
        .await,
        2
    );
    assert_eq!(
        scalar(
            &admin,
            "SELECT count(*)::bigint FROM crm.audit_records WHERE tenant_id = 'tenant-application-a' AND capability_id LIKE 'customer_enrichment.%'",
        )
        .await,
        5
    );
    assert_eq!(
        scalar(
            &admin,
            "SELECT count(*)::bigint FROM crm.idempotency_records WHERE tenant_id = 'tenant-application-a'",
        )
        .await,
        5
    );
    assert_eq!(
        scalar(
            &admin,
            "SELECT count(*)::bigint FROM crm.business_transactions WHERE tenant_id = 'tenant-application-a'",
        )
        .await,
        5
    );
}

fn accepted_review(suggestion: &Suggestion) -> ReviewDecision {
    ReviewDecision::decide(
        suggestion,
        ActorId::try_new(ACTOR_ID).unwrap(),
        ReviewDecisionKind::Accepted,
        "review-policy-v1".to_owned(),
        "reviewed_accepted".to_owned(),
        ApprovalRequirement::Required,
        Some("approval-application-1".to_owned()),
        40,
        Some(1_000),
    )
    .unwrap()
}

async fn seed_suggestion(
    store: &PostgresDataStore,
    suggestion: &Suggestion,
) -> Result<(), Box<dyn std::error::Error>> {
    let reference = suggestion_record_ref(suggestion.suggestion_id().as_str())?;
    let event_payload = support::protobuf_payload(
        MODULE_ID,
        SUGGESTION_MATERIALIZED_EVENT_SCHEMA,
        DataClass::Personal,
        &wire::SuggestionMaterializedEvent {
            suggestion: Some(suggestion_to_wire(suggestion, None, 30)?),
        },
    )?;
    seed_record(
        store,
        reference,
        suggestion_persisted_payload(suggestion)?,
        SUGGESTION_MATERIALIZED_EVENT_TYPE,
        event_payload,
        "application-seed-suggestion",
        "application-seed-suggestion-event",
        "application-seed-suggestion-audit",
        "application-seed-suggestion-idempotency",
        "application-seed-suggestion-tx",
        30_000_000,
    )
    .await
}

async fn seed_review(
    store: &PostgresDataStore,
    suggestion: &Suggestion,
    review: &ReviewDecision,
) -> Result<(), Box<dyn std::error::Error>> {
    let reference = review_decision_record_ref(review)?;
    let event_payload = support::protobuf_payload(
        MODULE_ID,
        SUGGESTION_REVIEWED_EVENT_SCHEMA,
        DataClass::Personal,
        &wire::SuggestionReviewedEvent {
            suggestion: Some(suggestion_to_wire(suggestion, Some(review), 40)?),
            review_decision: Some(review_decision_to_wire(review)?),
        },
    )?;
    seed_record(
        store,
        reference,
        review_decision_persisted_payload(review)?,
        SUGGESTION_REVIEWED_EVENT_TYPE,
        event_payload,
        "application-seed-review",
        "application-seed-review-event",
        "application-seed-review-audit",
        "application-seed-review-idempotency",
        "application-seed-review-tx",
        40_000_000,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn seed_record(
    store: &PostgresDataStore,
    reference: RecordRef,
    record_payload: TypedPayload,
    event_type: &str,
    event_payload: TypedPayload,
    request_id: &str,
    event_id: &str,
    audit_id: &str,
    idempotency_key: &str,
    transaction_id: &str,
    started_at_unix_nanos: i64,
) -> Result<(), Box<dyn std::error::Error>> {
    let request_hash = semantic_input_hash(&event_payload);
    store
        .create_record(&RecordCreatePlan {
            context: context(
                request_id,
                SEED_CAPABILITY,
                idempotency_key,
                transaction_id,
                started_at_unix_nanos,
            ),
            record: reference.clone(),
            record_payload,
            event_id: event_id.to_owned(),
            event: DomainEvent {
                event_type: EventType::try_new(event_type)?,
                aggregate: reference,
                expected_aggregate_version: None,
                deduplication_key: event_id.to_owned(),
                payload: event_payload,
            },
            idempotency: IdempotencyEvidence {
                scope: format!("{SEED_CAPABILITY}@1.0.0"),
                key: idempotency_key.to_owned(),
                request_hash,
                expires_at_unix_nanos: 86_400_000_000_000 + started_at_unix_nanos,
            },
            audit: AuditIntent {
                audit_record_id: audit_id.to_owned(),
                canonicalization_profile: "crm.cjson/v1".to_owned(),
                canonical_envelope: format!("{{\"seed\":\"{request_id}\"}}").into_bytes(),
                occurred_at_unix_nanos: started_at_unix_nanos,
            },
        })
        .await?;
    Ok(())
}

fn apply_request(
    suggestion: &Suggestion,
    review: &ReviewDecision,
    request_id: &str,
    idempotency_key: &str,
    transaction_id: &str,
) -> CapabilityRequest {
    let definition = apply_party_display_name_capability_definition().unwrap();
    let input = support::protobuf_payload(
        MODULE_ID,
        APPLY_PARTY_DISPLAY_NAME_REQUEST_SCHEMA,
        DataClass::Personal,
        &wire::ApplyPartyDisplayNameSuggestionRequest {
            suggestion_ref: Some(wire::SuggestionRef {
                suggestion_id: suggestion.suggestion_id().as_str().to_owned(),
            }),
            review_decision_ref: Some(wire::ReviewDecisionRef {
                review_decision_id: review.decision_id().as_str().to_owned(),
            }),
            expected_party_resource_version: 7,
            application_generation: 0,
        },
    )
    .unwrap();
    let input_hash = semantic_input_hash(&input);
    CapabilityRequest {
        context: context(
            request_id,
            definition.capability_id.as_str(),
            idempotency_key,
            transaction_id,
            50_000_000,
        ),
        input,
        input_hash,
        approval: None,
    }
}

fn outcome_request(
    attempt_id: &str,
    resulting_party_version: i64,
    recorded_at_unix_ms: i64,
    request_id: &str,
    idempotency_key: &str,
    transaction_id: &str,
) -> CapabilityRequest {
    let definition = record_application_outcome_capability_definition().unwrap();
    let input = support::protobuf_payload(
        MODULE_ID,
        RECORD_APPLICATION_OUTCOME_REQUEST_SCHEMA,
        DataClass::Personal,
        &wire::RecordApplicationOutcomeRequest {
            application_attempt_ref: Some(wire::ApplicationAttemptRef {
                application_attempt_id: attempt_id.to_owned(),
            }),
            outcome: Some(wire::ApplicationOutcome {
                result: Some(wire::application_outcome::Result::Succeeded(
                    wire::ApplicationSucceeded {
                        business_transaction_id: "party-update-tx-1".to_owned(),
                        resulting_party_resource_version: resulting_party_version,
                    },
                )),
            }),
            recorded_at_unix_ms,
        },
    )
    .unwrap();
    let input_hash = semantic_input_hash(&input);
    CapabilityRequest {
        context: context(
            request_id,
            definition.capability_id.as_str(),
            idempotency_key,
            transaction_id,
            recorded_at_unix_ms * 1_000_000,
        ),
        input,
        input_hash,
        approval: None,
    }
}

fn context(
    request_id: &str,
    capability_id: &str,
    idempotency_key: &str,
    transaction_id: &str,
    started_at_unix_nanos: i64,
) -> ModuleExecutionContext {
    ModuleExecutionContext {
        module_id: crm_module_sdk::ModuleId::try_new(MODULE_ID).unwrap(),
        execution: ExecutionContext {
            tenant_id: TenantId::try_new(TENANT_ID).unwrap(),
            actor_id: ActorId::try_new(ACTOR_ID).unwrap(),
            request_id: RequestId::try_new(request_id).unwrap(),
            correlation_id: CorrelationId::try_new(format!("correlation-{request_id}")).unwrap(),
            causation_id: CausationId::try_new(format!("causation-{request_id}")).unwrap(),
            trace_id: TraceId::try_new(format!("trace-{request_id}")).unwrap(),
            capability_id: CapabilityId::try_new(capability_id).unwrap(),
            capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
            idempotency_key: IdempotencyKey::try_new(idempotency_key).unwrap(),
            business_transaction_id: BusinessTransactionId::try_new(transaction_id).unwrap(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
            request_started_at_unix_nanos: started_at_unix_nanos,
        },
    }
}

fn suggestion() -> Suggestion {
    let profile = ProviderProfileVersion::publish(ProviderProfileDraft {
        provider_key: "application-registry".to_owned(),
        adapter_kind: "application-http-v1".to_owned(),
        adapter_contract_version: "1.0.0".to_owned(),
        supported_target_fields: vec![TargetField::PartyDisplayName],
        purpose_codes: vec!["customer_profile_enrichment".to_owned()],
        license_id: "Application registry licence".to_owned(),
        permitted_use_class: "customer_master_application".to_owned(),
        residency_region: "eu".to_owned(),
        retention_days: 30,
        raw_payload_policy: RawPayloadPolicy::GovernedProtectedEvidence,
        credential_handle_aliases: vec!["application_registry_primary".to_owned()],
        effective_at_unix_ms: 1,
        expires_at_unix_ms: Some(5_000),
    })
    .unwrap();
    let mapping = MappingVersion::publish(MappingDraft {
        mapping_key: "application_party_display_name".to_owned(),
        provider_profile_version_id: profile.version_id().clone(),
        provider_response_field_path: "organization.legal_name".to_owned(),
        target_field: TargetField::PartyDisplayName,
        normalization: MappingNormalization::CanonicalPartyDisplayNameV1,
        maximum_suggestions_per_response: 1,
        confidence_required: true,
    })
    .unwrap();
    let mut request = EnrichmentRequest::create(EnrichmentRequestDraft {
        tenant_id: TenantId::try_new(TENANT_ID).unwrap(),
        requested_by: ActorId::try_new("application-worker-a").unwrap(),
        idempotency_key: IdempotencyKey::try_new("application-domain-request").unwrap(),
        target: TargetSnapshot::try_new("party-application-1", 7, TargetField::PartyDisplayName)
            .unwrap(),
        provider_profile_version_id: profile.version_id().clone(),
        mapping_version_id: mapping.version_id().clone(),
        requested_fields: vec![TargetField::PartyDisplayName],
        policy_evidence: RequestPolicyEvidence::try_new(
            "customer_profile_enrichment",
            "legitimate_interest",
            Some("consent-application-1".to_owned()),
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
        replay_key: "application-provider-replay-1".to_owned(),
        provider_correlation_id: Some("application-provider-correlation-1".to_owned()),
        response_class: ProviderResponseClass::Success,
        canonical_response_digest: [82; 32],
        provider_observed_at_unix_ms: Some(20),
        retrieved_at_unix_ms: 30,
        metered_units: 1,
        protected_evidence_reference: Some("application-evidence-1".to_owned()),
    })
    .unwrap();
    Suggestion::materialize(SuggestionDraft {
        request_id: request.request_id().clone(),
        response_receipt_id: receipt.receipt_id().clone(),
        provider_profile_version_id: profile.version_id().clone(),
        mapping_version_id: mapping.version_id().clone(),
        target: request.target().clone(),
        proposed_value: "Applied Company".to_owned(),
        observed_at_unix_ms: Some(20),
        retrieved_at_unix_ms: 30,
        effective_at_unix_ms: 20,
        fresh_until_unix_ms: 1_000,
        expires_at_unix_ms: 1_500,
        confidence_basis_points: Some(9_100),
        purpose_code: "customer_profile_enrichment".to_owned(),
        legal_basis_code: "legitimate_interest".to_owned(),
        license_id: "Application registry licence".to_owned(),
        permitted_use_class: "customer_master_application".to_owned(),
        residency_region: "eu".to_owned(),
        retention_days: 30,
        consent_evidence_reference: Some("consent-application-1".to_owned()),
        evidence_references: vec!["application-evidence-1".to_owned()],
    })
    .unwrap()
}

async fn scalar(pool: &PgPool, query: &'static str) -> i64 {
    sqlx::query_scalar::<_, i64>(query)
        .fetch_one(pool)
        .await
        .expect("query application evidence")
}
