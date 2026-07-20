#![forbid(unsafe_code)]

//! Non-runtime deterministic review planning for governed Customer Enrichment suggestions.
//!
//! Infrastructure strictly rehydrates one immutable suggestion and resolves the versioned review
//! policy before constructing this planner. The planner validates exact optimistic bindings and
//! commits one immutable review decision with idempotency, outbox and audit evidence atomically.

use crm_capability_plan_support::{self as support, EventSpec, PersistedPayloadContract};
use crm_capability_runtime::{
    CapabilityDefinition, CapabilityRequest, CapabilityRisk, PayloadContract,
};
use crm_core_data::{
    AggregatePresence, AggregateTarget, BatchMutationPlan, CapabilityBatchExecutionPlan,
    RecordMutation, TransactionalAggregatePlanner,
};
use crm_customer_enrichment::{
    ApprovalRequirement, LIFECYCLE_STATE_RETENTION_POLICY_ID, LIFECYCLE_STATE_SCHEMA_VERSION,
    REVIEW_DECISION_RECORD_TYPE, REVIEW_DECISION_STATE_MAXIMUM_BYTES,
    REVIEW_DECISION_STATE_SCHEMA_ID, ReviewDecision, ReviewDecisionKind, SUGGESTION_RECORD_TYPE,
    SUGGESTION_STATE_MAXIMUM_BYTES, SUGGESTION_STATE_SCHEMA_ID, Suggestion, SuggestionId,
    SuggestionLifecycleStatus, TargetField, decode_review_decision_state, decode_suggestion_state,
    derive_suggestion_status, encode_review_decision_state, encode_suggestion_state,
    review_decision_state_descriptor_hash, suggestion_state_descriptor_hash,
};
use crm_customer_enrichment_capability_adapter::MODULE_ID;
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, RecordId, RecordRef,
    RecordSnapshot, SdkError, TypedPayload,
};
use crm_proto_contracts::crm::{customer::v1 as customer, customer_enrichment::v1 as wire};
use serde::Deserialize;

pub const CRATE_NAME: &str = "crm-customer-enrichment-review-adapter";
pub const ACCEPT_SUGGESTION_CAPABILITY: &str = "customer_enrichment.suggestion.accept";
pub const ACCEPT_SUGGESTION_REQUEST_SCHEMA: &str =
    "crm.customer_enrichment.v1.AcceptSuggestionRequest";
pub const ACCEPT_SUGGESTION_RESPONSE_SCHEMA: &str =
    "crm.customer_enrichment.v1.AcceptSuggestionResponse";
pub const REJECT_SUGGESTION_CAPABILITY: &str = "customer_enrichment.suggestion.reject";
pub const REJECT_SUGGESTION_REQUEST_SCHEMA: &str =
    "crm.customer_enrichment.v1.RejectSuggestionRequest";
pub const REJECT_SUGGESTION_RESPONSE_SCHEMA: &str =
    "crm.customer_enrichment.v1.RejectSuggestionResponse";
pub const SUGGESTION_REVIEWED_EVENT_TYPE: &str = "customer_enrichment.suggestion.reviewed";
pub const SUGGESTION_REVIEWED_EVENT_SCHEMA: &str =
    "crm.customer_enrichment.v1.SuggestionReviewedEvent";
pub const REVIEW_CAPABILITY_IDS: &[&str] =
    &[ACCEPT_SUGGESTION_CAPABILITY, REJECT_SUGGESTION_CAPABILITY];

pub fn review_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    Ok(vec![
        accept_suggestion_capability_definition()?,
        reject_suggestion_capability_definition()?,
    ])
}

pub fn accept_suggestion_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    review_definition(
        ACCEPT_SUGGESTION_CAPABILITY,
        ACCEPT_SUGGESTION_REQUEST_SCHEMA,
        ACCEPT_SUGGESTION_RESPONSE_SCHEMA,
    )
}

pub fn reject_suggestion_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    review_definition(
        REJECT_SUGGESTION_CAPABILITY,
        REJECT_SUGGESTION_REQUEST_SCHEMA,
        REJECT_SUGGESTION_RESPONSE_SCHEMA,
    )
}

fn review_definition(
    capability_id: &'static str,
    request_schema: &'static str,
    response_schema: &'static str,
) -> Result<CapabilityDefinition, SdkError> {
    Ok(CapabilityDefinition {
        capability_id: configured(CapabilityId::try_new(capability_id))?,
        capability_version: configured(CapabilityVersion::try_new(support::CONTRACT_VERSION))?,
        owner_module_id: configured(ModuleId::try_new(MODULE_ID))?,
        input_contract: review_contract(request_schema)?,
        output_contract: Some(review_contract(response_schema)?),
        risk: CapabilityRisk::Medium,
        mutation: true,
        requires_idempotency: true,
        requires_approval: false,
        authorization_policy_id: capability_id.to_owned(),
        rate_limit_policy_id: None,
    })
}

fn review_contract(schema: &'static str) -> Result<PayloadContract, SdkError> {
    support::protobuf_contract(MODULE_ID, schema, vec![DataClass::Personal])
}

/// Atomic review planner over one exact immutable suggestion.
#[derive(Debug, Clone)]
pub struct CustomerEnrichmentSuggestionReviewPlanner {
    suggestion: Suggestion,
    acceptance_approval_requirement: ApprovalRequirement,
}

impl CustomerEnrichmentSuggestionReviewPlanner {
    pub fn new(
        suggestion: Suggestion,
        acceptance_approval_requirement: ApprovalRequirement,
    ) -> Self {
        Self {
            suggestion,
            acceptance_approval_requirement,
        }
    }

    fn decision(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<ReviewDecision, SdkError> {
        ensure_definition(definition, request)?;
        let decided_at_unix_ms = request_started_at_unix_ms(request)?;
        match definition.capability_id.as_str() {
            ACCEPT_SUGGESTION_CAPABILITY => {
                let command: wire::AcceptSuggestionRequest =
                    support::decode_request_with_data_class(
                        request,
                        MODULE_ID,
                        ACCEPT_SUGGESTION_REQUEST_SCHEMA,
                        DataClass::Personal,
                    )?;
                ensure_suggestion_ref(command.suggestion_ref, &self.suggestion)?;
                ensure_expected_binding(
                    command.expected_party_resource_version,
                    command.expected_proposed_value_digest,
                    &self.suggestion,
                )?;
                ReviewDecision::decide(
                    &self.suggestion,
                    request.context.execution.actor_id.clone(),
                    ReviewDecisionKind::Accepted,
                    command.policy_version,
                    command.safe_reason_code,
                    self.acceptance_approval_requirement,
                    command.approval_evidence_reference,
                    decided_at_unix_ms,
                    command
                        .review_expires_at_unix_ms
                        .map(|value| non_negative_u64(value, "review_expires_at_unix_ms"))
                        .transpose()?,
                )
            }
            REJECT_SUGGESTION_CAPABILITY => {
                let command: wire::RejectSuggestionRequest =
                    support::decode_request_with_data_class(
                        request,
                        MODULE_ID,
                        REJECT_SUGGESTION_REQUEST_SCHEMA,
                        DataClass::Personal,
                    )?;
                ensure_suggestion_ref(command.suggestion_ref, &self.suggestion)?;
                ensure_expected_binding(
                    command.expected_party_resource_version,
                    command.expected_proposed_value_digest,
                    &self.suggestion,
                )?;
                ReviewDecision::decide(
                    &self.suggestion,
                    request.context.execution.actor_id.clone(),
                    ReviewDecisionKind::Rejected,
                    command.policy_version,
                    command.safe_reason_code,
                    ApprovalRequirement::NotRequired,
                    None,
                    decided_at_unix_ms,
                    None,
                )
            }
            _ => Err(unsupported_capability()),
        }
    }
}

impl TransactionalAggregatePlanner for CustomerEnrichmentSuggestionReviewPlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        let decision = self.decision(definition, request)?;
        Ok(AggregateTarget {
            reference: review_decision_record_ref(&decision)?,
            presence: AggregatePresence::MustBeAbsent,
        })
    }

    fn plan(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
        current: Option<&RecordSnapshot>,
    ) -> Result<CapabilityBatchExecutionPlan, SdkError> {
        let decision = self.decision(definition, request)?;
        let decision_reference = review_decision_record_ref(&decision)?;
        if current.is_some() {
            return Err(review_conflict(
                "the deterministic review-decision record already exists",
            ));
        }
        let at_unix_ms = request_started_at_unix_ms(request)?;
        let public_suggestion = suggestion_to_wire(&self.suggestion, Some(&decision), at_unix_ms)?;
        let public_decision = review_decision_to_wire(&decision)?;
        let output = match definition.capability_id.as_str() {
            ACCEPT_SUGGESTION_CAPABILITY => support::protobuf_payload(
                MODULE_ID,
                ACCEPT_SUGGESTION_RESPONSE_SCHEMA,
                DataClass::Personal,
                &wire::AcceptSuggestionResponse {
                    review_decision: Some(public_decision.clone()),
                    suggestion: Some(public_suggestion.clone()),
                },
            )?,
            REJECT_SUGGESTION_CAPABILITY => support::protobuf_payload(
                MODULE_ID,
                REJECT_SUGGESTION_RESPONSE_SCHEMA,
                DataClass::Personal,
                &wire::RejectSuggestionResponse {
                    review_decision: Some(public_decision.clone()),
                    suggestion: Some(public_suggestion.clone()),
                },
            )?,
            _ => return Err(unsupported_capability()),
        };
        let event = support::event_evidence_with_data_class(
            request,
            decision_reference.clone(),
            MODULE_ID,
            EventSpec {
                event_type: SUGGESTION_REVIEWED_EVENT_TYPE,
                event_schema_id: SUGGESTION_REVIEWED_EVENT_SCHEMA,
                aggregate_version: 1,
                previous_version: None,
            },
            DataClass::Personal,
            &wire::SuggestionReviewedEvent {
                suggestion: Some(public_suggestion),
                review_decision: Some(public_decision),
            },
        )?;
        Ok(CapabilityBatchExecutionPlan {
            batch: BatchMutationPlan {
                context: request.context.clone(),
                records: vec![RecordMutation::Create {
                    reference: decision_reference.clone(),
                    payload: review_decision_persisted_payload(&decision)?,
                }],
                relationships: Vec::new(),
                events: vec![event],
                idempotency: support::capability_idempotency(definition, request)?,
                audits: vec![support::audit_intent(
                    request,
                    &decision_reference,
                    1,
                    definition.capability_id.as_str(),
                    &output.bytes,
                )?],
            },
            output: Some(output),
        })
    }
}

pub fn suggestion_persisted_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: SUGGESTION_STATE_SCHEMA_ID,
        schema_version: LIFECYCLE_STATE_SCHEMA_VERSION,
        descriptor_hash: suggestion_state_descriptor_hash(),
        maximum_size_bytes: SUGGESTION_STATE_MAXIMUM_BYTES,
        retention_policy_id: LIFECYCLE_STATE_RETENTION_POLICY_ID,
    }
}

pub fn review_decision_persisted_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: REVIEW_DECISION_STATE_SCHEMA_ID,
        schema_version: LIFECYCLE_STATE_SCHEMA_VERSION,
        descriptor_hash: review_decision_state_descriptor_hash(),
        maximum_size_bytes: REVIEW_DECISION_STATE_MAXIMUM_BYTES,
        retention_policy_id: LIFECYCLE_STATE_RETENTION_POLICY_ID,
    }
}

pub fn suggestion_from_snapshot(snapshot: &RecordSnapshot) -> Result<Suggestion, SdkError> {
    if snapshot.reference.record_type.as_str() != SUGGESTION_RECORD_TYPE || snapshot.version != 1 {
        return Err(review_state_invalid(
            "suggestion record type or immutable version is invalid",
        ));
    }
    let bytes = support::persisted_json_bytes_with_data_class(
        snapshot,
        suggestion_persisted_contract(),
        DataClass::Personal,
    )?;
    let suggestion = decode_suggestion_state(bytes)?;
    if snapshot.reference.record_id.as_str() != suggestion.suggestion_id().as_str() {
        return Err(review_state_invalid(
            "suggestion record identity differs from its deterministic domain identity",
        ));
    }
    Ok(suggestion)
}

pub fn review_decision_from_snapshot(
    snapshot: &RecordSnapshot,
) -> Result<ReviewDecision, SdkError> {
    if snapshot.reference.record_type.as_str() != REVIEW_DECISION_RECORD_TYPE
        || snapshot.version != 1
    {
        return Err(review_state_invalid(
            "review-decision record type or immutable version is invalid",
        ));
    }
    let bytes = support::persisted_json_bytes_with_data_class(
        snapshot,
        review_decision_persisted_contract(),
        DataClass::Personal,
    )?;
    let decision = decode_review_decision_state(bytes)?;
    if snapshot.reference.record_id.as_str() != decision.decision_id().as_str() {
        return Err(review_state_invalid(
            "review-decision record identity differs from its deterministic domain identity",
        ));
    }
    Ok(decision)
}

pub fn suggestion_persisted_payload(suggestion: &Suggestion) -> Result<TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        suggestion_persisted_contract(),
        DataClass::Personal,
        encode_suggestion_state(suggestion)?,
    )
}

pub fn review_decision_persisted_payload(
    decision: &ReviewDecision,
) -> Result<TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        review_decision_persisted_contract(),
        DataClass::Personal,
        encode_review_decision_state(decision)?,
    )
}

pub fn suggestion_record_ref(suggestion_id: &str) -> Result<RecordRef, SdkError> {
    support::record_ref(
        SUGGESTION_RECORD_TYPE,
        suggestion_id,
        "customer_enrichment.suggestion_ref.suggestion_id",
    )
}

pub fn review_decision_record_ref(decision: &ReviewDecision) -> Result<RecordRef, SdkError> {
    support::record_ref(
        REVIEW_DECISION_RECORD_TYPE,
        decision.decision_id().as_str(),
        "customer_enrichment.review_decision_ref.review_decision_id",
    )
}

pub fn suggestion_to_wire(
    suggestion: &Suggestion,
    latest_decision: Option<&ReviewDecision>,
    at_unix_ms: u64,
) -> Result<wire::Suggestion, SdkError> {
    suggestion_to_wire_with_supersession(suggestion, latest_decision, None, at_unix_ms)
}

pub fn suggestion_to_wire_with_supersession(
    suggestion: &Suggestion,
    latest_decision: Option<&ReviewDecision>,
    superseded_by: Option<&SuggestionId>,
    at_unix_ms: u64,
) -> Result<wire::Suggestion, SdkError> {
    let state: SuggestionStateView = serde_json::from_slice(&encode_suggestion_state(suggestion)?)
        .map_err(|error| review_state_invalid(error.to_string()))?;
    Ok(wire::Suggestion {
        suggestion_ref: Some(wire::SuggestionRef {
            suggestion_id: state.suggestion_id,
        }),
        enrichment_request_ref: Some(wire::EnrichmentRequestRef {
            enrichment_request_id: state.request_id,
        }),
        provider_response_receipt_ref: Some(wire::ProviderResponseReceiptRef {
            provider_response_receipt_id: state.response_receipt_id,
        }),
        provider_profile_version_ref: Some(wire::ProviderProfileVersionRef {
            provider_profile_version_id: state.provider_profile_version_id,
        }),
        mapping_version_ref: Some(wire::MappingVersionRef {
            mapping_version_id: state.mapping_version_id,
        }),
        target: Some(wire::EnrichmentTargetSnapshot {
            party_ref: Some(customer::PartyRef {
                party_id: state.target.resource_id,
            }),
            party_resource_version: checked_i64(
                state.target.resource_version,
                "suggestion target resource version",
            )?,
            target_field: target_field_to_wire(state.target.target_field),
        }),
        proposed_value: state.proposed_value,
        proposed_value_digest: state.proposed_value_digest.to_vec(),
        observed_at_unix_ms: state
            .observed_at_unix_ms
            .map(|value| checked_i64(value, "suggestion observed timestamp"))
            .transpose()?,
        retrieved_at_unix_ms: checked_i64(
            state.retrieved_at_unix_ms,
            "suggestion retrieved timestamp",
        )?,
        effective_at_unix_ms: checked_i64(
            state.effective_at_unix_ms,
            "suggestion effective timestamp",
        )?,
        fresh_until_unix_ms: checked_i64(
            state.fresh_until_unix_ms,
            "suggestion fresh-until timestamp",
        )?,
        expires_at_unix_ms: checked_i64(state.expires_at_unix_ms, "suggestion expiry timestamp")?,
        confidence_basis_points: state.confidence_basis_points.map(u32::from),
        policy_evidence: Some(wire::ProviderPolicyEvidence {
            license_id: state.license_id,
            permitted_use_class: state.permitted_use_class,
            residency_region: state.residency_region,
            retention_days: state.retention_days,
            consent_evidence_reference: state.consent_evidence_reference,
        }),
        evidence_references: state.evidence_references,
        lifecycle_status: suggestion_status_to_wire(derive_suggestion_status(
            suggestion,
            latest_decision,
            None,
            superseded_by,
            at_unix_ms,
        )),
        superseded_by_suggestion_ref: superseded_by.map(|suggestion_id| wire::SuggestionRef {
            suggestion_id: suggestion_id.as_str().to_owned(),
        }),
    })
}

pub fn review_decision_to_wire(
    decision: &ReviewDecision,
) -> Result<wire::ReviewDecision, SdkError> {
    let state: ReviewDecisionStateView =
        serde_json::from_slice(&encode_review_decision_state(decision)?)
            .map_err(|error| review_state_invalid(error.to_string()))?;
    Ok(wire::ReviewDecision {
        review_decision_ref: Some(wire::ReviewDecisionRef {
            review_decision_id: state.decision_id,
        }),
        suggestion_ref: Some(wire::SuggestionRef {
            suggestion_id: state.suggestion_id,
        }),
        target_party_resource_version: checked_i64(
            state.target_resource_version,
            "review target resource version",
        )?,
        proposed_value_digest: state.proposed_value_digest.to_vec(),
        reviewed_by_actor_id: state.reviewed_by,
        kind: match state.kind {
            ReviewDecisionKind::Accepted => wire::SuggestionReviewDecisionKind::Accepted as i32,
            ReviewDecisionKind::Rejected => wire::SuggestionReviewDecisionKind::Rejected as i32,
        },
        policy_version: state.policy_version,
        safe_reason_code: state.safe_reason_code,
        approval_evidence_reference: state.approval_evidence_reference,
        decided_at_unix_ms: checked_i64(state.decided_at_unix_ms, "review decision timestamp")?,
        expires_at_unix_ms: state
            .expires_at_unix_ms
            .map(|value| checked_i64(value, "review expiry timestamp"))
            .transpose()?,
    })
}

fn ensure_definition(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<(), SdkError> {
    if !REVIEW_CAPABILITY_IDS.contains(&definition.capability_id.as_str())
        || definition.owner_module_id.as_str() != MODULE_ID
        || definition.capability_id != request.context.execution.capability_id
        || definition.capability_version != request.context.execution.capability_version
    {
        return Err(review_plan_invalid(
            "capability definition does not match request context",
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
    let requested_id = RecordId::try_new(reference.suggestion_id).map_err(|error| {
        SdkError::invalid_argument(
            "customer_enrichment.suggestion_ref.suggestion_id",
            error.to_string(),
        )
    })?;
    if requested_id.as_str() != suggestion.suggestion_id().as_str() {
        return Err(review_conflict(
            "review request does not reference the exact immutable suggestion",
        ));
    }
    Ok(())
}

fn ensure_expected_binding(
    expected_target_version: i64,
    expected_digest: Vec<u8>,
    suggestion: &Suggestion,
) -> Result<(), SdkError> {
    let expected_target_version = positive_u64(
        expected_target_version,
        "customer_enrichment.expected_party_resource_version",
    )?;
    if expected_target_version != suggestion.target().resource_version {
        return Err(review_conflict(
            "expected Party version differs from the suggestion target snapshot",
        ));
    }
    let expected_digest: [u8; 32] = expected_digest.try_into().map_err(|_| {
        SdkError::invalid_argument(
            "customer_enrichment.expected_proposed_value_digest",
            "Expected proposed-value digest must contain exactly 32 bytes",
        )
    })?;
    if &expected_digest != suggestion.proposed_value_digest() {
        return Err(review_conflict(
            "expected proposed-value digest differs from the immutable suggestion",
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
        .map_err(|_| review_plan_invalid("request timestamp exceeds u64"))
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

fn checked_i64(value: u64, label: &'static str) -> Result<i64, SdkError> {
    i64::try_from(value).map_err(|_| review_plan_invalid(format!("{label} exceeds i64")))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SuggestionStateView {
    suggestion_id: String,
    request_id: String,
    response_receipt_id: String,
    provider_profile_version_id: String,
    mapping_version_id: String,
    target: SuggestionTargetStateView,
    proposed_value: String,
    proposed_value_digest: [u8; 32],
    observed_at_unix_ms: Option<u64>,
    retrieved_at_unix_ms: u64,
    effective_at_unix_ms: u64,
    fresh_until_unix_ms: u64,
    expires_at_unix_ms: u64,
    confidence_basis_points: Option<u16>,
    #[serde(rename = "purpose_code")]
    _purpose_code: String,
    #[serde(rename = "legal_basis_code")]
    _legal_basis_code: String,
    license_id: String,
    permitted_use_class: String,
    residency_region: String,
    retention_days: u32,
    consent_evidence_reference: Option<String>,
    evidence_references: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SuggestionTargetStateView {
    resource_id: String,
    resource_version: u64,
    target_field: TargetField,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewDecisionStateView {
    decision_id: String,
    suggestion_id: String,
    target_resource_version: u64,
    proposed_value_digest: [u8; 32],
    reviewed_by: String,
    kind: ReviewDecisionKind,
    policy_version: String,
    safe_reason_code: String,
    approval_evidence_reference: Option<String>,
    decided_at_unix_ms: u64,
    expires_at_unix_ms: Option<u64>,
}

fn configured<T>(value: Result<T, crm_module_sdk::IdentifierError>) -> Result<T, SdkError> {
    value.map_err(|error| review_plan_invalid(error.to_string()))
}

fn unsupported_capability() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_REVIEW_CAPABILITY_UNSUPPORTED",
        ErrorCategory::InvalidArgument,
        false,
        "The requested suggestion review capability is not supported by this adapter.",
    )
}

fn review_conflict(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_REVIEW_CONFLICT",
        ErrorCategory::Conflict,
        false,
        "The suggestion could not be reviewed because its exact review preconditions changed.",
    )
    .with_internal_reference(reference.into())
}

fn review_state_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_REVIEW_PERSISTED_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "Persisted Customer Enrichment review evidence is invalid.",
    )
    .with_internal_reference(reference.into())
}

fn review_plan_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_REVIEW_PLAN_INVALID",
        ErrorCategory::Internal,
        false,
        "The suggestion review could not be planned safely.",
    )
    .with_internal_reference(reference.into())
}
