use crate::contract::{
    CAPABILITY_ID, CAPABILITY_VERSION, CONTRACT_SCHEMA_VERSION, DEFAULT_PAGE_SIZE,
    INPUT_MAXIMUM_BYTES, INPUT_RETENTION_POLICY_ID, INPUT_SCHEMA_ID,
    MAX_PRIVACY_ACTIVE_REDIRECT_EDGES, MAX_PRIVACY_ALIAS_HOPS, MAX_PRIVACY_ALIAS_NODES,
    MAX_PRIVACY_CANDIDATE_RECORDS_REHYDRATED, MAX_PRIVACY_MERGE_RECORDS_REHYDRATED,
    MAX_PRIVACY_OWNER_RECORDS_SCANNED, MAX_PRIVACY_RELATIONSHIP_CANDIDATES, MAXIMUM_PAGE_SIZE,
    OUTPUT_MAXIMUM_BYTES, OUTPUT_RETENTION_POLICY_ID, OUTPUT_SCHEMA_ID,
    identity_resolution_privacy_scope_definition, module_id, output_descriptor_hash, schema_id,
    schema_version,
};
use crate::errors::configured;
use crate::request::{
    CursorState, ResourceFamily, encode_cursor, validate_request_contract, validate_wire_request,
};
use crate::response::{VerifiedIdentityResolutionResource, build_response};
use crm_customer_privacy::{CANONICAL_SCOPE_REGISTRY_VERSION, OwnerScopeRegistry};
use crm_module_sdk::{
    ActorId, CapabilityId, CapabilityVersion, CorrelationId, DataClass, ErrorCategory, ModuleId,
    PayloadEncoding, RecordId, RequestId, RetentionPolicyId, SdkError, TenantId, TraceId,
    TypedPayload,
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
        request_id: RequestId::try_new("request-identity-resolution-scope").unwrap(),
        correlation_id: CorrelationId::try_new("correlation-identity-resolution-scope").unwrap(),
        trace_id: TraceId::try_new("trace-identity-resolution-scope").unwrap(),
        capability_id: CapabilityId::try_new(CAPABILITY_ID).unwrap(),
        capability_version: CapabilityVersion::try_new(CAPABILITY_VERSION).unwrap(),
        schema_version: schema_version(CONTRACT_SCHEMA_VERSION).unwrap(),
        request_started_at_unix_nanos: 2_000_000_000,
    }
}

fn wire_request(
    page_size: u32,
    cursor: String,
) -> privacy::IdentityResolutionPrivacyScopeContributionRequest {
    let registry = OwnerScopeRegistry::canonical_v1().unwrap();
    privacy::IdentityResolutionPrivacyScopeContributionRequest {
        contribution: Some(privacy::PrivacyScopeContributionRequestEnvelope {
            lineage: Some(privacy::PrivacyScopeContributionLineage {
                privacy_case_id: "privacy-case-identity-resolution".to_owned(),
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

fn query_request(
    message: &privacy::IdentityResolutionPrivacyScopeContributionRequest,
) -> QueryRequest {
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

fn assert_invalid_argument(error: SdkError, code: &str) {
    assert_eq!(error.code, code);
    assert_eq!(error.category, ErrorCategory::InvalidArgument);
    assert!(!error.retryable);
}

#[test]
fn publishes_exact_contract_only_coordinate_and_frozen_bounds() {
    let definition = identity_resolution_privacy_scope_definition().unwrap();
    assert_eq!(
        definition.owner_module_id.as_str(),
        crm_identity_resolution::MODULE_ID
    );
    assert_eq!(definition.capability_id.as_str(), CAPABILITY_ID);
    assert_eq!(definition.capability_version.as_str(), CAPABILITY_VERSION);
    assert!(!definition.mutation);
    assert!(!definition.requires_idempotency);
    assert!(!definition.requires_approval);
    assert_eq!(MAX_PRIVACY_ALIAS_HOPS, 64);
    assert_eq!(MAX_PRIVACY_ALIAS_NODES, 4_096);
    assert_eq!(MAX_PRIVACY_ACTIVE_REDIRECT_EDGES, 4_095);
    assert_eq!(MAX_PRIVACY_RELATIONSHIP_CANDIDATES, 16_384);
    assert_eq!(MAX_PRIVACY_CANDIDATE_RECORDS_REHYDRATED, 8_192);
    assert_eq!(MAX_PRIVACY_MERGE_RECORDS_REHYDRATED, 8_192);
    assert_eq!(MAX_PRIVACY_OWNER_RECORDS_SCANNED, 16_384);
}

#[test]
fn request_contract_preserves_identity_resolution_integrity_errors() {
    let wire = wire_request(DEFAULT_PAGE_SIZE, String::new());
    let valid = query_request(&wire);
    validate_request_contract(&valid).unwrap();

    let mut wrong_owner = valid.clone();
    wrong_owner.owner_module_id = ModuleId::try_new("crm.other-owner").unwrap();
    assert_invalid_argument(
        validate_request_contract(&wrong_owner).unwrap_err(),
        "IDENTITY_RESOLUTION_PRIVACY_SCOPE_REQUEST_BINDING_MISMATCH",
    );

    let mut wrong_hash = valid.clone();
    wrong_hash.input_hash = [9; 32];
    assert_invalid_argument(
        validate_request_contract(&wrong_hash).unwrap_err(),
        "IDENTITY_RESOLUTION_PRIVACY_SCOPE_INPUT_HASH_MISMATCH",
    );

    let mut wrong_schema = valid;
    wrong_schema.input.schema_id = schema_id(OUTPUT_SCHEMA_ID).unwrap();
    wrong_schema.input.descriptor_hash = output_descriptor_hash();
    wrong_schema.input.maximum_size_bytes = OUTPUT_MAXIMUM_BYTES;
    wrong_schema.input.retention_policy_id =
        configured(RetentionPolicyId::try_new(OUTPUT_RETENTION_POLICY_ID)).unwrap();
    assert_invalid_argument(
        validate_request_contract(&wrong_schema).unwrap_err(),
        "IDENTITY_RESOLUTION_PRIVACY_SCOPE_INPUT_CONTRACT_MISMATCH",
    );
}

#[test]
fn compound_cursor_round_trip_is_bound_to_family_lineage_and_page_size() {
    let validated = validate_wire_request(
        &context(),
        &wire_request(DEFAULT_PAGE_SIZE, String::new()).encode_to_vec(),
    )
    .unwrap();
    let candidate_cursor = encode_cursor(
        &validated,
        2,
        &CursorState {
            family: ResourceFamily::CandidateCase,
            after_record_id: Some(RecordId::try_new("candidate-001").unwrap()),
        },
    )
    .unwrap();
    let decoded = validate_wire_request(
        &context(),
        &wire_request(DEFAULT_PAGE_SIZE, candidate_cursor.clone()).encode_to_vec(),
    )
    .unwrap();
    assert_eq!(decoded.page_number, 2);
    assert_eq!(decoded.cursor_state.family, ResourceFamily::CandidateCase);
    assert_eq!(
        decoded.cursor_state.after_record_id.unwrap().as_str(),
        "candidate-001"
    );

    let merge_cursor = encode_cursor(
        &validated,
        2,
        &CursorState {
            family: ResourceFamily::MergeOperation,
            after_record_id: None,
        },
    )
    .unwrap();
    let merge_decoded = validate_wire_request(
        &context(),
        &wire_request(DEFAULT_PAGE_SIZE, merge_cursor).encode_to_vec(),
    )
    .unwrap();
    assert_eq!(
        merge_decoded.cursor_state.family,
        ResourceFamily::MergeOperation
    );
    assert!(merge_decoded.cursor_state.after_record_id.is_none());

    let rebound = validate_wire_request(
        &context(),
        &wire_request(DEFAULT_PAGE_SIZE - 1, candidate_cursor).encode_to_vec(),
    )
    .unwrap_err();
    assert_invalid_argument(rebound, "IDENTITY_RESOLUTION_PRIVACY_SCOPE_CURSOR_INVALID");
}

#[test]
fn response_is_cross_family_reference_only_and_deterministic() {
    let validated = validate_wire_request(
        &context(),
        &wire_request(DEFAULT_PAGE_SIZE, String::new()).encode_to_vec(),
    )
    .unwrap();
    let resources = vec![
        VerifiedIdentityResolutionResource {
            family: ResourceFamily::CandidateCase,
            record_id: RecordId::try_new("candidate-001").unwrap(),
            resource_version: 3,
        },
        VerifiedIdentityResolutionResource {
            family: ResourceFamily::MergeOperation,
            record_id: RecordId::try_new("merge-001").unwrap(),
            resource_version: 2,
        },
    ];
    let next = CursorState {
        family: ResourceFamily::MergeOperation,
        after_record_id: Some(RecordId::try_new("merge-001").unwrap()),
    };
    let first = build_response(&validated, &resources, 9, Some(&next)).unwrap();
    let second = build_response(&validated, &resources, 9, Some(&next)).unwrap();
    assert_eq!(first, second);

    let envelope = first.contribution.unwrap();
    assert_eq!(envelope.resources.len(), 2);
    assert_eq!(
        envelope.resources[0].resource_type,
        crm_identity_resolution_capability_adapter::RECORD_TYPE
    );
    assert_eq!(
        envelope.resources[1].resource_type,
        crm_identity_resolution_capability_adapter::MERGE_OPERATION_RECORD_TYPE
    );
    for resource in envelope.resources {
        assert_eq!(
            resource.evidence_class,
            privacy::PrivacyScopeEvidenceClass::RetainMinimizedEvidence as i32
        );
        assert_eq!(
            resource.data_class,
            privacy::CustomerDataClass::Personal as i32
        );
    }
    let evidence = envelope.page_evidence.unwrap();
    assert_eq!(evidence.scanned_resource_count, 9);
    assert_eq!(evidence.emitted_resource_count, 2);
    assert!(!evidence.terminal_complete);
    assert!(!evidence.next_cursor.is_empty());
}

#[test]
fn invalid_lineage_page_size_and_malformed_wire_are_rejected_before_owner_reads() {
    let mut tenant = wire_request(DEFAULT_PAGE_SIZE, String::new());
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
        "IDENTITY_RESOLUTION_PRIVACY_SCOPE_TENANT_MISMATCH",
    );

    let page_size = wire_request(MAXIMUM_PAGE_SIZE + 1, String::new());
    assert_invalid_argument(
        validate_wire_request(&context(), &page_size.encode_to_vec()).unwrap_err(),
        "IDENTITY_RESOLUTION_PRIVACY_SCOPE_PAGE_SIZE_INVALID",
    );

    let malformed = validate_wire_request(&context(), b"not-protobuf").unwrap_err();
    assert_invalid_argument(
        malformed,
        "IDENTITY_RESOLUTION_PRIVACY_SCOPE_REQUEST_INVALID",
    );
}
