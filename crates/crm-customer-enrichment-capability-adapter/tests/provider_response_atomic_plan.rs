use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityRequest, CapabilityDefinition};
use crm_core_data::{AggregatePresence, RecordMutation, TransactionalAggregatePlanner};
use crm_customer_enrichment::{
    ENRICHMENT_REQUEST_RECORD_TYPE, EnrichmentRequest, EnrichmentRequestDraft,
    EnrichmentRequestStatus, MappingDraft, MappingNormalization, MappingVersion,
    ProviderProfileDraft, ProviderProfileVersion, RawPayloadPolicy, RequestPolicyEvidence,
    TargetField, TargetSnapshot,
};
use crm_customer_enrichment_capability_adapter::{
    CustomerEnrichmentRequestReferencePlanner, DispatchExpectation, MODULE_ID,
    RECORD_PROVIDER_RESPONSE_REQUEST_SCHEMA, enrichment_request_persisted_payload,
    prepare_request_dispatch, provider_response_capability_definition,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, CorrelationId,
    DataClass, ExecutionContext, IdempotencyKey, ModuleExecutionContext, ModuleId, RecordId,
    RecordRef, RecordSnapshot, RecordType, RequestId, SchemaVersion, TenantId, TraceId,
};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;
use prost::Message;

#[test]
fn metered_response_is_one_valid_atomic_batch() {
    let (definition, request, snapshot) = fixture(0, 3, vec![9; 32]);
    let planner = CustomerEnrichmentRequestReferencePlanner;

    let target = planner.target(&definition, &request).unwrap();
    assert_eq!(target.reference, snapshot.reference);
    assert_eq!(target.presence, AggregatePresence::MustExist);

    let plan = planner
        .plan(&definition, &request, Some(&snapshot))
        .unwrap();
    plan.batch.validate().unwrap();
    assert_eq!(plan.batch.records.len(), 4);
    assert_eq!(plan.batch.events.len(), 4);
    assert_eq!(plan.batch.audits.len(), 4);
    assert!(plan.batch.relationships.is_empty());

    assert!(matches!(
        &plan.batch.records[0],
        RecordMutation::Update {
            reference,
            expected_version: 7,
            payload,
        } if reference == &snapshot.reference && payload.data_class == DataClass::Personal
    ));
    assert!(matches!(
        &plan.batch.records[1],
        RecordMutation::Create { reference, payload }
            if reference.record_type.as_str() == "customer_enrichment.provider_response_receipt"
                && payload.data_class == DataClass::Personal
    ));
    for mutation in &plan.batch.records[2..] {
        assert!(matches!(
            mutation,
            RecordMutation::Create { reference, payload }
                if reference.record_type.as_str() == "customer_enrichment.provider_usage_entry"
                    && payload.data_class == DataClass::Confidential
        ));
    }

    let output = plan.output.unwrap();
    let output = wire::RecordProviderResponseResponse::decode(output.bytes.as_slice()).unwrap();
    assert_eq!(output.provider_usage_entries.len(), 2);
    assert_eq!(
        output.enrichment_request.unwrap().status,
        wire::EnrichmentRequestStatus::ResponseRecorded as i32
    );
    assert!(output.provider_response_receipt.is_some());
}

#[test]
fn zero_metered_units_emit_only_response_received_usage() {
    let (definition, request, snapshot) = fixture(0, 0, vec![9; 32]);
    let plan = CustomerEnrichmentRequestReferencePlanner
        .plan(&definition, &request, Some(&snapshot))
        .unwrap();
    plan.batch.validate().unwrap();
    assert_eq!(plan.batch.records.len(), 3);
    assert_eq!(plan.batch.events.len(), 3);
    assert_eq!(plan.batch.audits.len(), 3);
    let output = wire::RecordProviderResponseResponse::decode(
        plan.output.unwrap().bytes.as_slice(),
    )
    .unwrap();
    assert_eq!(output.provider_usage_entries.len(), 1);
    assert_eq!(
        output.provider_usage_entries[0].kind,
        wire::ProviderUsageKind::ResponseReceived as i32
    );
}

#[test]
fn stale_generation_is_rejected_before_batch_creation() {
    let (definition, request, snapshot) = fixture(1, 3, vec![9; 32]);
    let error = CustomerEnrichmentRequestReferencePlanner
        .plan(&definition, &request, Some(&snapshot))
        .unwrap_err();
    assert_eq!(
        error.code,
        "CUSTOMER_ENRICHMENT_RESPONSE_EXPECTATION_CONFLICT"
    );
}

#[test]
fn zero_digest_is_rejected_before_batch_creation() {
    let (definition, request, snapshot) = fixture(0, 3, vec![0; 32]);
    let error = CustomerEnrichmentRequestReferencePlanner
        .plan(&definition, &request, Some(&snapshot))
        .unwrap_err();
    assert_eq!(error.code, "CUSTOMER_ENRICHMENT_RESPONSE_DIGEST_INVALID");
}

fn fixture(
    expected_retry_generation: u32,
    metered_units: u64,
    digest: Vec<u8>,
) -> (CapabilityDefinition, CapabilityRequest, RecordSnapshot) {
    let domain = dispatched_request();
    let snapshot = RecordSnapshot {
        reference: RecordRef {
            record_type: RecordType::try_new(ENRICHMENT_REQUEST_RECORD_TYPE).unwrap(),
            record_id: RecordId::try_new(domain.request_id().as_str()).unwrap(),
        },
        version: 7,
        payload: enrichment_request_persisted_payload(&domain).unwrap(),
    };
    let definition = provider_response_capability_definition().unwrap();
    let input = support::protobuf_payload(
        MODULE_ID,
        RECORD_PROVIDER_RESPONSE_REQUEST_SCHEMA,
        DataClass::Personal,
        &wire::RecordProviderResponseRequest {
            enrichment_request_ref: Some(wire::EnrichmentRequestRef {
                enrichment_request_id: domain.request_id().as_str().to_owned(),
            }),
            replay_key: "provider-response-1".to_owned(),
            provider_correlation_id: Some("correlation-1".to_owned()),
            response_class: wire::ProviderResponseClass::Success as i32,
            canonical_response_digest: digest,
            provider_observed_at_unix_ms: Some(2),
            retrieved_at_unix_ms: 3,
            metered_units,
            protected_evidence_reference: Some("evidence-1".to_owned()),
            safe_provider_code: Some("success".to_owned()),
            expected_retry_generation,
        },
    )
    .unwrap();
    let request = CapabilityRequest {
        context: ModuleExecutionContext {
            module_id: ModuleId::try_new(MODULE_ID).unwrap(),
            execution: ExecutionContext {
                tenant_id: TenantId::try_new("tenant-a").unwrap(),
                actor_id: ActorId::try_new("worker-a").unwrap(),
                request_id: RequestId::try_new("response-request-1").unwrap(),
                correlation_id: CorrelationId::try_new("correlation-1").unwrap(),
                causation_id: CausationId::try_new("causation-1").unwrap(),
                trace_id: TraceId::try_new("trace-1").unwrap(),
                capability_id: CapabilityId::try_new(
                    definition.capability_id.as_str(),
                )
                .unwrap(),
                capability_version: CapabilityVersion::try_new(
                    definition.capability_version.as_str(),
                )
                .unwrap(),
                idempotency_key: IdempotencyKey::try_new("response-idempotency-1").unwrap(),
                business_transaction_id: BusinessTransactionId::try_new("response-tx-1").unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                request_started_at_unix_nanos: 3_000_000,
            },
        },
        input,
        input_hash: [4; 32],
        approval: None,
    };
    (definition, request, snapshot)
}

fn dispatched_request() -> EnrichmentRequest {
    let profile = ProviderProfileVersion::publish(ProviderProfileDraft {
        provider_key: "provider".to_owned(),
        adapter_kind: "adapter".to_owned(),
        adapter_contract_version: "1.0.0".to_owned(),
        supported_target_fields: vec![TargetField::PartyDisplayName],
        purpose_codes: vec!["enrichment".to_owned()],
        license_id: "license-v1".to_owned(),
        permitted_use_class: "customer_data".to_owned(),
        residency_region: "eu".to_owned(),
        retention_days: 30,
        raw_payload_policy: RawPayloadPolicy::DigestOnly,
        credential_handle_aliases: vec!["provider_key".to_owned()],
        effective_at_unix_ms: 1,
        expires_at_unix_ms: None,
    })
    .unwrap();
    let mapping = MappingVersion::publish(MappingDraft {
        mapping_key: "display_name".to_owned(),
        provider_profile_version_id: profile.version_id().clone(),
        provider_response_field_path: "person.display_name".to_owned(),
        target_field: TargetField::PartyDisplayName,
        normalization: MappingNormalization::CanonicalPartyDisplayNameV1,
        maximum_suggestions_per_response: 1,
        confidence_required: false,
    })
    .unwrap();
    let mut request = EnrichmentRequest::create(EnrichmentRequestDraft {
        tenant_id: TenantId::try_new("tenant-a").unwrap(),
        requested_by: ActorId::try_new("worker-a").unwrap(),
        idempotency_key: IdempotencyKey::try_new("domain-request-1").unwrap(),
        target: TargetSnapshot::try_new("party-a", 1, TargetField::PartyDisplayName).unwrap(),
        provider_profile_version_id: profile.version_id().clone(),
        mapping_version_id: mapping.version_id().clone(),
        requested_fields: vec![TargetField::PartyDisplayName],
        policy_evidence: RequestPolicyEvidence::try_new(
            "enrichment",
            "legitimate_interest",
            None,
            "request-policy-v1",
        )
        .unwrap(),
        created_at_unix_ms: 1,
        deadline_at_unix_ms: 100,
        expires_at_unix_ms: 200,
    })
    .unwrap();
    prepare_request_dispatch(
        &mut request,
        DispatchExpectation {
            status: EnrichmentRequestStatus::Created,
            retry_generation: 0,
        },
        2,
    )
    .unwrap();
    request
}
