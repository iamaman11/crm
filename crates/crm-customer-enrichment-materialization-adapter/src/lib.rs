#![forbid(unsafe_code)]

//! Non-runtime deterministic Customer Enrichment suggestion materialization.
//!
//! Infrastructure loads and strictly rehydrates immutable response-receipt, provider-profile and
//! mapping snapshots before constructing this planner. PostgreSQL then locks the single mutable
//! enrichment request and commits its transition plus immutable suggestion evidence atomically.

use crm_capability_plan_support::{self as support, EventSpec, PersistedPayloadContract};
use crm_capability_runtime::{
    CapabilityDefinition, CapabilityRequest, CapabilityRisk, PayloadContract,
};
use crm_core_data::{
    AggregatePresence, AggregateTarget, BatchMutationPlan, CapabilityBatchExecutionPlan,
    RecordMutation, TransactionalAggregatePlanner,
};
use crm_customer_enrichment::{
    ENRICHMENT_REQUEST_RECORD_TYPE, LIFECYCLE_STATE_RETENTION_POLICY_ID,
    LIFECYCLE_STATE_SCHEMA_VERSION, MappingVersion, ProviderProfileVersion, ProviderResponseReceipt,
    SUGGESTION_RECORD_TYPE, SUGGESTION_STATE_MAXIMUM_BYTES, SUGGESTION_STATE_SCHEMA_ID, Suggestion,
    SuggestionCandidateDraft, SuggestionLifecycleStatus, TargetField, TargetSnapshot,
    derive_suggestion_status, encode_suggestion_state, materialize_suggestions,
    suggestion_state_descriptor_hash,
};
use crm_customer_enrichment_capability_adapter::{
    ENRICHMENT_REQUEST_STATUS_CHANGED_EVENT_SCHEMA, ENRICHMENT_REQUEST_STATUS_CHANGED_EVENT_TYPE,
    MODULE_ID, enrichment_request_from_snapshot, enrichment_request_persisted_payload,
    enrichment_request_to_wire,
};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, RecordId, RecordRef,
    RecordSnapshot, SdkError, TypedPayload,
};
use crm_proto_contracts::crm::{customer::v1 as customer, customer_enrichment::v1 as wire};
use serde::Deserialize;

/// Stable crate identity for architecture tooling.
pub const CRATE_NAME: &str = "crm-customer-enrichment-materialization-adapter";

pub const MATERIALIZE_SUGGESTIONS_CAPABILITY: &str =
    "customer_enrichment.suggestions.materialize";
pub const MATERIALIZE_SUGGESTIONS_REQUEST_SCHEMA: &str =
    "crm.customer_enrichment.v1.MaterializeSuggestionsRequest";
pub const MATERIALIZE_SUGGESTIONS_RESPONSE_SCHEMA: &str =
    "crm.customer_enrichment.v1.MaterializeSuggestionsResponse";
pub const SUGGESTION_MATERIALIZED_EVENT_TYPE: &str =
    "customer_enrichment.suggestion.materialized";
pub const SUGGESTION_MATERIALIZED_EVENT_SCHEMA: &str =
    "crm.customer_enrichment.v1.SuggestionMaterializedEvent";

/// Worker-only definition retained outside the public production mutation catalog.
pub fn suggestion_materialization_capability_definition(
) -> Result<CapabilityDefinition, SdkError> {
    Ok(CapabilityDefinition {
        capability_id: configured(CapabilityId::try_new(MATERIALIZE_SUGGESTIONS_CAPABILITY))?,
        capability_version: configured(CapabilityVersion::try_new(support::CONTRACT_VERSION))?,
        owner_module_id: configured(ModuleId::try_new(MODULE_ID))?,
        input_contract: materialization_contract(MATERIALIZE_SUGGESTIONS_REQUEST_SCHEMA)?,
        output_contract: Some(materialization_contract(
            MATERIALIZE_SUGGESTIONS_RESPONSE_SCHEMA,
        )?),
        risk: CapabilityRisk::Medium,
        mutation: true,
        requires_idempotency: true,
        requires_approval: false,
        authorization_policy_id: MATERIALIZE_SUGGESTIONS_CAPABILITY.to_owned(),
        rate_limit_policy_id: None,
    })
}

fn materialization_contract(schema: &'static str) -> Result<PayloadContract, SdkError> {
    support::protobuf_contract(MODULE_ID, schema, vec![DataClass::Personal])
}

/// Atomic planner over one mutable request and three exact immutable dependency snapshots.
#[derive(Debug, Clone)]
pub struct CustomerEnrichmentSuggestionMaterializationPlanner {
    receipt: ProviderResponseReceipt,
    profile: ProviderProfileVersion,
    mapping: MappingVersion,
}

impl CustomerEnrichmentSuggestionMaterializationPlanner {
    pub fn new(
        receipt: ProviderResponseReceipt,
        profile: ProviderProfileVersion,
        mapping: MappingVersion,
    ) -> Self {
        Self {
            receipt,
            profile,
            mapping,
        }
    }
}

impl TransactionalAggregatePlanner for CustomerEnrichmentSuggestionMaterializationPlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        ensure_definition(definition, request)?;
        let command = materialization_command(request)?;
        ensure_receipt_ref(&command, &self.receipt)?;
        Ok(AggregateTarget {
            reference: request_record_ref(&command)?,
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
        let command = materialization_command(request)?;
        ensure_receipt_ref(&command, &self.receipt)?;
        let current = current.ok_or_else(request_not_found)?;
        if current.reference != request_record_ref(&command)? || current.version <= 0 {
            return Err(materialization_plan_invalid(
                "locked request differs from the materialization target",
            ));
        }

        let mut enrichment_request = enrichment_request_from_snapshot(current)?;
        let materialized_at_unix_ms = request_started_at_unix_ms(request)?;
        let suggestions = materialize_suggestions(
            &mut enrichment_request,
            &self.receipt,
            &self.profile,
            &self.mapping,
            command
                .candidates
                .into_iter()
                .map(candidate_from_wire)
                .collect::<Result<Vec<_>, _>>()?,
            materialized_at_unix_ms,
        )?;

        let public_request = enrichment_request_to_wire(&enrichment_request)?;
        let public_suggestions = suggestions
            .iter()
            .map(|suggestion| suggestion_to_wire(suggestion, materialized_at_unix_ms))
            .collect::<Result<Vec<_>, _>>()?;
        let output = support::protobuf_payload(
            MODULE_ID,
            MATERIALIZE_SUGGESTIONS_RESPONSE_SCHEMA,
            DataClass::Personal,
            &wire::MaterializeSuggestionsResponse {
                enrichment_request: Some(public_request.clone()),
                suggestions: public_suggestions.clone(),
            },
        )?;

        let next_request_version = current
            .version
            .checked_add(1)
            .ok_or_else(|| materialization_plan_invalid("request version overflow"))?;
        let suggestion_references = suggestions
            .iter()
            .map(suggestion_record_ref)
            .collect::<Result<Vec<_>, _>>()?;

        let mut records = vec![RecordMutation::Update {
            reference: current.reference.clone(),
            expected_version: current.version,
            payload: enrichment_request_persisted_payload(&enrichment_request)?,
        }];
        records.extend(
            suggestions
                .iter()
                .zip(&suggestion_references)
                .map(|(suggestion, reference)| {
                    Ok(RecordMutation::Create {
                        reference: reference.clone(),
                        payload: suggestion_persisted_payload(suggestion)?,
                    })
                })
                .collect::<Result<Vec<_>, SdkError>>()?,
        );

        let mut events = vec![support::event_evidence_with_data_class(
            request,
            current.reference.clone(),
            MODULE_ID,
            EventSpec {
                event_type: ENRICHMENT_REQUEST_STATUS_CHANGED_EVENT_TYPE,
                event_schema_id: ENRICHMENT_REQUEST_STATUS_CHANGED_EVENT_SCHEMA,
                aggregate_version: next_request_version,
                previous_version: Some(current.version),
            },
            DataClass::Personal,
            &wire::EnrichmentRequestStatusChangedEvent {
                enrichment_request: Some(public_request),
            },
        )?];
        events.extend(
            suggestion_references
                .iter()
                .zip(public_suggestions)
                .map(|(reference, suggestion)| {
                    support::event_evidence_with_data_class(
                        request,
                        reference.clone(),
                        MODULE_ID,
                        EventSpec {
                            event_type: SUGGESTION_MATERIALIZED_EVENT_TYPE,
                            event_schema_id: SUGGESTION_MATERIALIZED_EVENT_SCHEMA,
                            aggregate_version: 1,
                            previous_version: None,
                        },
                        DataClass::Personal,
                        &wire::SuggestionMaterializedEvent {
                            suggestion: Some(suggestion),
                        },
                    )
                })
                .collect::<Result<Vec<_>, _>>()?,
        );

        let mut audits = vec![support::audit_intent(
            request,
            &current.reference,
            next_request_version,
            definition.capability_id.as_str(),
            &output.bytes,
        )?];
        audits.extend(
            suggestion_references
                .iter()
                .map(|reference| {
                    support::audit_intent(
                        request,
                        reference,
                        1,
                        definition.capability_id.as_str(),
                        &output.bytes,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?,
        );

        Ok(CapabilityBatchExecutionPlan {
            batch: BatchMutationPlan {
                context: request.context.clone(),
                records,
                relationships: Vec::new(),
                events,
                idempotency: support::capability_idempotency(definition, request)?,
                audits,
            },
            output: Some(output),
        })
    }
}

fn materialization_command(
    request: &CapabilityRequest,
) -> Result<wire::MaterializeSuggestionsRequest, SdkError> {
    support::decode_request_with_data_class(
        request,
        MODULE_ID,
        MATERIALIZE_SUGGESTIONS_REQUEST_SCHEMA,
        DataClass::Personal,
    )
}

fn ensure_receipt_ref(
    command: &wire::MaterializeSuggestionsRequest,
    receipt: &ProviderResponseReceipt,
) -> Result<(), SdkError> {
    let receipt_ref = command
        .provider_response_receipt_ref
        .as_ref()
        .ok_or_else(|| {
            SdkError::invalid_argument(
                "customer_enrichment.provider_response_receipt_ref",
                "Provider-response receipt reference is required",
            )
        })?;
    if receipt_ref.provider_response_receipt_id != receipt.receipt_id().as_str() {
        return Err(SdkError::new(
            "CUSTOMER_ENRICHMENT_MATERIALIZATION_RECEIPT_CONFLICT",
            ErrorCategory::Conflict,
            false,
            "The materialization request does not reference the exact immutable response receipt.",
        ));
    }
    Ok(())
}

fn request_record_ref(command: &wire::MaterializeSuggestionsRequest) -> Result<RecordRef, SdkError> {
    let request_ref = command.enrichment_request_ref.as_ref().ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_enrichment.enrichment_request_ref",
            "Enrichment-request reference is required",
        )
    })?;
    support::record_ref(
        ENRICHMENT_REQUEST_RECORD_TYPE,
        RecordId::try_new(request_ref.enrichment_request_id.clone())
            .map_err(|error| {
                SdkError::invalid_argument(
                    "customer_enrichment.enrichment_request_ref.enrichment_request_id",
                    error.to_string(),
                )
            })?
            .as_str(),
        "customer_enrichment.enrichment_request_ref.enrichment_request_id",
    )
}

fn candidate_from_wire(
    candidate: wire::ProviderSuggestionCandidate,
) -> Result<SuggestionCandidateDraft, SdkError> {
    let target = candidate.target.ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_enrichment.candidates.target",
            "Suggestion target snapshot is required",
        )
    })?;
    let party_ref = target.party_ref.ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_enrichment.candidates.target.party_ref",
            "Party reference is required",
        )
    })?;
    let policy = candidate.policy_evidence.ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_enrichment.candidates.policy_evidence",
            "Provider policy evidence is required",
        )
    })?;
    Ok(SuggestionCandidateDraft {
        target: TargetSnapshot::try_new(
            party_ref.party_id,
            positive_u64(
                target.party_resource_version,
                "customer_enrichment.candidates.target.party_resource_version",
            )?,
            target_field_from_wire(target.target_field)?,
        )?,
        proposed_value: candidate.proposed_value,
        observed_at_unix_ms: candidate
            .observed_at_unix_ms
            .map(|value| {
                non_negative_u64(value, "customer_enrichment.candidates.observed_at_unix_ms")
            })
            .transpose()?,
        effective_at_unix_ms: non_negative_u64(
            candidate.effective_at_unix_ms,
            "customer_enrichment.candidates.effective_at_unix_ms",
        )?,
        fresh_until_unix_ms: non_negative_u64(
            candidate.fresh_until_unix_ms,
            "customer_enrichment.candidates.fresh_until_unix_ms",
        )?,
        expires_at_unix_ms: non_negative_u64(
            candidate.expires_at_unix_ms,
            "customer_enrichment.candidates.expires_at_unix_ms",
        )?,
        confidence_basis_points: candidate
            .confidence_basis_points
            .map(|value| {
                u16::try_from(value).map_err(|_| {
                    SdkError::invalid_argument(
                        "customer_enrichment.candidates.confidence_basis_points",
                        "Confidence basis points exceed the supported range",
                    )
                })
            })
            .transpose()?,
        license_id: policy.license_id,
        permitted_use_class: policy.permitted_use_class,
        residency_region: policy.residency_region,
        retention_days: policy.retention_days,
        consent_evidence_reference: policy.consent_evidence_reference,
        evidence_references: candidate.evidence_references,
    })
}

fn suggestion_persisted_payload(suggestion: &Suggestion) -> Result<TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        PersistedPayloadContract {
            owner: MODULE_ID,
            schema_id: SUGGESTION_STATE_SCHEMA_ID,
            schema_version: LIFECYCLE_STATE_SCHEMA_VERSION,
            descriptor_hash: suggestion_state_descriptor_hash(),
            maximum_size_bytes: SUGGESTION_STATE_MAXIMUM_BYTES,
            retention_policy_id: LIFECYCLE_STATE_RETENTION_POLICY_ID,
        },
        DataClass::Personal,
        encode_suggestion_state(suggestion)?,
    )
}

fn suggestion_record_ref(suggestion: &Suggestion) -> Result<RecordRef, SdkError> {
    support::record_ref(
        SUGGESTION_RECORD_TYPE,
        suggestion.suggestion_id().as_str(),
        "customer_enrichment.suggestion_ref.suggestion_id",
    )
}

fn suggestion_to_wire(
    suggestion: &Suggestion,
    at_unix_ms: u64,
) -> Result<wire::Suggestion, SdkError> {
    let state: SuggestionStateView = serde_json::from_slice(&encode_suggestion_state(suggestion)?)
        .map_err(|error| materialization_plan_invalid(error.to_string()))?;
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
        expires_at_unix_ms: checked_i64(
            state.expires_at_unix_ms,
            "suggestion expiry timestamp",
        )?,
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
            None,
            None,
            None,
            at_unix_ms,
        )),
        superseded_by_suggestion_ref: None,
    })
}

fn ensure_definition(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<(), SdkError> {
    if definition.capability_id.as_str() != MATERIALIZE_SUGGESTIONS_CAPABILITY
        || definition.owner_module_id.as_str() != MODULE_ID
        || definition.capability_id != request.context.execution.capability_id
        || definition.capability_version != request.context.execution.capability_version
    {
        return Err(materialization_plan_invalid(
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
        .map_err(|_| materialization_plan_invalid("request timestamp exceeds u64"))
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
        .map_err(|_| SdkError::invalid_argument(field, "Timestamp must not be negative"))
}

fn target_field_from_wire(value: i32) -> Result<TargetField, SdkError> {
    match wire::EnrichmentTargetField::try_from(value) {
        Ok(wire::EnrichmentTargetField::PartyDisplayName) => Ok(TargetField::PartyDisplayName),
        Ok(wire::EnrichmentTargetField::Unspecified) | Err(_) => Err(SdkError::invalid_argument(
            "customer_enrichment.candidates.target.target_field",
            "Suggestion target field must be specified",
        )),
    }
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
        SuggestionLifecycleStatus::Superseded => {
            wire::SuggestionLifecycleStatus::Superseded as i32
        }
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
    i64::try_from(value).map_err(|_| materialization_plan_invalid(format!("{label} exceeds i64")))
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

fn configured<T>(value: Result<T, crm_module_sdk::IdentifierError>) -> Result<T, SdkError> {
    value.map_err(|error| materialization_plan_invalid(error.to_string()))
}

fn request_not_found() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_REQUEST_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The requested enrichment request was not found.",
    )
}

fn materialization_plan_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_MATERIALIZATION_PLAN_INVALID",
        ErrorCategory::Internal,
        false,
        "Suggestions could not be materialized safely.",
    )
    .with_internal_reference(reference.into())
}
