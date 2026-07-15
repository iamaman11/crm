use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRisk};
use crm_core_data::{PostgresDataStore, RecordGetQuery};
use crm_customer_data_operations::PartyExportJobStatus;
use crm_customer_data_operations_capability_adapter::{
    EXPORT_JOB_RECORD_TYPE, MODULE_ID, export_job_from_snapshot,
};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, FileId, ModuleId, PayloadEncoding,
    RecordId, RecordType, SdkError, TypedPayload,
};
use crm_proto_contracts::crm::customer_data_operations::v1 as wire;
use crm_query_runtime::{QueryRequest, QueryVisibilityAuthorizer};
use prost::Message;
use std::sync::Arc;

pub const DOWNLOAD_EXPORT_ARTIFACT_CAPABILITY: &str =
    "customer_data.export.party.artifact.download";
pub const DOWNLOAD_EXPORT_ARTIFACT_REQUEST_SCHEMA: &str =
    "crm.customer_data_operations.v1.DownloadPartyExportArtifactRequest";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyExportArtifactDownloadEvidence {
    pub export_job_id: String,
    pub export_job_version: i64,
    pub file_id: FileId,
    pub media_type: &'static str,
    pub content_sha256: [u8; 32],
    pub size_bytes: u64,
    pub retention_policy_id: String,
}

#[derive(Clone)]
pub struct PartyExportArtifactDownloadResolver {
    store: PostgresDataStore,
    visibility: Arc<dyn QueryVisibilityAuthorizer>,
}

impl std::fmt::Debug for PartyExportArtifactDownloadResolver {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PartyExportArtifactDownloadResolver")
            .field("store", &self.store)
            .field("visibility", &"dyn QueryVisibilityAuthorizer")
            .finish()
    }
}

impl PartyExportArtifactDownloadResolver {
    pub fn new(store: PostgresDataStore, visibility: Arc<dyn QueryVisibilityAuthorizer>) -> Self {
        Self { store, visibility }
    }

    pub async fn resolve(
        &self,
        request: &QueryRequest,
    ) -> Result<PartyExportArtifactDownloadEvidence, SdkError> {
        validate_download_request(request)?;
        let command: wire::DownloadPartyExportArtifactRequest =
            wire::DownloadPartyExportArtifactRequest::decode(request.input.bytes.as_slice())
                .map_err(|_| invalid_request())?;
        let export_job_id = command
            .export_job_ref
            .filter(|reference| !reference.export_job_id.is_empty())
            .ok_or_else(invalid_request)?
            .export_job_id;
        let record_id = RecordId::try_new(export_job_id.clone()).map_err(|_| invalid_request())?;
        let snapshot = self
            .store
            .get_record_for_query(&RecordGetQuery {
                tenant_id: request.context.tenant_id.clone(),
                owner_module_id: module_id()?,
                record_type: export_job_record_type()?,
                record_id,
            })
            .await?
            .ok_or_else(resource_not_found)?;
        let visibility = self
            .visibility
            .authorize_visibility(request, &snapshot.reference)
            .await?;
        if !visibility.resource_visible || !visibility.allows_field("artifact") {
            return Err(resource_not_found());
        }

        let job = export_job_from_snapshot(&snapshot)?;
        if job.status() != PartyExportJobStatus::Completed {
            return Err(artifact_not_ready());
        }
        let artifact = job.artifact().ok_or_else(stored_state_invalid)?;
        if artifact.retention_policy_id() != job.specification().profile().retention_policy_id() {
            return Err(stored_state_invalid());
        }

        Ok(PartyExportArtifactDownloadEvidence {
            export_job_id,
            export_job_version: job.version(),
            file_id: artifact.file_id().clone(),
            media_type: "text/csv; charset=utf-8",
            content_sha256: decode_sha256(artifact.content_sha256())?,
            size_bytes: artifact.size_bytes(),
            retention_policy_id: artifact.retention_policy_id().to_owned(),
        })
    }
}

pub fn artifact_download_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    Ok(CapabilityDefinition {
        capability_id: support::configured_identifier(CapabilityId::try_new(
            DOWNLOAD_EXPORT_ARTIFACT_CAPABILITY,
        ))?,
        capability_version: support::configured_identifier(CapabilityVersion::try_new(
            support::CONTRACT_VERSION,
        ))?,
        owner_module_id: support::configured_identifier(ModuleId::try_new(MODULE_ID))?,
        input_contract: support::protobuf_contract(
            MODULE_ID,
            DOWNLOAD_EXPORT_ARTIFACT_REQUEST_SCHEMA,
            vec![DataClass::Personal],
        )?,
        output_contract: None,
        risk: CapabilityRisk::High,
        mutation: false,
        requires_idempotency: false,
        requires_approval: false,
        authorization_policy_id: DOWNLOAD_EXPORT_ARTIFACT_CAPABILITY.to_owned(),
        rate_limit_policy_id: None,
    })
}

pub fn artifact_download_request_payload(export_job_id: &str) -> Result<TypedPayload, SdkError> {
    if export_job_id.is_empty() {
        return Err(invalid_request());
    }
    support::protobuf_payload(
        MODULE_ID,
        DOWNLOAD_EXPORT_ARTIFACT_REQUEST_SCHEMA,
        DataClass::Personal,
        &wire::DownloadPartyExportArtifactRequest {
            export_job_ref: Some(wire::ExportJobRef {
                export_job_id: export_job_id.to_owned(),
            }),
        },
    )
}

fn validate_download_request(request: &QueryRequest) -> Result<(), SdkError> {
    let payload = &request.input;
    if request.owner_module_id.as_str() != MODULE_ID
        || request.context.capability_id.as_str() != DOWNLOAD_EXPORT_ARTIFACT_CAPABILITY
        || payload.owner.as_str() != MODULE_ID
        || payload.schema_id.as_str() != DOWNLOAD_EXPORT_ARTIFACT_REQUEST_SCHEMA
        || payload.schema_version.as_str() != support::CONTRACT_VERSION
        || payload.descriptor_hash != support::message_descriptor_hash(DOWNLOAD_EXPORT_ARTIFACT_REQUEST_SCHEMA)
        || payload.data_class != DataClass::Personal
        || payload.encoding != PayloadEncoding::Protobuf
        || payload.maximum_size_bytes > support::MAX_PROTOBUF_BYTES
        || payload.validate().is_err()
    {
        return Err(invalid_request());
    }
    Ok(())
}

fn decode_sha256(value: &str) -> Result<[u8; 32], SdkError> {
    if value.len() != 64 {
        return Err(stored_state_invalid());
    }
    let mut output = [0_u8; 32];
    for (index, byte) in output.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .map_err(|_| stored_state_invalid())?;
    }
    Ok(output)
}

fn module_id() -> Result<ModuleId, SdkError> {
    ModuleId::try_new(MODULE_ID).map_err(configuration_error)
}

fn export_job_record_type() -> Result<RecordType, SdkError> {
    RecordType::try_new(EXPORT_JOB_RECORD_TYPE).map_err(configuration_error)
}

fn invalid_request() -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_ARTIFACT_DOWNLOAD_REQUEST_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "The export artifact disclosure request is invalid.",
    )
}

fn resource_not_found() -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_ARTIFACT_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The requested export artifact is unavailable.",
    )
}

fn artifact_not_ready() -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_ARTIFACT_NOT_READY",
        ErrorCategory::Conflict,
        true,
        "The requested export artifact is not ready for disclosure.",
    )
}

fn stored_state_invalid() -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_ARTIFACT_STORED_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "Stored export artifact evidence is invalid.",
    )
}

fn configuration_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_ARTIFACT_DOWNLOAD_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The export artifact disclosure capability is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_capability_is_high_risk_read_only_and_job_bound() {
        let definition = artifact_download_capability_definition().unwrap();
        assert_eq!(
            definition.capability_id.as_str(),
            DOWNLOAD_EXPORT_ARTIFACT_CAPABILITY
        );
        assert!(!definition.mutation);
        assert!(!definition.requires_idempotency);
        assert_eq!(definition.risk, CapabilityRisk::High);
        assert!(definition.output_contract.is_none());
        let payload = artifact_download_request_payload("export-job-1").unwrap();
        let command = wire::DownloadPartyExportArtifactRequest::decode(payload.bytes.as_slice())
            .unwrap();
        assert_eq!(
            command.export_job_ref.unwrap().export_job_id,
            "export-job-1"
        );
    }

    #[test]
    fn sha256_decoder_is_exact() {
        assert_eq!(decode_sha256(&"11".repeat(32)).unwrap(), [0x11; 32]);
        assert!(decode_sha256("11").is_err());
    }
}
