use crate::contract::{
    CAPABILITY_ID, CAPABILITY_VERSION, CONTRACT_SCHEMA_VERSION, DEFAULT_PAGE_SIZE,
    INPUT_MAXIMUM_BYTES, INPUT_RETENTION_POLICY_ID, INPUT_SCHEMA_ID, OUTPUT_MAXIMUM_BYTES,
    OUTPUT_RETENTION_POLICY_ID, OUTPUT_SCHEMA_ID, input_descriptor_hash, module_id,
    output_descriptor_hash, parties_privacy_scope_definition, schema_id, schema_version,
};
use crate::errors::configured;
use crate::request::{validate_request_contract, validate_wire_request};
use crate::response::build_response;
use crm_customer_privacy::{CANONICAL_SCOPE_REGISTRY_VERSION, canonical_scope_registry};
use crm_module_sdk::{
    ActorId, CapabilityId, CapabilityVersion, CorrelationId, DataClass, PayloadEncoding, RequestId,
    RetentionPolicyId, TraceId, TypedPayload,
};
use crm_proto_contracts::crm::{customer::v1 as customer, customer_privacy::v1 as privacy};
use crm_query_runtime::{QueryExecutionContext, QueryRequest};
use prost::Message;
use sha2::{Digest, Sha256};

fn context() -> QueryExecutionContext {
    QueryExecutionContext {
        tenant_id: crm_module_sdk::TenantId::try_new("tenant-a").unwrap(),
        actor_id: ActorId::try_new("privacy-worker").unwrap(),
        request_id: RequestId::try_new("request-1").unwrap(),
        correlation_id: CorrelationId::try_new("correlation-1").unwrap(),
        trace_id: TraceId::try_new("trace-1").unwrap(),
        capability_id: CapabilityId::try_new(CAPABILITY_ID).unwrap(),
        capability_version: CapabilityVersion::try_new(CAPABILITY_VERSION).unwrap(),
        schema_version: schema_version(CONTRACT_SCHEMA_VERSION).unwrap(),
        request_started_at_unix_nanos: 2_000_000_000,
    }
}

fn valid_wire_request() -> privacy::PartiesPrivacyScopeContributionRequest {
    let registry = canonical_scope_registry().unwrap();
    privacy::PartiesPrivacyScopeContributionRequest {
        contribution: Some(privacy::PrivacyScopeContributionRequestEnvelope {
            lineage: Some(privacy::PrivacyScopeContributionLineage {
                privacy_case_id: "privacy-case-1".to_owned(),
                tenant_id: "tenant-a".to_owned(),
                canonical_party_ref: Some(customer::PartyRef {
                    party_id: "party-1".to_owned(),
                }),
                identity_resolution_generation: 7,
                registry_version: CANONICAL_SCOPE_REGISTRY_VERSION.to_owned(),
                registry_digest_sha256: registry.digest().to_vec(),
                purpose_code: "PRIVACY_ERASURE_SCOPE".to_owned(),
                effective_request_at_unix_ms: 1_000,
            }),
            page_size: 0,
            cursor: String::new(),
        }),
    }
}

fn query_request(message: &privacy::PartiesPrivacyScopeContributionRequest) -> QueryRequest {
    let bytes = message.encode_to_vec();
    let input_hash: [u8; 32] = Sha256::digest(&bytes).into();
    QueryRequest {
        owner_module_id: module_id().unwrap(),
        context: context(),
        input: TypedPayload {
            owner: module_id().unwrap(),
            schema_id: schema_id(INPUT_SCHEMA_ID).unwrap(),
            schema_version: schema_version(CONTRACT_SCHEMA_VERSION).unwrap(),
            descriptor_hash: input_descriptor_hash(),
            data_class: DataClass::Confidential,
            encoding: PayloadEncoding::Protobuf,
            maximum_size_bytes: INPUT_MAXIMUM_BYTES,
            retention_policy_id: configured(RetentionPolicyId::try_new(
                INPUT_RETENTION_POLICY_ID,
            ))
            .unwrap(),
            bytes,
        },
        input_hash,
    }
}

#[test]
fn definition_is_internal_read_only_and_exactly_bound() {
    let definition = parties_privacy_scope_definition().unwrap();
    assert_eq!(definition.capability_id.as_str(), CAPABILITY_ID);
    assert_eq!(definition.capability_version.as_str(), CAPABILITY_VERSION);
    assert_eq!(definition.owner_module_id.as_str(), crm_parties::MODULE_ID);
    assert!(!definition.mutation);
    assert!(!definition.requires_idempotency);
    assert!(!definition.requires_approval);
    assert_eq!(definition.authorization_policy_id, CAPABILITY_ID);
    assert_eq!(
        definition.input_contract.maximum_size_bytes,
        INPUT_MAXIMUM_BYTES
    );
    assert_eq!(
        definition
            .output_contract
            .as_ref()
            .unwrap()
            .maximum_size_bytes,
        OUTPUT_MAXIMUM_BYTES
    );
}

#[test]
fn request_contract_requires_exact_payload_hash() {
    let wire = valid_wire_request();
    let request = query_request(&wire);
    validate_request_contract(&request).unwrap();

    let mut wrong_hash = request.clone();
    wrong_hash.input_hash = [9; 32];
    assert_eq!(
        validate_request_contract(&wrong_hash).unwrap_err().code,
        "PARTIES_PRIVACY_SCOPE_INPUT_HASH_MISMATCH"
    );

    let mut wrong_schema = request;
    wrong_schema.input.schema_id = schema_id(OUTPUT_SCHEMA_ID).unwrap();
    wrong_schema.input.descriptor_hash = output_descriptor_hash();
    wrong_schema.input.maximum_size_bytes = OUTPUT_MAXIMUM_BYTES;
    wrong_schema.input.retention_policy_id =
        configured(RetentionPolicyId::try_new(OUTPUT_RETENTION_POLICY_ID)).unwrap();
    assert_eq!(
        validate_request_contract(&wrong_schema).unwrap_err().code,
        "PARTIES_PRIVACY_SCOPE_INPUT_CONTRACT_MISMATCH"
    );
}

#[test]
fn wire_validation_defaults_page_size_and_rejects_registry_substitution() {
    let request = valid_wire_request();
    let validated = validate_wire_request(&context(), &request.encode_to_vec()).unwrap();
    assert_eq!(validated.page_size, DEFAULT_PAGE_SIZE);

    let mut invalid = request;
    invalid
        .contribution
        .as_mut()
        .unwrap()
        .lineage
        .as_mut()
        .unwrap()
        .registry_digest_sha256 = vec![9; 32];
    assert_eq!(
        validate_wire_request(&context(), &invalid.encode_to_vec())
            .unwrap_err()
            .code,
        "PARTIES_PRIVACY_SCOPE_REGISTRY_MISMATCH"
    );
}

#[test]
fn response_is_reference_only_terminal_and_deterministic() {
    let wire = valid_wire_request();
    let validated = validate_wire_request(&context(), &wire.encode_to_vec()).unwrap();
    let first = build_response(&validated, 3);
    let second = build_response(&validated, 3);
    assert_eq!(first, second);

    let envelope = first.contribution.unwrap();
    assert_eq!(envelope.resources.len(), 1);
    assert_eq!(envelope.resources[0].resource_id, "party-1");
    assert_eq!(envelope.resources[0].resource_version, 3);
    assert_eq!(
        envelope.resources[0].evidence_class,
        privacy::PrivacyScopeEvidenceClass::RetainMinimizedEvidence as i32
    );
    let page = envelope.page_evidence.unwrap();
    assert!(page.terminal_complete);
    assert!(page.next_cursor.is_empty());
    assert_eq!(page.cursor_digest_sha256.len(), 32);
    assert_eq!(page.page_digest_sha256.len(), 32);
}

#[test]
fn non_empty_cursor_future_time_and_invalid_purpose_fail_closed() {
    let mut cursor = valid_wire_request();
    cursor.contribution.as_mut().unwrap().cursor = "unexpected".to_owned();
    assert_eq!(
        validate_wire_request(&context(), &cursor.encode_to_vec())
            .unwrap_err()
            .code,
        "PARTIES_PRIVACY_SCOPE_CURSOR_INVALID"
    );

    let mut future = valid_wire_request();
    future
        .contribution
        .as_mut()
        .unwrap()
        .lineage
        .as_mut()
        .unwrap()
        .effective_request_at_unix_ms = 3_000;
    assert_eq!(
        validate_wire_request(&context(), &future.encode_to_vec())
            .unwrap_err()
            .code,
        "PARTIES_PRIVACY_SCOPE_REQUEST_TIME_INVALID"
    );

    let mut purpose = valid_wire_request();
    purpose
        .contribution
        .as_mut()
        .unwrap()
        .lineage
        .as_mut()
        .unwrap()
        .purpose_code = "not-normalized".to_owned();
    assert_eq!(
        validate_wire_request(&context(), &purpose.encode_to_vec())
            .unwrap_err()
            .code,
        "PARTIES_PRIVACY_SCOPE_PURPOSE_INVALID"
    );
}
