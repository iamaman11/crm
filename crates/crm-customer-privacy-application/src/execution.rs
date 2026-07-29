use crm_application_composition::ModuleActivationPort;
use crm_customer_privacy::{
    MODULE_ID, PrivacyOwnerActionAttempt, PrivacyOwnerActionOutcome, PrivacyOwnerOutcomeStatus,
};
use crm_module_sdk::{
    ActorId, CapabilityId, CapabilityVersion, CorrelationId, ModuleId, PortFuture, RecordId,
    RequestId, SdkError, TenantId, TraceId,
};
use std::sync::Arc;

pub const OWNER_ACTION_DISPATCH_CAPABILITY: &str = "customer_privacy.owner_action.dispatch";
pub const OWNER_OUTCOME_RECORD_CAPABILITY: &str = "customer_privacy.owner_outcome.record";
pub const OWNER_EXECUTION_CAPABILITY_VERSION: &str = "1.0.0";

const EXPECTED_OWNER_MODULES: &[&str] = &[
    "crm.consents",
    "crm.contact-points",
    "crm.customer-accounts",
    "crm.customer-data-operations",
    "crm.customer-enrichment",
    "crm.data-quality",
    "crm.identity-resolution",
    "crm.parties",
    "crm.party-relationships",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerExecutionInvocation {
    pub tenant_id: TenantId,
    pub privacy_case_id: RecordId,
    pub action_plan_id: RecordId,
    pub retention_decision_id: RecordId,
    pub actor_id: ActorId,
    pub request_id: RequestId,
    pub correlation_id: CorrelationId,
    pub trace_id: TraceId,
    pub initiating_capability_id: CapabilityId,
    pub initiating_capability_version: CapabilityVersion,
    pub request_started_at_unix_nanos: i64,
    pub planned_at_unix_nanos: i64,
    pub trusted_internal: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerActionRequest {
    pub tenant_id: TenantId,
    pub privacy_case_id: RecordId,
    pub action_plan_id: RecordId,
    pub retention_decision_id: RecordId,
    pub attempt_id: RecordId,
    pub owner_module_id: ModuleId,
    pub owner_capability_id: String,
    pub owner_capability_version: String,
    pub target_idempotency_key: String,
    pub resource_type: String,
    pub resource_id: RecordId,
    pub resource_version: u64,
    pub action_code: String,
    pub actor_id: ActorId,
    pub correlation_id: CorrelationId,
    pub trace_id: TraceId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerActionResult {
    pub status: PrivacyOwnerOutcomeStatus,
    pub safe_failure_code: Option<String>,
}

pub trait OwnerPrivacyActionPort: Send + Sync {
    fn apply<'a>(
        &'a self,
        request: OwnerActionRequest,
    ) -> PortFuture<'a, Result<OwnerActionResult, SdkError>>;
}

#[derive(Clone)]
pub struct OwnerActionEndpoint {
    pub owner_module_id: ModuleId,
    pub executor: Arc<dyn OwnerPrivacyActionPort>,
}

impl std::fmt::Debug for OwnerActionEndpoint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OwnerActionEndpoint")
            .field("owner_module_id", &self.owner_module_id)
            .field("executor", &"dyn OwnerPrivacyActionPort")
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct OwnerActionEndpoints {
    endpoints: Vec<OwnerActionEndpoint>,
}

impl OwnerActionEndpoints {
    pub fn exact_canonical(
        endpoints: impl IntoIterator<Item = OwnerActionEndpoint>,
    ) -> Result<Self, SdkError> {
        let mut endpoints = endpoints.into_iter().collect::<Vec<_>>();
        endpoints.sort_by(|left, right| left.owner_module_id.cmp(&right.owner_module_id));
        if endpoints.len() != EXPECTED_OWNER_MODULES.len()
            || endpoints
                .windows(2)
                .any(|pair| pair[0].owner_module_id == pair[1].owner_module_id)
            || endpoints
                .iter()
                .map(|endpoint| endpoint.owner_module_id.as_str())
                .ne(EXPECTED_OWNER_MODULES.iter().copied())
        {
            return Err(execution_configuration_invalid(
                "owner action endpoints must contain the exact nine canonical owners",
            ));
        }
        Ok(Self { endpoints })
    }

    fn get(
        &self,
        owner_module_id: &ModuleId,
    ) -> Result<&Arc<dyn OwnerPrivacyActionPort>, SdkError> {
        self.endpoints
            .binary_search_by(|endpoint| endpoint.owner_module_id.cmp(owner_module_id))
            .ok()
            .map(|index| &self.endpoints[index].executor)
            .ok_or_else(|| {
                execution_configuration_invalid(
                    "prepared attempt references an owner outside the exact endpoint registry",
                )
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionPreparation {
    Complete {
        total_items: u32,
        durable_outcomes: u32,
    },
    Ready {
        attempt: PrivacyOwnerActionAttempt,
        attempt_replayed: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckpointAdvance {
    pub next_sequence: u32,
    pub total_items: u32,
    pub complete: bool,
}

pub trait OwnerExecutionPersistencePort: Send + Sync {
    fn prepare_next<'a>(
        &'a self,
        invocation: &'a OwnerExecutionInvocation,
    ) -> PortFuture<'a, Result<ExecutionPreparation, SdkError>>;

    fn record_outcome<'a>(
        &'a self,
        invocation: &'a OwnerExecutionInvocation,
        attempt: &'a PrivacyOwnerActionAttempt,
        outcome: &'a PrivacyOwnerActionOutcome,
    ) -> PortFuture<'a, Result<bool, SdkError>>;

    fn advance_checkpoint<'a>(
        &'a self,
        invocation: &'a OwnerExecutionInvocation,
    ) -> PortFuture<'a, Result<CheckpointAdvance, SdkError>>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerExecutionResult {
    pub attempt: Option<PrivacyOwnerActionAttempt>,
    pub outcome: Option<PrivacyOwnerActionOutcome>,
    pub attempt_replayed: bool,
    pub outcome_replayed: bool,
    pub owner_invoked: bool,
    pub next_sequence: u32,
    pub total_items: u32,
    pub complete: bool,
}

#[derive(Clone)]
pub struct PrivacyOwnerExecutionService {
    activation: Arc<dyn ModuleActivationPort>,
    persistence: Arc<dyn OwnerExecutionPersistencePort>,
    endpoints: OwnerActionEndpoints,
}

impl std::fmt::Debug for PrivacyOwnerExecutionService {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PrivacyOwnerExecutionService")
            .field("activation", &"dyn ModuleActivationPort")
            .field("persistence", &"dyn OwnerExecutionPersistencePort")
            .field("endpoints", &self.endpoints)
            .finish()
    }
}

impl PrivacyOwnerExecutionService {
    pub fn new(
        activation: Arc<dyn ModuleActivationPort>,
        persistence: Arc<dyn OwnerExecutionPersistencePort>,
        endpoints: OwnerActionEndpoints,
    ) -> Self {
        Self {
            activation,
            persistence,
            endpoints,
        }
    }

    pub async fn execute_next(
        &self,
        invocation: OwnerExecutionInvocation,
    ) -> Result<OwnerExecutionResult, SdkError> {
        validate_invocation(&invocation)?;
        let module_id = ModuleId::try_new(MODULE_ID).map_err(execution_configuration_invalid)?;
        if !self
            .activation
            .is_active(&invocation.tenant_id, &module_id)
            .await?
        {
            return Err(execution_disabled());
        }

        let (attempt, attempt_replayed) = match self.persistence.prepare_next(&invocation).await? {
            ExecutionPreparation::Ready {
                attempt,
                attempt_replayed,
            } => (attempt, attempt_replayed),
            ExecutionPreparation::Complete {
                total_items,
                durable_outcomes,
            } => {
                return Ok(OwnerExecutionResult {
                    attempt: None,
                    outcome: None,
                    attempt_replayed: true,
                    outcome_replayed: true,
                    owner_invoked: false,
                    next_sequence: total_items.saturating_add(1),
                    total_items,
                    complete: durable_outcomes == total_items,
                });
            }
        };

        let (action_result, owner_invoked) = match attempt.coordinator_outcome_status() {
            Some(status) => (
                OwnerActionResult {
                    status,
                    safe_failure_code: None,
                },
                false,
            ),
            None => {
                let request = owner_request(&invocation, &attempt);
                (
                    self.endpoints
                        .get(attempt.owner_module_id())?
                        .apply(request)
                        .await?,
                    true,
                )
            }
        };
        let outcome = PrivacyOwnerActionOutcome::record(
            &attempt,
            action_result.status,
            action_result.safe_failure_code,
            attempt.planned_at_unix_nanos(),
        )?;
        let outcome_replayed = !self
            .persistence
            .record_outcome(&invocation, &attempt, &outcome)
            .await?;
        let checkpoint = self.persistence.advance_checkpoint(&invocation).await?;
        Ok(OwnerExecutionResult {
            attempt: Some(attempt),
            outcome: Some(outcome),
            attempt_replayed,
            outcome_replayed,
            owner_invoked,
            next_sequence: checkpoint.next_sequence,
            total_items: checkpoint.total_items,
            complete: checkpoint.complete,
        })
    }
}

fn owner_request(
    invocation: &OwnerExecutionInvocation,
    attempt: &PrivacyOwnerActionAttempt,
) -> OwnerActionRequest {
    OwnerActionRequest {
        tenant_id: invocation.tenant_id.clone(),
        privacy_case_id: invocation.privacy_case_id.clone(),
        action_plan_id: invocation.action_plan_id.clone(),
        retention_decision_id: invocation.retention_decision_id.clone(),
        attempt_id: attempt.attempt_id().clone(),
        owner_module_id: attempt.owner_module_id().clone(),
        owner_capability_id: attempt.owner_capability_id().to_owned(),
        owner_capability_version: attempt.owner_capability_version().to_owned(),
        target_idempotency_key: attempt.target_idempotency_key().as_str().to_owned(),
        resource_type: attempt.resource_type().to_owned(),
        resource_id: attempt.resource_id().clone(),
        resource_version: attempt.resource_version(),
        action_code: attempt.action_code().to_owned(),
        actor_id: invocation.actor_id.clone(),
        correlation_id: invocation.correlation_id.clone(),
        trace_id: invocation.trace_id.clone(),
    }
}

fn validate_invocation(invocation: &OwnerExecutionInvocation) -> Result<(), SdkError> {
    if !invocation.trusted_internal {
        return Err(execution_not_trusted());
    }
    if invocation.initiating_capability_id.as_str() != OWNER_ACTION_DISPATCH_CAPABILITY
        || invocation.initiating_capability_version.as_str() != OWNER_EXECUTION_CAPABILITY_VERSION
    {
        return Err(execution_configuration_invalid(
            "owner execution invocation uses an unexpected internal coordinate",
        ));
    }
    if invocation.request_started_at_unix_nanos <= 0
        || invocation.planned_at_unix_nanos < invocation.request_started_at_unix_nanos
    {
        return Err(execution_invalid_argument(
            "execution timestamps are missing or non-monotonic",
        ));
    }
    Ok(())
}

fn execution_disabled() -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_OWNER_EXECUTION_DISABLED",
        crm_module_sdk::ErrorCategory::Conflict,
        false,
        "Customer Privacy is disabled for the tenant.",
    )
}

fn execution_not_trusted() -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_OWNER_EXECUTION_NOT_TRUSTED",
        crm_module_sdk::ErrorCategory::Authorization,
        false,
        "The owner execution request is not trusted.",
    )
}

fn execution_invalid_argument(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_OWNER_EXECUTION_INVALID_ARGUMENT",
        crm_module_sdk::ErrorCategory::InvalidArgument,
        false,
        "The owner execution request is invalid.",
    )
    .with_internal_reference(reference)
}

fn execution_configuration_invalid(reference: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_OWNER_EXECUTION_CONFIGURATION_INVALID",
        crm_module_sdk::ErrorCategory::Internal,
        false,
        "Customer Privacy owner execution is not configured correctly.",
    )
    .with_internal_reference(reference.to_string())
}
