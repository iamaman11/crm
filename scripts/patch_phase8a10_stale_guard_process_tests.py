from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one exact anchor, found {count}")
    file.write_text(text.replace(old, new, 1))


review_test = "crates/crm-customer-enrichment-review-composition/tests/postgres_review_process.rs"
replace_once(
    review_test,
    """    ActorId, CapabilityId, CapabilityVersion, CorrelationId, DataClass, ModuleId, PortFuture,
    RecordRef, RequestId, SchemaVersion, SdkError, TenantId, TraceId,
""",
    """    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CorrelationId, DataClass,
    IdempotencyKey, ModuleId, PortFuture, RecordRef, RequestId, SchemaVersion, SdkError, TenantId,
    TraceId,
""",
)
replace_once(
    review_test,
    """    seed_suggestion_with_suffix(&query_store, &refreshed, "refreshed")
        .await
        .expect("seed refreshed immutable suggestion");

    let superseded_result = visible_queries
""",
    """    seed_suggestion_with_suffix(&query_store, &refreshed, "refreshed")
        .await
        .expect("seed refreshed immutable suggestion");

    let mut stale_review_request = accept_request(&suggestion);
    stale_review_request.context.execution.request_id =
        RequestId::try_new("review-request-superseded").unwrap();
    stale_review_request.context.execution.correlation_id =
        CorrelationId::try_new("correlation-review-request-superseded").unwrap();
    stale_review_request.context.execution.causation_id =
        crm_module_sdk::CausationId::try_new("causation-review-request-superseded").unwrap();
    stale_review_request.context.execution.trace_id =
        TraceId::try_new("trace-review-request-superseded").unwrap();
    stale_review_request.context.execution.idempotency_key =
        IdempotencyKey::try_new("review-idempotency-superseded").unwrap();
    stale_review_request.context.execution.business_transaction_id =
        BusinessTransactionId::try_new("review-tx-superseded").unwrap();
    stale_review_request.input_hash = [43; 32];
    let stale_review = executor
        .execute(stale_review_request)
        .await
        .expect_err("superseded suggestion must fail before review persistence");
    assert_eq!(
        stale_review.code,
        "CUSTOMER_ENRICHMENT_SUGGESTION_SUPERSEDED"
    );

    let superseded_result = visible_queries
""",
)

application_test = (
    "crates/crm-customer-enrichment-application-composition/tests/"
    "postgres_application_process.rs"
)
replace_once(
    application_test,
    """    let conflicting = outcome_request(
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
""",
    """    let conflicting = outcome_request(
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

    let refreshed = refreshed_suggestion();
    seed_suggestion_with_suffix(&attempt_executor.store, &refreshed, "refreshed")
        .await
        .expect("seed refreshed immutable application suggestion");
    let stale_application = attempt_executor
        .execute(apply_request(
            &suggestion,
            &review,
            "application-request-superseded",
            "application-idempotency-superseded",
            "application-tx-superseded",
        ))
        .await
        .expect_err("superseded suggestion must fail before application persistence");
    assert_eq!(
        stale_application.code,
        "CUSTOMER_ENRICHMENT_SUGGESTION_SUPERSEDED"
    );

    assert_eq!(
""",
)

# The executor store is private; retain an explicit clone for the successor seed.
replace_once(
    application_test,
    """    let attempt_executor = PostgresCustomerEnrichmentApplicationAttemptExecutor::new(store.clone());
    let apply = apply_request(
""",
    """    let application_store = store.clone();
    let attempt_executor = PostgresCustomerEnrichmentApplicationAttemptExecutor::new(store.clone());
    let apply = apply_request(
""",
)
replace_once(
    application_test,
    "seed_suggestion_with_suffix(&attempt_executor.store, &refreshed, \"refreshed\")",
    "seed_suggestion_with_suffix(&application_store, &refreshed, \"refreshed\")",
)

# Make seed identities suffix-safe so the existing test can persist a successor.
replace_once(
    application_test,
    """async fn seed_suggestion(
    store: &PostgresDataStore,
    suggestion: &Suggestion,
) -> Result<(), Box<dyn std::error::Error>> {
""",
    """async fn seed_suggestion(
    store: &PostgresDataStore,
    suggestion: &Suggestion,
) -> Result<(), Box<dyn std::error::Error>> {
    seed_suggestion_with_suffix(store, suggestion, "suggestion").await
}

async fn seed_suggestion_with_suffix(
    store: &PostgresDataStore,
    suggestion: &Suggestion,
    suffix: &str,
) -> Result<(), Box<dyn std::error::Error>> {
""",
)
for old, new in [
    ('"application-seed-suggestion",', ' &format!("application-seed-suggestion-{suffix}"),'),
    ('"application-seed-suggestion-event",', ' &format!("application-seed-suggestion-event-{suffix}"),'),
    ('"application-seed-suggestion-audit",', ' &format!("application-seed-suggestion-audit-{suffix}"),'),
    ('"application-seed-suggestion-idempotency",', ' &format!("application-seed-suggestion-idempotency-{suffix}"),'),
    ('"application-seed-suggestion-tx",', ' &format!("application-seed-suggestion-tx-{suffix}"),'),
]:
    replace_once(application_test, old, new)

# Parameterize the deterministic fixture to produce a same-coordinate newer successor.
replace_once(
    application_test,
    """fn suggestion() -> Suggestion {
    let profile = ProviderProfileVersion::publish(ProviderProfileDraft {
""",
    """fn suggestion() -> Suggestion {
    suggestion_at(
        "application-domain-request",
        "application-provider-replay-1",
        30,
        7,
    )
}

fn refreshed_suggestion() -> Suggestion {
    suggestion_at(
        "application-domain-request-refreshed",
        "application-provider-replay-2",
        45,
        8,
    )
}

fn suggestion_at(
    request_key: &str,
    replay_key: &str,
    retrieved_at_unix_ms: u64,
    party_resource_version: u64,
) -> Suggestion {
    let profile = ProviderProfileVersion::publish(ProviderProfileDraft {
""",
)
replace_once(
    application_test,
    'idempotency_key: IdempotencyKey::try_new("application-domain-request").unwrap(),',
    'idempotency_key: IdempotencyKey::try_new(request_key).unwrap(),',
)
replace_once(
    application_test,
    'target: TargetSnapshot::try_new("party-application-1", 7, TargetField::PartyDisplayName)',
    'target: TargetSnapshot::try_new(\n            "party-application-1",\n            party_resource_version,\n            TargetField::PartyDisplayName,\n        )',
)
replace_once(
    application_test,
    'replay_key: "application-provider-replay-1".to_owned(),',
    'replay_key: replay_key.to_owned(),',
)
replace_once(
    application_test,
    'provider_correlation_id: Some("application-provider-correlation-1".to_owned()),',
    'provider_correlation_id: Some(format!("application-provider-correlation-{replay_key}")),',
)
replace_once(
    application_test,
    'canonical_response_digest: [82; 32],',
    'canonical_response_digest: [u8::try_from(retrieved_at_unix_ms).unwrap(); 32],',
)
replace_once(
    application_test,
    'provider_observed_at_unix_ms: Some(20),\n        retrieved_at_unix_ms: 30,',
    'provider_observed_at_unix_ms: Some(retrieved_at_unix_ms - 1),\n        retrieved_at_unix_ms,',
)
replace_once(
    application_test,
    'observed_at_unix_ms: Some(20),\n        retrieved_at_unix_ms: 30,\n        effective_at_unix_ms: 20,',
    'observed_at_unix_ms: Some(retrieved_at_unix_ms - 1),\n        retrieved_at_unix_ms,\n        effective_at_unix_ms: retrieved_at_unix_ms - 1,',
)
replace_once(
    application_test,
    'protected_evidence_reference: Some("application-evidence-1".to_owned()),',
    'protected_evidence_reference: Some(format!("application-evidence-{replay_key}")),',
)
replace_once(
    application_test,
    'evidence_references: vec!["application-evidence-1".to_owned()],',
    'evidence_references: vec![format!("application-evidence-{replay_key}")],',
)
