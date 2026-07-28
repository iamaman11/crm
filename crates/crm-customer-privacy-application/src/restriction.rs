use crm_application_composition::ModuleActivationPort;
use crm_customer_privacy::{
    MODULE_ID, ProcessingRestriction, ProcessingRestrictionScope, RestrictedChannel,
};
use crm_module_sdk::{
    ActorId, CorrelationId, ErrorCategory, PortFuture, RecordId, RequestId, SdkError, TenantId,
    TraceId,
};
use std::sync::Arc;

pub const RESTRICTION_PLACEMENT_PHASE: u16 = 280;

#[derive(Debug, Clone)]
pub struct RestrictionPlacementInvocation {
    pub tenant_id: TenantId,
    pub privacy_case_id: RecordId,
    pub restriction_id: RecordId,
    pub actor_id: ActorId,
    pub request_id: RequestId,
    pub correlation_id: CorrelationId,
    pub trace_id: TraceId,
    pub idempotency_key: String,
    pub request_started_at_unix_nanos: i64,
    pub proposed_placed_at_unix_nanos: i64,
    pub scopes: Vec<ProcessingRestrictionScope>,
    pub channels: Vec<RestrictedChannel>,
    pub starts_at_unix_nanos: i64,
    pub expires_at_unix_nanos: Option<i64>,
    pub reason: String,
    pub legal_basis: String,
    pub policy_version: String,
    pub trusted_internal: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestrictionPlacementCommit {
    pub restriction: ProcessingRestriction,
    pub replayed: bool,
}

pub trait RestrictionPlacementPersistencePort: Send + Sync {
    fn place<'a>(
        &'a self,
        invocation: &'a RestrictionPlacementInvocation,
    ) -> PortFuture<'a, Result<RestrictionPlacementCommit, SdkError>>;
}

#[derive(Clone)]
pub struct ProcessingRestrictionPlacementService {
    activation: Arc<dyn ModuleActivationPort>,
    persistence: Arc<dyn RestrictionPlacementPersistencePort>,
}

impl std::fmt::Debug for ProcessingRestrictionPlacementService {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProcessingRestrictionPlacementService")
            .field("activation", &"dyn ModuleActivationPort")
            .field("persistence", &"dyn RestrictionPlacementPersistencePort")
            .finish()
    }
}

impl ProcessingRestrictionPlacementService {
    pub fn new(
        activation: Arc<dyn ModuleActivationPort>,
        persistence: Arc<dyn RestrictionPlacementPersistencePort>,
    ) -> Self {
        Self {
            activation,
            persistence,
        }
    }

    pub async fn place(
        &self,
        invocation: RestrictionPlacementInvocation,
    ) -> Result<RestrictionPlacementCommit, SdkError> {
        validate_invocation(&invocation)?;
        let module_id = crm_module_sdk::ModuleId::try_new(MODULE_ID)
            .map_err(|error| configuration_error(error.to_string()))?;
        if !self
            .activation
            .is_active(&invocation.tenant_id, &module_id)
            .await?
        {
            return Err(restriction_error(
                "CUSTOMER_PRIVACY_RESTRICTION_PLACEMENT_DISABLED",
                ErrorCategory::Conflict,
                true,
                "Customer Privacy is disabled for the tenant",
            ));
        }
        self.persistence.place(&invocation).await
    }
}

fn validate_invocation(invocation: &RestrictionPlacementInvocation) -> Result<(), SdkError> {
    if !invocation.trusted_internal {
        return Err(restriction_error(
            "CUSTOMER_PRIVACY_RESTRICTION_PLACEMENT_TRUST_REQUIRED",
            ErrorCategory::Authorization,
            false,
            "restriction placement invocation is not trusted internal",
        ));
    }
    if invocation.idempotency_key.trim().is_empty() {
        return Err(restriction_error(
            "CUSTOMER_PRIVACY_RESTRICTION_PLACEMENT_IDEMPOTENCY_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "restriction placement idempotency key is required",
        ));
    }
    if invocation.request_started_at_unix_nanos <= 0
        || invocation.proposed_placed_at_unix_nanos < invocation.request_started_at_unix_nanos
        || invocation.proposed_placed_at_unix_nanos % 1_000 != 0
    {
        return Err(restriction_error(
            "CUSTOMER_PRIVACY_RESTRICTION_PLACEMENT_TIME_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "restriction placement time must be positive, monotonic and microsecond aligned",
        ));
    }
    if invocation.scopes.is_empty() {
        return Err(restriction_error(
            "CUSTOMER_PRIVACY_RESTRICTION_PLACEMENT_SCOPE_REQUIRED",
            ErrorCategory::InvalidArgument,
            false,
            "at least one processing restriction scope is required",
        ));
    }
    Ok(())
}

fn restriction_error(
    code: &'static str,
    category: ErrorCategory,
    retryable: bool,
    message: impl Into<String>,
) -> SdkError {
    SdkError::new(code, category, retryable, message)
}

fn configuration_error(reference: impl Into<String>) -> SdkError {
    restriction_error(
        "CUSTOMER_PRIVACY_RESTRICTION_PLACEMENT_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "restriction placement configuration is invalid",
    )
    .with_internal_reference(reference.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_follows_planning() {
        assert_eq!(RESTRICTION_PLACEMENT_PHASE, 280);
    }
}
