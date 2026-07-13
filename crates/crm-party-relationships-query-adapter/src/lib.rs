#![forbid(unsafe_code)]

use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRisk};
use crm_core_data::{
    PostgresDataStore, RecordGetQuery, RecordListQuery, RecordQueryContinuation, RecordQuerySort,
};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, PayloadEncoding,
    PortFuture, RecordId, RecordType, SdkError, TypedPayload,
};
use crm_party_relationships::{
    PartyRelationship, PartyRelationshipStatus, RelationshipDirectionality,
};
use crm_party_relationships_capability_adapter::{
    MODULE_ID, RECORD_TYPE, party_relationship_from_snapshot, party_relationship_to_wire,
};
use crm_proto_contracts::crm::{core::v1 as core, party_relationships::v1 as wire};
use crm_query_runtime::{
    CursorBinding, CursorCodec, CursorContinuation, PageSizePolicy, QueryExecutionResult,
    QueryExecutor, QueryRequest, QuerySemanticValidator, QueryVisibilityAuthorizer,
    QueryVisibilityDecision, normalized_filter_hash,
};
use prost::Message;
use std::sync::Arc;

pub const GET_CAPABILITY: &str = "party-relationships.party-relationship.get";
pub const LIST_CAPABILITY: &str = "party-relationships.party-relationship.list";
pub const GET_REQUEST_SCHEMA: &str = "crm.party_relationships.v1.GetPartyRelationshipRequest";
pub const GET_RESPONSE_SCHEMA: &str = "crm.party_relationships.v1.GetPartyRelationshipResponse";
pub const LIST_REQUEST_SCHEMA: &str = "crm.party_relationships.v1.ListPartyRelationshipsRequest";
pub const LIST_RESPONSE_SCHEMA: &str = "crm.party_relationships.v1.ListPartyRelationshipsResponse";
pub const QUERY_CAPABILITY_IDS: [&str; 2] = [GET_CAPABILITY, LIST_CAPABILITY];

const DEFAULT_PAGE_SIZE: u32 = 50;
const MAXIMUM_PAGE_SIZE: u32 = 200;
const MAXIMUM_VISIBILITY_SCAN_RECORDS: usize = 10_000;

#[derive(Clone)]
pub struct PartyRelationshipQueryAdapter {
    store: PostgresDataStore,
    cursor_codec: CursorCodec,
    visibility: Arc<dyn QueryVisibilityAuthorizer>,
    page_policy: PageSizePolicy,
}

impl std::fmt::Debug for PartyRelationshipQueryAdapter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PartyRelationshipQueryAdapter")
            .field("store", &self.store)
            .field("cursor_codec", &self.cursor_codec)
            .field("visibility", &"dyn QueryVisibilityAuthorizer")
            .field("page_policy", &self.page_policy)
            .finish()
    }
}

impl PartyRelationshipQueryAdapter {
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
        LIST_CAPABILITY => (LIST_REQUEST_SCHEMA, LIST_RESPONSE_SCHEMA),
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

impl QuerySemanticValidator for PartyRelationshipQueryAdapter {
    fn validate<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a QueryRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            match definition.capability_id.as_str() {
                GET_CAPABILITY => {
                    let command: wire::GetPartyRelationshipRequest =
                        decode_input(request, GET_REQUEST_SCHEMA)?;
                    let relationship_ref = command.party_relationship_ref.ok_or_else(|| {
                        SdkError::invalid_argument(
                            "party_relationship.party_relationship_ref",
                            "Party Relationship reference is required",
                        )
                    })?;
                    validate_record_id(&relationship_ref.party_relationship_id)?;
                }
                LIST_CAPABILITY => {
                    let command: wire::ListPartyRelationshipsRequest =
                        decode_input(request, LIST_REQUEST_SCHEMA)?;
                    validate_list(self, request, &command)?;
                }
                _ => return Err(unsupported_query()),
            }
            Ok(())
        })
    }
}

impl QueryExecutor for PartyRelationshipQueryAdapter {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: QueryRequest,
    ) -> PortFuture<'a, Result<QueryExecutionResult, SdkError>> {
        Box::pin(async move {
            let output = match definition.capability_id.as_str() {
                GET_CAPABILITY => self.execute_get(&request).await?,
                LIST_CAPABILITY => self.execute_list(&request).await?,
                _ => return Err(unsupported_query()),
            };
            Ok(QueryExecutionResult { output })
        })
    }
}

impl PartyRelationshipQueryAdapter {
    async fn execute_get(&self, request: &QueryRequest) -> Result<TypedPayload, SdkError> {
        let command: wire::GetPartyRelationshipRequest = decode_input(request, GET_REQUEST_SCHEMA)?;
        let relationship_ref = command.party_relationship_ref.ok_or_else(|| {
            SdkError::invalid_argument(
                "party_relationship.party_relationship_ref",
                "Party Relationship reference is required",
            )
        })?;
        let record_id = validate_record_id(&relationship_ref.party_relationship_id)?;
        let snapshot = self
            .store
            .get_record_for_query(&RecordGetQuery {
                tenant_id: request.context.tenant_id.clone(),
                owner_module_id: ModuleId::try_new(MODULE_ID).map_err(config_error)?,
                record_type: relationship_record_type()?,
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
        let relationship = party_relationship_from_snapshot(&snapshot)?;

        support::protobuf_payload(
            MODULE_ID,
            GET_RESPONSE_SCHEMA,
            DataClass::Personal,
            &wire::GetPartyRelationshipResponse {
                party_relationship: Some(party_relationship_to_wire_with_visibility(
                    &relationship,
                    &visibility,
                )),
            },
        )
    }

    async fn execute_list(&self, request: &QueryRequest) -> Result<TypedPayload, SdkError> {
        let command: wire::ListPartyRelationshipsRequest =
            decode_input(request, LIST_REQUEST_SCHEMA)?;
        let page_size = resolve_page_size(self.page_policy, command.page.as_ref())?;
        let filters = RelationshipFilters::from_wire(&command)?;
        let binding = cursor_binding(
            request,
            relationship_record_type()?,
            relationship_filter_hash(&command),
            RecordQuerySort::UpdatedAtDescending,
            page_size,
        );
        let after = decode_after(self, command.page.as_ref(), &binding)?;
        let (relationships, next) = self
            .collect_relationships(request, page_size, after, &filters)
            .await?;
        let next_page_token = encode_next(self, &binding, next.as_ref())?;

        support::protobuf_payload(
            MODULE_ID,
            LIST_RESPONSE_SCHEMA,
            DataClass::Personal,
            &wire::ListPartyRelationshipsResponse {
                party_relationships: relationships,
                page: Some(core::PageInfo {
                    next_page_token,
                    total_size: 0,
                }),
            },
        )
    }

    async fn collect_relationships(
        &self,
        request: &QueryRequest,
        page_size: u32,
        mut after: Option<RecordQueryContinuation>,
        filters: &RelationshipFilters,
    ) -> Result<
        (
            Vec<wire::PartyRelationship>,
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
                    .has_more_visible_relationship(request, anchor.clone(), filters, &mut scanned)
                    .await?;
                return Ok((output, has_more.then_some(anchor).flatten()));
            }

            let page = self
                .store
                .list_records_for_query(&RecordListQuery {
                    tenant_id: request.context.tenant_id.clone(),
                    owner_module_id: ModuleId::try_new(MODULE_ID).map_err(config_error)?,
                    record_type: relationship_record_type()?,
                    page_size: u32::try_from(remaining).map_err(|_| scan_limit_error())?,
                    sort: RecordQuerySort::UpdatedAtDescending,
                    after: after.clone(),
                })
                .await?;
            scanned = scanned.saturating_add(page.records.len());
            enforce_scan_limit(scanned)?;
            for snapshot in &page.records {
                let relationship = party_relationship_from_snapshot(snapshot)?;
                if !relationship_matches_filters(&relationship, filters) {
                    continue;
                }
                let visibility = self
                    .visibility
                    .authorize_visibility(request, &snapshot.reference)
                    .await?;
                if visibility.resource_visible {
                    output.push(party_relationship_to_wire_with_visibility(
                        &relationship,
                        &visibility,
                    ));
                }
            }
            after = page.next;
            if after.is_none() {
                return Ok((output, None));
            }
        }
    }

    async fn has_more_visible_relationship(
        &self,
        request: &QueryRequest,
        mut after: Option<RecordQueryContinuation>,
        filters: &RelationshipFilters,
        scanned: &mut usize,
    ) -> Result<bool, SdkError> {
        while after.is_some() {
            let page = self
                .store
                .list_records_for_query(&RecordListQuery {
                    tenant_id: request.context.tenant_id.clone(),
                    owner_module_id: ModuleId::try_new(MODULE_ID).map_err(config_error)?,
                    record_type: relationship_record_type()?,
                    page_size: MAXIMUM_PAGE_SIZE,
                    sort: RecordQuerySort::UpdatedAtDescending,
                    after: after.clone(),
                })
                .await?;
            *scanned = scanned.saturating_add(page.records.len());
            enforce_scan_limit(*scanned)?;
            for snapshot in &page.records {
                let relationship = party_relationship_from_snapshot(snapshot)?;
                if relationship_matches_filters(&relationship, filters)
                    && self
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct RelationshipFilters {
    party_id: Option<String>,
    relationship_type_code: Option<String>,
    directionality: Option<RelationshipDirectionality>,
    status: Option<PartyRelationshipStatus>,
}

impl RelationshipFilters {
    fn from_wire(command: &wire::ListPartyRelationshipsRequest) -> Result<Self, SdkError> {
        let party_id = command
            .party_ref
            .as_ref()
            .map(|value| validate_party_id(&value.party_id).map(|_| value.party_id.clone()))
            .transpose()?;
        let relationship_type_code = command
            .relationship_type_code
            .as_deref()
            .map(validate_type_code_filter)
            .transpose()?;
        let directionality = command
            .directionality
            .map(directionality_from_wire)
            .transpose()?;
        let status = command.status.map(status_from_wire).transpose()?;
        Ok(Self {
            party_id,
            relationship_type_code,
            directionality,
            status,
        })
    }
}

fn validate_list(
    adapter: &PartyRelationshipQueryAdapter,
    request: &QueryRequest,
    command: &wire::ListPartyRelationshipsRequest,
) -> Result<(), SdkError> {
    let _ = RelationshipFilters::from_wire(command)?;
    match wire::PartyRelationshipSort::try_from(command.sort).ok() {
        Some(
            wire::PartyRelationshipSort::Unspecified
            | wire::PartyRelationshipSort::UpdatedAtDescending,
        ) => {}
        None => {
            return Err(SdkError::invalid_argument(
                "party_relationship.sort",
                "Party Relationship sort is invalid",
            ));
        }
    }

    let page_size = resolve_page_size(adapter.page_policy, command.page.as_ref())?;
    let binding = cursor_binding(
        request,
        relationship_record_type()?,
        relationship_filter_hash(command),
        RecordQuerySort::UpdatedAtDescending,
        page_size,
    );
    let _ = decode_after(adapter, command.page.as_ref(), &binding)?;
    Ok(())
}

fn relationship_matches_filters(
    relationship: &PartyRelationship,
    filters: &RelationshipFilters,
) -> bool {
    if let Some(party_id) = &filters.party_id
        && relationship.from_party_ref().as_str() != party_id
        && relationship.to_party_ref().as_str() != party_id
    {
        return false;
    }
    if let Some(code) = &filters.relationship_type_code
        && relationship.relationship_type().code() != code
    {
        return false;
    }
    if filters
        .directionality
        .is_some_and(|value| value != relationship.relationship_type().directionality())
    {
        return false;
    }
    if filters
        .status
        .is_some_and(|value| value != relationship.status())
    {
        return false;
    }
    true
}

fn party_relationship_to_wire_with_visibility(
    relationship: &PartyRelationship,
    visibility: &QueryVisibilityDecision,
) -> wire::PartyRelationship {
    let mut output = party_relationship_to_wire(relationship);
    if !visibility.allows_field("from_party_ref") {
        output.from_party_ref = None;
    }
    if !visibility.allows_field("to_party_ref") {
        output.to_party_ref = None;
    }
    if !visibility.allows_field("relationship_type") {
        output.relationship_type = None;
    }
    if !visibility.allows_field("status") {
        output.status = wire::PartyRelationshipStatus::Unspecified as i32;
    }
    if !visibility.allows_field("validity") {
        output.valid_from = None;
        output.valid_until = None;
    }
    output
}

fn resolve_page_size(
    policy: PageSizePolicy,
    page: Option<&core::PageRequest>,
) -> Result<u32, SdkError> {
    policy
        .resolve(page.map_or(0, |value| value.page_size))
        .map_err(cursor_error)
}

fn cursor_binding(
    request: &QueryRequest,
    resource_type: RecordType,
    filter_hash: [u8; 32],
    sort: RecordQuerySort,
    page_size: u32,
) -> CursorBinding {
    CursorBinding {
        tenant_id: request.context.tenant_id.clone(),
        actor_id: Some(request.context.actor_id.clone()),
        capability_id: request.context.capability_id.clone(),
        capability_version: request.context.capability_version.clone(),
        resource_type,
        normalized_filter_hash: filter_hash,
        sort_id: sort.id().to_owned(),
        page_size,
    }
}

fn decode_after(
    adapter: &PartyRelationshipQueryAdapter,
    page: Option<&core::PageRequest>,
    binding: &CursorBinding,
) -> Result<Option<RecordQueryContinuation>, SdkError> {
    let token = page.map(|value| value.page_token.as_str()).unwrap_or("");
    if token.is_empty() {
        return Ok(None);
    }
    let continuation = adapter
        .cursor_codec
        .decode(token, binding)
        .map_err(cursor_error)?;
    let sort_value = String::from_utf8(continuation.sort_key).map_err(|_| {
        SdkError::new(
            "PARTY_RELATIONSHIPS_QUERY_CURSOR_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "The Party Relationship page cursor is invalid.",
        )
    })?;
    let after = RecordQueryContinuation {
        sort_value,
        record_id: continuation.record_id,
    };
    after.validate()?;
    Ok(Some(after))
}

fn encode_next(
    adapter: &PartyRelationshipQueryAdapter,
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

fn relationship_filter_hash(command: &wire::ListPartyRelationshipsRequest) -> [u8; 32] {
    let party_id = command
        .party_ref
        .as_ref()
        .map(|value| value.party_id.as_str())
        .unwrap_or("");
    let type_code = command.relationship_type_code.as_deref().unwrap_or("");
    let directionality = command.directionality.unwrap_or_default().to_be_bytes();
    let status = command.status.unwrap_or_default().to_be_bytes();
    normalized_filter_hash([
        ("party_ref", party_id.as_bytes()),
        ("relationship_type_code", type_code.as_bytes()),
        ("directionality", directionality.as_slice()),
        ("status", status.as_slice()),
    ])
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
            "PARTY_RELATIONSHIPS_QUERY_INPUT_CONTRACT_MISMATCH",
            ErrorCategory::InvalidArgument,
            false,
            "The Party Relationship query input does not match the required contract.",
        ));
    }
    M::decode(payload.bytes.as_slice()).map_err(|_| {
        SdkError::new(
            "PARTY_RELATIONSHIPS_QUERY_INPUT_PROTOBUF_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "The Party Relationship query input is not valid Protobuf.",
        )
    })
}

fn validate_record_id(value: &str) -> Result<RecordId, SdkError> {
    RecordId::try_new(value.to_owned()).map_err(|error| {
        SdkError::invalid_argument(
            "party_relationship.party_relationship_ref.party_relationship_id",
            error.to_string(),
        )
    })
}

fn validate_party_id(value: &str) -> Result<RecordId, SdkError> {
    RecordId::try_new(value.to_owned()).map_err(|error| {
        SdkError::invalid_argument("party_relationship.party_ref.party_id", error.to_string())
    })
}

fn validate_type_code_filter(value: &str) -> Result<String, SdkError> {
    if value.is_empty()
        || value.len() > 96
        || value.chars().any(char::is_control)
        || !value
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        || !value
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_alphanumeric)
        || !value.as_bytes().iter().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(*byte, b'.' | b'-' | b'_')
        })
    {
        return Err(SdkError::invalid_argument(
            "party_relationship.relationship_type_code",
            "Party Relationship type-code filter must use canonical lowercase ASCII syntax",
        ));
    }
    Ok(value.to_owned())
}

fn directionality_from_wire(value: i32) -> Result<RelationshipDirectionality, SdkError> {
    match wire::PartyRelationshipDirectionality::try_from(value) {
        Ok(wire::PartyRelationshipDirectionality::Directional) => {
            Ok(RelationshipDirectionality::Directional)
        }
        Ok(wire::PartyRelationshipDirectionality::Reciprocal) => {
            Ok(RelationshipDirectionality::Reciprocal)
        }
        Ok(wire::PartyRelationshipDirectionality::Unspecified) | Err(_) => {
            Err(SdkError::invalid_argument(
                "party_relationship.directionality",
                "Party Relationship directionality filter must be DIRECTIONAL or RECIPROCAL",
            ))
        }
    }
}

fn status_from_wire(value: i32) -> Result<PartyRelationshipStatus, SdkError> {
    match wire::PartyRelationshipStatus::try_from(value) {
        Ok(wire::PartyRelationshipStatus::Active) => Ok(PartyRelationshipStatus::Active),
        Ok(wire::PartyRelationshipStatus::Inactive) => Ok(PartyRelationshipStatus::Inactive),
        Ok(wire::PartyRelationshipStatus::Unspecified) | Err(_) => Err(SdkError::invalid_argument(
            "party_relationship.status",
            "Party Relationship status filter must be ACTIVE or INACTIVE",
        )),
    }
}

fn relationship_record_type() -> Result<RecordType, SdkError> {
    RecordType::try_new(RECORD_TYPE).map_err(config_error)
}

fn enforce_scan_limit(scanned: usize) -> Result<(), SdkError> {
    if scanned > MAXIMUM_VISIBILITY_SCAN_RECORDS {
        Err(scan_limit_error())
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

fn unsupported_query() -> SdkError {
    SdkError::new(
        "PARTY_RELATIONSHIPS_QUERY_CAPABILITY_UNSUPPORTED",
        ErrorCategory::Internal,
        false,
        "The Party Relationship query capability is not configured.",
    )
}

fn cursor_error(error: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "PARTY_RELATIONSHIPS_QUERY_CURSOR_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "The Party Relationship page cursor is invalid.",
    )
    .with_internal_reference(error.to_string())
}

fn scan_limit_error() -> SdkError {
    SdkError::new(
        "PARTY_RELATIONSHIPS_QUERY_VISIBILITY_SCAN_LIMIT_EXCEEDED",
        ErrorCategory::Unavailable,
        true,
        "The Party Relationship list is temporarily unavailable.",
    )
}

fn config_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    SdkError::new(
        "PARTY_RELATIONSHIPS_QUERY_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Party Relationship query adapter is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_party_relationships::{
        CreatePartyRelationship, PartyReference, PartyRelationshipId, RelationshipType,
    };
    use std::collections::BTreeSet;

    fn relationship() -> PartyRelationship {
        PartyRelationship::create(CreatePartyRelationship {
            party_relationship_id: PartyRelationshipId::try_new("relationship-visible-1").unwrap(),
            from_party_ref: PartyReference::try_new("party-acme").unwrap(),
            to_party_ref: PartyReference::try_new("party-ada").unwrap(),
            relationship_type: RelationshipType::employment(),
            valid_from_unix_nanos: Some(10),
            valid_until_unix_nanos: Some(1_000),
            occurred_at_unix_nanos: 10,
        })
        .unwrap()
    }

    #[test]
    fn publishes_get_and_list_as_personal_read_only_queries() {
        let definitions = query_capability_definitions().unwrap();
        assert_eq!(definitions.len(), 2);
        assert_eq!(
            definitions
                .iter()
                .map(|definition| definition.capability_id.as_str())
                .collect::<Vec<_>>(),
            QUERY_CAPABILITY_IDS
        );
        assert!(definitions.iter().all(|definition| !definition.mutation));
        assert!(definitions.iter().all(|definition| {
            !definition.requires_idempotency
                && definition.input_contract.allowed_data_classes == vec![DataClass::Personal]
        }));
    }

    #[test]
    fn type_code_filter_accepts_reserved_reciprocal_codes_without_fabricating_semantics() {
        assert_eq!(validate_type_code_filter("household").unwrap(), "household");
        assert!(validate_type_code_filter(" Household ").is_err());
        assert!(validate_type_code_filter("bad type!").is_err());
    }

    #[test]
    fn field_visibility_redacts_relationship_data_without_hiding_resource_identity() {
        let value = relationship();
        let decision = QueryVisibilityDecision {
            resource_visible: true,
            allowed_fields: BTreeSet::from(["status".to_owned()]),
            decision_id: "decision-1".to_owned(),
            policy_version: "policy-1".to_owned(),
        };

        let output = party_relationship_to_wire_with_visibility(&value, &decision);
        assert_eq!(
            output.party_relationship_ref.unwrap().party_relationship_id,
            "relationship-visible-1"
        );
        assert!(output.from_party_ref.is_none());
        assert!(output.to_party_ref.is_none());
        assert!(output.relationship_type.is_none());
        assert_eq!(output.status, wire::PartyRelationshipStatus::Active as i32);
        assert!(output.valid_from.is_none());
        assert!(output.valid_until.is_none());
        assert_eq!(output.resource_version.unwrap().version, 1);
    }

    #[test]
    fn endpoint_type_directionality_and_status_filters_are_exact_and_cursor_bound() {
        let value = relationship();
        let filters = RelationshipFilters {
            party_id: Some("party-ada".to_owned()),
            relationship_type_code: Some("employment".to_owned()),
            directionality: Some(RelationshipDirectionality::Directional),
            status: Some(PartyRelationshipStatus::Active),
        };
        assert!(relationship_matches_filters(&value, &filters));

        let request = wire::ListPartyRelationshipsRequest {
            page: None,
            party_ref: Some(customer::PartyRef {
                party_id: "party-ada".to_owned(),
            }),
            relationship_type_code: Some("employment".to_owned()),
            directionality: Some(wire::PartyRelationshipDirectionality::Directional as i32),
            status: Some(wire::PartyRelationshipStatus::Active as i32),
            sort: wire::PartyRelationshipSort::UpdatedAtDescending as i32,
        };
        let different = wire::ListPartyRelationshipsRequest {
            status: Some(wire::PartyRelationshipStatus::Inactive as i32),
            ..request.clone()
        };
        assert_ne!(
            relationship_filter_hash(&request),
            relationship_filter_hash(&different)
        );
    }
}
