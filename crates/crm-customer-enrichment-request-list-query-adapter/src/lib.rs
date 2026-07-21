#![forbid(unsafe_code)]

//! Signed, permission-aware list query for Customer Enrichment requests.
//!
//! Cursor state is integrity-protected and bound to the tenant, actor, capability, filters,
//! stable storage sort and page size. Every candidate is strictly rehydrated and checked against
//! live Party and request visibility before disclosure.

use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRisk};
use crm_core_data::{PostgresDataStore, RecordListQuery, RecordQueryContinuation, RecordQuerySort};
use crm_customer_enrichment::{ENRICHMENT_REQUEST_RECORD_TYPE, EnrichmentRequestStatus};
use crm_customer_enrichment_capability_adapter::{
    MODULE_ID, enrichment_request_from_snapshot, enrichment_request_to_wire,
};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, PayloadEncoding,
    PortFuture, RecordId, RecordRef, RecordType, SdkError, TypedPayload,
};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;
use crm_query_runtime::{
    CursorBinding, CursorCodec, CursorContinuation, QueryExecutionResult, QueryExecutor,
    QueryRequest, QuerySemanticValidator, QueryVisibilityAuthorizer, normalized_filter_hash,
};
use prost::Message;
use std::sync::Arc;

pub const LIST_ENRICHMENT_REQUESTS_CAPABILITY: &str = "customer_enrichment.request.list";
pub const LIST_ENRICHMENT_REQUESTS_REQUEST_SCHEMA: &str =
    "crm.customer_enrichment.v1.ListEnrichmentRequestsRequest";
pub const LIST_ENRICHMENT_REQUESTS_RESPONSE_SCHEMA: &str =
    "crm.customer_enrichment.v1.ListEnrichmentRequestsResponse";

const PARTY_RECORD_TYPE: &str = "parties.party";
const DEFAULT_PAGE_SIZE: u32 = 50;
const MAXIMUM_PAGE_SIZE: u32 = 100;
const MAXIMUM_VISIBILITY_SCAN_RECORDS: usize = 4_096;

#[derive(Clone)]
pub struct CustomerEnrichmentRequestListQueryAdapter {
    store: PostgresDataStore,
    cursor_codec: CursorCodec,
    visibility: Arc<dyn QueryVisibilityAuthorizer>,
}

impl CustomerEnrichmentRequestListQueryAdapter {
    pub fn new(
        store: PostgresDataStore,
        cursor_codec: CursorCodec,
        visibility: Arc<dyn QueryVisibilityAuthorizer>,
    ) -> Self {
        Self {
            store,
            cursor_codec,
            visibility,
        }
    }

    async fn execute_list(&self, request: &QueryRequest) -> Result<TypedPayload, SdkError> {
        let command: wire::ListEnrichmentRequestsRequest = decode_input(request)?;
        let party_id = party_record_id(command.party_ref)?;
        let provider_profile_version_id =
            provider_profile_record_id(command.provider_profile_version_ref)?;
        let requested_status = command.status.map(validate_status).transpose()?;
        let page_size = resolve_page_size(command.page_size)?;
        let binding = cursor_binding(
            request,
            &party_id,
            &provider_profile_version_id,
            requested_status,
            page_size,
        )?;
        let after = decode_after(self, &command.cursor, &binding)?;

        let party_reference = RecordRef {
            record_type: party_record_type()?,
            record_id: party_id.clone(),
        };
        if !self
            .visibility
            .authorize_visibility(request, &party_reference)
            .await?
            .resource_visible
        {
            return list_response(Vec::new(), String::new());
        }

        let (requests, next) = self
            .collect_visible_requests(
                request,
                &party_id,
                &provider_profile_version_id,
                requested_status,
                page_size,
                after,
            )
            .await?;
        let next_cursor = encode_next(self, &binding, next.as_ref())?;
        list_response(requests, next_cursor)
    }

    async fn collect_visible_requests(
        &self,
        request: &QueryRequest,
        party_id: &RecordId,
        provider_profile_version_id: &RecordId,
        requested_status: Option<EnrichmentRequestStatus>,
        page_size: u32,
        mut after: Option<RecordQueryContinuation>,
    ) -> Result<
        (
            Vec<wire::EnrichmentRequest>,
            Option<RecordQueryContinuation>,
        ),
        SdkError,
    > {
        let mut output = Vec::with_capacity(page_size as usize);
        let mut scanned = 0_usize;

        loop {
            let remaining = page_size as usize - output.len();
            if remaining == 0 {
                let anchor = after.clone();
                let has_more = self
                    .has_more_visible(
                        request,
                        party_id,
                        provider_profile_version_id,
                        requested_status,
                        anchor.clone(),
                        &mut scanned,
                    )
                    .await?;
                return Ok((output, has_more.then_some(anchor).flatten()));
            }

            let page = self
                .store
                .list_records_for_query(&RecordListQuery {
                    tenant_id: request.context.tenant_id.clone(),
                    owner_module_id: module_id()?,
                    record_type: request_record_type()?,
                    page_size: u32::try_from(remaining).map_err(configuration_error)?,
                    sort: RecordQuerySort::UpdatedAtDescending,
                    after: after.clone(),
                })
                .await?;
            scanned = scanned.saturating_add(page.records.len());
            enforce_scan_limit(scanned)?;

            for snapshot in &page.records {
                let enrichment_request = enrichment_request_from_snapshot(snapshot)?;
                if !matches_filters(
                    &enrichment_request,
                    party_id,
                    provider_profile_version_id,
                    requested_status,
                ) {
                    continue;
                }
                let visibility = self
                    .visibility
                    .authorize_visibility(request, &snapshot.reference)
                    .await?;
                if !visibility.resource_visible {
                    continue;
                }
                let mut output_request = enrichment_request_to_wire(&enrichment_request)?;
                redact_enrichment_request(&mut output_request, |field| {
                    visibility.allows_field(field)
                });
                output.push(output_request);
            }

            after = page.next;
            if after.is_none() {
                return Ok((output, None));
            }
        }
    }

    async fn has_more_visible(
        &self,
        request: &QueryRequest,
        party_id: &RecordId,
        provider_profile_version_id: &RecordId,
        requested_status: Option<EnrichmentRequestStatus>,
        mut after: Option<RecordQueryContinuation>,
        scanned: &mut usize,
    ) -> Result<bool, SdkError> {
        while after.is_some() {
            let page = self
                .store
                .list_records_for_query(&RecordListQuery {
                    tenant_id: request.context.tenant_id.clone(),
                    owner_module_id: module_id()?,
                    record_type: request_record_type()?,
                    page_size: MAXIMUM_PAGE_SIZE,
                    sort: RecordQuerySort::UpdatedAtDescending,
                    after: after.clone(),
                })
                .await?;
            *scanned = scanned.saturating_add(page.records.len());
            enforce_scan_limit(*scanned)?;

            for snapshot in &page.records {
                let enrichment_request = enrichment_request_from_snapshot(snapshot)?;
                if matches_filters(
                    &enrichment_request,
                    party_id,
                    provider_profile_version_id,
                    requested_status,
                ) && self
                    .visibility
                    .authorize_visibility(request, &snapshot.reference)
                    .await?
                    .resource_visible
                {
                    return Ok(true);
                }
            }
            after = page.next;
        }
        Ok(false)
    }
}

impl std::fmt::Debug for CustomerEnrichmentRequestListQueryAdapter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CustomerEnrichmentRequestListQueryAdapter")
            .field("store", &self.store)
            .field("cursor_codec", &self.cursor_codec)
            .field("visibility", &"dyn QueryVisibilityAuthorizer")
            .finish()
    }
}

impl QuerySemanticValidator for CustomerEnrichmentRequestListQueryAdapter {
    fn validate<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a QueryRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            ensure_definition(definition)?;
            let command: wire::ListEnrichmentRequestsRequest = decode_input(request)?;
            let party_id = party_record_id(command.party_ref)?;
            let provider_profile_version_id =
                provider_profile_record_id(command.provider_profile_version_ref)?;
            let requested_status = command.status.map(validate_status).transpose()?;
            let page_size = resolve_page_size(command.page_size)?;
            let binding = cursor_binding(
                request,
                &party_id,
                &provider_profile_version_id,
                requested_status,
                page_size,
            )?;
            let _ = decode_after(self, &command.cursor, &binding)?;
            Ok(())
        })
    }
}

impl QueryExecutor for CustomerEnrichmentRequestListQueryAdapter {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: QueryRequest,
    ) -> PortFuture<'a, Result<QueryExecutionResult, SdkError>> {
        Box::pin(async move {
            ensure_definition(definition)?;
            Ok(QueryExecutionResult {
                output: self.execute_list(&request).await?,
            })
        })
    }
}

pub fn query_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    Ok(CapabilityDefinition {
        capability_id: configured(CapabilityId::try_new(LIST_ENRICHMENT_REQUESTS_CAPABILITY))?,
        capability_version: configured(CapabilityVersion::try_new(support::CONTRACT_VERSION))?,
        owner_module_id: configured(ModuleId::try_new(MODULE_ID))?,
        input_contract: support::protobuf_contract(
            MODULE_ID,
            LIST_ENRICHMENT_REQUESTS_REQUEST_SCHEMA,
            vec![DataClass::Personal],
        )?,
        output_contract: Some(support::protobuf_contract(
            MODULE_ID,
            LIST_ENRICHMENT_REQUESTS_RESPONSE_SCHEMA,
            vec![DataClass::Personal],
        )?),
        risk: CapabilityRisk::Low,
        mutation: false,
        requires_idempotency: false,
        requires_approval: false,
        authorization_policy_id: LIST_ENRICHMENT_REQUESTS_CAPABILITY.to_owned(),
        rate_limit_policy_id: None,
    })
}

fn decode_input<T: Message + Default>(request: &QueryRequest) -> Result<T, SdkError> {
    let payload = &request.input;
    if payload.owner.as_str() != MODULE_ID
        || payload.schema_id.as_str() != LIST_ENRICHMENT_REQUESTS_REQUEST_SCHEMA
        || payload.schema_version.as_str() != support::CONTRACT_VERSION
        || payload.descriptor_hash
            != support::message_descriptor_hash(LIST_ENRICHMENT_REQUESTS_REQUEST_SCHEMA)
        || payload.data_class != DataClass::Personal
        || payload.encoding != PayloadEncoding::Protobuf
        || payload.maximum_size_bytes > support::MAX_PROTOBUF_BYTES
        || payload.validate().is_err()
    {
        return Err(SdkError::new(
            "CUSTOMER_ENRICHMENT_REQUEST_LIST_CONTRACT_MISMATCH",
            ErrorCategory::InvalidArgument,
            false,
            "The enrichment request list input does not match the required contract.",
        ));
    }
    T::decode(payload.bytes.as_slice()).map_err(|_| {
        SdkError::new(
            "CUSTOMER_ENRICHMENT_REQUEST_LIST_PROTOBUF_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "The enrichment request list input is not valid Protobuf.",
        )
    })
}

fn party_record_id(
    value: Option<crm_proto_contracts::crm::customer::v1::PartyRef>,
) -> Result<RecordId, SdkError> {
    let value = value.ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_enrichment.request.list.party_ref",
            "Party reference is required",
        )
    })?;
    RecordId::try_new(value.party_id).map_err(|error| {
        SdkError::invalid_argument(
            "customer_enrichment.request.list.party_ref.party_id",
            error.to_string(),
        )
    })
}

fn provider_profile_record_id(
    value: Option<wire::ProviderProfileVersionRef>,
) -> Result<RecordId, SdkError> {
    let value = value.ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_enrichment.request.list.provider_profile_version_ref",
            "Provider-profile version reference is required",
        )
    })?;
    RecordId::try_new(value.provider_profile_version_id).map_err(|error| {
        SdkError::invalid_argument(
            "customer_enrichment.request.list.provider_profile_version_ref.provider_profile_version_id",
            error.to_string(),
        )
    })
}

fn validate_status(value: i32) -> Result<EnrichmentRequestStatus, SdkError> {
    match wire::EnrichmentRequestStatus::try_from(value) {
        Ok(wire::EnrichmentRequestStatus::Created) => Ok(EnrichmentRequestStatus::Created),
        Ok(wire::EnrichmentRequestStatus::Queued) => Ok(EnrichmentRequestStatus::Queued),
        Ok(wire::EnrichmentRequestStatus::Dispatched) => Ok(EnrichmentRequestStatus::Dispatched),
        Ok(wire::EnrichmentRequestStatus::ResponseRecorded) => {
            Ok(EnrichmentRequestStatus::ResponseRecorded)
        }
        Ok(wire::EnrichmentRequestStatus::SuggestionsMaterialized) => {
            Ok(EnrichmentRequestStatus::SuggestionsMaterialized)
        }
        Ok(wire::EnrichmentRequestStatus::Completed) => Ok(EnrichmentRequestStatus::Completed),
        Ok(wire::EnrichmentRequestStatus::FailedRetryable) => {
            Ok(EnrichmentRequestStatus::FailedRetryable)
        }
        Ok(wire::EnrichmentRequestStatus::FailedTerminal) => {
            Ok(EnrichmentRequestStatus::FailedTerminal)
        }
        Ok(wire::EnrichmentRequestStatus::Cancelled) => Ok(EnrichmentRequestStatus::Cancelled),
        Ok(wire::EnrichmentRequestStatus::Expired) => Ok(EnrichmentRequestStatus::Expired),
        Ok(wire::EnrichmentRequestStatus::Unspecified) | Err(_) => Err(SdkError::invalid_argument(
            "customer_enrichment.request.list.status",
            "Status must be a supported non-unspecified enrichment request status",
        )),
    }
}

fn status_to_wire(value: EnrichmentRequestStatus) -> i32 {
    match value {
        EnrichmentRequestStatus::Created => wire::EnrichmentRequestStatus::Created as i32,
        EnrichmentRequestStatus::Queued => wire::EnrichmentRequestStatus::Queued as i32,
        EnrichmentRequestStatus::Dispatched => wire::EnrichmentRequestStatus::Dispatched as i32,
        EnrichmentRequestStatus::ResponseRecorded => {
            wire::EnrichmentRequestStatus::ResponseRecorded as i32
        }
        EnrichmentRequestStatus::SuggestionsMaterialized => {
            wire::EnrichmentRequestStatus::SuggestionsMaterialized as i32
        }
        EnrichmentRequestStatus::Completed => wire::EnrichmentRequestStatus::Completed as i32,
        EnrichmentRequestStatus::FailedRetryable => {
            wire::EnrichmentRequestStatus::FailedRetryable as i32
        }
        EnrichmentRequestStatus::FailedTerminal => {
            wire::EnrichmentRequestStatus::FailedTerminal as i32
        }
        EnrichmentRequestStatus::Cancelled => wire::EnrichmentRequestStatus::Cancelled as i32,
        EnrichmentRequestStatus::Expired => wire::EnrichmentRequestStatus::Expired as i32,
    }
}

fn matches_filters(
    request: &crm_customer_enrichment::EnrichmentRequest,
    party_id: &RecordId,
    provider_profile_version_id: &RecordId,
    requested_status: Option<EnrichmentRequestStatus>,
) -> bool {
    request.target().resource_type() == PARTY_RECORD_TYPE
        && request.target().resource_id == party_id.as_str()
        && request.provider_profile_version_id().as_str() == provider_profile_version_id.as_str()
        && requested_status
            .map(|status| request.status() == status)
            .unwrap_or(true)
}

fn resolve_page_size(value: i32) -> Result<u32, SdkError> {
    if value < 0 {
        return Err(SdkError::invalid_argument(
            "customer_enrichment.request.list.page_size",
            "Page size must not be negative",
        ));
    }
    let value = u32::try_from(value).map_err(configuration_error)?;
    let resolved = if value == 0 { DEFAULT_PAGE_SIZE } else { value };
    if resolved > MAXIMUM_PAGE_SIZE {
        return Err(SdkError::invalid_argument(
            "customer_enrichment.request.list.page_size",
            format!("Page size must not exceed {MAXIMUM_PAGE_SIZE}"),
        ));
    }
    Ok(resolved)
}

fn cursor_binding(
    request: &QueryRequest,
    party_id: &RecordId,
    provider_profile_version_id: &RecordId,
    requested_status: Option<EnrichmentRequestStatus>,
    page_size: u32,
) -> Result<CursorBinding, SdkError> {
    let status = requested_status
        .map(status_to_wire)
        .unwrap_or(wire::EnrichmentRequestStatus::Unspecified as i32)
        .to_be_bytes();
    Ok(CursorBinding {
        tenant_id: request.context.tenant_id.clone(),
        actor_id: Some(request.context.actor_id.clone()),
        capability_id: request.context.capability_id.clone(),
        capability_version: request.context.capability_version.clone(),
        resource_type: request_record_type()?,
        normalized_filter_hash: normalized_filter_hash([
            ("party_id", party_id.as_str().as_bytes()),
            (
                "provider_profile_version_id",
                provider_profile_version_id.as_str().as_bytes(),
            ),
            ("status", status.as_slice()),
        ]),
        sort_id: RecordQuerySort::UpdatedAtDescending.id().to_owned(),
        page_size,
    })
}

fn decode_after(
    adapter: &CustomerEnrichmentRequestListQueryAdapter,
    token: &str,
    binding: &CursorBinding,
) -> Result<Option<RecordQueryContinuation>, SdkError> {
    if token.is_empty() {
        return Ok(None);
    }
    let continuation = adapter
        .cursor_codec
        .decode(token, binding)
        .map_err(cursor_error)?;
    let sort_value = String::from_utf8(continuation.sort_key).map_err(|_| cursor_invalid())?;
    let after = RecordQueryContinuation {
        sort_value,
        record_id: continuation.record_id,
    };
    after.validate().map_err(cursor_error)?;
    Ok(Some(after))
}

fn encode_next(
    adapter: &CustomerEnrichmentRequestListQueryAdapter,
    binding: &CursorBinding,
    next: Option<&RecordQueryContinuation>,
) -> Result<String, SdkError> {
    next.map(|next| {
        adapter
            .cursor_codec
            .encode(
                binding,
                &CursorContinuation {
                    sort_key: next.sort_value.as_bytes().to_vec(),
                    record_id: next.record_id.clone(),
                },
            )
            .map_err(cursor_error)
    })
    .transpose()
    .map(|value| value.unwrap_or_default())
}

fn list_response(
    enrichment_requests: Vec<wire::EnrichmentRequest>,
    next_cursor: String,
) -> Result<TypedPayload, SdkError> {
    support::protobuf_payload(
        MODULE_ID,
        LIST_ENRICHMENT_REQUESTS_RESPONSE_SCHEMA,
        DataClass::Personal,
        &wire::ListEnrichmentRequestsResponse {
            enrichment_requests,
            next_cursor,
        },
    )
}

fn redact_enrichment_request(
    output: &mut wire::EnrichmentRequest,
    allows_field: impl Fn(&str) -> bool,
) {
    if !allows_field("requested_by_actor_id") {
        output.requested_by_actor_id.clear();
    }
    if !allows_field("target") {
        output.target = None;
    }
    if !allows_field("provider_profile_version_ref") {
        output.provider_profile_version_ref = None;
    }
    if !allows_field("mapping_version_ref") {
        output.mapping_version_ref = None;
    }
    if !allows_field("requested_fields") {
        output.requested_fields.clear();
    }
    if !allows_field("policy_evidence") {
        output.policy_evidence = None;
    }
    if !allows_field("created_at_unix_ms") {
        output.created_at_unix_ms = 0;
    }
    if !allows_field("deadline_at_unix_ms") {
        output.deadline_at_unix_ms = 0;
    }
    if !allows_field("expires_at_unix_ms") {
        output.expires_at_unix_ms = 0;
    }
    if !allows_field("status") {
        output.status = wire::EnrichmentRequestStatus::Unspecified as i32;
    }
    if !allows_field("retry_generation") {
        output.retry_generation = 0;
    }
    if !allows_field("provider_response_receipt_ref") {
        output.provider_response_receipt_ref = None;
    }
    if !allows_field("last_safe_failure_code") {
        output.last_safe_failure_code = None;
    }
    if !allows_field("updated_at_unix_ms") {
        output.updated_at_unix_ms = 0;
    }
}

fn ensure_definition(definition: &CapabilityDefinition) -> Result<(), SdkError> {
    if definition.owner_module_id.as_str() != MODULE_ID
        || definition.capability_id.as_str() != LIST_ENRICHMENT_REQUESTS_CAPABILITY
        || definition.capability_version.as_str() != support::CONTRACT_VERSION
        || definition.mutation
    {
        return Err(unsupported_query());
    }
    Ok(())
}

fn module_id() -> Result<ModuleId, SdkError> {
    configured(ModuleId::try_new(MODULE_ID))
}

fn request_record_type() -> Result<RecordType, SdkError> {
    configured(RecordType::try_new(ENRICHMENT_REQUEST_RECORD_TYPE))
}

fn party_record_type() -> Result<RecordType, SdkError> {
    configured(RecordType::try_new(PARTY_RECORD_TYPE))
}

fn configured<T>(value: Result<T, crm_module_sdk::IdentifierError>) -> Result<T, SdkError> {
    value.map_err(configuration_error)
}

fn enforce_scan_limit(scanned: usize) -> Result<(), SdkError> {
    if scanned > MAXIMUM_VISIBILITY_SCAN_RECORDS {
        Err(SdkError::new(
            "CUSTOMER_ENRICHMENT_REQUEST_LIST_SCAN_LIMIT_EXCEEDED",
            ErrorCategory::Unavailable,
            true,
            "The enrichment request list is temporarily unavailable.",
        ))
    } else {
        Ok(())
    }
}

fn cursor_invalid() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_REQUEST_LIST_CURSOR_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "The enrichment request list cursor is invalid.",
    )
}

fn cursor_error(error: impl std::fmt::Display) -> SdkError {
    cursor_invalid().with_internal_reference(error.to_string())
}

fn unsupported_query() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_REQUEST_LIST_UNSUPPORTED",
        ErrorCategory::Internal,
        false,
        "The enrichment request list query is not configured.",
    )
}

fn configuration_error(error: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_REQUEST_LIST_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The enrichment request list query configuration is invalid.",
    )
    .with_internal_reference(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn definition_is_one_personal_low_risk_query() {
        let definition = query_capability_definition().unwrap();
        assert_eq!(
            definition.capability_id.as_str(),
            LIST_ENRICHMENT_REQUESTS_CAPABILITY
        );
        assert_eq!(definition.owner_module_id.as_str(), MODULE_ID);
        assert_eq!(definition.capability_version.as_str(), "1.0.0");
        assert_eq!(
            definition.input_contract.allowed_data_classes,
            vec![DataClass::Personal]
        );
        assert!(!definition.mutation);
        assert!(!definition.requires_idempotency);
        assert!(!definition.requires_approval);
        assert_eq!(definition.risk, CapabilityRisk::Low);
    }

    #[test]
    fn page_size_and_status_are_bounded() {
        assert_eq!(resolve_page_size(0).unwrap(), DEFAULT_PAGE_SIZE);
        assert_eq!(resolve_page_size(100).unwrap(), 100);
        assert!(resolve_page_size(-1).is_err());
        assert!(resolve_page_size(101).is_err());
        assert!(validate_status(wire::EnrichmentRequestStatus::Created as i32).is_ok());
        assert!(validate_status(wire::EnrichmentRequestStatus::Unspecified as i32).is_err());
    }
}
