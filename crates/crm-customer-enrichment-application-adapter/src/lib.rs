#![forbid(unsafe_code)]

//! Non-runtime deterministic planning for governed application of accepted enrichment suggestions.
//!
//! The public apply coordinate records one immutable application-attempt identity before any
//! authoritative Party mutation is invoked. The worker-only outcome coordinate then appends one
//! terminal or retryable result to that exact attempt. Both plans are idempotent, audited and emit
//! bounded Personal evidence without writing Party-owned state directly.

use crm_capability_plan_support::{self as support, EventSpec, PersistedPayloadContract};
use crm_capability_runtime::{
    CapabilityDefinition, CapabilityRequest, CapabilityRisk, PayloadContract,
};
use crm_core_data::{
    AggregatePresence, AggregateTarget, BatchMutationPlan, CapabilityBatchExecutionPlan,
    RecordMutation, TransactionalAggregatePlanner,
};
use crm_customer_enrichment::{
    APPLICATION_ATTEMPT_RECORD_TYPE, APPLICATION_ATTEMPT_STATE_MAXIMUM_BYTES,
    APPLICATION_ATTEMPT_STATE_SCHEMA_ID, ApplicationAttempt, ApplicationOutcome,
    LIFECYCLE_STATE_RETENTION_POLICY_ID, LIFECYCLE_STATE_SCHEMA_VERSION, RecordedApplicationOutcome,
    ReplayDisposition, ReviewDecision, Suggestion, SuggestionLifecycleStatus, TargetField,
    application_attempt_state_descriptor_hash, decode_application_attempt_state,
    derive_suggestion_status, encode_application_attempt_state,
};
use crm_customer_enrichment_capability_adapter::MODULE_ID;
use crm_customer_enrichment_review_adapter::{
    review_decision_from_snapshot, suggestion_from_snapshot, suggestion_to_wire,
};
use crm_module_sdk::{
    BusinessTransactionId, CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId,
    RecordId, RecordRef, RecordSnapshot, SdkError, TypedPayload,
};
use crm_proto_contracts::crm::{customer::v1 as customer, customer_enrichment::v1 as wire};
use serde::Deserialize;

pub const CRATE_NAME: &str = "crm-customer-enrichment-application-adapter";

pub const APPLY_PARTY_DISPLAY_NAME_CAPABILITY: &str =
    "customer_enrichment.party.display_name.apply";
pub const APPLY_PARTY_DISPLAY_NAME_REQUEST_SCHEMA: &str =
    "crm.customer_enrichment.v1.ApplyPartyDisplayNameSuggestionRequest";
pub const APPLY_PARTY_DISPLAY_NAME_RESPONSE_SCHEMA: &str =
    "crm.customer_enrichment.v1.ApplyPartyDisplayNameSuggestionResponse";
pub const RECORD_APPLICATION_OUTCOME_CAPABILITY: &str =
    "customer_enrichment.application.outcome.record";
pub const RECORD_APPLICATION_OUTCOME_REQUEST_SCHEMA: &str =
    "crm.customer_enrichment.v1.RecordApplicationOutcomeRequest";
pub const RECORD_APPLICATION_OUTCOME_RESPONSE_SCHEMA: &str =
    "crm.customer_enrichment.v1.RecordApplicationOutcomeResponse";
pub const SUGGESTION_APPLICATION_RECORDED_EVENT_TYPE: &str =
    "customer_enrichment.suggestion.application_recorded";
pub const SUGGESTION_APPLICATION_RECORDED_EVENT_SCHEMA: &str =
    "crm.customer_enrichment.v1.SuggestionApplicationRecordedEvent";

pub fn application_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    Ok(vec![
        apply_party_display_name_capability_definition()?,
        record_application_outcome_capability_definition()?,
    ])
}

pub fn apply_party_display_name_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    application_definition(
        APPLY_PARTY_DISPLAY_NAME_CAPABILITY,
        APPLY_PARTY_DISPLAY_NAME_REQUEST_SCHEMA,
        APPLY_PARTY_DISPLAY_NAME_RESPONSE_SCHEMA,
        CapabilityRisk::High,
    )
}

pub fn record_application_outcome_capability_definition() -> Result<CapabilityDefinition, SdkError>
{
    application_definition(
        RECORD_APPLICATION_OUTCOME_CAPABILITY,
        RECORD_APPLICATION_OUTCOME_REQUEST_SCHEMA,
        RECORD_APPLICATION_OUTCOME_RESPONSE_SCHEMA,
        CapabilityRisk::Medium,
    )
}

fn application_definition(
    capability_id: &'static str,
    request_schema: &'static str,
    response_schema: &'static str,
    risk: CapabilityRisk,
) -> Result<CapabilityDefinition, SdkError> {
    Ok(CapabilityDefinition {
        capability_id: configured(CapabilityId::try_new(capability_id))?,
        capability_version: configured(CapabilityVersion::try_new(support::CONTRACT_VERSION))?,
        owner_module_id: configured(ModuleId::try_new(MODULE_ID))?,
        input_contract: application_contract(request_schema)?,
        output_contract: Some(application_contract(response_schema)?),
        risk,
        mutation: true,
        requires_idempotency: true,
        requires_approval: false,
        authorization_policy_id: capability_id.to_owned(),
        rate_limit_policy_id: None,
    })
}

fn application_contract(schema: &'static str) -> Result<PayloadContract, SdkError> {
    support::protobuf_contract(MODULE_ID, schema, vec![DataClass::Personal])
}

/// Plans one deterministic pending application attempt over an exact suggestion and accepted review.
#[derive(Debug, Clone)]
pub struct CustomerEnrichmentApplicationAttemptPlanner {
    suggestion: Suggestion,
    review_decision: ReviewDecision,
}

impl CustomerEnrichmentApplicationAttemptPlanner {
    pub fn new(suggestion: Suggestion, review_decision: ReviewDecision) -> Self {
        Self {
            suggestion,
            review_decision,
        }
    }

    fn application_attempt(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<ApplicationAttempt, SdkError> {
        ensure_definition(
            definition,
            request,
            APPLY_PARTY_DISPLAY_NAME_CAPABILITY,
        )?;
        let command: wire::ApplyPartyDisplayNameSuggestionRequest =
            support::decode_request_with_data_class(
                request,
                MODULE_ID,
                APPLY_PARTY_DISPLAY_NAME_REQUEST_SCHEMA,
                DataClass::Personal,
            )?;
        ensure_suggestion_ref(command.suggestion_ref, &self.suggestion)?;
        ensure_review_ref(command.review_decision_ref, &self.review_decision)?;
        let expected_version = positive_u64(
            command.expected_party_resource_version,
            "customer_enrichment.expected_party_resource_version",
        )?;
        if expected_version != self.suggestion.target().resource_version {
            return Err(application_conflict(
                "expected Party version differs from the immutable suggestion target",
            ));
        }
        ApplicationAttempt::plan(
            request.context.execution.tenant_id.clone(),
            &self.suggestion,
            &self.review_decision,
            command.application_generation,
            request_started_at_unix_ms(request)?,
        )
    }
}

impl TransactionalAggregatePlanner for CustomerEnrichmentApplicationAttemptPlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        let attempt = self.application_attempt(definition, request)?;
        Ok(AggregateTarget {
            reference: application_attempt_record_ref(&attempt)?,
            presence: AggregatePresence::MustBeAbsent,
        })
    }

    fn plan(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
        current: Option<&RecordSnapshot>,
    ) -> Result<CapabilityBatchExecutionPlan, SdkError> {
        let attempt = self.application_attempt(definition, request)?;
        let reference = application_attempt_record_ref(&attempt)?;
        if current.is_some() {
            return Err(application_conflict(
                "the deterministic application-attempt record already exists",
            ));
        }
        let public_attempt = application_attempt_to_wire(&attempt)?;
        let mut public_suggestion = suggestion_to_wire(
            &self.suggestion,
            Some(&self.review_decision),
            request_started_at_unix_ms(request)?,
        )?;
        public_suggestion.lifecycle_status = suggestion_status_to_wire(derive_suggestion_status(
            &self.suggestion,
            Some(&self.review_decision),
            Some(&attempt),
            None,
            request_started_at_unix_ms(request)?,
        ));
        let output = support::protobuf_payload(
            MODULE_ID,
            APPLY_PARTY_DISPLAY_NAME_RESPONSE_SCHEMA,
            DataClass::Personal,
            &wire::ApplyPartyDisplayNameSuggestionResponse {
                application_attempt: Some(public_attempt.clone()),
            },
        )?;
        let event = support::event_evidence_with_data_class(
            request,
            reference.clone(),
            MODULE_ID,
            EventSpec {
                event_type: SUGGESTION_APPLICATION_RECORDED_EVENT_TYPE,
                event_schema_id: SUGGESTION_APPLICATION_RECORDED_EVENT_SCHEMA,
                aggregate_version: 1,
                previous_version: None,
            },
            DataClass::Personal,
            &wire::SuggestionApplicationRecordedEvent {
                suggestion: Some(public_suggestion),
                application_attempt: Some(public_attempt),
            },
        )?;
        Ok(CapabilityBatchExecutionPlan {
            batch: BatchMutationPlan {
                context: request.context.clone(),
                records: vec![RecordMutation::Create {
                    reference: reference.clone(),
                    payload: application_attempt_persisted_payload(&attempt)?,
                }],
                relationships: Vec::new(),
                events: vec![event],
                idempotency: support::capability_idempotency(definition, request)?,
                audits: vec![support::audit_intent(
                    request,
                    &reference,
                    1,
                    definition.capability_id.as_str(),
                    &output.bytes,
                )?],
            },
            output: Some(output),
        })
    }
}

/// Appends one exact outcome to a previously planned application attempt.
#[derive(Debug, Clone)]
pub struct CustomerEnrichmentApplicationOutcomePlanner {
    suggestion: Suggestion,
    review_decision: ReviewDecision,
}

impl CustomerEnrichmentApplicationOutcomePlanner {
    pub fn new(suggestion: Suggestion, review_decision: ReviewDecision) -> Self {
        Self {
            suggestion,
            review_decision,
        }
    }
}

impl TransactionalAggregatePlanner for CustomerEnrichmentApplicationOutcomePlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        ensure_definition(
            definition,
            request,
            RECORD_APPLICATION_OUTCOME_CAPABILITY,
        )?;
        Ok(AggregateTarget {
            reference: outcome_attempt_record_ref(request)?,
            presence: AggregatePresence::MustExist,
        })
    }

    fn plan(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
        current: Option<&RecordSnapshot>,
    ) -> Result<CapabilityBatchExecutionPlan, SdkError> {
        ensure_definition(
            definition,
            request,
            RECORD_APPLICATION_OUTCOME_CAPABILITY,
        )?;
        let current = current.ok_or_else(application_attempt_not_found)?;
        let expected_reference = outcome_attempt_record_ref(request)?;
        if current.reference != expected_reference {
            return Err(application_state_invalid(
                "locked application attempt differs from the requested attempt",
            ));
        }
        let mut attempt = application_attempt_from_snapshot(current)?;
        ensure_attempt_lineage(&attempt, &self.suggestion, &self.review_decision)?;
        let command: wire::RecordApplicationOutcomeRequest =
            support::decode_request_with_data_class(
                request,
                MODULE_ID,
                RECORD_APPLICATION_OUTCOME_REQUEST_SCHEMA,
                DataClass::Personal,
            )?;
        let outcome = application_outcome_from_wire(command.outcome)?;
        let recorded_at_unix_ms = non_negative_u64(
            command.recorded_at_unix_ms,
            "customer_enrichment.recorded_at_unix_ms",
        )?;
        let disposition = attempt.record_outcome(outcome, recorded_at_unix_ms)?;
        let public_attempt = application_attempt_to_wire(&attempt)?;
        let output = support::protobuf_payload(
            MODULE_ID,
            RECORD_APPLICATION_OUTCOME_RESPONSE_SCHEMA,
            DataClass::Personal,
            &wire::RecordApplicationOutcomeResponse {
                application_attempt: Some(public_attempt.clone()),
            },
        )?;
        let audit_version = match disposition {
            ReplayDisposition::New => current
                .version
                .checked_add(1)
                .ok_or_else(|| application_state_invalid("application attempt version overflow"))?,
            ReplayDisposition::Duplicate => current.version,
        };
        let (records, events) = match disposition {
            ReplayDisposition::New => {
                let mut public_suggestion = suggestion_to_wire(
                    &self.suggestion,
                    Some(&self.review_decision),
                    recorded_at_unix_ms,
                )?;
                public_suggestion.lifecycle_status = suggestion_status_to_wire(
                    derive_suggestion_status(
                        &self.suggestion,
                        Some(&self.review_decision),
                        Some(&attempt),
                        None,
                        recorded_at_unix_ms,
                    ),
                );
                let event = support::event_evidence_with_data_class(
                    request,
                    current.reference.clone(),
                    MODULE_ID,
                    EventSpec {
                        event_type: SUGGESTION_APPLICATION_RECORDED_EVENT_TYPE,
                        event_schema_id: SUGGESTION_APPLICATION_RECORDED_EVENT_SCHEMA,
                        aggregate_version: audit_version,
                        previous_version: Some(current.version),
                    },
                    DataClass::Personal,
                    &wire::SuggestionApplicationRecordedEvent {
                        suggestion: Some(public_suggestion),
                        application_attempt: Some(public_attempt),
                    },
                )?;
                (
                    vec![RecordMutation::Update {
                        reference: current.reference.clone(),
                        expected_version: current.version,
                        payload: application_attempt_persisted_payload(&attempt)?,
                    }],
                    vec![event],
                )
            }
            ReplayDisposition::Duplicate => (Vec::new(), Vec::new()),
        };
        Ok(CapabilityBatchExecutionPlan {
            batch: BatchMutationPlan {
                context: request.context.clone(),
                records,
                relationships: Vec::new(),
                events,
                idempotency: support::capability_idempotency(definition, request)?,
                audits: vec![support::audit_intent(
                    request,
                    &current.reference,
                    audit_version,
                    definition.capability_id.as_str(),
                    &output.bytes,
                )?],
            },
            output: Some(output),
        })
    }
}

pub fn application_attempt_persisted_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: APPLICATION_ATTEMPT_STATE_SCHEMA_ID,
        schema_version: LIFECYCLE_STATE_SCHEMA_VERSION,
        descriptor_hash: application_attempt_state_descriptor_hash(),
        maximum_size_bytes: APPLICATION_ATTEMPT_STATE_MAXIMUM_BYTES,
        retention_policy_id: LIFECYCLE_STATE_RETENTION_POLICY_ID,
    }
}

pub fn application_attempt_from_snapshot(
    snapshot: &RecordSnapshot,
) -> Result<ApplicationAttempt, SdkError> {
    if snapshot.reference.record_type.as_str() != APPLICATION_ATTEMPT_RECORD_TYPE
        || !(1..=2).contains(&snapshot.version)
    {
        return Err(application_state_invalid(
            "application-attempt record type or append-once version is invalid",
        ));
    }
    let bytes = support::persisted_json_bytes_with_data_class(
        snapshot,
        application_attempt_persisted_contract(),
        DataClass::Personal,
    )?;
    let attempt = decode_application_attempt_state(bytes)?;
    if snapshot.reference.record_id.as_str() != attempt.attempt_id().as_str() {
        return Err(application_state_invalid(
            "application-attempt record identity differs from its deterministic domain identity",
        ));
    }
    if (snapshot.version == 1) != attempt.recorded_outcome().is_none() {
        return Err(application_state_invalid(
            "application-attempt record version disagrees with outcome presence",
        ));
    }
    Ok(attempt)
}

pub fn application_attempt_persisted_payload(
    attempt: &ApplicationAttempt,
) -> Result<TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        application_attempt_persisted_contract(),
        DataClass::Personal,
        encode_application_attempt_state(attempt)?,
    )
}

pub fn application_attempt_record_ref(attempt: &ApplicationAttempt) -> Result<RecordRef, SdkError> {
    support::record_ref(
        APPLICATION_ATTEMPT_RECORD_TYPE,
        attempt.attempt_id().as_str(),
        "customer_enrichment.application_attempt_ref.application_attempt_id",
    )
}

pub fn suggestion_from_application_snapshot(
    snapshot: &RecordSnapshot,
) -> Result<Suggestion, SdkError> {
    suggestion_from_snapshot(snapshot)
}

pub fn review_from_application_snapshot(
    snapshot: &RecordSnapshot,
) -> Result<ReviewDecision, SdkError> {
    review_decision_from_snapshot(snapshot)
}

pub fn application_attempt_to_wire(
    attempt: &ApplicationAttempt,
) -> Result<wire::ApplicationAttempt, SdkError> {
    let state: ApplicationAttemptStateView =
        serde_json::from_slice(&encode_application_attempt_state(attempt)?)
            .map_err(|error| application_state_invalid(error.to_string()))?;
    Ok(wire::ApplicationAttempt {
        application_attempt_ref: Some(wire::ApplicationAttemptRef {
            application_attempt_id: state.attempt_id,
        }),
        suggestion_ref: Some(wire::SuggestionRef {
            suggestion_id: state.suggestion_id,
        }),
        review_decision_ref: Some(wire::ReviewDecisionRef {
            review_decision_id: state.review_decision_id,
        }),
        target: Some(wire::EnrichmentTargetSnapshot {
            party_ref: Some(customer::PartyRef {
                party_id: state.target.resource_id,
            }),
            party_resource_version: checked_i64(
                state.target.resource_version,
                "application target Party version",
            )?,
            target_field: target_field_to_wire(state.target.target_field),
        }),
        proposed_value_digest: state.proposed_value_digest.to_vec(),
        application_generation: state.application_generation,
        owner_capability_id: state.owner_capability_id,
        owner_capability_version: state.owner_capability_version,
        target_idempotency_key: state.target_idempotency_key,
        planned_at_unix_ms: checked_i64(state.planned_at_unix_ms, "application planned timestamp")?,
        recorded_outcome: state
            .recorded_outcome
            .as_ref()
            .map(recorded_application_outcome_to_wire)
            .transpose()?,
    })
}

fn recorded_application_outcome_to_wire(
    recorded: &RecordedApplicationOutcome,
) -> Result<wire::RecordedApplicationOutcome, SdkError> {
    Ok(wire::RecordedApplicationOutcome {
        outcome: Some(application_outcome_to_wire(&recorded.outcome)?),
        recorded_at_unix_ms: checked_i64(
            recorded.recorded_at_unix_ms,
            "application outcome timestamp",
        )?,
    })
}

fn application_outcome_to_wire(
    outcome: &ApplicationOutcome,
) -> Result<wire::ApplicationOutcome, SdkError> {
    use wire::application_outcome::Result as WireResult;
    let result = match outcome {
        ApplicationOutcome::Succeeded {
            business_transaction_id,
            resulting_target_version,
        } => WireResult::Succeeded(wire::ApplicationSucceeded {
            business_transaction_id: business_transaction_id.as_str().to_owned(),
            resulting_party_resource_version: checked_i64(
                *resulting_target_version,
                "resulting Party version",
            )?,
        }),
        ApplicationOutcome::RetryableFailure { safe_code } => {
            WireResult::RetryableFailure(wire::ApplicationRetryableFailure {
                safe_code: safe_code.clone(),
            })
        }
        ApplicationOutcome::TerminalFailure { safe_code } => {
            WireResult::TerminalFailure(wire::ApplicationTerminalFailure {
                safe_code: safe_code.clone(),
            })
        }
        ApplicationOutcome::StaleTarget {
            actual_target_version,
        } => WireResult::StaleTarget(wire::ApplicationStaleTarget {
            actual_party_resource_version: checked_i64(
                *actual_target_version,
                "actual Party version",
            )?,
        }),
        ApplicationOutcome::AuthorizationDenied => {
            WireResult::AuthorizationDenied(wire::ApplicationAuthorizationDenied {})
        }
        ApplicationOutcome::PolicyDenied => {
            WireResult::PolicyDenied(wire::ApplicationPolicyDenied {})
        }
    };
    Ok(wire::ApplicationOutcome {
        result: Some(result),
    })
}

fn application_outcome_from_wire(
    value: Option<wire::ApplicationOutcome>,
) -> Result<ApplicationOutcome, SdkError> {
    use wire::application_outcome::Result as WireResult;
    let result = value
        .and_then(|value| value.result)
        .ok_or_else(|| {
            SdkError::invalid_argument(
                "customer_enrichment.outcome",
                "Application outcome is required",
            )
        })?;
    match result {
        WireResult::Succeeded(value) => Ok(ApplicationOutcome::Succeeded {
            business_transaction_id: BusinessTransactionId::try_new(value.business_transaction_id)
                .map_err(|error| {
                    SdkError::invalid_argument(
                        "customer_enrichment.outcome.succeeded.business_transaction_id",
                        error.to_string(),
                    )
                })?,
            resulting_target_version: positive_u64(
                value.resulting_party_resource_version,
                "customer_enrichment.outcome.succeeded.resulting_party_resource_version",
            )?,
        }),
        WireResult::RetryableFailure(value) => Ok(ApplicationOutcome::RetryableFailure {
            safe_code: value.safe_code,
        }),
        WireResult::TerminalFailure(value) => Ok(ApplicationOutcome::TerminalFailure {
            safe_code: value.safe_code,
        }),
        WireResult::StaleTarget(value) => Ok(ApplicationOutcome::StaleTarget {
            actual_target_version: positive_u64(
                value.actual_party_resource_version,
                "customer_enrichment.outcome.stale_target.actual_party_resource_version",
            )?,
        }),
        WireResult::AuthorizationDenied(_) => Ok(ApplicationOutcome::AuthorizationDenied),
        WireResult::PolicyDenied(_) => Ok(ApplicationOutcome::PolicyDenied),
    }
}

fn outcome_attempt_record_ref(request: &CapabilityRequest) -> Result<RecordRef, SdkError> {
    let command: wire::RecordApplicationOutcomeRequest = support::decode_request_with_data_class(
        request,
        MODULE_ID,
        RECORD_APPLICATION_OUTCOME_REQUEST_SCHEMA,
        DataClass::Personal,
    )?;
    let reference = command.application_attempt_ref.ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_enrichment.application_attempt_ref",
            "Application-attempt reference is required",
        )
    })?;
    support::record_ref(
        APPLICATION_ATTEMPT_RECORD_TYPE,
        RecordId::try_new(reference.application_attempt_id)
            .map_err(|error| {
                SdkError::invalid_argument(
                    "customer_enrichment.application_attempt_ref.application_attempt_id",
                    error.to_string(),
                )
            })?
            .as_str(),
        "customer_enrichment.application_attempt_ref.application_attempt_id",
    )
}

fn ensure_attempt_lineage(
    attempt: &ApplicationAttempt,
    suggestion: &Suggestion,
    review: &ReviewDecision,
) -> Result<(), SdkError> {
    let state: ApplicationAttemptStateView =
        serde_json::from_slice(&encode_application_attempt_state(attempt)?)
            .map_err(|error| application_state_invalid(error.to_string()))?;
    if state.suggestion_id != suggestion.suggestion_id().as_str()
        || state.review_decision_id != review.decision_id().as_str()
        || state.target.resource_id != suggestion.target().resource_id.as_str()
        || state.target.resource_version != suggestion.target().resource_version
        || state.target.target_field != suggestion.target().target_field
        || state.proposed_value_digest != *suggestion.proposed_value_digest()
    {
        return Err(application_state_invalid(
            "application attempt does not bind the exact suggestion and review lineage",
        ));
    }
    Ok(())
}

fn ensure_suggestion_ref(
    reference: Option<wire::SuggestionRef>,
    suggestion: &Suggestion,
) -> Result<(), SdkError> {
    let reference = reference.ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_enrichment.suggestion_ref",
            "Suggestion reference is required",
        )
    })?;
    let requested = RecordId::try_new(reference.suggestion_id).map_err(|error| {
        SdkError::invalid_argument(
            "customer_enrichment.suggestion_ref.suggestion_id",
            error.to_string(),
        )
    })?;
    if requested.as_str() != suggestion.suggestion_id().as_str() {
        return Err(application_conflict(
            "application request does not reference the exact immutable suggestion",
        ));
    }
    Ok(())
}

fn ensure_review_ref(
    reference: Option<wire::ReviewDecisionRef>,
    review: &ReviewDecision,
) -> Result<(), SdkError> {
    let reference = reference.ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_enrichment.review_decision_ref",
            "Review-decision reference is required",
        )
    })?;
    let requested = RecordId::try_new(reference.review_decision_id).map_err(|error| {
        SdkError::invalid_argument(
            "customer_enrichment.review_decision_ref.review_decision_id",
            error.to_string(),
        )
    })?;
    if requested.as_str() != review.decision_id().as_str() {
        return Err(application_conflict(
            "application request does not reference the exact immutable review decision",
        ));
    }
    Ok(())
}

fn ensure_definition(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    capability_id: &'static str,
) -> Result<(), SdkError> {
    if definition.capability_id.as_str() != capability_id
        || definition.owner_module_id.as_str() != MODULE_ID
        || definition.capability_id != request.context.execution.capability_id
        || definition.capability_version != request.context.execution.capability_version
    {
        return Err(application_plan_invalid(
            "capability definition does not match request context",
        ));
    }
    Ok(())
}

fn request_started_at_unix_ms(request: &CapabilityRequest) -> Result<u64, SdkError> {
    let nanos = request.context.execution.request_started_at_unix_nanos;
    if nanos < 0 {
        return Err(SdkError::invalid_argument(
            "request_started_at_unix_nanos",
            "Request start timestamp must not be negative",
        ));
    }
    u64::try_from(nanos / 1_000_000)
        .map_err(|_| application_plan_invalid("request timestamp exceeds u64"))
}

fn positive_u64(value: i64, field: &'static str) -> Result<u64, SdkError> {
    let value = non_negative_u64(value, field)?;
    if value == 0 {
        return Err(SdkError::invalid_argument(
            field,
            "Value must be greater than zero",
        ));
    }
    Ok(value)
}

fn non_negative_u64(value: i64, field: &'static str) -> Result<u64, SdkError> {
    u64::try_from(value)
        .map_err(|_| SdkError::invalid_argument(field, "Value must not be negative"))
}

fn checked_i64(value: u64, label: &'static str) -> Result<i64, SdkError> {
    i64::try_from(value).map_err(|_| application_state_invalid(format!("{label} exceeds i64")))
}

fn target_field_to_wire(value: TargetField) -> i32 {
    match value {
        TargetField::PartyDisplayName => wire::EnrichmentTargetField::PartyDisplayName as i32,
    }
}

fn suggestion_status_to_wire(value: SuggestionLifecycleStatus) -> i32 {
    match value {
        SuggestionLifecycleStatus::Proposed => wire::SuggestionLifecycleStatus::Proposed as i32,
        SuggestionLifecycleStatus::Accepted => wire::SuggestionLifecycleStatus::Accepted as i32,
        SuggestionLifecycleStatus::Rejected => wire::SuggestionLifecycleStatus::Rejected as i32,
        SuggestionLifecycleStatus::Expired => wire::SuggestionLifecycleStatus::Expired as i32,
        SuggestionLifecycleStatus::Superseded => wire::SuggestionLifecycleStatus::Superseded as i32,
        SuggestionLifecycleStatus::Applied => wire::SuggestionLifecycleStatus::Applied as i32,
        SuggestionLifecycleStatus::ApplicationFailedRetryable => {
            wire::SuggestionLifecycleStatus::ApplicationFailedRetryable as i32
        }
        SuggestionLifecycleStatus::ApplicationFailedTerminal => {
            wire::SuggestionLifecycleStatus::ApplicationFailedTerminal as i32
        }
    }
}

fn configured<T>(
    value: Result<T, crm_module_sdk::IdentifierError>,
) -> Result<T, SdkError> {
    value.map_err(|error| {
        SdkError::new(
            "CUSTOMER_ENRICHMENT_APPLICATION_CONFIGURATION_INVALID",
            ErrorCategory::Internal,
            false,
            "The Customer Enrichment application capability is not configured safely.",
        )
        .with_internal_reference(error.to_string())
    })
}

fn application_attempt_not_found() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_APPLICATION_ATTEMPT_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The requested application attempt was not found.",
    )
}

fn application_conflict(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_APPLICATION_CONFLICT",
        ErrorCategory::Conflict,
        false,
        "The application request no longer matches its exact suggestion, review or target preconditions.",
    )
    .with_internal_reference(reference.into())
}

fn application_plan_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_APPLICATION_PLAN_INVALID",
        ErrorCategory::Internal,
        false,
        "The Customer Enrichment application could not be planned safely.",
    )
    .with_internal_reference(reference.into())
}

fn application_state_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_APPLICATION_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "Stored Customer Enrichment application evidence is invalid.",
    )
    .with_internal_reference(reference.into())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ApplicationAttemptStateView {
    attempt_id: String,
    tenant_id: String,
    suggestion_id: String,
    review_decision_id: String,
    target: ApplicationTargetStateView,
    proposed_value_digest: [u8; 32],
    application_generation: u32,
    owner_capability_id: String,
    owner_capability_version: String,
    target_idempotency_key: String,
    planned_at_unix_ms: u64,
    recorded_outcome: Option<RecordedApplicationOutcome>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ApplicationTargetStateView {
    resource_id: String,
    resource_version: u64,
    target_field: TargetField,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn definitions_are_exact_personal_mutations() {
        let definitions = application_capability_definitions().unwrap();
        assert_eq!(definitions.len(), 2);
        assert_eq!(definitions[0].risk, CapabilityRisk::High);
        assert_eq!(definitions[1].risk, CapabilityRisk::Medium);
        for definition in definitions {
            assert_eq!(definition.owner_module_id.as_str(), MODULE_ID);
            assert_eq!(definition.capability_version.as_str(), "1.0.0");
            assert_eq!(
                definition.input_contract.allowed_data_classes,
                vec![DataClass::Personal]
            );
            assert!(definition.mutation);
            assert!(definition.requires_idempotency);
            assert!(!definition.requires_approval);
        }
    }

    #[test]
    fn persisted_contract_is_exact_and_bounded() {
        let contract = application_attempt_persisted_contract();
        assert_eq!(contract.owner, MODULE_ID);
        assert_eq!(contract.schema_id, APPLICATION_ATTEMPT_STATE_SCHEMA_ID);
        assert_eq!(contract.schema_version, LIFECYCLE_STATE_SCHEMA_VERSION);
        assert_eq!(
            contract.maximum_size_bytes,
            APPLICATION_ATTEMPT_STATE_MAXIMUM_BYTES
        );
        assert!(contract.descriptor_hash.iter().any(|byte| *byte != 0));
    }
}
