#![forbid(unsafe_code)]

use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRisk};
use crm_contact_points::{ContactPoint, ContactPointKind, ContactPointStatus, VerificationState};
use crm_contact_points_capability_adapter::{
    MODULE_ID, RECORD_TYPE, contact_point_from_snapshot, contact_point_to_wire,
};
use crm_core_data::{
    PostgresDataStore, RecordGetQuery, RecordListQuery, RecordQueryContinuation, RecordQuerySort,
};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, PayloadEncoding,
    PortFuture, RecordId, RecordType, SdkError, TypedPayload,
};
use crm_proto_contracts::crm::{contact_points::v1 as wire, core::v1 as core};
use crm_query_runtime::{
    CursorBinding, CursorCodec, CursorContinuation, PageSizePolicy, QueryExecutionResult,
    QueryExecutor, QueryRequest, QuerySemanticValidator, QueryVisibilityAuthorizer,
    QueryVisibilityDecision, normalized_filter_hash,
};
use prost::Message;
use std::sync::Arc;

pub const GET_CAPABILITY: &str = "contact-points.contact-point.get";
pub const LIST_CAPABILITY: &str = "contact-points.contact-point.list";
pub const GET_REQUEST_SCHEMA: &str = "crm.contact_points.v1.GetContactPointRequest";
pub const GET_RESPONSE_SCHEMA: &str = "crm.contact_points.v1.GetContactPointResponse";
pub const LIST_REQUEST_SCHEMA: &str = "crm.contact_points.v1.ListContactPointsRequest";
pub const LIST_RESPONSE_SCHEMA: &str = "crm.contact_points.v1.ListContactPointsResponse";
pub const QUERY_CAPABILITY_IDS: [&str; 2] = [GET_CAPABILITY, LIST_CAPABILITY];

const DEFAULT_PAGE_SIZE: u32 = 50;
const MAXIMUM_PAGE_SIZE: u32 = 200;
const MAXIMUM_VISIBILITY_SCAN_RECORDS: usize = 10_000;

#[derive(Clone)]
pub struct ContactPointQueryAdapter {
    store: PostgresDataStore,
    cursor_codec: CursorCodec,
    visibility: Arc<dyn QueryVisibilityAuthorizer>,
    page_policy: PageSizePolicy,
}

impl std::fmt::Debug for ContactPointQueryAdapter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ContactPointQueryAdapter")
            .field("store", &self.store)
            .field("cursor_codec", &self.cursor_codec)
            .field("visibility", &"dyn QueryVisibilityAuthorizer")
            .field("page_policy", &self.page_policy)
            .finish()
    }
}

impl ContactPointQueryAdapter {
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

impl QuerySemanticValidator for ContactPointQueryAdapter {
    fn validate<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a QueryRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            match definition.capability_id.as_str() {
                GET_CAPABILITY => {
                    let command: wire::GetContactPointRequest =
                        decode_input(request, GET_REQUEST_SCHEMA)?;
                    let contact_point_ref = command.contact_point_ref.ok_or_else(|| {
                        SdkError::invalid_argument(
                            "contact_point.contact_point_ref",
                            "Contact Point reference is required",
                        )
                    })?;
                    validate_record_id(
                        &contact_point_ref.contact_point_id,
                        "contact_point.contact_point_ref.contact_point_id",
                    )?;
                }
                LIST_CAPABILITY => {
                    let command: wire::ListContactPointsRequest =
                        decode_input(request, LIST_REQUEST_SCHEMA)?;
                    validate_list(self, request, &command)?;
                }
                _ => return Err(unsupported_query()),
            }
            Ok(())
        })
    }
}

impl QueryExecutor for ContactPointQueryAdapter {
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

impl ContactPointQueryAdapter {
    async fn execute_get(&self, request: &QueryRequest) -> Result<TypedPayload, SdkError> {
        let command: wire::GetContactPointRequest = decode_input(request, GET_REQUEST_SCHEMA)?;
        let contact_point_ref = command.contact_point_ref.ok_or_else(|| {
            SdkError::invalid_argument(
                "contact_point.contact_point_ref",
                "Contact Point reference is required",
            )
        })?;
        let record_id = validate_record_id(
            &contact_point_ref.contact_point_id,
            "contact_point.contact_point_ref.contact_point_id",
        )?;
        let snapshot = self
            .store
            .get_record_for_query(&RecordGetQuery {
                tenant_id: request.context.tenant_id.clone(),
                owner_module_id: ModuleId::try_new(MODULE_ID).map_err(config_error)?,
                record_type: contact_point_record_type()?,
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
        let contact_point = contact_point_from_snapshot(&snapshot)?;

        support::protobuf_payload(
            MODULE_ID,
            GET_RESPONSE_SCHEMA,
            DataClass::Personal,
            &wire::GetContactPointResponse {
                contact_point: Some(contact_point_to_wire_with_visibility(
                    &contact_point,
                    &visibility,
                )),
            },
        )
    }

    async fn execute_list(&self, request: &QueryRequest) -> Result<TypedPayload, SdkError> {
        let command: wire::ListContactPointsRequest = decode_input(request, LIST_REQUEST_SCHEMA)?;
        let page_size = resolve_page_size(self.page_policy, command.page.as_ref())?;
        let filter_hash = contact_point_filter_hash(&command);
        let binding = cursor_binding(
            request,
            contact_point_record_type()?,
            filter_hash,
            RecordQuerySort::UpdatedAtDescending,
            page_size,
        );
        let after = decode_after(self, command.page.as_ref(), &binding)?;
        let (contact_points, next) = self
            .collect_contact_points(request, page_size, after, &command)
            .await?;
        let next_page_token = encode_next(self, &binding, next.as_ref())?;

        support::protobuf_payload(
            MODULE_ID,
            LIST_RESPONSE_SCHEMA,
            DataClass::Personal,
            &wire::ListContactPointsResponse {
                contact_points,
                page: Some(core::PageInfo {
                    next_page_token,
                    total_size: 0,
                }),
            },
        )
    }

    async fn collect_contact_points(
        &self,
        request: &QueryRequest,
        page_size: u32,
        mut after: Option<RecordQueryContinuation>,
        filters: &wire::ListContactPointsRequest,
    ) -> Result<(Vec<wire::ContactPoint>, Option<RecordQueryContinuation>), SdkError> {
        let mut output = Vec::with_capacity(page_size as usize);
        let mut scanned = 0_usize;
        loop {
            let remaining = page_size as usize - output.len();
            if remaining == 0 {
                let anchor = after.clone();
                let has_more = self
                    .has_more_visible_contact_point(
                        request,
                        anchor.clone(),
                        filters,
                        &mut scanned,
                    )
                    .await?;
                return Ok((output, has_more.then_some(anchor).flatten()));
            }

            let page = self
                .store
                .list_records_for_query(&RecordListQuery {
                    tenant_id: request.context.tenant_id.clone(),
                    owner_module_id: ModuleId::try_new(MODULE_ID).map_err(config_error)?,
                    record_type: contact_point_record_type()?,
                    page_size: u32::try_from(remaining).map_err(|_| scan_limit_error())?,
                    sort: RecordQuerySort::UpdatedAtDescending,
                    after: after.clone(),
                })
                .await?;
            scanned = scanned.saturating_add(page.records.len());
            enforce_scan_limit(scanned)?;
            for snapshot in &page.records {
                let contact_point = contact_point_from_snapshot(snapshot)?;
                if !contact_point_matches_filters(&contact_point, filters) {
                    continue;
                }
                let visibility = self
                    .visibility
                    .authorize_visibility(request, &snapshot.reference)
                    .await?;
                if visibility.resource_visible {
                    output.push(contact_point_to_wire_with_visibility(
                        &contact_point,
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

    async fn has_more_visible_contact_point(
        &self,
        request: &QueryRequest,
        mut after: Option<RecordQueryContinuation>,
        filters: &wire::ListContactPointsRequest,
        scanned: &mut usize,
    ) -> Result<bool, SdkError> {
        while after.is_some() {
            let page = self
                .store
                .list_records_for_query(&RecordListQuery {
                    tenant_id: request.context.tenant_id.clone(),
                    owner_module_id: ModuleId::try_new(MODULE_ID).map_err(config_error)?,
                    record_type: contact_point_record_type()?,
                    page_size: MAXIMUM_PAGE_SIZE,
                    sort: RecordQuerySort::UpdatedAtDescending,
                    after: after.clone(),
                })
                .await?;
            *scanned = scanned.saturating_add(page.records.len());
            enforce_scan_limit(*scanned)?;
            for snapshot in &page.records {
                let contact_point = contact_point_from_snapshot(snapshot)?;
                if contact_point_matches_filters(&contact_point, filters)
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
    adapter: &ContactPointQueryAdapter,
    request: &QueryRequest,
    command: &wire::ListContactPointsRequest,
) -> Result<(), SdkError> {
    if let Some(party_ref) = &command.party_ref {
        validate_record_id(&party_ref.party_id, "contact_point.party_ref.party_id")?;
    }
    if let Some(kind) = command.kind {
        match wire::ContactPointKind::try_from(kind).ok() {
            Some(
                wire::ContactPointKind::Email
                | wire::ContactPointKind::Phone
                | wire::ContactPointKind::Postal
                | wire::ContactPointKind::Web
                | wire::ContactPointKind::Messaging,
            ) => {}
            _ => {
                return Err(SdkError::invalid_argument(
                    "contact_point.kind",
                    "Contact Point kind filter is invalid",
                ));
            }
        }
    }
    if let Some(status) = command.status {
        match wire::ContactPointStatus::try_from(status).ok() {
            Some(wire::ContactPointStatus::Active | wire::ContactPointStatus::Inactive) => {}
            _ => {
                return Err(SdkError::invalid_argument(
                    "contact_point.status",
                    "Contact Point status filter must be ACTIVE or INACTIVE",
                ));
            }
        }
    }
    if let Some(verification_status) = command.verification_status {
        match wire::ContactPointVerificationStatus::try_from(verification_status).ok() {
            Some(
                wire::ContactPointVerificationStatus::Unverified
                | wire::ContactPointVerificationStatus::Verified,
            ) => {}
            _ => {
                return Err(SdkError::invalid_argument(
                    "contact_point.verification_status",
                    "Contact Point verification filter must be UNVERIFIED or VERIFIED",
                ));
            }
        }
    }
    match wire::ContactPointSort::try_from(command.sort).ok() {
        Some(
            wire::ContactPointSort::Unspecified | wire::ContactPointSort::UpdatedAtDescending,
        ) => {}
        None => {
            return Err(SdkError::invalid_argument(
                "contact_point.sort",
                "Contact Point sort is invalid",
            ));
        }
    }

    let page_size = resolve_page_size(adapter.page_policy, command.page.as_ref())?;
    let binding = cursor_binding(
        request,
        contact_point_record_type()?,
        contact_point_filter_hash(command),
        RecordQuerySort::UpdatedAtDescending,
        page_size,
    );
    let _ = decode_after(adapter, command.page.as_ref(), &binding)?;
    Ok(())
}

fn contact_point_matches_filters(
    contact_point: &ContactPoint,
    filters: &wire::ListContactPointsRequest,
) -> bool {
    if filters
        .party_ref
        .as_ref()
        .is_some_and(|party_ref| party_ref.party_id != contact_point.party_ref().as_str())
    {
        return false;
    }
    if let Some(kind) = filters.kind {
        let expected = match wire::ContactPointKind::try_from(kind).ok() {
            Some(wire::ContactPointKind::Email) => ContactPointKind::Email,
            Some(wire::ContactPointKind::Phone) => ContactPointKind::Phone,
            Some(wire::ContactPointKind::Postal) => ContactPointKind::Postal,
            Some(wire::ContactPointKind::Web) => ContactPointKind::Web,
            Some(wire::ContactPointKind::Messaging) => ContactPointKind::Messaging,
            _ => return false,
        };
        if contact_point.kind() != expected {
            return false;
        }
    }
    if let Some(status) = filters.status {
        let expected = match wire::ContactPointStatus::try_from(status).ok() {
            Some(wire::ContactPointStatus::Active) => ContactPointStatus::Active,
            Some(wire::ContactPointStatus::Inactive) => ContactPointStatus::Inactive,
            _ => return false,
        };
        if contact_point.status() != expected {
            return false;
        }
    }
    if let Some(verification_status) = filters.verification_status {
        let expected_verified = match wire::ContactPointVerificationStatus::try_from(
            verification_status,
        )
        .ok()
        {
            Some(wire::ContactPointVerificationStatus::Unverified) => false,
            Some(wire::ContactPointVerificationStatus::Verified) => true,
            _ => return false,
        };
        if contact_point.verification().is_verified() != expected_verified {
            return false;
        }
    }
    if filters
        .preferred
        .is_some_and(|preferred| preferred != contact_point.preferred())
    {
        return false;
    }
    true
}

fn contact_point_to_wire_with_visibility(
    contact_point: &ContactPoint,
    visibility: &QueryVisibilityDecision,
) -> wire::ContactPoint {
    let mut output = contact_point_to_wire(contact_point);
    if !visibility.allows_field("party_ref") {
        output.party_ref = None;
    }
    if !visibility.allows_field("kind") {
        output.kind = wire::ContactPointKind::Unspecified as i32;
    }
    if !visibility.allows_field("normalized_value") {
        output.normalized_value.clear();
    }
    if !visibility.allows_field("display_value") {
        output.display_value.clear();
    }
    if !visibility.allows_field("status") {
        output.status = wire::ContactPointStatus::Unspecified as i32;
    }
    if !visibility.allows_field("preferred") {
        output.preferred = false;
    }
    if !visibility.allows_field("validity") {
        output.valid_from = None;
        output.valid_until = None;
    }
    if !visibility.allows_field("verification") {
        output.verification = None;
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
    adapter: &ContactPointQueryAdapter,
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
            "CONTACT_POINTS_QUERY_CURSOR_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "The Contact Point page cursor is invalid.",
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
    adapter: &ContactPointQueryAdapter,
    binding: &CursorBinding,
    next: Option<&RecordQueryContinuation>,
) -> Result<String, SdkError> {
    next
        .map(|next| {
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

fn contact_point_filter_hash(command: &wire::ListContactPointsRequest) -> [u8; 32] {
    let party_id = command
        .party_ref
        .as_ref()
        .map(|value| value.party_id.as_str())
        .unwrap_or("");
    let kind = command.kind.unwrap_or_default().to_be_bytes();
    let status = command.status.unwrap_or_default().to_be_bytes();
    let verification_status = command
        .verification_status
        .unwrap_or_default()
        .to_be_bytes();
    let preferred = match command.preferred {
        None => [0_u8, 0_u8],
        Some(false) => [1_u8, 0_u8],
        Some(true) => [1_u8, 1_u8],
    };
    normalized_filter_hash([
        ("party_id", party_id.as_bytes()),
        ("kind", kind.as_slice()),
        ("status", status.as_slice()),
        ("verification_status", verification_status.as_slice()),
        ("preferred", preferred.as_slice()),
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
            "CONTACT_POINTS_QUERY_INPUT_CONTRACT_MISMATCH",
            ErrorCategory::InvalidArgument,
            false,
            "The Contact Point query input does not match the required contract.",
        ));
    }
    M::decode(payload.bytes.as_slice()).map_err(|_| {
        SdkError::new(
            "CONTACT_POINTS_QUERY_INPUT_PROTOBUF_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "The Contact Point query input is not valid Protobuf.",
        )
    })
}

fn validate_record_id(value: &str, field: &'static str) -> Result<RecordId, SdkError> {
    RecordId::try_new(value.to_owned())
        .map_err(|error| SdkError::invalid_argument(field, error.to_string()))
}

fn contact_point_record_type() -> Result<RecordType, SdkError> {
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
        "CONTACT_POINTS_QUERY_CAPABILITY_UNSUPPORTED",
        ErrorCategory::Internal,
        false,
        "The Contact Point query capability is not configured.",
    )
}

fn cursor_error(error: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CONTACT_POINTS_QUERY_CURSOR_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "The Contact Point page cursor is invalid.",
    )
    .with_internal_reference(error.to_string())
}

fn scan_limit_error() -> SdkError {
    SdkError::new(
        "CONTACT_POINTS_QUERY_VISIBILITY_SCAN_LIMIT_EXCEEDED",
        ErrorCategory::Unavailable,
        true,
        "The Contact Point list is temporarily unavailable.",
    )
}

fn config_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    SdkError::new(
        "CONTACT_POINTS_QUERY_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Contact Point query adapter is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_contact_points::{
        ContactPointId, CreateContactPoint, PartyReference, VerifyContactPoint,
    };
    use std::collections::BTreeSet;

    fn contact_point() -> ContactPoint {
        let mut value = ContactPoint::create(CreateContactPoint {
            contact_point_id: ContactPointId::try_new("contact-point-visible-1").unwrap(),
            party_ref: PartyReference::try_new("party-1").unwrap(),
            kind: ContactPointKind::Email,
            value: "Ada@EXAMPLE.COM".to_owned(),
            preferred: true,
            valid_from_unix_nanos: Some(10),
            valid_until_unix_nanos: Some(1_000),
            occurred_at_unix_nanos: 10,
        })
        .unwrap();
        value
            .verify(VerifyContactPoint {
                expected_version: 1,
                evidence_ref: "evidence-1".to_owned(),
                occurred_at_unix_nanos: 20,
            })
            .unwrap();
        value
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
    fn field_visibility_redacts_personal_endpoint_fields_without_hiding_identity() {
        let value = contact_point();
        let decision = QueryVisibilityDecision {
            resource_visible: true,
            allowed_fields: BTreeSet::from(["kind".to_owned(), "status".to_owned()]),
            decision_id: "decision-1".to_owned(),
            policy_version: "policy-1".to_owned(),
        };

        let output = contact_point_to_wire_with_visibility(&value, &decision);
        assert_eq!(
            output.contact_point_ref.unwrap().contact_point_id,
            "contact-point-visible-1"
        );
        assert!(output.party_ref.is_none());
        assert_eq!(output.kind, wire::ContactPointKind::Email as i32);
        assert!(output.normalized_value.is_empty());
        assert!(output.display_value.is_empty());
        assert_eq!(output.status, wire::ContactPointStatus::Active as i32);
        assert!(!output.preferred);
        assert!(output.valid_from.is_none());
        assert!(output.valid_until.is_none());
        assert!(output.verification.is_none());
        assert_eq!(output.resource_version.unwrap().version, 2);
    }

    #[test]
    fn typed_filters_are_exact_and_cursor_bound() {
        let value = contact_point();
        let active = wire::ListContactPointsRequest {
            page: None,
            party_ref: Some(crm_proto_contracts::crm::customer::v1::PartyRef {
                party_id: "party-1".to_owned(),
            }),
            kind: Some(wire::ContactPointKind::Email as i32),
            status: Some(wire::ContactPointStatus::Active as i32),
            verification_status: Some(wire::ContactPointVerificationStatus::Verified as i32),
            preferred: Some(true),
            sort: wire::ContactPointSort::UpdatedAtDescending as i32,
        };
        assert!(contact_point_matches_filters(&value, &active));

        let mut inactive = active.clone();
        inactive.status = Some(wire::ContactPointStatus::Inactive as i32);
        assert!(!contact_point_matches_filters(&value, &inactive));
        assert_ne!(
            contact_point_filter_hash(&active),
            contact_point_filter_hash(&inactive)
        );
    }
}
