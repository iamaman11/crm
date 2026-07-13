#![forbid(unsafe_code)]

use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRisk};
use crm_consents::{
    CommunicationAuthorizationReason as DomainAuthorizationReason, CommunicationChannel,
    ConsentAuthorization, ConsentAuthorizationStatus, ConsentEffect, ContactPointReference,
    EvaluateCommunicationAuthorization, PartyReference, PurposeCode,
    evaluate_communication_authorization,
};
use crm_consents_capability_adapter::{
    MODULE_ID, RECORD_TYPE, consent_authorization_from_snapshot, consent_authorization_to_wire,
};
use crm_core_data::{
    PostgresDataStore, RecordGetQuery, RecordListQuery, RecordQueryContinuation, RecordQuerySort,
};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, PayloadEncoding,
    PortFuture, RecordId, RecordType, SdkError, TypedPayload,
};
use crm_proto_contracts::crm::{consents::v1 as wire, core::v1 as core, customer::v1 as customer};
use crm_query_runtime::{
    CursorBinding, CursorCodec, CursorContinuation, PageSizePolicy, QueryExecutionResult,
    QueryExecutor, QueryRequest, QuerySemanticValidator, QueryVisibilityAuthorizer,
    QueryVisibilityDecision, normalized_filter_hash,
};
use prost::Message;
use std::sync::Arc;

pub const GET_CAPABILITY: &str = "consents.authorization.get";
pub const LIST_CAPABILITY: &str = "consents.authorization.list";
pub const AUTHORIZE_CAPABILITY: &str = "consents.communication.authorize";
pub const GET_REQUEST_SCHEMA: &str = "crm.consents.v1.GetConsentAuthorizationRequest";
pub const GET_RESPONSE_SCHEMA: &str = "crm.consents.v1.GetConsentAuthorizationResponse";
pub const LIST_REQUEST_SCHEMA: &str = "crm.consents.v1.ListConsentAuthorizationsRequest";
pub const LIST_RESPONSE_SCHEMA: &str = "crm.consents.v1.ListConsentAuthorizationsResponse";
pub const AUTHORIZE_REQUEST_SCHEMA: &str = "crm.consents.v1.AuthorizeCommunicationRequest";
pub const AUTHORIZE_RESPONSE_SCHEMA: &str = "crm.consents.v1.AuthorizeCommunicationResponse";
pub const QUERY_CAPABILITY_IDS: [&str; 3] = [GET_CAPABILITY, LIST_CAPABILITY, AUTHORIZE_CAPABILITY];

const DEFAULT_PAGE_SIZE: u32 = 50;
const MAXIMUM_PAGE_SIZE: u32 = 200;
const MAXIMUM_VISIBILITY_SCAN_RECORDS: usize = 10_000;
const MAXIMUM_AUTHORIZATION_SCAN_RECORDS: usize = 10_000;
const AUTHORIZATION_SCAN_PAGE_SIZE: u32 = 1_000;

#[derive(Clone)]
pub struct ConsentQueryAdapter {
    store: PostgresDataStore,
    cursor_codec: CursorCodec,
    visibility: Arc<dyn QueryVisibilityAuthorizer>,
    page_policy: PageSizePolicy,
}

impl std::fmt::Debug for ConsentQueryAdapter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ConsentQueryAdapter")
            .field("store", &self.store)
            .field("cursor_codec", &self.cursor_codec)
            .field("visibility", &"dyn QueryVisibilityAuthorizer")
            .field("page_policy", &self.page_policy)
            .finish()
    }
}

impl ConsentQueryAdapter {
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
        AUTHORIZE_CAPABILITY => (AUTHORIZE_REQUEST_SCHEMA, AUTHORIZE_RESPONSE_SCHEMA),
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

impl QuerySemanticValidator for ConsentQueryAdapter {
    fn validate<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a QueryRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            match definition.capability_id.as_str() {
                GET_CAPABILITY => {
                    let command: wire::GetConsentAuthorizationRequest =
                        decode_input(request, GET_REQUEST_SCHEMA)?;
                    let authorization_ref = command.authorization_ref.ok_or_else(|| {
                        SdkError::invalid_argument(
                            "consent_authorization.authorization_ref",
                            "Consent Authorization reference is required",
                        )
                    })?;
                    validate_record_id(
                        &authorization_ref.authorization_id,
                        "consent_authorization.authorization_ref.authorization_id",
                    )?;
                }
                LIST_CAPABILITY => {
                    let command: wire::ListConsentAuthorizationsRequest =
                        decode_input(request, LIST_REQUEST_SCHEMA)?;
                    validate_list(self, request, &command)?;
                }
                AUTHORIZE_CAPABILITY => {
                    let command: wire::AuthorizeCommunicationRequest =
                        decode_input(request, AUTHORIZE_REQUEST_SCHEMA)?;
                    let _ = evaluate_command(request, command)?;
                }
                _ => return Err(unsupported_query()),
            }
            Ok(())
        })
    }
}

impl QueryExecutor for ConsentQueryAdapter {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: QueryRequest,
    ) -> PortFuture<'a, Result<QueryExecutionResult, SdkError>> {
        Box::pin(async move {
            let output = match definition.capability_id.as_str() {
                GET_CAPABILITY => self.execute_get(&request).await?,
                LIST_CAPABILITY => self.execute_list(&request).await?,
                AUTHORIZE_CAPABILITY => self.execute_authorize(&request).await?,
                _ => return Err(unsupported_query()),
            };
            Ok(QueryExecutionResult { output })
        })
    }
}

impl ConsentQueryAdapter {
    async fn execute_get(&self, request: &QueryRequest) -> Result<TypedPayload, SdkError> {
        let command: wire::GetConsentAuthorizationRequest =
            decode_input(request, GET_REQUEST_SCHEMA)?;
        let authorization_ref = command.authorization_ref.ok_or_else(|| {
            SdkError::invalid_argument(
                "consent_authorization.authorization_ref",
                "Consent Authorization reference is required",
            )
        })?;
        let record_id = validate_record_id(
            &authorization_ref.authorization_id,
            "consent_authorization.authorization_ref.authorization_id",
        )?;
        let snapshot = self
            .store
            .get_record_for_query(&RecordGetQuery {
                tenant_id: request.context.tenant_id.clone(),
                owner_module_id: consent_module_id()?,
                record_type: consent_record_type()?,
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
        let authorization = consent_authorization_from_snapshot(&snapshot)?;

        support::protobuf_payload(
            MODULE_ID,
            GET_RESPONSE_SCHEMA,
            DataClass::Personal,
            &wire::GetConsentAuthorizationResponse {
                authorization: Some(consent_authorization_to_wire_with_visibility(
                    &authorization,
                    &visibility,
                )),
            },
        )
    }

    async fn execute_list(&self, request: &QueryRequest) -> Result<TypedPayload, SdkError> {
        let command: wire::ListConsentAuthorizationsRequest =
            decode_input(request, LIST_REQUEST_SCHEMA)?;
        let filters = ListFilters::try_from(&command)?;
        let page_size = self
            .page_policy
            .resolve(command.page_size)
            .map_err(cursor_error)?;
        let binding = cursor_binding(
            request,
            consent_record_type()?,
            filters.hash(),
            RecordQuerySort::UpdatedAtDescending,
            page_size,
        );
        let after = decode_after(self, &command.cursor, &binding)?;
        let (authorizations, next) = self
            .collect_authorizations(request, page_size, after, &filters)
            .await?;
        let next_cursor = encode_next(self, &binding, next.as_ref())?;

        support::protobuf_payload(
            MODULE_ID,
            LIST_RESPONSE_SCHEMA,
            DataClass::Personal,
            &wire::ListConsentAuthorizationsResponse {
                authorizations,
                next_cursor,
            },
        )
    }

    async fn execute_authorize(&self, request: &QueryRequest) -> Result<TypedPayload, SdkError> {
        let command: wire::AuthorizeCommunicationRequest =
            decode_input(request, AUTHORIZE_REQUEST_SCHEMA)?;
        let evaluation = evaluate_command(request, command)?;
        let decision = match self
            .load_authorization_candidates(request, &evaluation)
            .await
        {
            Ok(authorizations) => {
                evaluate_communication_authorization(&evaluation, authorizations.iter())?
            }
            Err(error) if is_authorization_data_unavailable(&error) => {
                return authorization_payload(
                    &evaluation,
                    false,
                    wire::CommunicationAuthorizationReason::DataUnavailable,
                    Vec::new(),
                );
            }
            Err(error) => return Err(error),
        };

        let reason = match decision.reason {
            DomainAuthorizationReason::ActiveGrant => {
                wire::CommunicationAuthorizationReason::ActiveGrant
            }
            DomainAuthorizationReason::ActiveDeny => {
                wire::CommunicationAuthorizationReason::ActiveDeny
            }
            DomainAuthorizationReason::Withdrawn => {
                wire::CommunicationAuthorizationReason::Withdrawn
            }
            DomainAuthorizationReason::NoApplicableGrant => {
                wire::CommunicationAuthorizationReason::NoApplicableGrant
            }
        };
        authorization_payload(
            &evaluation,
            decision.allowed,
            reason,
            decision
                .determining_authorization_ids
                .into_iter()
                .map(|authorization_id| wire::ConsentAuthorizationRef {
                    authorization_id: authorization_id.as_str().to_owned(),
                })
                .collect(),
        )
    }

    async fn collect_authorizations(
        &self,
        request: &QueryRequest,
        page_size: u32,
        mut after: Option<RecordQueryContinuation>,
        filters: &ListFilters,
    ) -> Result<
        (
            Vec<wire::ConsentAuthorization>,
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
                    .has_more_visible_authorization(request, anchor.clone(), filters, &mut scanned)
                    .await?;
                return Ok((output, has_more.then_some(anchor).flatten()));
            }

            let page = self
                .store
                .list_records_for_query(&RecordListQuery {
                    tenant_id: request.context.tenant_id.clone(),
                    owner_module_id: consent_module_id()?,
                    record_type: consent_record_type()?,
                    page_size: u32::try_from(remaining)
                        .map_err(|_| visibility_scan_limit_error())?,
                    sort: RecordQuerySort::UpdatedAtDescending,
                    after: after.clone(),
                })
                .await?;
            scanned = scanned.saturating_add(page.records.len());
            enforce_visibility_scan_limit(scanned)?;
            for snapshot in &page.records {
                let authorization = consent_authorization_from_snapshot(snapshot)?;
                if !filters.matches(&authorization) {
                    continue;
                }
                let visibility = self
                    .visibility
                    .authorize_visibility(request, &snapshot.reference)
                    .await?;
                if visibility.resource_visible {
                    output.push(consent_authorization_to_wire_with_visibility(
                        &authorization,
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

    async fn has_more_visible_authorization(
        &self,
        request: &QueryRequest,
        mut after: Option<RecordQueryContinuation>,
        filters: &ListFilters,
        scanned: &mut usize,
    ) -> Result<bool, SdkError> {
        while after.is_some() {
            let page = self
                .store
                .list_records_for_query(&RecordListQuery {
                    tenant_id: request.context.tenant_id.clone(),
                    owner_module_id: consent_module_id()?,
                    record_type: consent_record_type()?,
                    page_size: MAXIMUM_PAGE_SIZE,
                    sort: RecordQuerySort::UpdatedAtDescending,
                    after: after.clone(),
                })
                .await?;
            *scanned = scanned.saturating_add(page.records.len());
            enforce_visibility_scan_limit(*scanned)?;
            for snapshot in &page.records {
                let authorization = consent_authorization_from_snapshot(snapshot)?;
                if filters.matches(&authorization)
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

    async fn load_authorization_candidates(
        &self,
        request: &QueryRequest,
        evaluation: &EvaluateCommunicationAuthorization,
    ) -> Result<Vec<ConsentAuthorization>, SdkError> {
        let mut after = None;
        let mut candidates = Vec::new();
        let mut scanned = 0_usize;
        loop {
            let page = self
                .store
                .list_records_for_query(&RecordListQuery {
                    tenant_id: request.context.tenant_id.clone(),
                    owner_module_id: consent_module_id()?,
                    record_type: consent_record_type()?,
                    page_size: AUTHORIZATION_SCAN_PAGE_SIZE,
                    sort: RecordQuerySort::UpdatedAtDescending,
                    after: after.clone(),
                })
                .await?;
            scanned = scanned.saturating_add(page.records.len());
            enforce_authorization_scan_limit(scanned)?;
            for snapshot in &page.records {
                let authorization = consent_authorization_from_snapshot(snapshot)?;
                if static_scope_matches(evaluation, &authorization) {
                    candidates.push(authorization);
                }
            }
            after = page.next;
            if after.is_none() {
                return Ok(candidates);
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ListFilters {
    party_ref: Option<PartyReference>,
    contact_point_ref: Option<ContactPointReference>,
    purpose: Option<PurposeCode>,
    channel: Option<CommunicationChannel>,
    effect: Option<ConsentEffect>,
    status: Option<ConsentAuthorizationStatus>,
}

impl ListFilters {
    fn try_from(command: &wire::ListConsentAuthorizationsRequest) -> Result<Self, SdkError> {
        Ok(Self {
            party_ref: command
                .party_ref
                .as_ref()
                .map(|value| PartyReference::try_new(value.party_id.clone()))
                .transpose()?,
            contact_point_ref: command
                .contact_point_ref
                .as_ref()
                .map(|value| ContactPointReference::try_new(value.contact_point_id.clone()))
                .transpose()?,
            purpose: command
                .purpose
                .as_ref()
                .map(|value| PurposeCode::try_new(value.clone()))
                .transpose()?,
            channel: optional_channel(command.channel)?,
            effect: optional_effect(command.effect)?,
            status: optional_status(command.status)?,
        })
    }

    fn matches(&self, authorization: &ConsentAuthorization) -> bool {
        if self
            .party_ref
            .as_ref()
            .is_some_and(|value| value != authorization.party_ref())
        {
            return false;
        }
        if self.contact_point_ref.as_ref().is_some_and(|value| {
            authorization
                .contact_point_ref()
                .is_none_or(|authorization_value| authorization_value != value)
        }) {
            return false;
        }
        if self
            .purpose
            .as_ref()
            .is_some_and(|value| value != authorization.purpose())
        {
            return false;
        }
        if self
            .channel
            .is_some_and(|value| value != authorization.channel())
        {
            return false;
        }
        if self
            .effect
            .is_some_and(|value| value != authorization.effect())
        {
            return false;
        }
        if self
            .status
            .is_some_and(|value| value != authorization.status())
        {
            return false;
        }
        true
    }

    fn hash(&self) -> [u8; 32] {
        let party_id = self
            .party_ref
            .as_ref()
            .map(PartyReference::as_str)
            .unwrap_or("");
        let contact_point_id = self
            .contact_point_ref
            .as_ref()
            .map(ContactPointReference::as_str)
            .unwrap_or("");
        let purpose = self.purpose.as_ref().map(PurposeCode::as_str).unwrap_or("");
        let channel = self
            .channel
            .map(channel_wire_value)
            .unwrap_or_default()
            .to_be_bytes();
        let effect = self
            .effect
            .map(effect_wire_value)
            .unwrap_or_default()
            .to_be_bytes();
        let status = self
            .status
            .map(status_wire_value)
            .unwrap_or_default()
            .to_be_bytes();
        normalized_filter_hash([
            ("party_id", party_id.as_bytes()),
            ("contact_point_id", contact_point_id.as_bytes()),
            ("purpose", purpose.as_bytes()),
            ("channel", channel.as_slice()),
            ("effect", effect.as_slice()),
            ("status", status.as_slice()),
        ])
    }
}

fn validate_list(
    adapter: &ConsentQueryAdapter,
    request: &QueryRequest,
    command: &wire::ListConsentAuthorizationsRequest,
) -> Result<(), SdkError> {
    let filters = ListFilters::try_from(command)?;
    let page_size = adapter
        .page_policy
        .resolve(command.page_size)
        .map_err(cursor_error)?;
    let binding = cursor_binding(
        request,
        consent_record_type()?,
        filters.hash(),
        RecordQuerySort::UpdatedAtDescending,
        page_size,
    );
    let _ = decode_after(adapter, &command.cursor, &binding)?;
    Ok(())
}

fn evaluate_command(
    request: &QueryRequest,
    command: wire::AuthorizeCommunicationRequest,
) -> Result<EvaluateCommunicationAuthorization, SdkError> {
    let party_ref = command.party_ref.ok_or_else(|| {
        SdkError::invalid_argument(
            "communication_authorization.party_ref",
            "Party reference is required",
        )
    })?;
    Ok(EvaluateCommunicationAuthorization {
        party_ref: PartyReference::try_new(party_ref.party_id)?,
        contact_point_ref: command
            .contact_point_ref
            .map(|value| ContactPointReference::try_new(value.contact_point_id))
            .transpose()?,
        purpose: PurposeCode::try_new(command.purpose)?,
        channel: required_channel(command.channel)?,
        evaluation_time_unix_nanos: request.context.request_started_at_unix_nanos,
    })
}

fn static_scope_matches(
    evaluation: &EvaluateCommunicationAuthorization,
    authorization: &ConsentAuthorization,
) -> bool {
    if authorization.party_ref() != &evaluation.party_ref
        || authorization.purpose() != &evaluation.purpose
        || authorization.channel() != evaluation.channel
    {
        return false;
    }
    match authorization.contact_point_ref() {
        None => true,
        Some(assertion_contact_point) => {
            evaluation
                .contact_point_ref
                .as_ref()
                .is_some_and(|requested_contact_point| {
                    requested_contact_point == assertion_contact_point
                })
        }
    }
}

fn authorization_payload(
    evaluation: &EvaluateCommunicationAuthorization,
    allowed: bool,
    reason: wire::CommunicationAuthorizationReason,
    determining_authorizations: Vec<wire::ConsentAuthorizationRef>,
) -> Result<TypedPayload, SdkError> {
    support::protobuf_payload(
        MODULE_ID,
        AUTHORIZE_RESPONSE_SCHEMA,
        DataClass::Personal,
        &wire::AuthorizeCommunicationResponse {
            decision: Some(wire::CommunicationAuthorizationDecision {
                allowed,
                reason: reason as i32,
                party_ref: Some(customer::PartyRef {
                    party_id: evaluation.party_ref.as_str().to_owned(),
                }),
                purpose: evaluation.purpose.as_str().to_owned(),
                channel: channel_wire_value(evaluation.channel),
                contact_point_ref: evaluation.contact_point_ref.as_ref().map(|value| {
                    customer::ContactPointRef {
                        contact_point_id: value.as_str().to_owned(),
                    }
                }),
                evaluated_at: Some(core::UnixTime {
                    unix_nanos: evaluation.evaluation_time_unix_nanos,
                }),
                determining_authorizations,
            }),
        },
    )
}

fn consent_authorization_to_wire_with_visibility(
    authorization: &ConsentAuthorization,
    visibility: &QueryVisibilityDecision,
) -> wire::ConsentAuthorization {
    let mut output = consent_authorization_to_wire(authorization);
    if !visibility.allows_field("party_ref") {
        output.party_ref = None;
    }
    if !visibility.allows_field("contact_point_ref") {
        output.contact_point_ref = None;
    }
    if !visibility.allows_field("purpose") {
        output.purpose.clear();
    }
    if !visibility.allows_field("channel") {
        output.channel = wire::CommunicationChannel::Unspecified as i32;
    }
    if !visibility.allows_field("effect") {
        output.effect = wire::ConsentEffect::Unspecified as i32;
    }
    if !visibility.allows_field("legal_basis") {
        output.legal_basis.clear();
    }
    if !visibility.allows_field("jurisdiction") {
        output.jurisdiction.clear();
    }
    if !visibility.allows_field("source") {
        output.source.clear();
    }
    if !visibility.allows_field("evidence_ref") {
        output.evidence_ref.clear();
    }
    if !visibility.allows_field("validity") {
        output.effective_from = None;
        output.expires_at = None;
        output.withdrawn_at = None;
    }
    if !visibility.allows_field("status") {
        output.status = wire::ConsentAuthorizationStatus::Unspecified as i32;
    }
    if !visibility.allows_field("resource_version") {
        output.resource_version = None;
    }
    output
}

fn required_channel(value: i32) -> Result<CommunicationChannel, SdkError> {
    optional_channel(value)?.ok_or_else(|| {
        SdkError::invalid_argument(
            "communication_authorization.channel",
            "Communication channel is required",
        )
    })
}

fn optional_channel(value: i32) -> Result<Option<CommunicationChannel>, SdkError> {
    match wire::CommunicationChannel::try_from(value) {
        Ok(wire::CommunicationChannel::Unspecified) => Ok(None),
        Ok(wire::CommunicationChannel::Email) => Ok(Some(CommunicationChannel::Email)),
        Ok(wire::CommunicationChannel::Phone) => Ok(Some(CommunicationChannel::Phone)),
        Ok(wire::CommunicationChannel::Sms) => Ok(Some(CommunicationChannel::Sms)),
        Ok(wire::CommunicationChannel::Postal) => Ok(Some(CommunicationChannel::Postal)),
        Ok(wire::CommunicationChannel::Messaging) => Ok(Some(CommunicationChannel::Messaging)),
        Ok(wire::CommunicationChannel::Push) => Ok(Some(CommunicationChannel::Push)),
        Err(_) => Err(SdkError::invalid_argument(
            "consent_authorization.channel",
            "Communication channel filter is invalid",
        )),
    }
}

fn optional_effect(value: i32) -> Result<Option<ConsentEffect>, SdkError> {
    match wire::ConsentEffect::try_from(value) {
        Ok(wire::ConsentEffect::Unspecified) => Ok(None),
        Ok(wire::ConsentEffect::Grant) => Ok(Some(ConsentEffect::Grant)),
        Ok(wire::ConsentEffect::Deny) => Ok(Some(ConsentEffect::Deny)),
        Err(_) => Err(SdkError::invalid_argument(
            "consent_authorization.effect",
            "Consent effect filter is invalid",
        )),
    }
}

fn optional_status(value: i32) -> Result<Option<ConsentAuthorizationStatus>, SdkError> {
    match wire::ConsentAuthorizationStatus::try_from(value) {
        Ok(wire::ConsentAuthorizationStatus::Unspecified) => Ok(None),
        Ok(wire::ConsentAuthorizationStatus::Active) => {
            Ok(Some(ConsentAuthorizationStatus::Active))
        }
        Ok(wire::ConsentAuthorizationStatus::Withdrawn) => {
            Ok(Some(ConsentAuthorizationStatus::Withdrawn))
        }
        Err(_) => Err(SdkError::invalid_argument(
            "consent_authorization.status",
            "Consent Authorization status filter is invalid",
        )),
    }
}

fn channel_wire_value(value: CommunicationChannel) -> i32 {
    match value {
        CommunicationChannel::Email => wire::CommunicationChannel::Email as i32,
        CommunicationChannel::Phone => wire::CommunicationChannel::Phone as i32,
        CommunicationChannel::Sms => wire::CommunicationChannel::Sms as i32,
        CommunicationChannel::Postal => wire::CommunicationChannel::Postal as i32,
        CommunicationChannel::Messaging => wire::CommunicationChannel::Messaging as i32,
        CommunicationChannel::Push => wire::CommunicationChannel::Push as i32,
    }
}

fn effect_wire_value(value: ConsentEffect) -> i32 {
    match value {
        ConsentEffect::Grant => wire::ConsentEffect::Grant as i32,
        ConsentEffect::Deny => wire::ConsentEffect::Deny as i32,
    }
}

fn status_wire_value(value: ConsentAuthorizationStatus) -> i32 {
    match value {
        ConsentAuthorizationStatus::Active => wire::ConsentAuthorizationStatus::Active as i32,
        ConsentAuthorizationStatus::Withdrawn => wire::ConsentAuthorizationStatus::Withdrawn as i32,
    }
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
    adapter: &ConsentQueryAdapter,
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
    let sort_value = String::from_utf8(continuation.sort_key).map_err(|_| {
        SdkError::new(
            "CONSENTS_QUERY_CURSOR_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "The Consent Authorization page cursor is invalid.",
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
    adapter: &ConsentQueryAdapter,
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
            "CONSENTS_QUERY_INPUT_CONTRACT_MISMATCH",
            ErrorCategory::InvalidArgument,
            false,
            "The Consent Authorization query input does not match the required contract.",
        ));
    }
    M::decode(payload.bytes.as_slice()).map_err(|_| {
        SdkError::new(
            "CONSENTS_QUERY_INPUT_PROTOBUF_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "The Consent Authorization query input is not valid Protobuf.",
        )
    })
}

fn validate_record_id(value: &str, field: &'static str) -> Result<RecordId, SdkError> {
    RecordId::try_new(value.to_owned())
        .map_err(|error| SdkError::invalid_argument(field, error.to_string()))
}

fn consent_module_id() -> Result<ModuleId, SdkError> {
    ModuleId::try_new(MODULE_ID).map_err(config_error)
}

fn consent_record_type() -> Result<RecordType, SdkError> {
    RecordType::try_new(RECORD_TYPE).map_err(config_error)
}

fn enforce_visibility_scan_limit(scanned: usize) -> Result<(), SdkError> {
    if scanned > MAXIMUM_VISIBILITY_SCAN_RECORDS {
        Err(visibility_scan_limit_error())
    } else {
        Ok(())
    }
}

fn enforce_authorization_scan_limit(scanned: usize) -> Result<(), SdkError> {
    if scanned > MAXIMUM_AUTHORIZATION_SCAN_RECORDS {
        Err(SdkError::new(
            "CONSENTS_AUTHORIZATION_SCAN_LIMIT_EXCEEDED",
            ErrorCategory::Unavailable,
            true,
            "Communication authorization data is temporarily unavailable.",
        ))
    } else {
        Ok(())
    }
}

fn is_authorization_data_unavailable(error: &SdkError) -> bool {
    matches!(
        error.code.as_str(),
        "DATA_QUERY_UNAVAILABLE" | "CONSENTS_AUTHORIZATION_SCAN_LIMIT_EXCEEDED"
    )
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
        "CONSENTS_QUERY_CAPABILITY_UNSUPPORTED",
        ErrorCategory::Internal,
        false,
        "The Consent Authorization query capability is not configured.",
    )
}

fn cursor_error(error: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CONSENTS_QUERY_CURSOR_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "The Consent Authorization page cursor is invalid.",
    )
    .with_internal_reference(error.to_string())
}

fn visibility_scan_limit_error() -> SdkError {
    SdkError::new(
        "CONSENTS_QUERY_VISIBILITY_SCAN_LIMIT_EXCEEDED",
        ErrorCategory::Unavailable,
        true,
        "The Consent Authorization list is temporarily unavailable.",
    )
}

fn config_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    SdkError::new(
        "CONSENTS_QUERY_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Consent Authorization query adapter is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_consents::{
        ConsentAuthorizationId, CreateConsentAuthorization, EvidenceReference, JurisdictionCode,
        SourceCode,
    };
    use std::collections::BTreeSet;

    fn authorization() -> ConsentAuthorization {
        ConsentAuthorization::create(CreateConsentAuthorization {
            authorization_id: ConsentAuthorizationId::try_new("consent-auth-1").unwrap(),
            party_ref: PartyReference::try_new("party-1").unwrap(),
            contact_point_ref: Some(ContactPointReference::try_new("contact-point-1").unwrap()),
            purpose: PurposeCode::try_new("marketing.newsletter").unwrap(),
            channel: CommunicationChannel::Email,
            effect: ConsentEffect::Grant,
            legal_basis: crm_consents::LegalBasisCode::try_new("consent").unwrap(),
            jurisdiction: JurisdictionCode::try_new("eu-lt").unwrap(),
            source: SourceCode::try_new("web.form").unwrap(),
            evidence_ref: EvidenceReference::try_new("evidence://consent/1").unwrap(),
            effective_from_unix_nanos: 100,
            expires_at_unix_nanos: Some(1_000),
            occurred_at_unix_nanos: 100,
        })
        .unwrap()
    }

    #[test]
    fn publishes_get_list_and_authorize_as_personal_read_only_queries() {
        let definitions = query_capability_definitions().unwrap();
        assert_eq!(definitions.len(), 3);
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
    fn list_filters_canonicalize_semantic_purpose_for_stable_cursor_binding() {
        let raw = wire::ListConsentAuthorizationsRequest {
            party_ref: None,
            contact_point_ref: None,
            purpose: Some(" Marketing.Newsletter ".to_owned()),
            channel: wire::CommunicationChannel::Email as i32,
            effect: wire::ConsentEffect::Grant as i32,
            status: wire::ConsentAuthorizationStatus::Active as i32,
            page_size: 50,
            cursor: String::new(),
        };
        let canonical = wire::ListConsentAuthorizationsRequest {
            purpose: Some("marketing.newsletter".to_owned()),
            ..raw.clone()
        };
        assert_eq!(
            ListFilters::try_from(&raw).unwrap().hash(),
            ListFilters::try_from(&canonical).unwrap().hash()
        );
    }

    #[test]
    fn field_visibility_independently_redacts_legal_basis_and_evidence() {
        let value = authorization();
        let decision = QueryVisibilityDecision {
            resource_visible: true,
            allowed_fields: BTreeSet::from([
                "party_ref".to_owned(),
                "purpose".to_owned(),
                "channel".to_owned(),
                "effect".to_owned(),
                "status".to_owned(),
            ]),
            decision_id: "decision-1".to_owned(),
            policy_version: "policy-1".to_owned(),
        };
        let output = consent_authorization_to_wire_with_visibility(&value, &decision);
        assert_eq!(output.purpose, "marketing.newsletter");
        assert!(output.legal_basis.is_empty());
        assert!(output.evidence_ref.is_empty());
        assert!(output.contact_point_ref.is_none());
        assert!(output.resource_version.is_none());
    }

    #[test]
    fn data_unavailable_is_reserved_for_candidate_read_unavailability_not_corruption() {
        let unavailable = SdkError::new(
            "DATA_QUERY_UNAVAILABLE",
            ErrorCategory::Unavailable,
            true,
            "unavailable",
        );
        let corrupt = SdkError::new(
            "DATA_QUERY_STORED_VALUE_INVALID",
            ErrorCategory::Unavailable,
            true,
            "unavailable",
        );
        assert!(is_authorization_data_unavailable(&unavailable));
        assert!(!is_authorization_data_unavailable(&corrupt));
    }
}
