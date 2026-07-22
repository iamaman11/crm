#![forbid(unsafe_code)]

//! Governed planning for `customer_privacy.case.submit@1.0.0`.
//!
//! The planner is infrastructure-neutral. It resolves exactly one tenant-scoped
//! privacy-case aggregate, requires it to exist, strictly rehydrates the
//! canonical persisted envelope and plans the optimistic `Draft -> Submitted`
//! transition as one record update, one immutable status event, one audit and
//! one capability-idempotency claim.

use crm_capability_plan_support::{self as support, EventSpec};
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest, CapabilityRisk};
use crm_core_data::{
    AggregatePresence, AggregateTarget, BatchMutationPlan, CapabilityBatchExecutionPlan,
    RecordMutation, TransactionalAggregatePlanner,
};
use crm_customer_privacy::{
    MODULE_ID, PRIVACY_CASE_RECORD_TYPE, PrivacyCase, PrivacyCaseKind, PrivacyCaseStatus,
    PrivacyDomainError,
};
use crm_customer_privacy_persistence_adapter::{
    privacy_case_from_snapshot, privacy_case_persisted_payload, privacy_case_record_ref,
};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, RecordId, RecordRef,
    RecordSnapshot, SdkError,
};
use crm_proto_contracts::crm::customer_privacy::v1 as wire;

pub const SUBMIT_PRIVACY_CASE_CAPABILITY: &str = "customer_privacy.case.submit";
pub const SUBMIT_PRIVACY_CASE_REQUEST_SCHEMA: &str =
    "crm.customer_privacy.v1.SubmitPrivacyCaseRequest";
pub const SUBMIT_PRIVACY_CASE_RESPONSE_SCHEMA: &str =
    "crm.customer_privacy.v1.SubmitPrivacyCaseResponse";
pub const PRIVACY_CASE_STATUS_CHANGED_EVENT_TYPE: &str = "customer_privacy.case.status_changed";
pub const PRIVACY_CASE_STATUS_CHANGED_EVENT_SCHEMA: &str =
    "crm.customer_privacy.v1.PrivacyCaseStatusChangedEvent";
pub const IMPLEMENTED_MUTATION_CAPABILITY_IDS: &[&str] = &[SUBMIT_PRIVACY_CASE_CAPABILITY];

#[derive(Debug, Default, Clone, Copy)]
pub struct CustomerPrivacyCaseSubmitCapabilityPlanner;

impl TransactionalAggregatePlanner for CustomerPrivacyCaseSubmitCapabilityPlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        ensure_exact_coordinate(definition, request)?;
        let command = submit_command(request)?;
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
        let command = submit_command(request)?;
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
            .submit(
                positive_version(command.expected_version)?,
                request.context.execution.request_started_at_unix_nanos,
            )
            .map_err(domain_error)?;

        build_plan(definition, request, current, privacy_case, previous_version)
    }
}

pub fn capability_definition() -> Result<CapabilityDefinition, SdkError> {
    Ok(CapabilityDefinition {
        capability_id: configured(CapabilityId::try_new(SUBMIT_PRIVACY_CASE_CAPABILITY))?,
        capability_version: configured(CapabilityVersion::try_new(support::CONTRACT_VERSION))?,
        owner_module_id: configured(ModuleId::try_new(MODULE_ID))?,
        input_contract: support::protobuf_contract(
            MODULE_ID,
            SUBMIT_PRIVACY_CASE_REQUEST_SCHEMA,
            vec![DataClass::Confidential],
        )?,
        output_contract: Some(support::protobuf_contract(
            MODULE_ID,
            SUBMIT_PRIVACY_CASE_RESPONSE_SCHEMA,
            vec![DataClass::Confidential],
        )?),
        risk: CapabilityRisk::High,
        mutation: true,
        requires_idempotency: true,
        requires_approval: false,
        authorization_policy_id: SUBMIT_PRIVACY_CASE_CAPABILITY.to_owned(),
        rate_limit_policy_id: None,
    })
}

pub fn capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    Ok(vec![capability_definition()?])
}

fn submit_command(request: &CapabilityRequest) -> Result<wire::SubmitPrivacyCaseRequest, SdkError> {
    request.context.validate()?;
    let command = support::decode_request::<wire::SubmitPrivacyCaseRequest>(
        request,
        MODULE_ID,
        SUBMIT_PRIVACY_CASE_REQUEST_SCHEMA,
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
        .map_err(|_| invalid_plan("submitted case version exceeds i64"))?;
    if privacy_case.status() != PrivacyCaseStatus::Submitted
        || current.version != previous_version
        || next_version != previous_version + 1
    {
        return Err(invalid_plan(
            "case.submit must advance the locked Draft aggregate by one version",
        ));
    }
    if privacy_case.subject_binding().is_some()
        || privacy_case.pending_rescope().is_some()
        || privacy_case.scope_snapshot_id().is_some()
        || privacy_case.action_plan_id().is_some()
        || privacy_case.approval().is_some()
    {
        return Err(invalid_plan(
            "submitted case contains evidence from a later lifecycle phase",
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
    let output = support::protobuf_payload(
        MODULE_ID,
        SUBMIT_PRIVACY_CASE_RESPONSE_SCHEMA,
        DataClass::Confidential,
        &wire::SubmitPrivacyCaseResponse {
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

fn case_to_wire(
    privacy_case: &PrivacyCase,
    updated_at: i64,
) -> Result<wire::PrivacyCase, SdkError> {
    Ok(wire::PrivacyCase {
        privacy_case_ref: Some(wire::PrivacyCaseRef {
            privacy_case_id: privacy_case.case_id().as_str().to_owned(),
        }),
        kind: kind_to_wire(privacy_case.kind()),
        status: wire::PrivacyCaseStatus::Submitted as i32,
        version: i64::try_from(privacy_case.version())
            .map_err(|_| invalid_plan("submitted case version exceeds wire range"))?,
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
        subject_binding: None,
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
        || definition.capability_id.as_str() != SUBMIT_PRIVACY_CASE_CAPABILITY
        || definition.capability_version.as_str() != support::CONTRACT_VERSION
        || request.context.module_id.as_str() != MODULE_ID
        || request.context.execution.capability_id.as_str() != SUBMIT_PRIVACY_CASE_CAPABILITY
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
        "The customer privacy case could not be submitted.",
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
        "CUSTOMER_PRIVACY_CASE_SUBMIT_PLAN_INVALID",
        ErrorCategory::Internal,
        false,
        "The privacy case submission could not be planned safely.",
    )
    .with_internal_reference(reference.into())
}

fn configured<T>(value: Result<T, crm_module_sdk::IdentifierError>) -> Result<T, SdkError> {
    value.map_err(|error| {
        SdkError::new(
            "CUSTOMER_PRIVACY_CASE_SUBMIT_CONFIGURATION_INVALID",
            ErrorCategory::Internal,
            false,
            "The privacy case submit capability is not configured safely.",
        )
        .with_internal_reference(error.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_core_data::RecordMutation;
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
        let command = wire::SubmitPrivacyCaseRequest {
            privacy_case_ref: case_id.map(|value| wire::PrivacyCaseRef {
                privacy_case_id: value.to_owned(),
            }),
            expected_version,
        };
        CapabilityRequest {
            context: ModuleExecutionContext {
                module_id: ModuleId::try_new(MODULE_ID).unwrap(),
                execution: ExecutionContext {
                    tenant_id: TenantId::try_new(tenant).unwrap(),
                    actor_id: ActorId::try_new("privacy-officer").unwrap(),
                    request_id: RequestId::try_new("request-privacy-submit").unwrap(),
                    correlation_id: CorrelationId::try_new("correlation-privacy-submit").unwrap(),
                    causation_id: CausationId::try_new("causation-privacy-submit").unwrap(),
                    trace_id: TraceId::try_new("trace-privacy-submit").unwrap(),
                    capability_id: CapabilityId::try_new(SUBMIT_PRIVACY_CASE_CAPABILITY).unwrap(),
                    capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                    idempotency_key: IdempotencyKey::try_new(idempotency_key).unwrap(),
                    business_transaction_id: BusinessTransactionId::try_new(
                        "transaction-privacy-submit",
                    )
                    .unwrap(),
                    schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                    request_started_at_unix_nanos: started_at,
                },
            },
            input: support::protobuf_payload(
                MODULE_ID,
                SUBMIT_PRIVACY_CASE_REQUEST_SCHEMA,
                DataClass::Confidential,
                &command,
            )
            .unwrap(),
            input_hash: [11; 32],
            approval: None,
        }
    }

    fn snapshot(tenant: &str, case_id: &str, submitted: bool) -> RecordSnapshot {
        let mut case = PrivacyCase::new(
            RecordId::try_new(case_id).unwrap(),
            TenantId::try_new(tenant).unwrap(),
            PrivacyCaseKind::Erasure,
            SchemaVersion::try_new("privacy-policy/1").unwrap(),
            1_000_000_000,
            None,
        )
        .unwrap();
        if submitted {
            case.submit(1, 2_000_000_000).unwrap();
        }
        RecordSnapshot {
            reference: privacy_case_record_ref(&case).unwrap(),
            version: i64::try_from(case.version()).unwrap(),
            payload: privacy_case_persisted_payload(&case).unwrap(),
        }
    }

    #[test]
    fn target_and_plan_are_exact_versioned_update() {
        let definition = capability_definition().unwrap();
        let request = request(
            "tenant-a",
            Some("privacy-case-a"),
            1,
            "submit-a",
            2_000_000_000,
        );
        let current = snapshot("tenant-a", "privacy-case-a", false);
        let target = CustomerPrivacyCaseSubmitCapabilityPlanner
            .target(&definition, &request)
            .unwrap();
        assert_eq!(target.presence, AggregatePresence::MustExist);
        assert_eq!(target.reference, current.reference);

        let plan = CustomerPrivacyCaseSubmitCapabilityPlanner
            .plan(&definition, &request, Some(&current))
            .unwrap();
        assert_eq!(plan.batch.records.len(), 1);
        assert_eq!(plan.batch.events.len(), 1);
        assert_eq!(plan.batch.audits.len(), 1);
        match &plan.batch.records[0] {
            RecordMutation::Update {
                reference,
                expected_version,
                payload,
            } => {
                assert_eq!(reference, &current.reference);
                assert_eq!(*expected_version, 1);
                assert_eq!(payload.data_class, DataClass::Confidential);
                let submitted = privacy_case_from_snapshot(&RecordSnapshot {
                    reference: reference.clone(),
                    version: 2,
                    payload: payload.clone(),
                })
                .unwrap();
                assert_eq!(submitted.status(), PrivacyCaseStatus::Submitted);
                assert_eq!(submitted.version(), 2);
            }
            RecordMutation::Create { .. } => panic!("submit must update the case"),
        }
        assert_eq!(plan.batch.events[0].aggregate_version, 2);
        assert_eq!(
            plan.batch.events[0].event.expected_aggregate_version,
            Some(1)
        );
        let output =
            wire::SubmitPrivacyCaseResponse::decode(plan.output.as_ref().unwrap().bytes.as_slice())
                .unwrap()
                .privacy_case
                .unwrap();
        assert_eq!(output.status, wire::PrivacyCaseStatus::Submitted as i32);
        assert_eq!(output.version, 2);
        assert_eq!(output.updated_at_unix_ms, 2_000);
    }

    #[test]
    fn missing_tenant_stale_and_wrong_state_fail_closed() {
        let definition = capability_definition().unwrap();
        let base_request = request(
            "tenant-a",
            Some("privacy-case-a"),
            1,
            "submit-a",
            2_000_000_000,
        );
        assert_eq!(
            CustomerPrivacyCaseSubmitCapabilityPlanner
                .plan(&definition, &base_request, None)
                .unwrap_err()
                .code,
            "CUSTOMER_PRIVACY_CASE_NOT_FOUND"
        );
        assert_eq!(
            CustomerPrivacyCaseSubmitCapabilityPlanner
                .plan(
                    &definition,
                    &base_request,
                    Some(&snapshot("tenant-b", "privacy-case-a", false)),
                )
                .unwrap_err()
                .code,
            "CUSTOMER_PRIVACY_CASE_NOT_FOUND"
        );

        let stale = request(
            "tenant-a",
            Some("privacy-case-a"),
            2,
            "submit-b",
            2_000_000_000,
        );
        let stale_error = CustomerPrivacyCaseSubmitCapabilityPlanner
            .plan(
                &definition,
                &stale,
                Some(&snapshot("tenant-a", "privacy-case-a", false)),
            )
            .unwrap_err();
        assert_eq!(stale_error.code, "CUSTOMER_PRIVACY_VERSION_CONFLICT");
        assert!(stale_error.retryable);

        let wrong_state = request(
            "tenant-a",
            Some("privacy-case-a"),
            2,
            "submit-c",
            3_000_000_000,
        );
        let wrong_state_error = CustomerPrivacyCaseSubmitCapabilityPlanner
            .plan(
                &definition,
                &wrong_state,
                Some(&snapshot("tenant-a", "privacy-case-a", true)),
            )
            .unwrap_err();
        assert_eq!(
            wrong_state_error.code,
            "CUSTOMER_PRIVACY_INVALID_TRANSITION"
        );
        assert!(!wrong_state_error.retryable);
    }

    #[test]
    fn malformed_state_and_invalid_request_are_bounded() {
        let definition = capability_definition().unwrap();
        let base_request = request(
            "tenant-a",
            Some("privacy-case-a"),
            1,
            "submit-a",
            2_000_000_000,
        );
        let mut malformed = snapshot("tenant-a", "privacy-case-a", false);
        malformed.payload.bytes = b"{\"raw_secret\":\"must-not-leak\"}".to_vec();
        let error = CustomerPrivacyCaseSubmitCapabilityPlanner
            .plan(&definition, &base_request, Some(&malformed))
            .unwrap_err();
        assert_eq!(error.code, "CUSTOMER_PRIVACY_CASE_INVALID");
        assert_eq!(
            error.safe_message,
            "The privacy case could not be loaded safely."
        );
        assert!(!error.safe_message.contains("raw_secret"));

        let missing_ref = request("tenant-a", None, 1, "submit-b", 2_000_000_000);
        assert_eq!(
            CustomerPrivacyCaseSubmitCapabilityPlanner
                .target(&definition, &missing_ref)
                .unwrap_err()
                .code,
            "SDK_INVALID_ARGUMENT"
        );
        let invalid_version = request(
            "tenant-a",
            Some("privacy-case-a"),
            0,
            "submit-c",
            2_000_000_000,
        );
        assert_eq!(
            CustomerPrivacyCaseSubmitCapabilityPlanner
                .target(&definition, &invalid_version)
                .unwrap_err()
                .code,
            "SDK_INVALID_ARGUMENT"
        );
    }
}
