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
        ensure_definition(definition, request)?;
        let command = submit_command(request)?;
        Ok(AggregateTarget {
            reference: privacy_case_ref_from_wire(
                command
                    .privacy_case_ref
                    .ok_or_else(|| missing_reference("customer_privacy.privacy_case_ref"))?,
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
        ensure_definition(definition, request)?;
        let command = submit_command(request)?;
        let requested_reference = privacy_case_ref_from_wire(
            command
                .privacy_case_ref
                .ok_or_else(|| missing_reference("customer_privacy.privacy_case_ref"))?,
        )?;
        let current = current.ok_or_else(case_not_found)?;
        if current.reference != requested_reference {
            return Err(case_not_found());
        }

        let mut privacy_case = privacy_case_from_snapshot(current).map_err(case_state_invalid)?;
        if privacy_case.case_id() != &requested_reference.record_id
            || privacy_case.tenant_id() != &request.context.execution.tenant_id
        {
            return Err(case_not_found());
        }

        let expected_version = expected_version(command.expected_version)?;
        let previous_version = i64::try_from(privacy_case.version())
            .map_err(|_| plan_invalid("privacy case version exceeds the record envelope range"))?;
        privacy_case
            .submit(
                expected_version,
                request.context.execution.request_started_at_unix_nanos,
            )
            .map_err(domain_error)?;
        plan_case_submit(definition, request, current, privacy_case, previous_version)
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
    let command = support::decode_request(request, MODULE_ID, SUBMIT_PRIVACY_CASE_REQUEST_SCHEMA)?;
    expected_version(command.expected_version)?;
    Ok(command)
}

fn plan_case_submit(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    current: &RecordSnapshot,
    privacy_case: PrivacyCase,
    previous_version: i64,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    if privacy_case.status() != PrivacyCaseStatus::Submitted {
        return Err(plan_invalid(
            "case.submit output must contain a submitted privacy case",
        ));
    }
    let next_version = i64::try_from(privacy_case.version())
        .map_err(|_| plan_invalid("privacy case version exceeds the wire range"))?;
    if next_version != previous_version + 1 || current.version != previous_version {
        return Err(plan_invalid(
            "case.submit must advance the authoritative aggregate by exactly one version",
        ));
    }
    if privacy_case.subject_binding().is_some()
        || privacy_case.pending_rescope().is_some()
        || privacy_case.scope_snapshot_id().is_some()
        || privacy_case.action_plan_id().is_some()
        || privacy_case.approval().is_some()
    {
        return Err(plan_invalid(
            "submitted privacy case contains lifecycle evidence from a later phase",
        ));
    }

    let aggregate = privacy_case_record_ref(&privacy_case)?;
    if aggregate != current.reference {
        return Err(case_not_found());
    }
    let public_case = submitted_case_to_wire(
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

fn submitted_case_to_wire(
    privacy_case: &PrivacyCase,
    updated_at_unix_nanos: i64,
) -> Result<wire::PrivacyCase, SdkError> {
    let version = i64::try_from(privacy_case.version())
        .map_err(|_| plan_invalid("privacy case version exceeds the wire range"))?;
    Ok(wire::PrivacyCase {
        privacy_case_ref: Some(wire::PrivacyCaseRef {
            privacy_case_id: privacy_case.case_id().as_str().to_owned(),
        }),
        kind: privacy_case_kind_to_wire(privacy_case.kind()),
        status: wire::PrivacyCaseStatus::Submitted as i32,
        version,
        policy_version: privacy_case.policy_version().as_str().to_owned(),
        created_at_unix_ms: nanos_to_millis(
            privacy_case.created_at_unix_nanos(),
            "customer_privacy.case.created_at",
        )?,
        updated_at_unix_ms: nanos_to_millis(
            updated_at_unix_nanos,
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

fn ensure_definition(
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
        return Err(plan_invalid(
            "capability definition does not match the exact request coordinate",
        ));
    }
    Ok(())
}

fn expected_version(value: i64) -> Result<u64, SdkError> {
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

fn privacy_case_ref_from_wire(reference: wire::PrivacyCaseRef) -> Result<RecordRef, SdkError> {
    let case_id = RecordId::try_new(reference.privacy_case_id).map_err(|error| {
        SdkError::invalid_argument(
            "customer_privacy.privacy_case_ref.privacy_case_id",
            error.to_string(),
        )
    })?;
    support::record_ref(
        PRIVACY_CASE_RECORD_TYPE,
        case_id.as_str(),
        "customer_privacy.privacy_case_ref.privacy_case_id",
    )
}

fn privacy_case_kind_to_wire(value: PrivacyCaseKind) -> i32 {
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

fn missing_reference(field: &'static str) -> SdkError {
    SdkError::invalid_argument(field, "Privacy case reference is required.")
}

fn plan_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_CASE_SUBMIT_PLAN_INVALID",
        ErrorCategory::Internal,
        false,
        "The privacy case submission could not be planned safely.",
    )
    .with_internal_reference(reference.into())
}

fn configured<T>(value: Result<T, crm_module_sdk::IdentifierError>) -> Result<T, SdkError> {
    value.map_err(configuration_error)
}

fn configuration_error(error: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_CASE_SUBMIT_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The privacy case submit capability is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_capability_runtime::CapabilityRequest;
    use crm_core_data::RecordMutation;
    use crm_customer_privacy_persistence_adapter::privacy_case_persisted_payload;
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

    fn draft_snapshot(tenant: &str, case_id: &str) -> RecordSnapshot {
        let case = PrivacyCase::new(
            RecordId::try_new(case_id).unwrap(),
            TenantId::try_new(tenant).unwrap(),
            PrivacyCaseKind::Erasure,
            SchemaVersion::try_new("privacy-policy/1").unwrap(),
            1_000_000_000,
            None,
        )
        .unwrap();
        RecordSnapshot {
            reference: privacy_case_record_ref(&case).unwrap(),
            version: 1,
            payload: privacy_case_persisted_payload(&case).unwrap(),
        }
    }

    fn submitted_snapshot(tenant: &str, case_id: &str) -> RecordSnapshot {
        let mut case = PrivacyCase::new(
            RecordId::try_new(case_id).unwrap(),
            TenantId::try_new(tenant).unwrap(),
            PrivacyCaseKind::Erasure,
            SchemaVersion::try_new("privacy-policy/1").unwrap(),
            1_000_000_000,
            None,
        )
        .unwrap();
        case.submit(1, 2_000_000_000).unwrap();
        RecordSnapshot {
            reference: privacy_case_record_ref(&case).unwrap(),
            version: 2,
            payload: privacy_case_persisted_payload(&case).unwrap(),
        }
    }

    #[test]
    fn target_is_exact_must_exist_case() {
        let request = request(
            "tenant-a",
            Some("privacy-case-a"),
            1,
            "submit-a",
            2_000_000_000,
        );
        let target = CustomerPrivacyCaseSubmitCapabilityPlanner
            .target(&capability_definition().unwrap(), &request)
            .unwrap();
        assert_eq!(target.presence, AggregatePresence::MustExist);
        assert_eq!(
            target.reference.record_type.as_str(),
            PRIVACY_CASE_RECORD_TYPE
        );
        assert_eq!(target.reference.record_id.as_str(), "privacy-case-a");
    }

    #[test]
    fn submit_is_one_confidential_versioned_atomic_update() {
        let request = request(
            "tenant-a",
            Some("privacy-case-a"),
            1,
            "submit-a",
            2_000_000_000,
        );
        let current = draft_snapshot("tenant-a", "privacy-case-a");
        let definition = capability_definition().unwrap();
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
                let updated = privacy_case_from_snapshot(&RecordSnapshot {
                    reference: reference.clone(),
                    version: 2,
                    payload: payload.clone(),
                })
                .unwrap();
                assert_eq!(updated.status(), PrivacyCaseStatus::Submitted);
                assert_eq!(updated.version(), 2);
            }
            RecordMutation::Create { .. } => panic!("submit must update the existing case"),
        }
        assert_eq!(plan.batch.events[0].aggregate_version, 2);
        assert_eq!(
            plan.batch.events[0].event.expected_aggregate_version,
            Some(1)
        );
        assert_eq!(
            plan.batch.events[0].event.event_type.as_str(),
            PRIVACY_CASE_STATUS_CHANGED_EVENT_TYPE
        );

        let output =
            wire::SubmitPrivacyCaseResponse::decode(plan.output.as_ref().unwrap().bytes.as_slice())
                .unwrap()
                .privacy_case
                .unwrap();
        assert_eq!(output.status, wire::PrivacyCaseStatus::Submitted as i32);
        assert_eq!(output.version, 2);
        assert_eq!(output.updated_at_unix_ms, 2_000);

        let event = wire::PrivacyCaseStatusChangedEvent::decode(
            plan.batch.events[0].event.payload.bytes.as_slice(),
        )
        .unwrap()
        .privacy_case
        .unwrap();
        assert_eq!(event, output);
    }

    #[test]
    fn missing_cross_tenant_stale_and_wrong_state_fail_closed() {
        let definition = capability_definition().unwrap();
        let request = request(
            "tenant-a",
            Some("privacy-case-a"),
            1,
            "submit-a",
            2_000_000_000,
        );
        let missing = CustomerPrivacyCaseSubmitCapabilityPlanner
            .plan(&definition, &request, None)
            .unwrap_err();
        assert_eq!(missing.code, "CUSTOMER_PRIVACY_CASE_NOT_FOUND");

        let cross_tenant = draft_snapshot("tenant-b", "privacy-case-a");
        let concealed = CustomerPrivacyCaseSubmitCapabilityPlanner
            .plan(&definition, &request, Some(&cross_tenant))
            .unwrap_err();
        assert_eq!(concealed.code, "CUSTOMER_PRIVACY_CASE_NOT_FOUND");

        let stale_request = request(
            "tenant-a",
            Some("privacy-case-a"),
            2,
            "submit-b",
            2_000_000_000,
        );
        let stale = CustomerPrivacyCaseSubmitCapabilityPlanner
            .plan(
                &definition,
                &stale_request,
                Some(&draft_snapshot("tenant-a", "privacy-case-a")),
            )
            .unwrap_err();
        assert_eq!(stale.code, "CUSTOMER_PRIVACY_VERSION_CONFLICT");
        assert!(stale.retryable);

        let wrong_state_request = request(
            "tenant-a",
            Some("privacy-case-a"),
            2,
            "submit-c",
            3_000_000_000,
        );
        let wrong_state = CustomerPrivacyCaseSubmitCapabilityPlanner
            .plan(
                &definition,
                &wrong_state_request,
                Some(&submitted_snapshot("tenant-a", "privacy-case-a")),
            )
            .unwrap_err();
        assert_eq!(wrong_state.code, "CUSTOMER_PRIVACY_INVALID_TRANSITION");
        assert!(!wrong_state.retryable);
    }

    #[test]
    fn malformed_state_and_invalid_request_are_bounded() {
        let definition = capability_definition().unwrap();
        let request = request(
            "tenant-a",
            Some("privacy-case-a"),
            1,
            "submit-a",
            2_000_000_000,
        );
        let mut malformed = draft_snapshot("tenant-a", "privacy-case-a");
        malformed.payload.bytes = b"{\"raw_secret\":\"must-not-leak\"}".to_vec();
        let error = CustomerPrivacyCaseSubmitCapabilityPlanner
            .plan(&definition, &request, Some(&malformed))
            .unwrap_err();
        assert_eq!(error.code, "CUSTOMER_PRIVACY_CASE_INVALID");
        assert_eq!(
            error.safe_message,
            "The privacy case could not be loaded safely."
        );
        assert!(!error.safe_message.contains("raw_secret"));

        let no_ref = request("tenant-a", None, 1, "submit-b", 2_000_000_000);
        assert!(
            CustomerPrivacyCaseSubmitCapabilityPlanner
                .target(&definition, &no_ref)
                .is_err()
        );
        let invalid_version = request(
            "tenant-a",
            Some("privacy-case-a"),
            0,
            "submit-c",
            2_000_000_000,
        );
        let invalid = CustomerPrivacyCaseSubmitCapabilityPlanner
            .target(&definition, &invalid_version)
            .unwrap_err();
        assert_eq!(invalid.code, "INVALID_ARGUMENT");
    }
}
