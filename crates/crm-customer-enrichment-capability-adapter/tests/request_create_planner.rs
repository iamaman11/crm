use crm_capability_plan_support as support;
use crm_capability_runtime::CapabilityRequest;
use crm_core_data::{AggregatePresence, TransactionalAggregatePlanner};
use crm_customer_enrichment_capability_adapter::{
    CREATE_ENRICHMENT_REQUEST_CAPABILITY, CREATE_ENRICHMENT_REQUEST_REQUEST_SCHEMA,
    CustomerEnrichmentRequestReferencePlanner, MODULE_ID, REQUEST_PARTY_SOURCE_RECORD_TYPE,
    request_create_capability_definition,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, CorrelationId,
    DataClass, ExecutionContext, IdempotencyKey, ModuleExecutionContext, ModuleId, RecordId,
    RecordRef, RecordSnapshot, RecordType, RequestId, SchemaVersion, TenantId, TraceId,
};
use crm_proto_contracts::crm::{customer::v1 as customer, customer_enrichment::v1 as wire};

fn provider_profile_version_id() -> String {
    format!("enrichment-provider-profile-{}", "a".repeat(64))
}

fn mapping_version_id() -> String {
    format!("enrichment-mapping-{}", "b".repeat(64))
}

fn capability_request() -> CapabilityRequest {
    let command = wire::CreateEnrichmentRequestRequest {
        target: Some(wire::EnrichmentTargetSnapshot {
            party_ref: Some(customer::PartyRef {
                party_id: "party-request-1".to_owned(),
            }),
            party_resource_version: 7,
            target_field: wire::EnrichmentTargetField::PartyDisplayName as i32,
        }),
        provider_profile_version_ref: Some(wire::ProviderProfileVersionRef {
            provider_profile_version_id: provider_profile_version_id(),
        }),
        mapping_version_ref: Some(wire::MappingVersionRef {
            mapping_version_id: mapping_version_id(),
        }),
        requested_fields: vec![wire::EnrichmentTargetField::PartyDisplayName as i32],
        policy_evidence: Some(wire::EnrichmentRequestPolicyEvidence {
            purpose_code: "party_enrichment".to_owned(),
            legal_basis_code: "legitimate_interest".to_owned(),
            consent_evidence_reference: None,
            policy_version: "request-policy-v1".to_owned(),
        }),
        deadline_at_unix_ms: 2_000,
        expires_at_unix_ms: 3_000,
    };
    CapabilityRequest {
        context: ModuleExecutionContext {
            module_id: ModuleId::try_new(MODULE_ID).unwrap(),
            execution: ExecutionContext {
                tenant_id: TenantId::try_new("tenant-request-1").unwrap(),
                actor_id: ActorId::try_new("actor-request-1").unwrap(),
                request_id: RequestId::try_new("request-request-1").unwrap(),
                correlation_id: CorrelationId::try_new("correlation-request-1").unwrap(),
                causation_id: CausationId::try_new("causation-request-1").unwrap(),
                trace_id: TraceId::try_new("trace-request-1").unwrap(),
                capability_id: CapabilityId::try_new(CREATE_ENRICHMENT_REQUEST_CAPABILITY)
                    .unwrap(),
                capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                idempotency_key: IdempotencyKey::try_new("idempotency-request-1").unwrap(),
                business_transaction_id: BusinessTransactionId::try_new(
                    "transaction-request-1",
                )
                .unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                request_started_at_unix_nanos: 1_000_000_000,
            },
        },
        input: support::protobuf_payload(
            MODULE_ID,
            CREATE_ENRICHMENT_REQUEST_REQUEST_SCHEMA,
            DataClass::Personal,
            &command,
        )
        .unwrap(),
        input_hash: [7; 32],
        approval: None,
    }
}

fn party_snapshot(version: i64, request: &CapabilityRequest) -> RecordSnapshot {
    RecordSnapshot {
        reference: RecordRef {
            record_type: RecordType::try_new(REQUEST_PARTY_SOURCE_RECORD_TYPE).unwrap(),
            record_id: RecordId::try_new("party-request-1").unwrap(),
        },
        version,
        payload: request.input.clone(),
    }
}

#[test]
fn target_locks_the_exact_party_aggregate() {
    let request = capability_request();
    let definition = request_create_capability_definition().unwrap();
    let target = CustomerEnrichmentRequestReferencePlanner
        .target(&definition, &request)
        .unwrap();

    assert_eq!(target.presence, AggregatePresence::MustExist);
    assert_eq!(
        target.reference.record_type.as_str(),
        REQUEST_PARTY_SOURCE_RECORD_TYPE
    );
    assert_eq!(target.reference.record_id.as_str(), "party-request-1");
}

#[test]
fn stale_party_version_is_rejected_before_write_planning() {
    let request = capability_request();
    let definition = request_create_capability_definition().unwrap();
    let error = CustomerEnrichmentRequestReferencePlanner
        .plan(&definition, &request, Some(&party_snapshot(8, &request)))
        .unwrap_err();

    assert_eq!(error.code, "CUSTOMER_ENRICHMENT_REQUEST_TARGET_STALE");
}

#[test]
fn exact_party_version_produces_one_atomic_personal_request_plan() {
    let request = capability_request();
    let definition = request_create_capability_definition().unwrap();
    let plan = CustomerEnrichmentRequestReferencePlanner
        .plan(&definition, &request, Some(&party_snapshot(7, &request)))
        .unwrap();

    assert_eq!(plan.batch.records.len(), 1);
    assert_eq!(plan.batch.relationships.len(), 1);
    assert_eq!(plan.batch.events.len(), 1);
    assert_eq!(plan.batch.audits.len(), 1);
    assert_eq!(plan.output.unwrap().data_class, DataClass::Personal);
}
