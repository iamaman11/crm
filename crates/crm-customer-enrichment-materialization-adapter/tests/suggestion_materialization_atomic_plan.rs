use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest};
use crm_core_data::{AggregatePresence, RecordMutation, TransactionalAggregatePlanner};
use crm_customer_enrichment::{
    ENRICHMENT_REQUEST_RECORD_TYPE, EnrichmentRequest, EnrichmentRequestDraft, MappingDraft,
    MappingNormalization, MappingVersion, ProviderProfileDraft, ProviderProfileVersion,
    ProviderResponseClass, ProviderResponseReceipt, ProviderResponseReceiptDraft, RawPayloadPolicy,
    RequestPolicyEvidence, TargetField, TargetSnapshot,
};
use crm_customer_enrichment_capability_adapter::{MODULE_ID, enrichment_request_persisted_payload};
use crm_customer_enrichment_materialization_adapter::{
    CustomerEnrichmentSuggestionMaterializationPlanner, MATERIALIZE_SUGGESTIONS_REQUEST_SCHEMA,
    suggestion_materialization_capability_definition,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, CorrelationId,
    DataClass, ExecutionContext, IdempotencyKey, ModuleExecutionContext, ModuleId, RecordId,
    RecordRef, RecordSnapshot, RecordType, RequestId, SchemaVersion, TenantId, TraceId,
};
use crm_proto_contracts::crm::{customer::v1 as customer, customer_enrichment::v1 as wire};
use prost::Message;

#[test]
fn success_materialization_is_one_atomic_batch() {
    let fixture = fixture(ProviderResponseClass::Success, success_candidates(), None);
    let planner = planner(&fixture);

    let target = planner
        .target(&fixture.definition, &fixture.request)
        .unwrap();
    assert_eq!(target.reference, fixture.snapshot.reference);
    assert_eq!(target.presence, AggregatePresence::MustExist);

    let plan = planner
        .plan(
            &fixture.definition,
            &fixture.request,
            Some(&fixture.snapshot),
        )
        .unwrap();
    plan.batch.validate().unwrap();
    assert_eq!(plan.batch.records.len(), 3);
    assert_eq!(plan.batch.events.len(), 3);
    assert_eq!(plan.batch.audits.len(), 3);
    assert!(plan.batch.relationships.is_empty());
    assert!(matches!(
        &plan.batch.records[0],
        RecordMutation::Update {
            reference,
            expected_version: 9,
            payload,
        } if reference == &fixture.snapshot.reference && payload.data_class == DataClass::Personal
    ));
    for mutation in &plan.batch.records[1..] {
        assert!(matches!(
            mutation,
            RecordMutation::Create { reference, payload }
                if reference.record_type.as_str() == "customer_enrichment.suggestion"
                    && payload.data_class == DataClass::Personal
        ));
    }

    let output =
        wire::MaterializeSuggestionsResponse::decode(plan.output.unwrap().bytes.as_slice())
            .unwrap();
    assert_eq!(output.suggestions.len(), 2);
    assert_eq!(
        output.enrichment_request.unwrap().status,
        wire::EnrichmentRequestStatus::SuggestionsMaterialized as i32
    );
    let ids = output
        .suggestions
        .iter()
        .map(|suggestion| {
            suggestion
                .suggestion_ref
                .as_ref()
                .unwrap()
                .suggestion_id
                .as_str()
        })
        .collect::<Vec<_>>();
    assert!(ids[0] < ids[1]);
}

#[test]
fn no_match_updates_only_the_request() {
    let fixture = fixture(ProviderResponseClass::NoMatch, Vec::new(), None);
    let plan = planner(&fixture)
        .plan(
            &fixture.definition,
            &fixture.request,
            Some(&fixture.snapshot),
        )
        .unwrap();
    plan.batch.validate().unwrap();
    assert_eq!(plan.batch.records.len(), 1);
    assert_eq!(plan.batch.events.len(), 1);
    assert_eq!(plan.batch.audits.len(), 1);
    assert!(plan.batch.relationships.is_empty());

    let output =
        wire::MaterializeSuggestionsResponse::decode(plan.output.unwrap().bytes.as_slice())
            .unwrap();
    assert!(output.suggestions.is_empty());
    assert_eq!(
        output.enrichment_request.unwrap().status,
        wire::EnrichmentRequestStatus::SuggestionsMaterialized as i32
    );
}

#[test]
fn wrong_receipt_reference_is_rejected_before_locking() {
    let fixture = fixture(
        ProviderResponseClass::Success,
        success_candidates(),
        Some("different-receipt"),
    );
    let error = planner(&fixture)
        .target(&fixture.definition, &fixture.request)
        .unwrap_err();
    assert_eq!(
        error.code,
        "CUSTOMER_ENRICHMENT_MATERIALIZATION_RECEIPT_CONFLICT"
    );
}

#[test]
fn stale_candidate_target_is_rejected_before_batch_creation() {
    let mut candidates = success_candidates();
    candidates[0]
        .target
        .as_mut()
        .unwrap()
        .party_resource_version += 1;
    let fixture = fixture(ProviderResponseClass::Success, candidates, None);
    let error = planner(&fixture)
        .plan(
            &fixture.definition,
            &fixture.request,
            Some(&fixture.snapshot),
        )
        .unwrap_err();
    assert_eq!(error.code, "CUSTOMER_ENRICHMENT_CANDIDATE_TARGET_CONFLICT");
}

struct Fixture {
    definition: CapabilityDefinition,
    request: CapabilityRequest,
    snapshot: RecordSnapshot,
    receipt: ProviderResponseReceipt,
    profile: ProviderProfileVersion,
    mapping: MappingVersion,
}

fn planner(fixture: &Fixture) -> CustomerEnrichmentSuggestionMaterializationPlanner {
    CustomerEnrichmentSuggestionMaterializationPlanner::new(
        fixture.receipt.clone(),
        fixture.profile.clone(),
        fixture.mapping.clone(),
    )
}

fn fixture(
    response_class: ProviderResponseClass,
    candidates: Vec<wire::ProviderSuggestionCandidate>,
    receipt_reference_override: Option<&str>,
) -> Fixture {
    let profile = ProviderProfileVersion::publish(ProviderProfileDraft {
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
        credential_handle_aliases: vec!["provider_primary".to_owned()],
        effective_at_unix_ms: 1,
        expires_at_unix_ms: Some(1_000),
    })
    .unwrap();
    let mapping = MappingVersion::publish(MappingDraft {
        mapping_key: "party_display_name".to_owned(),
        provider_profile_version_id: profile.version_id().clone(),
        provider_response_field_path: "organization.legal_name".to_owned(),
        target_field: TargetField::PartyDisplayName,
        normalization: MappingNormalization::CanonicalPartyDisplayNameV1,
        maximum_suggestions_per_response: 2,
        confidence_required: true,
    })
    .unwrap();
    let mut domain = EnrichmentRequest::create(EnrichmentRequestDraft {
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
    domain.queue(10).unwrap();
    domain.mark_dispatched(10).unwrap();
    let receipt = ProviderResponseReceipt::record(ProviderResponseReceiptDraft {
        request_id: domain.request_id().clone(),
        provider_profile_version_id: profile.version_id().clone(),
        mapping_version_id: mapping.version_id().clone(),
        replay_key: "provider-replay-1".to_owned(),
        provider_correlation_id: None,
        response_class,
        canonical_response_digest: [7; 32],
        provider_observed_at_unix_ms: Some(20),
        retrieved_at_unix_ms: 30,
        metered_units: 1,
        protected_evidence_reference: Some("evidence-1".to_owned()),
    })
    .unwrap();
    domain
        .record_response(receipt.receipt_id().clone(), 30)
        .unwrap();

    let snapshot = RecordSnapshot {
        reference: RecordRef {
            record_type: RecordType::try_new(ENRICHMENT_REQUEST_RECORD_TYPE).unwrap(),
            record_id: RecordId::try_new(domain.request_id().as_str()).unwrap(),
        },
        version: 9,
        payload: enrichment_request_persisted_payload(&domain).unwrap(),
    };
    let definition = suggestion_materialization_capability_definition().unwrap();
    let receipt_id = receipt_reference_override
        .unwrap_or(receipt.receipt_id().as_str())
        .to_owned();
    let input = support::protobuf_payload(
        MODULE_ID,
        MATERIALIZE_SUGGESTIONS_REQUEST_SCHEMA,
        DataClass::Personal,
        &wire::MaterializeSuggestionsRequest {
            enrichment_request_ref: Some(wire::EnrichmentRequestRef {
                enrichment_request_id: domain.request_id().as_str().to_owned(),
            }),
            provider_response_receipt_ref: Some(wire::ProviderResponseReceiptRef {
                provider_response_receipt_id: receipt_id,
            }),
            candidates,
        },
    )
    .unwrap();
    let request = CapabilityRequest {
        context: ModuleExecutionContext {
            module_id: ModuleId::try_new(MODULE_ID).unwrap(),
            execution: ExecutionContext {
                tenant_id: TenantId::try_new("tenant-a").unwrap(),
                actor_id: ActorId::try_new("worker-a").unwrap(),
                request_id: RequestId::try_new("materialization-request-1").unwrap(),
                correlation_id: CorrelationId::try_new("correlation-1").unwrap(),
                causation_id: CausationId::try_new("causation-1").unwrap(),
                trace_id: TraceId::try_new("trace-1").unwrap(),
                capability_id: CapabilityId::try_new(definition.capability_id.as_str()).unwrap(),
                capability_version: CapabilityVersion::try_new(
                    definition.capability_version.as_str(),
                )
                .unwrap(),
                idempotency_key: IdempotencyKey::try_new("materialization-idempotency-1").unwrap(),
                business_transaction_id: BusinessTransactionId::try_new("materialization-tx-1")
                    .unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                request_started_at_unix_nanos: 40_000_000,
            },
        },
        input,
        input_hash: [4; 32],
        approval: None,
    };

    Fixture {
        definition,
        request,
        snapshot,
        receipt,
        profile,
        mapping,
    }
}

fn success_candidates() -> Vec<wire::ProviderSuggestionCandidate> {
    vec![
        candidate("Zeta Company", 8_000),
        candidate("Alpha Company", 9_000),
    ]
}

fn candidate(
    proposed_value: &str,
    confidence_basis_points: u32,
) -> wire::ProviderSuggestionCandidate {
    wire::ProviderSuggestionCandidate {
        target: Some(wire::EnrichmentTargetSnapshot {
            party_ref: Some(customer::PartyRef {
                party_id: "party-a".to_owned(),
            }),
            party_resource_version: 7,
            target_field: wire::EnrichmentTargetField::PartyDisplayName as i32,
        }),
        proposed_value: proposed_value.to_owned(),
        observed_at_unix_ms: Some(20),
        effective_at_unix_ms: 20,
        fresh_until_unix_ms: 100,
        expires_at_unix_ms: 150,
        confidence_basis_points: Some(confidence_basis_points),
        policy_evidence: Some(wire::ProviderPolicyEvidence {
            license_id: "provider-license".to_owned(),
            permitted_use_class: "customer_master_review".to_owned(),
            residency_region: "eu".to_owned(),
            retention_days: 30,
            consent_evidence_reference: Some("consent-a".to_owned()),
        }),
        evidence_references: vec!["evidence-1".to_owned()],
    }
}
