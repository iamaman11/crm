use crm_application_composition::ModuleActivationPort;
use crm_customer_privacy::{MODULE_ID, PrivacyRetentionDecisionSet};
use crm_module_sdk::{
    ActorId, CapabilityId, CapabilityVersion, CorrelationId, ErrorCategory, PortFuture, RecordId,
    RequestId, SdkError, TenantId, TraceId,
};
use std::sync::Arc;

pub const RETENTION_EVALUATION_PHASE: u16 = 260;
pub const RETENTION_APPROVAL_TRIGGER_CAPABILITY: &str = "customer_privacy.case.approve";
pub const RETENTION_LEGAL_HOLD_TRIGGER_CAPABILITY: &str = "customer_privacy.legal_hold.place";
pub const RETENTION_TRIGGER_CAPABILITY_VERSION: &str = "1.0.0";

#[derive(Debug, Clone)]
pub struct RetentionEvaluationInvocation {
    pub tenant_id: TenantId,
    pub privacy_case_id: RecordId,
    pub action_plan_id: RecordId,
    pub actor_id: ActorId,
    pub request_id: RequestId,
    pub correlation_id: CorrelationId,
    pub trace_id: TraceId,
    pub initiating_capability_id: CapabilityId,
    pub initiating_capability_version: CapabilityVersion,
    pub request_started_at_unix_nanos: i64,
    pub evaluated_at_unix_nanos: i64,
    pub trusted_internal: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetentionEvaluationCommit {
    pub decision: PrivacyRetentionDecisionSet,
    pub replayed: bool,
}

pub trait RetentionEvaluationPersistencePort: Send + Sync {
    fn evaluate_and_persist<'a>(
        &'a self,
        invocation: &'a RetentionEvaluationInvocation,
    ) -> PortFuture<'a, Result<RetentionEvaluationCommit, SdkError>>;
}

#[derive(Clone)]
pub struct PrivacyRetentionEvaluationService {
    activation: Arc<dyn ModuleActivationPort>,
    persistence: Arc<dyn RetentionEvaluationPersistencePort>,
}

impl std::fmt::Debug for PrivacyRetentionEvaluationService {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PrivacyRetentionEvaluationService")
            .field("activation", &"dyn ModuleActivationPort")
            .field("persistence", &"dyn RetentionEvaluationPersistencePort")
            .finish()
    }
}

impl PrivacyRetentionEvaluationService {
    pub fn new(
        activation: Arc<dyn ModuleActivationPort>,
        persistence: Arc<dyn RetentionEvaluationPersistencePort>,
    ) -> Self {
        Self {
            activation,
            persistence,
        }
    }

    pub async fn evaluate(
        &self,
        invocation: RetentionEvaluationInvocation,
    ) -> Result<RetentionEvaluationCommit, SdkError> {
        validate_invocation(&invocation)?;
        let module_id = crm_module_sdk::ModuleId::try_new(MODULE_ID).map_err(|error| {
            retention_error(
                "CONFIGURATION_INVALID",
                ErrorCategory::Internal,
                false,
                error.to_string(),
            )
        })?;
        if !self
            .activation
            .is_active(&invocation.tenant_id, &module_id)
            .await?
        {
            return Err(retention_error(
                "DISABLED",
                ErrorCategory::Conflict,
                true,
                "Customer Privacy is disabled for the tenant",
            ));
        }

        let commit = self.persistence.evaluate_and_persist(&invocation).await?;
        validate_commit(&invocation, &commit)?;
        Ok(commit)
    }
}

fn validate_invocation(invocation: &RetentionEvaluationInvocation) -> Result<(), SdkError> {
    if !invocation.trusted_internal {
        return Err(retention_error(
            "TRUST_REQUIRED",
            ErrorCategory::Authorization,
            false,
            "retention evaluation invocation is not trusted internal",
        ));
    }
    if invocation.initiating_capability_version.as_str() != RETENTION_TRIGGER_CAPABILITY_VERSION
        || !matches!(
            invocation.initiating_capability_id.as_str(),
            RETENTION_APPROVAL_TRIGGER_CAPABILITY | RETENTION_LEGAL_HOLD_TRIGGER_CAPABILITY
        )
    {
        return Err(retention_error(
            "TRIGGER_INVALID",
            ErrorCategory::Authorization,
            false,
            "retention evaluation has no registered initiating Customer Privacy capability",
        ));
    }
    if invocation.request_started_at_unix_nanos <= 0
        || invocation.evaluated_at_unix_nanos < invocation.request_started_at_unix_nanos
        || invocation.evaluated_at_unix_nanos % 1_000 != 0
    {
        return Err(retention_error(
            "TIME_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "retention evaluation time must be positive, monotonic and exactly microsecond aligned",
        ));
    }
    Ok(())
}

fn validate_commit(
    invocation: &RetentionEvaluationInvocation,
    commit: &RetentionEvaluationCommit,
) -> Result<(), SdkError> {
    let decision = &commit.decision;
    if decision.tenant_id() != &invocation.tenant_id
        || decision.privacy_case_id() != &invocation.privacy_case_id
        || decision.action_plan_id() != &invocation.action_plan_id
        || decision.evaluated_at_unix_nanos() != invocation.evaluated_at_unix_nanos
    {
        return Err(retention_error(
            "COMMIT_MISMATCH",
            ErrorCategory::Internal,
            false,
            "retention-decision commit differs from the requested immutable lineage",
        ));
    }
    Ok(())
}

fn retention_error(
    suffix: &'static str,
    category: ErrorCategory,
    retryable: bool,
    reference: impl Into<String>,
) -> SdkError {
    let code = match suffix {
        "CONFIGURATION_INVALID" => "CUSTOMER_PRIVACY_RETENTION_CONFIGURATION_INVALID",
        "DISABLED" => "CUSTOMER_PRIVACY_RETENTION_DISABLED",
        "TRUST_REQUIRED" => "CUSTOMER_PRIVACY_RETENTION_TRUST_REQUIRED",
        "TRIGGER_INVALID" => "CUSTOMER_PRIVACY_RETENTION_TRIGGER_INVALID",
        "TIME_INVALID" => "CUSTOMER_PRIVACY_RETENTION_TIME_INVALID",
        "COMMIT_MISMATCH" => "CUSTOMER_PRIVACY_RETENTION_COMMIT_MISMATCH",
        _ => "CUSTOMER_PRIVACY_RETENTION_FAILED",
    };
    SdkError::new(
        code,
        category,
        retryable,
        "Customer Privacy retention evaluation failed closed.",
    )
    .with_internal_reference(reference)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn invocation(evaluated_at: i64, trusted_internal: bool) -> RetentionEvaluationInvocation {
        RetentionEvaluationInvocation {
            tenant_id: TenantId::try_new("tenant-a").unwrap(),
            privacy_case_id: RecordId::try_new("case-a").unwrap(),
            action_plan_id: RecordId::try_new("plan-a").unwrap(),
            actor_id: ActorId::try_new("privacy-worker").unwrap(),
            request_id: RequestId::try_new("request-a").unwrap(),
            correlation_id: CorrelationId::try_new("correlation-a").unwrap(),
            trace_id: TraceId::try_new("trace-a").unwrap(),
            initiating_capability_id: CapabilityId::try_new(RETENTION_APPROVAL_TRIGGER_CAPABILITY)
                .unwrap(),
            initiating_capability_version: CapabilityVersion::try_new(
                RETENTION_TRIGGER_CAPABILITY_VERSION,
            )
            .unwrap(),
            request_started_at_unix_nanos: 1_000,
            evaluated_at_unix_nanos: evaluated_at,
            trusted_internal,
        }
    }

    #[test]
    fn retention_phase_coordinate_and_trust_boundary_are_exact() {
        assert_eq!(RETENTION_EVALUATION_PHASE, 260);
        assert_eq!(
            crm_customer_privacy::RETENTION_EVALUATE_COORDINATE,
            "customer_privacy.retention.evaluate@1.0.0"
        );
        assert_eq!(
            validate_invocation(&invocation(2_000, false))
                .unwrap_err()
                .code,
            "CUSTOMER_PRIVACY_RETENTION_TRUST_REQUIRED"
        );
    }

    #[test]
    fn only_registered_step_six_triggers_are_accepted() {
        let approval = invocation(2_000, true);
        assert!(validate_invocation(&approval).is_ok());

        let mut legal_hold = approval.clone();
        legal_hold.initiating_capability_id =
            CapabilityId::try_new(RETENTION_LEGAL_HOLD_TRIGGER_CAPABILITY).unwrap();
        assert!(validate_invocation(&legal_hold).is_ok());

        let mut unregistered = approval;
        unregistered.initiating_capability_id =
            CapabilityId::try_new("customer_privacy.retention.evaluate").unwrap();
        assert_eq!(
            validate_invocation(&unregistered).unwrap_err().code,
            "CUSTOMER_PRIVACY_RETENTION_TRIGGER_INVALID"
        );
    }

    #[test]
    fn evaluation_time_must_round_trip_through_postgresql_exactly() {
        assert!(validate_invocation(&invocation(2_000, true)).is_ok());
        assert_eq!(
            validate_invocation(&invocation(2_001, true))
                .unwrap_err()
                .code,
            "CUSTOMER_PRIVACY_RETENTION_TIME_INVALID"
        );
    }
}
