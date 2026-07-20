#![cfg(feature = "postgres-integration")]

use crm_capability_ingress::semantic_input_hash;
use crm_capability_plan_support::{self as support, PersistedPayloadContract};
use crm_core_data::{
    AuditIntent, IdempotencyEvidence, PostgresDataStore, PostgresImmutableFileArtifactStore,
    RecordCreatePlan,
};
use crm_core_events::ProjectionStore;
use crm_core_files::{
    AppendImmutableFileChunk, CreateImmutableFileArtifact, ImmutableFileArtifactStore,
};
use crm_customer_enrichment::{
    EnrichmentRequest, EnrichmentRequestDraft, LIFECYCLE_STATE_RETENTION_POLICY_ID,
    LIFECYCLE_STATE_SCHEMA_VERSION, MappingDraft, MappingNormalization, MappingVersion,
    PROVIDER_RESPONSE_RECEIPT_RECORD_TYPE, PROVIDER_RESPONSE_RECEIPT_STATE_MAXIMUM_BYTES,
    PROVIDER_RESPONSE_RECEIPT_STATE_SCHEMA_ID, ProviderProfileDraft, ProviderProfileVersion,
    ProviderResponseClass, ProviderResponseReceipt, ProviderResponseReceiptDraft, RawPayloadPolicy,
    RequestPolicyEvidence, TargetField, TargetSnapshot, encode_provider_response_receipt_state,
    provider_response_receipt_state_descriptor_hash,
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
    MATERIALIZATION_PROCESS_WORKER_ACTOR_ID, PROVIDER_RESPONSE_RECORDED_EVENT_SCHEMA,
    PROVIDER_RESPONSE_RECORDED_EVENT_TYPE, PROVIDER_SUGGESTION_CANDIDATE_EVIDENCE_MEDIA_TYPE,
    PostgresCustomerEnrichmentSuggestionMaterializationWorker,
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
const ACTOR_ID: &str = "materialization-process-seed-actor";
const FILE_ID: &str = "materialization-candidate-evidence-process-1";
const SEED_CAPABILITY: &str = "customer_enrichment.materialization.process.seed";

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
    let process = CustomerEnrichmentMaterializationProcessWorker::new(
        store.clone(),
        Arc::new(GovernedFileProviderSuggestionCandidateEvidenceSource::new(
            artifacts.clone(),
        )),
        Arc::new(PostgresCustomerEnrichmentSuggestionMaterializationWorker::new(store.clone())),
        ActorId::try_new(MATERIALIZATION_PROCESS_WORKER_ACTOR_ID).unwrap(),
    )
    .expect("compose materialization event process");
    let tenant_id = TenantId::try_new(TENANT_ID).unwrap();

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

    let first = process
        .run_cycle(tenant_id.clone(), 60_000_000)
        .await
        .expect("materialize response event after evidence recovery");
    assert_eq!(first.response_events, 1);
    assert_eq!(first.materialized, 1);
    assert_eq!(first.replays, 0);
    assert_eq!(first.skipped_failed_responses, 0);
    assert_eq!(suggestion_count(&admin).await, 1);

    let checkpoint = ProjectionStore::projection_checkpoint(
        &store,
        tenant_id.clone(),
        MATERIALIZATION_PROCESS_PROJECTION_ID.to_owned(),
    )
    .await
    .unwrap()
    .expect("materialization checkpoint exists");
    assert_eq!(checkpoint.applied_event_count, 1);

    let replay = process
        .run_cycle(tenant_id, 70_000_000)
        .await
        .expect("checkpointed materialization replay");
    assert_eq!(replay.response_events, 0);
    assert_eq!(replay.materialized, 0);
    assert_eq!(suggestion_count(&admin).await, 1);
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
            at_unix_ms: 4,
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

async fn suggestion_count(admin: &PgPool) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*)::bigint FROM crm.records WHERE tenant_id = $1 AND record_type = 'customer_enrichment.suggestion'",
    )
    .bind(TENANT_ID)
    .fetch_one(admin)
    .await
    .expect("query materialization event-process suggestions")
}
