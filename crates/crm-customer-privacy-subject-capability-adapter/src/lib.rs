#![forbid(unsafe_code)]

//! Infrastructure-neutral planning for
//! `customer_privacy.case.subject.verify@1.0.0`.
//!
//! This planner validates and persists the exact `Submitted -> SubjectVerified`
//! aggregate transition. It deliberately does not prove Party visibility,
//! canonical redirects, Identity Resolution generation freshness or acquire the
//! shared tenant + canonical Party lock. Those transaction-scoped guarantees
//! belong to the composition guard and must exist before runtime promotion.

use crm_capability_plan_support::{self as support, EventSpec};
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest, CapabilityRisk};
use crm_core_data::{
    AggregatePresence, AggregateTarget, BatchMutationPlan, CapabilityBatchExecutionPlan,
    RecordMutation, TransactionalAggregatePlanner,
};
use crm_customer_privacy::{
    MODULE_ID, PRIVACY_CASE_RECORD_TYPE, PrivacyCase, PrivacyCaseKind, PrivacyCaseStatus,
    PrivacyDomainError, SubjectVerificationMethod,
};
use crm_customer_privacy_persistence_adapter::{
    privacy_case_from_snapshot, privacy_case_persisted_payload, privacy_case_record_ref,
};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, RecordId, RecordRef,
    RecordSnapshot, SdkError,
};
use crm_proto_contracts::crm::customer::v1 as customer_wire;
use crm_proto_contracts::crm::customer_privacy::v1 as wire;

pub const VERIFY_PRIVACY_CASE_SUBJECT_CAPABILITY: &str =
    "customer_privacy.case.subject.verify";
pub const VERIFY_PRIVACY_CASE_SUBJECT_REQUEST_SCHEMA: &str =
    "crm.customer_privacy.v1.VerifyPrivacyCaseSubjectRequest";
pub const VERIFY_PRIVACY_CASE_SUBJECT_RESPONSE_SCHEMA: &str =
    "crm.customer_privacy.v1.VerifyPrivacyCaseSubjectResponse";
pub const PRIVACY_CASE_SUBJECT_VERIFIED_EVENT_TYPE: &str =
    "customer_privacy.case.subject_verified";
pub const PRIVACY_CASE_SUBJECT_VERIFIED_EVENT_SCHEMA: &str =
    "crm.customer_privacy.v1.PrivacyCaseSubjectVerifiedEvent";
pub const IMPLEMENTED_MUTATION_CAPABILITY_IDS: &[&str] =
    &[VERIFY_PRIVACY_CASE_SUBJECT_CAPABILITY];

#[derive(Debug, Default, Clone, Copy)]
pub struct CustomerPrivacyCaseSubjectVerifyCapabilityPlanner;

impl TransactionalAggregatePlanner for CustomerPrivacyCaseSubjectVerifyCapabilityPlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        ensure_exact_coordinate(definition, request)?;
        let command = verify_command(request)?;
        Ok(AggregateTarget {
            reference: case_ref(
                command
                    .privacy_case_ref
                    .ok_or_else(|| required("customer_privacy.privacy_case_ref"))?,
            )?,
            presence: AggregatePresence::MustExist,
        })
    }

    fn plan(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
        current: Option<&RecordSnapshot>,
    ) -> Result<CapabilityBatchExecutionPlan, SdkError> {
        ensure_exact_coordinate(definition, request)?;
        let command = verify_command(request)?;
        let requested_ref = case_ref(
            command
                .privacy_case_ref
                .ok_or_else(|| required("customer_privacy.privacy_case_ref"))?,
        )?;
        let current = current.ok_or_else(case_not_found)?;
        if current.reference != requested_ref {
            return Err(case_not_found());
        }

        let mut privacy_case = privacy_case_from_snapshot(current).map_err(case_state_invalid)?;
        if privacy_case.case_id() != &requested_ref.record_id
            || privacy_case.tenant_id() != &request.context.execution.tenant_id
        {
            return Err(case_not_found());
        }

        let submitted_party_id = party_id(
            command.submitted_party_ref,
            "customer_privacy.submitted_party_ref.party_id",
        )?;
        let canonical_party_id = party_id(
            command.canonical_party_ref,
            "customer_privacy.canonical_party_ref.party_id",
        )?;
        let verification_method = verification_method(command.verification_method)?;
        let previous_version = i64::try_from(privacy_case.version())
            .map_err(|_| invalid_plan("persisted case version exceeds i64"))?;

        privacy_case
            .verify_subject(
                positive_version(command.expected_version)?,
                submitted_party_id,
                canonical_party_id,
                command.identity_resolution_generation,
                verification_method,
                request.context.execution.actor_id.clone(),
                request.context.execution.request_started_at_unix_nanos,
            )
            .map_err(domain_error)?;

        build_plan(definition, request, current, privacy_case, previous_version)
    }
}

pub fn capability_definition() -> Result<CapabilityDefinition, SdkError> {
    Ok(CapabilityDefinition {
        capability_id: configured(CapabilityId::try_new(
            VERIFY_PRIVACY_CASE_SUBJECT_CAPABILITY,
        ))?,
        capability_version: configured(CapabilityVersion::try_new(support::CONTRACT_VERSION))?,
        owner_module_id: configured(ModuleId::try_new(MODULE_ID))?,
        input_contract: support::protobuf_contract(
            MODULE_ID,
            VERIFY_PRIVACY_CASE_SUBJECT_REQUEST_SCHEMA,
            vec![DataClass::Confidential],
        )?,
        output_contract: Some(support::protobuf_contract(
            MODULE_ID,
            VERIFY_PRIVACY_CASE_SUBJECT_RESPONSE_SCHEMA,
            vec![DataClass::Confidential],
        )?),
        risk: CapabilityRisk::High,
        mutation: true,
        requires_idempotency: true,
        requires_approval: false,
        authorization_policy_id: VERIFY_PRIVACY_CASE_SUBJECT_CAPABILITY.to_owned(),
        rate_limit_policy_id: None,
    })
}

pub fn capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    Ok(vec![capability_definition()?])
}

fn verify_command(
    request: &CapabilityRequest,
) -> Result<wire::VerifyPrivacyCaseSubjectRequest, SdkError> {
    request.context.validate()?;
    let command = support::decode_request::<wire::VerifyPrivacyCaseSubjectRequest>(
        request,
        MODULE_ID,
        VERIFY_PRIVACY_CASE_SUBJECT_REQUEST_SCHEMA,
    )?;
    positive_version(command.expected_version)?;
    if command.identity_resolution_generation == 0 {
        return Err(SdkError::invalid_argument(
            "customer_privacy.identity_resolution_generation",
            "Identity Resolution generation must be positive.",
        ));
    }
    verification_method(command.verification_method)?;
    Ok(command)
}

fn build_plan(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    current: &RecordSnapshot,
    privacy_case: PrivacyCase,
    previous_version: i64,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    let next_version = i64::try_from(privacy_case.version())
        .map_err(|_| invalid_plan("verified case version exceeds i64"))?;
    if privacy_case.status() != PrivacyCaseStatus::SubjectVerified
        || current.version != previous_version
        || next_version != previous_version + 1
        || privacy_case.subject_binding().is_none()
    {
        return Err(invalid_plan(
            "subject verification must advance the locked Submitted case by one version",
        ));
    }
    if privacy_case.pending_rescope().is_some()
        || privacy_case.scope_snapshot_id().is_some()
        || privacy_case.action_plan_id().is_some()
        || privacy_case.approval().is_some()
    {
        return Err(invalid_plan(
            "subject-verified case contains evidence from a later lifecycle phase",
        ));
    }

    let aggregate = privacy_case_record_ref(&privacy_case)?;
    if aggregate != current.reference {
        return Err(case_not_found());
    }
    let public_case = case_to_wire(
        &privacy_case,
        request.context.execution.request_started_at_unix_nanos,
    )?;
    let subject_binding = public_case
        .subject_binding
        .clone()
        .ok_or_else(|| invalid_plan("verified case output lacks subject binding"))?;
    let output = support::protobuf_payload(
        MODULE_ID,
        VERIFY_PRIVACY_CASE_SUBJECT_RESPONSE_SCHEMA,
        DataClass::Confidential,
        &wire::VerifyPrivacyCaseSubjectResponse {
            privacy_case: Some(public_case.clone()),
        },
    )?;
    let event = support::event_evidence(
        request,
        aggregate.clone(),
        MODULE_ID,
        EventSpec {
            event_type: PRIVACY_CASE_SUBJECT_VERIFIED_EVENT_TYPE,
            event_schema_id: PRIVACY_CASE_SUBJECT_VERIFIED_EVENT_SCHEMA,
            aggregate_version: next_version,
            previous_version: Some(previous_version),
        },
        &wire::PrivacyCaseSubjectVerifiedEvent {
            privacy_case: Some(public_case),
            subject_binding: Some(subject_binding),
        },
    )?;
    let audit = support::audit_intent(
        request,
        &aggregate,
        next_version,
        definition.capability_id.as_str(),
        &output.bytes,
    )?;

    Ok(CapabilityBatchExecutionPlan {
        batch: BatchMutationPlan {
            context: request.context.clone(),
            records: vec![RecordMutation::Update {
                reference: aggregate,
                expected_version: previous_version,
                payload: privacy_case_persisted_payload(&privacy_case)?,
            }],
            relationships: Vec::new(),
            events: vec![event],
            idempotency: support::capability_idempotency(definition, request)?,
            audits: vec![audit],
        },
        output: Some(output),
    })
}

fn case_to_wire(
    privacy_case: &PrivacyCase,
    updated_at: i64,
) -> Result<wire::PrivacyCase, SdkError> {
    let binding = privacy_case
        .subject_binding()
        .ok_or_else(|| invalid_plan("subject binding is required"))?;
    Ok(wire::PrivacyCase {
        privacy_case_ref: Some(wire::PrivacyCaseRef {
            privacy_case_id: privacy_case.case_id().as_str().to_owned(),
        }),
        kind: kind_to_wire(privacy_case.kind()),
        status: wire::PrivacyCaseStatus::SubjectVerified as i32,
        version: i64::try_from(privacy_case.version())
            .map_err(|_| invalid_plan("verified case version exceeds wire range"))?,
        policy_version: privacy_case.policy_version().as_str().to_owned(),
        created_at_unix_ms: nanos_to_millis(
            privacy_case.created_at_unix_nanos(),
            "customer_privacy.case.created_at",
        )?,
        updated_at_unix_ms: nanos_to_millis(
            updated_at,
            "execution_context.request_started_at_unix_nanos",
        )?,
        previous_privacy_case_ref: privacy_case.previous_case_id().map(|value| {
            wire::PrivacyCaseRef {
                privacy_case_id: value.as_str().to_owned(),
            }
        }),
        subject_binding: Some(wire::SubjectBindingEvidence {
            submitted_party_ref: Some(customer_wire::PartyRef {
                party_id: binding.submitted_party_id.as_str().to_owned(),
            }),
            canonical_party_ref: Some(customer_wire::PartyRef {
                party_id: binding.canonical_party_id.as_str().to_owned(),
            }),
            identity_resolution_generation: binding.identity_resolution_generation,
            verification_method: method_to_wire(binding.verification_method),
            verified_by_actor_id: binding.verified_by.as_str().to_owned(),
            verified_at_unix_ms: nanos_to_millis(
                binding.verified_at_unix_nanos,
                "customer_privacy.subject_binding.verified_at",
            )?,
        }),
        pending_rescope: None,
        scope_snapshot_id: String::new(),
        privacy_action_plan_ref: None,
        approval: None,
        retry_resume_stage: None,
    })
}

fn ensure_exact_coordinate(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<(), SdkError> {
    if definition.owner_module_id.as_str() != MODULE_ID
        || definition.capability_id.as_str() != VERIFY_PRIVACY_CASE_SUBJECT_CAPABILITY
        || definition.capability_version.as_str() != support::CONTRACT_VERSION
        || request.context.module_id.as_str() != MODULE_ID
        || request.context.execution.capability_id.as_str()
            != VERIFY_PRIVACY_CASE_SUBJECT_CAPABILITY
        || request.context.execution.capability_version.as_str() != support::CONTRACT_VERSION
    {
        return Err(invalid_plan(
            "capability definition and request coordinate do not match",
        ));
    }
    Ok(())
}

fn positive_version(value: i64) -> Result<u64, SdkError> {
    if value <= 0 {
        return Err(SdkError::invalid_argument(
            "customer_privacy.case.expected_version",
            "Expected version must be positive.",
        ));
    }
    u64::try_from(value).map_err(|_| {
        SdkError::invalid_argument(
            "customer_privacy.case.expected_version",
            "Expected version is outside the supported range.",
        )
    })
}

fn case_ref(reference: wire::PrivacyCaseRef) -> Result<RecordRef, SdkError> {
    let id = RecordId::try_new(reference.privacy_case_id).map_err(|error| {
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

fn party_id(reference: Option<customer_wire::PartyRef>, field: &'static str) -> Result<RecordId, SdkError> {
    let reference = reference.ok_or_else(|| SdkError::invalid_argument(field, "Party reference is required."))?;
    RecordId::try_new(reference.party_id)
        .map_err(|error| SdkError::invalid_argument(field, error.to_string()))
}

fn verification_method(value: i32) -> Result<SubjectVerificationMethod, SdkError> {
    match wire::SubjectVerificationMethod::try_from(value).ok() {
        Some(wire::SubjectVerificationMethod::AuthenticatedPortal) => {
            Ok(SubjectVerificationMethod::AuthenticatedPortal)
        }
        Some(wire::SubjectVerificationMethod::StaffAssisted) => {
            Ok(SubjectVerificationMethod::StaffAssisted)
        }
        Some(wire::SubjectVerificationMethod::VerifiedDocument) => {
            Ok(SubjectVerificationMethod::VerifiedDocument)
        }
        Some(wire::SubjectVerificationMethod::ExistingHighAssuranceIdentity) => {
            Ok(SubjectVerificationMethod::ExistingHighAssuranceIdentity)
        }
        _ => Err(SdkError::invalid_argument(
            "customer_privacy.verification_method",
            "A supported subject verification method is required.",
        )),
    }
}

fn method_to_wire(value: SubjectVerificationMethod) -> i32 {
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

fn kind_to_wire(value: PrivacyCaseKind) -> i32 {
    match value {
        PrivacyCaseKind::Access => wire::PrivacyCaseKind::Access as i32,
        PrivacyCaseKind::PortabilityExport => wire::PrivacyCaseKind::PortabilityExport as i32,
        PrivacyCaseKind::RestrictProcessing => wire::PrivacyCaseKind::RestrictProcessing as i32,
        PrivacyCaseKind::Erasure => wire::PrivacyCaseKind::Erasure as i32,
    }
}

fn nanos_to_millis(value: i64, field: &'static str) -> Result<i64, SdkError> {
    if value < 0 {
        return Err(SdkError::invalid_argument(
            field,
            "Timestamp must not be negative.",
        ));
    }
    Ok(value / 1_000_000)
}

fn domain_error(error: PrivacyDomainError) -> SdkError {
    let category = match error {
        PrivacyDomainError::VersionConflict { .. }
        | PrivacyDomainError::InvalidTransition { .. } => ErrorCategory::Conflict,
        PrivacyDomainError::InvalidArgument { .. } => ErrorCategory::InvalidArgument,
    };
    SdkError::new(
        error.code(),
        category,
        error.retryable(),
        "The customer privacy subject could not be verified.",
    )
    .with_internal_reference(error.to_string())
}

fn case_not_found() -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_CASE_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The privacy case was not found.",
    )
}

fn case_state_invalid(error: SdkError) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_CASE_INVALID",
        ErrorCategory::Internal,
        false,
        "The privacy case could not be loaded safely.",
    )
    .with_internal_reference(error.code)
}

fn required(field: &'static str) -> SdkError {
    SdkError::invalid_argument(field, "Privacy case reference is required.")
}

fn invalid_plan(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_CASE_SUBJECT_VERIFY_PLAN_INVALID",
        ErrorCategory::Internal,
        false,
        "The privacy case subject verification could not be planned safely.",
    )
    .with_internal_reference(reference.into())
}

fn configured<T>(value: Result<T, crm_module_sdk::IdentifierError>) -> Result<T, SdkError> {
    value.map_err(|error| {
        SdkError::new(
            "CUSTOMER_PRIVACY_CASE_SUBJECT_VERIFY_CONFIGURATION_INVALID",
            ErrorCategory::Internal,
            false,
            "The privacy subject verification capability is not configured safely.",
        )
        .with_internal_reference(error.to_string())
    })
}
