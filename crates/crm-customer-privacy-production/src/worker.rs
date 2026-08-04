use crate::{CustomerPrivacyProductionDependencies, build_canonical_internal_owner_execution};
use crm_customer_privacy_application::{
    CustomerPrivacyOwnerExecutionWorker, OwnerExecutionStepPort, OwnerExecutionWorkSourcePort,
};
use crm_module_sdk::SdkError;
use std::sync::Arc;

/// Build the owner-owned Step 19 worker boundary over the exact canonical
/// nine-owner execution coordinator.
///
/// This constructor deliberately accepts an injected pending-work source. It
/// does not register a process worker, publish a route or classify a manifest
/// coordinate. Those actions require the later PostgreSQL discovery and real
/// `crm-api` process-acceptance slice.
pub fn build_internal_owner_execution_worker(
    dependencies: &CustomerPrivacyProductionDependencies,
    source: Arc<dyn OwnerExecutionWorkSourcePort>,
    maximum_items_per_cycle: u32,
) -> Result<CustomerPrivacyOwnerExecutionWorker, SdkError> {
    let execution: Arc<dyn OwnerExecutionStepPort> =
        Arc::new(build_canonical_internal_owner_execution(dependencies)?);
    CustomerPrivacyOwnerExecutionWorker::try_new(
        dependencies.activation.clone(),
        source,
        execution,
        maximum_items_per_cycle,
    )
}
