use super::{
    PostgresCustomerEnrichmentApplicationAttemptExecutor,
    PostgresCustomerEnrichmentApplicationOutcomeExecutor, application_attempt_not_found,
    application_store_unavailable, required_attempt_ref, required_review_ref,
    required_suggestion_ref, review_not_found, suggestion_not_found,
};
use crm_capability_ingress::semantic_input_hash;
use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityExecutionResult, CapabilityRequest};
use crm_core_data::PostgresDataStore;
use crm_customer_enrichment::{
    ApplicationAttempt, EnrichmentPolicyDecision, EnrichmentPolicyPort, EnrichmentPolicyRequest,
    EnrichmentRequestId, MappingVersionId, PARTY_DISPLAY_NAME_UPDATE_CAPABILITY_ID,
    PARTY_DISPLAY_NAME_UPDATE_CAPABILITY_VERSION, PartyDisplayNameApplicationPort,
    PartyDisplayNameApplicationRequest, PartyDisplayNameApplicationResult, PolicyEvaluationPhase,
    ProviderProfileVersionId, ReviewDecision, ReviewDecisionKind, Suggestion,
    encode_suggestion_state,
};
use crm_customer_enrichment_application_adapter::{
    APPLY_PARTY_DISPLAY_NAME_CAPABILITY, APPLY_PARTY_DISPLAY_NAME_REQUEST_SCHEMA,
    APPLY_PARTY_DISPLAY_NAME_RESPONSE_SCHEMA, RECORD_APPLICATION_OUTCOME_CAPABILITY,
    RECORD_APPLICATION_OUTCOME_REQUEST_SCHEMA, RECORD_APPLICATION_OUTCOME_RESPONSE_SCHEMA,
    application_attempt_from_snapshot, application_attempt_to_wire,
    apply_party_display_name_capability_definition, record_application_outcome_capability_definition,
    review_from_application_snapshot, suggestion_from_application_snapshot,
};
use crm_customer_enrichment_capability_adapter::MODULE_ID;
use crm_module_sdk::{
    BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, Clock, CorrelationId,
    DataClass, ErrorCategory, ExecutionContext, IdempotencyKey, ModuleExecutionContext, ModuleId,
    PayloadEncoding, RecordId, RecordRef, RequestId, SchemaVersion, SdkError, TraceId,
};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;
use prost::Message;
use serde::Deserialize;
use std::fmt;
use std::sync::Arc;

const MAX_INTERNAL_ID_BYTES: usize = 180;

#[derive(Debug, Clone, PartialEq)]
pub struct PartyDisplayNameApplicationOrchestrationResult {
    pub attempt_replayed: bool,
    pub outcome_replayed: bool,
    pub policy_evaluated: bool,
    pub owner_invoked: bool,
    pub application_attempt: wire::ApplicationAttempt,
}

#[derive(Clone)]
pub struct CustomerEnrichmentPartyApplicationOrchestrator {
    attempt_executor: PostgresCustomerEnrichmentApplicationAttemptExecutor,
    outcome_executor: PostgresCustomerEnrichmentApplicationOutcomeExecutor,
    evidence_reader: PostgresCustomerEnrichmentApplicationEvidenceReader,
    policy: Arc<dyn EnrichmentPolicyPort>,
    owner: Arc<dyn PartyDisplayNameApplicationPort>,
    clock: Arc<dyn Clock>,
    attempt_definition: CapabilityDefinition,
    outcome_definition: CapabilityDefinition,
}

impl fmt::Debug for CustomerEnrichmentPartyApplicationOrchestrator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CustomerEnrichmentPartyApplicationOrchestrator")
            .field("attempt_executor", &self.attempt_executor)
            .field("outcome_executor", &self.outcome_executor)
            .field("evidence_reader", &self.evidence_reader)
            .field("policy", &"dyn EnrichmentPolicyPort")
            .field("owner", &"dyn PartyDisplayNameApplicationPort")
            .field("clock", &"dyn Clock")
            .finish()
    }
}

impl CustomerEnrichmentPartyApplicationOrchestrator {
    pub fn postgres(
        store: PostgresDataStore,
        policy: Arc<dyn EnrichmentPolicyPort>,
        owner: Arc<dyn PartyDisplayNameApplicationPort>,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, SdkError> {
        Ok(Self {
            attempt_executor: PostgresCustomerEnrichmentApplicationAttemptExecutor::new(
                store.clone(),
            ),
            outcome_executor: PostgresCustomerEnrichmentApplicationOutcomeExecutor::new(
                store.clone(),
            ),
            evidence_reader: PostgresCustomerEnrichmentApplicationEvidenceReader::new(store),
            policy,
            owner,
            clock,
            attempt_definition: apply_party_display_name_capability_definition()?,
            outcome_definition: record_application_outcome_capability_definition()?,
        })
    }

    pub async fn execute(
        &self,
        request: CapabilityRequest,
    ) -> Result<PartyDisplayNameApplicationOrchestrationResult, SdkError> {
        validate_attempt_request(&self.attempt_definition, &request)?;
        let command: wire::ApplyPartyDisplayNameSuggestionRequest =
            support::decode_request_with_data_class(
                &request,
                MODULE_ID,
                APPLY_PARTY_DISPLAY_NAME_REQUEST_SCHEMA,
                DataClass::Personal,
            )?;
        let suggestion_ref = required_suggestion_ref(command.suggestion_ref.clone())?;
        let review_ref = required_review_ref(command.review_decision_ref.clone())?;

        let attempt_result = self.attempt_executor.execute(request.clone()).await?;
        let attempt_output: wire::ApplyPartyDisplayNameSuggestionResponse = decode_output(
            &attempt_result,
            APPLY_PARTY_DISPLAY_NAME_RESPONSE_SCHEMA,
            DataClass::Personal,
        )?;
        let output_attempt = attempt_output.application_attempt.ok_or_else(|| {
            orchestration_output_invalid("attempt transaction omitted application-attempt state")
        })?;
        let attempt_ref = required_attempt_ref(output_attempt.application_attempt_ref.clone())?;

        let evidence = self
            .evidence_reader
            .load(ApplicationEvidenceReadRequest {
                context: request.context.clone(),
                suggestion_ref,
                review_ref,
                attempt_ref,
            })
            .await?;
        let current_attempt = application_attempt_to_wire(&evidence.application_attempt)?;
        validate_committed_attempt(
            &request,
            &command,
            &output_attempt,
            &current_attempt,
            &evidence,
        )?;

        if current_attempt.recorded_outcome.is_some() {
            return Ok(PartyDisplayNameApplicationOrchestrationResult {
                attempt_replayed: attempt_result.replayed,
                outcome_replayed: true,
                policy_evaluated: false,
                owner_invoked: false,
                application_attempt: current_attempt,
            });
        }

        let policy_now_nanos = self.clock.now_unix_nanos();
        let policy_now_ms = nonnegative_unix_ms(policy_now_nanos)?;
        let policy_request = build_policy_request(&request, &evidence, policy_now_ms)?;
        let policy_decision = self.policy.evaluate(policy_request).await?;
        validate_policy_decision(&policy_decision)?;

        let (outcome, owner_invoked) = match &policy_decision {
            EnrichmentPolicyDecision::Denied { .. } => (policy_denied_outcome(), false),
            EnrichmentPolicyDecision::Allowed { decision_id, .. } => {
                let owner_request = build_owner_request(&request, &evidence, decision_id)?;
                let owner_result = self.owner.apply(owner_request.clone()).await?;
                (
                    owner_result_to_wire(&owner_request, decision_id, owner_result)?,
                    true,
                )
            }
        };

        let recorded_at_nanos = self.clock.now_unix_nanos();
        let recorded_at_ms = nonnegative_unix_ms(recorded_at_nanos)?;
        let outcome_request = build_outcome_request(
            &self.outcome_definition,
            &request,
            &evidence.application_attempt,
            policy_decision.decision_id(),
            outcome.clone(),
            recorded_at_nanos,
            recorded_at_ms,
        )?;
        let outcome_result = self.outcome_executor.execute(outcome_request).await?;
        let outcome_output: wire::RecordApplicationOutcomeResponse = decode_output(
            &outcome_result,
            RECORD_APPLICATION_OUTCOME_RESPONSE_SCHEMA,
            DataClass::Personal,
        )?;
        let completed_attempt = outcome_output.application_attempt.ok_or_else(|| {
            orchestration_output_invalid("outcome transaction omitted application-attempt state")
        })?;
        validate_completed_attempt(
            &current_attempt,
            &completed_attempt,
            &outcome,
            recorded_at_ms,
        )?;

        Ok(PartyDisplayNameApplicationOrchestrationResult {
            attempt_replayed: attempt_result.replayed,
            outcome_replayed: outcome_result.replayed,
            policy_evaluated: true,
            owner_invoked,
            application_attempt: completed_attempt,
        })
    }
}

#[derive(Debug, Clone)]
struct ApplicationEvidenceReadRequest {
    context: ModuleExecutionContext,
    suggestion_ref: RecordRef,
    review_ref: RecordRef,
    attempt_ref: RecordRef,
}

#[derive(Debug, Clone)]
struct ApplicationEvidence {
    suggestion: Suggestion,
    review: ReviewDecision,
    application_attempt: ApplicationAttempt,
}

#[derive(Debug, Clone)]
struct PostgresCustomerEnrichmentApplicationEvidenceReader {
    store: PostgresDataStore,
}

impl PostgresCustomerEnrichmentApplicationEvidenceReader {
    fn new(store: PostgresDataStore) -> Self {
        Self { store }
    }

    async fn load(
        &self,
        request: ApplicationEvidenceReadRequest,
    ) -> Result<ApplicationEvidence, SdkError> {
        let suggestion_snapshot = self
            .load_required(
                &request.context,
                &request.suggestion_ref,
                suggestion_not_found(),
            )
            .await?;
        let review_snapshot = self
            .load_required(&request.context, &request.review_ref, review_not_found())
            .await?;
        let attempt_snapshot = self
            .load_required(
                &request.context,
                &request.attempt_ref,
                application_attempt_not_found(),
            )
            .await?;
        Ok(ApplicationEvidence {
            suggestion: suggestion_from_application_snapshot(&suggestion_snapshot)?,
            review: review_from_application_snapshot(&review_snapshot)?,
            application_attempt: application_attempt_from_snapshot(&attempt_snapshot)?,
        })
    }

    async fn load_required(
        &self,
        context: &ModuleExecutionContext,
        reference: &RecordRef,
        not_found: SdkError,
    ) -> Result<crm_module_sdk::RecordSnapshot, SdkError> {
        self.store
            .get_record(context, reference)
            .await
            .map_err(|error| application_store_unavailable(error.to_string()))?
            .ok_or(not_found)
    }
}

#[derive(Debug, Deserialize)]
struct SuggestionPolicyStateView {
    request_id: EnrichmentRequestId,
    provider_profile_version_id: ProviderProfileVersionId,
    mapping_version_id: MappingVersionId,
    purpose_code: String,
    legal_basis_code: String,
    consent_evidence_reference: Option<String>,
}

fn validate_attempt_request(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<(), SdkError> {
    if definition.capability_id.as_str() != APPLY_PARTY_DISPLAY_NAME_CAPABILITY
        || definition.owner_module_id.as_str() != MODULE_ID
        || request.context.module_id.as_str() != MODULE_ID
        || request.context.execution.capability_id != definition.capability_id
        || request.context.execution.capability_version != definition.capability_version
        || semantic_input_hash(&request.input) != request.input_hash
    {
        return Err(orchestration_input_invalid(
            "request context does not match the exact application-attempt capability",
        ));
    }
    Ok(())
}

fn validate_committed_attempt(
    request: &CapabilityRequest,
    command: &wire::ApplyPartyDisplayNameSuggestionRequest,
    output_attempt: &wire::ApplicationAttempt,
    current_attempt: &wire::ApplicationAttempt,
    evidence: &ApplicationEvidence,
) -> Result<(), SdkError> {
    let requested_suggestion = command
        .suggestion_ref
        .as_ref()
        .ok_or_else(|| orchestration_input_invalid("application command omitted suggestion"))?;
    let requested_review = command
        .review_decision_ref
        .as_ref()
        .ok_or_else(|| orchestration_input_invalid("application command omitted review decision"))?;
    let output_attempt_id = application_attempt_id(output_attempt)?;
    let current_attempt_id = application_attempt_id(current_attempt)?;
    let current_suggestion = current_attempt.suggestion_ref.as_ref().ok_or_else(|| {
        orchestration_output_invalid("persisted attempt omitted suggestion identity")
    })?;
    let current_review = current_attempt.review_decision_ref.as_ref().ok_or_else(|| {
        orchestration_output_invalid("persisted attempt omitted review-decision identity")
    })?;
    let current_target = current_attempt
        .target
        .as_ref()
        .ok_or_else(|| orchestration_output_invalid("persisted attempt omitted target state"))?;
    let current_party = current_target
        .party_ref
        .as_ref()
        .ok_or_else(|| orchestration_output_invalid("persisted attempt omitted Party identity"))?;
    let expected_planned_at =
        nonnegative_unix_ms(request.context.execution.request_started_at_unix_nanos)?;

    if output_attempt_id != current_attempt_id
        || requested_suggestion.suggestion_id != evidence.suggestion.suggestion_id().as_str()
        || requested_review.review_decision_id != evidence.review.decision_id().as_str()
        || current_suggestion.suggestion_id != requested_suggestion.suggestion_id
        || current_review.review_decision_id != requested_review.review_decision_id
        || evidence.review.kind() != ReviewDecisionKind::Accepted
        || evidence.review.suggestion_id() != evidence.suggestion.suggestion_id()
        || current_party.party_id != evidence.suggestion.target().resource_id.as_str()
        || current_target.party_resource_version != command.expected_party_resource_version
        || current_target.party_resource_version
            != checked_i64(
                evidence.suggestion.target().resource_version,
                "suggestion Party version",
            )?
        || current_target.target_field != wire::EnrichmentTargetField::PartyDisplayName as i32
        || current_attempt.proposed_value_digest.as_slice()
            != evidence.suggestion.proposed_value_digest().as_slice()
        || current_attempt.application_generation != command.application_generation
        || current_attempt.owner_capability_id != PARTY_DISPLAY_NAME_UPDATE_CAPABILITY_ID
        || current_attempt.owner_capability_version
            != PARTY_DISPLAY_NAME_UPDATE_CAPABILITY_VERSION
        || current_attempt.target_idempotency_key
            != evidence
                .application_attempt
                .target_idempotency_key()
                .as_str()
        || current_attempt.planned_at_unix_ms != expected_planned_at
    {
        return Err(orchestration_output_invalid(
            "committed application attempt does not match the exact request, suggestion and review lineage",
        ));
    }
    Ok(())
}

fn build_policy_request(
    request: &CapabilityRequest,
    evidence: &ApplicationEvidence,
    evaluated_at_unix_ms: i64,
) -> Result<EnrichmentPolicyRequest, SdkError> {
    let state: SuggestionPolicyStateView =
        serde_json::from_slice(&encode_suggestion_state(&evidence.suggestion)?)
            .map_err(|error| orchestration_output_invalid(error.to_string()))?;
    let party_id = RecordId::try_new(evidence.suggestion.target().resource_id.clone())
        .map_err(|error| orchestration_output_invalid(error.to_string()))?;
    Ok(EnrichmentPolicyRequest {
        tenant_id: request.context.execution.tenant_id.clone(),
        actor_id: request.context.execution.actor_id.clone(),
        request_identity: evidence.application_attempt.attempt_id().as_str().to_owned(),
        enrichment_request_id: state.request_id,
        party_id,
        target_field: evidence.suggestion.target().target_field,
        provider_profile_version_id: state.provider_profile_version_id,
        mapping_version_id: state.mapping_version_id,
        purpose_code: state.purpose_code,
        legal_basis_code: state.legal_basis_code,
        consent_evidence_reference: state.consent_evidence_reference,
        phase: PolicyEvaluationPhase::OwnerApplication,
        evaluated_at_unix_ms,
    })
}

fn build_owner_request(
    request: &CapabilityRequest,
    evidence: &ApplicationEvidence,
    policy_decision_id: &str,
) -> Result<PartyDisplayNameApplicationRequest, SdkError> {
    Ok(PartyDisplayNameApplicationRequest {
        tenant_id: request.context.execution.tenant_id.clone(),
        actor_id: request.context.execution.actor_id.clone(),
        suggestion_id: evidence.suggestion.suggestion_id().clone(),
        review_decision_id: evidence.review.decision_id().clone(),
        application_attempt_id: evidence.application_attempt.attempt_id().clone(),
        party_id: RecordId::try_new(evidence.suggestion.target().resource_id.clone())
            .map_err(|error| orchestration_output_invalid(error.to_string()))?,
        expected_party_resource_version: checked_i64(
            evidence.suggestion.target().resource_version,
            "suggestion Party version",
        )?,
        reviewed_display_name: evidence.suggestion.proposed_value().to_owned(),
        target_idempotency_key: evidence
            .application_attempt
            .target_idempotency_key()
            .as_str()
            .to_owned(),
        final_authorization_decision_id: policy_decision_id.to_owned(),
    })
}

fn owner_result_to_wire(
    request: &PartyDisplayNameApplicationRequest,
    expected_decision_id: &str,
    result: PartyDisplayNameApplicationResult,
) -> Result<wire::ApplicationOutcome, SdkError> {
    use wire::application_outcome::Result as WireResult;
    let result = match result {
        PartyDisplayNameApplicationResult::Applied {
            business_transaction_id,
            resulting_party_resource_version,
        } => {
            BusinessTransactionId::try_new(business_transaction_id.clone())
                .map_err(|error| orchestration_owner_invalid(error.to_string()))?;
            let expected_version = request
                .expected_party_resource_version
                .checked_add(1)
                .ok_or_else(|| orchestration_owner_invalid("Party version overflow"))?;
            if resulting_party_resource_version != expected_version {
                return Err(orchestration_owner_invalid(
                    "owner success did not advance the exact expected Party version",
                ));
            }
            WireResult::Succeeded(wire::ApplicationSucceeded {
                business_transaction_id,
                resulting_party_resource_version,
            })
        }
        PartyDisplayNameApplicationResult::StaleTarget {
            actual_party_resource_version,
        } => {
            if actual_party_resource_version <= 0 {
                return Err(orchestration_owner_invalid(
                    "owner stale-target result contained an invalid Party version",
                ));
            }
            WireResult::StaleTarget(wire::ApplicationStaleTarget {
                actual_party_resource_version,
            })
        }
        PartyDisplayNameApplicationResult::AuthorizationDenied { decision_id } => {
            if decision_id != expected_decision_id {
                return Err(orchestration_owner_invalid(
                    "owner authorization denial did not preserve the final policy decision identity",
                ));
            }
            WireResult::AuthorizationDenied(wire::ApplicationAuthorizationDenied {})
        }
        PartyDisplayNameApplicationResult::RetryableFailure { safe_code } => {
            validate_internal_id(&safe_code, "owner retryable failure code")?;
            WireResult::RetryableFailure(wire::ApplicationRetryableFailure { safe_code })
        }
        PartyDisplayNameApplicationResult::TerminalFailure { safe_code } => {
            validate_internal_id(&safe_code, "owner terminal failure code")?;
            WireResult::TerminalFailure(wire::ApplicationTerminalFailure { safe_code })
        }
    };
    Ok(wire::ApplicationOutcome {
        result: Some(result),
    })
}

fn policy_denied_outcome() -> wire::ApplicationOutcome {
    wire::ApplicationOutcome {
        result: Some(wire::application_outcome::Result::PolicyDenied(
            wire::ApplicationPolicyDenied {},
        )),
    }
}

#[allow(clippy::too_many_arguments)]
fn build_outcome_request(
    definition: &CapabilityDefinition,
    original: &CapabilityRequest,
    attempt: &ApplicationAttempt,
    policy_decision_id: &str,
    outcome: wire::ApplicationOutcome,
    recorded_at_unix_nanos: i64,
    recorded_at_unix_ms: i64,
) -> Result<CapabilityRequest, SdkError> {
    if definition.capability_id.as_str() != RECORD_APPLICATION_OUTCOME_CAPABILITY
        || definition.owner_module_id.as_str() != MODULE_ID
    {
        return Err(orchestration_input_invalid(
            "outcome definition does not match the exact worker-only coordinate",
        ));
    }
    validate_internal_id(policy_decision_id, "owner-application policy decision")?;
    let attempt_id = attempt.attempt_id().as_str();
    let internal_id = format!("customer-enrichment-outcome-{attempt_id}");
    validate_internal_id(&internal_id, "application outcome request identity")?;
    let input = support::protobuf_payload(
        MODULE_ID,
        RECORD_APPLICATION_OUTCOME_REQUEST_SCHEMA,
        DataClass::Personal,
        &wire::RecordApplicationOutcomeRequest {
            application_attempt_ref: Some(wire::ApplicationAttemptRef {
                application_attempt_id: attempt_id.to_owned(),
            }),
            outcome: Some(outcome),
            recorded_at_unix_ms,
        },
    )?;
    let input_hash = semantic_input_hash(&input);
    Ok(CapabilityRequest {
        context: ModuleExecutionContext {
            module_id: configured(ModuleId::try_new(MODULE_ID))?,
            execution: ExecutionContext {
                tenant_id: original.context.execution.tenant_id.clone(),
                actor_id: original.context.execution.actor_id.clone(),
                request_id: configured(RequestId::try_new(internal_id.as_str()))?,
                correlation_id: configured(CorrelationId::try_new(
                    attempt.attempt_id().as_str(),
                ))?,
                causation_id: configured(CausationId::try_new(policy_decision_id))?,
                trace_id: configured(TraceId::try_new(attempt_id))?,
                capability_id: configured(CapabilityId::try_new(
                    RECORD_APPLICATION_OUTCOME_CAPABILITY,
                ))?,
                capability_version: configured(CapabilityVersion::try_new(
                    support::CONTRACT_VERSION,
                ))?,
                idempotency_key: configured(IdempotencyKey::try_new(internal_id.as_str()))?,
                business_transaction_id: configured(BusinessTransactionId::try_new(
                    internal_id.as_str(),
                ))?,
                schema_version: configured(SchemaVersion::try_new(support::CONTRACT_VERSION))?,
                request_started_at_unix_nanos: recorded_at_unix_nanos,
            },
        },
        input,
        input_hash,
        approval: None,
    })
}

fn validate_completed_attempt(
    pending: &wire::ApplicationAttempt,
    completed: &wire::ApplicationAttempt,
    expected_outcome: &wire::ApplicationOutcome,
    recorded_at_unix_ms: i64,
) -> Result<(), SdkError> {
    if application_attempt_id(pending)? != application_attempt_id(completed)?
        || pending.suggestion_ref != completed.suggestion_ref
        || pending.review_decision_ref != completed.review_decision_ref
        || pending.target != completed.target
        || pending.proposed_value_digest != completed.proposed_value_digest
        || pending.application_generation != completed.application_generation
        || pending.owner_capability_id != completed.owner_capability_id
        || pending.owner_capability_version != completed.owner_capability_version
        || pending.target_idempotency_key != completed.target_idempotency_key
        || pending.planned_at_unix_ms != completed.planned_at_unix_ms
    {
        return Err(orchestration_output_invalid(
            "outcome transaction changed immutable application-attempt lineage",
        ));
    }
    let recorded = completed.recorded_outcome.as_ref().ok_or_else(|| {
        orchestration_output_invalid("outcome transaction did not append outcome evidence")
    })?;
    if recorded.recorded_at_unix_ms != recorded_at_unix_ms
        || recorded.outcome.as_ref() != Some(expected_outcome)
    {
        return Err(orchestration_output_invalid(
            "recorded application outcome differs from the exact policy/owner result",
        ));
    }
    Ok(())
}

fn validate_policy_decision(decision: &EnrichmentPolicyDecision) -> Result<(), SdkError> {
    validate_internal_id(
        decision.decision_id(),
        "owner-application policy decision",
    )?;
    validate_version(
        decision.policy_version(),
        "owner-application policy version",
    )?;
    if let EnrichmentPolicyDecision::Denied {
        safe_reason_code, ..
    } = decision
    {
        validate_internal_id(safe_reason_code, "owner-application policy denial code")?;
    }
    Ok(())
}

fn decode_output<T>(
    result: &CapabilityExecutionResult,
    schema: &'static str,
    data_class: DataClass,
) -> Result<T, SdkError>
where
    T: Message + Default,
{
    let payload = result
        .output
        .as_ref()
        .ok_or_else(|| orchestration_output_invalid("capability output payload is missing"))?;
    if payload.owner.as_str() != MODULE_ID
        || payload.schema_id.as_str() != schema
        || payload.schema_version.as_str() != support::CONTRACT_VERSION
        || payload.descriptor_hash != support::message_descriptor_hash(schema)
        || payload.data_class != data_class
        || payload.encoding != PayloadEncoding::Protobuf
        || payload.maximum_size_bytes != support::MAX_PROTOBUF_BYTES
        || payload.validate().is_err()
    {
        return Err(orchestration_output_invalid(
            "capability output did not match the exact typed contract",
        ));
    }
    T::decode(payload.bytes.as_slice())
        .map_err(|error| orchestration_output_invalid(error.to_string()))
}

fn application_attempt_id(attempt: &wire::ApplicationAttempt) -> Result<&str, SdkError> {
    attempt
        .application_attempt_ref
        .as_ref()
        .map(|reference| reference.application_attempt_id.as_str())
        .ok_or_else(|| orchestration_output_invalid("application-attempt identity is missing"))
}

fn nonnegative_unix_ms(value: i64) -> Result<i64, SdkError> {
    if value < 0 {
        return Err(orchestration_input_invalid(
            "application orchestration clock returned a negative timestamp",
        ));
    }
    Ok(value / 1_000_000)
}

fn checked_i64(value: u64, label: &'static str) -> Result<i64, SdkError> {
    i64::try_from(value).map_err(|_| orchestration_output_invalid(format!("{label} exceeds i64")))
}

fn validate_internal_id(value: &str, label: &'static str) -> Result<(), SdkError> {
    if value.is_empty()
        || value.len() > MAX_INTERNAL_ID_BYTES
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(orchestration_input_invalid(format!(
            "{label} must be non-empty, canonical and at most {MAX_INTERNAL_ID_BYTES} bytes"
        )));
    }
    Ok(())
}

fn validate_version(value: &str, label: &'static str) -> Result<(), SdkError> {
    if value.is_empty()
        || value.len() > 80
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(orchestration_input_invalid(format!(
            "{label} must be non-empty, canonical and at most 80 bytes"
        )));
    }
    Ok(())
}

fn configured<T>(result: Result<T, crm_module_sdk::IdentifierError>) -> Result<T, SdkError> {
    result.map_err(|error| orchestration_input_invalid(error.to_string()))
}

fn orchestration_input_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_APPLICATION_ORCHESTRATION_INPUT_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "The Customer Enrichment application orchestration input is invalid.",
    )
    .with_internal_reference(reference.into())
}

fn orchestration_output_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_APPLICATION_ORCHESTRATION_OUTPUT_INVALID",
        ErrorCategory::Dependency,
        false,
        "Customer Enrichment application evidence was inconsistent.",
    )
    .with_internal_reference(reference.into())
}

fn orchestration_owner_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_APPLICATION_OWNER_RESULT_INVALID",
        ErrorCategory::Dependency,
        false,
        "The authoritative owner application result was inconsistent.",
    )
    .with_internal_reference(reference.into())
}
