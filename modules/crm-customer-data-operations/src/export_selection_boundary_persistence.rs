use crate::{
    ExportJobId, ExportSpecificationVersionId, PartyExportSelectionBoundary,
};
use crm_module_sdk::{ErrorCategory, SdkError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const EXPORT_SELECTION_BOUNDARY_STATE_SCHEMA_ID: &str =
    "crm.customer-data-operations.export_selection_boundary.state";
pub const EXPORT_SELECTION_BOUNDARY_STATE_SCHEMA_VERSION: &str = "1.0.0";
pub const EXPORT_SELECTION_BOUNDARY_STATE_MAXIMUM_BYTES: u64 = 8 * 1024;
pub const EXPORT_SELECTION_BOUNDARY_STATE_RETENTION_POLICY_ID: &str =
    "crm.customer_data.export_selection_boundary";

const EXPORT_SELECTION_BOUNDARY_STATE_DESCRIPTOR: &[u8] = b"crm.customer-data-operations.export_selection_boundary.state/v1:boundary_id,export_job_id,export_specification_version_id,selection_cutoff_unix_nanos,version";
const PERSISTED_STATE_VERSION: u32 = 1;

pub fn export_selection_boundary_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(EXPORT_SELECTION_BOUNDARY_STATE_DESCRIPTOR).into()
}

pub fn encode_export_selection_boundary_state(
    boundary: &PartyExportSelectionBoundary,
) -> Result<Vec<u8>, SdkError> {
    let bytes = serde_json::to_vec(&ExportSelectionBoundaryStateV1::from(boundary)).map_err(
        |error| {
            persisted_error(format!(
                "export selection boundary serialization failed: {error}"
            ))
        },
    )?;
    validate_size(&bytes)?;
    Ok(bytes)
}

/// Decodes immutable boundary state against the authoritative job/specification context.
///
/// `ExportSpecificationVersionId` is deliberately not reconstructible from arbitrary persisted text;
/// callers must provide the already-validated immutable specification identity from the export job.
pub fn decode_export_selection_boundary_state(
    bytes: &[u8],
    expected_job_id: &ExportJobId,
    expected_specification_version_id: &ExportSpecificationVersionId,
) -> Result<PartyExportSelectionBoundary, SdkError> {
    validate_size(bytes)?;
    let state: ExportSelectionBoundaryStateV1 = serde_json::from_slice(bytes).map_err(|error| {
        persisted_error(format!("export selection boundary JSON is invalid: {error}"))
    })?;
    if state.version != PERSISTED_STATE_VERSION
        || state.export_job_id != expected_job_id.as_str()
        || state.export_specification_version_id != expected_specification_version_id.as_str()
    {
        return Err(persisted_error(
            "export selection boundary job/specification identity is inconsistent".to_owned(),
        ));
    }

    let boundary = PartyExportSelectionBoundary::create(
        expected_job_id.clone(),
        expected_specification_version_id.clone(),
        state.selection_cutoff_unix_nanos,
    )
    .map_err(|error| persisted_domain_error("export selection boundary", error))?;
    if boundary.boundary_id().as_str() != state.boundary_id {
        return Err(persisted_error(
            "export selection boundary deterministic identity is inconsistent".to_owned(),
        ));
    }

    let canonical = encode_export_selection_boundary_state(&boundary)?;
    if canonical != bytes {
        return Err(persisted_error(
            "export selection boundary state is not the strict canonical v1 encoding".to_owned(),
        ));
    }
    Ok(boundary)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportSelectionBoundaryStateV1 {
    boundary_id: String,
    export_job_id: String,
    export_specification_version_id: String,
    selection_cutoff_unix_nanos: i64,
    version: u32,
}

impl From<&PartyExportSelectionBoundary> for ExportSelectionBoundaryStateV1 {
    fn from(boundary: &PartyExportSelectionBoundary) -> Self {
        Self {
            boundary_id: boundary.boundary_id().as_str().to_owned(),
            export_job_id: boundary.job_id().as_str().to_owned(),
            export_specification_version_id: boundary
                .export_specification_version_id()
                .as_str()
                .to_owned(),
            selection_cutoff_unix_nanos: boundary.selection_cutoff_unix_nanos(),
            version: PERSISTED_STATE_VERSION,
        }
    }
}

fn validate_size(bytes: &[u8]) -> Result<(), SdkError> {
    if bytes.len() as u64 > EXPORT_SELECTION_BOUNDARY_STATE_MAXIMUM_BYTES {
        return Err(persisted_error(format!(
            "export selection boundary state exceeds {EXPORT_SELECTION_BOUNDARY_STATE_MAXIMUM_BYTES} bytes"
        )));
    }
    Ok(())
}

fn persisted_domain_error(context: &str, error: SdkError) -> SdkError {
    persisted_error(format!("{context}: {}: {}", error.code, error.safe_message))
}

fn persisted_error(detail: String) -> SdkError {
    let _ = detail;
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_SELECTION_BOUNDARY_PERSISTED_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "Stored customer export selection boundary state is invalid.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PartyExportField, PartyExportProfile, PartyExportScope, PartyExportSpecification};

    fn specification() -> PartyExportSpecification {
        PartyExportSpecification::try_new(
            PartyExportScope::try_new(None, 10).unwrap(),
            PartyExportProfile::v1(vec![PartyExportField::PartyId], "customer-export-30d").unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn round_trips_against_authoritative_job_and_specification_identity() {
        let job_id = ExportJobId::try_new("export-boundary-persistence-job").unwrap();
        let specification = specification();
        let boundary = PartyExportSelectionBoundary::create(
            job_id.clone(),
            specification.version_id().clone(),
            100,
        )
        .unwrap();
        let bytes = encode_export_selection_boundary_state(&boundary).unwrap();
        let decoded = decode_export_selection_boundary_state(
            &bytes,
            &job_id,
            specification.version_id(),
        )
        .unwrap();
        assert_eq!(decoded, boundary);
        assert_eq!(encode_export_selection_boundary_state(&decoded).unwrap(), bytes);
    }

    #[test]
    fn rejects_unknown_fields_tampered_identity_and_wrong_authoritative_context() {
        let job_id = ExportJobId::try_new("export-boundary-persistence-job").unwrap();
        let specification = specification();
        let boundary = PartyExportSelectionBoundary::create(
            job_id.clone(),
            specification.version_id().clone(),
            100,
        )
        .unwrap();
        let bytes = encode_export_selection_boundary_state(&boundary).unwrap();

        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["unknown"] = serde_json::json!(true);
        assert!(
            decode_export_selection_boundary_state(
                &serde_json::to_vec(&value).unwrap(),
                &job_id,
                specification.version_id(),
            )
            .is_err()
        );

        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["boundary_id"] = serde_json::json!("cdo-export-boundary-tampered");
        assert!(
            decode_export_selection_boundary_state(
                &serde_json::to_vec(&value).unwrap(),
                &job_id,
                specification.version_id(),
            )
            .is_err()
        );

        let other_job = ExportJobId::try_new("export-boundary-other-job").unwrap();
        assert!(
            decode_export_selection_boundary_state(
                &bytes,
                &other_job,
                specification.version_id(),
            )
            .is_err()
        );
    }

    #[test]
    fn descriptor_hash_is_stable_and_non_zero() {
        assert_ne!(export_selection_boundary_state_descriptor_hash(), [0; 32]);
    }
}
