use crate::{
    RETENTION_APPROVAL_TRIGGER_CAPABILITY, RETENTION_LEGAL_HOLD_TRIGGER_CAPABILITY,
    RETENTION_TRIGGER_CAPABILITY_VERSION,
};
use crm_application_composition::{ModuleActivationPort, TenantBackgroundWorker};
use crm_customer_privacy::{
    MODULE_ID, PrivacyOwnerActionAttempt, PrivacyOwnerActionOutcome, PrivacyOwnerOutcomeStatus,
};
use crm_module_sdk::{
    ActorId, CapabilityId, CapabilityVersion, CorrelationId, ModuleId, PortFuture, RecordId,
    RequestId, SdkError, TenantId, TraceId,
};
use std::collections::BTreeSet;
use std::sync::Arc;

pub const OWNER_ACTION_DISPATCH_CAPABILITY: &str = "customer_privacy.owner_action.dispatch";
pub const OWNER_OUTCOME_RECORD_CAPABILITY: &str = "customer_privacy.owner_outcome.record";
pub const OWNER_EXECUTION_CAPABILITY_VERSION: &str = "1.0.0";

const OWNER_EXECUTION_WORK_LIMIT: u32 = 64;
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
        attempt: Box<PrivacyOwnerActionAttempt>,
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
    fn load_ready<'a>(
        &'a self,
        _tenant_id: &'a TenantId,
        _now_unix_nanos: i64,
        _maximum_items: u32,
    ) -> PortFuture<'a, Result<Vec<OwnerExecutionInvocation>, SdkError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

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
            } => (*attempt, attempt_replayed),
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

impl TenantBackgroundWorker for PrivacyOwnerExecutionService {
    fn run_tenant_cycle<'a>(
        &'a self,
        tenant_id: TenantId,
        now_unix_nanos: i64,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            if now_unix_nanos <= 0 {
                return Err(execution_invalid_argument(
                    "worker cycle time must be after the Unix epoch",
                ));
            }
            let module_id =
                ModuleId::try_new(MODULE_ID).map_err(execution_configuration_invalid)?;
            if !self.activation.is_active(&tenant_id, &module_id).await? {
                return Ok(());
            }
            let work = self
                .persistence
                .load_ready(&tenant_id, now_unix_nanos, OWNER_EXECUTION_WORK_LIMIT)
                .await?;
            validate_work_batch(
                &tenant_id,
                now_unix_nanos,
                OWNER_EXECUTION_WORK_LIMIT,
                &work,
            )?;
            for invocation in work {
                self.execute_next(invocation).await?;
            }
            Ok(())
        })
    }
}

fn validate_work_batch(
    tenant_id: &TenantId,
    now_unix_nanos: i64,
    maximum_items: u32,
    work: &[OwnerExecutionInvocation],
) -> Result<(), SdkError> {
    if work.len() > maximum_items as usize {
        return Err(worker_batch_invalid(
            "work source exceeded the requested bounded item limit",
        ));
    }
    let mut identities = BTreeSet::new();
    for invocation in work {
        if &invocation.tenant_id != tenant_id {
            return Err(worker_batch_invalid(
                "work source returned an invocation for another tenant",
            ));
        }
        if !invocation.trusted_internal {
            return Err(worker_batch_invalid(
                "work source returned an invocation without trusted-internal provenance",
            ));
        }
        if invocation.request_started_at_unix_nanos <= 0
            || invocation.planned_at_unix_nanos < invocation.request_started_at_unix_nanos
            || invocation.planned_at_unix_nanos > now_unix_nanos
        {
            return Err(worker_batch_invalid(
                "work source returned missing, non-monotonic or future execution time",
            ));
        }
        let identity = (
            invocation.privacy_case_id.as_str().to_owned(),
            invocation.action_plan_id.as_str().to_owned(),
            invocation.retention_decision_id.as_str().to_owned(),
        );
        if !identities.insert(identity) {
            return Err(worker_batch_invalid(
                "work source returned duplicate execution identity in one cycle",
            ));
        }
    }
    Ok(())
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
    if invocation.initiating_capability_version.as_str() != RETENTION_TRIGGER_CAPABILITY_VERSION
        || !matches!(
            invocation.initiating_capability_id.as_str(),
            RETENTION_APPROVAL_TRIGGER_CAPABILITY | RETENTION_LEGAL_HOLD_TRIGGER_CAPABILITY
        )
    {
        return Err(execution_configuration_invalid(
            "owner execution has no registered initiating Customer Privacy capability",
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

fn worker_batch_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_OWNER_EXECUTION_WORK_BATCH_INVALID",
        crm_module_sdk::ErrorCategory::Conflict,
        false,
        "The Customer Privacy owner-execution work batch is invalid.",
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

#[cfg(test)]
mod execution_tests {
    use super::*;
    use crm_customer_privacy::{
        ActionPlanningPolicy, ContributionCompletenessProof, DiscoveryOwnerScopeContribution,
        DiscoveryScopeSnapshot, EvidenceClass, OwnerScopeContract, OwnerScopeContribution,
        OwnerScopeRegistry, PrivacyActionPlan, PrivacyCaseKind, PrivacyRetentionDecisionSet,
        ScopeDiscoveryLineage, ScopeResource,
    };
    use crm_module_sdk::{DataClass, RetentionPolicyId, SchemaVersion};
    use std::future::Future;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::task::{Context, Poll, Waker};

    fn block_on<F: Future>(future: F) -> F::Output {
        let mut context = Context::from_waker(Waker::noop());
        let mut future = Box::pin(future);
        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(output) => return output,
                Poll::Pending => std::thread::yield_now(),
            }
        }
    }

    #[derive(Debug)]
    struct Activation {
        active: bool,
    }

    impl ModuleActivationPort for Activation {
        fn is_active<'a>(
            &'a self,
            _tenant_id: &'a TenantId,
            _module_id: &'a ModuleId,
        ) -> PortFuture<'a, Result<bool, SdkError>> {
            Box::pin(async move { Ok(self.active) })
        }
    }

    #[derive(Debug)]
    struct RecordingOwner {
        result: OwnerActionResult,
        order: Arc<Mutex<Vec<&'static str>>>,
        requests: Mutex<Vec<OwnerActionRequest>>,
    }

    impl OwnerPrivacyActionPort for RecordingOwner {
        fn apply<'a>(
            &'a self,
            request: OwnerActionRequest,
        ) -> PortFuture<'a, Result<OwnerActionResult, SdkError>> {
            Box::pin(async move {
                self.order.lock().unwrap().push("owner");
                self.requests.lock().unwrap().push(request);
                Ok(self.result.clone())
            })
        }
    }

    #[derive(Debug)]
    struct ScriptedPersistence {
        preparation: ExecutionPreparation,
        record_inserted: bool,
        checkpoint: CheckpointAdvance,
        order: Arc<Mutex<Vec<&'static str>>>,
        recorded: Mutex<Vec<(PrivacyOwnerActionAttempt, PrivacyOwnerActionOutcome)>>,
    }

    impl OwnerExecutionPersistencePort for ScriptedPersistence {
        fn prepare_next<'a>(
            &'a self,
            _invocation: &'a OwnerExecutionInvocation,
        ) -> PortFuture<'a, Result<ExecutionPreparation, SdkError>> {
            Box::pin(async move {
                self.order.lock().unwrap().push("prepare");
                Ok(self.preparation.clone())
            })
        }

        fn record_outcome<'a>(
            &'a self,
            _invocation: &'a OwnerExecutionInvocation,
            attempt: &'a PrivacyOwnerActionAttempt,
            outcome: &'a PrivacyOwnerActionOutcome,
        ) -> PortFuture<'a, Result<bool, SdkError>> {
            Box::pin(async move {
                self.order.lock().unwrap().push("record");
                self.recorded
                    .lock()
                    .unwrap()
                    .push((attempt.clone(), outcome.clone()));
                Ok(self.record_inserted)
            })
        }

        fn advance_checkpoint<'a>(
            &'a self,
            _invocation: &'a OwnerExecutionInvocation,
        ) -> PortFuture<'a, Result<CheckpointAdvance, SdkError>> {
            Box::pin(async move {
                self.order.lock().unwrap().push("advance");
                Ok(self.checkpoint.clone())
            })
        }
    }

    #[derive(Debug)]
    struct WorkerPersistence {
        work: Vec<OwnerExecutionInvocation>,
        load_calls: AtomicUsize,
        prepare_calls: AtomicUsize,
    }

    impl OwnerExecutionPersistencePort for WorkerPersistence {
        fn load_ready<'a>(
            &'a self,
            _tenant_id: &'a TenantId,
            _now_unix_nanos: i64,
            _maximum_items: u32,
        ) -> PortFuture<'a, Result<Vec<OwnerExecutionInvocation>, SdkError>> {
            Box::pin(async move {
                self.load_calls.fetch_add(1, Ordering::SeqCst);
                Ok(self.work.clone())
            })
        }

        fn prepare_next<'a>(
            &'a self,
            _invocation: &'a OwnerExecutionInvocation,
        ) -> PortFuture<'a, Result<ExecutionPreparation, SdkError>> {
            Box::pin(async move {
                self.prepare_calls.fetch_add(1, Ordering::SeqCst);
                Ok(ExecutionPreparation::Complete {
                    total_items: 1,
                    durable_outcomes: 1,
                })
            })
        }

        fn record_outcome<'a>(
            &'a self,
            _invocation: &'a OwnerExecutionInvocation,
            _attempt: &'a PrivacyOwnerActionAttempt,
            _outcome: &'a PrivacyOwnerActionOutcome,
        ) -> PortFuture<'a, Result<bool, SdkError>> {
            Box::pin(async { panic!("completed worker work must not record an outcome") })
        }

        fn advance_checkpoint<'a>(
            &'a self,
            _invocation: &'a OwnerExecutionInvocation,
        ) -> PortFuture<'a, Result<CheckpointAdvance, SdkError>> {
            Box::pin(async { panic!("completed worker work must not advance a checkpoint") })
        }
    }

    fn exact_endpoints(owner: Arc<RecordingOwner>) -> OwnerActionEndpoints {
        OwnerActionEndpoints::exact_canonical(EXPECTED_OWNER_MODULES.iter().map(|module_id| {
            OwnerActionEndpoint {
                owner_module_id: ModuleId::try_new(*module_id).unwrap(),
                executor: owner.clone(),
            }
        }))
        .unwrap()
    }

    fn attempt(evidence_class: EvidenceClass, generation: u32) -> PrivacyOwnerActionAttempt {
        let tenant_id = TenantId::try_new("tenant-a").unwrap();
        let party_id = RecordId::try_new("party-a").unwrap();
        let case_id = RecordId::try_new("privacy-case-a").unwrap();
        let contract = OwnerScopeContract::new(
            ModuleId::try_new("crm.parties").unwrap(),
            CapabilityId::try_new("parties.privacy.scope.contribute").unwrap(),
            CapabilityVersion::try_new("1.0.0").unwrap(),
        );
        let registry = OwnerScopeRegistry::new(
            SchemaVersion::try_new("registry/1").unwrap(),
            [contract.clone()],
        )
        .unwrap();
        let lineage = ScopeDiscoveryLineage::new(
            case_id.clone(),
            tenant_id.clone(),
            party_id.clone(),
            1,
            registry.registry_version().clone(),
            *registry.digest(),
            "ERASURE_DISCOVERY",
            1,
        )
        .unwrap();
        let resource = ScopeResource::new(
            "party.profile",
            party_id,
            7,
            DataClass::Personal,
            evidence_class,
            RetentionPolicyId::try_new("privacy-policy").unwrap(),
        )
        .unwrap();
        let contribution = OwnerScopeContribution::new(
            contract,
            tenant_id.clone(),
            RecordId::try_new("party-a").unwrap(),
            1,
            [resource],
            ContributionCompletenessProof::new(true, 1, 1, 1, [3; 32]).unwrap(),
        )
        .unwrap();
        let discovery =
            DiscoveryOwnerScopeContribution::new(lineage.clone(), contribution).unwrap();
        let snapshot =
            DiscoveryScopeSnapshot::finalize(lineage, registry, 2_000_000, [discovery]).unwrap();
        let plan = PrivacyActionPlan::build(
            &snapshot,
            5,
            PrivacyCaseKind::Erasure,
            ActionPlanningPolicy::new(
                SchemaVersion::try_new("privacy-policy/1").unwrap(),
                "EU",
                false,
                false,
            )
            .unwrap(),
            3_000_000,
        )
        .unwrap();
        let decision = PrivacyRetentionDecisionSet::build(&plan, &[], 4_000_000).unwrap();
        PrivacyOwnerActionAttempt::build(
            tenant_id,
            case_id,
            plan.plan_id().clone(),
            *plan.digest(),
            decision.decision_id().clone(),
            *decision.digest(),
            &decision.items()[0],
            generation,
            5_000_000 + i64::from(generation),
        )
        .unwrap()
    }

    fn invocation(attempt: &PrivacyOwnerActionAttempt) -> OwnerExecutionInvocation {
        OwnerExecutionInvocation {
            tenant_id: attempt.tenant_id().clone(),
            privacy_case_id: attempt.privacy_case_id().clone(),
            action_plan_id: attempt.action_plan_id().clone(),
            retention_decision_id: attempt.retention_decision_id().clone(),
            actor_id: ActorId::try_new("actor-a").unwrap(),
            request_id: RequestId::try_new("request-a").unwrap(),
            correlation_id: CorrelationId::try_new("correlation-a").unwrap(),
            trace_id: TraceId::try_new("trace-a").unwrap(),
            initiating_capability_id: CapabilityId::try_new(RETENTION_APPROVAL_TRIGGER_CAPABILITY)
                .unwrap(),
            initiating_capability_version: CapabilityVersion::try_new(
                OWNER_EXECUTION_CAPABILITY_VERSION,
            )
            .unwrap(),
            request_started_at_unix_nanos: 41,
            planned_at_unix_nanos: attempt.planned_at_unix_nanos(),
            trusted_internal: true,
        }
    }

    struct Harness {
        service: PrivacyOwnerExecutionService,
        owner: Arc<RecordingOwner>,
        persistence: Arc<ScriptedPersistence>,
        order: Arc<Mutex<Vec<&'static str>>>,
    }

    fn harness(
        attempt: PrivacyOwnerActionAttempt,
        attempt_replayed: bool,
        record_inserted: bool,
        active: bool,
    ) -> Harness {
        let order = Arc::new(Mutex::new(Vec::new()));
        let owner = Arc::new(RecordingOwner {
            result: OwnerActionResult {
                status: PrivacyOwnerOutcomeStatus::Succeeded,
                safe_failure_code: None,
            },
            order: order.clone(),
            requests: Mutex::new(Vec::new()),
        });
        let persistence = Arc::new(ScriptedPersistence {
            preparation: ExecutionPreparation::Ready {
                attempt: Box::new(attempt),
                attempt_replayed,
            },
            record_inserted,
            checkpoint: CheckpointAdvance {
                next_sequence: 2,
                total_items: 1,
                complete: true,
            },
            order: order.clone(),
            recorded: Mutex::new(Vec::new()),
        });
        let service = PrivacyOwnerExecutionService::new(
            Arc::new(Activation { active }),
            persistence.clone(),
            exact_endpoints(owner.clone()),
        );
        Harness {
            service,
            owner,
            persistence,
            order,
        }
    }

    fn worker_harness(
        active: bool,
        work: Vec<OwnerExecutionInvocation>,
    ) -> (PrivacyOwnerExecutionService, Arc<WorkerPersistence>) {
        let order = Arc::new(Mutex::new(Vec::new()));
        let owner = Arc::new(RecordingOwner {
            result: OwnerActionResult {
                status: PrivacyOwnerOutcomeStatus::Succeeded,
                safe_failure_code: None,
            },
            order,
            requests: Mutex::new(Vec::new()),
        });
        let persistence = Arc::new(WorkerPersistence {
            work,
            load_calls: AtomicUsize::new(0),
            prepare_calls: AtomicUsize::new(0),
        });
        (
            PrivacyOwnerExecutionService::new(
                Arc::new(Activation { active }),
                persistence.clone(),
                exact_endpoints(owner),
            ),
            persistence,
        )
    }

    #[test]
    fn exact_owner_registry_rejects_missing_or_duplicate_owners() {
        let order = Arc::new(Mutex::new(Vec::new()));
        let owner = Arc::new(RecordingOwner {
            result: OwnerActionResult {
                status: PrivacyOwnerOutcomeStatus::Succeeded,
                safe_failure_code: None,
            },
            order,
            requests: Mutex::new(Vec::new()),
        });
        let missing = EXPECTED_OWNER_MODULES[..8]
            .iter()
            .map(|module_id| OwnerActionEndpoint {
                owner_module_id: ModuleId::try_new(*module_id).unwrap(),
                executor: owner.clone(),
            });
        assert!(OwnerActionEndpoints::exact_canonical(missing).is_err());

        let mut duplicate = EXPECTED_OWNER_MODULES
            .iter()
            .map(|module_id| OwnerActionEndpoint {
                owner_module_id: ModuleId::try_new(*module_id).unwrap(),
                executor: owner.clone(),
            })
            .collect::<Vec<_>>();
        duplicate.push(OwnerActionEndpoint {
            owner_module_id: ModuleId::try_new("crm.parties").unwrap(),
            executor: owner,
        });
        assert!(OwnerActionEndpoints::exact_canonical(duplicate).is_err());
    }

    #[test]
    fn execution_is_activation_and_trusted_internal_gated() {
        let attempt = attempt(EvidenceClass::DestroyableSubjectData, 0);
        let inactive = harness(attempt.clone(), false, true, false);
        let error = block_on(inactive.service.execute_next(invocation(&attempt))).unwrap_err();
        assert_eq!(
            error.code.as_str(),
            "CUSTOMER_PRIVACY_OWNER_EXECUTION_DISABLED"
        );

        let untrusted = harness(attempt.clone(), false, true, true);
        let mut request = invocation(&attempt);
        request.trusted_internal = false;
        let error = block_on(untrusted.service.execute_next(request)).unwrap_err();
        assert_eq!(
            error.code.as_str(),
            "CUSTOMER_PRIVACY_OWNER_EXECUTION_NOT_TRUSTED"
        );
    }

    #[test]
    fn durable_prepare_precedes_owner_and_outcome_checkpoint_writes() {
        let attempt = attempt(EvidenceClass::DestroyableSubjectData, 0);
        let permanent_key = attempt.target_idempotency_key().as_str().to_owned();
        let harness = harness(attempt.clone(), true, false, true);
        let result = block_on(harness.service.execute_next(invocation(&attempt))).unwrap();

        assert_eq!(
            &*harness.order.lock().unwrap(),
            &["prepare", "owner", "record", "advance"]
        );
        assert!(result.attempt_replayed);
        assert!(result.outcome_replayed);
        assert!(result.owner_invoked);
        assert!(result.complete);
        let requests = harness.owner.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].target_idempotency_key, permanent_key);
        assert_eq!(
            requests[0].owner_capability_id,
            "parties.privacy.action.apply"
        );
        assert_eq!(harness.persistence.recorded.lock().unwrap().len(), 1);
    }

    #[test]
    fn retry_generation_keeps_permanent_owner_idempotency_identity() {
        let first = attempt(EvidenceClass::DestroyableSubjectData, 0);
        let retry = attempt(EvidenceClass::DestroyableSubjectData, 1);
        assert_ne!(first.attempt_id(), retry.attempt_id());
        assert_eq!(
            first.target_idempotency_key(),
            retry.target_idempotency_key()
        );
    }

    #[test]
    fn coordinator_only_retention_never_invokes_owner_capability() {
        let attempt = attempt(EvidenceClass::ImmutableRequiredEvidence, 0);
        assert_eq!(
            attempt.coordinator_outcome_status(),
            Some(PrivacyOwnerOutcomeStatus::BlockedByRetention)
        );
        let harness = harness(attempt.clone(), false, true, true);
        let result = block_on(harness.service.execute_next(invocation(&attempt))).unwrap();
        assert!(!result.owner_invoked);
        assert!(harness.owner.requests.lock().unwrap().is_empty());
        assert_eq!(
            &*harness.order.lock().unwrap(),
            &["prepare", "record", "advance"]
        );
    }

    #[test]
    fn completed_execution_is_terminal_and_has_no_owner_or_write_replay() {
        let order = Arc::new(Mutex::new(Vec::new()));
        let owner = Arc::new(RecordingOwner {
            result: OwnerActionResult {
                status: PrivacyOwnerOutcomeStatus::Succeeded,
                safe_failure_code: None,
            },
            order: order.clone(),
            requests: Mutex::new(Vec::new()),
        });
        let persistence = Arc::new(ScriptedPersistence {
            preparation: ExecutionPreparation::Complete {
                total_items: 1,
                durable_outcomes: 1,
            },
            record_inserted: false,
            checkpoint: CheckpointAdvance {
                next_sequence: 2,
                total_items: 1,
                complete: true,
            },
            order: order.clone(),
            recorded: Mutex::new(Vec::new()),
        });
        let service = PrivacyOwnerExecutionService::new(
            Arc::new(Activation { active: true }),
            persistence.clone(),
            exact_endpoints(owner.clone()),
        );
        let attempt = attempt(EvidenceClass::DestroyableSubjectData, 0);
        let result = block_on(service.execute_next(invocation(&attempt))).unwrap();
        assert!(result.complete);
        assert!(!result.owner_invoked);
        assert!(result.attempt.is_none());
        assert!(result.outcome.is_none());
        assert!(owner.requests.lock().unwrap().is_empty());
        assert!(persistence.recorded.lock().unwrap().is_empty());
        assert_eq!(&*order.lock().unwrap(), &["prepare"]);
    }

    #[test]
    fn inactive_worker_cycle_does_not_discover_or_execute_work() {
        let prepared = attempt(EvidenceClass::DestroyableSubjectData, 0);
        let (service, persistence) = worker_harness(false, vec![invocation(&prepared)]);
        block_on(service.run_tenant_cycle(prepared.tenant_id().clone(), 6_000_000)).unwrap();
        assert_eq!(persistence.load_calls.load(Ordering::SeqCst), 0);
        assert_eq!(persistence.prepare_calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn active_worker_cycle_loads_bounded_work_and_delegates_to_replay_safe_execution() {
        let prepared = attempt(EvidenceClass::DestroyableSubjectData, 0);
        let (service, persistence) = worker_harness(true, vec![invocation(&prepared)]);
        block_on(service.run_tenant_cycle(prepared.tenant_id().clone(), 6_000_000)).unwrap();
        assert_eq!(persistence.load_calls.load(Ordering::SeqCst), 1);
        assert_eq!(persistence.prepare_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn invalid_worker_batch_fails_before_any_execution() {
        let prepared = attempt(EvidenceClass::DestroyableSubjectData, 0);
        let duplicate = invocation(&prepared);
        let (service, persistence) = worker_harness(true, vec![duplicate.clone(), duplicate]);
        let error = block_on(service.run_tenant_cycle(prepared.tenant_id().clone(), 6_000_000))
            .expect_err("duplicate work must fail closed");
        assert_eq!(
            error.code.as_str(),
            "CUSTOMER_PRIVACY_OWNER_EXECUTION_WORK_BATCH_INVALID"
        );
        assert_eq!(persistence.load_calls.load(Ordering::SeqCst), 1);
        assert_eq!(persistence.prepare_calls.load(Ordering::SeqCst), 0);
    }
}
