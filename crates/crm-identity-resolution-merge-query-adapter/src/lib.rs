#![forbid(unsafe_code)]

//! Permission-aware queries for authoritative Party merge lineage and current
//! canonical Party resolution.

use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRisk};
use crm_core_data::{
    MAXIMUM_RELATED_RECORD_QUERY_PAGE_SIZE, PostgresDataStore, RecordGetQuery,
    RelatedRecordListQuery,
};
use crm_identity_resolution::{MergeOperation, MergeOperationId, MergeOperationStatus, PartyReference};
use crm_identity_resolution_capability_adapter::{
    CANONICAL_REDIRECT_PARTY_RECORD_TYPE, CANONICAL_REDIRECT_RELATIONSHIP_TYPE,
    MERGE_OPERATION_RECORD_TYPE, MODULE_ID, PARTY_MERGE_RELATIONSHIP_TYPE,
    PARTY_MERGE_SOURCE_RECORD_TYPE, merge_operation_from_snapshot, merge_operation_to_wire,
};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, PayloadEncoding, PortFuture,
    RecordId, RecordRef, RecordType, RelationshipType, SdkError, TypedPayload,
};
use crm_parties_capability_adapter::{MODULE_ID as PARTIES_MODULE_ID, RECORD_TYPE as PARTY_RECORD_TYPE};
use crm_proto_contracts::crm::{customer::v1 as customer, identity_resolution::v1 as wire};
use crm_query_runtime::{
    CursorBinding, CursorCodec, CursorContinuation, PageSizePolicy, QueryExecutionResult, QueryExecutor,
    QueryRequest, QuerySemanticValidator, QueryVisibilityAuthorizer, QueryVisibilityDecision,
    normalized_filter_hash,
};
use prost::Message;
use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

pub const GET_CAPABILITY: &str = "identity_resolution.merge.get";
pub const LIST_BY_PARTY_CAPABILITY: &str = "identity_resolution.merge.list_by_party";
pub const RESOLVE_CANONICAL_CAPABILITY: &str = "identity_resolution.party.resolve_canonical";

pub const GET_REQUEST_SCHEMA: &str = "crm.identity_resolution.v1.GetMergeOperationRequest";
pub const GET_RESPONSE_SCHEMA: &str = "crm.identity_resolution.v1.GetMergeOperationResponse";
pub const LIST_BY_PARTY_REQUEST_SCHEMA: &str =
    "crm.identity_resolution.v1.ListMergeOperationsByPartyRequest";
pub const LIST_BY_PARTY_RESPONSE_SCHEMA: &str =
    "crm.identity_resolution.v1.ListMergeOperationsByPartyResponse";
pub const RESOLVE_CANONICAL_REQUEST_SCHEMA: &str =
    "crm.identity_resolution.v1.ResolveCanonicalPartyRequest";
pub const RESOLVE_CANONICAL_RESPONSE_SCHEMA: &str =
    "crm.identity_resolution.v1.ResolveCanonicalPartyResponse";

pub const QUERY_CAPABILITY_IDS: [&str; 3] = [
    GET_CAPABILITY,
    LIST_BY_PARTY_CAPABILITY,
    RESOLVE_CANONICAL_CAPABILITY,
];

const DEFAULT_PAGE_SIZE: u32 = 50;
const MAXIMUM_PAGE_SIZE: u32 = 200;
const MAXIMUM_VISIBILITY_SCAN_RECORDS: usize = 10_000;
const MAXIMUM_CANONICAL_REDIRECT_HOPS: usize = 64;
const RELATED_RECORD_SORT_ID: &str = "related-record-id-ascending";

pub fn query_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    QUERY_CAPABILITY_IDS
        .into_iter()
        .map(query_capability_definition)
        .collect()
}

pub fn query_capability_definition(capability_id: &str) -> Result<CapabilityDefinition, SdkError> {
    let (input_schema, output_schema) = match capability_id {
        GET_CAPABILITY => (GET_REQUEST_SCHEMA, GET_RESPONSE_SCHEMA),
        LIST_BY_PARTY_CAPABILITY => (LIST_BY_PARTY_REQUEST_SCHEMA, LIST_BY_PARTY_RESPONSE_SCHEMA),
        RESOLVE_CANONICAL_CAPABILITY => (
            RESOLVE_CANONICAL_REQUEST_SCHEMA,
            RESOLVE_CANONICAL_RESPONSE_SCHEMA,
        ),
        _ => return Err(unsupported_query()),
    };
    Ok(CapabilityDefinition {
        capability_id: configured_capability_id(capability_id)?,
        capability_version: configured_capability_version(support::CONTRACT_VERSION)?,
        owner_module_id: configured_module_id(MODULE_ID)?,
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

#[derive(Clone)]
pub struct IdentityResolutionMergeQueryAdapter {
    store: PostgresDataStore,
    cursor_codec: CursorCodec,
    visibility: Arc<dyn QueryVisibilityAuthorizer>,
    page_policy: PageSizePolicy,
}

impl fmt::Debug for IdentityResolutionMergeQueryAdapter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IdentityResolutionMergeQueryAdapter")
            .field("store", &self.store)
            .field("cursor_codec", &self.cursor_codec)
            .field("visibility", &"QueryVisibilityAuthorizer")
            .field("page_policy", &self.page_policy)
            .finish()
    }
}

impl IdentityResolutionMergeQueryAdapter {
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

    async fn execute_get(&self, request: &QueryRequest) -> Result<TypedPayload, SdkError> {
        let command: wire::GetMergeOperationRequest = decode_input(request, GET_REQUEST_SCHEMA)?;
        let operation_id = required_merge_operation_ref(command.merge_operation_ref)?;
        let snapshot = self
            .store
            .get_record_for_query(&RecordGetQuery {
                tenant_id: request.context.tenant_id.clone(),
                owner_module_id: configured_module_id(MODULE_ID)?,
                record_type: configured_record_type(MERGE_OPERATION_RECORD_TYPE)?,
                record_id: validate_record_id(
                    operation_id.as_str(),
                    "identity_resolution.merge.merge_operation_ref.merge_operation_id",
                )?,
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
        let operation = merge_operation_from_snapshot(&snapshot)?;
        support::protobuf_payload(
            MODULE_ID,
            GET_RESPONSE_SCHEMA,
            DataClass::Personal,
            &wire::GetMergeOperationResponse {
                merge_operation: Some(merge_operation_to_wire_with_visibility(
                    &operation,
                    &visibility,
                )),
            },
        )
    }

    async fn execute_list_by_party(&self, request: &QueryRequest) -> Result<TypedPayload, SdkError> {
        let command: wire::ListMergeOperationsByPartyRequest =
            decode_input(request, LIST_BY_PARTY_REQUEST_SCHEMA)?;
        let party_ref = required_party_ref(command.party_ref.as_ref())?;
        let status = optional_status(command.status)?;
        let page_size = self
            .page_policy
            .resolve(command.page_size)
            .map_err(cursor_error)?;
        let binding = cursor_binding(request, &party_ref, status, page_size)?;
        let after_record_id = decode_after(self, &command.cursor, &binding)?;
        let (merge_operations, next_record_id) = self
            .collect_merge_operations(request, &party_ref, status, page_size, after_record_id)
            .await?;
        let next_cursor = encode_next(self, &binding, next_record_id.as_ref())?;
        support::protobuf_payload(
            MODULE_ID,
            LIST_BY_PARTY_RESPONSE_SCHEMA,
            DataClass::Personal,
            &wire::ListMergeOperationsByPartyResponse {
                merge_operations,
                next_cursor,
            },
        )
    }

    async fn execute_resolve(&self, request: &QueryRequest) -> Result<TypedPayload, SdkError> {
        let command: wire::ResolveCanonicalPartyRequest =
            decode_input(request, RESOLVE_CANONICAL_REQUEST_SCHEMA)?;
        let requested = required_party_ref(command.party_ref.as_ref())?;
        let mut current = requested.clone();
        let mut party_path = vec![customer::PartyRef {
            party_id: current.as_str().to_owned(),
        }];
        let mut operation_path = Vec::new();
        let mut visited = BTreeSet::from([current.clone()]);

        for _ in 0..MAXIMUM_CANONICAL_REDIRECT_HOPS {
            let Some(target) = self.immediate_redirect_target(request, &current).await? else {
                return support::protobuf_payload(
                    MODULE_ID,
                    RESOLVE_CANONICAL_RESPONSE_SCHEMA,
                    DataClass::Personal,
                    &wire::ResolveCanonicalPartyResponse {
                        resolution: Some(wire::CanonicalPartyResolution {
                            requested_party_ref: Some(customer::PartyRef {
                                party_id: requested.as_str().to_owned(),
                            }),
                            canonical_party_ref: Some(customer::PartyRef {
                                party_id: current.as_str().to_owned(),
                            }),
                            party_path,
                            merge_operation_path: operation_path,
                        }),
                    },
                );
            };
            if !visited.insert(target.clone()) {
                return Err(canonical_redirect_corrupt("canonical redirect topology contains a cycle"));
            }
            let operation = self
                .active_operation_for_edge(request, &current, &target)
                .await?;
            operation_path.push(wire::MergeOperationRef {
                merge_operation_id: operation.operation_id().as_str().to_owned(),
            });
            current = target;
            party_path.push(customer::PartyRef {
                party_id: current.as_str().to_owned(),
            });
        }
        Err(canonical_redirect_corrupt(
            "canonical redirect topology exceeds the supported hop bound",
        ))
    }

    async fn collect_merge_operations(
        &self,
        request: &QueryRequest,
        party_ref: &PartyReference,
        status: Option<MergeOperationStatus>,
        page_size: u32,
        mut after_record_id: Option<RecordId>,
    ) -> Result<(Vec<wire::MergeOperation>, Option<RecordId>), SdkError> {
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
                .list_related_records_for_query(&merge_related_query(
                    request,
                    party_ref,
                    related_page_size,
                    after_record_id.clone(),
                )?)
                .await?;
            scanned = scanned.saturating_add(page.records.len());
            enforce_visibility_scan_limit(scanned)?;
            for snapshot in &page.records {
                let operation = merge_operation_from_snapshot(snapshot)?;
                if status.is_some_and(|expected| operation.status() != expected) {
                    continue;
                }
                let visibility = self
                    .visibility
                    .authorize_visibility(request, &snapshot.reference)
                    .await?;
                if visibility.resource_visible {
                    output.push(merge_operation_to_wire_with_visibility(
                        &operation,
                        &visibility,
                    ));
                }
            }
            let next_anchor = page.next_record_id;
            if output.len() == page_size as usize {
                let has_more = match next_anchor.as_ref() {
                    Some(anchor) => {
                        self.has_more_visible_operation(
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

    async fn has_more_visible_operation(
        &self,
        request: &QueryRequest,
        party_ref: &PartyReference,
        status: Option<MergeOperationStatus>,
        mut after_record_id: RecordId,
        scanned: &mut usize,
    ) -> Result<bool, SdkError> {
        loop {
            let page = self
                .store
                .list_related_records_for_query(&merge_related_query(
                    request,
                    party_ref,
                    MAXIMUM_PAGE_SIZE,
                    Some(after_record_id),
                )?)
                .await?;
            *scanned = scanned.saturating_add(page.records.len());
            enforce_visibility_scan_limit(*scanned)?;
            for snapshot in &page.records {
                let operation = merge_operation_from_snapshot(snapshot)?;
                if status.is_some_and(|expected| operation.status() != expected) {
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

    async fn immediate_redirect_target(
        &self,
        request: &QueryRequest,
        source: &PartyReference,
    ) -> Result<Option<PartyReference>, SdkError> {
        let page = self
            .store
            .list_related_records_for_query(&RelatedRecordListQuery {
                tenant_id: request.context.tenant_id.clone(),
                relationship_owner_module_id: configured_module_id(MODULE_ID)?,
                relationship_type: configured_relationship_type(
                    CANONICAL_REDIRECT_RELATIONSHIP_TYPE,
                )?,
                source: RecordRef {
                    record_type: configured_record_type(CANONICAL_REDIRECT_PARTY_RECORD_TYPE)?,
                    record_id: validate_record_id(
                        source.as_str(),
                        "identity_resolution.resolve.party_ref.party_id",
                    )?,
                },
                target_owner_module_id: configured_module_id(PARTIES_MODULE_ID)?,
                target_record_type: configured_record_type(PARTY_RECORD_TYPE)?,
                page_size: 2,
                after_record_id: None,
            })
            .await?;
        if page.records.len() > 1 || page.next_record_id.is_some() {
            return Err(canonical_redirect_corrupt(
                "more than one active canonical redirect exists for one source Party",
            ));
        }
        page.records
            .first()
            .map(|snapshot| PartyReference::try_new(snapshot.reference.record_id.as_str()))
            .transpose()
    }

    async fn active_operation_for_edge(
        &self,
        request: &QueryRequest,
        source: &PartyReference,
        target: &PartyReference,
    ) -> Result<MergeOperation, SdkError> {
        let mut after_record_id = None;
        let mut match_found = None;
        let mut scanned = 0_usize;
        loop {
            let page = self
                .store
                .list_related_records_for_query(&merge_related_query(
                    request,
                    source,
                    MAXIMUM_PAGE_SIZE,
                    after_record_id.clone(),
                )?)
                .await?;
            scanned = scanned.saturating_add(page.records.len());
            enforce_visibility_scan_limit(scanned)?;
            for snapshot in &page.records {
                let operation = merge_operation_from_snapshot(snapshot)?;
                if operation.status() != MergeOperationStatus::Active
                    || operation.source_party_ref() != source
                    || operation.survivor_party_ref() != target
                {
                    continue;
                }
                if match_found.is_some() {
                    return Err(canonical_redirect_corrupt(
                        "more than one active merge operation matches one canonical redirect edge",
                    ));
                }
                let visibility = self
                    .visibility
                    .authorize_visibility(request, &snapshot.reference)
                    .await?;
                if !visibility.resource_visible {
                    return Err(resource_not_found());
                }
                match_found = Some(operation);
            }
            after_record_id = page.next_record_id;
            if after_record_id.is_none() {
                return match_found.ok_or_else(|| {
                    canonical_redirect_corrupt(
                        "canonical redirect edge has no matching active merge operation",
                    )
                });
            }
        }
    }
}

impl QuerySemanticValidator for IdentityResolutionMergeQueryAdapter {
    fn validate<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a QueryRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            match definition.capability_id.as_str() {
                GET_CAPABILITY => {
                    let command: wire::GetMergeOperationRequest =
                        decode_input(request, GET_REQUEST_SCHEMA)?;
                    let _ = required_merge_operation_ref(command.merge_operation_ref)?;
                }
                LIST_BY_PARTY_CAPABILITY => {
                    let command: wire::ListMergeOperationsByPartyRequest =
                        decode_input(request, LIST_BY_PARTY_REQUEST_SCHEMA)?;
                    validate_list(self, request, &command)?;
                }
                RESOLVE_CANONICAL_CAPABILITY => {
                    let command: wire::ResolveCanonicalPartyRequest =
                        decode_input(request, RESOLVE_CANONICAL_REQUEST_SCHEMA)?;
                    let _ = required_party_ref(command.party_ref.as_ref())?;
                }
                _ => return Err(unsupported_query()),
            }
            Ok(())
        })
    }
}

impl QueryExecutor for IdentityResolutionMergeQueryAdapter {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: QueryRequest,
    ) -> PortFuture<'a, Result<QueryExecutionResult, SdkError>> {
        Box::pin(async move {
            let output = match definition.capability_id.as_str() {
                GET_CAPABILITY => self.execute_get(&request).await?,
                LIST_BY_PARTY_CAPABILITY => self.execute_list_by_party(&request).await?,
                RESOLVE_CANONICAL_CAPABILITY => self.execute_resolve(&request).await?,
                _ => return Err(unsupported_query()),
            };
            Ok(QueryExecutionResult { output })
        })
    }
}

fn merge_related_query(
    request: &QueryRequest,
    party_ref: &PartyReference,
    page_size: u32,
    after_record_id: Option<RecordId>,
) -> Result<RelatedRecordListQuery, SdkError> {
    Ok(RelatedRecordListQuery {
        tenant_id: request.context.tenant_id.clone(),
        relationship_owner_module_id: configured_module_id(MODULE_ID)?,
        relationship_type: configured_relationship_type(PARTY_MERGE_RELATIONSHIP_TYPE)?,
        source: RecordRef {
            record_type: configured_record_type(PARTY_MERGE_SOURCE_RECORD_TYPE)?,
            record_id: validate_record_id(
                party_ref.as_str(),
                "identity_resolution.merge.party_ref.party_id",
            )?,
        },
        target_owner_module_id: configured_module_id(MODULE_ID)?,
        target_record_type: configured_record_type(MERGE_OPERATION_RECORD_TYPE)?,
        page_size,
        after_record_id,
    })
}

fn validate_list(
    adapter: &IdentityResolutionMergeQueryAdapter,
    request: &QueryRequest,
    command: &wire::ListMergeOperationsByPartyRequest,
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

fn merge_operation_to_wire_with_visibility(
    operation: &MergeOperation,
    visibility: &QueryVisibilityDecision,
) -> wire::MergeOperation {
    let mut output = merge_operation_to_wire(operation);
    if !visibility.allows_field("party_pair") {
        output.source_party_ref = None;
        output.survivor_party_ref = None;
    }
    if !visibility.allows_field("decision") {
        output.decision_ref.clear();
        output.decided_by_actor_id.clear();
        output.reason.clear();
    }
    if !visibility.allows_field("survivorship") {
        output.survivorship.clear();
    }
    if !visibility.allows_field("status") {
        output.status = wire::MergeOperationStatus::Unspecified as i32;
    }
    if !visibility.allows_field("unmerge_decision") {
        output.unmerge_decision = None;
    }
    output
}

fn required_party_ref(value: Option<&customer::PartyRef>) -> Result<PartyReference, SdkError> {
    let value = value.ok_or_else(|| {
        SdkError::invalid_argument(
            "identity_resolution.merge.party_ref",
            "Party reference is required",
        )
    })?;
    PartyReference::try_new(value.party_id.clone())
}

fn required_merge_operation_ref(
    value: Option<wire::MergeOperationRef>,
) -> Result<MergeOperationId, SdkError> {
    let value = value.ok_or_else(|| {
        SdkError::invalid_argument(
            "identity_resolution.merge.merge_operation_ref",
            "merge operation reference is required",
        )
    })?;
    MergeOperationId::try_new(value.merge_operation_id)
}

fn optional_status(value: i32) -> Result<Option<MergeOperationStatus>, SdkError> {
    match wire::MergeOperationStatus::try_from(value) {
        Ok(wire::MergeOperationStatus::Unspecified) => Ok(None),
        Ok(wire::MergeOperationStatus::Active) => Ok(Some(MergeOperationStatus::Active)),
        Ok(wire::MergeOperationStatus::Unmerged) => Ok(Some(MergeOperationStatus::Unmerged)),
        Err(_) => Err(SdkError::invalid_argument(
            "identity_resolution.merge.status",
            "merge operation status filter is invalid",
        )),
    }
}

fn status_wire_value(value: Option<MergeOperationStatus>) -> i32 {
    match value {
        None => wire::MergeOperationStatus::Unspecified as i32,
        Some(MergeOperationStatus::Active) => wire::MergeOperationStatus::Active as i32,
        Some(MergeOperationStatus::Unmerged) => wire::MergeOperationStatus::Unmerged as i32,
    }
}

fn cursor_binding(
    request: &QueryRequest,
    party_ref: &PartyReference,
    status: Option<MergeOperationStatus>,
    page_size: u32,
) -> Result<CursorBinding, SdkError> {
    let status = status_wire_value(status).to_be_bytes();
    Ok(CursorBinding {
        tenant_id: request.context.tenant_id.clone(),
        actor_id: Some(request.context.actor_id.clone()),
        capability_id: request.context.capability_id.clone(),
        capability_version: request.context.capability_version.clone(),
        resource_type: configured_record_type(MERGE_OPERATION_RECORD_TYPE)?,
        normalized_filter_hash: normalized_filter_hash([
            ("party_id", party_ref.as_str().as_bytes()),
            ("status", status.as_slice()),
        ]),
        sort_id: RELATED_RECORD_SORT_ID.to_owned(),
        page_size,
    })
}

fn decode_after(
    adapter: &IdentityResolutionMergeQueryAdapter,
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
    adapter: &IdentityResolutionMergeQueryAdapter,
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
            "IDENTITY_RESOLUTION_MERGE_QUERY_INPUT_CONTRACT_MISMATCH",
            ErrorCategory::InvalidArgument,
            false,
            "The Identity Resolution merge query input does not match the required contract.",
        ));
    }
    M::decode(payload.bytes.as_slice()).map_err(|_| {
        SdkError::new(
            "IDENTITY_RESOLUTION_MERGE_QUERY_INPUT_PROTOBUF_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "The Identity Resolution merge query input is not valid Protobuf.",
        )
    })
}

fn validate_record_id(value: &str, field: &'static str) -> Result<RecordId, SdkError> {
    RecordId::try_new(value.to_owned())
        .map_err(|error| SdkError::invalid_argument(field, error.to_string()))
}

fn configured_capability_id(value: &str) -> Result<CapabilityId, SdkError> {
    CapabilityId::try_new(value).map_err(config_error)
}

fn configured_capability_version(value: &str) -> Result<CapabilityVersion, SdkError> {
    CapabilityVersion::try_new(value).map_err(config_error)
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
        "IDENTITY_RESOLUTION_MERGE_QUERY_SCAN_LIMIT_EXCEEDED",
        ErrorCategory::Unavailable,
        true,
        "The Identity Resolution merge query could not complete within the bounded visibility scan.",
    )
}

fn canonical_redirect_corrupt(internal: impl Into<String>) -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_CANONICAL_REDIRECT_INVALID",
        ErrorCategory::Internal,
        false,
        "The canonical Party redirect topology is temporarily unavailable.",
    )
    .with_internal_reference(internal)
}

fn cursor_invalid() -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_MERGE_QUERY_CURSOR_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "The Identity Resolution merge page cursor is invalid.",
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
            crm_query_runtime::CursorError::PageSizeTooLarge
            | crm_query_runtime::CursorError::InvalidPageSize
            | crm_query_runtime::CursorError::ContinuationTooLarge
            | crm_query_runtime::CursorError::TokenTooLarge
            | crm_query_runtime::CursorError::MalformedToken
            | crm_query_runtime::CursorError::IntegrityFailed
            | crm_query_runtime::CursorError::BindingMismatch
            | crm_query_runtime::CursorError::UnsupportedVersion
            | crm_query_runtime::CursorError::InvalidStoredValue => ErrorCategory::InvalidArgument,
        },
        false,
        error.safe_message(),
    )
}

fn unsupported_query() -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_MERGE_QUERY_CAPABILITY_UNSUPPORTED",
        ErrorCategory::Internal,
        false,
        "The Identity Resolution merge query capability is unsupported.",
    )
}

fn config_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_MERGE_QUERY_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Identity Resolution merge query adapter is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publishes_get_list_and_canonical_resolution_query_coordinates() {
        let definitions = query_capability_definitions().unwrap();
        assert_eq!(definitions.len(), 3);
        assert_eq!(definitions[0].capability_id.as_str(), GET_CAPABILITY);
        assert_eq!(definitions[1].capability_id.as_str(), LIST_BY_PARTY_CAPABILITY);
        assert_eq!(
            definitions[2].capability_id.as_str(),
            RESOLVE_CANONICAL_CAPABILITY
        );
        for definition in definitions {
            assert_eq!(definition.owner_module_id.as_str(), MODULE_ID);
            assert!(!definition.mutation);
            assert!(!definition.requires_idempotency);
            assert!(!definition.requires_approval);
        }
    }
}
