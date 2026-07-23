#![forbid(unsafe_code)]

use crm_capability_runtime::{CapabilityDefinition, PayloadContract, RiskLevel};
use crm_core_data::PostgresDataStore;
use crm_customer_privacy::{CANONICAL_SCOPE_REGISTRY_VERSION, canonical_scope_registry};
use crm_identity_resolution_topology_composition::prove_canonical_party_in_transaction;
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, PayloadEncoding,
    PortFuture, RecordId, RecordRef, RecordSnapshot, RecordType, RetentionPolicyId, SchemaId,
    SchemaVersion, SdkError, TenantId, TypedPayload,
};
use crm_parties::{
    MODULE_ID, PARTY_RECORD_TYPE, PARTY_STATE_MAXIMUM_BYTES, PARTY_STATE_RETENTION_POLICY_ID,
    PARTY_STATE_SCHEMA_ID, PARTY_STATE_SCHEMA_VERSION, party_state_descriptor_hash,
};
use crm_parties_capability_adapter::party_from_snapshot;
use crm_proto_contracts::{crm::customer_privacy::v1 as privacy, message_descriptor_hash};
use crm_query_runtime::{QueryExecutionContext, QueryExecutionResult, QueryExecutor, QueryRequest};
use prost::Message;
use sha2::{Digest, Sha256};
use sqlx::Row;

pub const CAPABILITY_ID: &str = "parties.privacy.scope.contribute";
pub const CAPABILITY_VERSION: &str = "1.0.0";
pub const INPUT_SCHEMA_ID: &str = "crm.parties.privacy.scope.contribution.request";
pub const OUTPUT_SCHEMA_ID: &str = "crm.parties.privacy.scope.contribution.response";
pub const CONTRACT_SCHEMA_VERSION: &str = "1.0.0";
pub const INPUT_MAXIMUM_BYTES: u64 = 16 * 1024;
pub const OUTPUT_MAXIMUM_BYTES: u64 = 64 * 1024;
pub const INPUT_RETENTION_POLICY_ID: &str = "crm.parties.privacy.scope.request";
pub const OUTPUT_RETENTION_POLICY_ID: &str = "crm.parties.privacy.scope.response";
pub const DEFAULT_PAGE_SIZE: u32 = 64;
pub const MAXIMUM_PAGE_SIZE: u32 = 128;
const MAXIMUM_PURPOSE_CODE_BYTES: usize = 96;

#[derive(Clone)]
pub struct PartiesPrivacyScopeQueryAdapter {
    store: PostgresDataStore,
}

impl std::fmt::Debug for PartiesPrivacyScopeQueryAdapter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PartiesPrivacyScopeQueryAdapter")
            .field("store", &"PostgresDataStore")
            .finish()
    }
}

impl PartiesPrivacyScopeQueryAdapter {
    pub fn new(store: PostgresDataStore) -> Self {
        Self { store }
    }

    async fn execute_query(
        &self,
        definition: &CapabilityDefinition,
        request: QueryRequest,
    ) -> Result<QueryExecutionResult, SdkError> {
        validate_definition(definition)?;
        validate_request_contract(&request)?;
        let validated = validate_wire_request(&request.context, &request.input.bytes)?;

        let mut transaction = self
            .store
            .begin_bound_read_transaction(&request.context.tenant_id)
            .await?;
        prove_canonical_party_in_transaction(
            &mut transaction,
            &request.context.tenant_id,
            &validated.canonical_party_id,
            &validated.canonical_party_id,
            validated.identity_resolution_generation,
        )
        .await
        .map_err(map_lineage_error)?;

        let row = sqlx::query(
            r#"
            SELECT
              version,
              owner_module_id,
              schema_id,
              schema_version,
              descriptor_hash,
              data_class,
              payload_encoding,
              maximum_payload_size,
              retention_policy_id,
              payload_bytes
            FROM crm.records
            WHERE tenant_id = $1
              AND owner_module_id = $2
              AND record_type = $3
              AND record_id = $4
              AND deleted_at IS NULL
            FOR SHARE
            "#,
        )
        .bind(request.context.tenant_id.as_str())
        .bind(MODULE_ID)
        .bind(PARTY_RECORD_TYPE)
        .bind(validated.canonical_party_id.as_str())
        .fetch_optional(&mut *transaction)
        .await
        .map_err(database_unavailable)?
        .ok_or_else(subject_not_found)?;

        let snapshot = strict_party_snapshot(&validated.canonical_party_id, row)?;
        let party = party_from_snapshot(snapshot)?;
        let resource_version = u64::try_from(party.version()).map_err(|_| {
            stored_state_invalid("persisted Party version must be positive".to_owned())
        })?;
        if party.party_id().as_str() != validated.canonical_party_id.as_str() {
            return Err(stored_state_invalid(
                "persisted Party identity does not match the locked record".to_owned(),
            ));
        }

        let response = build_response(&validated, resource_version);
        let output = typed_output(response.encode_to_vec())?;
        transaction.commit().await.map_err(database_unavailable)?;
        Ok(QueryExecutionResult { output })
    }
}

impl QueryExecutor for PartiesPrivacyScopeQueryAdapter {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: QueryRequest,
    ) -> PortFuture<'a, Result<QueryExecutionResult, SdkError>> {
        Box::pin(async move { self.execute_query(definition, request).await })
    }
}

#[derive(Debug, Clone)]
struct ValidatedRequest {
    lineage: privacy::PrivacyScopeContributionLineage,
    canonical_party_id: RecordId,
    identity_resolution_generation: u64,
    page_size: u32,
}

pub fn parties_privacy_scope_definition() -> CapabilityDefinition {
    CapabilityDefinition {
        capability_id: capability_id(),
        capability_version: capability_version(),
        owner_module_id: module_id(),
        mutation: false,
        requires_idempotency: false,
        risk: RiskLevel::Medium,
        authorization_policy_id: "privacy.scope.contribute".to_owned(),
        requires_approval: false,
        input_contract: PayloadContract {
            owner: module_id(),
            schema_id: schema_id(INPUT_SCHEMA_ID),
            schema_version: schema_version(CONTRACT_SCHEMA_VERSION),
            descriptor_hash: message_descriptor_hash(
                "crm.customer_privacy.v1.PartiesPrivacyScopeContributionRequest",
            ),
            allowed_data_classes: vec![DataClass::Confidential],
            allowed_encodings: vec![PayloadEncoding::Protobuf],
            maximum_size_bytes: INPUT_MAXIMUM_BYTES,
            retention_policy_id: retention_policy_id(INPUT_RETENTION_POLICY_ID),
        },
        output_contract: PayloadContract {
            owner: module_id(),
            schema_id: schema_id(OUTPUT_SCHEMA_ID),
            schema_version: schema_version(CONTRACT_SCHEMA_VERSION),
            descriptor_hash: message_descriptor_hash(
                "crm.customer_privacy.v1.PartiesPrivacyScopeContributionResponse",
            ),
            allowed_data_classes: vec![DataClass::Confidential],
            allowed_encodings: vec![PayloadEncoding::Protobuf],
            maximum_size_bytes: OUTPUT_MAXIMUM_BYTES,
            retention_policy_id: retention_policy_id(OUTPUT_RETENTION_POLICY_ID),
        },
    }
}

fn validate_definition(definition: &CapabilityDefinition) -> Result<(), SdkError> {
    let expected = parties_privacy_scope_definition();
    if definition.capability_id != expected.capability_id
        || definition.capability_version != expected.capability_version
        || definition.owner_module_id != expected.owner_module_id
        || definition.mutation
        || definition.requires_idempotency
        || definition.requires_approval
        || definition.input_contract != expected.input_contract
        || definition.output_contract != expected.output_contract
    {
        return Err(invalid_contract(
            "PARTIES_PRIVACY_SCOPE_DEFINITION_MISMATCH",
            "The Parties privacy scope definition is invalid.",
        ));
    }
    Ok(())
}

fn validate_request_contract(request: &QueryRequest) -> Result<(), SdkError> {
    request.context.validate()?;
    request.input.validate()?;
    if request.owner_module_id.as_str() != MODULE_ID
        || request.context.capability_id.as_str() != CAPABILITY_ID
        || request.context.capability_version.as_str() != CAPABILITY_VERSION
    {
        return Err(invalid_contract(
            "PARTIES_PRIVACY_SCOPE_REQUEST_BINDING_MISMATCH",
            "The Parties privacy scope request binding is invalid.",
        ));
    }
    let definition = parties_privacy_scope_definition();
    if !definition.input_contract.matches(&request.input) {
        return Err(invalid_contract(
            "PARTIES_PRIVACY_SCOPE_INPUT_CONTRACT_MISMATCH",
            "The Parties privacy scope request contract is invalid.",
        ));
    }
    let actual_hash: [u8; 32] = Sha256::digest(&request.input.bytes).into();
    if request.input_hash != actual_hash {
        return Err(invalid_contract(
            "PARTIES_PRIVACY_SCOPE_INPUT_HASH_MISMATCH",
            "The Parties privacy scope request integrity check failed.",
        ));
    }
    Ok(())
}

fn validate_wire_request(
    context: &QueryExecutionContext,
    bytes: &[u8],
) -> Result<ValidatedRequest, SdkError> {
    let request =
        privacy::PartiesPrivacyScopeContributionRequest::decode(bytes).map_err(|error| {
            invalid_contract_with_reference(
                "PARTIES_PRIVACY_SCOPE_REQUEST_INVALID",
                "The Parties privacy scope request is invalid.",
                error.to_string(),
            )
        })?;
    let contribution = request.contribution.ok_or_else(|| {
        invalid_contract(
            "PARTIES_PRIVACY_SCOPE_REQUEST_INVALID",
            "The Parties privacy scope request is invalid.",
        )
    })?;
    let lineage = contribution.lineage.ok_or_else(|| {
        invalid_contract(
            "PARTIES_PRIVACY_SCOPE_LINEAGE_INVALID",
            "The Parties privacy scope lineage is invalid.",
        )
    })?;
    if lineage.tenant_id != context.tenant_id.as_str() {
        return Err(invalid_contract(
            "PARTIES_PRIVACY_SCOPE_TENANT_MISMATCH",
            "The Parties privacy scope lineage is invalid.",
        ));
    }
    RecordId::try_new(lineage.privacy_case_id.clone()).map_err(|error| {
        invalid_contract_with_reference(
            "PARTIES_PRIVACY_SCOPE_CASE_ID_INVALID",
            "The Parties privacy scope lineage is invalid.",
            error.to_string(),
        )
    })?;
    let canonical_party_id = lineage
        .canonical_party_ref
        .as_ref()
        .ok_or_else(|| {
            invalid_contract(
                "PARTIES_PRIVACY_SCOPE_PARTY_INVALID",
                "The Parties privacy scope lineage is invalid.",
            )
        })
        .and_then(|reference| {
            RecordId::try_new(reference.party_id.clone()).map_err(|error| {
                invalid_contract_with_reference(
                    "PARTIES_PRIVACY_SCOPE_PARTY_INVALID",
                    "The Parties privacy scope lineage is invalid.",
                    error.to_string(),
                )
            })
        })?;
    if lineage.identity_resolution_generation == 0 {
        return Err(invalid_contract(
            "PARTIES_PRIVACY_SCOPE_GENERATION_INVALID",
            "The Parties privacy scope lineage is invalid.",
        ));
    }
    if lineage.registry_version != CANONICAL_SCOPE_REGISTRY_VERSION
        || lineage.registry_digest_sha256.len() != 32
        || lineage.registry_digest_sha256.iter().all(|byte| *byte == 0)
    {
        return Err(invalid_contract(
            "PARTIES_PRIVACY_SCOPE_REGISTRY_INVALID",
            "The Parties privacy scope registry identity is invalid.",
        ));
    }
    let registry = canonical_scope_registry().map_err(|error| {
        SdkError::new(
            "PARTIES_PRIVACY_SCOPE_REGISTRY_UNAVAILABLE",
            ErrorCategory::Internal,
            false,
            "The Parties privacy scope registry is unavailable.",
        )
        .with_internal_reference(error.to_string())
    })?;
    if lineage.registry_digest_sha256.as_slice() != registry.digest() {
        return Err(invalid_contract(
            "PARTIES_PRIVACY_SCOPE_REGISTRY_MISMATCH",
            "The Parties privacy scope registry identity is invalid.",
        ));
    }
    validate_purpose_code(&lineage.purpose_code)?;
    let request_started_at_unix_ms = context.request_started_at_unix_nanos / 1_000_000;
    if lineage.effective_request_at_unix_ms <= 0
        || lineage.effective_request_at_unix_ms > request_started_at_unix_ms
    {
        return Err(invalid_contract(
            "PARTIES_PRIVACY_SCOPE_REQUEST_TIME_INVALID",
            "The Parties privacy scope request time is invalid.",
        ));
    }
    let page_size = if contribution.page_size == 0 {
        DEFAULT_PAGE_SIZE
    } else {
        contribution.page_size
    };
    if page_size > MAXIMUM_PAGE_SIZE {
        return Err(invalid_contract(
            "PARTIES_PRIVACY_SCOPE_PAGE_SIZE_INVALID",
            "The Parties privacy scope page size is invalid.",
        ));
    }
    if !contribution.cursor.is_empty() {
        return Err(invalid_contract(
            "PARTIES_PRIVACY_SCOPE_CURSOR_INVALID",
            "The Parties privacy scope cursor is invalid.",
        ));
    }
    let generation = lineage.identity_resolution_generation;
    Ok(ValidatedRequest {
        lineage,
        canonical_party_id,
        identity_resolution_generation: generation,
        page_size,
    })
}

fn strict_party_snapshot(
    canonical_party_id: &RecordId,
    row: sqlx::postgres::PgRow,
) -> Result<RecordSnapshot, SdkError> {
    let version: i64 = row.try_get("version").map_err(row_decode_error)?;
    let owner_module_id: String = row.try_get("owner_module_id").map_err(row_decode_error)?;
    let stored_schema_id: String = row.try_get("schema_id").map_err(row_decode_error)?;
    let stored_schema_version: String = row.try_get("schema_version").map_err(row_decode_error)?;
    let stored_descriptor_hash: Vec<u8> =
        row.try_get("descriptor_hash").map_err(row_decode_error)?;
    let stored_data_class: String = row.try_get("data_class").map_err(row_decode_error)?;
    let stored_encoding: String = row.try_get("payload_encoding").map_err(row_decode_error)?;
    let stored_maximum_size: i64 = row
        .try_get("maximum_payload_size")
        .map_err(row_decode_error)?;
    let stored_retention: String = row
        .try_get("retention_policy_id")
        .map_err(row_decode_error)?;
    let payload_bytes: Vec<u8> = row.try_get("payload_bytes").map_err(row_decode_error)?;

    let expected_descriptor_hash = party_state_descriptor_hash();
    if owner_module_id != MODULE_ID
        || stored_schema_id != PARTY_STATE_SCHEMA_ID
        || stored_schema_version != PARTY_STATE_SCHEMA_VERSION
        || stored_descriptor_hash.as_slice() != expected_descriptor_hash
        || stored_data_class != "personal"
        || stored_encoding != "json"
        || stored_maximum_size != PARTY_STATE_MAXIMUM_BYTES as i64
        || stored_retention != PARTY_STATE_RETENTION_POLICY_ID
    {
        return Err(stored_state_invalid(
            "persisted Party metadata does not match the canonical state contract".to_owned(),
        ));
    }

    Ok(RecordSnapshot {
        reference: RecordRef {
            record_type: record_type(PARTY_RECORD_TYPE),
            record_id: canonical_party_id.clone(),
        },
        version,
        payload: TypedPayload {
            owner: module_id(),
            schema_id: schema_id(PARTY_STATE_SCHEMA_ID),
            schema_version: schema_version(PARTY_STATE_SCHEMA_VERSION),
            descriptor_hash: expected_descriptor_hash,
            data_class: DataClass::Personal,
            encoding: PayloadEncoding::Json,
            maximum_size_bytes: PARTY_STATE_MAXIMUM_BYTES,
            retention_policy_id: retention_policy_id(PARTY_STATE_RETENTION_POLICY_ID),
            bytes: payload_bytes,
        },
    })
}

fn build_response(
    request: &ValidatedRequest,
    resource_version: u64,
) -> privacy::PartiesPrivacyScopeContributionResponse {
    let cursor_digest = framed_digest(
        b"crm.parties.privacy.scope.cursor/v1",
        &[
            request.lineage.tenant_id.as_bytes(),
            request.canonical_party_id.as_str().as_bytes(),
            request
                .identity_resolution_generation
                .to_string()
                .as_bytes(),
            request.lineage.registry_digest_sha256.as_slice(),
            request.page_size.to_string().as_bytes(),
            b"terminal",
        ],
    );
    let page_digest = framed_digest(
        b"crm.parties.privacy.scope.page/v1",
        &[
            request.lineage.privacy_case_id.as_bytes(),
            request.canonical_party_id.as_str().as_bytes(),
            resource_version.to_string().as_bytes(),
            b"personal",
            b"retain_minimized_evidence",
            PARTY_STATE_RETENTION_POLICY_ID.as_bytes(),
            cursor_digest.as_slice(),
        ],
    );
    privacy::PartiesPrivacyScopeContributionResponse {
        contribution: Some(privacy::PrivacyScopeContributionResponseEnvelope {
            owner_module_id: MODULE_ID.to_owned(),
            capability_id: CAPABILITY_ID.to_owned(),
            capability_version: CAPABILITY_VERSION.to_owned(),
            lineage: Some(request.lineage.clone()),
            resources: vec![privacy::PrivacyScopeResourceReference {
                resource_type: PARTY_RECORD_TYPE.to_owned(),
                resource_id: request.canonical_party_id.as_str().to_owned(),
                resource_version,
                data_class: privacy::CustomerDataClass::Personal as i32,
                evidence_class: privacy::PrivacyScopeEvidenceClass::RetainMinimizedEvidence as i32,
                retention_policy_id: PARTY_STATE_RETENTION_POLICY_ID.to_owned(),
            }],
            page_evidence: Some(privacy::PrivacyScopeContributionPageEvidence {
                page_number: 1,
                scanned_resource_count: 1,
                emitted_resource_count: 1,
                next_cursor: String::new(),
                terminal_complete: true,
                cursor_digest_sha256: cursor_digest.to_vec(),
                page_digest_sha256: page_digest.to_vec(),
            }),
        }),
    }
}

fn typed_output(bytes: Vec<u8>) -> Result<TypedPayload, SdkError> {
    let output = TypedPayload {
        owner: module_id(),
        schema_id: schema_id(OUTPUT_SCHEMA_ID),
        schema_version: schema_version(CONTRACT_SCHEMA_VERSION),
        descriptor_hash: message_descriptor_hash(
            "crm.customer_privacy.v1.PartiesPrivacyScopeContributionResponse",
        ),
        data_class: DataClass::Confidential,
        encoding: PayloadEncoding::Protobuf,
        maximum_size_bytes: OUTPUT_MAXIMUM_BYTES,
        retention_policy_id: retention_policy_id(OUTPUT_RETENTION_POLICY_ID),
        bytes,
    };
    output.validate()?;
    Ok(output)
}

fn validate_purpose_code(value: &str) -> Result<(), SdkError> {
    if value.is_empty()
        || value.len() > MAXIMUM_PURPOSE_CODE_BYTES
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(invalid_contract(
            "PARTIES_PRIVACY_SCOPE_PURPOSE_INVALID",
            "The Parties privacy scope purpose is invalid.",
        ));
    }
    Ok(())
}

fn framed_digest(domain: &[u8], fields: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    append_frame(&mut hasher, domain);
    for field in fields {
        append_frame(&mut hasher, field);
    }
    hasher.finalize().into()
}

fn append_frame(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}

fn map_lineage_error(error: SdkError) -> SdkError {
    let category = if error.category == ErrorCategory::NotFound {
        ErrorCategory::NotFound
    } else {
        ErrorCategory::Conflict
    };
    SdkError::new(
        "PARTIES_PRIVACY_SCOPE_LINEAGE_INVALID",
        category,
        false,
        "The requested Parties privacy scope is not available.",
    )
    .with_internal_reference(error.code)
}

fn subject_not_found() -> SdkError {
    SdkError::new(
        "PARTIES_PRIVACY_SCOPE_SUBJECT_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The requested Parties privacy scope was not found.",
    )
}

fn stored_state_invalid(reference: String) -> SdkError {
    SdkError::new(
        "PARTIES_PRIVACY_SCOPE_STORED_STATE_INVALID",
        ErrorCategory::Unavailable,
        true,
        "The Parties privacy scope is temporarily unavailable.",
    )
    .with_internal_reference(reference)
}

fn row_decode_error(error: sqlx::Error) -> SdkError {
    stored_state_invalid(error.to_string())
}

fn database_unavailable(error: sqlx::Error) -> SdkError {
    SdkError::new(
        "PARTIES_PRIVACY_SCOPE_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "The Parties privacy scope is temporarily unavailable.",
    )
    .with_internal_reference(error.to_string())
}

fn invalid_contract(code: &'static str, safe_message: &'static str) -> SdkError {
    SdkError::new(code, ErrorCategory::InvalidArgument, false, safe_message)
}

fn invalid_contract_with_reference(
    code: &'static str,
    safe_message: &'static str,
    reference: String,
) -> SdkError {
    invalid_contract(code, safe_message).with_internal_reference(reference)
}

fn module_id() -> ModuleId {
    ModuleId::try_new(MODULE_ID).expect("static Parties module id must be valid")
}

fn capability_id() -> CapabilityId {
    CapabilityId::try_new(CAPABILITY_ID).expect("static capability id must be valid")
}

fn capability_version() -> CapabilityVersion {
    CapabilityVersion::try_new(CAPABILITY_VERSION).expect("static capability version must be valid")
}

fn schema_id(value: &'static str) -> SchemaId {
    SchemaId::try_new(value).expect("static schema id must be valid")
}

fn schema_version(value: &'static str) -> SchemaVersion {
    SchemaVersion::try_new(value).expect("static schema version must be valid")
}

fn retention_policy_id(value: &'static str) -> RetentionPolicyId {
    RetentionPolicyId::try_new(value).expect("static retention policy id must be valid")
}

fn record_type(value: &'static str) -> RecordType {
    RecordType::try_new(value).expect("static record type must be valid")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_module_sdk::{ActorId, CorrelationId, RequestId, TraceId};

    fn context() -> QueryExecutionContext {
        QueryExecutionContext {
            tenant_id: TenantId::try_new("tenant-a").unwrap(),
            actor_id: ActorId::try_new("privacy-worker").unwrap(),
            request_id: RequestId::try_new("request-1").unwrap(),
            correlation_id: CorrelationId::try_new("correlation-1").unwrap(),
            trace_id: TraceId::try_new("trace-1").unwrap(),
            capability_id: capability_id(),
            capability_version: capability_version(),
            schema_version: schema_version(CONTRACT_SCHEMA_VERSION),
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
                    canonical_party_ref: Some(crm_proto_contracts::crm::customer::v1::PartyRef {
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

    #[test]
    fn definition_is_internal_read_only_and_exactly_bound() {
        let definition = parties_privacy_scope_definition();
        assert_eq!(definition.capability_id.as_str(), CAPABILITY_ID);
        assert_eq!(definition.capability_version.as_str(), CAPABILITY_VERSION);
        assert_eq!(definition.owner_module_id.as_str(), MODULE_ID);
        assert!(!definition.mutation);
        assert!(!definition.requires_idempotency);
        assert!(!definition.requires_approval);
        assert_eq!(
            definition.authorization_policy_id,
            "privacy.scope.contribute"
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
    fn non_empty_cursor_and_future_request_time_fail_closed() {
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
    }
}
