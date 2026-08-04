use crate::{OwnerExecutionInvocation, OwnerExecutionResult, PrivacyOwnerExecutionService};
use crm_application_composition::ModuleActivationPort;
use crm_customer_privacy::MODULE_ID;
use crm_module_sdk::{ErrorCategory, ModuleId, PortFuture, SdkError, TenantId};
use std::collections::BTreeSet;
use std::sync::Arc;

pub const DEFAULT_OWNER_EXECUTION_WORK_LIMIT: u32 = 64;
pub const MAXIMUM_OWNER_EXECUTION_WORK_LIMIT: u32 = 1_024;

/// Supplies already planned, immutable Customer Privacy owner-execution work.
///
/// A production implementation must discover only cases whose action plan and
/// retention decision are durable. It must not perform owner effects while
/// loading work; those remain exclusively owned by [`OwnerExecutionStepPort`].
pub trait OwnerExecutionWorkSourcePort: Send + Sync {
    fn load_ready<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        now_unix_nanos: i64,
        maximum_items: u32,
    ) -> PortFuture<'a, Result<Vec<OwnerExecutionInvocation>, SdkError>>;
}

/// Executes one replay-safe owner-action step for a prepared privacy case.
pub trait OwnerExecutionStepPort: Send + Sync {
    fn execute_next<'a>(
        &'a self,
        invocation: OwnerExecutionInvocation,
    ) -> PortFuture<'a, Result<OwnerExecutionResult, SdkError>>;
}

impl OwnerExecutionStepPort for PrivacyOwnerExecutionService {
    fn execute_next<'a>(
        &'a self,
        invocation: OwnerExecutionInvocation,
    ) -> PortFuture<'a, Result<OwnerExecutionResult, SdkError>> {
        Box::pin(async move { PrivacyOwnerExecutionService::execute_next(self, invocation).await })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerExecutionWorkerCycleResult {
    pub active: bool,
    pub loaded: u32,
    pub attempted: u32,
    pub completed: u32,
    pub replayed: u32,
}

/// Bounded owner-owned application worker over the accepted replay-safe
/// Customer Privacy execution service.
///
/// The worker checks durable tenant activation before work discovery so a
/// disabled or uninstalled module cannot read pending work or invoke an owner.
/// The execution service repeats activation immediately before effects.
#[derive(Clone)]
pub struct CustomerPrivacyOwnerExecutionWorker {
    activation: Arc<dyn ModuleActivationPort>,
    source: Arc<dyn OwnerExecutionWorkSourcePort>,
    execution: Arc<dyn OwnerExecutionStepPort>,
    maximum_items_per_cycle: u32,
}

impl std::fmt::Debug for CustomerPrivacyOwnerExecutionWorker {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CustomerPrivacyOwnerExecutionWorker")
            .field("activation", &"dyn ModuleActivationPort")
            .field("source", &"dyn OwnerExecutionWorkSourcePort")
            .field("execution", &"dyn OwnerExecutionStepPort")
            .field("maximum_items_per_cycle", &self.maximum_items_per_cycle)
            .finish()
    }
}

impl CustomerPrivacyOwnerExecutionWorker {
    pub fn try_new(
        activation: Arc<dyn ModuleActivationPort>,
        source: Arc<dyn OwnerExecutionWorkSourcePort>,
        execution: Arc<dyn OwnerExecutionStepPort>,
        maximum_items_per_cycle: u32,
    ) -> Result<Self, SdkError> {
        if maximum_items_per_cycle == 0
            || maximum_items_per_cycle > MAXIMUM_OWNER_EXECUTION_WORK_LIMIT
        {
            return Err(worker_configuration_invalid(
                "owner-execution work limit must be between one and the frozen maximum",
            ));
        }
        Ok(Self {
            activation,
            source,
            execution,
            maximum_items_per_cycle,
        })
    }

    pub async fn run_tenant_cycle(
        &self,
        tenant_id: TenantId,
        now_unix_nanos: i64,
    ) -> Result<OwnerExecutionWorkerCycleResult, SdkError> {
        if now_unix_nanos <= 0 {
            return Err(worker_input_invalid(
                "worker cycle time must be after the Unix epoch",
            ));
        }
        let module_id = ModuleId::try_new(MODULE_ID).map_err(worker_configuration_invalid)?;
        if !self.activation.is_active(&tenant_id, &module_id).await? {
            return Ok(OwnerExecutionWorkerCycleResult {
                active: false,
                loaded: 0,
                attempted: 0,
                completed: 0,
                replayed: 0,
            });
        }

        let work = self
            .source
            .load_ready(&tenant_id, now_unix_nanos, self.maximum_items_per_cycle)
            .await?;
        validate_work_batch(
            &tenant_id,
            now_unix_nanos,
            self.maximum_items_per_cycle,
            &work,
        )?;

        let loaded = u32::try_from(work.len()).map_err(worker_configuration_invalid)?;
        let mut result = OwnerExecutionWorkerCycleResult {
            active: true,
            loaded,
            attempted: 0,
            completed: 0,
            replayed: 0,
        };
        for invocation in work {
            let execution = self.execution.execute_next(invocation).await?;
            result.attempted = result.attempted.saturating_add(1);
            if execution.complete {
                result.completed = result.completed.saturating_add(1);
            }
            if !execution.owner_invoked && execution.attempt_replayed && execution.outcome_replayed
            {
                result.replayed = result.replayed.saturating_add(1);
            }
        }
        Ok(result)
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

fn worker_configuration_invalid(reference: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_OWNER_EXECUTION_WORKER_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Customer Privacy owner-execution worker is not configured correctly.",
    )
    .with_internal_reference(reference.to_string())
}

fn worker_input_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_OWNER_EXECUTION_WORKER_INPUT_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "The Customer Privacy owner-execution worker input is invalid.",
    )
    .with_internal_reference(reference)
}

fn worker_batch_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_OWNER_EXECUTION_WORK_BATCH_INVALID",
        ErrorCategory::Conflict,
        false,
        "The Customer Privacy owner-execution work batch is invalid.",
    )
    .with_internal_reference(reference)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RETENTION_APPROVAL_TRIGGER_CAPABILITY, RETENTION_TRIGGER_CAPABILITY_VERSION};
    use crm_module_sdk::{
        ActorId, CapabilityId, CapabilityVersion, CorrelationId, RecordId, RequestId, TraceId,
    };
    use std::collections::VecDeque;
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
        calls: AtomicUsize,
    }

    impl ModuleActivationPort for Activation {
        fn is_active<'a>(
            &'a self,
            _tenant_id: &'a TenantId,
            _module_id: &'a ModuleId,
        ) -> PortFuture<'a, Result<bool, SdkError>> {
            Box::pin(async move {
                self.calls.fetch_add(1, Ordering::SeqCst);
                Ok(self.active)
            })
        }
    }

    #[derive(Debug)]
    struct Source {
        calls: AtomicUsize,
        work: Vec<OwnerExecutionInvocation>,
    }

    impl OwnerExecutionWorkSourcePort for Source {
        fn load_ready<'a>(
            &'a self,
            _tenant_id: &'a TenantId,
            _now_unix_nanos: i64,
            _maximum_items: u32,
        ) -> PortFuture<'a, Result<Vec<OwnerExecutionInvocation>, SdkError>> {
            Box::pin(async move {
                self.calls.fetch_add(1, Ordering::SeqCst);
                Ok(self.work.clone())
            })
        }
    }

    #[derive(Debug)]
    struct Execution {
        calls: Mutex<Vec<OwnerExecutionInvocation>>,
        results: Mutex<VecDeque<Result<OwnerExecutionResult, SdkError>>>,
    }

    impl OwnerExecutionStepPort for Execution {
        fn execute_next<'a>(
            &'a self,
            invocation: OwnerExecutionInvocation,
        ) -> PortFuture<'a, Result<OwnerExecutionResult, SdkError>> {
            Box::pin(async move {
                self.calls.lock().unwrap().push(invocation);
                self.results
                    .lock()
                    .unwrap()
                    .pop_front()
                    .expect("script one result for every invocation")
            })
        }
    }

    #[test]
    fn disabled_module_performs_no_work_discovery_or_execution() {
        let activation = Arc::new(Activation {
            active: false,
            calls: AtomicUsize::new(0),
        });
        let source = Arc::new(Source {
            calls: AtomicUsize::new(0),
            work: vec![invocation("tenant-a", "a")],
        });
        let execution = Arc::new(Execution {
            calls: Mutex::new(Vec::new()),
            results: Mutex::new(VecDeque::from([Ok(completed_replay())])),
        });
        let worker = CustomerPrivacyOwnerExecutionWorker::try_new(
            activation.clone(),
            source.clone(),
            execution.clone(),
            DEFAULT_OWNER_EXECUTION_WORK_LIMIT,
        )
        .unwrap();

        let result = block_on(worker.run_tenant_cycle(tenant("tenant-a"), 100)).unwrap();
        assert_eq!(
            result,
            OwnerExecutionWorkerCycleResult {
                active: false,
                loaded: 0,
                attempted: 0,
                completed: 0,
                replayed: 0,
            }
        );
        assert_eq!(activation.calls.load(Ordering::SeqCst), 1);
        assert_eq!(source.calls.load(Ordering::SeqCst), 0);
        assert!(execution.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn active_cycle_is_bounded_and_reports_completion_and_replay() {
        let activation = Arc::new(Activation {
            active: true,
            calls: AtomicUsize::new(0),
        });
        let source = Arc::new(Source {
            calls: AtomicUsize::new(0),
            work: vec![invocation("tenant-a", "a"), invocation("tenant-a", "b")],
        });
        let execution = Arc::new(Execution {
            calls: Mutex::new(Vec::new()),
            results: Mutex::new(VecDeque::from([Ok(completed_replay()), Ok(progressed())])),
        });
        let worker = CustomerPrivacyOwnerExecutionWorker::try_new(
            activation,
            source.clone(),
            execution.clone(),
            2,
        )
        .unwrap();

        let result = block_on(worker.run_tenant_cycle(tenant("tenant-a"), 100)).unwrap();
        assert_eq!(
            result,
            OwnerExecutionWorkerCycleResult {
                active: true,
                loaded: 2,
                attempted: 2,
                completed: 1,
                replayed: 1,
            }
        );
        assert_eq!(source.calls.load(Ordering::SeqCst), 1);
        assert_eq!(execution.calls.lock().unwrap().len(), 2);
    }

    #[test]
    fn invalid_or_duplicate_source_batch_fails_before_any_execution() {
        let duplicate = invocation("tenant-a", "same");
        let source = Arc::new(Source {
            calls: AtomicUsize::new(0),
            work: vec![duplicate.clone(), duplicate],
        });
        let execution = Arc::new(Execution {
            calls: Mutex::new(Vec::new()),
            results: Mutex::new(VecDeque::new()),
        });
        let worker = CustomerPrivacyOwnerExecutionWorker::try_new(
            Arc::new(Activation {
                active: true,
                calls: AtomicUsize::new(0),
            }),
            source,
            execution.clone(),
            2,
        )
        .unwrap();
        let error = block_on(worker.run_tenant_cycle(tenant("tenant-a"), 100))
            .expect_err("duplicate work must fail closed");
        assert_eq!(
            error.code.as_str(),
            "CUSTOMER_PRIVACY_OWNER_EXECUTION_WORK_BATCH_INVALID"
        );
        assert!(execution.calls.lock().unwrap().is_empty());

        let cross_tenant_source = Arc::new(Source {
            calls: AtomicUsize::new(0),
            work: vec![invocation("tenant-b", "cross")],
        });
        let cross_tenant_execution = Arc::new(Execution {
            calls: Mutex::new(Vec::new()),
            results: Mutex::new(VecDeque::new()),
        });
        let worker = CustomerPrivacyOwnerExecutionWorker::try_new(
            Arc::new(Activation {
                active: true,
                calls: AtomicUsize::new(0),
            }),
            cross_tenant_source,
            cross_tenant_execution.clone(),
            1,
        )
        .unwrap();
        let error = block_on(worker.run_tenant_cycle(tenant("tenant-a"), 100))
            .expect_err("cross-tenant work must fail closed");
        assert_eq!(
            error.code.as_str(),
            "CUSTOMER_PRIVACY_OWNER_EXECUTION_WORK_BATCH_INVALID"
        );
        assert!(cross_tenant_execution.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn source_cannot_exceed_requested_limit() {
        let worker = CustomerPrivacyOwnerExecutionWorker::try_new(
            Arc::new(Activation {
                active: true,
                calls: AtomicUsize::new(0),
            }),
            Arc::new(Source {
                calls: AtomicUsize::new(0),
                work: vec![invocation("tenant-a", "a"), invocation("tenant-a", "b")],
            }),
            Arc::new(Execution {
                calls: Mutex::new(Vec::new()),
                results: Mutex::new(VecDeque::new()),
            }),
            1,
        )
        .unwrap();
        let error = block_on(worker.run_tenant_cycle(tenant("tenant-a"), 100))
            .expect_err("oversized work batch must fail closed");
        assert_eq!(
            error.code.as_str(),
            "CUSTOMER_PRIVACY_OWNER_EXECUTION_WORK_BATCH_INVALID"
        );
    }

    fn tenant(value: &str) -> TenantId {
        TenantId::try_new(value).unwrap()
    }

    fn invocation(tenant_id: &str, suffix: &str) -> OwnerExecutionInvocation {
        OwnerExecutionInvocation {
            tenant_id: tenant(tenant_id),
            privacy_case_id: RecordId::try_new(format!("privacy-case-{suffix}")).unwrap(),
            action_plan_id: RecordId::try_new(format!("action-plan-{suffix}")).unwrap(),
            retention_decision_id: RecordId::try_new(format!("retention-decision-{suffix}"))
                .unwrap(),
            actor_id: ActorId::try_new("privacy-worker").unwrap(),
            request_id: RequestId::try_new(format!("privacy-worker-request-{suffix}")).unwrap(),
            correlation_id: CorrelationId::try_new(format!("privacy-worker-correlation-{suffix}"))
                .unwrap(),
            trace_id: TraceId::try_new(format!("privacy-worker-trace-{suffix}")).unwrap(),
            initiating_capability_id: CapabilityId::try_new(RETENTION_APPROVAL_TRIGGER_CAPABILITY)
                .unwrap(),
            initiating_capability_version: CapabilityVersion::try_new(
                RETENTION_TRIGGER_CAPABILITY_VERSION,
            )
            .unwrap(),
            request_started_at_unix_nanos: 10,
            planned_at_unix_nanos: 20,
            trusted_internal: true,
        }
    }

    fn completed_replay() -> OwnerExecutionResult {
        OwnerExecutionResult {
            attempt: None,
            outcome: None,
            attempt_replayed: true,
            outcome_replayed: true,
            owner_invoked: false,
            next_sequence: 2,
            total_items: 1,
            complete: true,
        }
    }

    fn progressed() -> OwnerExecutionResult {
        OwnerExecutionResult {
            attempt: None,
            outcome: None,
            attempt_replayed: false,
            outcome_replayed: false,
            owner_invoked: true,
            next_sequence: 2,
            total_items: 2,
            complete: false,
        }
    }
}
