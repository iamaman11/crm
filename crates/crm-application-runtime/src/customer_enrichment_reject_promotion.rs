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
    ACCEPT_SUGGESTION_CAPABILITY, ACCEPT_SUGGESTION_REQUEST_SCHEMA, REJECT_SUGGESTION_CAPABILITY,
    REJECT_SUGGESTION_REQUEST_SCHEMA, accept_suggestion_capability_definition,
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
const MAX_APPROVAL_REFERENCE_BYTES: usize = 240;

/// Returns the exact public mutation inventory after promoting suggestion review.
pub fn application_mutation_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    let mut definitions = native_composition::application_mutation_definitions()?;
    definitions.push(reject_suggestion_capability_definition()?);
    definitions.push(accept_suggestion_capability_definition()?);
    Ok(definitions)
}

pub use suggestion_reads::application_query_definitions;

/// Extends the accepted suggestion read surface with exact accept and reject mutations.
pub fn build_production_composition(
    dependencies: ProductionCompositionDependencies,
) -> Result<ApplicationComposition, SdkError> {
    let party_queries = Arc::new(PartyQueryAdapter::new(
        dependencies.store.clone(),
        cursor(dependencies.cursor_key)?,
        dependencies.visibility_authorizer.clone(),
    )?);
    let review_policy: Arc<dyn SuggestionReviewPolicyPort> = Arc::new(
        ProductionSuggestionReviewPolicy::new(party_queries, dependencies.query_authorizer.clone()),
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
            Arc::new(ReviewSuggestionSemanticValidator),
        ));
    let executor: Arc<dyn TransactionalCapabilityExecutor> =
        Arc::new(ReviewSuggestionExecutor::new(review_executor));
    contributions
        .add_mutations(
            [
                reject_suggestion_capability_definition()?,
                accept_suggestion_capability_definition()?,
            ],
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
struct ReviewSuggestionSemanticValidator;

impl CapabilitySemanticValidator for ReviewSuggestionSemanticValidator {
    fn validate<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a CapabilityRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            if definition.owner_module_id.as_str() != MODULE_ID
                || request.context.execution.capability_id != definition.capability_id
                || request.context.execution.capability_version != definition.capability_version
            {
                return Err(review_input_invalid(
                    "request context does not match the exact suggestion review capability",
                ));
            }
            match definition.capability_id.as_str() {
                ACCEPT_SUGGESTION_CAPABILITY => {
                    let command: wire::AcceptSuggestionRequest =
                        support::decode_request_with_data_class(
                            request,
                            MODULE_ID,
                            ACCEPT_SUGGESTION_REQUEST_SCHEMA,
                            DataClass::Personal,
                        )?;
                    validate_review_binding(
                        command.suggestion_ref,
                        command.expected_party_resource_version,
                        command.expected_proposed_value_digest,
                        &command.policy_version,
                        &command.safe_reason_code,
                    )?;
                    if let Some(reference) = command.approval_evidence_reference {
                        validate_reference(
                            &reference,
                            MAX_APPROVAL_REFERENCE_BYTES,
                            "customer_enrichment.approval_evidence_reference",
                        )?;
                    }
                    if command
                        .review_expires_at_unix_ms
                        .is_some_and(|value| value < 0)
                    {
                        return Err(SdkError::invalid_argument(
                            "customer_enrichment.review_expires_at_unix_ms",
                            "Review expiry must not be negative",
                        ));
                    }
                    Ok(())
                }
                REJECT_SUGGESTION_CAPABILITY => {
                    let command: wire::RejectSuggestionRequest =
                        support::decode_request_with_data_class(
                            request,
                            MODULE_ID,
                            REJECT_SUGGESTION_REQUEST_SCHEMA,
                            DataClass::Personal,
                        )?;
                    validate_review_binding(
                        command.suggestion_ref,
                        command.expected_party_resource_version,
                        command.expected_proposed_value_digest,
                        &command.policy_version,
                        &command.safe_reason_code,
                    )
                }
                _ => Err(review_input_invalid(
                    "only exact suggestion acceptance and rejection are configured",
                )),
            }
        })
    }
}

fn validate_review_binding(
    suggestion_ref: Option<wire::SuggestionRef>,
    expected_party_resource_version: i64,
    expected_proposed_value_digest: Vec<u8>,
    policy_version: &str,
    safe_reason_code: &str,
) -> Result<(), SdkError> {
    let suggestion_id = suggestion_ref
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
    if expected_party_resource_version <= 0 {
        return Err(SdkError::invalid_argument(
            "customer_enrichment.expected_party_resource_version",
            "Expected Party resource version must be greater than zero",
        ));
    }
    if expected_proposed_value_digest.len() != 32 {
        return Err(SdkError::invalid_argument(
            "customer_enrichment.expected_proposed_value_digest",
            "Expected proposed-value digest must contain exactly 32 bytes",
        ));
    }
    validate_token(
        policy_version,
        80,
        "customer_enrichment.policy_version",
    )?;
    validate_token(
        safe_reason_code,
        80,
        "customer_enrichment.safe_reason_code",
    )
}

#[derive(Clone)]
struct ReviewSuggestionExecutor {
    inner: Arc<PostgresCustomerEnrichmentSuggestionReviewExecutor>,
}

impl ReviewSuggestionExecutor {
    fn new(inner: Arc<PostgresCustomerEnrichmentSuggestionReviewExecutor>) -> Self {
        Self { inner }
    }
}

impl fmt::Debug for ReviewSuggestionExecutor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReviewSuggestionExecutor")
            .field(
                "inner",
                &"PostgresCustomerEnrichmentSuggestionReviewExecutor",
            )
            .finish()
    }
}

impl TransactionalCapabilityExecutor for ReviewSuggestionExecutor {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: CapabilityRequest,
    ) -> PortFuture<'a, Result<CapabilityExecutionResult, SdkError>> {
        Box::pin(async move {
            if !matches!(
                definition.capability_id.as_str(),
                ACCEPT_SUGGESTION_CAPABILITY | REJECT_SUGGESTION_CAPABILITY
            ) || definition.owner_module_id.as_str() != MODULE_ID
            {
                return Err(review_input_invalid(
                    "executor received a capability other than exact suggestion review",
                ));
            }
            self.inner.execute(request).await
        })
    }
}

#[derive(Clone)]
struct ProductionSuggestionReviewPolicy {
    party_queries: Arc<PartyQueryAdapter>,
    query_authorizer: Arc<dyn QueryAuthorizer>,
}

impl ProductionSuggestionReviewPolicy {
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

impl fmt::Debug for ProductionSuggestionReviewPolicy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductionSuggestionReviewPolicy")
            .field("party_queries", &"PartyQueryAdapter")
            .field("query_authorizer", &"dyn QueryAuthorizer")
            .finish()
    }
}

impl SuggestionReviewPolicyPort for ProductionSuggestionReviewPolicy {
    fn evaluate<'a>(
        &'a self,
        request: SuggestionReviewPolicyRequest,
    ) -> PortFuture<'a, Result<SuggestionReviewPolicyDecision, SdkError>> {
        Box::pin(async move {
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
                        acceptance_approval_requirement: match request.decision_kind {
                            ReviewDecisionKind::Accepted => ApprovalRequirement::Required,
                            ReviewDecisionKind::Rejected => ApprovalRequirement::NotRequired,
                        },
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
    let decision_kind = match request.decision_kind {
        ReviewDecisionKind::Accepted => "accepted",
        ReviewDecisionKind::Rejected => "rejected",
    };
    let mut hasher = Sha256::new();
    for value in [
        "crm.customer-enrichment.review-policy/v1",
        request.tenant_id.as_str(),
        request.actor_id.as_str(),
        request.suggestion_id.as_str(),
        request.party_id.as_str(),
        decision_kind,
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

fn validate_reference(value: &str, maximum: usize, field: &'static str) -> Result<(), SdkError> {
    if value.is_empty()
        || value.len() > maximum
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(SdkError::invalid_argument(
            field,
            "Evidence reference is not canonical",
        ));
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

fn review_input_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_SUGGESTION_REVIEW_INPUT_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "The suggestion review input is invalid.",
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
