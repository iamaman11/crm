#![forbid(unsafe_code)]

use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRisk};
use crm_core_data::{
    MAXIMUM_RELATED_RECORD_QUERY_PAGE_SIZE, PostgresDataStore, RecordGetQuery,
    RelatedRecordListQuery,
};
use crm_identity_resolution::{
    DuplicateCandidateCase, DuplicateCandidateCaseStatus, PartyReference,
};
use crm_identity_resolution_capability_adapter::{
    MODULE_ID, PARTY_CANDIDATE_RELATIONSHIP_TYPE, PARTY_CANDIDATE_SOURCE_RECORD_TYPE, RECORD_TYPE,
    duplicate_candidate_case_from_snapshot, duplicate_candidate_case_to_wire,
};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, PayloadEncoding,
    PortFuture, RecordId, RecordRef, RecordType, RelationshipType, SdkError, TypedPayload,
};
use crm_proto_contracts::crm::{customer::v1 as customer, identity_resolution::v1 as wire};
use crm_query_runtime::{
    CursorBinding, CursorCodec, CursorContinuation, PageSizePolicy, QueryExecutionResult,
    QueryExecutor, QueryRequest, QuerySemanticValidator, QueryVisibilityAuthorizer,
    QueryVisibilityDecision, normalized_filter_hash,
};
use prost::Message;
use std::sync::Arc;

pub const GET_CAPABILITY: &str = "identity_resolution.candidate.get";
pub const LIST_BY_PARTY_CAPABILITY: &str = "identity_resolution.candidate.list_by_party";
pub const GET_REQUEST_SCHEMA: &str = "crm.identity_resolution.v1.GetDuplicateCandidateCaseRequest";
pub const GET_RESPONSE_SCHEMA: &str =
    "crm.identity_resolution.v1.GetDuplicateCandidateCaseResponse";
pub const LIST_BY_PARTY_REQUEST_SCHEMA: &str =
    "crm.identity_resolution.v1.ListDuplicateCandidateCasesByPartyRequest";
pub const LIST_BY_PARTY_RESPONSE_SCHEMA: &str =
    "crm.identity_resolution.v1.ListDuplicateCandidateCasesByPartyResponse";
pub const QUERY_CAPABILITY_IDS: [&str; 2] = [GET_CAPABILITY, LIST_BY_PARTY_CAPABILITY];

const DEFAULT_PAGE_SIZE: u32 = 50;
const MAXIMUM_PAGE_SIZE: u32 = 200;
const MAXIMUM_VISIBILITY_SCAN_RECORDS: usize = 10_000;
const RELATED_RECORD_SORT_ID: &str = "record_id_ascending";

#[derive(Clone)]
pub struct IdentityResolutionQueryAdapter {
    store: PostgresDataStore,
    cursor_codec: CursorCodec,
    visibility: Arc<dyn QueryVisibilityAuthorizer>,
    page_policy: PageSizePolicy,
}

impl std::fmt::Debug for IdentityResolutionQueryAdapter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("IdentityResolutionQueryAdapter")
            .field("store", &self.store)
            .field("cursor_codec", &self.cursor_codec)
            .field("visibility", &"dyn QueryVisibilityAuthorizer")
            .field("page_policy", &self.page_policy)
            .finish()
    }
}

impl IdentityResolutionQueryAdapter {
    pub fn new(
        store: PostgresDataStore,
        cursor_codec: CursorCodec,
        visibility: Arc<dyn QueryVisibilityAuthorizer>,
    ) -> Result<Self, SdkError> {
        let page_policy = PageSizePolicy {
            default_size: DEFAULT_PAGE_SIZE,
            maximum_size: MAXIMUM_PAGE_SIZE,
        }
        .validate()
        .map_err(cursor_error)?;
        Ok(Self {
            store,
            cursor_codec,
            visibility,
            page_policy,
        })
    }
}

pub fn query_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    QUERY_CAPABILITY_IDS
        .iter()
        .map(|capability_id| query_capability_definition(capability_id))
        .collect()
}

pub fn query_capability_definition(capability_id: &str) -> Result<CapabilityDefinition, SdkError> {
    let (input_schema, output_schema) = match capability_id {
        GET_CAPABILITY => (GET_REQUEST_SCHEMA, GET_RESPONSE_SCHEMA),
        LIST_BY_PARTY_CAPABILITY => (LIST_BY_PARTY_REQUEST_SCHEMA, LIST_BY_PARTY_RESPONSE_SCHEMA),
        _ => return Err(unsupported_query()),
    };

    Ok(CapabilityDefinition {
        capability_id: support::configured_identifier(CapabilityId::try_new(capability_id))?,
        capability_version: support::configured_identifier(CapabilityVersion::try_new(
            support::CONTRACT_VERSION,
        ))?,
        owner_module_id: support::configured_identifier(ModuleId::try_new(MODULE_ID))?,
        input_contract: support::protobuf_contract(
            MODULE_ID,
            input_schema,
            vec![DataClass::Personal],
        )?,
        output_contract: Some(support::protobuf_contract(
            MODULE_ID,
            output_schema,
            vec![DataClass::Personal],
        )?),
        risk: CapabilityRisk::Low,
        mutation: false,
        requires_idempotency: false,
        requires_approval: false,
        authorization_policy_id: capability_id.to_owned(),
        rate_limit_policy_id: None,
    })
}

impl QuerySemanticValidator for IdentityResolutionQueryAdapter {
    fn validate<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a QueryRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            match definition.capability_id.as_str() {
                GET_CAPABILITY => {
                    let command: wire::GetDuplicateCandidateCaseRequest =
                        decode_input(request, GET_REQUEST_SCHEMA)?;
                    let case_ref = command.case_ref.ok_or_else(|| {
                        SdkError::invalid_argument(
                            "identity_resolution.candidate.case_ref",
                            "candidate case ref is required",
                        )
                    })?;
                    validate_record_id(
                        &case_ref.case_id,
                        "identity_resolution.candidate.case_ref.case_id",
                    )?;
                }
                LIST_BY_PARTY_CAPABILITY => {
                    let command: wire::ListDuplicateCandidateCasesByPartyRequest =
                        decode_input(request, LIST_BY_PARTY_REQUEST_SCHEMA)?;
                    validate_list(self, request, &command)?;
                }
                _ => return Err(unsupported_query()),
            }
            Ok(())
        })
    }
}

impl QueryExecutor for IdentityResolutionQueryAdapter {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: QueryRequest,
    ) -> PortFuture<'a, Result<QueryExecutionResult, SdkError>> {
        Box::pin(async move {
            let output = match definition.capability_id.as_str() {
                GET_CAPABILITY => self.execute_get(&request).await?,
                LIST_BY_PARTY_CAPABILITY => self.execute_list_by_party(&request).await?,
                _ => return Err(unsupported_query()),
            };
            Ok(QueryExecutionResult { output })
        })
    }
}

impl IdentityResolutionQueryAdapter {
    async fn execute_get(&self, request: &QueryRequest) -> Result<TypedPayload, SdkError> {
        let command: wire::GetDuplicateCandidateCaseRequest =
            decode_input(request, GET_REQUEST_SCHEMA)?;
        let case_ref = command.case_ref.ok_or_else(|| {
            SdkError::invalid_argument(
                "identity_resolution.candidate.case_ref",
                "candidate case ref is required",
            )
        })?;
        let record_id = validate_record_id(
            &case_ref.case_id,
            "identity_resolution.candidate.case_ref.case_id",
        )?;
        let snapshot = self
            .store
            .get_record_for_query(&RecordGetQuery {
                tenant_id: request.context.tenant_id.clone(),
                owner_module_id: configured_module_id(MODULE_ID)?,
                record_type: configured_record_type(RECORD_TYPE)?,
                record_id,
            })
            .await?
            .ok_or_else(resource_not_found)?;
        let visibility = self
            .visibility
            .authorize_visibility(request, &snapshot.reference)
            .await?;
        if !visibility.resource_visible {
            return Err(resource_not_found());
        }
        let candidate = duplicate_candidate_case_from_snapshot(&snapshot)?;
        support::protobuf_payload(
            MODULE_ID,
            GET_RESPONSE_SCHEMA,
            DataClass::Personal,
            &wire::GetDuplicateCandidateCaseResponse {
                candidate_case: Some(candidate_to_wire_with_visibility(&candidate, &visibility)),
            },
        )
    }

    async fn execute_list_by_party(
        &self,
        request: &QueryRequest,
    ) -> Result<TypedPayload, SdkError> {
        let command: wire::ListDuplicateCandidateCasesByPartyRequest =
            decode_input(request, LIST_BY_PARTY_REQUEST_SCHEMA)?;
        let party_ref = required_party_ref(command.party_ref.as_ref())?;
        let status = optional_status(command.status)?;
        let page_size = self
            .page_policy
            .resolve(command.page_size)
            .map_err(cursor_error)?;
        let binding = cursor_binding(request, &party_ref, status, page_size)?;
        let after_record_id = decode_after(self, &command.cursor, &binding)?;
        let (candidate_cases, next_record_id) = self
            .collect_candidates(request, &party_ref, status, page_size, after_record_id)
            .await?;
        let next_cursor = encode_next(self, &binding, next_record_id.as_ref())?;

        support::protobuf_payload(
            MODULE_ID,
            LIST_BY_PARTY_RESPONSE_SCHEMA,
            DataClass::Personal,
            &wire::ListDuplicateCandidateCasesByPartyResponse {
                candidate_cases,
                next_cursor,
            },
        )
    }

    async fn collect_candidates(
        &self,
        request: &QueryRequest,
        party_ref: &PartyReference,
        status: Option<DuplicateCandidateCaseStatus>,
        page_size: u32,
        mut after_record_id: Option<RecordId>,
    ) -> Result<(Vec<wire::DuplicateCandidateCase>, Option<RecordId>), SdkError> {
        let mut output = Vec::with_capacity(page_size as usize);
        let mut scanned = 0_usize;
        loop {
            let remaining = page_size as usize - output.len();
            if remaining == 0 {
                return Ok((output, None));
            }
            let related_page_size = u32::try_from(remaining)
                .map_err(|_| visibility_scan_limit_error())?
                .min(MAXIMUM_RELATED_RECORD_QUERY_PAGE_SIZE);
            let page = self
                .store
                .list_related_records_for_query(&related_query(
                    request,
                    party_ref,
                    related_page_size,
                    after_record_id.clone(),
                )?)
                .await?;
            scanned = scanned.saturating_add(page.records.len());
            enforce_visibility_scan_limit(scanned)?;

            for snapshot in &page.records {
                let candidate = duplicate_candidate_case_from_snapshot(snapshot)?;
                if status.is_some_and(|expected| candidate.status() != expected) {
                    continue;
                }
                let visibility = self
                    .visibility
                    .authorize_visibility(request, &snapshot.reference)
                    .await?;
                if visibility.resource_visible {
                    output.push(candidate_to_wire_with_visibility(&candidate, &visibility));
                }
            }

            let next_anchor = page.next_record_id;
            if output.len() == page_size as usize {
                let has_more = match next_anchor.as_ref() {
                    Some(anchor) => {
                        self.has_more_visible_candidate(
                            request,
                            party_ref,
                            status,
                            anchor.clone(),
                            &mut scanned,
                        )
                        .await?
                    }
                    None => false,
                };
                return Ok((output, has_more.then_some(next_anchor).flatten()));
            }
            after_record_id = next_anchor;
            if after_record_id.is_none() {
                return Ok((output, None));
            }
        }
    }

    async fn has_more_visible_candidate(
        &self,
        request: &QueryRequest,
        party_ref: &PartyReference,
        status: Option<DuplicateCandidateCaseStatus>,
        mut after_record_id: RecordId,
        scanned: &mut usize,
    ) -> Result<bool, SdkError> {
        loop {
            let page = self
                .store
                .list_related_records_for_query(&related_query(
                    request,
                    party_ref,
                    MAXIMUM_PAGE_SIZE,
                    Some(after_record_id),
                )?)
                .await?;
            *scanned = scanned.saturating_add(page.records.len());
            enforce_visibility_scan_limit(*scanned)?;
            for snapshot in &page.records {
                let candidate = duplicate_candidate_case_from_snapshot(snapshot)?;
                if status.is_some_and(|expected| candidate.status() != expected) {
                    continue;
                }
                if self
                    .visibility
                    .authorize_visibility(request, &snapshot.reference)
                    .await?
                    .resource_visible
                {
                    return Ok(true);
                }
            }
            let Some(next) = page.next_record_id else {
                return Ok(false);
            };
            after_record_id = next;
        }
    }
}

fn related_query(
    request: &QueryRequest,
    party_ref: &PartyReference,
    page_size: u32,
    after_record_id: Option<RecordId>,
) -> Result<RelatedRecordListQuery, SdkError> {
    Ok(RelatedRecordListQuery {
        tenant_id: request.context.tenant_id.clone(),
        relationship_owner_module_id: configured_module_id(MODULE_ID)?,
        relationship_type: configured_relationship_type(PARTY_CANDIDATE_RELATIONSHIP_TYPE)?,
        source: RecordRef {
            record_type: configured_record_type(PARTY_CANDIDATE_SOURCE_RECORD_TYPE)?,
            record_id: RecordId::try_new(party_ref.as_str()).map_err(config_error)?,
        },
        target_owner_module_id: configured_module_id(MODULE_ID)?,
        target_record_type: configured_record_type(RECORD_TYPE)?,
        page_size,
        after_record_id,
    })
}

fn validate_list(
    adapter: &IdentityResolutionQueryAdapter,
    request: &QueryRequest,
    command: &wire::ListDuplicateCandidateCasesByPartyRequest,
) -> Result<(), SdkError> {
    let party_ref = required_party_ref(command.party_ref.as_ref())?;
    let status = optional_status(command.status)?;
    let page_size = adapter
        .page_policy
        .resolve(command.page_size)
        .map_err(cursor_error)?;
    let binding = cursor_binding(request, &party_ref, status, page_size)?;
    let _ = decode_after(adapter, &command.cursor, &binding)?;
    Ok(())
}

fn candidate_to_wire_with_visibility(
    candidate: &DuplicateCandidateCase,
    visibility: &QueryVisibilityDecision,
) -> wire::DuplicateCandidateCase {
    let mut output = duplicate_candidate_case_to_wire(candidate);
    if !visibility.allows_field("party_pair") {
        output.left_party_ref = None;
        output.right_party_ref = None;
        for evidence in &mut output.evidence_history {
            evidence.first_party_ref = None;
            evidence.second_party_ref = None;
        }
    }
    if !visibility.allows_field("evidence_history") {
        output.evidence_history.clear();
    }
    if !visibility.allows_field("status") {
        output.status = wire::DuplicateCandidateCaseStatus::Unspecified as i32;
    }
    if !visibility.allows_field("decision_reason") {
        output.decision_reason.clear();
    }
    output
}

fn required_party_ref(value: Option<&customer::PartyRef>) -> Result<PartyReference, SdkError> {
    let value = value.ok_or_else(|| {
        SdkError::invalid_argument(
            "identity_resolution.candidate.party_ref",
            "Party reference is required",
        )
    })?;
    PartyReference::try_new(value.party_id.clone())
}

fn optional_status(value: i32) -> Result<Option<DuplicateCandidateCaseStatus>, SdkError> {
    match wire::DuplicateCandidateCaseStatus::try_from(value) {
        Ok(wire::DuplicateCandidateCaseStatus::Unspecified) => Ok(None),
        Ok(wire::DuplicateCandidateCaseStatus::Open) => {
            Ok(Some(DuplicateCandidateCaseStatus::Open))
        }
        Ok(wire::DuplicateCandidateCaseStatus::Dismissed) => {
            Ok(Some(DuplicateCandidateCaseStatus::Dismissed))
        }
        Ok(wire::DuplicateCandidateCaseStatus::ConfirmedDuplicate) => {
            Ok(Some(DuplicateCandidateCaseStatus::ConfirmedDuplicate))
        }
        Err(_) => Err(SdkError::invalid_argument(
            "identity_resolution.candidate.status",
            "candidate status filter is invalid",
        )),
    }
}

fn status_wire_value(value: Option<DuplicateCandidateCaseStatus>) -> i32 {
    match value {
        None => wire::DuplicateCandidateCaseStatus::Unspecified as i32,
        Some(DuplicateCandidateCaseStatus::Open) => wire::DuplicateCandidateCaseStatus::Open as i32,
        Some(DuplicateCandidateCaseStatus::Dismissed) => {
            wire::DuplicateCandidateCaseStatus::Dismissed as i32
        }
        Some(DuplicateCandidateCaseStatus::ConfirmedDuplicate) => {
            wire::DuplicateCandidateCaseStatus::ConfirmedDuplicate as i32
        }
    }
}

fn cursor_binding(
    request: &QueryRequest,
    party_ref: &PartyReference,
    status: Option<DuplicateCandidateCaseStatus>,
    page_size: u32,
) -> Result<CursorBinding, SdkError> {
    let status = status_wire_value(status).to_be_bytes();
    Ok(CursorBinding {
        tenant_id: request.context.tenant_id.clone(),
        actor_id: Some(request.context.actor_id.clone()),
        capability_id: request.context.capability_id.clone(),
        capability_version: request.context.capability_version.clone(),
        resource_type: configured_record_type(RECORD_TYPE)?,
        normalized_filter_hash: normalized_filter_hash([
            ("party_id", party_ref.as_str().as_bytes()),
            ("status", status.as_slice()),
        ]),
        sort_id: RELATED_RECORD_SORT_ID.to_owned(),
        page_size,
    })
}

fn decode_after(
    adapter: &IdentityResolutionQueryAdapter,
    token: &str,
    binding: &CursorBinding,
) -> Result<Option<RecordId>, SdkError> {
    if token.is_empty() {
        return Ok(None);
    }
    let continuation = adapter
        .cursor_codec
        .decode(token, binding)
        .map_err(cursor_error)?;
    continuation.validate().map_err(cursor_error)?;
    if continuation.sort_key != continuation.record_id.as_str().as_bytes() {
        return Err(cursor_invalid());
    }
    Ok(Some(continuation.record_id))
}

fn encode_next(
    adapter: &IdentityResolutionQueryAdapter,
    binding: &CursorBinding,
    next: Option<&RecordId>,
) -> Result<String, SdkError> {
    next.map(|record_id| {
        adapter
            .cursor_codec
            .encode(
                binding,
                &CursorContinuation {
                    sort_key: record_id.as_str().as_bytes().to_vec(),
                    record_id: record_id.clone(),
                },
            )
            .map_err(cursor_error)
    })
    .transpose()
    .map(|value| value.unwrap_or_default())
}

fn decode_input<M: Message + Default>(
    request: &QueryRequest,
    schema_id: &str,
) -> Result<M, SdkError> {
    let payload = &request.input;
    if payload.owner.as_str() != MODULE_ID
        || payload.schema_id.as_str() != schema_id
        || payload.schema_version.as_str() != support::CONTRACT_VERSION
        || payload.descriptor_hash != support::message_descriptor_hash(schema_id)
        || payload.data_class != DataClass::Personal
        || payload.encoding != PayloadEncoding::Protobuf
        || payload.maximum_size_bytes > support::MAX_PROTOBUF_BYTES
        || payload.validate().is_err()
    {
        return Err(SdkError::new(
            "IDENTITY_RESOLUTION_QUERY_INPUT_CONTRACT_MISMATCH",
            ErrorCategory::InvalidArgument,
            false,
            "The Identity Resolution query input does not match the required contract.",
        ));
    }
    M::decode(payload.bytes.as_slice()).map_err(|_| {
        SdkError::new(
            "IDENTITY_RESOLUTION_QUERY_INPUT_PROTOBUF_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "The Identity Resolution query input is not valid Protobuf.",
        )
    })
}

fn validate_record_id(value: &str, field: &'static str) -> Result<RecordId, SdkError> {
    RecordId::try_new(value.to_owned())
        .map_err(|error| SdkError::invalid_argument(field, error.to_string()))
}

fn configured_module_id(value: &str) -> Result<ModuleId, SdkError> {
    ModuleId::try_new(value).map_err(config_error)
}

fn configured_record_type(value: &str) -> Result<RecordType, SdkError> {
    RecordType::try_new(value).map_err(config_error)
}

fn configured_relationship_type(value: &str) -> Result<RelationshipType, SdkError> {
    RelationshipType::try_new(value).map_err(config_error)
}

fn enforce_visibility_scan_limit(scanned: usize) -> Result<(), SdkError> {
    if scanned > MAXIMUM_VISIBILITY_SCAN_RECORDS {
        Err(visibility_scan_limit_error())
    } else {
        Ok(())
    }
}

fn resource_not_found() -> SdkError {
    SdkError::new(
        "QUERY_RESOURCE_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The requested resource was not found.",
    )
}

fn visibility_scan_limit_error() -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_QUERY_SCAN_LIMIT_EXCEEDED",
        ErrorCategory::Unavailable,
        true,
        "The Identity Resolution query could not complete within the bounded visibility scan.",
    )
}

fn cursor_invalid() -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_QUERY_CURSOR_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "The Identity Resolution page cursor is invalid.",
    )
}

fn cursor_error(error: crm_query_runtime::CursorError) -> SdkError {
    SdkError::new(
        error.code(),
        match error {
            crm_query_runtime::CursorError::SigningKeyTooShort
            | crm_query_runtime::CursorError::SigningUnavailable
            | crm_query_runtime::CursorError::InvalidBinding
            | crm_query_runtime::CursorError::InvalidPagePolicy => ErrorCategory::Internal,
            _ => ErrorCategory::InvalidArgument,
        },
        false,
        error.safe_message(),
    )
}

fn config_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_QUERY_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Identity Resolution query adapter is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}

fn unsupported_query() -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_QUERY_CAPABILITY_UNSUPPORTED",
        ErrorCategory::InvalidArgument,
        false,
        "The Identity Resolution query capability is unsupported.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publishes_exact_get_and_list_by_party_coordinates() {
        let definitions = query_capability_definitions().unwrap();
        assert_eq!(definitions.len(), 2);
        assert_eq!(definitions[0].capability_id.as_str(), GET_CAPABILITY);
        assert_eq!(
            definitions[1].capability_id.as_str(),
            LIST_BY_PARTY_CAPABILITY
        );
        for definition in definitions {
            assert_eq!(definition.owner_module_id.as_str(), MODULE_ID);
            assert!(!definition.mutation);
            assert!(!definition.requires_idempotency);
            assert_eq!(
                definition.input_contract.allowed_data_classes,
                vec![DataClass::Personal]
            );
        }
    }

    #[test]
    fn authoritative_party_access_path_coordinates_are_stable() {
        assert_eq!(
            configured_relationship_type(PARTY_CANDIDATE_RELATIONSHIP_TYPE)
                .unwrap()
                .as_str(),
            "identity_resolution.candidate.party"
        );
        assert_eq!(
            configured_record_type(PARTY_CANDIDATE_SOURCE_RECORD_TYPE)
                .unwrap()
                .as_str(),
            "parties.party"
        );
    }
}
