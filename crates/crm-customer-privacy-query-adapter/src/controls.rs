use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRisk};
use crm_core_data::{
    PostgresDataStore, RecordGetQuery, RecordListQuery, RecordQueryContinuation, RecordQuerySort,
};
use crm_customer_privacy::{
    CustomerDataLegalHold, LEGAL_HOLD_RECORD_TYPE, LegalHoldScope, LegalHoldStatus, MODULE_ID,
    ProcessingRestriction, RESTRICTION_RECORD_TYPE, RestrictionScope, RestrictionStatus,
};
use crm_customer_privacy_persistence_adapter::{
    legal_hold_from_snapshot, processing_restriction_from_snapshot,
};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, PayloadEncoding,
    PortFuture, RecordId, RecordRef, RecordType, SdkError, TypedPayload,
};
use crm_proto_contracts::crm::{customer::v1 as customer, customer_privacy::v1 as wire};
use crm_query_runtime::{
    CursorBinding, CursorCodec, CursorContinuation, QueryExecutionResult, QueryExecutor,
    QueryRequest, QuerySemanticValidator, QueryVisibilityAuthorizer, normalized_filter_hash,
};
use prost::Message;
use std::collections::BTreeSet;
use std::sync::Arc;

pub const GET_PROCESSING_RESTRICTION_CAPABILITY: &str = "customer_privacy.restriction.get";
pub const GET_PROCESSING_RESTRICTION_REQUEST_SCHEMA: &str =
    "crm.customer_privacy.v1.GetProcessingRestrictionRequest";
pub const GET_PROCESSING_RESTRICTION_RESPONSE_SCHEMA: &str =
    "crm.customer_privacy.v1.GetProcessingRestrictionResponse";
pub const GET_CUSTOMER_DATA_LEGAL_HOLD_CAPABILITY: &str = "customer_privacy.legal_hold.get";
pub const GET_CUSTOMER_DATA_LEGAL_HOLD_REQUEST_SCHEMA: &str =
    "crm.customer_privacy.v1.GetCustomerDataLegalHoldRequest";
pub const GET_CUSTOMER_DATA_LEGAL_HOLD_RESPONSE_SCHEMA: &str =
    "crm.customer_privacy.v1.GetCustomerDataLegalHoldResponse";
pub const LIST_CUSTOMER_DATA_LEGAL_HOLDS_BY_SUBJECT_CAPABILITY: &str =
    "customer_privacy.legal_hold.list_by_subject";
pub const LIST_CUSTOMER_DATA_LEGAL_HOLDS_BY_SUBJECT_REQUEST_SCHEMA: &str =
    "crm.customer_privacy.v1.ListCustomerDataLegalHoldsBySubjectRequest";
pub const LIST_CUSTOMER_DATA_LEGAL_HOLDS_BY_SUBJECT_RESPONSE_SCHEMA: &str =
    "crm.customer_privacy.v1.ListCustomerDataLegalHoldsBySubjectResponse";
pub const CONTROL_QUERY_CAPABILITY_IDS: &[&str] = &[
    GET_PROCESSING_RESTRICTION_CAPABILITY,
    GET_CUSTOMER_DATA_LEGAL_HOLD_CAPABILITY,
    LIST_CUSTOMER_DATA_LEGAL_HOLDS_BY_SUBJECT_CAPABILITY,
];
pub const PARTY_RECORD_TYPE: &str = "parties.party";

const DEFAULT_PAGE_SIZE: u32 = 50;
const MAXIMUM_PAGE_SIZE: u32 = 100;
const INTERNAL_SCAN_PAGE_SIZE: u32 = 100;
const MAXIMUM_VISIBILITY_SCAN_RECORDS: usize = 4_096;
const NANOS_PER_MILLISECOND: i64 = 1_000_000;

const RESTRICTION_FIELDS: &[&str] = &[
    "canonical_party_ref",
    "scope",
    "status",
    "version",
    "policy_version",
    "placed_by_actor_id",
    "placed_at_unix_ms",
    "effective_from_unix_ms",
    "expires_at_unix_ms",
    "released_by_actor_id",
    "released_at_unix_ms",
];
const LEGAL_HOLD_FIELDS: &[&str] = &[
    "canonical_party_ref",
    "scope",
    "authority_reference_id",
    "reason_code",
    "policy_version",
    "status",
    "version",
    "placed_by_actor_id",
    "effective_from_unix_ms",
    "effective_until_unix_ms",
    "released_by_actor_id",
    "released_at_unix_ms",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlVisibilityResource {
    pub owner_module_id: &'static str,
    pub resource_type: &'static str,
    pub allowed_fields: BTreeSet<String>,
}

pub fn control_query_visibility_resources(capability_id: &str) -> Vec<ControlVisibilityResource> {
    let fields = match capability_id {
        GET_PROCESSING_RESTRICTION_CAPABILITY => {
            Some((RESTRICTION_RECORD_TYPE, RESTRICTION_FIELDS))
        }
        GET_CUSTOMER_DATA_LEGAL_HOLD_CAPABILITY
        | LIST_CUSTOMER_DATA_LEGAL_HOLDS_BY_SUBJECT_CAPABILITY => {
            Some((LEGAL_HOLD_RECORD_TYPE, LEGAL_HOLD_FIELDS))
        }
        _ => None,
    };
    let Some((resource_type, fields)) = fields else {
        return Vec::new();
    };
    vec![
        ControlVisibilityResource {
            owner_module_id: MODULE_ID,
            resource_type: PARTY_RECORD_TYPE,
            allowed_fields: BTreeSet::new(),
        },
        ControlVisibilityResource {
            owner_module_id: MODULE_ID,
            resource_type,
            allowed_fields: fields.iter().copied().map(str::to_owned).collect(),
        },
    ]
}

#[derive(Clone)]
pub struct CustomerPrivacyControlQueryAdapter {
    store: PostgresDataStore,
    visibility: Arc<dyn QueryVisibilityAuthorizer>,
    cursor_codec: Option<CursorCodec>,
}

impl CustomerPrivacyControlQueryAdapter {
    pub fn new(store: PostgresDataStore, visibility: Arc<dyn QueryVisibilityAuthorizer>) -> Self {
        Self {
            store,
            visibility,
            cursor_codec: None,
        }
    }

    pub fn new_with_cursor(
        store: PostgresDataStore,
        cursor_codec: CursorCodec,
        visibility: Arc<dyn QueryVisibilityAuthorizer>,
    ) -> Self {
        Self {
            store,
            visibility,
            cursor_codec: Some(cursor_codec),
        }
    }

    fn cursor_codec(&self) -> Result<&CursorCodec, SdkError> {
        self.cursor_codec
            .as_ref()
            .ok_or_else(|| configuration_invalid("legal-hold list cursor codec is not configured"))
    }

    async fn get_restriction(&self, request: &QueryRequest) -> Result<TypedPayload, SdkError> {
        let command: wire::GetProcessingRestrictionRequest =
            decode_input(request, GET_PROCESSING_RESTRICTION_REQUEST_SCHEMA)?;
        let reference = processing_restriction_ref(command.processing_restriction_ref)?;
        let snapshot = self
            .store
            .get_record_for_query(&RecordGetQuery {
                tenant_id: request.context.tenant_id.clone(),
                owner_module_id: module_id()?,
                record_type: record_type(RESTRICTION_RECORD_TYPE)?,
                record_id: reference.record_id.clone(),
            })
            .await?
            .ok_or_else(control_not_found)?;
        let visibility = self
            .visibility
            .authorize_visibility(request, &snapshot.reference)
            .await?;
        if !visibility.resource_visible {
            return Err(control_not_found());
        }
        let restriction = rehydrate_restriction(request, &snapshot)?;
        require_subject_visible(
            self.visibility.as_ref(),
            request,
            restriction.canonical_party_id(),
        )
        .await?;
        let mut public = processing_restriction_to_wire(&restriction)?;
        redact_restriction(&mut public, |field| visibility.allows_field(field));
        support::protobuf_payload(
            MODULE_ID,
            GET_PROCESSING_RESTRICTION_RESPONSE_SCHEMA,
            DataClass::Personal,
            &wire::GetProcessingRestrictionResponse {
                processing_restriction: Some(public),
            },
        )
    }

    async fn get_legal_hold(&self, request: &QueryRequest) -> Result<TypedPayload, SdkError> {
        let command: wire::GetCustomerDataLegalHoldRequest =
            decode_input(request, GET_CUSTOMER_DATA_LEGAL_HOLD_REQUEST_SCHEMA)?;
        let reference = legal_hold_ref(command.customer_data_legal_hold_ref)?;
        let snapshot = self
            .store
            .get_record_for_query(&RecordGetQuery {
                tenant_id: request.context.tenant_id.clone(),
                owner_module_id: module_id()?,
                record_type: record_type(LEGAL_HOLD_RECORD_TYPE)?,
                record_id: reference.record_id.clone(),
            })
            .await?
            .ok_or_else(control_not_found)?;
        let visibility = self
            .visibility
            .authorize_visibility(request, &snapshot.reference)
            .await?;
        if !visibility.resource_visible {
            return Err(control_not_found());
        }
        let hold = rehydrate_hold(request, &snapshot)?;
        require_subject_visible(self.visibility.as_ref(), request, hold.canonical_party_id())
            .await?;
        let mut public = customer_data_legal_hold_to_wire(&hold)?;
        redact_legal_hold(&mut public, |field| visibility.allows_field(field));
        support::protobuf_payload(
            MODULE_ID,
            GET_CUSTOMER_DATA_LEGAL_HOLD_RESPONSE_SCHEMA,
            DataClass::Personal,
            &wire::GetCustomerDataLegalHoldResponse {
                customer_data_legal_hold: Some(public),
            },
        )
    }
}

impl std::fmt::Debug for CustomerPrivacyControlQueryAdapter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CustomerPrivacyControlQueryAdapter")
            .field("store", &self.store)
            .field("visibility", &"dyn QueryVisibilityAuthorizer")
            .field("cursor_codec_configured", &self.cursor_codec.is_some())
            .finish()
    }
}

impl QuerySemanticValidator for CustomerPrivacyControlQueryAdapter {
    fn validate<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a QueryRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            ensure_definition(definition)?;
            match definition.capability_id.as_str() {
                GET_PROCESSING_RESTRICTION_CAPABILITY => {
                    let command: wire::GetProcessingRestrictionRequest =
                        decode_input(request, GET_PROCESSING_RESTRICTION_REQUEST_SCHEMA)?;
                    processing_restriction_ref(command.processing_restriction_ref).map(|_| ())
                }
                GET_CUSTOMER_DATA_LEGAL_HOLD_CAPABILITY => {
                    let command: wire::GetCustomerDataLegalHoldRequest =
                        decode_input(request, GET_CUSTOMER_DATA_LEGAL_HOLD_REQUEST_SCHEMA)?;
                    legal_hold_ref(command.customer_data_legal_hold_ref).map(|_| ())
                }
                LIST_CUSTOMER_DATA_LEGAL_HOLDS_BY_SUBJECT_CAPABILITY => {
                    list_parameters(self, request).map(|_| ())
                }
                _ => Err(unsupported_query()),
            }
        })
    }
}

impl QueryExecutor for CustomerPrivacyControlQueryAdapter {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: QueryRequest,
    ) -> PortFuture<'a, Result<QueryExecutionResult, SdkError>> {
        Box::pin(async move {
            ensure_definition(definition)?;
            let output = match definition.capability_id.as_str() {
                GET_PROCESSING_RESTRICTION_CAPABILITY => self.get_restriction(&request).await?,
                GET_CUSTOMER_DATA_LEGAL_HOLD_CAPABILITY => self.get_legal_hold(&request).await?,
                LIST_CUSTOMER_DATA_LEGAL_HOLDS_BY_SUBJECT_CAPABILITY => {
                    list_legal_holds(self, &request).await?
                }
                _ => return Err(unsupported_query()),
            };
            Ok(QueryExecutionResult { output })
        })
    }
}

pub fn control_query_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    Ok(vec![
        query_definition(
            GET_PROCESSING_RESTRICTION_CAPABILITY,
            GET_PROCESSING_RESTRICTION_REQUEST_SCHEMA,
            GET_PROCESSING_RESTRICTION_RESPONSE_SCHEMA,
        )?,
        query_definition(
            GET_CUSTOMER_DATA_LEGAL_HOLD_CAPABILITY,
            GET_CUSTOMER_DATA_LEGAL_HOLD_REQUEST_SCHEMA,
            GET_CUSTOMER_DATA_LEGAL_HOLD_RESPONSE_SCHEMA,
        )?,
        query_definition(
            LIST_CUSTOMER_DATA_LEGAL_HOLDS_BY_SUBJECT_CAPABILITY,
            LIST_CUSTOMER_DATA_LEGAL_HOLDS_BY_SUBJECT_REQUEST_SCHEMA,
            LIST_CUSTOMER_DATA_LEGAL_HOLDS_BY_SUBJECT_RESPONSE_SCHEMA,
        )?,
    ])
}

fn query_definition(
    capability_id: &'static str,
    request_schema: &'static str,
    response_schema: &'static str,
) -> Result<CapabilityDefinition, SdkError> {
    Ok(CapabilityDefinition {
        capability_id: configured(CapabilityId::try_new(capability_id))?,
        capability_version: configured(CapabilityVersion::try_new(support::CONTRACT_VERSION))?,
        owner_module_id: configured(ModuleId::try_new(MODULE_ID))?,
        input_contract: support::protobuf_contract(
            MODULE_ID,
            request_schema,
            vec![DataClass::Personal],
        )?,
        output_contract: Some(support::protobuf_contract(
            MODULE_ID,
            response_schema,
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

#[derive(Debug)]
struct ListParameters {
    party_id: RecordId,
    status: Option<i32>,
    page_size: u32,
    binding: CursorBinding,
    after: Option<RecordQueryContinuation>,
}

async fn list_legal_holds(
    adapter: &CustomerPrivacyControlQueryAdapter,
    request: &QueryRequest,
) -> Result<TypedPayload, SdkError> {
    let parameters = list_parameters(adapter, request)?;
    let party_reference = support::record_ref(
        PARTY_RECORD_TYPE,
        parameters.party_id.as_str(),
        "customer_privacy.legal_hold.list.canonical_party_ref.party_id",
    )?;
    if !adapter
        .visibility
        .authorize_visibility(request, &party_reference)
        .await?
        .resource_visible
    {
        return list_response(Vec::new(), String::new());
    }
    let (holds, next) = collect_holds(adapter, request, &parameters).await?;
    let next_cursor = encode_next(adapter, &parameters.binding, next.as_ref())?;
    list_response(holds, next_cursor)
}

async fn collect_holds(
    adapter: &CustomerPrivacyControlQueryAdapter,
    request: &QueryRequest,
    parameters: &ListParameters,
) -> Result<
    (
        Vec<wire::CustomerDataLegalHold>,
        Option<RecordQueryContinuation>,
    ),
    SdkError,
> {
    let mut output = Vec::with_capacity(parameters.page_size as usize);
    let mut after = parameters.after.clone();
    let mut scanned = 0_usize;
    loop {
        let remaining = parameters.page_size as usize - output.len();
        if remaining == 0 {
            let anchor = after.clone();
            let more = has_more(adapter, request, parameters, anchor.clone(), &mut scanned).await?;
            return Ok((output, more.then_some(anchor).flatten()));
        }
        let page = adapter
            .store
            .list_records_for_query(&RecordListQuery {
                tenant_id: request.context.tenant_id.clone(),
                owner_module_id: module_id()?,
                record_type: record_type(LEGAL_HOLD_RECORD_TYPE)?,
                page_size: u32::try_from(remaining).map_err(configuration_invalid)?,
                sort: RecordQuerySort::UpdatedAtDescending,
                after: after.clone(),
            })
            .await?;
        scanned = scanned.saturating_add(page.records.len());
        enforce_scan_limit(scanned)?;
        for snapshot in &page.records {
            let hold = rehydrate_hold(request, snapshot)?;
            if hold.canonical_party_id() != &parameters.party_id
                || !status_matches(&hold, parameters.status)
            {
                continue;
            }
            let visibility = adapter
                .visibility
                .authorize_visibility(request, &snapshot.reference)
                .await?;
            if !visibility.resource_visible {
                continue;
            }
            let mut public = customer_data_legal_hold_to_wire(&hold)?;
            redact_legal_hold(&mut public, |field| visibility.allows_field(field));
            output.push(public);
        }
        after = page.next;
        if after.is_none() {
            return Ok((output, None));
        }
    }
}

async fn has_more(
    adapter: &CustomerPrivacyControlQueryAdapter,
    request: &QueryRequest,
    parameters: &ListParameters,
    mut after: Option<RecordQueryContinuation>,
    scanned: &mut usize,
) -> Result<bool, SdkError> {
    while after.is_some() {
        let page = adapter
            .store
            .list_records_for_query(&RecordListQuery {
                tenant_id: request.context.tenant_id.clone(),
                owner_module_id: module_id()?,
                record_type: record_type(LEGAL_HOLD_RECORD_TYPE)?,
                page_size: INTERNAL_SCAN_PAGE_SIZE,
                sort: RecordQuerySort::UpdatedAtDescending,
                after: after.clone(),
            })
            .await?;
        *scanned = scanned.saturating_add(page.records.len());
        enforce_scan_limit(*scanned)?;
        for snapshot in &page.records {
            let hold = rehydrate_hold(request, snapshot)?;
            if hold.canonical_party_id() != &parameters.party_id
                || !status_matches(&hold, parameters.status)
            {
                continue;
            }
            if adapter
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

fn list_parameters(
    adapter: &CustomerPrivacyControlQueryAdapter,
    request: &QueryRequest,
) -> Result<ListParameters, SdkError> {
    let command: wire::ListCustomerDataLegalHoldsBySubjectRequest = decode_input(
        request,
        LIST_CUSTOMER_DATA_LEGAL_HOLDS_BY_SUBJECT_REQUEST_SCHEMA,
    )?;
    let party_id = party_id(command.canonical_party_ref)?;
    let status = legal_hold_status_filter(command.status)?;
    let page_size = page_size(command.page_size)?;
    let binding = cursor_binding(request, &party_id, status, page_size)?;
    let after = decode_after(adapter, &command.cursor, &binding)?;
    Ok(ListParameters {
        party_id,
        status,
        page_size,
        binding,
        after,
    })
}

fn cursor_binding(
    request: &QueryRequest,
    party_id: &RecordId,
    status: Option<i32>,
    page_size: u32,
) -> Result<CursorBinding, SdkError> {
    let status = status
        .unwrap_or(wire::CustomerDataLegalHoldStatus::Unspecified as i32)
        .to_be_bytes();
    Ok(CursorBinding {
        tenant_id: request.context.tenant_id.clone(),
        actor_id: Some(request.context.actor_id.clone()),
        capability_id: request.context.capability_id.clone(),
        capability_version: request.context.capability_version.clone(),
        resource_type: record_type(LEGAL_HOLD_RECORD_TYPE)?,
        normalized_filter_hash: normalized_filter_hash([
            ("canonical_party_id", party_id.as_str().as_bytes()),
            ("status", status.as_slice()),
        ]),
        sort_id: RecordQuerySort::UpdatedAtDescending.id().to_owned(),
        page_size,
    })
}

fn decode_after(
    adapter: &CustomerPrivacyControlQueryAdapter,
    token: &str,
    binding: &CursorBinding,
) -> Result<Option<RecordQueryContinuation>, SdkError> {
    if token.is_empty() {
        return Ok(None);
    }
    let value = adapter
        .cursor_codec()?
        .decode(token, binding)
        .map_err(cursor_error)?;
    let after = RecordQueryContinuation {
        sort_value: String::from_utf8(value.sort_key).map_err(|_| cursor_invalid())?,
        record_id: value.record_id,
    };
    after.validate().map_err(cursor_error)?;
    Ok(Some(after))
}

fn encode_next(
    adapter: &CustomerPrivacyControlQueryAdapter,
    binding: &CursorBinding,
    next: Option<&RecordQueryContinuation>,
) -> Result<String, SdkError> {
    next.map(|value| {
        adapter
            .cursor_codec()?
            .encode(
                binding,
                &CursorContinuation {
                    sort_key: value.sort_value.as_bytes().to_vec(),
                    record_id: value.record_id.clone(),
                },
            )
            .map_err(cursor_error)
    })
    .transpose()
    .map(|value| value.unwrap_or_default())
}

fn list_response(
    holds: Vec<wire::CustomerDataLegalHold>,
    next_cursor: String,
) -> Result<TypedPayload, SdkError> {
    support::protobuf_payload(
        MODULE_ID,
        LIST_CUSTOMER_DATA_LEGAL_HOLDS_BY_SUBJECT_RESPONSE_SCHEMA,
        DataClass::Personal,
        &wire::ListCustomerDataLegalHoldsBySubjectResponse {
            customer_data_legal_holds: holds,
            next_cursor,
        },
    )
}

async fn require_subject_visible(
    visibility: &dyn QueryVisibilityAuthorizer,
    request: &QueryRequest,
    party_id: &RecordId,
) -> Result<(), SdkError> {
    let reference = support::record_ref(
        PARTY_RECORD_TYPE,
        party_id.as_str(),
        "customer_privacy.control.canonical_party_ref.party_id",
    )?;
    if !visibility
        .authorize_visibility(request, &reference)
        .await?
        .resource_visible
    {
        return Err(control_not_found());
    }
    Ok(())
}

fn rehydrate_restriction(
    request: &QueryRequest,
    snapshot: &crm_module_sdk::RecordSnapshot,
) -> Result<ProcessingRestriction, SdkError> {
    let restriction = processing_restriction_from_snapshot(snapshot)
        .map_err(|error| control_state_invalid(error.to_string()))?;
    if restriction.restriction_id() != &snapshot.reference.record_id
        || restriction.tenant_id() != &request.context.tenant_id
    {
        return Err(control_state_invalid(
            "processing restriction identity differs from persisted query snapshot",
        ));
    }
    Ok(restriction)
}

fn rehydrate_hold(
    request: &QueryRequest,
    snapshot: &crm_module_sdk::RecordSnapshot,
) -> Result<CustomerDataLegalHold, SdkError> {
    let hold = legal_hold_from_snapshot(snapshot)
        .map_err(|error| control_state_invalid(error.to_string()))?;
    if hold.hold_id() != &snapshot.reference.record_id
        || hold.tenant_id() != &request.context.tenant_id
    {
        return Err(control_state_invalid(
            "customer-data legal-hold identity differs from persisted query snapshot",
        ));
    }
    Ok(hold)
}

fn status_matches(hold: &CustomerDataLegalHold, status: Option<i32>) -> bool {
    status.is_none_or(|status| {
        status
            == match hold.status() {
                LegalHoldStatus::Active => wire::CustomerDataLegalHoldStatus::Active as i32,
                LegalHoldStatus::Released => wire::CustomerDataLegalHoldStatus::Released as i32,
            }
    })
}

pub fn processing_restriction_to_wire(
    restriction: &ProcessingRestriction,
) -> Result<wire::ProcessingRestriction, SdkError> {
    Ok(wire::ProcessingRestriction {
        processing_restriction_ref: Some(wire::ProcessingRestrictionRef {
            processing_restriction_id: restriction.restriction_id().as_str().to_owned(),
        }),
        canonical_party_ref: Some(customer::PartyRef {
            party_id: restriction.canonical_party_id().as_str().to_owned(),
        }),
        scope: match restriction.scope() {
            RestrictionScope::Processing => wire::ProcessingRestrictionScope::Processing as i32,
            RestrictionScope::Communication => {
                wire::ProcessingRestrictionScope::Communication as i32
            }
            RestrictionScope::ProcessingAndCommunication => {
                wire::ProcessingRestrictionScope::ProcessingAndCommunication as i32
            }
        },
        status: match restriction.status() {
            RestrictionStatus::Active => wire::ProcessingRestrictionStatus::Active as i32,
            RestrictionStatus::Released => wire::ProcessingRestrictionStatus::Released as i32,
            RestrictionStatus::Expired => wire::ProcessingRestrictionStatus::Expired as i32,
        },
        version: i64::try_from(restriction.version())
            .map_err(|_| control_state_invalid("processing restriction version exceeds i64"))?,
        policy_version: restriction.policy_version().as_str().to_owned(),
        placed_by_actor_id: restriction.placed_by().as_str().to_owned(),
        placed_at_unix_ms: nanos_to_millis(restriction.placed_at_unix_nanos())?,
        effective_from_unix_ms: nanos_to_millis(restriction.effective_from_unix_nanos())?,
        expires_at_unix_ms: restriction
            .expires_at_unix_nanos()
            .map(nanos_to_millis)
            .transpose()?,
        released_by_actor_id: restriction
            .released_by()
            .map(|value| value.as_str().to_owned()),
        released_at_unix_ms: restriction
            .released_at_unix_nanos()
            .map(nanos_to_millis)
            .transpose()?,
    })
}

pub fn customer_data_legal_hold_to_wire(
    hold: &CustomerDataLegalHold,
) -> Result<wire::CustomerDataLegalHold, SdkError> {
    Ok(wire::CustomerDataLegalHold {
        customer_data_legal_hold_ref: Some(wire::CustomerDataLegalHoldRef {
            customer_data_legal_hold_id: hold.hold_id().as_str().to_owned(),
        }),
        canonical_party_ref: Some(customer::PartyRef {
            party_id: hold.canonical_party_id().as_str().to_owned(),
        }),
        scope: Some(legal_hold_scope_to_wire(hold.scope())),
        authority_reference_id: hold.authority_reference().as_str().to_owned(),
        reason_code: hold.reason_code().to_owned(),
        policy_version: hold.policy_version().as_str().to_owned(),
        status: match hold.status() {
            LegalHoldStatus::Active => wire::CustomerDataLegalHoldStatus::Active as i32,
            LegalHoldStatus::Released => wire::CustomerDataLegalHoldStatus::Released as i32,
        },
        version: i64::try_from(hold.version())
            .map_err(|_| control_state_invalid("customer-data legal-hold version exceeds i64"))?,
        placed_by_actor_id: hold.placed_by().as_str().to_owned(),
        effective_from_unix_ms: nanos_to_millis(hold.effective_from_unix_nanos())?,
        effective_until_unix_ms: hold
            .effective_until_unix_nanos()
            .map(nanos_to_millis)
            .transpose()?,
        released_by_actor_id: hold.released_by().map(|value| value.as_str().to_owned()),
        released_at_unix_ms: hold
            .released_at_unix_nanos()
            .map(nanos_to_millis)
            .transpose()?,
    })
}

fn legal_hold_scope_to_wire(value: &LegalHoldScope) -> wire::CustomerDataLegalHoldScope {
    let scope = match value {
        LegalHoldScope::AllCustomerData => {
            wire::customer_data_legal_hold_scope::Scope::AllCustomerData(true)
        }
        LegalHoldScope::DataClass(value) => {
            wire::customer_data_legal_hold_scope::Scope::DataClass(data_class_to_wire(*value))
        }
        LegalHoldScope::Owner(value) => {
            wire::customer_data_legal_hold_scope::Scope::OwnerModuleId(value.as_str().to_owned())
        }
    };
    wire::CustomerDataLegalHoldScope { scope: Some(scope) }
}

fn data_class_to_wire(value: DataClass) -> i32 {
    match value {
        DataClass::Public => wire::CustomerDataClass::Public as i32,
        DataClass::Internal => wire::CustomerDataClass::Internal as i32,
        DataClass::Confidential => wire::CustomerDataClass::Confidential as i32,
        DataClass::Restricted => wire::CustomerDataClass::Restricted as i32,
        DataClass::Personal => wire::CustomerDataClass::Personal as i32,
        DataClass::SensitivePersonal => wire::CustomerDataClass::SensitivePersonal as i32,
        DataClass::Biometric => wire::CustomerDataClass::Biometric as i32,
        DataClass::Financial => wire::CustomerDataClass::Financial as i32,
        DataClass::Credential => wire::CustomerDataClass::Credential as i32,
    }
}

fn redact_restriction(value: &mut wire::ProcessingRestriction, allows: impl Fn(&str) -> bool) {
    if !allows("canonical_party_ref") {
        value.canonical_party_ref = None;
    }
    if !allows("scope") {
        value.scope = wire::ProcessingRestrictionScope::Unspecified as i32;
    }
    if !allows("status") {
        value.status = wire::ProcessingRestrictionStatus::Unspecified as i32;
    }
    if !allows("version") {
        value.version = 0;
    }
    if !allows("policy_version") {
        value.policy_version.clear();
    }
    if !allows("placed_by_actor_id") {
        value.placed_by_actor_id.clear();
    }
    if !allows("placed_at_unix_ms") {
        value.placed_at_unix_ms = 0;
    }
    if !allows("effective_from_unix_ms") {
        value.effective_from_unix_ms = 0;
    }
    if !allows("expires_at_unix_ms") {
        value.expires_at_unix_ms = None;
    }
    if !allows("released_by_actor_id") {
        value.released_by_actor_id = None;
    }
    if !allows("released_at_unix_ms") {
        value.released_at_unix_ms = None;
    }
}

fn redact_legal_hold(value: &mut wire::CustomerDataLegalHold, allows: impl Fn(&str) -> bool) {
    if !allows("canonical_party_ref") {
        value.canonical_party_ref = None;
    }
    if !allows("scope") {
        value.scope = None;
    }
    if !allows("authority_reference_id") {
        value.authority_reference_id.clear();
    }
    if !allows("reason_code") {
        value.reason_code.clear();
    }
    if !allows("policy_version") {
        value.policy_version.clear();
    }
    if !allows("status") {
        value.status = wire::CustomerDataLegalHoldStatus::Unspecified as i32;
    }
    if !allows("version") {
        value.version = 0;
    }
    if !allows("placed_by_actor_id") {
        value.placed_by_actor_id.clear();
    }
    if !allows("effective_from_unix_ms") {
        value.effective_from_unix_ms = 0;
    }
    if !allows("effective_until_unix_ms") {
        value.effective_until_unix_ms = None;
    }
    if !allows("released_by_actor_id") {
        value.released_by_actor_id = None;
    }
    if !allows("released_at_unix_ms") {
        value.released_at_unix_ms = None;
    }
}

fn decode_input<M>(request: &QueryRequest, schema: &'static str) -> Result<M, SdkError>
where
    M: Message + Default,
{
    let payload = &request.input;
    if payload.owner.as_str() != MODULE_ID
        || payload.schema_id.as_str() != schema
        || payload.schema_version.as_str() != support::CONTRACT_VERSION
        || payload.descriptor_hash != support::message_descriptor_hash(schema)
        || payload.data_class != DataClass::Personal
        || payload.encoding != PayloadEncoding::Protobuf
        || payload.maximum_size_bytes != support::MAX_PROTOBUF_BYTES
        || payload.validate().is_err()
    {
        return Err(SdkError::new(
            "CUSTOMER_PRIVACY_CONTROL_QUERY_CONTRACT_MISMATCH",
            ErrorCategory::InvalidArgument,
            false,
            "The Customer Privacy control query input does not match the required contract.",
        ));
    }
    M::decode(payload.bytes.as_slice()).map_err(|_| {
        SdkError::new(
            "CUSTOMER_PRIVACY_CONTROL_QUERY_PROTOBUF_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "The Customer Privacy control query input is not valid Protobuf.",
        )
    })
}

fn processing_restriction_ref(
    value: Option<wire::ProcessingRestrictionRef>,
) -> Result<RecordRef, SdkError> {
    let value = value.ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_privacy.restriction.ref",
            "Processing restriction reference is required.",
        )
    })?;
    support::record_ref(
        RESTRICTION_RECORD_TYPE,
        &value.processing_restriction_id,
        "customer_privacy.restriction.ref.processing_restriction_id",
    )
}

fn legal_hold_ref(value: Option<wire::CustomerDataLegalHoldRef>) -> Result<RecordRef, SdkError> {
    let value = value.ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_privacy.legal_hold.ref",
            "Customer-data legal-hold reference is required.",
        )
    })?;
    support::record_ref(
        LEGAL_HOLD_RECORD_TYPE,
        &value.customer_data_legal_hold_id,
        "customer_privacy.legal_hold.ref.customer_data_legal_hold_id",
    )
}

fn party_id(value: Option<customer::PartyRef>) -> Result<RecordId, SdkError> {
    let value = value.ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_privacy.legal_hold.list.canonical_party_ref",
            "Canonical Party reference is required.",
        )
    })?;
    RecordId::try_new(value.party_id).map_err(|error| {
        SdkError::invalid_argument(
            "customer_privacy.legal_hold.list.canonical_party_ref.party_id",
            error.to_string(),
        )
    })
}

fn legal_hold_status_filter(value: Option<i32>) -> Result<Option<i32>, SdkError> {
    value
        .map(
            |value| match wire::CustomerDataLegalHoldStatus::try_from(value) {
                Ok(wire::CustomerDataLegalHoldStatus::Active)
                | Ok(wire::CustomerDataLegalHoldStatus::Released) => Ok(value),
                Ok(wire::CustomerDataLegalHoldStatus::Unspecified) | Err(_) => {
                    Err(SdkError::invalid_argument(
                        "customer_privacy.legal_hold.list.status",
                        "Status must be active or released.",
                    ))
                }
            },
        )
        .transpose()
}

fn page_size(value: i32) -> Result<u32, SdkError> {
    if value < 0 {
        return Err(SdkError::invalid_argument(
            "customer_privacy.legal_hold.list.page_size",
            "Page size must not be negative.",
        ));
    }
    let value = u32::try_from(value).map_err(configuration_invalid)?;
    let value = if value == 0 { DEFAULT_PAGE_SIZE } else { value };
    if value > MAXIMUM_PAGE_SIZE {
        return Err(SdkError::invalid_argument(
            "customer_privacy.legal_hold.list.page_size",
            format!("Page size must not exceed {MAXIMUM_PAGE_SIZE}."),
        ));
    }
    Ok(value)
}

fn ensure_definition(definition: &CapabilityDefinition) -> Result<(), SdkError> {
    if definition.owner_module_id.as_str() != MODULE_ID
        || !CONTROL_QUERY_CAPABILITY_IDS.contains(&definition.capability_id.as_str())
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

fn record_type(value: &'static str) -> Result<RecordType, SdkError> {
    configured(RecordType::try_new(value))
}

fn configured<T>(value: Result<T, crm_module_sdk::IdentifierError>) -> Result<T, SdkError> {
    value.map_err(configuration_invalid)
}

fn nanos_to_millis(value: i64) -> Result<i64, SdkError> {
    if value < 0 {
        return Err(control_state_invalid("control timestamp is negative"));
    }
    Ok(value / NANOS_PER_MILLISECOND)
}

fn enforce_scan_limit(scanned: usize) -> Result<(), SdkError> {
    if scanned > MAXIMUM_VISIBILITY_SCAN_RECORDS {
        Err(SdkError::new(
            "CUSTOMER_PRIVACY_LEGAL_HOLD_LIST_SCAN_LIMIT_EXCEEDED",
            ErrorCategory::Unavailable,
            true,
            "The customer-data legal-hold list is temporarily unavailable.",
        ))
    } else {
        Ok(())
    }
}

fn control_not_found() -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_CONTROL_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The requested Customer Privacy control was not found.",
    )
}

fn control_state_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_CONTROL_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "The Customer Privacy control could not be loaded safely.",
    )
    .with_internal_reference(reference.into())
}

fn unsupported_query() -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_CONTROL_QUERY_UNSUPPORTED",
        ErrorCategory::InvalidArgument,
        false,
        "The requested Customer Privacy control query is not supported.",
    )
}

fn configuration_invalid(error: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_CONTROL_QUERY_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Customer Privacy control query configuration is invalid.",
    )
    .with_internal_reference(error.to_string())
}

fn cursor_invalid() -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_LEGAL_HOLD_LIST_CURSOR_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "The customer-data legal-hold list cursor is invalid.",
    )
}

fn cursor_error(error: impl std::fmt::Display) -> SdkError {
    cursor_invalid().with_internal_reference(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_query_catalog_and_visibility_are_exact() {
        let definitions = control_query_capability_definitions().unwrap();
        assert_eq!(definitions.len(), 3);
        assert_eq!(
            definitions
                .iter()
                .map(|definition| definition.capability_id.as_str())
                .collect::<Vec<_>>(),
            CONTROL_QUERY_CAPABILITY_IDS
        );
        for capability in CONTROL_QUERY_CAPABILITY_IDS {
            let resources = control_query_visibility_resources(capability);
            assert_eq!(resources.len(), 2);
            assert_eq!(resources[0].resource_type, PARTY_RECORD_TYPE);
            assert!(resources[0].allowed_fields.is_empty());
        }
    }

    #[test]
    fn legal_hold_list_bounds_and_filter_are_strict() {
        assert_eq!(page_size(0).unwrap(), DEFAULT_PAGE_SIZE);
        assert_eq!(
            page_size(MAXIMUM_PAGE_SIZE as i32).unwrap(),
            MAXIMUM_PAGE_SIZE
        );
        assert!(page_size(-1).is_err());
        assert!(page_size(MAXIMUM_PAGE_SIZE as i32 + 1).is_err());
        assert!(
            legal_hold_status_filter(Some(wire::CustomerDataLegalHoldStatus::Active as i32))
                .is_ok()
        );
        assert!(
            legal_hold_status_filter(Some(wire::CustomerDataLegalHoldStatus::Unspecified as i32))
                .is_err()
        );
    }
}
