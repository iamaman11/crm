use crate::{
    CustomerPrivacyProductionDependencies, OwnerExecutionInvocation, OwnerExecutionPersistencePort,
    PostgresOwnerExecutionPersistence, PrivacyOwnerExecutionService,
    build_canonical_internal_owner_execution,
};
use crm_application_composition::{ModuleActivationPort, TenantBackgroundWorker};
use crm_core_data::PostgresDataStore;
use crm_module_sdk::{ErrorCategory, PortFuture, RecordRef, SdkError, TenantId};
use crm_query_runtime::{QueryRequest, QueryVisibilityAuthorizer, QueryVisibilityDecision};
use std::collections::BTreeSet;
use std::sync::Arc;

const OWNER_EXECUTION_WORK_LIMIT: u32 = 64;

struct CustomerPrivacyProductionOwnerExecutionWorker {
    ready: Arc<dyn OwnerExecutionPersistencePort>,
    execution: Arc<PrivacyOwnerExecutionService>,
}

impl std::fmt::Debug for CustomerPrivacyProductionOwnerExecutionWorker {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CustomerPrivacyProductionOwnerExecutionWorker")
            .field("ready", &"dyn OwnerExecutionPersistencePort")
            .field("execution", &self.execution)
            .finish()
    }
}

impl From<(PostgresDataStore, Arc<dyn ModuleActivationPort>)>
    for CustomerPrivacyProductionDependencies
{
    fn from((store, activation): (PostgresDataStore, Arc<dyn ModuleActivationPort>)) -> Self {
        Self {
            store,
            activation,
            visibility_authorizer: Arc::new(BackgroundOnlyVisibilityAuthorizer),
            cursor_key: [0x43; 32],
        }
    }
}

impl TryFrom<CustomerPrivacyProductionDependencies> for Arc<dyn TenantBackgroundWorker> {
    type Error = SdkError;

    fn try_from(dependencies: CustomerPrivacyProductionDependencies) -> Result<Self, Self::Error> {
        let ready: Arc<dyn OwnerExecutionPersistencePort> =
            PostgresOwnerExecutionPersistence::new(Arc::new(dependencies.store.clone())).into();
        let execution = Arc::new(build_canonical_internal_owner_execution(&dependencies)?);
        Ok(Arc::new(CustomerPrivacyProductionOwnerExecutionWorker {
            ready,
            execution,
        }))
    }
}

impl TenantBackgroundWorker for CustomerPrivacyProductionOwnerExecutionWorker {
    fn run_tenant_cycle<'a>(
        &'a self,
        tenant_id: TenantId,
        now_unix_nanos: i64,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            let work = self
                .ready
                .load_ready(&tenant_id, now_unix_nanos, OWNER_EXECUTION_WORK_LIMIT)
                .await?;
            validate_work_batch(&tenant_id, now_unix_nanos, &work)?;
            for invocation in work {
                self.execution.execute_next(invocation).await?;
            }
            Ok(())
        })
    }
}

#[derive(Debug)]
struct BackgroundOnlyVisibilityAuthorizer;

impl QueryVisibilityAuthorizer for BackgroundOnlyVisibilityAuthorizer {
    fn authorize_visibility<'a>(
        &'a self,
        _request: &'a QueryRequest,
        _resource: &'a RecordRef,
    ) -> PortFuture<'a, Result<QueryVisibilityDecision, SdkError>> {
        Box::pin(async {
            Ok(QueryVisibilityDecision::denied(
                "customer-privacy-background-only",
                "not-applicable",
            ))
        })
    }
}

fn validate_work_batch(
    tenant_id: &TenantId,
    now_unix_nanos: i64,
    work: &[OwnerExecutionInvocation],
) -> Result<(), SdkError> {
    if work.len() > OWNER_EXECUTION_WORK_LIMIT as usize {
        return Err(work_batch_invalid(
            "ready source exceeded the frozen production work limit",
        ));
    }
    let mut identities = BTreeSet::new();
    for invocation in work {
        if &invocation.tenant_id != tenant_id {
            return Err(work_batch_invalid(
                "ready source returned work for another tenant",
            ));
        }
        if !invocation.trusted_internal {
            return Err(work_batch_invalid(
                "ready source returned work without trusted-internal provenance",
            ));
        }
        if invocation.request_started_at_unix_nanos <= 0
            || invocation.planned_at_unix_nanos < invocation.request_started_at_unix_nanos
            || invocation.planned_at_unix_nanos > now_unix_nanos
        {
            return Err(work_batch_invalid(
                "ready source returned missing, non-monotonic or future execution time",
            ));
        }
        let identity = (
            invocation.privacy_case_id.as_str().to_owned(),
            invocation.action_plan_id.as_str().to_owned(),
            invocation.retention_decision_id.as_str().to_owned(),
        );
        if !identities.insert(identity) {
            return Err(work_batch_invalid(
                "ready source returned duplicate execution identity",
            ));
        }
    }
    Ok(())
}

fn work_batch_invalid(reference: impl Into<String>) -> SdkError {
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
    use crm_module_sdk::{
        ActorId, CapabilityId, CapabilityVersion, CorrelationId, RecordId, RequestId, TraceId,
    };

    #[test]
    fn production_batch_rejects_duplicate_identity_before_execution() {
        let tenant = TenantId::try_new("tenant-a").unwrap();
        let invocation = OwnerExecutionInvocation {
            tenant_id: tenant.clone(),
            privacy_case_id: RecordId::try_new("privacy-case-a").unwrap(),
            action_plan_id: RecordId::try_new("privacy-plan-a").unwrap(),
            retention_decision_id: RecordId::try_new("privacy-decision-a").unwrap(),
            actor_id: ActorId::try_new("privacy-worker").unwrap(),
            request_id: RequestId::try_new("privacy-request-a").unwrap(),
            correlation_id: CorrelationId::try_new("privacy-correlation-a").unwrap(),
            trace_id: TraceId::try_new("privacy-trace-a").unwrap(),
            initiating_capability_id: CapabilityId::try_new("customer_privacy.case.approve")
                .unwrap(),
            initiating_capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
            request_started_at_unix_nanos: 10,
            planned_at_unix_nanos: 20,
            trusted_internal: true,
        };
        let error = validate_work_batch(&tenant, 20, &[invocation.clone(), invocation])
            .expect_err("duplicate production work must fail closed");
        assert_eq!(
            error.code.as_str(),
            "CUSTOMER_PRIVACY_OWNER_EXECUTION_WORK_BATCH_INVALID"
        );
    }
}
