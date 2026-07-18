use super::*;
use crm_capability_runtime::CapabilityExecutionResult;
use crm_customer_enrichment::{
    EnrichmentRequest, EnrichmentRequestDraft, MappingDraft, MappingNormalization, MappingVersion,
    PartySnapshot, ProviderDispatchExpectation, ProviderProfileDraft, ProviderProfileVersion,
    RawPayloadPolicy, RequestPolicyEvidence, TargetField, TargetSnapshot,
    prepare_provider_dispatch_attempt,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityVersion, CausationId, CorrelationId,
    ExecutionContext, IdempotencyKey, PortFuture, RecordId, RequestId, SchemaVersion, TenantId,
    TraceId,
};
use crm_proto_contracts::crm::{customer::v1 as customer, customer_enrichment::v1 as wire};
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExecutorStage {
    Dispatch,
    Response,
}

#[derive(Clone)]
struct FakeExecutor {
    stage: ExecutorStage,
    provider_request: ProviderDispatchRequest,
    calls: Arc<Mutex<Vec<&'static str>>>,
    captured: Arc<Mutex<Vec<CapabilityRequest>>>,
    failure: Option<SdkError>,
}

impl TransactionalCapabilityExecutor for FakeExecutor {
    fn execute<'a>(
        &'a self,
        _definition: &'a CapabilityDefinition,
        request: CapabilityRequest,
    ) -> PortFuture<'a, Result<CapabilityExecutionResult, SdkError>> {
        self.calls
            .lock()
            .expect("call log lock")
            .push(match self.stage {
                ExecutorStage::Dispatch => "dispatch_commit",
                ExecutorStage::Response => "response_commit",
            });
        self.captured
            .lock()
            .expect("capture lock")
            .push(request.clone());
        let failure = self.failure.clone();
        let provider_request = self.provider_request.clone();
        let stage = self.stage;
        Box::pin(async move {
            if let Some(error) = failure {
                return Err(error);
            }
            let output = match stage {
                ExecutorStage::Dispatch => dispatch_output(&provider_request)?,
                ExecutorStage::Response => response_output(&provider_request, &request)?,
            };
            Ok(CapabilityExecutionResult {
                output: Some(output),
                affected_resources: Vec::new(),
                replayed: false,
            })
        })
    }
}

#[derive(Clone)]
struct FakeRegistry {
    calls: Arc<Mutex<Vec<&'static str>>>,
    response: SanitizedProviderResponse,
    failure: Option<SdkError>,
}

impl ProviderAdapterRegistryPort for FakeRegistry {
    fn dispatch_exact<'a>(
        &'a self,
        _request: ProviderDispatchRequest,
    ) -> PortFuture<'a, Result<SanitizedProviderResponse, SdkError>> {
        self.calls.lock().expect("call log lock").push("provider");
        let result = self
            .failure
            .clone()
            .map_or_else(|| Ok(self.response.clone()), Err);
        Box::pin(async move { result })
    }
}

#[tokio::test]
async fn worker_orders_dispatch_commit_provider_and_response_commit() {
    let fixture = fixture();
    let worker = worker(&fixture, None, None, fixture.response.clone());
    let result = worker.execute(fixture.item.clone()).await.unwrap();

    assert!(!result.dispatch_replayed);
    assert!(!result.response_replayed);
    assert_eq!(
        result.response.enrichment_request.as_ref().unwrap().status,
        wire::EnrichmentRequestStatus::ResponseRecorded as i32
    );
    assert_eq!(
        fixture.calls.lock().expect("call log lock").as_slice(),
        ["dispatch_commit", "provider", "response_commit"]
    );
}

#[tokio::test]
async fn provider_is_not_called_when_dispatch_commit_fails() {
    let fixture = fixture();
    let failure = SdkError::new(
        "TEST_DISPATCH_COMMIT_FAILED",
        ErrorCategory::Unavailable,
        true,
        "The dispatch commit failed.",
    );
    let worker = worker(&fixture, Some(failure), None, fixture.response.clone());
    let error = worker.execute(fixture.item.clone()).await.unwrap_err();
    assert_eq!(error.code, "TEST_DISPATCH_COMMIT_FAILED");
    assert_eq!(
        fixture.calls.lock().expect("call log lock").as_slice(),
        ["dispatch_commit"]
    );
}

#[tokio::test]
async fn response_commit_is_not_called_for_mismatched_provider_replay_key() {
    let fixture = fixture();
    let mut response = fixture.response.clone();
    response.replay_key = "different-provider-attempt".to_owned();
    let worker = worker(&fixture, None, None, response);
    let error = worker.execute(fixture.item.clone()).await.unwrap_err();
    assert_eq!(
        error.code,
        "CUSTOMER_ENRICHMENT_PROVIDER_REPLAY_KEY_MISMATCH"
    );
    assert_eq!(
        fixture.calls.lock().expect("call log lock").as_slice(),
        ["dispatch_commit", "provider"]
    );
}

#[tokio::test]
async fn repeated_work_item_builds_the_same_response_commit_identity() {
    let fixture = fixture();
    let worker = worker(&fixture, None, None, fixture.response.clone());
    worker.execute(fixture.item.clone()).await.unwrap();
    worker.execute(fixture.item.clone()).await.unwrap();

    let requests = fixture.response_requests.lock().expect("capture lock");
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[0].context.execution.request_id,
        requests[1].context.execution.request_id
    );
    assert_eq!(
        requests[0].context.execution.idempotency_key,
        requests[1].context.execution.idempotency_key
    );
    assert_eq!(requests[0].input_hash, requests[1].input_hash);
    assert_eq!(requests[0].input, requests[1].input);
}

struct Fixture {
    item: ProviderDispatchWorkItem,
    response: SanitizedProviderResponse,
    calls: Arc<Mutex<Vec<&'static str>>>,
    dispatch_requests: Arc<Mutex<Vec<CapabilityRequest>>>,
    response_requests: Arc<Mutex<Vec<CapabilityRequest>>>,
}

fn fixture() -> Fixture {
    let profile = ProviderProfileVersion::publish(ProviderProfileDraft {
        provider_key: "company_registry".to_owned(),
        adapter_kind: "registry_http_v1".to_owned(),
        adapter_contract_version: "1.0.0".to_owned(),
        supported_target_fields: vec![TargetField::PartyDisplayName],
        purpose_codes: vec!["customer_profile_enrichment".to_owned()],
        license_id: "Registry licence".to_owned(),
        permitted_use_class: "customer_master_review".to_owned(),
        residency_region: "eu".to_owned(),
        retention_days: 30,
        raw_payload_policy: RawPayloadPolicy::DigestOnly,
        credential_handle_aliases: vec!["registry_primary".to_owned()],
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
        maximum_suggestions_per_response: 1,
        confidence_required: true,
    })
    .unwrap();
    let actor = ActorId::try_new("worker-actor").unwrap();
    let mut domain = EnrichmentRequest::create(EnrichmentRequestDraft {
        tenant_id: TenantId::try_new("tenant-1").unwrap(),
        requested_by: actor.clone(),
        idempotency_key: IdempotencyKey::try_new("request-key-1").unwrap(),
        target: TargetSnapshot::try_new("party-1", 7, TargetField::PartyDisplayName).unwrap(),
        provider_profile_version_id: profile.version_id().clone(),
        mapping_version_id: mapping.version_id().clone(),
        requested_fields: vec![TargetField::PartyDisplayName],
        policy_evidence: RequestPolicyEvidence::try_new(
            "customer_profile_enrichment",
            "legitimate_interest",
            None,
            "1.0.0",
        )
        .unwrap(),
        created_at_unix_ms: 10,
        deadline_at_unix_ms: 100,
        expires_at_unix_ms: 200,
    })
    .unwrap();
    let party = PartySnapshot {
        party_id: RecordId::try_new("party-1").unwrap(),
        display_name: "Example Company".to_owned(),
        resource_version: 7,
        observed_at_unix_ms: 15,
    };
    let provider_request = prepare_provider_dispatch_attempt(
        &mut domain,
        ProviderDispatchExpectation {
            status: crm_customer_enrichment::EnrichmentRequestStatus::Created,
            retry_generation: 0,
        },
        &profile,
        &party,
        actor,
        20,
    )
    .unwrap();
    let dispatch_definition = request_dispatch_capability_definition().unwrap();
    let input = support::protobuf_payload(
        MODULE_ID,
        DISPATCH_ENRICHMENT_REQUEST_REQUEST_SCHEMA,
        DataClass::Personal,
        &wire::DispatchEnrichmentRequestRequest {
            enrichment_request_ref: Some(wire::EnrichmentRequestRef {
                enrichment_request_id: domain.request_id().as_str().to_owned(),
            }),
            expected_status: wire::EnrichmentRequestStatus::Created as i32,
            expected_retry_generation: 0,
        },
    )
    .unwrap();
    let input_hash = semantic_input_hash(&input);
    let dispatch_request = CapabilityRequest {
        context: ModuleExecutionContext {
            module_id: ModuleId::try_new(MODULE_ID).unwrap(),
            execution: ExecutionContext {
                tenant_id: TenantId::try_new("tenant-1").unwrap(),
                actor_id: ActorId::try_new("worker-actor").unwrap(),
                request_id: RequestId::try_new("dispatch-request-1").unwrap(),
                correlation_id: CorrelationId::try_new("correlation-1").unwrap(),
                causation_id: CausationId::try_new("causation-1").unwrap(),
                trace_id: TraceId::try_new("trace-1").unwrap(),
                capability_id: dispatch_definition.capability_id.clone(),
                capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                idempotency_key: IdempotencyKey::try_new("dispatch-idempotency-1").unwrap(),
                business_transaction_id: BusinessTransactionId::try_new("dispatch-tx-1").unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                request_started_at_unix_nanos: 20_000_000,
            },
        },
        input,
        input_hash,
        approval: None,
    };
    let response = SanitizedProviderResponse {
        replay_key: provider_request.provider_idempotency_key.clone(),
        provider_correlation_id: Some("provider-correlation-1".to_owned()),
        response_class: ProviderResponseClass::Success,
        canonical_response_digest: [9; 32],
        provider_observed_at_unix_ms: Some(21),
        retrieved_at_unix_ms: 22,
        metered_units: 3,
        protected_evidence_reference: None,
        safe_provider_code: Some("success".to_owned()),
    };
    Fixture {
        item: ProviderDispatchWorkItem {
            dispatch_request,
            provider_request,
        },
        response,
        calls: Arc::new(Mutex::new(Vec::new())),
        dispatch_requests: Arc::new(Mutex::new(Vec::new())),
        response_requests: Arc::new(Mutex::new(Vec::new())),
    }
}

fn worker(
    fixture: &Fixture,
    dispatch_failure: Option<SdkError>,
    response_failure: Option<SdkError>,
    response: SanitizedProviderResponse,
) -> CustomerEnrichmentProviderWorker {
    let dispatch_executor = Arc::new(FakeExecutor {
        stage: ExecutorStage::Dispatch,
        provider_request: fixture.item.provider_request.clone(),
        calls: fixture.calls.clone(),
        captured: fixture.dispatch_requests.clone(),
        failure: dispatch_failure,
    });
    let response_executor = Arc::new(FakeExecutor {
        stage: ExecutorStage::Response,
        provider_request: fixture.item.provider_request.clone(),
        calls: fixture.calls.clone(),
        captured: fixture.response_requests.clone(),
        failure: response_failure,
    });
    let registry = Arc::new(FakeRegistry {
        calls: fixture.calls.clone(),
        response,
        failure: None,
    });
    CustomerEnrichmentProviderWorker::try_new(dispatch_executor, response_executor, registry)
        .unwrap()
}

fn dispatch_output(provider: &ProviderDispatchRequest) -> Result<TypedPayload, SdkError> {
    support::protobuf_payload(
        MODULE_ID,
        DISPATCH_ENRICHMENT_REQUEST_RESPONSE_SCHEMA,
        DataClass::Personal,
        &wire::DispatchEnrichmentRequestResponse {
            enrichment_request: Some(enrichment_request_wire(
                provider,
                wire::EnrichmentRequestStatus::Dispatched,
                None,
            )),
        },
    )
}

fn response_output(
    provider: &ProviderDispatchRequest,
    request: &CapabilityRequest,
) -> Result<TypedPayload, SdkError> {
    let command: wire::RecordProviderResponseRequest = support::decode_request_with_data_class(
        request,
        MODULE_ID,
        RECORD_PROVIDER_RESPONSE_REQUEST_SCHEMA,
        DataClass::Personal,
    )?;
    let receipt = wire::ProviderResponseReceipt {
        provider_response_receipt_ref: Some(wire::ProviderResponseReceiptRef {
            provider_response_receipt_id: "provider-receipt-1".to_owned(),
        }),
        enrichment_request_ref: command.enrichment_request_ref.clone(),
        provider_profile_version_ref: Some(wire::ProviderProfileVersionRef {
            provider_profile_version_id: provider.provider_profile_version_id.as_str().to_owned(),
        }),
        mapping_version_ref: Some(wire::MappingVersionRef {
            mapping_version_id: provider.mapping_version_id.as_str().to_owned(),
        }),
        replay_key: command.replay_key.clone(),
        provider_correlation_id: command.provider_correlation_id.clone(),
        response_class: command.response_class,
        canonical_response_digest: command.canonical_response_digest.clone(),
        provider_observed_at_unix_ms: command.provider_observed_at_unix_ms,
        retrieved_at_unix_ms: command.retrieved_at_unix_ms,
        metered_units: command.metered_units,
        protected_evidence_reference: command.protected_evidence_reference.clone(),
    };
    support::protobuf_payload(
        MODULE_ID,
        RECORD_PROVIDER_RESPONSE_RESPONSE_SCHEMA,
        DataClass::Personal,
        &wire::RecordProviderResponseResponse {
            enrichment_request: Some(enrichment_request_wire(
                provider,
                wire::EnrichmentRequestStatus::ResponseRecorded,
                Some("provider-receipt-1"),
            )),
            provider_response_receipt: Some(receipt),
            provider_usage_entries: Vec::new(),
        },
    )
}

fn enrichment_request_wire(
    provider: &ProviderDispatchRequest,
    status: wire::EnrichmentRequestStatus,
    receipt_id: Option<&str>,
) -> wire::EnrichmentRequest {
    wire::EnrichmentRequest {
        enrichment_request_ref: Some(wire::EnrichmentRequestRef {
            enrichment_request_id: provider.enrichment_request_id.as_str().to_owned(),
        }),
        requested_by_actor_id: provider.actor_id.as_str().to_owned(),
        target: Some(wire::EnrichmentTargetSnapshot {
            party_ref: Some(customer::PartyRef {
                party_id: provider.party_id.as_str().to_owned(),
            }),
            party_resource_version: provider.party_resource_version,
            target_field: wire::EnrichmentTargetField::PartyDisplayName as i32,
        }),
        provider_profile_version_ref: Some(wire::ProviderProfileVersionRef {
            provider_profile_version_id: provider.provider_profile_version_id.as_str().to_owned(),
        }),
        mapping_version_ref: Some(wire::MappingVersionRef {
            mapping_version_id: provider.mapping_version_id.as_str().to_owned(),
        }),
        requested_fields: vec![wire::EnrichmentTargetField::PartyDisplayName as i32],
        policy_evidence: Some(wire::EnrichmentRequestPolicyEvidence {
            purpose_code: "customer_profile_enrichment".to_owned(),
            legal_basis_code: "legitimate_interest".to_owned(),
            consent_evidence_reference: None,
            policy_version: "1.0.0".to_owned(),
        }),
        created_at_unix_ms: 10,
        deadline_at_unix_ms: provider.deadline_at_unix_ms,
        expires_at_unix_ms: 200,
        status: status as i32,
        retry_generation: provider.retry_generation,
        provider_response_receipt_ref: receipt_id.map(|value| wire::ProviderResponseReceiptRef {
            provider_response_receipt_id: value.to_owned(),
        }),
        last_safe_failure_code: None,
        updated_at_unix_ms: 22,
    }
}
