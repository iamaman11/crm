#![cfg(feature = "postgres-integration")]

use crm_capability_ingress::semantic_input_hash;
use crm_capability_plan_support::{self as support, PersistedPayloadContract};
use crm_capability_runtime::CapabilityRequest;
use crm_core_data::{
    AuditIntent, IdempotencyEvidence, PostgresDataStore, PostgresImmutableFileArtifactStore,
    RecordCreatePlan,
};
use crm_core_events::{
    EventHistoryRequest, ProjectionDocumentWrite, ProjectionEventApplication, ProjectionStore,
};
use crm_core_files::{
    AppendImmutableFileChunk, CreateImmutableFileArtifact, ImmutableFileArtifactStore,
};
use crm_customer_enrichment::{
    ApprovalRequirement, EnrichmentRequest, EnrichmentRequestDraft,
    LIFECYCLE_STATE_RETENTION_POLICY_ID, LIFECYCLE_STATE_SCHEMA_VERSION, MappingDraft,
    MappingNormalization, MappingVersion, PROVIDER_PROCESS_OUTCOME_RESOURCE_TYPE,
    PROVIDER_PROCESS_PROJECTION_ID, PROVIDER_RESPONSE_RECEIPT_RECORD_TYPE,
    PROVIDER_RESPONSE_RECEIPT_STATE_MAXIMUM_BYTES, PROVIDER_RESPONSE_RECEIPT_STATE_SCHEMA_ID,
    ProviderProcessCanonicalOutcome, ProviderProfileDraft, ProviderProfileVersion,
    ProviderResponseClass, ProviderResponseReceipt, ProviderResponseReceiptDraft, RawPayloadPolicy,
    RequestPolicyEvidence, ReviewDecision, ReviewDecisionKind, Suggestion, SuggestionDraft,
    TargetField, TargetSnapshot, encode_provider_response_receipt_state,
    provider_response_receipt_state_descriptor_hash,
};
use crm_customer_enrichment_application_adapter::{
    APPLY_PARTY_DISPLAY_NAME_REQUEST_SCHEMA, RECORD_APPLICATION_OUTCOME_REQUEST_SCHEMA,
    apply_party_display_name_capability_definition,
    record_application_outcome_capability_definition,
};
use crm_customer_enrichment_application_composition::{
    PostgresCustomerEnrichmentApplicationAttemptExecutor,
    PostgresCustomerEnrichmentApplicationOutcomeExecutor,
};
use crm_customer_enrichment_capability_adapter::{
    ENRICHMENT_REQUEST_CREATED_EVENT_SCHEMA, ENRICHMENT_REQUEST_CREATED_EVENT_TYPE,
    MAPPING_PUBLISHED_EVENT_SCHEMA, MAPPING_PUBLISHED_EVENT_TYPE, MODULE_ID,
    PROVIDER_PROFILE_PUBLISHED_EVENT_SCHEMA, PROVIDER_PROFILE_PUBLISHED_EVENT_TYPE,
    enrichment_request_persisted_payload, enrichment_request_record_ref,
    enrichment_request_to_wire, mapping_persisted_payload, mapping_record_ref, mapping_to_wire,
    provider_profile_persisted_payload, provider_profile_record_ref, provider_profile_to_wire,
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
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, CorrelationId,
    DataClass, DomainEvent, EventType, ExecutionContext, FileId, IdempotencyKey,
    ModuleExecutionContext, ModuleId, RecordRef, RequestId, RetentionPolicyId, SchemaVersion,
    SdkError, TenantId, TraceId, TypedPayload,
};
use crm_proto_contracts::crm::{customer::v1 as customer, customer_enrichment::v1 as wire};
use prost::Message;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::sync::Arc;

const TENANT_ID: &str = "tenant-a";
const ACTOR_ID: &str = "actor-a";
const FILE_ID: &str = "materialization-candidate-evidence-process-1";
const SEED_CAPABILITY: &str = "customer_enrichment.materialization.seed";
const SUGGESTION_MATERIALIZED_EVENT_TYPE: &str = "customer_enrichment.suggestion.materialized";
const SUGGESTION_MATERIALIZED_EVENT_SCHEMA: &str =
    "crm.customer_enrichment.v1.SuggestionMaterializedEvent";
const SUGGESTION_REVIEWED_EVENT_TYPE: &str = "customer_enrichment.suggestion.reviewed";
const SUGGESTION_REVIEWED_EVENT_SCHEMA: &str = "crm.customer_enrichment.v1.SuggestionReviewedEvent";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn response_event_waits_for_finalized_evidence_then_materializes_once() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!(
            "skipping PostgreSQL materialization event process because DATABASE_URL is absent"
        );
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let store = PostgresDataStore::connect(&database_url, 6)
        .await
        .expect("connect materialization event-process store");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect materialization event-process evidence reader");
    let fixture = fixture();
    seed_dependencies(&store, &fixture)
        .await
        .expect("seed materialization event-process dependencies");

    let artifacts = Arc::new(PostgresImmutableFileArtifactStore::new(store.clone()));
    let process = materialization_process(store.clone(), artifacts.clone());
    let tenant_id = TenantId::try_new(TENANT_ID).unwrap();

    let pending_choice = process
        .run_cycle(tenant_id.clone(), 45_000_000)
        .await
        .expect_err("response event must wait for the provider canonical choice");
    assert_eq!(
        pending_choice.code,
        "CUSTOMER_ENRICHMENT_PROVIDER_CANONICAL_CHOICE_PENDING"
    );
    assert!(pending_choice.retryable);
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
    assert_eq!(suggestion_count(&admin).await, 0);

    apply_recorded_provider_outcome(&store, &fixture)
        .await
        .expect("apply canonical provider outcome with provider checkpoint");

    let missing = process
        .run_cycle(tenant_id.clone(), 50_000_000)
        .await
        .unwrap_err();
    assert_eq!(
        missing.code,
        "CUSTOMER_ENRICHMENT_SUGGESTION_EVIDENCE_UNAVAILABLE"
    );
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
    assert_eq!(suggestion_count(&admin).await, 0);

    upload_candidate_evidence(artifacts.as_ref(), &fixture)
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

    let outcome_executor = PostgresCustomerEnrichmentApplicationOutcomeExecutor::new(store.clone());
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
    assert_eq!(first.response_events, 1);
    assert_eq!(first.materialized, 1);
    assert_eq!(first.replays, 0);
    assert_eq!(first.skipped_failed_responses, 0);
    assert_eq!(suggestion_count(&admin).await, 2);
    assert_eq!(request_version(&admin, &fixture).await, 2);
    assert_eq!(application_attempt_count(&admin).await, 1);
    assert_eq!(application_attempt_version(&admin, &attempt_id).await, 2);
    let materialized_baseline = evidence_counts(&admin).await;

    let checkpoint = ProjectionStore::projection_checkpoint(
        &store,
        tenant_id.clone(),
        MATERIALIZATION_PROCESS_PROJECTION_ID.to_owned(),
    )
    .await
    .unwrap()
    .expect("materialization checkpoint exists");
    assert_eq!(checkpoint.applied_event_count, 1);

    drop(recovered_process);
    let replay_process = materialization_process(store.clone(), artifacts);
    let replay = replay_process
        .run_cycle(tenant_id, 70_000_000)
        .await
        .expect("checkpointed materialization replay after restart");
    assert_eq!(replay.response_events, 0);
    assert_eq!(replay.materialized, 0);
    assert_eq!(suggestion_count(&admin).await, 2);
    assert_eq!(request_version(&admin, &fixture).await, 2);
    assert_eq!(application_attempt_count(&admin).await, 1);
    assert_eq!(evidence_counts(&admin).await, materialized_baseline);
}

async fn apply_recorded_provider_outcome(
    store: &PostgresDataStore,
    fixture: &Fixture,
) -> Result<(), SdkError> {
    let tenant_id = TenantId::try_new(TENANT_ID).unwrap();
    let page = ProjectionStore::list_event_history(
        store,
        EventHistoryRequest {
            tenant_id: tenant_id.clone(),
            consumer_module_id: ModuleId::try_new(MODULE_ID).unwrap(),
            event_types: vec![EventType::try_new(ENRICHMENT_REQUEST_CREATED_EVENT_TYPE).unwrap()],
            after: None,
            page_size: 100,
        },
    )
    .await?;
    let delivery = page
        .deliveries
        .into_iter()
        .find(|delivery| {
            delivery.aggregate.record_id.as_str() == fixture.request.request_id().as_str()
        })
        .expect("request-created delivery exists");
    let outcome = ProviderProcessCanonicalOutcome::response_recorded(
        fixture.request.request_id().as_str().to_owned(),
        fixture.request.retry_generation(),
        fixture.receipt.receipt_id().as_str().to_owned(),
        delivery.event_id.as_str().to_owned(),
    )?;
    ProjectionStore::apply_projection_event(
        store,
        ProjectionEventApplication {
            projection_id: PROVIDER_PROCESS_PROJECTION_ID.to_owned(),
            writes: vec![ProjectionDocumentWrite {
                resource_type: PROVIDER_PROCESS_OUTCOME_RESOURCE_TYPE.to_owned(),
                resource_id: fixture.request.request_id().as_str().to_owned(),
                source_version: delivery.aggregate_version,
                document: outcome.to_projection_document()?,
            }],
            delivery,
        },
    )
    .await?;
    Ok(())
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
    profile: ProviderProfileVersion,
    mapping: MappingVersion,
    request: EnrichmentRequest,
    receipt: ProviderResponseReceipt,
}

fn fixture() -> Fixture {
    let profile = ProviderProfileVersion::publish(ProviderProfileDraft {
        provider_key: "company_registry_materialization_event".to_owned(),
        adapter_kind: "registry_http_v1".to_owned(),
        adapter_contract_version: "1.0.0".to_owned(),
        supported_target_fields: vec![TargetField::PartyDisplayName],
        purpose_codes: vec!["customer_profile_enrichment".to_owned()],
        license_id: "Registry materialization event licence".to_owned(),
        permitted_use_class: "customer_master_review".to_owned(),
        residency_region: "eu".to_owned(),
        retention_days: 30,
        raw_payload_policy: RawPayloadPolicy::GovernedProtectedEvidence,
        credential_handle_aliases: vec!["registry_materialization_event".to_owned()],
        effective_at_unix_ms: 1,
        expires_at_unix_ms: Some(5_000),
    })
    .unwrap();
    let mapping = MappingVersion::publish(MappingDraft {
        mapping_key: "party_display_name_materialization_event".to_owned(),
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
        requested_by: ActorId::try_new(ACTOR_ID).unwrap(),
        idempotency_key: IdempotencyKey::try_new("materialization-event-domain-request").unwrap(),
        target: TargetSnapshot::try_new(
            "party-materialization-event-1",
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
            Some("consent-materialization-event-1".to_owned()),
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
        provider_profile_version_id: profile.version_id().clone(),
        mapping_version_id: mapping.version_id().clone(),
        replay_key: "materialization-event-provider-replay-1".to_owned(),
        provider_correlation_id: Some("materialization-event-provider-correlation-1".to_owned()),
        response_class: ProviderResponseClass::Success,
        canonical_response_digest: [83; 32],
        provider_observed_at_unix_ms: Some(20),
        retrieved_at_unix_ms: 30,
        metered_units: 1,
        protected_evidence_reference: Some(FILE_ID.to_owned()),
    })
    .unwrap();
    request
        .record_response(receipt.receipt_id().clone(), 30)
        .unwrap();
    Fixture {
        profile,
        mapping,
        request,
        receipt,
    }
}

async fn seed_dependencies(
    store: &PostgresDataStore,
    fixture: &Fixture,
) -> Result<(), Box<dyn std::error::Error>> {
    seed_record(
        store,
        SeedRecord {
            suffix: "event-profile",
            at_unix_ms: 1,
            reference: provider_profile_record_ref(&fixture.profile)?,
            record_payload: provider_profile_persisted_payload(&fixture.profile)?,
            event_type: PROVIDER_PROFILE_PUBLISHED_EVENT_TYPE,
            event_payload: support::protobuf_payload(
                MODULE_ID,
                PROVIDER_PROFILE_PUBLISHED_EVENT_SCHEMA,
                DataClass::Confidential,
                &wire::ProviderProfileVersionPublishedEvent {
                    provider_profile_version: Some(provider_profile_to_wire(&fixture.profile)),
                },
            )?,
        },
    )
    .await?;
    seed_record(
        store,
        SeedRecord {
            suffix: "event-mapping",
            at_unix_ms: 2,
            reference: mapping_record_ref(&fixture.mapping)?,
            record_payload: mapping_persisted_payload(&fixture.mapping)?,
            event_type: MAPPING_PUBLISHED_EVENT_TYPE,
            event_payload: support::protobuf_payload(
                MODULE_ID,
                MAPPING_PUBLISHED_EVENT_SCHEMA,
                DataClass::Confidential,
                &wire::MappingVersionPublishedEvent {
                    mapping_version: Some(mapping_to_wire(&fixture.mapping)),
                },
            )?,
        },
    )
    .await?;
    seed_record(
        store,
        SeedRecord {
            suffix: "event-request",
            at_unix_ms: 3,
            reference: enrichment_request_record_ref(&fixture.request)?,
            record_payload: enrichment_request_persisted_payload(&fixture.request)?,
            event_type: ENRICHMENT_REQUEST_CREATED_EVENT_TYPE,
            event_payload: support::protobuf_payload(
                MODULE_ID,
                ENRICHMENT_REQUEST_CREATED_EVENT_SCHEMA,
                DataClass::Personal,
                &wire::EnrichmentRequestCreatedEvent {
                    enrichment_request: Some(enrichment_request_to_wire(&fixture.request)?),
                },
            )?,
        },
    )
    .await?;
    seed_record(
        store,
        SeedRecord {
            suffix: "event-receipt",
            at_unix_ms: 40,
            reference: receipt_record_ref(&fixture.receipt)?,
            record_payload: receipt_persisted_payload(&fixture.receipt)?,
            event_type: PROVIDER_RESPONSE_RECORDED_EVENT_TYPE,
            event_payload: support::protobuf_payload(
                MODULE_ID,
                PROVIDER_RESPONSE_RECORDED_EVENT_SCHEMA,
                DataClass::Personal,
                &wire::ProviderResponseRecordedEvent {
                    provider_response_receipt: Some(receipt_to_wire(fixture)),
                },
            )?,
        },
    )
    .await?;
    Ok(())
}

async fn upload_candidate_evidence(
    artifacts: &PostgresImmutableFileArtifactStore,
    fixture: &Fixture,
) -> Result<(), SdkError> {
    let command = wire::MaterializeSuggestionsRequest {
        enrichment_request_ref: Some(wire::EnrichmentRequestRef {
            enrichment_request_id: fixture.request.request_id().as_str().to_owned(),
        }),
        provider_response_receipt_ref: Some(wire::ProviderResponseReceiptRef {
            provider_response_receipt_id: fixture.receipt.receipt_id().as_str().to_owned(),
        }),
        candidates: vec![candidate()],
    };
    let bytes = command.encode_to_vec();
    let digest: [u8; 32] = Sha256::digest(&bytes).into();
    let context = artifact_context();
    let file_id = FileId::try_new(FILE_ID).unwrap();
    artifacts
        .create(
            &context,
            CreateImmutableFileArtifact {
                file_id: file_id.clone(),
                owner_module_id: ModuleId::try_new(MODULE_ID).unwrap(),
                media_type: PROVIDER_SUGGESTION_CANDIDATE_EVIDENCE_MEDIA_TYPE.to_owned(),
                data_class: DataClass::Personal,
                retention_policy_id: RetentionPolicyId::try_new(
                    "crm.customer_enrichment.provider_suggestion_evidence",
                )
                .unwrap(),
                expected_size_bytes: bytes.len() as u64,
                expected_sha256: digest,
            },
        )
        .await?;
    artifacts
        .append_chunk(
            &context,
            AppendImmutableFileChunk {
                file_id: file_id.clone(),
                chunk_index: 0,
                chunk_sha256: digest,
                bytes,
            },
        )
        .await?;
    artifacts.finalize(&context, &file_id).await?;
    Ok(())
}

fn prior_suggestion(fixture: &Fixture) -> Suggestion {
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
    wire::ProviderSuggestionCandidate {
        target: Some(wire::EnrichmentTargetSnapshot {
            party_ref: Some(customer::PartyRef {
                party_id: "party-materialization-event-1".to_owned(),
            }),
            party_resource_version: 7,
            target_field: wire::EnrichmentTargetField::PartyDisplayName as i32,
        }),
        proposed_value: "Materialized Event Company".to_owned(),
        observed_at_unix_ms: Some(20),
        effective_at_unix_ms: 20,
        fresh_until_unix_ms: 1_000,
        expires_at_unix_ms: 1_500,
        confidence_basis_points: Some(9_000),
        policy_evidence: Some(wire::ProviderPolicyEvidence {
            license_id: "Registry materialization event licence".to_owned(),
            permitted_use_class: "customer_master_review".to_owned(),
            residency_region: "eu".to_owned(),
            retention_days: 30,
            consent_evidence_reference: Some("consent-materialization-event-1".to_owned()),
        }),
        evidence_references: vec![FILE_ID.to_owned()],
    }
}

struct SeedRecord {
    suffix: &'static str,
    at_unix_ms: u64,
    reference: RecordRef,
    record_payload: TypedPayload,
    event_type: &'static str,
    event_payload: TypedPayload,
}

async fn seed_record(
    store: &PostgresDataStore,
    seed: SeedRecord,
) -> Result<(), Box<dyn std::error::Error>> {
    let request_hash = semantic_input_hash(&seed.event_payload);
    let at_unix_nanos = i64::try_from(seed.at_unix_ms * 1_000_000).unwrap();
    store
        .create_record(&RecordCreatePlan {
            context: seed_context(seed.suffix, at_unix_nanos),
            record: seed.reference.clone(),
            record_payload: seed.record_payload,
            event_id: format!("materialization-process-seed-event-{}", seed.suffix),
            event: DomainEvent {
                event_type: EventType::try_new(seed.event_type).unwrap(),
                aggregate: seed.reference,
                expected_aggregate_version: None,
                deduplication_key: format!("materialization-process-seed-{}", seed.suffix),
                payload: seed.event_payload,
            },
            idempotency: IdempotencyEvidence {
                scope: format!("{SEED_CAPABILITY}@1.0.0"),
                key: format!("materialization-process-seed-{}", seed.suffix),
                request_hash,
                expires_at_unix_nanos: 86_400_000_000_000 + at_unix_nanos,
            },
            audit: AuditIntent {
                audit_record_id: format!("materialization-process-seed-audit-{}", seed.suffix),
                canonicalization_profile: "crm.cjson/v1".to_owned(),
                canonical_envelope: format!("{{\"seed\":\"{}\"}}", seed.suffix).into_bytes(),
                occurred_at_unix_nanos: at_unix_nanos,
            },
        })
        .await?;
    Ok(())
}

fn receipt_persisted_payload(receipt: &ProviderResponseReceipt) -> Result<TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        PersistedPayloadContract {
            owner: MODULE_ID,
            schema_id: PROVIDER_RESPONSE_RECEIPT_STATE_SCHEMA_ID,
            schema_version: LIFECYCLE_STATE_SCHEMA_VERSION,
            descriptor_hash: provider_response_receipt_state_descriptor_hash(),
            maximum_size_bytes: PROVIDER_RESPONSE_RECEIPT_STATE_MAXIMUM_BYTES,
            retention_policy_id: LIFECYCLE_STATE_RETENTION_POLICY_ID,
        },
        DataClass::Personal,
        encode_provider_response_receipt_state(receipt)?,
    )
}

fn receipt_record_ref(receipt: &ProviderResponseReceipt) -> Result<RecordRef, SdkError> {
    support::record_ref(
        PROVIDER_RESPONSE_RECEIPT_RECORD_TYPE,
        receipt.receipt_id().as_str(),
        "customer_enrichment.provider_response_receipt_ref.provider_response_receipt_id",
    )
}

fn receipt_to_wire(fixture: &Fixture) -> wire::ProviderResponseReceipt {
    wire::ProviderResponseReceipt {
        provider_response_receipt_ref: Some(wire::ProviderResponseReceiptRef {
            provider_response_receipt_id: fixture.receipt.receipt_id().as_str().to_owned(),
        }),
        enrichment_request_ref: Some(wire::EnrichmentRequestRef {
            enrichment_request_id: fixture.request.request_id().as_str().to_owned(),
        }),
        provider_profile_version_ref: Some(wire::ProviderProfileVersionRef {
            provider_profile_version_id: fixture.profile.version_id().as_str().to_owned(),
        }),
        mapping_version_ref: Some(wire::MappingVersionRef {
            mapping_version_id: fixture.mapping.version_id().as_str().to_owned(),
        }),
        replay_key: "materialization-event-provider-replay-1".to_owned(),
        provider_correlation_id: Some("materialization-event-provider-correlation-1".to_owned()),
        response_class: wire::ProviderResponseClass::Success as i32,
        canonical_response_digest: vec![83; 32],
        provider_observed_at_unix_ms: Some(20),
        retrieved_at_unix_ms: 30,
        metered_units: 1,
        protected_evidence_reference: Some(FILE_ID.to_owned()),
    }
}

fn seed_context(suffix: &str, started_at_unix_nanos: i64) -> ModuleExecutionContext {
    execution_context(
        &format!("materialization-process-seed-request-{suffix}"),
        SEED_CAPABILITY,
        &format!("materialization-process-seed-idempotency-{suffix}"),
        &format!("materialization-process-seed-tx-{suffix}"),
        started_at_unix_nanos,
    )
}

fn artifact_context() -> ModuleExecutionContext {
    execution_context(
        "materialization-process-artifact-request",
        "customer_enrichment.suggestion.evidence.store",
        "materialization-process-artifact-idempotency",
        "materialization-process-artifact-tx",
        50_000_000,
    )
}

fn execution_context(
    request_id: &str,
    capability_id: &str,
    idempotency_key: &str,
    transaction_id: &str,
    started_at_unix_nanos: i64,
) -> ModuleExecutionContext {
    ModuleExecutionContext {
        module_id: ModuleId::try_new(MODULE_ID).unwrap(),
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

async fn evidence_counts(admin: &PgPool) -> EvidenceCounts {
    EvidenceCounts {
        records: sqlx::query_scalar::<_, i64>(
            "SELECT count(*)::bigint FROM crm.records WHERE tenant_id = $1",
        )
        .bind(TENANT_ID)
        .fetch_one(admin)
        .await
        .expect("query tenant record count"),
        events: sqlx::query_scalar::<_, i64>(
            "SELECT count(*)::bigint FROM crm.outbox_events WHERE tenant_id = $1",
        )
        .bind(TENANT_ID)
        .fetch_one(admin)
        .await
        .expect("query tenant event count"),
        audits: sqlx::query_scalar::<_, i64>(
            "SELECT count(*)::bigint FROM crm.audit_records WHERE tenant_id = $1",
        )
        .bind(TENANT_ID)
        .fetch_one(admin)
        .await
        .expect("query tenant audit count"),
        idempotency: sqlx::query_scalar::<_, i64>(
            "SELECT count(*)::bigint FROM crm.idempotency_records WHERE tenant_id = $1",
        )
        .bind(TENANT_ID)
        .fetch_one(admin)
        .await
        .expect("query tenant idempotency count"),
        transactions: sqlx::query_scalar::<_, i64>(
            "SELECT count(*)::bigint FROM crm.business_transactions WHERE tenant_id = $1",
        )
        .bind(TENANT_ID)
        .fetch_one(admin)
        .await
        .expect("query tenant transaction count"),
    }
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
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*)::bigint FROM crm.records WHERE tenant_id = $1 AND record_type = 'customer_enrichment.suggestion'",
    )
    .bind(TENANT_ID)
    .fetch_one(admin)
    .await
    .expect("query materialization event-process suggestions")
}
