#![forbid(unsafe_code)]

//! PostgreSQL composition for non-runtime governed suggestion review.

use crm_capability_plan_support as support;
use crm_capability_runtime::{
    CapabilityExecutionResult, CapabilityRequest, TransactionalCapabilityExecutor,
};
use crm_core_data::{PostgresDataStore, PostgresTransactionalAggregateExecutor};
use crm_customer_enrichment::{
    ApprovalRequirement, EnrichmentRequestId, MappingVersionId, ProviderProfileVersionId,
    ReviewDecisionKind, Suggestion, SuggestionId, SuggestionReviewPolicyDecision,
    SuggestionReviewPolicyPort, SuggestionReviewPolicyRequest, TargetSnapshot,
    encode_suggestion_state,
};
use crm_customer_enrichment_capability_adapter::MODULE_ID;
use crm_customer_enrichment_review_adapter::{
    ACCEPT_SUGGESTION_CAPABILITY, ACCEPT_SUGGESTION_REQUEST_SCHEMA,
    CustomerEnrichmentSuggestionReviewPlanner, REJECT_SUGGESTION_CAPABILITY,
    REJECT_SUGGESTION_REQUEST_SCHEMA, accept_suggestion_capability_definition,
    reject_suggestion_capability_definition, suggestion_from_snapshot, suggestion_record_ref,
};
use crm_module_sdk::{DataClass, ErrorCategory, RecordId, SdkError};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;
use serde::Deserialize;
use std::sync::Arc;

pub const CRATE_NAME: &str = "crm-customer-enrichment-review-composition";

#[derive(Clone)]
pub struct PostgresCustomerEnrichmentSuggestionReviewExecutor {
    store: PostgresDataStore,
    policy: Arc<dyn SuggestionReviewPolicyPort>,
}

impl std::fmt::Debug for PostgresCustomerEnrichmentSuggestionReviewExecutor {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PostgresCustomerEnrichmentSuggestionReviewExecutor")
            .field("store", &self.store)
            .field("policy", &"dyn SuggestionReviewPolicyPort")
            .finish()
    }
}

impl PostgresCustomerEnrichmentSuggestionReviewExecutor {
    pub fn new(store: PostgresDataStore, policy: Arc<dyn SuggestionReviewPolicyPort>) -> Self {
        Self { store, policy }
    }

    pub async fn execute(
        &self,
        request: CapabilityRequest,
    ) -> Result<CapabilityExecutionResult, SdkError> {
        let command = review_command(&request)?;
        let definition = match command.kind {
            ReviewDecisionKind::Accepted => accept_suggestion_capability_definition()?,
            ReviewDecisionKind::Rejected => reject_suggestion_capability_definition()?,
        };
        if request.context.execution.capability_id != definition.capability_id
            || request.context.execution.capability_version != definition.capability_version
        {
            return Err(review_input_invalid(
                "request context does not match the exact review capability definition",
            ));
        }

        let snapshot = self
            .store
            .get_record(
                &request.context,
                &suggestion_record_ref(&command.suggestion_id)?,
            )
            .await
            .map_err(|error| review_store_unavailable(error.to_string()))?
            .ok_or_else(suggestion_not_found)?;
        let suggestion = suggestion_from_snapshot(&snapshot)?;
        if suggestion.suggestion_id().as_str() != command.suggestion_id {
            return Err(review_input_conflict(
                "loaded suggestion identity differs from the review command",
            ));
        }

        let policy_request = suggestion_review_policy_request(&request, &suggestion, command.kind)?;
        let policy_decision = self.policy.evaluate(policy_request).await?;
        let approval_requirement =
            resolve_policy(&command.policy_version, command.kind, policy_decision)?;

        let planner =
            CustomerEnrichmentSuggestionReviewPlanner::new(suggestion, approval_requirement);
        let executor =
            PostgresTransactionalAggregateExecutor::new(self.store.clone(), Arc::new(planner));
        executor.execute(&definition, request).await
    }
}

pub fn suggestion_review_policy_request(
    request: &CapabilityRequest,
    suggestion: &Suggestion,
    decision_kind: ReviewDecisionKind,
) -> Result<SuggestionReviewPolicyRequest, SdkError> {
    let state: SuggestionPolicyState =
        serde_json::from_slice(&encode_suggestion_state(suggestion)?)
            .map_err(|error| review_state_invalid(error.to_string()))?;
    if state.suggestion_id != *suggestion.suggestion_id()
        || state.target != *suggestion.target()
        || state.proposed_value_digest != *suggestion.proposed_value_digest()
    {
        return Err(review_state_invalid(
            "strict suggestion state differs from the rehydrated domain object",
        ));
    }
    let evaluated_at_unix_ms = request.context.execution.request_started_at_unix_nanos / 1_000_000;
    if evaluated_at_unix_ms < 0 {
        return Err(review_input_invalid(
            "review evaluation timestamp must not be negative",
        ));
    }
    let party_resource_version = i64::try_from(state.target.resource_version)
        .map_err(|_| review_state_invalid("Party version exceeds i64"))?;
    Ok(SuggestionReviewPolicyRequest {
        tenant_id: request.context.execution.tenant_id.clone(),
        actor_id: request.context.execution.actor_id.clone(),
        request_identity: request.context.execution.request_id.as_str().to_owned(),
        enrichment_request_id: state.request_id,
        suggestion_id: state.suggestion_id,
        party_id: state.target.resource_id,
        party_resource_version,
        target_field: state.target.target_field,
        provider_profile_version_id: state.provider_profile_version_id,
        mapping_version_id: state.mapping_version_id,
        purpose_code: state.purpose_code,
        legal_basis_code: state.legal_basis_code,
        consent_evidence_reference: state.consent_evidence_reference,
        proposed_value_digest: state.proposed_value_digest,
        decision_kind,
        evaluated_at_unix_ms,
    })
}

struct ReviewCommand {
    suggestion_id: String,
    kind: ReviewDecisionKind,
    policy_version: String,
}

fn review_command(request: &CapabilityRequest) -> Result<ReviewCommand, SdkError> {
    match request.context.execution.capability_id.as_str() {
        ACCEPT_SUGGESTION_CAPABILITY => {
            let command: wire::AcceptSuggestionRequest = support::decode_request_with_data_class(
                request,
                MODULE_ID,
                ACCEPT_SUGGESTION_REQUEST_SCHEMA,
                DataClass::Personal,
            )?;
            Ok(ReviewCommand {
                suggestion_id: required_suggestion_id(command.suggestion_ref)?,
                kind: ReviewDecisionKind::Accepted,
                policy_version: command.policy_version,
            })
        }
        REJECT_SUGGESTION_CAPABILITY => {
            let command: wire::RejectSuggestionRequest = support::decode_request_with_data_class(
                request,
                MODULE_ID,
                REJECT_SUGGESTION_REQUEST_SCHEMA,
                DataClass::Personal,
            )?;
            Ok(ReviewCommand {
                suggestion_id: required_suggestion_id(command.suggestion_ref)?,
                kind: ReviewDecisionKind::Rejected,
                policy_version: command.policy_version,
            })
        }
        _ => Err(review_input_invalid(
            "only exact accept and reject capabilities are supported",
        )),
    }
}

fn required_suggestion_id(reference: Option<wire::SuggestionRef>) -> Result<String, SdkError> {
    let value = reference.ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_enrichment.suggestion_ref",
            "Suggestion reference is required",
        )
    })?;
    Ok(RecordId::try_new(value.suggestion_id)
        .map_err(|error| {
            SdkError::invalid_argument(
                "customer_enrichment.suggestion_ref.suggestion_id",
                error.to_string(),
            )
        })?
        .into_inner())
}

fn resolve_policy(
    supplied_policy_version: &str,
    decision_kind: ReviewDecisionKind,
    decision: SuggestionReviewPolicyDecision,
) -> Result<ApprovalRequirement, SdkError> {
    match decision {
        SuggestionReviewPolicyDecision::Allowed {
            decision_id,
            policy_version,
            acceptance_approval_requirement,
        } => {
            validate_policy_evidence(&decision_id, &policy_version)?;
            if supplied_policy_version != policy_version {
                return Err(review_input_conflict(
                    "review command policy version differs from the final policy decision",
                ));
            }
            Ok(match decision_kind {
                ReviewDecisionKind::Accepted => acceptance_approval_requirement,
                ReviewDecisionKind::Rejected => ApprovalRequirement::NotRequired,
            })
        }
        SuggestionReviewPolicyDecision::Denied {
            decision_id,
            policy_version,
            safe_reason_code,
        } => {
            validate_policy_evidence(&decision_id, &policy_version)?;
            validate_safe_reason(&safe_reason_code)?;
            Err(SdkError::new(
                "CUSTOMER_ENRICHMENT_SUGGESTION_REVIEW_POLICY_DENIED",
                ErrorCategory::Authorization,
                false,
                "The suggestion review is not permitted by the active policy.",
            )
            .with_internal_reference(format!(
                "decision={decision_id};policy={policy_version};reason={safe_reason_code}"
            )))
        }
    }
}

fn validate_policy_evidence(decision_id: &str, policy_version: &str) -> Result<(), SdkError> {
    if decision_id.is_empty()
        || policy_version.is_empty()
        || decision_id.len() > 180
        || policy_version.len() > 80
        || decision_id.trim() != decision_id
        || policy_version.trim() != policy_version
        || decision_id.chars().any(char::is_control)
        || policy_version.chars().any(char::is_control)
    {
        return Err(review_policy_invalid(
            "policy decision identity or version is not canonical",
        ));
    }
    Ok(())
}

fn validate_safe_reason(value: &str) -> Result<(), SdkError> {
    if value.is_empty()
        || value.len() > 80
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(review_policy_invalid(
            "policy denial safe reason is not canonical",
        ));
    }
    Ok(())
}

#[derive(Deserialize)]
struct SuggestionPolicyState {
    suggestion_id: SuggestionId,
    request_id: EnrichmentRequestId,
    provider_profile_version_id: ProviderProfileVersionId,
    mapping_version_id: MappingVersionId,
    target: TargetSnapshot,
    proposed_value_digest: [u8; 32],
    purpose_code: String,
    legal_basis_code: String,
    consent_evidence_reference: Option<String>,
}

fn suggestion_not_found() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_SUGGESTION_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The requested suggestion was not found.",
    )
}

fn review_input_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_SUGGESTION_REVIEW_INPUT_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "The suggestion review input is invalid.",
    )
    .with_internal_reference(reference.into())
}

fn review_input_conflict(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_SUGGESTION_REVIEW_CONFLICT",
        ErrorCategory::Conflict,
        false,
        "The suggestion review no longer matches its exact policy or resource preconditions.",
    )
    .with_internal_reference(reference.into())
}

fn review_store_unavailable(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_SUGGESTION_REVIEW_STORE_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "Suggestion review evidence could not be loaded.",
    )
    .with_internal_reference(reference.into())
}

fn review_state_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_SUGGESTION_REVIEW_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "Stored suggestion review evidence is invalid.",
    )
    .with_internal_reference(reference.into())
}

fn review_policy_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_SUGGESTION_REVIEW_POLICY_INVALID",
        ErrorCategory::Internal,
        false,
        "The suggestion review policy response is invalid.",
    )
    .with_internal_reference(reference.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowed_policy_preserves_acceptance_approval_requirement() {
        let result = resolve_policy(
            "review-policy-v1",
            ReviewDecisionKind::Accepted,
            SuggestionReviewPolicyDecision::Allowed {
                decision_id: "decision-a".to_owned(),
                policy_version: "review-policy-v1".to_owned(),
                acceptance_approval_requirement: ApprovalRequirement::Required,
            },
        )
        .unwrap();
        assert_eq!(result, ApprovalRequirement::Required);
    }

    #[test]
    fn rejection_never_requires_acceptance_approval() {
        let result = resolve_policy(
            "review-policy-v1",
            ReviewDecisionKind::Rejected,
            SuggestionReviewPolicyDecision::Allowed {
                decision_id: "decision-a".to_owned(),
                policy_version: "review-policy-v1".to_owned(),
                acceptance_approval_requirement: ApprovalRequirement::Required,
            },
        )
        .unwrap();
        assert_eq!(result, ApprovalRequirement::NotRequired);
    }

    #[test]
    fn denied_and_mismatched_policy_fail_closed() {
        let denied = resolve_policy(
            "review-policy-v1",
            ReviewDecisionKind::Accepted,
            SuggestionReviewPolicyDecision::Denied {
                decision_id: "decision-a".to_owned(),
                policy_version: "review-policy-v1".to_owned(),
                safe_reason_code: "review_not_permitted".to_owned(),
            },
        )
        .unwrap_err();
        assert_eq!(denied.category, ErrorCategory::Authorization);

        let mismatch = resolve_policy(
            "review-policy-v2",
            ReviewDecisionKind::Accepted,
            SuggestionReviewPolicyDecision::Allowed {
                decision_id: "decision-a".to_owned(),
                policy_version: "review-policy-v1".to_owned(),
                acceptance_approval_requirement: ApprovalRequirement::NotRequired,
            },
        )
        .unwrap_err();
        assert_eq!(mismatch.category, ErrorCategory::Conflict);
    }
}
