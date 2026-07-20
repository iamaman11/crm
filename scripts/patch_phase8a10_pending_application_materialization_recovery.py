from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one exact anchor, found {count}")
    file.write_text(text.replace(old, new, 1))


cargo = "crates/crm-customer-enrichment-materialization-composition/Cargo.toml"
replace_once(
    cargo,
    """[dev-dependencies]
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
""",
    """[dev-dependencies]
crm-customer-enrichment-application-adapter = { path = "../crm-customer-enrichment-application-adapter" }
crm-customer-enrichment-application-composition = { path = "../crm-customer-enrichment-application-composition" }
crm-customer-enrichment-review-adapter = { path = "../crm-customer-enrichment-review-adapter" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
""",
)

path = "crates/crm-customer-enrichment-materialization-composition/tests/postgres_materialization_event_process.rs"
replace_once(
    path,
    """use crm_customer_enrichment::{
    EnrichmentRequest, EnrichmentRequestDraft, LIFECYCLE_STATE_RETENTION_POLICY_ID,
    LIFECYCLE_STATE_SCHEMA_VERSION, MappingDraft, MappingNormalization, MappingVersion,
    PROVIDER_RESPONSE_RECEIPT_RECORD_TYPE, PROVIDER_RESPONSE_RECEIPT_STATE_MAXIMUM_BYTES,
    PROVIDER_RESPONSE_RECEIPT_STATE_SCHEMA_ID, ProviderProfileDraft, ProviderProfileVersion,
    ProviderResponseClass, ProviderResponseReceipt, ProviderResponseReceiptDraft, RawPayloadPolicy,
    RequestPolicyEvidence, TargetField, TargetSnapshot, encode_provider_response_receipt_state,
    provider_response_receipt_state_descriptor_hash,
};
""",
    """use crm_customer_enrichment::{
    ApprovalRequirement, EnrichmentRequest, EnrichmentRequestDraft,
    LIFECYCLE_STATE_RETENTION_POLICY_ID, LIFECYCLE_STATE_SCHEMA_VERSION, MappingDraft,
    MappingNormalization, MappingVersion, PROVIDER_RESPONSE_RECEIPT_RECORD_TYPE,
    PROVIDER_RESPONSE_RECEIPT_STATE_MAXIMUM_BYTES, PROVIDER_RESPONSE_RECEIPT_STATE_SCHEMA_ID,
    ProviderProfileDraft, ProviderProfileVersion, ProviderResponseClass, ProviderResponseReceipt,
    ProviderResponseReceiptDraft, RawPayloadPolicy, RequestPolicyEvidence, ReviewDecision,
    ReviewDecisionKind, Suggestion, SuggestionDraft, TargetField, TargetSnapshot,
    encode_provider_response_receipt_state, provider_response_receipt_state_descriptor_hash,
};
""",
)
replace_once(
    path,
    """use crm_customer_enrichment_materialization_composition::{
    CustomerEnrichmentMaterializationProcessWorker,
    GovernedFileProviderSuggestionCandidateEvidenceSource, MATERIALIZATION_PROCESS_PROJECTION_ID,
    PROVIDER_RESPONSE_RECORDED_EVENT_SCHEMA, PROVIDER_RESPONSE_RECORDED_EVENT_TYPE,
    PROVIDER_SUGGESTION_CANDIDATE_EVIDENCE_MEDIA_TYPE,
    PostgresCustomerEnrichmentSuggestionMaterializationWorker,
};
""",
    """use crm_customer_enrichment_application_adapter::{
    APPLY_PARTY_DISPLAY_NAME_REQUEST_SCHEMA, RECORD_APPLICATION_OUTCOME_REQUEST_SCHEMA,
    apply_party_display_name_capability_definition, record_application_outcome_capability_definition,
};
use crm_customer_enrichment_application_composition::{
    PostgresCustomerEnrichmentApplicationAttemptExecutor,
    PostgresCustomerEnrichmentApplicationOutcomeExecutor,
};
use crm_customer_enrichment_materialization_composition::{
    CustomerEnrichmentMaterializationProcessWorker,
    GovernedFileProviderSuggestionCandidateEvidenceSource, MATERIALIZATION_PROCESS_PROJECTION_ID,
    PROVIDER_RESPONSE_RECORDED_EVENT_SCHEMA, PROVIDER_RESPONSE_RECORDED_EVENT_TYPE,
    PROVIDER_SUGGESTION_CANDIDATE_EVIDENCE_MEDIA_TYPE,
    PostgresCustomerEnrichmentSuggestionMaterializationWorker,
};
use crm_customer_enrichment_review_adapter::{
    review_decision_persisted_payload, review_decision_record_ref, review_decision_to_wire,
    suggestion_persisted_payload, suggestion_record_ref, suggestion_to_wire,
};
""",
)
replace_once(
    path,
    """const FILE_ID: &str = "materialization-candidate-evidence-process-1";
const SEED_CAPABILITY: &str = "customer_enrichment.materialization.seed";
""",
    """const FILE_ID: &str = "materialization-candidate-evidence-process-1";
const SEED_CAPABILITY: &str = "customer_enrichment.materialization.seed";
const SUGGESTION_MATERIALIZED_EVENT_TYPE: &str = "customer_enrichment.suggestion.materialized";
const SUGGESTION_MATERIALIZED_EVENT_SCHEMA: &str =
    "crm.customer_enrichment.v1.SuggestionMaterializedEvent";
const SUGGESTION_REVIEWED_EVENT_TYPE: &str = "customer_enrichment.suggestion.reviewed";
const SUGGESTION_REVIEWED_EVENT_SCHEMA: &str =
    "crm.customer_enrichment.v1.SuggestionReviewedEvent";
""",
)
replace_once(
    path,
    """    let artifacts = Arc::new(PostgresImmutableFileArtifactStore::new(store.clone()));
    let process = CustomerEnrichmentMaterializationProcessWorker::new(
        store.clone(),
        Arc::new(GovernedFileProviderSuggestionCandidateEvidenceSource::new(
            artifacts.clone(),
        )),
        Arc::new(PostgresCustomerEnrichmentSuggestionMaterializationWorker::new(store.clone())),
        ActorId::try_new(ACTOR_ID).unwrap(),
    )
    .expect("compose materialization event process");
""",
    """    let artifacts = Arc::new(PostgresImmutableFileArtifactStore::new(store.clone()));
    let process = materialization_process(store.clone(), artifacts.clone());
""",
)
replace_once(
    path,
    """    upload_candidate_evidence(artifacts.as_ref(), &fixture)
        .await
        .expect("finalize canonical candidate evidence");

    let first = process
        .run_cycle(tenant_id.clone(), 60_000_000)
        .await
        .expect("materialize response event after evidence recovery");
""",
    """    upload_candidate_evidence(artifacts.as_ref(), &fixture)
        .await
        .expect("finalize canonical candidate evidence");

    let prior = prior_suggestion(&fixture);
    let review = accepted_review(&prior);
    seed_suggestion(&store, &prior)
        .await
        .expect("seed prior same-coordinate suggestion");
    seed_review(&store, &prior, &review)
        .await
        .expect("seed prior accepted review");
    let pending_result = PostgresCustomerEnrichmentApplicationAttemptExecutor::new(store.clone())
        .execute(application_request(&prior, &review))
        .await
        .expect("persist pending same-coordinate application attempt");
    let pending = wire::ApplyPartyDisplayNameSuggestionResponse::decode(
        pending_result
            .output
            .as_ref()
            .expect("pending application output")
            .bytes
            .as_slice(),
    )
    .expect("decode pending application attempt")
    .application_attempt
    .expect("pending application attempt");
    let attempt_id = pending
        .application_attempt_ref
        .expect("pending application attempt reference")
        .application_attempt_id;
    assert!(pending.recorded_outcome.is_none());

    let blocked_baseline = evidence_counts(&admin).await;
    assert_eq!(request_version(&admin, &fixture).await, 1);
    let blocked = process
        .run_cycle(tenant_id.clone(), 55_000_000)
        .await
        .expect_err("pending application must block materialization");
    assert_eq!(blocked.code, "CUSTOMER_ENRICHMENT_APPLICATION_IN_PROGRESS");
    assert!(blocked.retryable);
    assert!(
        ProjectionStore::projection_checkpoint(
            &store,
            tenant_id.clone(),
            MATERIALIZATION_PROCESS_PROJECTION_ID.to_owned(),
        )
        .await
        .unwrap()
        .is_none()
    );
    assert_eq!(request_version(&admin, &fixture).await, 1);
    assert_eq!(suggestion_count(&admin).await, 1);
    assert_eq!(evidence_counts(&admin).await, blocked_baseline);

    let outcome_executor =
        PostgresCustomerEnrichmentApplicationOutcomeExecutor::new(store.clone());
    let outcome_request = terminal_outcome_request(&attempt_id);
    let first_outcome = outcome_executor
        .execute(outcome_request.clone())
        .await
        .expect("append terminal application outcome");
    assert!(!first_outcome.replayed);
    assert_eq!(application_attempt_version(&admin, &attempt_id).await, 2);
    let terminal_baseline = evidence_counts(&admin).await;
    let outcome_replay = outcome_executor
        .execute(outcome_request)
        .await
        .expect("replay terminal outcome exactly");
    assert!(outcome_replay.replayed);
    assert_eq!(evidence_counts(&admin).await, terminal_baseline);

    drop(process);
    let recovered_process = materialization_process(store.clone(), artifacts.clone());
    let first = recovered_process
        .run_cycle(tenant_id.clone(), 60_000_000)
        .await
        .expect("materialize response event after terminal-outcome recovery");
""",
)
replace_once(path, "    assert_eq!(suggestion_count(&admin).await, 1);\n\n    let checkpoint", "    assert_eq!(suggestion_count(&admin).await, 2);\n    assert_eq!(request_version(&admin, &fixture).await, 2);\n    assert_eq!(application_attempt_count(&admin).await, 1);\n    assert_eq!(application_attempt_version(&admin, &attempt_id).await, 2);\n    let materialized_baseline = evidence_counts(&admin).await;\n\n    let checkpoint",)
replace_once(
    path,
    """    let replay = process
        .run_cycle(tenant_id, 70_000_000)
        .await
        .expect("checkpointed materialization replay");
""",
    """    drop(recovered_process);
    let replay_process = materialization_process(store.clone(), artifacts);
    let replay = replay_process
        .run_cycle(tenant_id, 70_000_000)
        .await
        .expect("checkpointed materialization replay after restart");
""",
)
replace_once(
    path,
    """    assert_eq!(replay.response_events, 0);
    assert_eq!(replay.materialized, 0);
    assert_eq!(suggestion_count(&admin).await, 1);
}

struct Fixture {
""",
    """    assert_eq!(replay.response_events, 0);
    assert_eq!(replay.materialized, 0);
    assert_eq!(suggestion_count(&admin).await, 2);
    assert_eq!(request_version(&admin, &fixture).await, 2);
    assert_eq!(application_attempt_count(&admin).await, 1);
    assert_eq!(evidence_counts(&admin).await, materialized_baseline);
}

fn materialization_process(
    store: PostgresDataStore,
    artifacts: Arc<PostgresImmutableFileArtifactStore>,
) -> CustomerEnrichmentMaterializationProcessWorker {
    CustomerEnrichmentMaterializationProcessWorker::new(
        store.clone(),
        Arc::new(GovernedFileProviderSuggestionCandidateEvidenceSource::new(
            artifacts,
        )),
        Arc::new(PostgresCustomerEnrichmentSuggestionMaterializationWorker::new(store)),
        ActorId::try_new(ACTOR_ID).unwrap(),
    )
    .expect("compose materialization event process")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EvidenceCounts {
    records: i64,
    events: i64,
    audits: i64,
    idempotency: i64,
    transactions: i64,
}

struct Fixture {
""",
)
replace_once(
    path,
    """fn candidate() -> wire::ProviderSuggestionCandidate {
""",
    """fn prior_suggestion(fixture: &Fixture) -> Suggestion {
    let mut request = EnrichmentRequest::create(EnrichmentRequestDraft {
        tenant_id: TenantId::try_new(TENANT_ID).unwrap(),
        requested_by: ActorId::try_new(ACTOR_ID).unwrap(),
        idempotency_key: IdempotencyKey::try_new("materialization-event-prior-request").unwrap(),
        target: fixture.request.target().clone(),
        provider_profile_version_id: fixture.profile.version_id().clone(),
        mapping_version_id: fixture.mapping.version_id().clone(),
        requested_fields: vec![TargetField::PartyDisplayName],
        policy_evidence: RequestPolicyEvidence::try_new(
            "customer_profile_enrichment",
            "legitimate_interest",
            Some("consent-materialization-event-prior".to_owned()),
            "materialization-event-policy-v1",
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
        provider_profile_version_id: fixture.profile.version_id().clone(),
        mapping_version_id: fixture.mapping.version_id().clone(),
        replay_key: "materialization-event-prior-replay".to_owned(),
        provider_correlation_id: Some("materialization-event-prior-correlation".to_owned()),
        response_class: ProviderResponseClass::Success,
        canonical_response_digest: [82; 32],
        provider_observed_at_unix_ms: Some(19),
        retrieved_at_unix_ms: 25,
        metered_units: 1,
        protected_evidence_reference: Some("materialization-event-prior-evidence".to_owned()),
    })
    .unwrap();
    Suggestion::materialize(SuggestionDraft {
        request_id: request.request_id().clone(),
        response_receipt_id: receipt.receipt_id().clone(),
        provider_profile_version_id: fixture.profile.version_id().clone(),
        mapping_version_id: fixture.mapping.version_id().clone(),
        target: request.target().clone(),
        proposed_value: "Prior Reviewed Company".to_owned(),
        observed_at_unix_ms: Some(19),
        retrieved_at_unix_ms: 25,
        effective_at_unix_ms: 19,
        fresh_until_unix_ms: 1_000,
        expires_at_unix_ms: 1_500,
        confidence_basis_points: Some(8_500),
        purpose_code: "customer_profile_enrichment".to_owned(),
        legal_basis_code: "legitimate_interest".to_owned(),
        license_id: "Registry materialization event licence".to_owned(),
        permitted_use_class: "customer_master_review".to_owned(),
        residency_region: "eu".to_owned(),
        retention_days: 30,
        consent_evidence_reference: Some("consent-materialization-event-prior".to_owned()),
        evidence_references: vec!["materialization-event-prior-evidence".to_owned()],
    })
    .unwrap()
}

fn accepted_review(suggestion: &Suggestion) -> ReviewDecision {
    ReviewDecision::decide(
        suggestion,
        ActorId::try_new(ACTOR_ID).unwrap(),
        ReviewDecisionKind::Accepted,
        "review-policy-v1".to_owned(),
        "reviewed_accepted".to_owned(),
        ApprovalRequirement::Required,
        Some("approval-materialization-event-prior".to_owned()),
        35,
        Some(1_000),
    )
    .unwrap()
}

async fn seed_suggestion(
    store: &PostgresDataStore,
    suggestion: &Suggestion,
) -> Result<(), Box<dyn std::error::Error>> {
    seed_record(
        store,
        SeedRecord {
            suffix: "event-prior-suggestion",
            at_unix_ms: 31,
            reference: suggestion_record_ref(suggestion.suggestion_id().as_str())?,
            record_payload: suggestion_persisted_payload(suggestion)?,
            event_type: SUGGESTION_MATERIALIZED_EVENT_TYPE,
            event_payload: support::protobuf_payload(
                MODULE_ID,
                SUGGESTION_MATERIALIZED_EVENT_SCHEMA,
                DataClass::Personal,
                &wire::SuggestionMaterializedEvent {
                    suggestion: Some(suggestion_to_wire(suggestion, None, 31)?),
                },
            )?,
        },
    )
    .await
}

async fn seed_review(
    store: &PostgresDataStore,
    suggestion: &Suggestion,
    review: &ReviewDecision,
) -> Result<(), Box<dyn std::error::Error>> {
    seed_record(
        store,
        SeedRecord {
            suffix: "event-prior-review",
            at_unix_ms: 35,
            reference: review_decision_record_ref(review)?,
            record_payload: review_decision_persisted_payload(review)?,
            event_type: SUGGESTION_REVIEWED_EVENT_TYPE,
            event_payload: support::protobuf_payload(
                MODULE_ID,
                SUGGESTION_REVIEWED_EVENT_SCHEMA,
                DataClass::Personal,
                &wire::SuggestionReviewedEvent {
                    suggestion: Some(suggestion_to_wire(suggestion, Some(review), 35)?),
                    review_decision: Some(review_decision_to_wire(review)?),
                },
            )?,
        },
    )
    .await
}

fn application_request(suggestion: &Suggestion, review: &ReviewDecision) -> CapabilityRequest {
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
    CapabilityRequest {
        context: execution_context(
            "materialization-event-application-request",
            definition.capability_id.as_str(),
            "materialization-event-application-idempotency",
            "materialization-event-application-tx",
            50_000_000,
        ),
        input_hash: semantic_input_hash(&input),
        input,
        approval: None,
    }
}

fn terminal_outcome_request(attempt_id: &str) -> CapabilityRequest {
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
                result: Some(wire::application_outcome::Result::TerminalFailure(
                    wire::ApplicationTerminalFailure {
                        safe_code: "owner_application_terminal".to_owned(),
                    },
                )),
            }),
            recorded_at_unix_ms: 60,
        },
    )
    .unwrap();
    CapabilityRequest {
        context: execution_context(
            "materialization-event-outcome-request",
            definition.capability_id.as_str(),
            "materialization-event-outcome-idempotency",
            "materialization-event-outcome-tx",
            60_000_000,
        ),
        input_hash: semantic_input_hash(&input),
        input,
        approval: None,
    }
}

fn candidate() -> wire::ProviderSuggestionCandidate {
""",
)
replace_once(
    path,
    """async fn suggestion_count(admin: &PgPool) -> i64 {
""",
    """async fn evidence_counts(admin: &PgPool) -> EvidenceCounts {
    EvidenceCounts {
        records: tenant_count(admin, "crm.records").await,
        events: tenant_count(admin, "crm.outbox_events").await,
        audits: tenant_count(admin, "crm.audit_records").await,
        idempotency: tenant_count(admin, "crm.idempotency_records").await,
        transactions: tenant_count(admin, "crm.business_transactions").await,
    }
}

async fn tenant_count(admin: &PgPool, table: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(&format!(
        "SELECT count(*)::bigint FROM {table} WHERE tenant_id = $1"
    ))
    .bind(TENANT_ID)
    .fetch_one(admin)
    .await
    .expect("query tenant evidence count")
}

async fn request_version(admin: &PgPool, fixture: &Fixture) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT version::bigint FROM crm.records WHERE tenant_id = $1 AND record_type = 'customer_enrichment.request' AND record_id = $2",
    )
    .bind(TENANT_ID)
    .bind(fixture.request.request_id().as_str())
    .fetch_one(admin)
    .await
    .expect("query materialization request version")
}

async fn application_attempt_count(admin: &PgPool) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*)::bigint FROM crm.records WHERE tenant_id = $1 AND record_type = 'customer_enrichment.application_attempt'",
    )
    .bind(TENANT_ID)
    .fetch_one(admin)
    .await
    .expect("query application-attempt count")
}

async fn application_attempt_version(admin: &PgPool, attempt_id: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT version::bigint FROM crm.records WHERE tenant_id = $1 AND record_type = 'customer_enrichment.application_attempt' AND record_id = $2",
    )
    .bind(TENANT_ID)
    .bind(attempt_id)
    .fetch_one(admin)
    .await
    .expect("query application-attempt version")
}

async fn suggestion_count(admin: &PgPool) -> i64 {
""",
)
