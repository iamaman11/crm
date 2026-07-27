use crm_application_composition::ModuleActivationPort;
use crm_customer_privacy::{
    ActionPlanningPolicy, DiscoveryScopeSnapshot, MODULE_ID, PrivacyActionPlan, PrivacyCase,
    PrivacyCaseStatus,
};
use crm_module_sdk::{
    ActorId, CorrelationId, ErrorCategory, PortFuture, RecordId, RequestId, SdkError, TenantId,
    TraceId,
};
use std::sync::Arc;

pub const PLANNING_PHASE: u16 = 270;

#[derive(Debug, Clone)]
pub struct PlanningInvocation {
    pub tenant_id: TenantId,
    pub privacy_case_id: RecordId,
    pub actor_id: ActorId,
    pub request_id: RequestId,
    pub correlation_id: CorrelationId,
    pub trace_id: TraceId,
    pub request_started_at_unix_nanos: i64,
    pub proposed_planned_at_unix_nanos: i64,
    pub policy: ActionPlanningPolicy,
    pub trusted_internal: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanningSource {
    pub privacy_case: PrivacyCase,
    pub scope_snapshot: DiscoveryScopeSnapshot,
    pub existing_plan: Option<PrivacyActionPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanningCommit {
    pub privacy_case: PrivacyCase,
    pub action_plan: PrivacyActionPlan,
    pub replayed: bool,
}

pub trait PlanningPersistencePort: Send + Sync {
    fn load_source<'a>(
        &'a self,
        invocation: &'a PlanningInvocation,
    ) -> PortFuture<'a, Result<PlanningSource, SdkError>>;

    fn finalize_plan<'a>(
        &'a self,
        invocation: &'a PlanningInvocation,
        plan: &'a PrivacyActionPlan,
    ) -> PortFuture<'a, Result<PlanningCommit, SdkError>>;
}

#[derive(Clone)]
pub struct PrivacyPlanningService {
    activation: Arc<dyn ModuleActivationPort>,
    persistence: Arc<dyn PlanningPersistencePort>,
}

impl std::fmt::Debug for PrivacyPlanningService {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PrivacyPlanningService")
            .field("activation", &"dyn ModuleActivationPort")
            .field("persistence", &"dyn PlanningPersistencePort")
            .finish()
    }
}

impl PrivacyPlanningService {
    pub fn new(
        activation: Arc<dyn ModuleActivationPort>,
        persistence: Arc<dyn PlanningPersistencePort>,
    ) -> Self {
        Self {
            activation,
            persistence,
        }
    }

    pub async fn build(&self, invocation: PlanningInvocation) -> Result<PlanningCommit, SdkError> {
        validate_invocation(&invocation)?;
        let module_id = crm_module_sdk::ModuleId::try_new(MODULE_ID)
            .map_err(|error| configuration_error(error.to_string()))?;
        if !self
            .activation
            .is_active(&invocation.tenant_id, &module_id)
            .await?
        {
            return Err(planning_error(
                "CUSTOMER_PRIVACY_PLANNING_DISABLED",
                ErrorCategory::Conflict,
                true,
                "Customer Privacy is disabled for the tenant",
            ));
        }

        let source = self.persistence.load_source(&invocation).await?;
        validate_source(&invocation, &source)?;

        if let Some(existing) = source.existing_plan.as_ref() {
            validate_existing_plan(&invocation, &source.privacy_case, existing)?;
            return Ok(PlanningCommit {
                privacy_case: source.privacy_case,
                action_plan: existing.clone(),
                replayed: true,
            });
        }

        if source.privacy_case.status() != PrivacyCaseStatus::Scoped {
            return Err(planning_error(
                "CUSTOMER_PRIVACY_PLANNING_CASE_STATE_INVALID",
                ErrorCategory::Conflict,
                false,
                format!(
                    "privacy case is not scoped: {}",
                    source.privacy_case.status().label()
                ),
            ));
        }

        let plan = PrivacyActionPlan::build(
            &source.scope_snapshot,
            source.privacy_case.version(),
            source.privacy_case.kind(),
            invocation.policy.clone(),
            invocation.proposed_planned_at_unix_nanos,
        )
        .map_err(domain_error)?;
        self.persistence.finalize_plan(&invocation, &plan).await
    }
}

fn validate_invocation(invocation: &PlanningInvocation) -> Result<(), SdkError> {
    if !invocation.trusted_internal {
        return Err(planning_error(
            "CUSTOMER_PRIVACY_PLANNING_TRUST_REQUIRED",
            ErrorCategory::Authorization,
            false,
            "planning invocation is not trusted internal",
        ));
    }
    if invocation.request_started_at_unix_nanos <= 0
        || invocation.proposed_planned_at_unix_nanos < invocation.request_started_at_unix_nanos
    {
        return Err(planning_error(
            "CUSTOMER_PRIVACY_PLANNING_TIME_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "planning timestamps are invalid",
        ));
    }
    Ok(())
}

fn validate_source(
    invocation: &PlanningInvocation,
    source: &PlanningSource,
) -> Result<(), SdkError> {
    let privacy_case = &source.privacy_case;
    let snapshot = &source.scope_snapshot;
    let binding = privacy_case.subject_binding().ok_or_else(|| {
        planning_error(
            "CUSTOMER_PRIVACY_PLANNING_SUBJECT_UNVERIFIED",
            ErrorCategory::Conflict,
            false,
            "privacy case has no verified subject binding",
        )
    })?;
    let snapshot_lineage = snapshot.lineage();
    if privacy_case.case_id() != &invocation.privacy_case_id
        || privacy_case.tenant_id() != &invocation.tenant_id
        || snapshot_lineage.privacy_case_id() != privacy_case.case_id()
        || snapshot_lineage.tenant_id() != privacy_case.tenant_id()
        || snapshot_lineage.canonical_party_id() != &binding.canonical_party_id
        || snapshot_lineage.identity_resolution_generation()
            != binding.identity_resolution_generation
        || privacy_case.scope_snapshot_id() != Some(snapshot.snapshot_id())
        || privacy_case.policy_version() != invocation.policy.policy_version()
    {
        return Err(planning_error(
            "CUSTOMER_PRIVACY_PLANNING_SOURCE_MISMATCH",
            ErrorCategory::Conflict,
            false,
            "privacy case, policy and immutable snapshot lineage do not match",
        ));
    }
    if !matches!(
        privacy_case.status(),
        PrivacyCaseStatus::Scoped
            | PrivacyCaseStatus::Planned
            | PrivacyCaseStatus::AwaitingApproval
    ) {
        return Err(planning_error(
            "CUSTOMER_PRIVACY_PLANNING_CASE_STATE_INVALID",
            ErrorCategory::Conflict,
            false,
            format!(
                "privacy case state cannot enter planning: {}",
                privacy_case.status().label()
            ),
        ));
    }
    if privacy_case.action_plan_id().is_some() != source.existing_plan.is_some() {
        return Err(planning_error(
            "CUSTOMER_PRIVACY_PLANNING_EVIDENCE_INVALID",
            ErrorCategory::Internal,
            false,
            "privacy case plan reference and durable action plan disagree",
        ));
    }
    Ok(())
}

fn validate_existing_plan(
    invocation: &PlanningInvocation,
    privacy_case: &PrivacyCase,
    plan: &PrivacyActionPlan,
) -> Result<(), SdkError> {
    let lineage = plan.lineage();
    let expected_resulting_version =
        lineage
            .source_case_version()
            .checked_add(1)
            .ok_or_else(|| {
                planning_error(
                    "CUSTOMER_PRIVACY_PLANNING_EVIDENCE_INVALID",
                    ErrorCategory::Internal,
                    false,
                    "planning case version overflowed",
                )
            })?;
    if privacy_case.action_plan_id() != Some(plan.plan_id())
        || !matches!(
            privacy_case.status(),
            PrivacyCaseStatus::Planned | PrivacyCaseStatus::AwaitingApproval
        )
        || privacy_case.version() != expected_resulting_version
        || lineage.privacy_case_id() != privacy_case.case_id()
        || lineage.tenant_id() != privacy_case.tenant_id()
        || lineage.case_kind() != privacy_case.kind()
        || lineage.policy_version() != invocation.policy.policy_version()
        || lineage.jurisdiction_code() != invocation.policy.jurisdiction_code()
        || lineage.approval_required() != invocation.policy.approval_required()
        || lineage.crypto_shred_supported() != invocation.policy.crypto_shred_supported()
        || plan.planned_at_unix_nanos() != invocation.proposed_planned_at_unix_nanos
    {
        return Err(planning_error(
            "CUSTOMER_PRIVACY_PLANNING_REPLAY_CONFLICT",
            ErrorCategory::Conflict,
            false,
            "planning replay differs from the immutable accepted plan",
        ));
    }
    Ok(())
}

fn domain_error(error: impl std::fmt::Display) -> SdkError {
    planning_error(
        "CUSTOMER_PRIVACY_PLANNING_DOMAIN_INVALID",
        ErrorCategory::Conflict,
        false,
        error.to_string(),
    )
}

fn configuration_error(reference: impl Into<String>) -> SdkError {
    planning_error(
        "CUSTOMER_PRIVACY_PLANNING_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        reference,
    )
}

fn planning_error(
    code: &'static str,
    category: ErrorCategory,
    retryable: bool,
    reference: impl Into<String>,
) -> SdkError {
    SdkError::new(
        code,
        category,
        retryable,
        "Customer Privacy planning failed closed.",
    )
    .with_internal_reference(reference)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_customer_privacy::{PrivacyCaseKind, SchemaVersion};

    #[test]
    fn planning_phase_and_trust_boundary_are_exact() {
        assert_eq!(PLANNING_PHASE, 270);
        let invocation = PlanningInvocation {
            tenant_id: TenantId::try_new("tenant-a").unwrap(),
            privacy_case_id: RecordId::try_new("case-a").unwrap(),
            actor_id: ActorId::try_new("privacy-worker").unwrap(),
            request_id: RequestId::try_new("request-a").unwrap(),
            correlation_id: CorrelationId::try_new("correlation-a").unwrap(),
            trace_id: TraceId::try_new("trace-a").unwrap(),
            request_started_at_unix_nanos: 10,
            proposed_planned_at_unix_nanos: 20,
            policy: ActionPlanningPolicy::new(
                SchemaVersion::try_new("privacy-policy/1").unwrap(),
                "EU",
                false,
                false,
            )
            .unwrap(),
            trusted_internal: false,
        };
        assert_eq!(
            validate_invocation(&invocation).unwrap_err().code,
            "CUSTOMER_PRIVACY_PLANNING_TRUST_REQUIRED"
        );
        assert_eq!(
            PrivacyCaseKind::Erasure as u8,
            PrivacyCaseKind::Erasure as u8
        );
    }
}
