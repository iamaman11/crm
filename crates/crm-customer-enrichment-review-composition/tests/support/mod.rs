use crm_capability_ingress::semantic_input_hash;
use crm_capability_plan_support as support;
use crm_capability_runtime::CapabilityRequest;
use crm_core_data::{AuditIntent, IdempotencyEvidence, PostgresDataStore, RecordCreatePlan};
use crm_customer_enrichment::{
    EnrichmentRequest, EnrichmentRequestDraft, MappingDraft, MappingNormalization, MappingVersion,
    ProviderProfileDraft, ProviderProfileVersion, ProviderResponseClass, ProviderResponseReceipt,
    ProviderResponseReceiptDraft, RawPayloadPolicy, RequestPolicyEvidence, Suggestion,
    SuggestionDraft, TargetField, TargetSnapshot,
};
use crm_customer_enrichment_capability_adapter::MODULE_ID;
use crm_customer_enrichment_review_adapter::{
    ACCEPT_SUGGESTION_REQUEST_SCHEMA, accept_suggestion_capability_definition,
    suggestion_persisted_payload, suggestion_record_ref, suggestion_to_wire,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, CorrelationId,
    DataClass, DomainEvent, EventType, ExecutionContext, IdempotencyKey, ModuleExecutionContext,
    ModuleId, RequestId, SchemaVersion, TenantId, TraceId,
};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;

pub const TENANT_ID: &str = "tenant-a";
pub const ACTOR_ID: &str = "reviewer-a";
const SEED_CAPABILITY: &str = "customer_enrichment.review.seed";
const SUGGESTION_MATERIALIZED_EVENT_TYPE: &str = "customer_enrichment.suggestion.materialized";
const SUGGESTION_MATERIALIZED_EVENT_SCHEMA: &str =
    "crm.customer_enrichment.v1.SuggestionMaterializedEvent";

pub fn suggestion() -> Suggestion {
    suggestion_at(
        "review-domain-request",
        "review-provider-replay-1",
        30,
        1_500,
        7,
    )
}

pub fn refreshed_suggestion() -> Suggestion {
    suggestion_at(
        "review-domain-request-refreshed",
        "review-provider-replay-2",
        45,
        2_000,
        8,
    )
}

fn suggestion_at(
    request_key: &str,
    replay_key: &str,
    retrieved_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    party_resource_version: u64,
) -> Suggestion {
    let profile = ProviderProfileVersion::publish(ProviderProfileDraft {
        provider_key: "review-registry".to_owned(),
        adapter_kind: "review-http-v1".to_owned(),
        adapter_contract_version: "1.0.0".to_owned(),
        supported_target_fields: vec![TargetField::PartyDisplayName],
        purpose_codes: vec!["customer_profile_enrichment".to_owned()],
        license_id: "Review registry licence".to_owned(),
        permitted_use_class: "customer_master_review".to_owned(),
        residency_region: "eu".to_owned(),
        retention_days: 30,
        raw_payload_policy: RawPayloadPolicy::GovernedProtectedEvidence,
        credential_handle_aliases: vec!["review_registry_primary".to_owned()],
        effective_at_unix_ms: 1,
        expires_at_unix_ms: Some(5_000),
    })
    .unwrap();
    let mapping = MappingVersion::publish(MappingDraft {
        mapping_key: "review_party_display_name".to_owned(),
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
        requested_by: ActorId::try_new("worker-a").unwrap(),
        idempotency_key: IdempotencyKey::try_new(request_key).unwrap(),
        target: TargetSnapshot::try_new(
            "party-review-1",
            party_resource_version,
            TargetField::PartyDisplayName,
        )
        .unwrap(),
        provider_profile_version_id: profile.version_id().clone(),
        mapping_version_id: mapping.version_id().clone(),
        requested_fields: vec![TargetField::PartyDisplayName],
        policy_evidence: RequestPolicyEvidence::try_new(
            "customer_profile_enrichment",
            "legitimate_interest",
            Some("consent-review-1".to_owned()),
            "request-policy-v1",
        )
        .unwrap(),
        created_at_unix_ms: 1,
        deadline_at_unix_ms: 1_000,
        expires_at_unix_ms: 2_500,
    })
    .unwrap();
    request.queue(10).unwrap();
    request.mark_dispatched(10).unwrap();
    let receipt = ProviderResponseReceipt::record(ProviderResponseReceiptDraft {
        request_id: request.request_id().clone(),
        provider_profile_version_id: profile.version_id().clone(),
        mapping_version_id: mapping.version_id().clone(),
        replay_key: replay_key.to_owned(),
        provider_correlation_id: Some(format!("correlation-{replay_key}")),
        response_class: ProviderResponseClass::Success,
        canonical_response_digest: [u8::try_from(retrieved_at_unix_ms).unwrap(); 32],
        provider_observed_at_unix_ms: Some(retrieved_at_unix_ms - 1),
        retrieved_at_unix_ms,
        metered_units: 1,
        protected_evidence_reference: Some(format!("evidence-{replay_key}")),
    })
    .unwrap();
    Suggestion::materialize(SuggestionDraft {
        request_id: request.request_id().clone(),
        response_receipt_id: receipt.receipt_id().clone(),
        provider_profile_version_id: profile.version_id().clone(),
        mapping_version_id: mapping.version_id().clone(),
        target: request.target().clone(),
        proposed_value: "Reviewed Company".to_owned(),
        observed_at_unix_ms: Some(retrieved_at_unix_ms - 1),
        retrieved_at_unix_ms,
        effective_at_unix_ms: retrieved_at_unix_ms,
        fresh_until_unix_ms: 1_000,
        expires_at_unix_ms,
        confidence_basis_points: Some(9_000),
        purpose_code: "customer_profile_enrichment".to_owned(),
        legal_basis_code: "legitimate_interest".to_owned(),
        license_id: "Review registry licence".to_owned(),
        permitted_use_class: "customer_master_review".to_owned(),
        residency_region: "eu".to_owned(),
        retention_days: 30,
        consent_evidence_reference: Some("consent-review-1".to_owned()),
        evidence_references: vec![format!("evidence-{replay_key}")],
    })
    .unwrap()
}

pub async fn seed_suggestion(
    store: &PostgresDataStore,
    suggestion: &Suggestion,
) -> Result<(), Box<dyn std::error::Error>> {
    seed_suggestion_with_suffix(store, suggestion, "suggestion").await
}

pub async fn seed_suggestion_with_suffix(
    store: &PostgresDataStore,
    suggestion: &Suggestion,
    suffix: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let reference = suggestion_record_ref(suggestion.suggestion_id().as_str())?;
    let event_payload = support::protobuf_payload(
        MODULE_ID,
        SUGGESTION_MATERIALIZED_EVENT_SCHEMA,
        DataClass::Personal,
        &wire::SuggestionMaterializedEvent {
            suggestion: Some(suggestion_to_wire(suggestion, None, 50)?),
        },
    )?;
    let request_hash = semantic_input_hash(&event_payload);
    store
        .create_record(&RecordCreatePlan {
            context: context(
                &format!("review-seed-request-{suffix}"),
                SEED_CAPABILITY,
                &format!("review-seed-idempotency-{suffix}"),
                &format!("review-seed-tx-{suffix}"),
                50_000_000,
            ),
            record: reference.clone(),
            record_payload: suggestion_persisted_payload(suggestion)?,
            event_id: format!("review-seed-event-{suffix}"),
            event: DomainEvent {
                event_type: EventType::try_new(SUGGESTION_MATERIALIZED_EVENT_TYPE)?,
                aggregate: reference,
                expected_aggregate_version: None,
                deduplication_key: format!("review-seed-event-{suffix}"),
                payload: event_payload,
            },
            idempotency: IdempotencyEvidence {
                scope: format!("{SEED_CAPABILITY}@1.0.0"),
                key: format!("review-seed-idempotency-{suffix}"),
                request_hash,
                expires_at_unix_nanos: 86_400_050_000_000,
            },
            audit: AuditIntent {
                audit_record_id: format!("review-seed-audit-{suffix}"),
                canonicalization_profile: "crm.cjson/v1".to_owned(),
                canonical_envelope: format!("{{\"seed\":\"{suffix}\"}}").into_bytes(),
                occurred_at_unix_nanos: 50_000_000,
            },
        })
        .await?;
    Ok(())
}

pub fn accept_request(suggestion: &Suggestion) -> CapabilityRequest {
    let definition = accept_suggestion_capability_definition().unwrap();
    let input = support::protobuf_payload(
        MODULE_ID,
        ACCEPT_SUGGESTION_REQUEST_SCHEMA,
        DataClass::Personal,
        &wire::AcceptSuggestionRequest {
            suggestion_ref: Some(wire::SuggestionRef {
                suggestion_id: suggestion.suggestion_id().as_str().to_owned(),
            }),
            expected_party_resource_version: 7,
            expected_proposed_value_digest: suggestion.proposed_value_digest().to_vec(),
            policy_version: "review-policy-v1".to_owned(),
            safe_reason_code: "reviewed_accepted".to_owned(),
            approval_evidence_reference: Some("approval-review-1".to_owned()),
            review_expires_at_unix_ms: Some(1_000),
        },
    )
    .unwrap();
    CapabilityRequest {
        context: context(
            "review-request-1",
            definition.capability_id.as_str(),
            "review-idempotency-1",
            "review-tx-1",
            40_000_000,
        ),
        input,
        input_hash: [41; 32],
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
