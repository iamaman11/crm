use crate::contract::{
    CAPABILITY_ID, CAPABILITY_VERSION, CONTRACT_SCHEMA_VERSION, DEFAULT_PAGE_SIZE,
    INPUT_MAXIMUM_BYTES, INPUT_RETENTION_POLICY_ID, INPUT_SCHEMA_ID, MAXIMUM_PAGE_SIZE,
    OUTPUT_MAXIMUM_BYTES, OUTPUT_RETENTION_POLICY_ID, OUTPUT_SCHEMA_ID,
    customer_accounts_privacy_scope_definition, module_id, output_descriptor_hash, schema_id,
    schema_version,
};
use crate::errors::configured;
use crate::request::{encode_cursor, validate_request_contract, validate_wire_request};
use crate::response::{VerifiedAccountResource, build_response};
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
        request_id: RequestId::try_new("request-account-scope").unwrap(),
        correlation_id: CorrelationId::try_new("correlation-account-scope").unwrap(),
        trace_id: TraceId::try_new("trace-account-scope").unwrap(),
        capability_id: CapabilityId::try_new(CAPABILITY_ID).unwrap(),
        capability_version: CapabilityVersion::try_new(CAPABILITY_VERSION).unwrap(),
        schema_version: schema_version(CONTRACT_SCHEMA_VERSION).unwrap(),
        request_started_at_unix_nanos: 2_000_000_000,
    }
}

fn wire_request(
    page_size: u32,
    cursor: String,
) -> privacy::CustomerAccountsPrivacyScopeContributionRequest {
    let registry = OwnerScopeRegistry::canonical_v1().unwrap();
    privacy::CustomerAccountsPrivacyScopeContributionRequest {
        contribution: Some(privacy::PrivacyScopeContributionRequestEnvelope {
            lineage: Some(privacy::PrivacyScopeContributionLineage {
                privacy_case_id: "privacy-case-accounts".to_owned(),
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
    message: &privacy::CustomerAccountsPrivacyScopeContributionRequest,
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
fn publishes_exact_contract_only_coordinate() {
    let definition = customer_accounts_privacy_scope_definition().unwrap();
    assert_eq!(
        definition.owner_module_id.as_str(),
        crm_customer_accounts::MODULE_ID
    );
    assert_eq!(definition.capability_id.as_str(), CAPABILITY_ID);
    assert_eq!(definition.capability_version.as_str(), CAPABILITY_VERSION);
    assert!(!definition.mutation);
    assert!(!definition.requires_idempotency);
    assert!(!definition.requires_approval);
}

#[test]
fn request_contract_preserves_customer_accounts_integrity_errors() {
    let wire = wire_request(DEFAULT_PAGE_SIZE, String::new());
    let valid = query_request(&wire);
    validate_request_contract(&valid).unwrap();

    let mut wrong_owner = valid.clone();
    wrong_owner.owner_module_id = ModuleId::try_new("crm.other-owner").unwrap();
    assert_invalid_argument(
        validate_request_contract(&wrong_owner).unwrap_err(),
        "CUSTOMER_ACCOUNTS_PRIVACY_SCOPE_REQUEST_BINDING_MISMATCH",
    );

    let mut wrong_hash = valid.clone();
    wrong_hash.input_hash = [9; 32];
    assert_invalid_argument(
        validate_request_contract(&wrong_hash).unwrap_err(),
        "CUSTOMER_ACCOUNTS_PRIVACY_SCOPE_INPUT_HASH_MISMATCH",
    );

    let mut wrong_schema = valid;
    wrong_schema.input.schema_id = schema_id(OUTPUT_SCHEMA_ID).unwrap();
    wrong_schema.input.descriptor_hash = output_descriptor_hash();
    wrong_schema.input.maximum_size_bytes = OUTPUT_MAXIMUM_BYTES;
    wrong_schema.input.retention_policy_id =
        configured(RetentionPolicyId::try_new(OUTPUT_RETENTION_POLICY_ID)).unwrap();
    assert_invalid_argument(
        validate_request_contract(&wrong_schema).unwrap_err(),
        "CUSTOMER_ACCOUNTS_PRIVACY_SCOPE_INPUT_CONTRACT_MISMATCH",
    );
}

#[test]
fn cursor_round_trip_is_bound_to_lineage_and_page_size() {
    let validated = validate_wire_request(
        &context(),
        &wire_request(DEFAULT_PAGE_SIZE, String::new()).encode_to_vec(),
    )
    .unwrap();
    let cursor = encode_cursor(
        &validated,
        2,
        &RecordId::try_new("account-001").unwrap(),
    )
    .unwrap();
    let decoded = validate_wire_request(
        &context(),
        &wire_request(DEFAULT_PAGE_SIZE, cursor.clone()).encode_to_vec(),
    )
    .unwrap();
    assert_eq!(decoded.page_number, 2);
    assert_eq!(decoded.after_record_id.unwrap().as_str(), "account-001");

    let rebound = validate_wire_request(
        &context(),
        &wire_request(DEFAULT_PAGE_SIZE - 1, cursor).encode_to_vec(),
    )
    .unwrap_err();
    assert_invalid_argument(
        rebound,
        "CUSTOMER_ACCOUNTS_PRIVACY_SCOPE_CURSOR_INVALID",
    );
}

#[test]
fn common_lineage_failures_keep_customer_accounts_codes() {
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
        "CUSTOMER_ACCOUNTS_PRIVACY_SCOPE_TENANT_MISMATCH",
    );

    let page_size = wire_request(MAXIMUM_PAGE_SIZE + 1, String::new());
    assert_invalid_argument(
        validate_wire_request(&context(), &page_size.encode_to_vec()).unwrap_err(),
        "CUSTOMER_ACCOUNTS_PRIVACY_SCOPE_PAGE_SIZE_INVALID",
    );
}

#[test]
fn response_is_reference_only_deterministic_and_supports_sparse_progress() {
    let validated = validate_wire_request(
        &context(),
        &wire_request(DEFAULT_PAGE_SIZE, String::new()).encode_to_vec(),
    )
    .unwrap();
    let resources = vec![VerifiedAccountResource {
        record_id: RecordId::try_new("account-001").unwrap(),
        resource_version: 3,
    }];
    let next_after = RecordId::try_new("account-127").unwrap();
    let first = build_response(&validated, &resources, 128, Some(&next_after)).unwrap();
    let second = build_response(&validated, &resources, 128, Some(&next_after)).unwrap();
    assert_eq!(first, second);

    let envelope = first.contribution.unwrap();
    assert_eq!(envelope.resources.len(), 1);
    assert_eq!(envelope.resources[0].resource_id, "account-001");
    assert_eq!(
        envelope.resources[0].evidence_class,
        privacy::PrivacyScopeEvidenceClass::RetainMinimizedEvidence as i32
    );
    assert_eq!(
        envelope.resources[0].data_class,
        privacy::CustomerDataClass::Personal as i32
    );
    let evidence = envelope.page_evidence.unwrap();
    assert_eq!(evidence.scanned_resource_count, 128);
    assert_eq!(evidence.emitted_resource_count, 1);
    assert!(!evidence.terminal_complete);
    assert!(!evidence.next_cursor.is_empty());

    let sparse = build_response(&validated, &[], 128, Some(&next_after)).unwrap();
    let sparse_evidence = sparse.contribution.unwrap().page_evidence.unwrap();
    assert_eq!(sparse_evidence.emitted_resource_count, 0);
    assert!(!sparse_evidence.terminal_complete);
    assert!(!sparse_evidence.next_cursor.is_empty());
}

#[test]
fn malformed_wire_request_is_rejected_before_owner_reads() {
    let error = validate_wire_request(&context(), b"not-protobuf").unwrap_err();
    assert_invalid_argument(
        error,
        "CUSTOMER_ACCOUNTS_PRIVACY_SCOPE_REQUEST_INVALID",
    );
}
