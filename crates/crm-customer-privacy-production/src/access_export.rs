use crate::legacy::CustomerPrivacyProductionDependencies;
pub use crm_customer_privacy_application::{
    AccessExportInvocation, AccessExportResult, PrivacyAccessExportService,
    PrivacyExportTargetPort, PrivacyExportTargetRequest, PrivacyExportTargetResult,
};
pub use crm_customer_privacy_postgres::PostgresAccessExportPersistence;
use std::sync::Arc;

/// Build the frozen trusted-internal Customer Privacy access/export coordinator.
///
/// The caller supplies the exact Customer Data Operations target. This function
/// registers no public route and creates no worker.
pub fn build_internal_access_export(
    dependencies: &CustomerPrivacyProductionDependencies,
    target: Arc<dyn PrivacyExportTargetPort>,
) -> PrivacyAccessExportService {
    PrivacyAccessExportService::new(
        dependencies.activation.clone(),
        Arc::new(PostgresAccessExportPersistence::new(Arc::new(
            dependencies.store.clone(),
        ))),
        target,
    )
}
