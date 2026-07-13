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
use crm_parties::{Party, PartyKind};
use crm_parties_capability_adapter::{MODULE_ID, RECORD_TYPE, party_from_snapshot, party_to_wire};
use crm_proto_contracts::crm::{core::v1 as core, parties::v1 as wire};
use crm_query_runtime::{
    CursorBinding, CursorCodec, CursorContinuation, PageSizePolicy, QueryExecutionResult,
    QueryExecutor, QueryRequest, QuerySemanticValidator, QueryVisibilityAuthorizer,
    QueryVisibilityDecision, normalized_filter_hash,
};
use prost::Message;
use std::sync::Arc;

pub const GET_CAPABILITY: &str = "parties.party.get";
pub const LIST_CAPABILITY: &str = "parties.party.list";
pub const GET_REQUEST_SCHEMA: &str = "crm.parties.v1.GetPartyRequest";
pub const GET_RESPONSE_SCHEMA: &str = "crm.parties.v1.GetPartyResponse";
pub const LIST_REQUEST_SCHEMA: &str = "crm.parties.v1.ListPartiesRequest";
pub const LIST_RESPONSE_SCHEMA: &str = "crm.parties.v1.ListPartiesResponse";
pub const PARTY_QUERY_CAPABILITY_IDS: [&str; 2] = [GET_CAPABILITY, LIST_CAPABILITY];

const DEFAULT_PAGE_SIZE: u32 = 50;
const MAXIMUM_PAGE_SIZE: u32 = 200;
const MAXIMUM_VISIBILITY_SCAN_RECORDS: usize = 10_000;

#[derive(Clone)]
pub struct PartyQueryAdapter {
    store: PostgresDataStore,
    cursor_codec: CursorCodec,
    visibility: Arc<dyn QueryVisibilityAuthorizer>,
    page_policy: PageSizePolicy,
}

impl std::fmt::Debug for PartyQueryAdapter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PartyQueryAdapter")
            .field("store", &self.store)
            .field("cursor_codec", &self.cursor_codec)
            .field("visibility", &"dyn QueryVisibilityAuthorizer")
            .field("page_policy", &self.page_policy)
            .finish()
    }
}

impl PartyQueryAdapter {
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
    PARTY_QUERY_CAPABILITY_IDS
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

impl QuerySemanticValidator for PartyQueryAdapter {
    fn validate<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a QueryRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            match definition.capability_id.as_str() {
                GET_CAPABILITY => {
                    let command: wire::GetPartyRequest = decode_input(request, GET_REQUEST_SCHEMA)?;
                    let party_ref = command.party_ref.ok_or_else(|| {
                        SdkError::invalid_argument("party.party_ref", "Party reference is required")
                    })?;
                    validate_record_id(&party_ref.party_id)?;
                }
                LIST_CAPABILITY => {
                    let command: wire::ListPartiesRequest =
                        decode_input(request, LIST_REQUEST_SCHEMA)?;
                    validate_list(self, request, &command)?;
                }
                _ => return Err(unsupported_query()),
            }
            Ok(())
        })
    }
}

impl QueryExecutor for PartyQueryAdapter {
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

impl PartyQueryAdapter {
    async fn execute_get(&self, request: &QueryRequest) -> Result<TypedPayload, SdkError> {
        let command: wire::GetPartyRequest = decode_input(request, GET_REQUEST_SCHEMA)?;
        let party_ref = command.party_ref.ok_or_else(|| {
            SdkError::invalid_argument("party.party_ref", "Party reference is required")
        })?;
        let record_id = validate_record_id(&party_ref.party_id)?;
        let snapshot = self
            .store
            .get_record_for_query(&RecordGetQuery {
                tenant_id: request.context.tenant_id.clone(),
                owner_module_id: ModuleId::try_new(MODULE_ID).map_err(config_error)?,
                record_type: party_record_type()?,
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
        let party = party_from_snapshot(&snapshot)?;

        support::protobuf_payload(
            MODULE_ID,
            GET_RESPONSE_SCHEMA,
            DataClass::Personal,
            &wire::GetPartyResponse {
                party: Some(party_to_wire_with_visibility(&party, &visibility)),
            },
        )
    }

    async fn execute_list(&self, request: &QueryRequest) -> Result<TypedPayload, SdkError> {
        let command: wire::ListPartiesRequest = decode_input(request, LIST_REQUEST_SCHEMA)?;
        let page_size = resolve_page_size(self.page_policy, command.page.as_ref())?;
        let filter_hash = party_filter_hash(&command);
        let binding = cursor_binding(
            request,
            party_record_type()?,
            filter_hash,
            RecordQuerySort::UpdatedAtDescending,
            page_size,
        );
        let after = decode_after(self, command.page.as_ref(), &binding)?;
        let (parties, next) = self
            .collect_parties(request, page_size, after, command.kind)
            .await?;
        let next_page_token = encode_next(self, &binding, next.as_ref())?;

        support::protobuf_payload(
            MODULE_ID,
            LIST_RESPONSE_SCHEMA,
            DataClass::Personal,
            &wire::ListPartiesResponse {
                parties,
                page: Some(core::PageInfo {
                    next_page_token,
                    total_size: 0,
                }),
            },
        )
    }

    async fn collect_parties(
        &self,
        request: &QueryRequest,
        page_size: u32,
        mut after: Option<RecordQueryContinuation>,
        kind: Option<i32>,
    ) -> Result<(Vec<wire::Party>, Option<RecordQueryContinuation>), SdkError> {
        let mut output = Vec::with_capacity(page_size as usize);
        let mut scanned = 0_usize;
        loop {
            let remaining = page_size as usize - output.len();
            if remaining == 0 {
                let anchor = after.clone();
                let has_more = self
                    .has_more_visible_party(request, anchor.clone(), kind, &mut scanned)
                    .await?;
                return Ok((output, has_more.then_some(anchor).flatten()));
            }

            let page = self
                .store
                .list_records_for_query(&RecordListQuery {
                    tenant_id: request.context.tenant_id.clone(),
                    owner_module_id: ModuleId::try_new(MODULE_ID).map_err(config_error)?,
                    record_type: party_record_type()?,
                    page_size: u32::try_from(remaining).map_err(|_| scan_limit_error())?,
                    sort: RecordQuerySort::UpdatedAtDescending,
                    after: after.clone(),
                })
                .await?;
            scanned = scanned.saturating_add(page.records.len());
            enforce_scan_limit(scanned)?;
            for snapshot in &page.records {
                let party = party_from_snapshot(snapshot)?;
                if !party_matches_kind(&party, kind) {
                    continue;
                }
                let visibility = self
                    .visibility
                    .authorize_visibility(request, &snapshot.reference)
                    .await?;
                if visibility.resource_visible {
                    output.push(party_to_wire_with_visibility(&party, &visibility));
                }
            }
            after = page.next;
            if after.is_none() {
                return Ok((output, None));
            }
        }
    }

    async fn has_more_visible_party(
        &self,
        request: &QueryRequest,
        mut after: Option<RecordQueryContinuation>,
        kind: Option<i32>,
        scanned: &mut usize,
    ) -> Result<bool, SdkError> {
        while after.is_some() {
            let page = self
                .store
                .list_records_for_query(&RecordListQuery {
                    tenant_id: request.context.tenant_id.clone(),
                    owner_module_id: ModuleId::try_new(MODULE_ID).map_err(config_error)?,
                    record_type: party_record_type()?,
                    page_size: MAXIMUM_PAGE_SIZE,
                    sort: RecordQuerySort::UpdatedAtDescending,
                    after: after.clone(),
                })
                .await?;
            *scanned = scanned.saturating_add(page.records.len());
            enforce_scan_limit(*scanned)?;
            for snapshot in &page.records {
                let party = party_from_snapshot(snapshot)?;
                if party_matches_kind(&party, kind)
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

fn validate_list(
    adapter: &PartyQueryAdapter,
    request: &QueryRequest,
    command: &wire::ListPartiesRequest,
) -> Result<(), SdkError> {
    if let Some(kind) = command.kind {
        match wire::PartyKind::try_from(kind).ok() {
            Some(wire::PartyKind::Person | wire::PartyKind::Organization) => {}
            _ => {
                return Err(SdkError::invalid_argument(
                    "party.kind",
                    "Party kind filter must be PERSON or ORGANIZATION",
                ));
            }
        }
    }
    match wire::PartySort::try_from(command.sort).ok() {
        Some(wire::PartySort::Unspecified | wire::PartySort::UpdatedAtDescending) => {}
        None => {
            return Err(SdkError::invalid_argument(
                "party.sort",
                "Party sort is invalid",
            ));
        }
    }

    let page_size = resolve_page_size(adapter.page_policy, command.page.as_ref())?;
    let binding = cursor_binding(
        request,
        party_record_type()?,
        party_filter_hash(command),
        RecordQuerySort::UpdatedAtDescending,
        page_size,
    );
    let _ = decode_after(adapter, command.page.as_ref(), &binding)?;
    Ok(())
}

fn party_matches_kind(party: &Party, kind: Option<i32>) -> bool {
    match kind.and_then(|value| wire::PartyKind::try_from(value).ok()) {
        None => true,
        Some(wire::PartyKind::Person) => party.kind() == PartyKind::Person,
        Some(wire::PartyKind::Organization) => party.kind() == PartyKind::Organization,
        Some(wire::PartyKind::Unspecified) => false,
    }
}

fn party_to_wire_with_visibility(
    party: &Party,
    visibility: &QueryVisibilityDecision,
) -> wire::Party {
    let mut output = party_to_wire(party);
    if !visibility.allows_field("kind") {
        output.kind = wire::PartyKind::Unspecified as i32;
    }
    if !visibility.allows_field("display_name") {
        output.display_name.clear();
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
    adapter: &PartyQueryAdapter,
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
            "PARTIES_QUERY_CURSOR_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "The Party page cursor is invalid.",
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
    adapter: &PartyQueryAdapter,
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

fn party_filter_hash(command: &wire::ListPartiesRequest) -> [u8; 32] {
    let kind = command.kind.unwrap_or_default().to_be_bytes();
    normalized_filter_hash([("kind", kind.as_slice())])
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
            "PARTIES_QUERY_INPUT_CONTRACT_MISMATCH",
            ErrorCategory::InvalidArgument,
            false,
            "The Party query input does not match the required contract.",
        ));
    }
    M::decode(payload.bytes.as_slice()).map_err(|_| {
        SdkError::new(
            "PARTIES_QUERY_INPUT_PROTOBUF_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "The Party query input is not valid Protobuf.",
        )
    })
}

fn validate_record_id(value: &str) -> Result<RecordId, SdkError> {
    RecordId::try_new(value.to_owned())
        .map_err(|error| SdkError::invalid_argument("party.party_ref.party_id", error.to_string()))
}

fn party_record_type() -> Result<RecordType, SdkError> {
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
        "PARTIES_QUERY_CAPABILITY_UNSUPPORTED",
        ErrorCategory::Internal,
        false,
        "The Party query capability is not configured.",
    )
}

fn cursor_error(error: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "PARTIES_QUERY_CURSOR_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "The Party page cursor is invalid.",
    )
    .with_internal_reference(error.to_string())
}

fn scan_limit_error() -> SdkError {
    SdkError::new(
        "PARTIES_QUERY_VISIBILITY_SCAN_LIMIT_EXCEEDED",
        ErrorCategory::Unavailable,
        true,
        "The Party list is temporarily unavailable.",
    )
}

fn config_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    SdkError::new(
        "PARTIES_QUERY_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Party query adapter is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_parties::{CreateParty, PartyId};
    use std::collections::BTreeSet;

    #[test]
    fn publishes_get_and_list_as_personal_read_only_queries() {
        let definitions = query_capability_definitions().unwrap();
        assert_eq!(definitions.len(), 2);
        assert_eq!(
            definitions
                .iter()
                .map(|definition| definition.capability_id.as_str())
                .collect::<Vec<_>>(),
            PARTY_QUERY_CAPABILITY_IDS
        );
        assert!(definitions.iter().all(|definition| !definition.mutation));
        assert!(definitions.iter().all(|definition| {
            !definition.requires_idempotency
                && definition.input_contract.allowed_data_classes == vec![DataClass::Personal]
        }));
    }

    #[test]
    fn field_visibility_redacts_personal_fields_without_hiding_resource_identity() {
        let party = Party::create(CreateParty {
            party_id: PartyId::try_new("party-visible-1").unwrap(),
            kind: PartyKind::Person,
            display_name: "Ada Lovelace".to_owned(),
            occurred_at_unix_nanos: 10,
        })
        .unwrap();
        let decision = QueryVisibilityDecision {
            resource_visible: true,
            allowed_fields: BTreeSet::from(["display_name".to_owned()]),
            decision_id: "decision-1".to_owned(),
            policy_version: "policy-1".to_owned(),
        };

        let output = party_to_wire_with_visibility(&party, &decision);
        assert_eq!(output.party_ref.unwrap().party_id, "party-visible-1");
        assert_eq!(output.display_name, "Ada Lovelace");
        assert_eq!(output.kind, wire::PartyKind::Unspecified as i32);
        assert_eq!(output.resource_version.unwrap().version, 1);
    }

    #[test]
    fn kind_filter_is_exact_and_cursor_bound() {
        let person = Party::create(CreateParty {
            party_id: PartyId::try_new("party-person-1").unwrap(),
            kind: PartyKind::Person,
            display_name: "Ada Lovelace".to_owned(),
            occurred_at_unix_nanos: 10,
        })
        .unwrap();
        assert!(party_matches_kind(
            &person,
            Some(wire::PartyKind::Person as i32)
        ));
        assert!(!party_matches_kind(
            &person,
            Some(wire::PartyKind::Organization as i32)
        ));

        let person_request = wire::ListPartiesRequest {
            page: None,
            kind: Some(wire::PartyKind::Person as i32),
            sort: wire::PartySort::UpdatedAtDescending as i32,
        };
        let organization_request = wire::ListPartiesRequest {
            kind: Some(wire::PartyKind::Organization as i32),
            ..person_request.clone()
        };
        assert_ne!(
            party_filter_hash(&person_request),
            party_filter_hash(&organization_request)
        );
    }
}
