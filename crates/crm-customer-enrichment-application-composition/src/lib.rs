#![forbid(unsafe_code)]

//! PostgreSQL composition for governed suggestion application evidence, recovery and worker execution.
//!
//! The attempt and outcome executors preserve append-once evidence. The orchestration layer commits
//! the deterministic attempt before policy or owner I/O, invokes only the governed Party boundary,
//! records one exact outcome and reads completed attempts before replaying external work. The
//! event-driven worker consumes accepted review evidence through a durable checkpoint.

mod orchestration;
mod owner_application;
mod worker;

pub use orchestration::*;
pub use owner_application::*;
pub use worker::*;

use crm_capability_plan_support as support;
use crm_capability_runtime::{
    CapabilityAuthorizer, CapabilityDefinition, CapabilityExecutionResult, CapabilityRequest,
    TransactionalCapabilityExecutor,
};
use crm_core_data::{PostgresDataStore, PostgresTransactionalAggregateExecutor};
use crm_customer_enrichment::{
    APPLICATION_ATTEMPT_RECORD_TYPE, REVIEW_DECISION_RECORD_TYPE, SUGGESTION_RECORD_TYPE,
};
use crm_customer_enrichment_application_adapter::{
    APPLY_PARTY_DISPLAY_NAME_CAPABILITY, APPLY_PARTY_DISPLAY_NAME_REQUEST_SCHEMA,
    CustomerEnrichmentApplicationAttemptPlanner, CustomerEnrichmentApplicationOutcomePlanner,
    RECORD_APPLICATION_OUTCOME_CAPABILITY, RECORD_APPLICATION_OUTCOME_REQUEST_SCHEMA,
    application_attempt_from_snapshot, application_attempt_to_wire,
    apply_party_display_name_capability_definition,
    record_application_outcome_capability_definition, review_from_application_snapshot,
    suggestion_from_application_snapshot,
};
use crm_customer_enrichment_capability_adapter::MODULE_ID;
use crm_module_sdk::{DataClass, ErrorCategory, RecordId, RecordRef, RecordSnapshot, SdkError};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;
use std::fmt;
use std::sync::Arc;

pub const CRATE_NAME: &str = "crm-customer-enrichment-application-composition";

#[derive(Debug, Clone)]
pub struct PostgresCustomerEnrichmentApplicationAttemptExecutor {
    store: PostgresDataStore,
}

impl PostgresCustomerEnrichmentApplicationAttemptExecutor {
    pub fn new(store: PostgresDataStore) -> Self {
        Self { store }
    }

    pub async fn execute(
        &self,
        request: CapabilityRequest,
    ) -> Result<CapabilityExecutionResult, SdkError> {
        let definition = apply_party_display_name_capability_definition()?;
        ensure_exact_definition(&definition, &request, APPLY_PARTY_DISPLAY_NAME_CAPABILITY)?;
        let command: wire::ApplyPartyDisplayNameSuggestionRequest =
            support::decode_request_with_data_class(
                &request,
                MODULE_ID,
                APPLY_PARTY_DISPLAY_NAME_REQUEST_SCHEMA,
                DataClass::Personal,
            )?;
        let suggestion_reference = required_suggestion_ref(command.suggestion_ref)?;
        let review_reference = required_review_ref(command.review_decision_ref)?;
        let suggestion_snapshot = self
            .load_required(&request, &suggestion_reference, suggestion_not_found())
            .await?;
        let review_snapshot = self
            .load_required(&request, &review_reference, review_not_found())
            .await?;
        let suggestion = suggestion_from_application_snapshot(&suggestion_snapshot)?;
        let review = review_from_application_snapshot(&review_snapshot)?;
        let planner = CustomerEnrichmentApplicationAttemptPlanner::new(suggestion, review);
        PostgresTransactionalAggregateExecutor::new(self.store.clone(), Arc::new(planner))
            .execute(&definition, request)
            .await
    }

    async fn load_required(
        &self,
        request: &CapabilityRequest,
        reference: &RecordRef,
        not_found: SdkError,
    ) -> Result<RecordSnapshot, SdkError> {
        self.store
            .get_record(&request.context, reference)
            .await
            .map_err(|error| application_store_unavailable(error.to_string()))?
            .ok_or(not_found)
    }
}

#[derive(Clone)]
pub struct PostgresCustomerEnrichmentApplicationOutcomeExecutor {
    store: PostgresDataStore,
    authorizer: Option<Arc<dyn CapabilityAuthorizer>>,
}

impl fmt::Debug for PostgresCustomerEnrichmentApplicationOutcomeExecutor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostgresCustomerEnrichmentApplicationOutcomeExecutor")
            .field("store", &self.store)
            .field(
                "authorizer",
                &self.authorizer.as_ref().map(|_| "dyn CapabilityAuthorizer"),
            )
            .finish()
    }
}

impl PostgresCustomerEnrichmentApplicationOutcomeExecutor {
    pub fn new(store: PostgresDataStore) -> Self {
        Self {
            store,
            authorizer: None,
        }
    }

    pub fn authorized(store: PostgresDataStore, authorizer: Arc<dyn CapabilityAuthorizer>) -> Self {
        Self {
            store,
            authorizer: Some(authorizer),
        }
    }

    pub async fn execute(
        &self,
        request: CapabilityRequest,
    ) -> Result<CapabilityExecutionResult, SdkError> {
        let definition = record_application_outcome_capability_definition()?;
        ensure_exact_definition(&definition, &request, RECORD_APPLICATION_OUTCOME_CAPABILITY)?;
        self.authorize(&definition, &request).await?;
        let command: wire::RecordApplicationOutcomeRequest =
            support::decode_request_with_data_class(
                &request,
                MODULE_ID,
                RECORD_APPLICATION_OUTCOME_REQUEST_SCHEMA,
                DataClass::Personal,
            )?;
        let attempt_reference = required_attempt_ref(command.application_attempt_ref)?;
        let attempt_snapshot = self
            .load_required(
                &request,
                &attempt_reference,
                application_attempt_not_found(),
            )
            .await?;
        let attempt = application_attempt_from_snapshot(&attempt_snapshot)?;
        let public_attempt = application_attempt_to_wire(&attempt)?;
        let suggestion_reference = required_suggestion_ref(public_attempt.suggestion_ref)?;
        let review_reference = required_review_ref(public_attempt.review_decision_ref)?;
        let suggestion_snapshot = self
            .load_required(&request, &suggestion_reference, suggestion_not_found())
            .await?;
        let review_snapshot = self
            .load_required(&request, &review_reference, review_not_found())
            .await?;
        let suggestion = suggestion_from_application_snapshot(&suggestion_snapshot)?;
        let review = review_from_application_snapshot(&review_snapshot)?;
        let planner = CustomerEnrichmentApplicationOutcomePlanner::new(suggestion, review);
        PostgresTransactionalAggregateExecutor::new(self.store.clone(), Arc::new(planner))
            .execute(&definition, request)
            .await
    }

    async fn authorize(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<(), SdkError> {
        let Some(authorizer) = &self.authorizer else {
            return Ok(());
        };
        let decision = authorizer.authorize(definition, request).await?;
        if decision.allowed {
            return Ok(());
        }
        Err(
            application_outcome_permission_denied().with_internal_reference(format!(
                "decision_id={};reason_code={};policy_version={}",
                decision.decision_id, decision.reason_code, decision.policy_version
            )),
        )
    }

    async fn load_required(
        &self,
        request: &CapabilityRequest,
        reference: &RecordRef,
        not_found: SdkError,
    ) -> Result<RecordSnapshot, SdkError> {
        self.store
            .get_record(&request.context, reference)
            .await
            .map_err(|error| application_store_unavailable(error.to_string()))?
            .ok_or(not_found)
    }
}

fn ensure_exact_definition(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    capability_id: &'static str,
) -> Result<(), SdkError> {
    if definition.capability_id.as_str() != capability_id
        || definition.owner_module_id.as_str() != MODULE_ID
        || definition.capability_id != request.context.execution.capability_id
        || definition.capability_version != request.context.execution.capability_version
    {
        return Err(application_input_invalid(
            "request context does not match the exact application capability definition",
        ));
    }
    Ok(())
}

fn required_suggestion_ref(reference: Option<wire::SuggestionRef>) -> Result<RecordRef, SdkError> {
    let reference = reference.ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_enrichment.suggestion_ref",
            "Suggestion reference is required",
        )
    })?;
    required_record_ref(
        SUGGESTION_RECORD_TYPE,
        reference.suggestion_id,
        "customer_enrichment.suggestion_ref.suggestion_id",
    )
}

fn required_review_ref(reference: Option<wire::ReviewDecisionRef>) -> Result<RecordRef, SdkError> {
    let reference = reference.ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_enrichment.review_decision_ref",
            "Review-decision reference is required",
        )
    })?;
    required_record_ref(
        REVIEW_DECISION_RECORD_TYPE,
        reference.review_decision_id,
        "customer_enrichment.review_decision_ref.review_decision_id",
    )
}

fn required_attempt_ref(
    reference: Option<wire::ApplicationAttemptRef>,
) -> Result<RecordRef, SdkError> {
    let reference = reference.ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_enrichment.application_attempt_ref",
            "Application-attempt reference is required",
        )
    })?;
    required_record_ref(
        APPLICATION_ATTEMPT_RECORD_TYPE,
        reference.application_attempt_id,
        "customer_enrichment.application_attempt_ref.application_attempt_id",
    )
}

fn required_record_ref(
    record_type: &'static str,
    record_id: String,
    field: &'static str,
) -> Result<RecordRef, SdkError> {
    let record_id = RecordId::try_new(record_id)
        .map_err(|error| SdkError::invalid_argument(field, error.to_string()))?;
    support::record_ref(record_type, record_id.as_str(), field)
}

fn suggestion_not_found() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_SUGGESTION_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The requested suggestion was not found.",
    )
}

fn review_not_found() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_REVIEW_DECISION_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The requested review decision was not found.",
    )
}

fn application_attempt_not_found() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_APPLICATION_ATTEMPT_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The requested application attempt was not found.",
    )
}

fn application_outcome_permission_denied() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_APPLICATION_OUTCOME_PERMISSION_DENIED",
        ErrorCategory::Authorization,
        false,
        "The application worker is not authorized to persist the application outcome.",
    )
}

fn application_input_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_APPLICATION_INPUT_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "The Customer Enrichment application input is invalid.",
    )
    .with_internal_reference(reference.into())
}

fn application_store_unavailable(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_APPLICATION_STORE_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "Customer Enrichment application evidence could not be loaded.",
    )
    .with_internal_reference(reference.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_references_are_exact_and_typed() {
        let suggestion = required_suggestion_ref(Some(wire::SuggestionRef {
            suggestion_id: "suggestion-a".to_owned(),
        }))
        .unwrap();
        assert_eq!(suggestion.record_type.as_str(), SUGGESTION_RECORD_TYPE);
        assert_eq!(suggestion.record_id.as_str(), "suggestion-a");

        let review = required_review_ref(Some(wire::ReviewDecisionRef {
            review_decision_id: "review-a".to_owned(),
        }))
        .unwrap();
        assert_eq!(review.record_type.as_str(), REVIEW_DECISION_RECORD_TYPE);
        assert_eq!(review.record_id.as_str(), "review-a");

        let attempt = required_attempt_ref(Some(wire::ApplicationAttemptRef {
            application_attempt_id: "attempt-a".to_owned(),
        }))
        .unwrap();
        assert_eq!(
            attempt.record_type.as_str(),
            APPLICATION_ATTEMPT_RECORD_TYPE
        );
        assert_eq!(attempt.record_id.as_str(), "attempt-a");
    }
}
