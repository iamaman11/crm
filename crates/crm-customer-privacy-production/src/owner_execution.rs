use crate::legacy::CustomerPrivacyProductionDependencies;
pub use crm_customer_privacy_application::{
    OwnerActionEndpoint, OwnerActionEndpoints, OwnerActionRequest, OwnerActionResult,
    OwnerExecutionInvocation, OwnerExecutionResult, OwnerPrivacyActionPort,
    PrivacyOwnerExecutionService,
};
use crm_customer_privacy_postgres::PostgresOwnerExecutionPersistence;
use crm_module_sdk::SdkError;
use std::sync::Arc;

/// Build the trusted-internal repository-step-eight execution coordinator.
///
/// The exact nine owner endpoints are supplied by the process composition root.
/// This function registers no public route and creates no worker.
pub fn build_internal_owner_execution(
    dependencies: &CustomerPrivacyProductionDependencies,
    endpoints: impl IntoIterator<Item = OwnerActionEndpoint>,
) -> Result<PrivacyOwnerExecutionService, SdkError> {
    Ok(PrivacyOwnerExecutionService::new(
        dependencies.activation.clone(),
        Arc::new(PostgresOwnerExecutionPersistence::new(Arc::new(
            dependencies.store.clone(),
        ))),
        OwnerActionEndpoints::exact_canonical(endpoints)?,
    ))
}
