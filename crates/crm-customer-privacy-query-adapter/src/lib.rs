#![forbid(unsafe_code)]

//! Permission-aware Customer Privacy case queries.

mod list;

use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRisk};
use crm_core_data::{PostgresDataStore, RecordGetQuery};
use crm_customer_privacy::{
    MODULE_ID, PRIVACY_CASE_RECORD_TYPE, PrivacyCase, PrivacyCaseKind, PrivacyCaseStatus,
    RescopeRequirement, ResumeStage, SubjectBinding, SubjectVerificationMethod,
};
use crm_customer_privacy_persistence_adapter::privacy_case_from_snapshot;
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, PayloadEncoding,
    PortFuture, RecordId, RecordRef, RecordType, SdkError, TypedPayload,
};
use crm_proto_contracts::crm::{customer::v1 as customer_wire, customer_privacy::v1 as wire};
use crm_query_runtime::{
    CursorCodec, QueryExecutionResult, QueryExecutor, QueryRequest, QuerySemanticValidator,
    QueryVisibilityAuthorizer,
};
use prost::Message;
use std::collections::BTreeSet;
use std::sync::Arc;

pub const GET_PRIVACY_CASE_CAPABILITY: &str = "customer_privacy.case.get";
pub const GET_PRIVACY_CASE_REQUEST_SCHEMA: &str = "crm.customer_privacy.v1.GetPrivacyCaseRequest";
pub const GET_PRIVACY_CASE_RESPONSE_SCHEMA: &str = "crm.customer_privacy.v1.GetPrivacyCaseResponse";
pub const LIST_PRIVACY_CASES_CAPABILITY: &str = "customer_privacy.case.list";
pub const LIST_PRIVACY_CASES_REQUEST_SCHEMA: &str =
    "crm.customer_privacy.v1.ListPrivacyCasesRequest";
pub const LIST_PRIVACY_CASES_RESPONSE_SCHEMA: &str =
    "crm.customer_privacy.v1.ListPrivacyCasesResponse";
pub const QUERY_CAPABILITY_IDS: &[&str] = &[
    GET_PRIVACY_CASE_CAPABILITY,
    LIST_PRIVACY_CASES_CAPABILITY,
];
pub const PARTY_RECORD_TYPE: &str = "parties.party";

const PRIVACY_CASE_FIELDS: &[&str] = &[
    "kind",
    "status",
    "version",
    "policy_version",
    "created_at_unix_ms",
    "updated_at_unix_ms",
    "previous_privacy_case_ref",
    "subject_binding",
    "pending_rescope",
    "scope_snapshot_id",
    "privacy_action_plan_ref",
    "approval",
    "retry_resume_stage",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerPrivacyVisibilityResource {
    pub owner_module_id: &'static str,
    pub resource_type: &'static str,
    pub allowed_fields: BTreeSet<String>,
}

pub fn query_visibility_resources(capability_id: &str) -> Vec<CustomerPrivacyVisibilityResource> {
    if !QUERY_CAPABILITY_IDS.contains(&capability_id) {
        return Vec::new();
    }
    vec![
        CustomerPrivacyVisibilityResource {
            owner_module_id: MODULE_ID,
            resource_type: PARTY_RECORD_TYPE,
            allowed_fields: BTreeSet::new(),
        },
        CustomerPrivacyVisibilityResource {
            owner_module_id: MODULE_ID,
            resource_type: PRIVACY_CASE_RECORD_TYPE,
            allowed_fields: PRIVACY_CASE_FIELDS
                .iter()
                .copied()
                .map(str::to_owned)
                .collect(),
        },
    ]
}

#[derive(Clone)]
pub struct CustomerPrivacyQueryAdapter {
    pub(crate) store: PostgresDataStore,
    pub(crate) visibility: Arc<dyn QueryVisibilityAuthorizer>,
    cursor_codec: Option<CursorCodec>,
}

impl CustomerPrivacyQueryAdapter {
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

    pub(crate) fn cursor_codec(&self) -> Result<&CursorCodec, SdkError> {
        self.cursor_codec.as_ref().ok_or_else(|| {
            query_configuration_invalid("privacy case list cursor codec is not configured")
        })
    }

    async fn execute_get_case(&self, request: &QueryRequest) -> Result<TypedPayload, SdkError> {
        let command: wire::GetPrivacyCaseRequest =
            decode_input(request, GET_PRIVACY_CASE_REQUEST_SCHEMA)?;
        let case_reference = privacy_case_ref(command.privacy_case_ref)?;
        let snapshot = self
            .store
            .get_record_for_query(&RecordGetQuery {
                tenant_id: request.context.tenant_id.clone(),
                owner_module_id: module_id()?,
                record_type: privacy_case_record_type()?,
                record_id: case_reference.record_id.clone(),
            })
            .await?
            .ok_or_else(case_not_found)?;

        let case_visibility = self
            .visibility
            .authorize_visibility(request, &snapshot.reference)
            .await?;
        if !case_visibility.resource_visible {
            return Err(case_not_found());
        }

        let privacy_case = privacy_case_from_snapshot(&snapshot)
            .map_err(|error| case_state_invalid(error.to_string()))?;
        if privacy_case.case_id() != &case_reference.record_id
            || privacy_case.tenant_id() != &request.context.tenant_id
        {
            return Err(case_not_found());
        }

        if let Some(binding) = privacy_case.subject_binding() {
            let canonical_party = support::record_ref(
                PARTY_RECORD_TYPE,
                binding.canonical_party_id.as_str(),
                "customer_privacy.case.subject_binding.canonical_party_ref.party_id",
            )?;
            let subject_visibility = self
                .visibility
                .authorize_visibility(request, &canonical_party)
                .await?;
            if !subject_visibility.resource_visible {
                return Err(case_not_found());
            }
        }

        let mut output = privacy_case_to_wire(&privacy_case)?;
        redact_privacy_case(&mut output, |field| case_visibility.allows_field(field));
        support::protobuf_payload(
            MODULE_ID,
            GET_PRIVACY_CASE_RESPONSE_SCHEMA,
            DataClass::Confidential,
            &wire::GetPrivacyCaseResponse {
                privacy_case: Some(output),
            },
        )
    }
}

impl std::fmt::Debug for CustomerPrivacyQueryAdapter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CustomerPrivacyQueryAdapter")
            .field("store", &self.store)
            .field("visibility", &"dyn QueryVisibilityAuthorizer")
            .field("cursor_codec_configured", &self.cursor_codec.is_some())
            .finish()
    }
}

impl QuerySemanticValidator for CustomerPrivacyQueryAdapter {
    fn validate<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a QueryRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            ensure_definition(definition)?;
            match definition.capability_id.as_str() {
                GET_PRIVACY_CASE_CAPABILITY => {
                    let command: wire::GetPrivacyCaseRequest =
                        decode_input(request, GET_PRIVACY_CASE_REQUEST_SCHEMA)?;
                    privacy_case_ref(command.privacy_case_ref).map(|_| ())
                }
                LIST_PRIVACY_CASES_CAPABILITY => list::validate(self, request),
                _ => Err(unsupported_query()),
            }
        })
    }
}

impl QueryExecutor for CustomerPrivacyQueryAdapter {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: QueryRequest,
    ) -> PortFuture<'a, Result<QueryExecutionResult, SdkError>> {
        Box::pin(async move {
            ensure_definition(definition)?;
            let output = match definition.capability_id.as_str() {
                GET_PRIVACY_CASE_CAPABILITY => self.execute_get_case(&request).await?,
                LIST_PRIVACY_CASES_CAPABILITY => list::execute(self, &request).await?,
                _ => return Err(unsupported_query()),
            };
            Ok(QueryExecutionResult { output })
        })
    }
}

pub fn query_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    get_privacy_case_capability_definition()
}

pub fn get_privacy_case_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    query_definition(
        GET_PRIVACY_CASE_CAPABILITY,
        GET_PRIVACY_CASE_REQUEST_SCHEMA,
        GET_PRIVACY_CASE_RESPONSE_SCHEMA,
    )
}

pub fn list_privacy_cases_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    query_definition(
        LIST_PRIVACY_CASES_CAPABILITY,
        LIST_PRIVACY_CASES_REQUEST_SCHEMA,
        LIST_PRIVACY_CASES_RESPONSE_SCHEMA,
    )
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
            vec![DataClass::Confidential],
        )?,
        output_contract: Some(support::protobuf_contract(
            MODULE_ID,
            response_schema,
            vec![DataClass::Confidential],
        )?),
        risk: CapabilityRisk::Low,
        mutation: false,
        requires_idempotency: false,
        requires_approval: false,
        authorization_policy_id: capability_id.to_owned(),
        rate_limit_policy_id: None,
    })
}

pub fn query_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    Ok(vec![
        get_privacy_case_capability_definition()?,
        list_privacy_cases_capability_definition()?,
    ])
}

pub fn privacy_case_to_wire(privacy_case: &PrivacyCase) -> Result<wire::PrivacyCase, SdkError> {
    let (status, retry_resume_stage) = status_to_wire(privacy_case.status());
    Ok(wire::PrivacyCase {
        privacy_case_ref: Some(wire::PrivacyCaseRef {
            privacy_case_id: privacy_case.case_id().as_str().to_owned(),
        }),
        kind: kind_to_wire(privacy_case.kind()),
        status,
        version: i64::try_from(privacy_case.version())
            .map_err(|_| case_state_invalid("privacy case version exceeds wire range"))?,
        policy_version: privacy_case.policy_version().as_str().to_owned(),
        created_at_unix_ms: nanos_to_millis(
            privacy_case.created_at_unix_nanos(),
            "privacy case creation timestamp",
        )?,
        updated_at_unix_ms: nanos_to_millis(
            privacy_case.last_transition_at_unix_nanos(),
            "privacy case update timestamp",
        )?,
        previous_privacy_case_ref: privacy_case.previous_case_id().map(|value| {
            wire::PrivacyCaseRef {
                privacy_case_id: value.as_str().to_owned(),
            }
        }),
        subject_binding: privacy_case
            .subject_binding()
            .map(subject_binding_to_wire)
            .transpose()?,
        pending_rescope: privacy_case
            .pending_rescope()
            .map(rescope_to_wire)
            .transpose()?,
        scope_snapshot_id: privacy_case
            .scope_snapshot_id()
            .map(|value| value.as_str().to_owned())
            .unwrap_or_default(),
        privacy_action_plan_ref: privacy_case.action_plan_id().map(|value| {
            wire::PrivacyActionPlanRef {
                privacy_action_plan_id: value.as_str().to_owned(),
            }
        }),
        approval: privacy_case
            .approval()
            .map(|value| {
                Ok(wire::PrivacyApprovalEvidence {
                    approved_by_actor_id: value.approved_by.as_str().to_owned(),
                    approved_at_unix_ms: nanos_to_millis(
                        value.approved_at_unix_nanos,
                        "privacy approval timestamp",
                    )?,
                })
            })
            .transpose()?,
        retry_resume_stage,
    })
}

fn subject_binding_to_wire(
    value: &SubjectBinding,
) -> Result<wire::SubjectBindingEvidence, SdkError> {
    Ok(wire::SubjectBindingEvidence {
        submitted_party_ref: Some(customer_wire::PartyRef {
            party_id: value.submitted_party_id.as_str().to_owned(),
        }),
        canonical_party_ref: Some(customer_wire::PartyRef {
            party_id: value.canonical_party_id.as_str().to_owned(),
        }),
        identity_resolution_generation: value.identity_resolution_generation,
        verification_method: verification_method_to_wire(value.verification_method),
        verified_by_actor_id: value.verified_by.as_str().to_owned(),
        verified_at_unix_ms: nanos_to_millis(
            value.verified_at_unix_nanos,
            "subject verification timestamp",
        )?,
    })
}

fn rescope_to_wire(
    value: &RescopeRequirement,
) -> Result<wire::PrivacyRescopeRequirement, SdkError> {
    Ok(wire::PrivacyRescopeRequirement {
        previous_canonical_party_ref: Some(customer_wire::PartyRef {
            party_id: value.previous_canonical_party_id.as_str().to_owned(),
        }),
        proposed_canonical_party_ref: Some(customer_wire::PartyRef {
            party_id: value.proposed_canonical_party_id.as_str().to_owned(),
        }),
        previous_identity_resolution_generation: value.previous_identity_resolution_generation,
        proposed_identity_resolution_generation: value.proposed_identity_resolution_generation,
        detected_at_unix_ms: nanos_to_millis(
            value.detected_at_unix_nanos,
            "privacy rescope timestamp",
        )?,
    })
}

pub(crate) fn redact_privacy_case(
    output: &mut wire::PrivacyCase,
    allows_field: impl Fn(&str) -> bool,
) {
    if !allows_field("kind") {
        output.kind = wire::PrivacyCaseKind::Unspecified as i32;
    }
    if !allows_field("status") {
        output.status = wire::PrivacyCaseStatus::Unspecified as i32;
    }
    if !allows_field("version") {
        output.version = 0;
    }
    if !allows_field("policy_version") {
        output.policy_version.clear();
    }
    if !allows_field("created_at_unix_ms") {
        output.created_at_unix_ms = 0;
    }
    if !allows_field("updated_at_unix_ms") {
        output.updated_at_unix_ms = 0;
    }
    if !allows_field("previous_privacy_case_ref") {
        output.previous_privacy_case_ref = None;
    }
    if !allows_field("subject_binding") {
        output.subject_binding = None;
    }
    if !allows_field("pending_rescope") {
        output.pending_rescope = None;
    }
    if !allows_field("scope_snapshot_id") {
        output.scope_snapshot_id.clear();
    }
    if !allows_field("privacy_action_plan_ref") {
        output.privacy_action_plan_ref = None;
    }
    if !allows_field("approval") {
        output.approval = None;
    }
    if !allows_field("retry_resume_stage") {
        output.retry_resume_stage = None;
    }
}

pub(crate) fn decode_input<M>(
    request: &QueryRequest,
    schema: &'static str,
) -> Result<M, SdkError>
where
    M: Message + Default,
{
    let payload = &request.input;
    if payload.owner.as_str() != MODULE_ID
        || payload.schema_id.as_str() != schema
        || payload.schema_version.as_str() != support::CONTRACT_VERSION
        || payload.descriptor_hash != support::message_descriptor_hash(schema)
        || payload.data_class != DataClass::Confidential
        || payload.encoding != PayloadEncoding::Protobuf
        || payload.maximum_size_bytes != support::MAX_PROTOBUF_BYTES
        || payload.validate().is_err()
    {
        return Err(SdkError::new(
            "CUSTOMER_PRIVACY_QUERY_CONTRACT_MISMATCH",
            ErrorCategory::InvalidArgument,
            false,
            "The Customer Privacy query input does not match the required contract.",
        ));
    }
    M::decode(payload.bytes.as_slice()).map_err(|_| {
        SdkError::new(
            "CUSTOMER_PRIVACY_QUERY_PROTOBUF_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "The Customer Privacy query input is not valid Protobuf.",
        )
    })
}

fn privacy_case_ref(value: Option<wire::PrivacyCaseRef>) -> Result<RecordRef, SdkError> {
    let value = value.ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_privacy.privacy_case_ref",
            "Privacy case reference is required.",
        )
    })?;
    let id = RecordId::try_new(value.privacy_case_id).map_err(|error| {
        SdkError::invalid_argument(
            "customer_privacy.privacy_case_ref.privacy_case_id",
            error.to_string(),
        )
    })?;
    support::record_ref(
        PRIVACY_CASE_RECORD_TYPE,
        id.as_str(),
        "customer_privacy.privacy_case_ref.privacy_case_id",
    )
}

fn ensure_definition(definition: &CapabilityDefinition) -> Result<(), SdkError> {
    if definition.owner_module_id.as_str() != MODULE_ID
        || !QUERY_CAPABILITY_IDS.contains(&definition.capability_id.as_str())
        || definition.capability_version.as_str() != support::CONTRACT_VERSION
        || definition.mutation
    {
        return Err(unsupported_query());
    }
    Ok(())
}

pub(crate) fn module_id() -> Result<ModuleId, SdkError> {
    configured(ModuleId::try_new(MODULE_ID))
}

pub(crate) fn privacy_case_record_type() -> Result<RecordType, SdkError> {
    configured(RecordType::try_new(PRIVACY_CASE_RECORD_TYPE))
}

pub(crate) fn configured<T>(
    value: Result<T, crm_module_sdk::IdentifierError>,
) -> Result<T, SdkError> {
    value.map_err(query_configuration_invalid)
}

fn case_not_found() -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_CASE_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The requested privacy case was not found.",
    )
}

pub(crate) fn case_state_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_CASE_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "The privacy case could not be loaded safely.",
    )
    .with_internal_reference(reference.into())
}

fn unsupported_query() -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_QUERY_UNSUPPORTED",
        ErrorCategory::InvalidArgument,
        false,
        "The requested Customer Privacy query is not supported.",
    )
}

pub(crate) fn query_configuration_invalid(error: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_QUERY_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Customer Privacy query configuration is invalid.",
    )
    .with_internal_reference(error.to_string())
}

fn nanos_to_millis(value: i64, reference: &'static str) -> Result<i64, SdkError> {
    if value < 0 {
        return Err(case_state_invalid(format!("{reference} is negative")));
    }
    Ok(value / 1_000_000)
}

fn kind_to_wire(value: PrivacyCaseKind) -> i32 {
    match value {
        PrivacyCaseKind::Access => wire::PrivacyCaseKind::Access as i32,
        PrivacyCaseKind::PortabilityExport => wire::PrivacyCaseKind::PortabilityExport as i32,
        PrivacyCaseKind::RestrictProcessing => wire::PrivacyCaseKind::RestrictProcessing as i32,
        PrivacyCaseKind::Erasure => wire::PrivacyCaseKind::Erasure as i32,
    }
}

fn verification_method_to_wire(value: SubjectVerificationMethod) -> i32 {
    match value {
        SubjectVerificationMethod::AuthenticatedPortal => {
            wire::SubjectVerificationMethod::AuthenticatedPortal as i32
        }
        SubjectVerificationMethod::StaffAssisted => {
            wire::SubjectVerificationMethod::StaffAssisted as i32
        }
        SubjectVerificationMethod::VerifiedDocument => {
            wire::SubjectVerificationMethod::VerifiedDocument as i32
        }
        SubjectVerificationMethod::ExistingHighAssuranceIdentity => {
            wire::SubjectVerificationMethod::ExistingHighAssuranceIdentity as i32
        }
    }
}

fn status_to_wire(value: PrivacyCaseStatus) -> (i32, Option<i32>) {
    let status = match value {
        PrivacyCaseStatus::Draft => wire::PrivacyCaseStatus::Draft,
        PrivacyCaseStatus::Submitted => wire::PrivacyCaseStatus::Submitted,
        PrivacyCaseStatus::SubjectVerified => wire::PrivacyCaseStatus::SubjectVerified,
        PrivacyCaseStatus::Scoping => wire::PrivacyCaseStatus::Scoping,
        PrivacyCaseStatus::Scoped => wire::PrivacyCaseStatus::Scoped,
        PrivacyCaseStatus::Planned => wire::PrivacyCaseStatus::Planned,
        PrivacyCaseStatus::AwaitingApproval => wire::PrivacyCaseStatus::AwaitingApproval,
        PrivacyCaseStatus::Executing => wire::PrivacyCaseStatus::Executing,
        PrivacyCaseStatus::Converging => wire::PrivacyCaseStatus::Converging,
        PrivacyCaseStatus::RescopeRequired => wire::PrivacyCaseStatus::RescopeRequired,
        PrivacyCaseStatus::FailedRetryable(stage) => {
            return (
                wire::PrivacyCaseStatus::FailedRetryable as i32,
                Some(resume_stage_to_wire(stage)),
            );
        }
        PrivacyCaseStatus::Completed => wire::PrivacyCaseStatus::Completed,
        PrivacyCaseStatus::PartiallyCompleted => wire::PrivacyCaseStatus::PartiallyCompleted,
        PrivacyCaseStatus::Denied => wire::PrivacyCaseStatus::Denied,
        PrivacyCaseStatus::Cancelled => wire::PrivacyCaseStatus::Cancelled,
        PrivacyCaseStatus::FailedTerminal => wire::PrivacyCaseStatus::FailedTerminal,
    };
    (status as i32, None)
}

fn resume_stage_to_wire(value: ResumeStage) -> i32 {
    match value {
        ResumeStage::Scoping => wire::RetryResumeStage::Scoping as i32,
        ResumeStage::Planning => wire::RetryResumeStage::Planning as i32,
        ResumeStage::Executing => wire::RetryResumeStage::Executing as i32,
        ResumeStage::Converging => wire::RetryResumeStage::Converging as i32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_catalog_and_visibility_are_exact() {
        let definitions = query_capability_definitions().unwrap();
        assert_eq!(definitions.len(), 2);
        assert_eq!(
            definitions
                .iter()
                .map(|definition| definition.capability_id.as_str())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([GET_PRIVACY_CASE_CAPABILITY, LIST_PRIVACY_CASES_CAPABILITY])
        );
        for definition in definitions {
            assert_eq!(definition.owner_module_id.as_str(), MODULE_ID);
            assert!(!definition.mutation);
            assert!(!definition.requires_idempotency);
            assert!(!definition.requires_approval);
            assert_eq!(definition.risk, CapabilityRisk::Low);
        }

        for capability in QUERY_CAPABILITY_IDS {
            let resources = query_visibility_resources(capability);
            assert_eq!(resources.len(), 2);
            assert_eq!(resources[0].resource_type, PARTY_RECORD_TYPE);
            assert!(resources[0].allowed_fields.is_empty());
            assert_eq!(resources[1].resource_type, PRIVACY_CASE_RECORD_TYPE);
            assert_eq!(resources[1].allowed_fields.len(), PRIVACY_CASE_FIELDS.len());
        }
        assert!(query_visibility_resources("customer_privacy.unknown").is_empty());
    }

    #[test]
    fn redaction_preserves_identity_and_removes_only_hidden_fields() {
        let mut output = wire::PrivacyCase {
            privacy_case_ref: Some(wire::PrivacyCaseRef {
                privacy_case_id: "privacy-case-1".to_owned(),
            }),
            kind: wire::PrivacyCaseKind::Erasure as i32,
            status: wire::PrivacyCaseStatus::SubjectVerified as i32,
            version: 3,
            policy_version: "privacy-policy/1".to_owned(),
            created_at_unix_ms: 1,
            updated_at_unix_ms: 2,
            previous_privacy_case_ref: None,
            subject_binding: Some(wire::SubjectBindingEvidence::default()),
            pending_rescope: None,
            scope_snapshot_id: String::new(),
            privacy_action_plan_ref: None,
            approval: None,
            retry_resume_stage: None,
        };
        redact_privacy_case(&mut output, |field| field != "subject_binding");
        assert_eq!(
            output.privacy_case_ref.unwrap().privacy_case_id,
            "privacy-case-1"
        );
        assert_eq!(output.version, 3);
        assert!(output.subject_binding.is_none());
    }
}
