use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest};
use crm_core_data::{AggregatePresence, RecordMutation, TransactionalAggregatePlanner};
use crm_customer_enrichment::{
    ENRICHMENT_REQUEST_RECORD_TYPE, EnrichmentRequest, EnrichmentRequestDraft,
    EnrichmentRequestStatus, MappingDraft, MappingNormalization, MappingVersion,
    ProviderProfileDraft, ProviderProfileVersion, ProviderUsageKind, RawPayloadPolicy,
    RequestPolicyEvidence, TargetField, TargetSnapshot, decode_provider_usage_entry_state,
};
use crm_customer_enrichment_capability_adapter::{
    CustomerEnrichmentRequestDispatchPlanner, DISPATCH_ENRICHMENT_REQUEST_REQUEST_SCHEMA,
    MODULE_ID, enrichment_request_persisted_payload, request_dispatch_capability_definition,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, CorrelationId,
    DataClass, ExecutionContext, IdempotencyKey, ModuleExecutionContext, ModuleId, RecordId,
    RecordRef, RecordSnapshot, RecordType, RequestId, SchemaVersion, TenantId, TraceId,
};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;
use prost::Message;

#[test]
fn created_request_dispatch_is_one_valid_atomic_batch() {
    let (definition, request, snapshot) = fixture(EnrichmentRequestStatus::Created, 0, 0);
    let planner = CustomerEnrichmentRequestDispatchPlanner;

    let target = planner.target(&definition, &request).unwrap();
    assert_eq!(target.reference, snapshot.reference);
    assert_eq!(target.presence, AggregatePresence::MustExist);

    let plan = planner
        .plan(&definition, &request, Some(&snapshot))
        .unwrap();
    plan.batch.validate().unwrap();
    assert_eq!(plan.batch.records.len(), 2);
    assert_eq!(plan.batch.events.len(), 2);
    assert_eq!(plan.batch.audits.len(), 2);
    assert!(plan.batch.relationships.is_empty());

    assert!(matches!(
        &plan.batch.records[0],
        RecordMutation::Update {
            reference,
            expected_version: 7,
            payload,
        } if reference == &snapshot.reference && payload.data_class == DataClass::Personal
    ));
    let usage_payload = match &plan.batch.records[1] {
        RecordMutation::Create { reference, payload }
            if reference.record_type.as_str() == "customer_enrichment.provider_usage_entry"
                && payload.data_class == DataClass::Confidential =>
        {
            payload
        }
        other => panic!("unexpected provider usage mutation: {other:?}"),
    };
    let usage = decode_provider_usage_entry_state(&usage_payload.bytes).unwrap();
    assert_eq!(usage.kind(), ProviderUsageKind::RequestDispatched);
    assert_eq!(usage.metered_units(), 0);

    let output =
        wire::DispatchEnrichmentRequestResponse::decode(plan.output.unwrap().bytes.as_slice())
            .unwrap();
    let output = output.enrichment_request.unwrap();
    assert_eq!(
        output.status,
        wire::EnrichmentRequestStatus::Dispatched as i32
    );
    assert_eq!(output.retry_generation, 0);
}

#[test]
fn retryable_failure_dispatch_increments_generation_atomically() {
    let (definition, request, snapshot) = fixture(EnrichmentRequestStatus::FailedRetryable, 0, 0);
    let plan = CustomerEnrichmentRequestDispatchPlanner
        .plan(&definition, &request, Some(&snapshot))
        .unwrap();
    plan.batch.validate().unwrap();
    let output =
        wire::DispatchEnrichmentRequestResponse::decode(plan.output.unwrap().bytes.as_slice())
            .unwrap()
            .enrichment_request
            .unwrap();
    assert_eq!(
        output.status,
        wire::EnrichmentRequestStatus::Dispatched as i32
    );
    assert_eq!(output.retry_generation, 1);
}

#[test]
fn stale_retry_generation_is_rejected_before_batch_creation() {
    let (definition, request, snapshot) = fixture(EnrichmentRequestStatus::Created, 1, 0);
    let error = CustomerEnrichmentRequestDispatchPlanner
        .plan(&definition, &request, Some(&snapshot))
        .unwrap_err();
    assert_eq!(error.code, "CUSTOMER_ENRICHMENT_REQUEST_DISPATCH_CONFLICT");
}

fn fixture(
    status: EnrichmentRequestStatus,
    expected_retry_generation: u32,
    domain_retry_generation: u32,
) -> (CapabilityDefinition, CapabilityRequest, RecordSnapshot) {
    let mut domain = domain_request();
    match status {
        EnrichmentRequestStatus::Created => {}
        EnrichmentRequestStatus::Queued => domain.queue(2).unwrap(),
        EnrichmentRequestStatus::FailedRetryable => {
            domain.fail_retryable("provider_unavailable", 2).unwrap();
        }
        _ => panic!("unsupported fixture status"),
    }
    assert_eq!(domain.retry_generation(), domain_retry_generation);

    let snapshot = RecordSnapshot {
        reference: RecordRef {
            record_type: RecordType::try_new(ENRICHMENT_REQUEST_RECORD_TYPE).unwrap(),
            record_id: RecordId::try_new(domain.request_id().as_str()).unwrap(),
        },
        version: 7,
        payload: enrichment_request_persisted_payload(&domain).unwrap(),
    };
    let definition = request_dispatch_capability_definition().unwrap();
    let input = support::protobuf_payload(
        MODULE_ID,
        DISPATCH_ENRICHMENT_REQUEST_REQUEST_SCHEMA,
        DataClass::Personal,
        &wire::DispatchEnrichmentRequestRequest {
            enrichment_request_ref: Some(wire::EnrichmentRequestRef {
                enrichment_request_id: domain.request_id().as_str().to_owned(),
            }),
            expected_status: status_to_wire(status),
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
                request_id: RequestId::try_new("dispatch-request-1").unwrap(),
                correlation_id: CorrelationId::try_new("correlation-1").unwrap(),
                causation_id: CausationId::try_new("causation-1").unwrap(),
                trace_id: TraceId::try_new("trace-1").unwrap(),
                capability_id: CapabilityId::try_new(definition.capability_id.as_str()).unwrap(),
                capability_version: CapabilityVersion::try_new(
                    definition.capability_version.as_str(),
                )
                .unwrap(),
                idempotency_key: IdempotencyKey::try_new("dispatch-idempotency-1").unwrap(),
                business_transaction_id: BusinessTransactionId::try_new("dispatch-tx-1").unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                request_started_at_unix_nanos: 3_000_000,
            },
        },
        input,
        input_hash: [6; 32],
        approval: None,
    };
    (definition, request, snapshot)
}

fn domain_request() -> EnrichmentRequest {
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
    EnrichmentRequest::create(EnrichmentRequestDraft {
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
    .unwrap()
}

fn status_to_wire(status: EnrichmentRequestStatus) -> i32 {
    match status {
        EnrichmentRequestStatus::Created => wire::EnrichmentRequestStatus::Created as i32,
        EnrichmentRequestStatus::Queued => wire::EnrichmentRequestStatus::Queued as i32,
        EnrichmentRequestStatus::FailedRetryable => {
            wire::EnrichmentRequestStatus::FailedRetryable as i32
        }
        _ => panic!("unsupported fixture status"),
    }
}
