use crm_capability_plan_support::{self as support, EventSpec};
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest, CapabilityRisk};
use crm_core_data::{
    AggregatePresence, AggregateTarget, BatchMutationPlan, CapabilityBatchExecutionPlan,
    RecordMutation, TransactionalAggregatePlanner,
};
use crm_customer_privacy::{
    MODULE_ID, PRIVACY_CASE_RECORD_TYPE, PrivacyCase, PrivacyCaseStatus, PrivacyDomainError,
};
use crm_customer_privacy_persistence_adapter::{
    privacy_case_from_snapshot, privacy_case_persisted_payload, privacy_case_record_ref,
};
use crm_customer_privacy_query_adapter::privacy_case_to_wire;
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, RecordId, RecordRef,
    RecordSnapshot, SdkError,
};
use crm_proto_contracts::crm::customer_privacy::v1 as wire;

pub const APPROVE_PRIVACY_CASE_CAPABILITY: &str = "customer_privacy.case.approve";
pub const APPROVE_PRIVACY_CASE_REQUEST_SCHEMA: &str =
    "crm.customer_privacy.v1.ApprovePrivacyCaseRequest";
pub const APPROVE_PRIVACY_CASE_RESPONSE_SCHEMA: &str =
    "crm.customer_privacy.v1.ApprovePrivacyCaseResponse";
pub const PRIVACY_CASE_STATUS_CHANGED_EVENT_TYPE: &str = "customer_privacy.case.status_changed";
pub const PRIVACY_CASE_STATUS_CHANGED_EVENT_SCHEMA: &str =
    "crm.customer_privacy.v1.PrivacyCaseStatusChangedEvent";

#[derive(Debug, Default, Clone, Copy)]
pub struct CustomerPrivacyCaseApprovalCapabilityPlanner;

impl TransactionalAggregatePlanner for CustomerPrivacyCaseApprovalCapabilityPlanner {
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
        let command = approval_command(request)?;
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
            .approve(
                positive_version(command.expected_version)?,
                request.context.execution.actor_id.clone(),
                request.context.execution.request_started_at_unix_nanos,
            )
            .map_err(domain_error)?;

        build_plan(definition, request, current, privacy_case, previous_version)
    }
}

pub fn approval_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    Ok(CapabilityDefinition {
        capability_id: configured(CapabilityId::try_new(APPROVE_PRIVACY_CASE_CAPABILITY))?,
        capability_version: configured(CapabilityVersion::try_new(support::CONTRACT_VERSION))?,
        owner_module_id: configured(ModuleId::try_new(MODULE_ID))?,
        input_contract: support::protobuf_contract(
            MODULE_ID,
            APPROVE_PRIVACY_CASE_REQUEST_SCHEMA,
            vec![DataClass::Confidential],
        )?,
        output_contract: Some(support::protobuf_contract(
            MODULE_ID,
            APPROVE_PRIVACY_CASE_RESPONSE_SCHEMA,
            vec![DataClass::Confidential],
        )?),
        risk: CapabilityRisk::High,
        mutation: true,
        requires_idempotency: true,
        requires_approval: false,
        authorization_policy_id: APPROVE_PRIVACY_CASE_CAPABILITY.to_owned(),
        rate_limit_policy_id: None,
    })
}

pub fn privacy_case_ref_from_request(request: &CapabilityRequest) -> Result<RecordRef, SdkError> {
    let command = approval_command(request)?;
    case_ref(
        command
            .privacy_case_ref
            .ok_or_else(|| required("customer_privacy.privacy_case_ref"))?,
    )
}

pub fn expected_version_from_request(request: &CapabilityRequest) -> Result<u64, SdkError> {
    positive_version(approval_command(request)?.expected_version)
}

fn approval_command(
    request: &CapabilityRequest,
) -> Result<wire::ApprovePrivacyCaseRequest, SdkError> {
    request.context.validate()?;
    let command = support::decode_request::<wire::ApprovePrivacyCaseRequest>(
        request,
        MODULE_ID,
        APPROVE_PRIVACY_CASE_REQUEST_SCHEMA,
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
        .map_err(|_| invalid_plan("approved case version exceeds i64"))?;
    let approval = privacy_case
        .approval()
        .ok_or_else(|| invalid_plan("approved case is missing approval evidence"))?;
    if privacy_case.status() != PrivacyCaseStatus::Planned
        || current.version != previous_version
        || next_version != previous_version + 1
        || approval.approved_by != request.context.execution.actor_id
        || approval.approved_at_unix_nanos
            != request.context.execution.request_started_at_unix_nanos
    {
        return Err(invalid_plan(
            "case.approve must advance one awaiting-approval aggregate version with exact actor/time evidence",
        ));
    }

    let aggregate = privacy_case_record_ref(&privacy_case)?;
    if aggregate != current.reference {
        return Err(case_not_found());
    }
    let public_case = privacy_case_to_wire(&privacy_case)?;
    let output = support::protobuf_payload(
        MODULE_ID,
        APPROVE_PRIVACY_CASE_RESPONSE_SCHEMA,
        DataClass::Confidential,
        &wire::ApprovePrivacyCaseResponse {
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

fn ensure_exact_coordinate(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<(), SdkError> {
    if definition.owner_module_id.as_str() != MODULE_ID
        || definition.capability_id.as_str() != APPROVE_PRIVACY_CASE_CAPABILITY
        || definition.capability_version.as_str() != support::CONTRACT_VERSION
        || request.context.module_id.as_str() != MODULE_ID
        || request.context.execution.capability_id.as_str() != APPROVE_PRIVACY_CASE_CAPABILITY
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
        "The customer privacy case could not be approved.",
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
        "CUSTOMER_PRIVACY_CASE_APPROVAL_PLAN_INVALID",
        ErrorCategory::Internal,
        false,
        "The privacy case approval could not be planned safely.",
    )
    .with_internal_reference(reference.into())
}

fn configured<T>(value: Result<T, crm_module_sdk::IdentifierError>) -> Result<T, SdkError> {
    value.map_err(|error| {
        SdkError::new(
            "CUSTOMER_PRIVACY_CASE_APPROVAL_CONFIGURATION_INVALID",
            ErrorCategory::Internal,
            false,
            "The privacy case approval capability is not configured safely.",
        )
        .with_internal_reference(error.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_customer_privacy::{PrivacyCaseKind, SubjectVerificationMethod};
    use crm_module_sdk::{
        ActorId, BusinessTransactionId, CausationId, CorrelationId, ExecutionContext,
        IdempotencyKey, RequestId, SchemaVersion, TenantId, TraceId,
    };
    use prost::Message;

    fn request(expected_version: i64, idempotency_key: &str, started_at: i64) -> CapabilityRequest {
        let command = wire::ApprovePrivacyCaseRequest {
            privacy_case_ref: Some(wire::PrivacyCaseRef {
                privacy_case_id: "privacy-case-a".to_owned(),
            }),
            expected_version,
        };
        CapabilityRequest {
            context: crm_module_sdk::ModuleExecutionContext {
                module_id: ModuleId::try_new(MODULE_ID).unwrap(),
                execution: ExecutionContext {
                    tenant_id: TenantId::try_new("tenant-a").unwrap(),
                    actor_id: ActorId::try_new("privacy-approver").unwrap(),
                    request_id: RequestId::try_new("request-privacy-approve").unwrap(),
                    correlation_id: CorrelationId::try_new("correlation-privacy-approve").unwrap(),
                    causation_id: CausationId::try_new("causation-privacy-approve").unwrap(),
                    trace_id: TraceId::try_new("trace-privacy-approve").unwrap(),
                    capability_id: CapabilityId::try_new(APPROVE_PRIVACY_CASE_CAPABILITY).unwrap(),
                    capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                    idempotency_key: IdempotencyKey::try_new(idempotency_key).unwrap(),
                    business_transaction_id: BusinessTransactionId::try_new(
                        "transaction-privacy-approve",
                    )
                    .unwrap(),
                    schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                    request_started_at_unix_nanos: started_at,
                },
            },
            input: support::protobuf_payload(
                MODULE_ID,
                APPROVE_PRIVACY_CASE_REQUEST_SCHEMA,
                DataClass::Confidential,
                &command,
            )
            .unwrap(),
            input_hash: [41; 32],
            approval: None,
        }
    }

    fn awaiting_approval_snapshot() -> RecordSnapshot {
        let mut case = PrivacyCase::new(
            RecordId::try_new("privacy-case-a").unwrap(),
            TenantId::try_new("tenant-a").unwrap(),
            PrivacyCaseKind::Erasure,
            SchemaVersion::try_new("privacy-policy/1").unwrap(),
            1_000_000_000,
            None,
        )
        .unwrap();
        case.submit(1, 2_000_000_000).unwrap();
        case.verify_subject(
            2,
            RecordId::try_new("submitted-party").unwrap(),
            RecordId::try_new("canonical-party").unwrap(),
            1,
            SubjectVerificationMethod::VerifiedDocument,
            ActorId::try_new("privacy-verifier").unwrap(),
            3_000_000_000,
        )
        .unwrap();
        case.begin_scoping(3, 4_000_000_000).unwrap();
        case.record_scope(
            4,
            RecordId::try_new("privacy-snapshot-a").unwrap(),
            5_000_000_000,
        )
        .unwrap();
        case.record_plan(
            5,
            RecordId::try_new("privacy-plan-a").unwrap(),
            true,
            6_000_000_000,
        )
        .unwrap();
        RecordSnapshot {
            reference: privacy_case_record_ref(&case).unwrap(),
            version: i64::try_from(case.version()).unwrap(),
            payload: privacy_case_persisted_payload(&case).unwrap(),
        }
    }

    #[test]
    fn awaiting_approval_case_advances_with_exact_append_only_evidence() {
        let definition = approval_capability_definition().unwrap();
        let current = awaiting_approval_snapshot();
        let request = request(current.version, "approve-a", 7_000_000_000);
        let plan = CustomerPrivacyCaseApprovalCapabilityPlanner
            .plan(&definition, &request, Some(&current))
            .unwrap();
        assert_eq!(plan.batch.records.len(), 1);
        assert_eq!(plan.batch.events.len(), 1);
        assert_eq!(plan.batch.audits.len(), 1);
        let output = wire::ApprovePrivacyCaseResponse::decode(
            plan.output.as_ref().unwrap().bytes.as_slice(),
        )
        .unwrap()
        .privacy_case
        .unwrap();
        assert_eq!(output.status, wire::PrivacyCaseStatus::Planned as i32);
        assert_eq!(output.version, current.version + 1);
        let approval = output.approval.unwrap();
        assert_eq!(approval.approved_by_actor_id, "privacy-approver");
        assert_eq!(approval.approved_at_unix_ms, 7_000);
    }

    #[test]
    fn stale_and_ineligible_cases_fail_closed() {
        let definition = approval_capability_definition().unwrap();
        let current = awaiting_approval_snapshot();
        let stale = request(current.version - 1, "approve-stale", 7_000_000_000);
        let error = CustomerPrivacyCaseApprovalCapabilityPlanner
            .plan(&definition, &stale, Some(&current))
            .unwrap_err();
        assert_eq!(error.code, "CUSTOMER_PRIVACY_VERSION_CONFLICT");
        assert!(error.retryable);

        let mut planned = privacy_case_from_snapshot(&current).unwrap();
        planned
            .approve(
                planned.version(),
                ActorId::try_new("privacy-approver").unwrap(),
                7_000_000_000,
            )
            .unwrap();
        let planned_snapshot = RecordSnapshot {
            reference: privacy_case_record_ref(&planned).unwrap(),
            version: i64::try_from(planned.version()).unwrap(),
            payload: privacy_case_persisted_payload(&planned).unwrap(),
        };
        let replay_with_new_key = request(planned_snapshot.version, "approve-again", 8_000_000_000);
        assert_eq!(
            CustomerPrivacyCaseApprovalCapabilityPlanner
                .plan(&definition, &replay_with_new_key, Some(&planned_snapshot))
                .unwrap_err()
                .code,
            "CUSTOMER_PRIVACY_INVALID_TRANSITION"
        );
    }
}
