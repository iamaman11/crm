use crate::contract::{
    CAPABILITY_ID, CAPABILITY_VERSION, CONTRACT_SCHEMA_VERSION, DEFAULT_PAGE_SIZE,
    INPUT_MAXIMUM_BYTES, INPUT_RETENTION_POLICY_ID, INPUT_SCHEMA_ID,
    MAX_PRIVACY_ASSOCIATED_EXPORT_RECORDS_REHYDRATED, MAX_PRIVACY_CANONICAL_PARTY_RESOLUTIONS,
    MAX_PRIVACY_EXPORT_SELECTION_ITEMS_SCANNED, MAX_PRIVACY_IMPORT_ROWS_SCANNED,
    MAX_PRIVACY_OWNER_RECORDS_SCANNED, MAXIMUM_PAGE_SIZE, OUTPUT_MAXIMUM_BYTES,
    OUTPUT_RETENTION_POLICY_ID, OUTPUT_SCHEMA_ID, customer_data_privacy_scope_definition, module_id,
    output_descriptor_hash, schema_id, schema_version,
};
use crate::errors::configured;
use crate::request::{
    CursorState, ResourceFamily, encode_cursor, validate_request_contract, validate_wire_request,
};
use crate::response::{VerifiedCustomerDataResource, build_response, typed_output};
use crm_capability_runtime::CapabilityRisk;
use crm_customer_data_operations_capability_adapter::MODULE_ID;
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
        request_id: RequestId::try_new("request-customer-data-scope").unwrap(),
        correlation_id: CorrelationId::try_new("correlation-customer-data-scope").unwrap(),
        trace_id: TraceId::try_new("trace-customer-data-scope").unwrap(),
        capability_id: CapabilityId::try_new(CAPABILITY_ID).unwrap(),
        capability_version: CapabilityVersion::try_new(CAPABILITY_VERSION).unwrap(),
        schema_version: schema_version(CONTRACT_SCHEMA_VERSION).unwrap(),
        request_started_at_unix_nanos: 2_000_000_000,
    }
}

fn wire_request(
    page_size: u32,
    cursor: String,
) -> privacy::CustomerDataPrivacyScopeContributionRequest {
    let registry = OwnerScopeRegistry::canonical_v1().unwrap();
    privacy::CustomerDataPrivacyScopeContributionRequest {
        contribution: Some(privacy::PrivacyScopeContributionRequestEnvelope {
            lineage: Some(privacy::PrivacyScopeContributionLineage {
                privacy_case_id: "privacy-case-customer-data".to_owned(),
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

fn query_request(message: &privacy::CustomerDataPrivacyScopeContributionRequest) -> QueryRequest {
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
    let definition = customer_data_privacy_scope_definition().unwrap();
    assert_eq!(definition.owner_module_id.as_str(), MODULE_ID);
    assert_eq!(definition.capability_id.as_str(), CAPABILITY_ID);
    assert_eq!(definition.capability_version.as_str(), CAPABILITY_VERSION);
    assert_eq!(definition.risk, CapabilityRisk::Medium);
    assert!(!definition.mutation);
    assert!(!definition.requires_idempotency);
    assert!(!definition.requires_approval);
    assert_eq!(MAX_PRIVACY_IMPORT_ROWS_SCANNED, 16_384);
    assert_eq!(MAX_PRIVACY_EXPORT_SELECTION_ITEMS_SCANNED, 16_384);
    assert_eq!(MAX_PRIVACY_ASSOCIATED_EXPORT_RECORDS_REHYDRATED, 32_768);
    assert_eq!(MAX_PRIVACY_CANONICAL_PARTY_RESOLUTIONS, 32_768);
    assert_eq!(MAX_PRIVACY_OWNER_RECORDS_SCANNED, 65_536);
}

#[test]
fn request_contract_preserves_customer_data_integrity_errors() {
    let wire = wire_request(DEFAULT_PAGE_SIZE, String::new());
    let valid = query_request(&wire);
    validate_request_contract(&valid).unwrap();

    let mut wrong_owner = valid.clone();
    wrong_owner.owner_module_id = ModuleId::try_new("crm.other-owner").unwrap();
    assert_invalid_argument(
        validate_request_contract(&wrong_owner).unwrap_err(),
        "CUSTOMER_DATA_PRIVACY_SCOPE_REQUEST_BINDING_MISMATCH",
    );

    let mut wrong_hash = valid.clone();
    wrong_hash.input_hash = [9; 32];
    assert_invalid_argument(
        validate_request_contract(&wrong_hash).unwrap_err(),
        "CUSTOMER_DATA_PRIVACY_SCOPE_INPUT_HASH_MISMATCH",
    );

    let mut wrong_schema = valid;
    wrong_schema.input.schema_id = schema_id(OUTPUT_SCHEMA_ID).unwrap();
    wrong_schema.input.descriptor_hash = output_descriptor_hash();
    wrong_schema.input.maximum_size_bytes = OUTPUT_MAXIMUM_BYTES;
    wrong_schema.input.retention_policy_id =
        configured(RetentionPolicyId::try_new(OUTPUT_RETENTION_POLICY_ID)).unwrap();
    assert_invalid_argument(
        validate_request_contract(&wrong_schema).unwrap_err(),
        "CUSTOMER_DATA_PRIVACY_SCOPE_INPUT_CONTRACT_MISMATCH",
    );
}

#[test]
fn four_family_cursor_round_trip_is_bound_to_lineage_and_page_size() {
    let validated = validate_wire_request(
        &context(),
        &wire_request(DEFAULT_PAGE_SIZE, String::new()).encode_to_vec(),
    )
    .unwrap();
    assert_eq!(validated.identity_resolution_generation, 7);

    let import_cursor = encode_cursor(
        &validated,
        2,
        &CursorState {
            family: ResourceFamily::ImportRow,
            after_record_id: Some(RecordId::try_new("import-row-001").unwrap()),
        },
    )
    .unwrap();
    let decoded = validate_wire_request(
        &context(),
        &wire_request(DEFAULT_PAGE_SIZE, import_cursor.clone()).encode_to_vec(),
    )
    .unwrap();
    assert_eq!(decoded.page_number, 2);
    assert_eq!(decoded.cursor_state.family, ResourceFamily::ImportRow);

    let stage_cursor = encode_cursor(
        &validated,
        2,
        &CursorState {
            family: ResourceFamily::ExportExecutionStage,
            after_record_id: None,
        },
    )
    .unwrap();
    let stage = validate_wire_request(
        &context(),
        &wire_request(DEFAULT_PAGE_SIZE, stage_cursor).encode_to_vec(),
    )
    .unwrap();
    assert_eq!(stage.cursor_state.family, ResourceFamily::ExportExecutionStage);
    assert!(stage.cursor_state.after_record_id.is_none());

    assert_invalid_argument(
        validate_wire_request(
            &context(),
            &wire_request(DEFAULT_PAGE_SIZE - 1, import_cursor).encode_to_vec(),
        )
        .unwrap_err(),
        "CUSTOMER_DATA_PRIVACY_SCOPE_CURSOR_INVALID",
    );
}

#[test]
fn response_is_four_family_reference_only_and_deterministic() {
    let validated = validate_wire_request(
        &context(),
        &wire_request(DEFAULT_PAGE_SIZE, String::new()).encode_to_vec(),
    )
    .unwrap();
    let resources = vec![
        VerifiedCustomerDataResource {
            family: ResourceFamily::ImportRow,
            record_id: RecordId::try_new("import-row-001").unwrap(),
            resource_version: 3,
        },
        VerifiedCustomerDataResource {
            family: ResourceFamily::ExportSelectionItem,
            record_id: RecordId::try_new("selection-001").unwrap(),
            resource_version: 1,
        },
        VerifiedCustomerDataResource {
            family: ResourceFamily::ExportExecutionStage,
            record_id: RecordId::try_new("stage-001").unwrap(),
            resource_version: 1,
        },
        VerifiedCustomerDataResource {
            family: ResourceFamily::ExportExecutionOutcome,
            record_id: RecordId::try_new("outcome-001").unwrap(),
            resource_version: 1,
        },
    ];
    let next = CursorState {
        family: ResourceFamily::ExportExecutionOutcome,
        after_record_id: Some(RecordId::try_new("outcome-001").unwrap()),
    };
    let first = build_response(&validated, &resources, 17, Some(&next)).unwrap();
    let second = build_response(&validated, &resources, 17, Some(&next)).unwrap();
    assert_eq!(first, second);
    let bytes = first.encode_to_vec();
    typed_output(bytes).unwrap();

    let envelope = first.contribution.unwrap();
    assert_eq!(envelope.resources.len(), 4);
    assert_eq!(envelope.resources[0].resource_type, "customer_data.import_row");
    assert_eq!(
        envelope.resources[1].resource_type,
        "customer_data.export_selection_item"
    );
    assert_eq!(
        envelope.resources[2].resource_type,
        "customer_data.export_execution_stage"
    );
    assert_eq!(
        envelope.resources[3].resource_type,
        "customer_data.export_execution_outcome"
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
    assert_eq!(evidence.scanned_resource_count, 17);
    assert_eq!(evidence.emitted_resource_count, 4);
    assert!(!evidence.terminal_complete);
    assert!(!evidence.next_cursor.is_empty());
}

#[test]
fn invalid_lineage_page_size_and_owner_errors_are_stable() {
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
        "CUSTOMER_DATA_PRIVACY_SCOPE_TENANT_MISMATCH",
    );

    let page_size = wire_request(MAXIMUM_PAGE_SIZE + 1, String::new());
    assert_invalid_argument(
        validate_wire_request(&context(), &page_size.encode_to_vec()).unwrap_err(),
        "CUSTOMER_DATA_PRIVACY_SCOPE_PAGE_SIZE_INVALID",
    );

    assert_invalid_argument(
        validate_wire_request(&context(), b"not-protobuf").unwrap_err(),
        "CUSTOMER_DATA_PRIVACY_SCOPE_REQUEST_INVALID",
    );
    assert_eq!(
        crate::errors::association_state_invalid("private association").code,
        "CUSTOMER_DATA_PRIVACY_SCOPE_ASSOCIATION_STATE_INVALID"
    );
    assert_eq!(
        crate::errors::canonical_resolution_unavailable("private party").safe_message,
        "The Customer Data Operations privacy scope is temporarily unavailable."
    );
}
