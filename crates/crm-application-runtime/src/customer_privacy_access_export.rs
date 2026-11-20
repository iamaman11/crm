use crm_customer_data_operations_execution_composition::{
    PrivacyManifestExportPublisher, PrivacyManifestExportRequest,
};
use crm_customer_privacy_production::{
    CustomerPrivacyProductionDependencies, PrivacyAccessExportService, PrivacyExportTargetPort,
    PrivacyExportTargetRequest, PrivacyExportTargetResult, build_internal_access_export,
};
use crm_module_sdk::{PortFuture, SdkError};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct CustomerDataPrivacyExportTarget {
    publisher: Arc<PrivacyManifestExportPublisher>,
}

impl CustomerDataPrivacyExportTarget {
    pub fn new(publisher: Arc<PrivacyManifestExportPublisher>) -> Self {
        Self { publisher }
    }
}

impl PrivacyExportTargetPort for CustomerDataPrivacyExportTarget {
    fn request<'a>(
        &'a self,
        request: PrivacyExportTargetRequest,
    ) -> PortFuture<'a, Result<PrivacyExportTargetResult, SdkError>> {
        Box::pin(async move {
            let result = self
                .publisher
                .request(PrivacyManifestExportRequest {
                    tenant_id: request.tenant_id,
                    privacy_case_id: request.privacy_case_id,
                    export_job_id: request.export_job_id,
                    target_idempotency_key: request.target_idempotency_key,
                    manifest_id: request.manifest_id,
                    manifest_digest: request.manifest_digest,
                    manifest_bytes: request.manifest_bytes,
                    actor_id: request.actor_id,
                    correlation_id: request.correlation_id,
                    trace_id: request.trace_id,
                    initiating_capability_id: request.initiating_capability_id,
                    initiating_capability_version: request.initiating_capability_version,
                    prepared_at_unix_nanos: request.prepared_at_unix_nanos,
                })
                .await?;
            Ok(PrivacyExportTargetResult {
                export_job_id: result.export_job_id,
                file_id: result.file_id,
                media_type: result.media_type,
                content_sha256: result.content_sha256,
                size_bytes: result.size_bytes,
                retention_policy_id: result.retention_policy_id,
                completed_at_unix_nanos: result.completed_at_unix_nanos,
                replayed: result.replayed,
            })
        })
    }
}

/// Compose repository-step-ten trusted-internal access/export assembly.
///
/// The returned service is not registered in the public mutation/query catalog.
pub fn build_customer_privacy_access_export(
    dependencies: &CustomerPrivacyProductionDependencies,
    publisher: Arc<PrivacyManifestExportPublisher>,
) -> PrivacyAccessExportService {
    build_internal_access_export(
        dependencies,
        Arc::new(CustomerDataPrivacyExportTarget::new(publisher)),
    )
}
