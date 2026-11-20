use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest};
use crm_core_data::{AggregatePresence, RecordMutation, TransactionalAggregatePlanner};
use crm_customer_enrichment::{
    ApprovalRequirement, EnrichmentRequest, EnrichmentRequestDraft, MappingDraft,
    MappingNormalization, MappingVersion, ProviderProfileDraft, ProviderProfileVersion,
    ProviderResponseClass, ProviderResponseReceipt, ProviderResponseReceiptDraft, RawPayloadPolicy,
    RequestPolicyEvidence, Suggestion, SuggestionDraft, TargetField, TargetSnapshot,
};
use crm_customer_enrichment_capability_adapter::MODULE_ID;
use crm_customer_enrichment_review_adapter::{
    ACCEPT_SUGGESTION_REQUEST_SCHEMA, CustomerEnrichmentSuggestionReviewPlanner,
    REJECT_SUGGESTION_REQUEST_SCHEMA, accept_suggestion_capability_definition,
    reject_suggestion_capability_definition, suggestion_from_snapshot,
    suggestion_persisted_payload,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, CorrelationId,
    DataClass, ExecutionContext, IdempotencyKey, ModuleExecutionContext, ModuleId, RecordId,
    RecordRef, RecordSnapshot, RecordType, RequestId, SchemaVersion, TenantId, TraceId,
};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;
use prost::Message;

#[test]
fn accepted_suggestion_is_one_immutable_atomic_batch() {
    let suggestion = suggestion(150);
    let definition = accept_suggestion_capability_definition().unwrap();
    let request = accept_request(
        &definition,
        &suggestion,
        7,
        digest(&suggestion),
        Some("approval-1"),
        100,
        40,
    );
    let planner =
        CustomerEnrichmentSuggestionReviewPlanner::new(suggestion, ApprovalRequirement::Required);

    let target = planner.target(&definition, &request).unwrap();
    assert_eq!(target.presence, AggregatePresence::MustBeAbsent);
    assert_eq!(
        target.reference.record_type.as_str(),
        "customer_enrichment.review_decision"
    );

    let plan = planner.plan(&definition, &request, None).unwrap();
    plan.batch.validate().unwrap();
    assert_eq!(plan.batch.records.len(), 1);
    assert_eq!(plan.batch.events.len(), 1);
    assert_eq!(plan.batch.audits.len(), 1);
    assert!(plan.batch.relationships.is_empty());
    assert!(matches!(
        &plan.batch.records[0],
        RecordMutation::Create { reference, payload }
            if reference == &target.reference && payload.data_class == DataClass::Personal
    ));

    let output =
        wire::AcceptSuggestionResponse::decode(plan.output.unwrap().bytes.as_slice()).unwrap();
    assert_eq!(
        output.suggestion.unwrap().lifecycle_status,
        wire::SuggestionLifecycleStatus::Accepted as i32
    );
    let decision = output.review_decision.unwrap();
    assert_eq!(
        decision.kind,
        wire::SuggestionReviewDecisionKind::Accepted as i32
    );
    assert_eq!(
        decision.approval_evidence_reference.as_deref(),
        Some("approval-1")
    );
}

#[test]
fn rejected_suggestion_is_one_immutable_atomic_batch_without_approval() {
    let suggestion = suggestion(150);
    let definition = reject_suggestion_capability_definition().unwrap();
    let request = reject_request(&definition, &suggestion, 7, digest(&suggestion), 40);
    let planner =
        CustomerEnrichmentSuggestionReviewPlanner::new(suggestion, ApprovalRequirement::Required);

    let plan = planner.plan(&definition, &request, None).unwrap();
    plan.batch.validate().unwrap();
    let output =
        wire::RejectSuggestionResponse::decode(plan.output.unwrap().bytes.as_slice()).unwrap();
    assert_eq!(
        output.suggestion.unwrap().lifecycle_status,
        wire::SuggestionLifecycleStatus::Rejected as i32
    );
    assert_eq!(
        output.review_decision.unwrap().kind,
        wire::SuggestionReviewDecisionKind::Rejected as i32
    );
}

#[test]
fn acceptance_requires_policy_approval_evidence() {
    let suggestion = suggestion(150);
    let definition = accept_suggestion_capability_definition().unwrap();
    let request = accept_request(
        &definition,
        &suggestion,
        7,
        digest(&suggestion),
        None,
        100,
        40,
    );
    let planner =
        CustomerEnrichmentSuggestionReviewPlanner::new(suggestion, ApprovalRequirement::Required);

    let error = planner.target(&definition, &request).unwrap_err();
    assert_eq!(error.code, "CUSTOMER_ENRICHMENT_APPROVAL_REQUIRED");
}

#[test]
fn stale_party_version_is_rejected_before_locking() {
    let suggestion = suggestion(150);
    let definition = accept_suggestion_capability_definition().unwrap();
    let request = accept_request(
        &definition,
        &suggestion,
        8,
        digest(&suggestion),
        Some("approval-1"),
        100,
        40,
    );
    let planner =
        CustomerEnrichmentSuggestionReviewPlanner::new(suggestion, ApprovalRequirement::Required);

    let error = planner.target(&definition, &request).unwrap_err();
    assert_eq!(error.code, "CUSTOMER_ENRICHMENT_REVIEW_CONFLICT");
}

#[test]
fn changed_value_digest_is_rejected_before_locking() {
    let suggestion = suggestion(150);
    let definition = reject_suggestion_capability_definition().unwrap();
    let request = reject_request(&definition, &suggestion, 7, vec![9; 32], 40);
    let planner = CustomerEnrichmentSuggestionReviewPlanner::new(
        suggestion,
        ApprovalRequirement::NotRequired,
    );

    let error = planner.target(&definition, &request).unwrap_err();
    assert_eq!(error.code, "CUSTOMER_ENRICHMENT_REVIEW_CONFLICT");
}

#[test]
fn expired_suggestion_is_rejected_before_locking() {
    let suggestion = suggestion(50);
    let definition = reject_suggestion_capability_definition().unwrap();
    let request = reject_request(&definition, &suggestion, 7, digest(&suggestion), 50);
    let planner = CustomerEnrichmentSuggestionReviewPlanner::new(
        suggestion,
        ApprovalRequirement::NotRequired,
    );

    let error = planner.target(&definition, &request).unwrap_err();
    assert_eq!(error.code, "CUSTOMER_ENRICHMENT_SUGGESTION_EXPIRED");
}

#[test]
fn strict_snapshot_round_trip_preserves_deterministic_suggestion_identity() {
    let suggestion = suggestion(150);
    let snapshot = RecordSnapshot {
        reference: RecordRef {
            record_type: RecordType::try_new("customer_enrichment.suggestion").unwrap(),
            record_id: RecordId::try_new(suggestion.suggestion_id().as_str()).unwrap(),
        },
        version: 1,
        payload: suggestion_persisted_payload(&suggestion).unwrap(),
    };
    let restored = suggestion_from_snapshot(&snapshot).unwrap();
    assert_eq!(restored, suggestion);
}

fn suggestion(expires_at_unix_ms: u64) -> Suggestion {
    let profile = profile();
    let mapping = mapping(&profile);
    let mut request = EnrichmentRequest::create(EnrichmentRequestDraft {
        tenant_id: TenantId::try_new("tenant-a").unwrap(),
        requested_by: ActorId::try_new("worker-a").unwrap(),
        idempotency_key: IdempotencyKey::try_new("domain-request-1").unwrap(),
        target: TargetSnapshot::try_new("party-a", 7, TargetField::PartyDisplayName).unwrap(),
        provider_profile_version_id: profile.version_id().clone(),
        mapping_version_id: mapping.version_id().clone(),
        requested_fields: vec![TargetField::PartyDisplayName],
        policy_evidence: RequestPolicyEvidence::try_new(
            "customer_profile_enrichment",
            "legitimate_interest",
            Some("consent-a".to_owned()),
            "request-policy-v1",
        )
        .unwrap(),
        created_at_unix_ms: 1,
        deadline_at_unix_ms: 100,
        expires_at_unix_ms: 200,
    })
    .unwrap();
    request.queue(10).unwrap();
    request.mark_dispatched(10).unwrap();
    let receipt = ProviderResponseReceipt::record(ProviderResponseReceiptDraft {
        request_id: request.request_id().clone(),
        provider_profile_version_id: profile.version_id().clone(),
        mapping_version_id: mapping.version_id().clone(),
        replay_key: "provider-replay-1".to_owned(),
        provider_correlation_id: None,
        response_class: ProviderResponseClass::Success,
        canonical_response_digest: [7; 32],
        provider_observed_at_unix_ms: Some(20),
        retrieved_at_unix_ms: 30,
        metered_units: 1,
        protected_evidence_reference: Some("evidence-1".to_owned()),
    })
    .unwrap();
    Suggestion::materialize(SuggestionDraft {
        request_id: request.request_id().clone(),
        response_receipt_id: receipt.receipt_id().clone(),
        provider_profile_version_id: profile.version_id().clone(),
        mapping_version_id: mapping.version_id().clone(),
        target: request.target().clone(),
        proposed_value: "Reviewed Company".to_owned(),
        observed_at_unix_ms: Some(20),
        retrieved_at_unix_ms: 30,
        effective_at_unix_ms: 20,
        fresh_until_unix_ms: expires_at_unix_ms,
        expires_at_unix_ms,
        confidence_basis_points: Some(9_000),
        purpose_code: "customer_profile_enrichment".to_owned(),
        legal_basis_code: "legitimate_interest".to_owned(),
        license_id: "provider-license".to_owned(),
        permitted_use_class: "customer_master_review".to_owned(),
        residency_region: "eu".to_owned(),
        retention_days: 30,
        consent_evidence_reference: Some("consent-a".to_owned()),
        evidence_references: vec!["evidence-1".to_owned()],
    })
    .unwrap()
}

fn profile() -> ProviderProfileVersion {
    ProviderProfileVersion::publish(ProviderProfileDraft {
        provider_key: "provider".to_owned(),
        adapter_kind: "adapter".to_owned(),
        adapter_contract_version: "1.0.0".to_owned(),
        supported_target_fields: vec![TargetField::PartyDisplayName],
        purpose_codes: vec!["customer_profile_enrichment".to_owned()],
        license_id: "provider-license".to_owned(),
        permitted_use_class: "customer_master_review".to_owned(),
        residency_region: "eu".to_owned(),
        retention_days: 30,
        raw_payload_policy: RawPayloadPolicy::DigestOnly,
        credential_handle_aliases: vec!["provider-primary".to_owned()],
        effective_at_unix_ms: 1,
        expires_at_unix_ms: Some(1_000),
    })
    .unwrap()
}

fn mapping(profile: &ProviderProfileVersion) -> MappingVersion {
    MappingVersion::publish(MappingDraft {
        mapping_key: "party_display_name".to_owned(),
        provider_profile_version_id: profile.version_id().clone(),
        provider_response_field_path: "organization.legal_name".to_owned(),
        target_field: TargetField::PartyDisplayName,
        normalization: MappingNormalization::CanonicalPartyDisplayNameV1,
        maximum_suggestions_per_response: 2,
        confidence_required: true,
    })
    .unwrap()
}

fn digest(suggestion: &Suggestion) -> Vec<u8> {
    suggestion.proposed_value_digest().to_vec()
}

fn accept_request(
    definition: &CapabilityDefinition,
    suggestion: &Suggestion,
    expected_version: i64,
    expected_digest: Vec<u8>,
    approval: Option<&str>,
    review_expiry: i64,
    at_unix_ms: i64,
) -> CapabilityRequest {
    request(
        definition,
        ACCEPT_SUGGESTION_REQUEST_SCHEMA,
        &wire::AcceptSuggestionRequest {
            suggestion_ref: Some(wire::SuggestionRef {
                suggestion_id: suggestion.suggestion_id().as_str().to_owned(),
            }),
            expected_party_resource_version: expected_version,
            expected_proposed_value_digest: expected_digest,
            policy_version: "review-policy-v1".to_owned(),
            safe_reason_code: "reviewed-accepted".to_owned(),
            approval_evidence_reference: approval.map(str::to_owned),
            review_expires_at_unix_ms: Some(review_expiry),
        },
        at_unix_ms,
    )
}

fn reject_request(
    definition: &CapabilityDefinition,
    suggestion: &Suggestion,
    expected_version: i64,
    expected_digest: Vec<u8>,
    at_unix_ms: i64,
) -> CapabilityRequest {
    request(
        definition,
        REJECT_SUGGESTION_REQUEST_SCHEMA,
        &wire::RejectSuggestionRequest {
            suggestion_ref: Some(wire::SuggestionRef {
                suggestion_id: suggestion.suggestion_id().as_str().to_owned(),
            }),
            expected_party_resource_version: expected_version,
            expected_proposed_value_digest: expected_digest,
            policy_version: "review-policy-v1".to_owned(),
            safe_reason_code: "reviewed-rejected".to_owned(),
        },
        at_unix_ms,
    )
}

fn request<M: Message>(
    definition: &CapabilityDefinition,
    schema: &'static str,
    message: &M,
    at_unix_ms: i64,
) -> CapabilityRequest {
    let input = support::protobuf_payload(MODULE_ID, schema, DataClass::Personal, message).unwrap();
    CapabilityRequest {
        context: ModuleExecutionContext {
            module_id: ModuleId::try_new(MODULE_ID).unwrap(),
            execution: ExecutionContext {
                tenant_id: TenantId::try_new("tenant-a").unwrap(),
                actor_id: ActorId::try_new("reviewer-a").unwrap(),
                request_id: RequestId::try_new(format!("review-request-{at_unix_ms}")).unwrap(),
                correlation_id: CorrelationId::try_new("correlation-a").unwrap(),
                causation_id: CausationId::try_new("causation-a").unwrap(),
                trace_id: TraceId::try_new("trace-a").unwrap(),
                capability_id: CapabilityId::try_new(definition.capability_id.as_str()).unwrap(),
                capability_version: CapabilityVersion::try_new(
                    definition.capability_version.as_str(),
                )
                .unwrap(),
                idempotency_key: IdempotencyKey::try_new(format!(
                    "review-idempotency-{at_unix_ms}"
                ))
                .unwrap(),
                business_transaction_id: BusinessTransactionId::try_new(format!(
                    "review-tx-{at_unix_ms}"
                ))
                .unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                request_started_at_unix_nanos: at_unix_ms * 1_000_000,
            },
        },
        input,
        input_hash: [5; 32],
        approval: None,
    }
}
