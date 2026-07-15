#![forbid(unsafe_code)]

use crm_application_runtime::{
    PartyExportArtifactDownloadRequest, PartyExportArtifactDownloadResult,
    PartyExportArtifactDownloadService,
};
use crm_capability_runtime::CapabilityRisk;
use crm_customer_data_operations_query_adapter::{
    DOWNLOAD_EXPORT_ARTIFACT_CAPABILITY, DOWNLOAD_EXPORT_ARTIFACT_REQUEST_SCHEMA,
    artifact_download_capability_definition,
};
use crm_module_sdk::DataClass;

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn disclosure_service_and_request_are_thread_safe() {
    assert_send_sync::<PartyExportArtifactDownloadService>();
    assert_send_sync::<PartyExportArtifactDownloadRequest>();
    assert_send_sync::<PartyExportArtifactDownloadResult>();
}

#[test]
fn disclosure_capability_remains_dedicated_high_risk_read_only_surface() {
    let definition = artifact_download_capability_definition().unwrap();
    assert_eq!(
        definition.capability_id.as_str(),
        DOWNLOAD_EXPORT_ARTIFACT_CAPABILITY
    );
    assert_eq!(
        definition.authorization_policy_id,
        DOWNLOAD_EXPORT_ARTIFACT_CAPABILITY
    );
    assert_eq!(
        definition.input_contract.schema_id.as_str(),
        DOWNLOAD_EXPORT_ARTIFACT_REQUEST_SCHEMA
    );
    assert_eq!(
        definition.input_contract.allowed_data_classes,
        vec![DataClass::Personal]
    );
    assert_eq!(definition.risk, CapabilityRisk::High);
    assert!(!definition.mutation);
    assert!(!definition.requires_idempotency);
    assert!(!definition.requires_approval);
    assert!(definition.output_contract.is_none());
}
