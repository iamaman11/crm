#![forbid(unsafe_code)]

//! Governed planning for `customer_privacy.case.cancel@1.0.0`.
//!
//! Cancellation is an optimistic, idempotent terminal transition. The pure planner
//! strictly rehydrates the authoritative case, preserves all immutable lineage and
//! plans exactly one record update, one status event and one audit intent.

use crm_capability_plan_support::{self as support, EventSpec};
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest, CapabilityRisk};
use crm_core_data::{
    AggregatePresence, AggregateTarget, BatchMutationPlan, CapabilityBatchExecutionPlan,
    RecordMutation, TransactionalAggregatePlanner,
};
use crm_customer_privacy::{
    MODULE_ID, PRIVACY_CASE_RECORD_TYPE, PrivacyCase, PrivacyCaseKind, PrivacyCaseStatus,
    PrivacyDomainError, RescopeRequirement, ResumeStage, SubjectBinding, SubjectVerificationMethod,
};
use crm_customer_privacy_persistence_adapter::{
    privacy_case_from_snapshot, privacy_case_persisted_payload, privacy_case_record_ref,
};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, RecordId, RecordRef,
    RecordSnapshot, SdkError,
};
use crm_proto_contracts::crm::{customer::v1 as customer_wire, customer_privacy::v1 as wire};

pub const CANCEL_PRIVACY_CASE_CAPABILITY: &str = "customer_privacy.case.cancel";
pub const CANCEL_PRIVACY_CASE_REQUEST_SCHEMA: &str =
    "crm.customer_privacy.v1.CancelPrivacyCaseRequest";
pub const CANCEL_PRIVACY_CASE_RESPONSE_SCHEMA: &str =
    "crm.customer_privacy.v1.CancelPrivacyCaseResponse";
pub const PRIVACY_CASE_STATUS_CHANGED_EVENT_TYPE: &str = "customer_privacy.case.status_changed";
pub const PRIVACY_CASE_STATUS_CHANGED_EVENT_SCHEMA: &str =
    "crm.customer_privacy.v1.PrivacyCaseStatusChangedEvent";
pub const IMPLEMENTED_MUTATION_CAPABILITY_IDS: &[&str] = &[CANCEL_PRIVACY_CASE_CAPABILITY];

#[derive(Debug, Default, Clone, Copy)]
pub struct CustomerPrivacyCaseCancelCapabilityPlanner;

impl TransactionalAggregatePlanner for CustomerPrivacyCaseCancelCapabilityPlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        ensure_exact_coordinate(definition, request)?;
        Ok(AggregateTarget {
            reference: privacy_case_ref_from_request(request)?,
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
        let command = cancel_command(request)?;
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

        let previous_version = i64::try_from(privacy_case.version())
            .map_err(|_| invalid_plan("persisted case version exceeds i64"))?;
        privacy_case
            .cancel(
                positive_version(command.expected_version)?,
                request.context.execution.request_started_at_unix_nanos,
            )
            .map_err(domain_error)?;

        build_plan(definition, request, current, privacy_case, previous_version)
    }
}

pub fn capability_definition() -> Result<CapabilityDefinition, SdkError> {
    Ok(CapabilityDefinition {
        capability_id: configured(CapabilityId::try_new(CANCEL_PRIVACY_CASE_CAPABILITY))?,
        capability_version: configured(CapabilityVersion::try_new(support::CONTRACT_VERSION))?,
        owner_module_id: configured(ModuleId::try_new(MODULE_ID))?,
        input_contract: support::protobuf_contract(
            MODULE_ID,
            CANCEL_PRIVACY_CASE_REQUEST_SCHEMA,
            vec![DataClass::Confidential],
        )?,
        output_contract: Some(support::protobuf_contract(
            MODULE_ID,
            CANCEL_PRIVACY_CASE_RESPONSE_SCHEMA,
            vec![DataClass::Confidential],
        )?),
        risk: CapabilityRisk::High,
        mutation: true,
        requires_idempotency: true,
        requires_approval: false,
        authorization_policy_id: CANCEL_PRIVACY_CASE_CAPABILITY.to_owned(),
        rate_limit_policy_id: None,
    })
}

pub fn capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    Ok(vec![capability_definition()?])
}

pub fn privacy_case_ref_from_request(request: &CapabilityRequest) -> Result<RecordRef, SdkError> {
    let command = cancel_command(request)?;
    case_ref(
        command
            .privacy_case_ref
            .ok_or_else(|| required("customer_privacy.privacy_case_ref"))?,
    )
}

pub fn cancellation_subject_lock_ids(snapshot: &RecordSnapshot) -> Result<Vec<RecordId>, SdkError> {
    let privacy_case = privacy_case_from_snapshot(snapshot).map_err(case_state_invalid)?;
    let mut ids = Vec::with_capacity(2);
    if let Some(binding) = privacy_case.subject_binding() {
        ids.push(binding.canonical_party_id.clone());
    }
    if let Some(rescope) = privacy_case.pending_rescope() {
        ids.push(rescope.proposed_canonical_party_id.clone());
    }
    ids.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    ids.dedup();
    Ok(ids)
}

fn cancel_command(request: &CapabilityRequest) -> Result<wire::CancelPrivacyCaseRequest, SdkError> {
    request.context.validate()?;
    let command = support::decode_request::<wire::CancelPrivacyCaseRequest>(
        request,
        MODULE_ID,
        CANCEL_PRIVACY_CASE_REQUEST_SCHEMA,
    )?;
    positive_version(command.expected_version)?;
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
        .map_err(|_| invalid_plan("cancelled case version exceeds i64"))?;
    if privacy_case.status() != PrivacyCaseStatus::Cancelled
        || current.version != previous_version
        || next_version != previous_version + 1
    {
        return Err(invalid_plan(
            "case.cancel must advance the locked cancellable aggregate by one version",
        ));
    }

    let aggregate = privacy_case_record_ref(&privacy_case)?;
    if aggregate != current.reference {
        return Err(case_not_found());
    }
    let public_case = case_to_wire(&privacy_case)?;
    let output = support::protobuf_payload(
        MODULE_ID,
        CANCEL_PRIVACY_CASE_RESPONSE_SCHEMA,
        DataClass::Confidential,
        &wire::CancelPrivacyCaseResponse {
            privacy_case: Some(public_case.clone()),
        },
    )?;
    let event = support::event_evidence(
        request,
        aggregate.clone(),
        MODULE_ID,
        EventSpec {
            event_type: PRIVACY_CASE_STATUS_CHANGED_EVENT_TYPE,
            event_schema_id: PRIVACY_CASE_STATUS_CHANGED_EVENT_SCHEMA,
            aggregate_version: next_version,
            previous_version: Some(previous_version),
        },
        &wire::PrivacyCaseStatusChangedEvent {
            privacy_case: Some(public_case),
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

fn case_to_wire(privacy_case: &PrivacyCase) -> Result<wire::PrivacyCase, SdkError> {
    let (_, retry_resume_stage) = status_to_wire(privacy_case.status());
    Ok(wire::PrivacyCase {
        privacy_case_ref: Some(wire::PrivacyCaseRef {
            privacy_case_id: privacy_case.case_id().as_str().to_owned(),
        }),
        kind: kind_to_wire(privacy_case.kind()),
        status: wire::PrivacyCaseStatus::Cancelled as i32,
        version: i64::try_from(privacy_case.version())
            .map_err(|_| invalid_plan("cancelled case version exceeds wire range"))?,
        policy_version: privacy_case.policy_version().as_str().to_owned(),
        created_at_unix_ms: nanos_to_millis(
            privacy_case.created_at_unix_nanos(),
            "customer_privacy.case.created_at",
        )?,
        updated_at_unix_ms: nanos_to_millis(
            privacy_case.last_transition_at_unix_nanos(),
            "customer_privacy.case.updated_at",
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
                        "customer_privacy.case.approval.approved_at",
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
            "customer_privacy.case.subject.verified_at",
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
            "customer_privacy.case.rescope.detected_at",
        )?,
    })
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

fn ensure_exact_coordinate(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<(), SdkError> {
    if definition.owner_module_id.as_str() != MODULE_ID
        || definition.capability_id.as_str() != CANCEL_PRIVACY_CASE_CAPABILITY
        || definition.capability_version.as_str() != support::CONTRACT_VERSION
        || request.context.module_id.as_str() != MODULE_ID
        || request.context.execution.capability_id.as_str() != CANCEL_PRIVACY_CASE_CAPABILITY
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
        "The customer privacy case could not be cancelled.",
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
        "CUSTOMER_PRIVACY_CASE_CANCEL_PLAN_INVALID",
        ErrorCategory::Internal,
        false,
        "The privacy case cancellation could not be planned safely.",
    )
    .with_internal_reference(reference.into())
}

fn configured<T>(value: Result<T, crm_module_sdk::IdentifierError>) -> Result<T, SdkError> {
    value.map_err(|error| {
        SdkError::new(
            "CUSTOMER_PRIVACY_CASE_CANCEL_CONFIGURATION_INVALID",
            ErrorCategory::Internal,
            false,
            "The privacy case cancel capability is not configured safely.",
        )
        .with_internal_reference(error.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_module_sdk::{
        ActorId, BusinessTransactionId, CausationId, CorrelationId, ExecutionContext,
        IdempotencyKey, ModuleExecutionContext, RequestId, SchemaVersion, TenantId, TraceId,
    };
    use prost::Message;

    fn request(
        tenant: &str,
        case_id: Option<&str>,
        expected_version: i64,
        idempotency_key: &str,
        started_at: i64,
    ) -> CapabilityRequest {
        let command = wire::CancelPrivacyCaseRequest {
            privacy_case_ref: case_id.map(|value| wire::PrivacyCaseRef {
                privacy_case_id: value.to_owned(),
            }),
            expected_version,
        };
        CapabilityRequest {
            context: crm_module_sdk::ModuleExecutionContext {
                module_id: ModuleId::try_new(MODULE_ID).unwrap(),
                execution: ExecutionContext {
                    tenant_id: TenantId::try_new(tenant).unwrap(),
                    actor_id: ActorId::try_new("privacy-officer").unwrap(),
                    request_id: RequestId::try_new("request-privacy-cancel").unwrap(),
                    correlation_id: CorrelationId::try_new("correlation-privacy-cancel").unwrap(),
                    causation_id: CausationId::try_new("causation-privacy-cancel").unwrap(),
                    trace_id: TraceId::try_new("trace-privacy-cancel").unwrap(),
                    capability_id: CapabilityId::try_new(CANCEL_PRIVACY_CASE_CAPABILITY).unwrap(),
                    capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                    idempotency_key: IdempotencyKey::try_new(idempotency_key).unwrap(),
                    business_transaction_id: BusinessTransactionId::try_new(
                        "transaction-privacy-cancel",
                    )
                    .unwrap(),
                    schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                    request_started_at_unix_nanos: started_at,
                },
            },
            input: support::protobuf_payload(
                MODULE_ID,
                CANCEL_PRIVACY_CASE_REQUEST_SCHEMA,
                DataClass::Confidential,
                &command,
            )
            .unwrap(),
            input_hash: [31; 32],
            approval: None,
        }
    }

    fn snapshot(tenant: &str, case_id: &str, status: PrivacyCaseStatus) -> RecordSnapshot {
        let mut case = PrivacyCase::new(
            RecordId::try_new(case_id).unwrap(),
            TenantId::try_new(tenant).unwrap(),
            PrivacyCaseKind::Erasure,
            SchemaVersion::try_new("privacy-policy/1").unwrap(),
            1_000_000_000,
            None,
        )
        .unwrap();
        if status != PrivacyCaseStatus::Draft {
            case.submit(1, 2_000_000_000).unwrap();
        }
        if status == PrivacyCaseStatus::SubjectVerified {
            case.verify_subject(
                2,
                RecordId::try_new("submitted-party").unwrap(),
                RecordId::try_new("canonical-party").unwrap(),
                1,
                SubjectVerificationMethod::VerifiedDocument,
                ActorId::try_new("privacy-officer").unwrap(),
                3_000_000_000,
            )
            .unwrap();
        }
        if status == PrivacyCaseStatus::Cancelled {
            case.cancel(case.version(), 4_000_000_000).unwrap();
        }
        RecordSnapshot {
            reference: privacy_case_record_ref(&case).unwrap(),
            version: i64::try_from(case.version()).unwrap(),
            payload: privacy_case_persisted_payload(&case).unwrap(),
        }
    }

    #[test]
    fn draft_and_verified_cases_cancel_with_exact_evidence() {
        let definition = capability_definition().unwrap();
        for (status, expected_version, at) in [
            (PrivacyCaseStatus::Draft, 1, 2_000_000_000),
            (PrivacyCaseStatus::SubjectVerified, 3, 4_000_000_000),
        ] {
            let current = snapshot("tenant-a", "privacy-case-a", status);
            let request = request(
                "tenant-a",
                Some("privacy-case-a"),
                expected_version,
                "cancel-a",
                at,
            );
            let plan = CustomerPrivacyCaseCancelCapabilityPlanner
                .plan(&definition, &request, Some(&current))
                .unwrap();
            assert_eq!(plan.batch.records.len(), 1);
            assert_eq!(plan.batch.events.len(), 1);
            assert_eq!(plan.batch.audits.len(), 1);
            let output = wire::CancelPrivacyCaseResponse::decode(
                plan.output.as_ref().unwrap().bytes.as_slice(),
            )
            .unwrap()
            .privacy_case
            .unwrap();
            assert_eq!(output.status, wire::PrivacyCaseStatus::Cancelled as i32);
            assert_eq!(output.version, expected_version + 1);
            if status == PrivacyCaseStatus::SubjectVerified {
                assert!(output.subject_binding.is_some());
                assert_eq!(cancellation_subject_lock_ids(&current).unwrap().len(), 1);
            } else {
                assert!(output.subject_binding.is_none());
                assert!(cancellation_subject_lock_ids(&current).unwrap().is_empty());
            }
        }
    }

    #[test]
    fn stale_terminal_cross_tenant_and_malformed_state_fail_closed() {
        let definition = capability_definition().unwrap();
        let current = snapshot("tenant-a", "privacy-case-a", PrivacyCaseStatus::Draft);
        let stale = request(
            "tenant-a",
            Some("privacy-case-a"),
            2,
            "cancel-stale",
            2_000_000_000,
        );
        let error = CustomerPrivacyCaseCancelCapabilityPlanner
            .plan(&definition, &stale, Some(&current))
            .unwrap_err();
        assert_eq!(error.code, "CUSTOMER_PRIVACY_VERSION_CONFLICT");
        assert!(error.retryable);

        let terminal = snapshot("tenant-a", "privacy-case-a", PrivacyCaseStatus::Cancelled);
        let terminal_request = request(
            "tenant-a",
            Some("privacy-case-a"),
            terminal.version,
            "cancel-terminal",
            5_000_000_000,
        );
        assert_eq!(
            CustomerPrivacyCaseCancelCapabilityPlanner
                .plan(&definition, &terminal_request, Some(&terminal))
                .unwrap_err()
                .code,
            "CUSTOMER_PRIVACY_INVALID_TRANSITION"
        );

        let cross_tenant = request(
            "tenant-b",
            Some("privacy-case-a"),
            1,
            "cancel-cross-tenant",
            2_000_000_000,
        );
        assert_eq!(
            CustomerPrivacyCaseCancelCapabilityPlanner
                .plan(&definition, &cross_tenant, Some(&current))
                .unwrap_err()
                .code,
            "CUSTOMER_PRIVACY_CASE_NOT_FOUND"
        );

        let mut malformed = current;
        malformed.payload.bytes = b"{\"raw_secret\":\"must-not-leak\"}".to_vec();
        let malformed_request = request(
            "tenant-a",
            Some("privacy-case-a"),
            1,
            "cancel-malformed",
            2_000_000_000,
        );
        let malformed_error = CustomerPrivacyCaseCancelCapabilityPlanner
            .plan(&definition, &malformed_request, Some(&malformed))
            .unwrap_err();
        assert_eq!(malformed_error.code, "CUSTOMER_PRIVACY_CASE_INVALID");
        assert!(!malformed_error.safe_message.contains("raw_secret"));
    }
}
