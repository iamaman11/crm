use crate::customer_enrichment_suggestion_list_promotion as suggestion_reads;
use crate::native_composition::{self, ProductionCompositionDependencies};
use crm_application_composition::{
    ActivationGatedMutationValidator, ApplicationComposition, ModuleContributionSet,
};
use crm_capability_plan_support as support;
use crm_capability_runtime::{
    CapabilityDefinition, CapabilityExecutionResult, CapabilityRequest,
    CapabilitySemanticValidator, TransactionalCapabilityExecutor,
};
use crm_customer_enrichment::{
    ApprovalRequirement, ReviewDecisionKind, SuggestionReviewPolicyDecision,
    SuggestionReviewPolicyPort, SuggestionReviewPolicyRequest, TargetField,
};
use crm_customer_enrichment_capability_adapter::MODULE_ID;
use crm_customer_enrichment_review_adapter::{
    REJECT_SUGGESTION_CAPABILITY, REJECT_SUGGESTION_REQUEST_SCHEMA,
    reject_suggestion_capability_definition,
};
use crm_customer_enrichment_review_composition::PostgresCustomerEnrichmentSuggestionReviewExecutor;
use crm_module_sdk::{DataClass, ErrorCategory, ModuleId, PortFuture, RecordId, SdkError};
use crm_parties_query_adapter::{
    GET_CAPABILITY as PARTY_GET_CAPABILITY, PartyExportExecutionRead, PartyQueryAdapter,
    export_execution_query_request, query_capability_definition as party_query_definition,
};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;
use crm_query_runtime::{CursorCodec, QueryAuthorizer, QuerySemanticValidator};
use sha2::{Digest, Sha256};
use std::fmt;
use std::sync::Arc;

pub const PRODUCTION_REVIEW_POLICY_VERSION: &str = "review-policy-v1";
const DISPLAY_NAME_FIELD: &str = "display_name";

/// Returns the exact public mutation inventory after promoting suggestion rejection.
pub fn application_mutation_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    let mut definitions = native_composition::application_mutation_definitions()?;
    definitions.push(reject_suggestion_capability_definition()?);
    Ok(definitions)
}

pub use suggestion_reads::application_query_definitions;

/// Extends the accepted suggestion read surface with exactly one public review mutation.
/// Acceptance remains non-runtime until its separate promotion slice is complete.
pub fn build_production_composition(
    dependencies: ProductionCompositionDependencies,
) -> Result<ApplicationComposition, SdkError> {
    let party_queries = Arc::new(PartyQueryAdapter::new(
        dependencies.store.clone(),
        cursor(dependencies.cursor_key)?,
        dependencies.visibility_authorizer.clone(),
    )?);
    let review_policy: Arc<dyn SuggestionReviewPolicyPort> = Arc::new(
        ProductionSuggestionRejectPolicy::new(party_queries, dependencies.query_authorizer.clone()),
    );
    let review_executor = Arc::new(PostgresCustomerEnrichmentSuggestionReviewExecutor::new(
        dependencies.store.clone(),
        review_policy,
    ));

    let base_dependencies = ProductionCompositionDependencies {
        store: dependencies.store,
        activation: dependencies.activation.clone(),
        capability_authorizer: dependencies.capability_authorizer,
        query_authorizer: dependencies.query_authorizer,
        visibility_authorizer: dependencies.visibility_authorizer,
        cursor_key: dependencies.cursor_key,
    };
    let base = suggestion_reads::build_production_composition(base_dependencies)?;
    let mut contributions = ModuleContributionSet::new();
    contributions
        .add_mutations(
            base.mutation_definitions().iter().cloned(),
            base.mutation_validator(),
            base.mutation_executor(),
        )
        .map_err(composition_error)?;
    contributions
        .add_queries(
            base.query_definitions().iter().cloned(),
            base.query_validator(),
            base.query_executor(),
        )
        .map_err(composition_error)?;

    let validator: Arc<dyn CapabilitySemanticValidator> =
        Arc::new(ActivationGatedMutationValidator::new(
            dependencies.activation,
            Arc::new(RejectSuggestionSemanticValidator),
        ));
    let executor: Arc<dyn TransactionalCapabilityExecutor> =
        Arc::new(RejectSuggestionExecutor::new(review_executor));
    contributions
        .add_mutations(
            [reject_suggestion_capability_definition()?],
            validator,
            executor,
        )
        .map_err(composition_error)?;

    for module_id in base.module_ids() {
        contributions
            .add_empty_module(ModuleId::try_new(module_id.clone()).map_err(configuration_error)?)
            .map_err(composition_error)?;
    }
    contributions.build().map_err(composition_error)
}

#[derive(Debug, Clone, Copy)]
struct RejectSuggestionSemanticValidator;

impl CapabilitySemanticValidator for RejectSuggestionSemanticValidator {
    fn validate<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a CapabilityRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            if definition.capability_id.as_str() != REJECT_SUGGESTION_CAPABILITY
                || definition.owner_module_id.as_str() != MODULE_ID
                || request.context.execution.capability_id != definition.capability_id
                || request.context.execution.capability_version != definition.capability_version
            {
                return Err(reject_input_invalid(
                    "request context does not match the exact rejection capability",
                ));
            }
            let command: wire::RejectSuggestionRequest = support::decode_request_with_data_class(
                request,
                MODULE_ID,
                REJECT_SUGGESTION_REQUEST_SCHEMA,
                DataClass::Personal,
            )?;
            let suggestion_id = command
                .suggestion_ref
                .ok_or_else(|| {
                    SdkError::invalid_argument(
                        "customer_enrichment.suggestion_ref",
                        "Suggestion reference is required",
                    )
                })?
                .suggestion_id;
            RecordId::try_new(suggestion_id).map_err(|error| {
                SdkError::invalid_argument(
                    "customer_enrichment.suggestion_ref.suggestion_id",
                    error.to_string(),
                )
            })?;
            if command.expected_party_resource_version <= 0 {
                return Err(SdkError::invalid_argument(
                    "customer_enrichment.expected_party_resource_version",
                    "Expected Party resource version must be greater than zero",
                ));
            }
            if command.expected_proposed_value_digest.len() != 32 {
                return Err(SdkError::invalid_argument(
                    "customer_enrichment.expected_proposed_value_digest",
                    "Expected proposed-value digest must contain exactly 32 bytes",
                ));
            }
            validate_token(
                &command.policy_version,
                80,
                "customer_enrichment.policy_version",
            )?;
            validate_token(
                &command.safe_reason_code,
                80,
                "customer_enrichment.safe_reason_code",
            )?;
            Ok(())
        })
    }
}

#[derive(Clone)]
struct RejectSuggestionExecutor {
    inner: Arc<PostgresCustomerEnrichmentSuggestionReviewExecutor>,
}

impl RejectSuggestionExecutor {
    fn new(inner: Arc<PostgresCustomerEnrichmentSuggestionReviewExecutor>) -> Self {
        Self { inner }
    }
}

impl fmt::Debug for RejectSuggestionExecutor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RejectSuggestionExecutor")
            .field(
                "inner",
                &"PostgresCustomerEnrichmentSuggestionReviewExecutor",
            )
            .finish()
    }
}

impl TransactionalCapabilityExecutor for RejectSuggestionExecutor {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: CapabilityRequest,
    ) -> PortFuture<'a, Result<CapabilityExecutionResult, SdkError>> {
        Box::pin(async move {
            if definition.capability_id.as_str() != REJECT_SUGGESTION_CAPABILITY
                || definition.owner_module_id.as_str() != MODULE_ID
            {
                return Err(reject_input_invalid(
                    "executor received a capability other than exact suggestion rejection",
                ));
            }
            self.inner.execute(request).await
        })
    }
}

#[derive(Clone)]
struct ProductionSuggestionRejectPolicy {
    party_queries: Arc<PartyQueryAdapter>,
    query_authorizer: Arc<dyn QueryAuthorizer>,
}

impl ProductionSuggestionRejectPolicy {
    fn new(
        party_queries: Arc<PartyQueryAdapter>,
        query_authorizer: Arc<dyn QueryAuthorizer>,
    ) -> Self {
        Self {
            party_queries,
            query_authorizer,
        }
    }
}

impl fmt::Debug for ProductionSuggestionRejectPolicy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductionSuggestionRejectPolicy")
            .field("party_queries", &"PartyQueryAdapter")
            .field("query_authorizer", &"dyn QueryAuthorizer")
            .finish()
    }
}

impl SuggestionReviewPolicyPort for ProductionSuggestionRejectPolicy {
    fn evaluate<'a>(
        &'a self,
        request: SuggestionReviewPolicyRequest,
    ) -> PortFuture<'a, Result<SuggestionReviewPolicyDecision, SdkError>> {
        Box::pin(async move {
            if request.decision_kind != ReviewDecisionKind::Rejected {
                return Ok(policy_denied(
                    &request,
                    "accept_not_promoted",
                    "not-promoted",
                ));
            }
            if request.target_field != TargetField::PartyDisplayName
                || !canonical_evidence(&request.purpose_code)
                || !canonical_evidence(&request.legal_basis_code)
            {
                return Err(review_policy_invalid(
                    "suggestion policy evidence is not canonical",
                ));
            }
            let evaluated_at_unix_nanos = request
                .evaluated_at_unix_ms
                .checked_mul(1_000_000)
                .ok_or_else(|| review_policy_invalid("review evaluation timestamp overflow"))?;
            let query = export_execution_query_request(
                &request.tenant_id,
                &request.actor_id,
                &request.request_identity,
                &request.party_id,
                evaluated_at_unix_nanos,
            )?;
            let definition = party_query_definition(PARTY_GET_CAPABILITY)?;
            let authorization = self.query_authorizer.authorize(&definition, &query).await?;
            if !authorization.allowed {
                return Ok(policy_denied(
                    &request,
                    "party_permission_denied",
                    &authorization.decision_id,
                ));
            }
            self.party_queries.validate(&definition, &query).await?;
            match self
                .party_queries
                .get_for_export_execution(&query, &request.party_id, request.party_resource_version)
                .await?
            {
                PartyExportExecutionRead::Visible { allowed_fields, .. }
                    if allowed_fields.contains(DISPLAY_NAME_FIELD) =>
                {
                    Ok(SuggestionReviewPolicyDecision::Allowed {
                        decision_id: policy_decision_id(&request, &authorization.decision_id),
                        policy_version: PRODUCTION_REVIEW_POLICY_VERSION.to_owned(),
                        acceptance_approval_requirement: ApprovalRequirement::NotRequired,
                    })
                }
                PartyExportExecutionRead::VersionChanged => Err(SdkError::new(
                    "CUSTOMER_ENRICHMENT_SUGGESTION_REVIEW_TARGET_STALE",
                    ErrorCategory::Conflict,
                    false,
                    "The Party resource version changed before the suggestion review was authorized.",
                )),
                PartyExportExecutionRead::NotVisible
                | PartyExportExecutionRead::Unavailable
                | PartyExportExecutionRead::Visible { .. } => Ok(policy_denied(
                    &request,
                    "party_not_visible",
                    &authorization.decision_id,
                )),
            }
        })
    }
}

fn policy_denied(
    request: &SuggestionReviewPolicyRequest,
    safe_reason_code: &str,
    evidence: &str,
) -> SuggestionReviewPolicyDecision {
    SuggestionReviewPolicyDecision::Denied {
        decision_id: policy_decision_id(request, evidence),
        policy_version: PRODUCTION_REVIEW_POLICY_VERSION.to_owned(),
        safe_reason_code: safe_reason_code.to_owned(),
    }
}

fn policy_decision_id(request: &SuggestionReviewPolicyRequest, evidence: &str) -> String {
    let mut hasher = Sha256::new();
    for value in [
        "crm.customer-enrichment.review-policy/v1",
        request.tenant_id.as_str(),
        request.actor_id.as_str(),
        request.suggestion_id.as_str(),
        request.party_id.as_str(),
        evidence,
    ] {
        hasher.update((value.len() as u64).to_be_bytes());
        hasher.update(value.as_bytes());
    }
    let digest = hasher.finalize();
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(encoded, "{byte:02x}");
    }
    format!("review-policy-{encoded}")
}

fn canonical_evidence(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 120
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn validate_token(value: &str, maximum: usize, field: &'static str) -> Result<(), SdkError> {
    if value.is_empty()
        || value.len() > maximum
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(SdkError::invalid_argument(field, "Value is not canonical"));
    }
    Ok(())
}

fn cursor(key: [u8; 32]) -> Result<CursorCodec, SdkError> {
    CursorCodec::new(key).map_err(|error| {
        SdkError::new(
            "APPLICATION_CURSOR_CONFIGURATION_INVALID",
            ErrorCategory::Internal,
            false,
            "The application cursor configuration is invalid.",
        )
        .with_internal_reference(error.to_string())
    })
}

fn reject_input_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_SUGGESTION_REJECT_INPUT_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "The suggestion rejection input is invalid.",
    )
    .with_internal_reference(reference.into())
}

fn review_policy_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_SUGGESTION_REVIEW_POLICY_INVALID",
        ErrorCategory::Internal,
        false,
        "The suggestion review policy could not be evaluated safely.",
    )
    .with_internal_reference(reference.into())
}

fn composition_error(error: impl fmt::Display) -> SdkError {
    SdkError::new(
        "APPLICATION_COMPOSITION_INVALID",
        ErrorCategory::Internal,
        false,
        "The production application composition is invalid.",
    )
    .with_internal_reference(error.to_string())
}

fn configuration_error(error: impl fmt::Display) -> SdkError {
    SdkError::new(
        "APPLICATION_COMPOSITION_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The production application composition configuration is invalid.",
    )
    .with_internal_reference(error.to_string())
}
