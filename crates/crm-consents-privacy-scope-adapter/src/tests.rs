use crate::contract::{
    CAPABILITY_ID, CAPABILITY_VERSION, CONTRACT_SCHEMA_VERSION, DEFAULT_PAGE_SIZE,
    consents_privacy_scope_definition,
};
use crate::request::{ValidatedRequest, encode_cursor, validate_wire_request};
use crate::response::{VerifiedConsentResource, build_response};
use crm_customer_privacy::{CANONICAL_SCOPE_REGISTRY_VERSION, OwnerScopeRegistry};
use crm_module_sdk::{
    ActorId, CapabilityId, CapabilityVersion, CorrelationId, RecordId, RequestId, SchemaVersion,
    TenantId, TraceId,
};
use crm_proto_contracts::crm::{customer::v1 as customer, customer_privacy::v1 as privacy};
use crm_query_runtime::QueryExecutionContext;
use prost::Message;

fn context() -> QueryExecutionContext {
    QueryExecutionContext {
        tenant_id: TenantId::try_new("tenant-a").unwrap(),
        actor_id: ActorId::try_new("privacy-worker").unwrap(),
        request_id: RequestId::try_new("request-consents-scope").unwrap(),
        correlation_id: CorrelationId::try_new("correlation-consents-scope").unwrap(),
        trace_id: TraceId::try_new("trace-consents-scope").unwrap(),
        capability_id: CapabilityId::try_new(CAPABILITY_ID).unwrap(),
        capability_version: CapabilityVersion::try_new(CAPABILITY_VERSION).unwrap(),
        schema_version: SchemaVersion::try_new(CONTRACT_SCHEMA_VERSION).unwrap(),
        request_started_at_unix_nanos: 2_000_000_000,
    }
}

fn request(page_size: u32, cursor: String) -> privacy::ConsentsPrivacyScopeContributionRequest {
    let registry = OwnerScopeRegistry::canonical_v1().unwrap();
    privacy::ConsentsPrivacyScopeContributionRequest {
        contribution: Some(privacy::PrivacyScopeContributionRequestEnvelope {
            lineage: Some(privacy::PrivacyScopeContributionLineage {
                privacy_case_id: "privacy-case-consents".to_owned(),
                tenant_id: "tenant-a".to_owned(),
                canonical_party_ref: Some(customer::PartyRef {
                    party_id: "party-a".to_owned(),
                }),
                identity_resolution_generation: 7,
                registry_version: CANONICAL_SCOPE_REGISTRY_VERSION.to_owned(),
                registry_digest_sha256: registry.digest().to_vec(),
                purpose_code: "PRIVACY_ERASURE_SCOPE".to_owned(),
                effective_request_at_unix_ms: 1_000,
            }),
            page_size,
            cursor,
        }),
    }
}

fn validated() -> ValidatedRequest {
    validate_wire_request(
        &context(),
        &request(DEFAULT_PAGE_SIZE, String::new()).encode_to_vec(),
    )
    .unwrap()
}

#[test]
fn publishes_exact_contract_only_coordinate() {
    let definition = consents_privacy_scope_definition().unwrap();
    assert_eq!(definition.owner_module_id.as_str(), crm_consents::MODULE_ID);
    assert_eq!(definition.capability_id.as_str(), CAPABILITY_ID);
    assert_eq!(definition.capability_version.as_str(), CAPABILITY_VERSION);
    assert!(!definition.mutation);
    assert!(!definition.requires_idempotency);
    assert!(!definition.requires_approval);
}

#[test]
fn cursor_round_trip_is_bound_to_lineage_and_page_size() {
    let validated = validated();
    let cursor = encode_cursor(
        &validated,
        2,
        &RecordId::try_new("consent-authorization-001").unwrap(),
    )
    .unwrap();
    let decoded = validate_wire_request(
        &context(),
        &request(DEFAULT_PAGE_SIZE, cursor.clone()).encode_to_vec(),
    )
    .unwrap();
    assert_eq!(decoded.page_number, 2);
    assert_eq!(
        decoded.after_record_id.unwrap().as_str(),
        "consent-authorization-001"
    );

    let rebound = validate_wire_request(
        &context(),
        &request(DEFAULT_PAGE_SIZE - 1, cursor).encode_to_vec(),
    )
    .unwrap_err();
    assert_eq!(rebound.code, "CONSENTS_PRIVACY_SCOPE_CURSOR_INVALID");
}

#[test]
fn response_is_reference_only_deterministic_and_preserves_consent_evidence() {
    let request = validated();
    let resources = vec![
        VerifiedConsentResource {
            record_id: RecordId::try_new("consent-001").unwrap(),
            resource_version: 1,
        },
        VerifiedConsentResource {
            record_id: RecordId::try_new("consent-002").unwrap(),
            resource_version: 2,
        },
    ];
    let first = build_response(&request, &resources, 2, false).unwrap();
    let second = build_response(&request, &resources, 2, false).unwrap();
    assert_eq!(first, second);
    let envelope = first.contribution.unwrap();
    assert_eq!(envelope.resources.len(), 2);
    assert!(envelope.resources.iter().all(|resource| {
        resource.evidence_class
            == privacy::PrivacyScopeEvidenceClass::ImmutableRequiredEvidence as i32
            && resource.data_class == privacy::CustomerDataClass::Personal as i32
    }));
    assert!(envelope.page_evidence.unwrap().terminal_complete);
}

#[test]
fn malformed_wire_request_is_rejected_before_owner_reads() {
    let error = validate_wire_request(&context(), b"not-protobuf").unwrap_err();
    assert_eq!(error.code, "CONSENTS_PRIVACY_SCOPE_REQUEST_INVALID");
}
