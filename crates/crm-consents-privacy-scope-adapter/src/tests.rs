use crate::contract::{
    CAPABILITY_ID, CAPABILITY_VERSION, CONTRACT_SCHEMA_VERSION, DEFAULT_PAGE_SIZE,
    INPUT_MAXIMUM_BYTES, INPUT_RETENTION_POLICY_ID, INPUT_SCHEMA_ID, MAXIMUM_PAGE_SIZE,
    OUTPUT_MAXIMUM_BYTES, OUTPUT_RETENTION_POLICY_ID, OUTPUT_SCHEMA_ID,
    consents_privacy_scope_definition, module_id, output_descriptor_hash, schema_id, schema_version,
};
use crate::errors::configured;
use crate::request::{
    ValidatedRequest, encode_cursor, validate_request_contract, validate_wire_request,
};
use crate::response::{VerifiedConsentResource, build_response};
use crm_customer_privacy::{CANONICAL_SCOPE_REGISTRY_VERSION, OwnerScopeRegistry};
use crm_module_sdk::{
    ActorId, CapabilityId, CapabilityVersion, CorrelationId, DataClass, ErrorCategory, ModuleId,
    PayloadEncoding, RecordId, RequestId, RetentionPolicyId, SchemaVersion, SdkError, TenantId,
    TraceId, TypedPayload,
};
use crm_proto_contracts::{
    crm::{customer::v1 as customer, customer_privacy::v1 as privacy},
    message_descriptor_hash,
};
use crm_query_runtime::{QueryExecutionContext, QueryRequest};
use prost::Message;
use sha2::{Digest, Sha256};

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

fn query_request(message: &privacy::ConsentsPrivacyScopeContributionRequest) -> QueryRequest {
    let bytes = message.encode_to_vec();
    let input_hash: [u8; 32] = Sha256::digest(&bytes).into();
    QueryRequest {
        owner_module_id: module_id().unwrap(),
        context: context(),
        input: TypedPayload {
            owner: module_id().unwrap(),
            schema_id: schema_id(INPUT_SCHEMA_ID).unwrap(),
            schema_version: schema_version(CONTRACT_SCHEMA_VERSION).unwrap(),
            descriptor_hash: message_descriptor_hash(INPUT_SCHEMA_ID),
            data_class: DataClass::Confidential,
            encoding: PayloadEncoding::Protobuf,
            maximum_size_bytes: INPUT_MAXIMUM_BYTES,
            retention_policy_id: configured(RetentionPolicyId::try_new(INPUT_RETENTION_POLICY_ID))
                .unwrap(),
            bytes,
        },
        input_hash,
    }
}

fn validated() -> ValidatedRequest {
    validate_wire_request(
        &context(),
        &request(DEFAULT_PAGE_SIZE, String::new()).encode_to_vec(),
    )
    .unwrap()
}

fn assert_invalid_argument(error: SdkError, code: &str) {
    assert_eq!(error.code, code);
    assert_eq!(error.category, ErrorCategory::InvalidArgument);
    assert!(!error.retryable);
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
fn request_contract_preserves_consents_specific_integrity_errors() {
    let wire = request(DEFAULT_PAGE_SIZE, String::new());
    let valid = query_request(&wire);
    validate_request_contract(&valid).unwrap();

    let mut wrong_owner = valid.clone();
    wrong_owner.owner_module_id = ModuleId::try_new("crm.other-owner").unwrap();
    assert_invalid_argument(
        validate_request_contract(&wrong_owner).unwrap_err(),
        "CONSENTS_PRIVACY_SCOPE_REQUEST_BINDING_MISMATCH",
    );

    let mut wrong_hash = valid.clone();
    wrong_hash.input_hash = [9; 32];
    assert_invalid_argument(
        validate_request_contract(&wrong_hash).unwrap_err(),
        "CONSENTS_PRIVACY_SCOPE_INPUT_HASH_MISMATCH",
    );

    let mut wrong_schema = valid;
    wrong_schema.input.schema_id = schema_id(OUTPUT_SCHEMA_ID).unwrap();
    wrong_schema.input.descriptor_hash = output_descriptor_hash();
    wrong_schema.input.maximum_size_bytes = OUTPUT_MAXIMUM_BYTES;
    wrong_schema.input.retention_policy_id =
        configured(RetentionPolicyId::try_new(OUTPUT_RETENTION_POLICY_ID)).unwrap();
    assert_invalid_argument(
        validate_request_contract(&wrong_schema).unwrap_err(),
        "CONSENTS_PRIVACY_SCOPE_INPUT_CONTRACT_MISMATCH",
    );
}

#[test]
fn common_lineage_failures_keep_exact_consents_error_contracts() {
    let mut tenant = request(DEFAULT_PAGE_SIZE, String::new());
    tenant
        .contribution
        .as_mut()
        .unwrap()
        .lineage
        .as_mut()
        .unwrap()
        .tenant_id = "tenant-b".to_owned();
    assert_invalid_argument(
        validate_wire_request(&context(), &tenant.encode_to_vec()).unwrap_err(),
        "CONSENTS_PRIVACY_SCOPE_TENANT_MISMATCH",
    );

    let mut case = request(DEFAULT_PAGE_SIZE, String::new());
    case.contribution
        .as_mut()
        .unwrap()
        .lineage
        .as_mut()
        .unwrap()
        .privacy_case_id
        .clear();
    assert_invalid_argument(
        validate_wire_request(&context(), &case.encode_to_vec()).unwrap_err(),
        "CONSENTS_PRIVACY_SCOPE_CASE_ID_INVALID",
    );

    let mut party = request(DEFAULT_PAGE_SIZE, String::new());
    party
        .contribution
        .as_mut()
        .unwrap()
        .lineage
        .as_mut()
        .unwrap()
        .canonical_party_ref = None;
    assert_invalid_argument(
        validate_wire_request(&context(), &party.encode_to_vec()).unwrap_err(),
        "CONSENTS_PRIVACY_SCOPE_PARTY_INVALID",
    );

    let mut generation = request(DEFAULT_PAGE_SIZE, String::new());
    generation
        .contribution
        .as_mut()
        .unwrap()
        .lineage
        .as_mut()
        .unwrap()
        .identity_resolution_generation = 0;
    assert_invalid_argument(
        validate_wire_request(&context(), &generation.encode_to_vec()).unwrap_err(),
        "CONSENTS_PRIVACY_SCOPE_GENERATION_INVALID",
    );

    let mut registry_shape = request(DEFAULT_PAGE_SIZE, String::new());
    registry_shape
        .contribution
        .as_mut()
        .unwrap()
        .lineage
        .as_mut()
        .unwrap()
        .registry_digest_sha256 = vec![0; 32];
    assert_invalid_argument(
        validate_wire_request(&context(), &registry_shape.encode_to_vec()).unwrap_err(),
        "CONSENTS_PRIVACY_SCOPE_REGISTRY_INVALID",
    );

    let mut registry_mismatch = request(DEFAULT_PAGE_SIZE, String::new());
    registry_mismatch
        .contribution
        .as_mut()
        .unwrap()
        .lineage
        .as_mut()
        .unwrap()
        .registry_digest_sha256 = vec![9; 32];
    assert_invalid_argument(
        validate_wire_request(&context(), &registry_mismatch.encode_to_vec()).unwrap_err(),
        "CONSENTS_PRIVACY_SCOPE_REGISTRY_MISMATCH",
    );

    let mut purpose = request(DEFAULT_PAGE_SIZE, String::new());
    purpose
        .contribution
        .as_mut()
        .unwrap()
        .lineage
        .as_mut()
        .unwrap()
        .purpose_code = "not-normalized".to_owned();
    assert_invalid_argument(
        validate_wire_request(&context(), &purpose.encode_to_vec()).unwrap_err(),
        "CONSENTS_PRIVACY_SCOPE_PURPOSE_INVALID",
    );

    let mut future = request(DEFAULT_PAGE_SIZE, String::new());
    future
        .contribution
        .as_mut()
        .unwrap()
        .lineage
        .as_mut()
        .unwrap()
        .effective_request_at_unix_ms = 3_000;
    assert_invalid_argument(
        validate_wire_request(&context(), &future.encode_to_vec()).unwrap_err(),
        "CONSENTS_PRIVACY_SCOPE_REQUEST_TIME_INVALID",
    );

    assert_invalid_argument(
        validate_wire_request(
            &context(),
            &request(MAXIMUM_PAGE_SIZE + 1, String::new()).encode_to_vec(),
        )
        .unwrap_err(),
        "CONSENTS_PRIVACY_SCOPE_PAGE_SIZE_INVALID",
    );
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
