use super::{
    CommonLineageError, QueryRequestContractError, framed_digest, validate_common_lineage,
    validate_query_request_contract,
};
use crm_capability_runtime::{CapabilityDefinition, CapabilityRisk, PayloadContract};
use crm_customer_privacy::{CANONICAL_SCOPE_REGISTRY_VERSION, OwnerScopeRegistry};
use crm_module_sdk::{
    ActorId, CapabilityId, CapabilityVersion, CorrelationId, DataClass, ModuleId, PayloadEncoding,
    RecordId, RequestId, RetentionPolicyId, SchemaId, SchemaVersion, TenantId, TraceId, TypedPayload,
};
use crm_proto_contracts::crm::{customer::v1 as customer, customer_privacy::v1 as privacy};
use crm_query_runtime::{QueryExecutionContext, QueryRequest};
use sha2::{Digest, Sha256};

const OWNER_ID: &str = "crm.test-privacy-owner";
const CAPABILITY_ID: &str = "test_owner.privacy.scope.contribute";
const CAPABILITY_VERSION: &str = "1.0.0";
const INPUT_SCHEMA_ID: &str = "crm.test_privacy_owner.scope.request";
const INPUT_SCHEMA_VERSION: &str = "1.0.0";
const INPUT_MAXIMUM_BYTES: u64 = 1_024;

fn context() -> QueryExecutionContext {
    QueryExecutionContext {
        tenant_id: TenantId::try_new("tenant-a").unwrap(),
        actor_id: ActorId::try_new("privacy-worker").unwrap(),
        request_id: RequestId::try_new("request-shared-scope").unwrap(),
        correlation_id: CorrelationId::try_new("correlation-shared-scope").unwrap(),
        trace_id: TraceId::try_new("trace-shared-scope").unwrap(),
        capability_id: CapabilityId::try_new(CAPABILITY_ID).unwrap(),
        capability_version: CapabilityVersion::try_new(CAPABILITY_VERSION).unwrap(),
        schema_version: SchemaVersion::try_new(INPUT_SCHEMA_VERSION).unwrap(),
        request_started_at_unix_nanos: 2_000_000_000,
    }
}

fn definition() -> CapabilityDefinition {
    CapabilityDefinition {
        capability_id: CapabilityId::try_new(CAPABILITY_ID).unwrap(),
        capability_version: CapabilityVersion::try_new(CAPABILITY_VERSION).unwrap(),
        owner_module_id: ModuleId::try_new(OWNER_ID).unwrap(),
        input_contract: PayloadContract {
            owner: ModuleId::try_new(OWNER_ID).unwrap(),
            schema_id: SchemaId::try_new(INPUT_SCHEMA_ID).unwrap(),
            schema_version: SchemaVersion::try_new(INPUT_SCHEMA_VERSION).unwrap(),
            descriptor_hash: [7; 32],
            allowed_data_classes: vec![DataClass::Confidential],
            allowed_encodings: vec![PayloadEncoding::Protobuf],
            maximum_size_bytes: INPUT_MAXIMUM_BYTES,
        },
        output_contract: None,
        risk: CapabilityRisk::Medium,
        mutation: false,
        requires_idempotency: false,
        requires_approval: false,
        authorization_policy_id: CAPABILITY_ID.to_owned(),
        rate_limit_policy_id: None,
    }
}

fn query_request() -> QueryRequest {
    let bytes = vec![1, 2, 3, 4];
    QueryRequest {
        owner_module_id: ModuleId::try_new(OWNER_ID).unwrap(),
        context: context(),
        input: TypedPayload {
            owner: ModuleId::try_new(OWNER_ID).unwrap(),
            schema_id: SchemaId::try_new(INPUT_SCHEMA_ID).unwrap(),
            schema_version: SchemaVersion::try_new(INPUT_SCHEMA_VERSION).unwrap(),
            descriptor_hash: [7; 32],
            data_class: DataClass::Confidential,
            encoding: PayloadEncoding::Protobuf,
            maximum_size_bytes: INPUT_MAXIMUM_BYTES,
            retention_policy_id: RetentionPolicyId::try_new("crm.test_privacy_owner.scope.request")
                .unwrap(),
            bytes,
        },
        input_hash: [0; 32],
    }
}

fn valid_query_request() -> QueryRequest {
    let mut request = query_request();
    request.input_hash = Sha256::digest(&request.input.bytes).into();
    request
}

fn lineage() -> privacy::PrivacyScopeContributionLineage {
    let registry = OwnerScopeRegistry::canonical_v1().unwrap();
    privacy::PrivacyScopeContributionLineage {
        privacy_case_id: "privacy-case-shared".to_owned(),
        tenant_id: "tenant-a".to_owned(),
        canonical_party_ref: Some(customer::PartyRef {
            party_id: "party-a".to_owned(),
        }),
        identity_resolution_generation: 7,
        registry_version: CANONICAL_SCOPE_REGISTRY_VERSION.to_owned(),
        registry_digest_sha256: registry.digest().to_vec(),
        purpose_code: "PRIVACY_ERASURE_SCOPE".to_owned(),
        effective_request_at_unix_ms: 1_000,
    }
}

#[test]
fn framed_digest_preserves_field_boundaries_and_domain() {
    let first = framed_digest(b"privacy-scope/v1", &[b"ab", b"c"]);
    let second = framed_digest(b"privacy-scope/v1", &[b"a", b"bc"]);
    let other_domain = framed_digest(b"privacy-scope/v2", &[b"ab", b"c"]);
    assert_ne!(first, second);
    assert_ne!(first, other_domain);
    assert_eq!(first, framed_digest(b"privacy-scope/v1", &[b"ab", b"c"]));
}

#[test]
fn query_request_contract_accepts_exact_binding_contract_and_hash() {
    validate_query_request_contract(&valid_query_request(), &definition()).unwrap();
}

#[test]
fn query_request_contract_rejects_every_integrity_boundary() {
    let expected = definition();

    let mut invalid_context = valid_query_request();
    invalid_context.context.request_started_at_unix_nanos = -1;
    assert!(matches!(
        validate_query_request_contract(&invalid_context, &expected).unwrap_err(),
        QueryRequestContractError::InvalidContext(_)
    ));

    let mut invalid_input = valid_query_request();
    invalid_input.input.descriptor_hash = [0; 32];
    assert!(matches!(
        validate_query_request_contract(&invalid_input, &expected).unwrap_err(),
        QueryRequestContractError::InvalidInput(_)
    ));

    let mut owner_mismatch = valid_query_request();
    owner_mismatch.owner_module_id = ModuleId::try_new("crm.other-owner").unwrap();
    assert!(matches!(
        validate_query_request_contract(&owner_mismatch, &expected).unwrap_err(),
        QueryRequestContractError::BindingMismatch
    ));

    let mut capability_mismatch = valid_query_request();
    capability_mismatch.context.capability_id = CapabilityId::try_new("other.capability").unwrap();
    assert!(matches!(
        validate_query_request_contract(&capability_mismatch, &expected).unwrap_err(),
        QueryRequestContractError::BindingMismatch
    ));

    let mut contract_mismatch = valid_query_request();
    contract_mismatch.input.schema_id = SchemaId::try_new("crm.other.request").unwrap();
    assert!(matches!(
        validate_query_request_contract(&contract_mismatch, &expected).unwrap_err(),
        QueryRequestContractError::InputContractMismatch
    ));

    let mut hash_mismatch = valid_query_request();
    hash_mismatch.input_hash = [9; 32];
    assert!(matches!(
        validate_query_request_contract(&hash_mismatch, &expected).unwrap_err(),
        QueryRequestContractError::InputHashMismatch
    ));
}

#[test]
fn common_lineage_defaults_page_size_and_preserves_identity() {
    let validated = validate_common_lineage(&context(), lineage(), 0, 64, 128).unwrap();
    assert_eq!(validated.canonical_party_id.as_str(), "party-a");
    assert_eq!(validated.identity_resolution_generation, 7);
    assert_eq!(validated.page_size, 64);
    assert_eq!(validated.lineage.privacy_case_id, "privacy-case-shared");
}

#[test]
fn common_lineage_rejects_identity_and_registry_substitution() {
    let mut tenant = lineage();
    tenant.tenant_id = "tenant-b".to_owned();
    assert_eq!(
        validate_common_lineage(&context(), tenant, 64, 64, 128).unwrap_err(),
        CommonLineageError::TenantMismatch
    );

    let mut case = lineage();
    case.privacy_case_id.clear();
    assert!(matches!(
        validate_common_lineage(&context(), case, 64, 64, 128).unwrap_err(),
        CommonLineageError::CaseIdInvalid(_)
    ));

    let mut missing_party = lineage();
    missing_party.canonical_party_ref = None;
    assert_eq!(
        validate_common_lineage(&context(), missing_party, 64, 64, 128).unwrap_err(),
        CommonLineageError::PartyMissing
    );

    let mut invalid_party = lineage();
    invalid_party.canonical_party_ref.as_mut().unwrap().party_id.clear();
    assert!(matches!(
        validate_common_lineage(&context(), invalid_party, 64, 64, 128).unwrap_err(),
        CommonLineageError::PartyInvalid(_)
    ));

    let mut generation = lineage();
    generation.identity_resolution_generation = 0;
    assert_eq!(
        validate_common_lineage(&context(), generation, 64, 64, 128).unwrap_err(),
        CommonLineageError::GenerationInvalid
    );

    let mut registry_shape = lineage();
    registry_shape.registry_digest_sha256 = vec![0; 32];
    assert_eq!(
        validate_common_lineage(&context(), registry_shape, 64, 64, 128).unwrap_err(),
        CommonLineageError::RegistryInvalid
    );

    let mut substituted = lineage();
    substituted.registry_digest_sha256 = vec![9; 32];
    assert_eq!(
        validate_common_lineage(&context(), substituted, 64, 64, 128).unwrap_err(),
        CommonLineageError::RegistryMismatch
    );
}

#[test]
fn common_lineage_rejects_invalid_purpose_time_and_page_size() {
    let mut purpose = lineage();
    purpose.purpose_code = "not-normalized".to_owned();
    assert_eq!(
        validate_common_lineage(&context(), purpose, 64, 64, 128).unwrap_err(),
        CommonLineageError::PurposeInvalid
    );

    let mut future = lineage();
    future.effective_request_at_unix_ms = 3_000;
    assert_eq!(
        validate_common_lineage(&context(), future, 64, 64, 128).unwrap_err(),
        CommonLineageError::RequestTimeInvalid
    );

    assert_eq!(
        validate_common_lineage(&context(), lineage(), 129, 64, 128).unwrap_err(),
        CommonLineageError::PageSizeInvalid
    );
}

#[test]
fn validated_common_lineage_keeps_the_exact_canonical_party_id() {
    let validated = validate_common_lineage(&context(), lineage(), 64, 64, 128).unwrap();
    assert_eq!(
        validated.canonical_party_id,
        RecordId::try_new("party-a").unwrap()
    );
}
