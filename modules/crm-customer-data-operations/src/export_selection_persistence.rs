use crate::{ExportJobId, PartyExportSelectionItem, SelectedPartyId};
use crm_module_sdk::{ErrorCategory, SdkError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const EXPORT_SELECTION_ITEM_STATE_SCHEMA_ID: &str =
    "crm.customer-data-operations.export_selection_item.state";
pub const EXPORT_SELECTION_ITEM_STATE_SCHEMA_VERSION: &str = "1.0.0";
pub const EXPORT_SELECTION_ITEM_STATE_MAXIMUM_BYTES: u64 = 16 * 1024;
pub const EXPORT_SELECTION_ITEM_STATE_RETENTION_POLICY_ID: &str =
    "crm.customer_data.export_selection_item";

const EXPORT_SELECTION_ITEM_STATE_DESCRIPTOR: &[u8] = b"crm.customer-data-operations.export_selection_item.state/v1:item_id,export_job_id,manifest_position,party_id,party_resource_version,created_at_unix_nanos,version";

pub fn export_selection_item_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(EXPORT_SELECTION_ITEM_STATE_DESCRIPTOR).into()
}

pub fn encode_export_selection_item_state(
    item: &PartyExportSelectionItem,
) -> Result<Vec<u8>, SdkError> {
    let bytes = serde_json::to_vec(&ExportSelectionItemStateV1::from(item)).map_err(|error| {
        persisted_error(format!(
            "export selection item serialization failed: {error}"
        ))
    })?;
    validate_size(&bytes)?;
    Ok(bytes)
}

pub fn decode_export_selection_item_state(
    bytes: &[u8],
) -> Result<PartyExportSelectionItem, SdkError> {
    validate_size(bytes)?;
    let state: ExportSelectionItemStateV1 = serde_json::from_slice(bytes).map_err(|error| {
        persisted_error(format!("export selection item JSON is invalid: {error}"))
    })?;
    let expected_item_id = state.item_id.clone();
    let expected_version = state.version;
    let item = PartyExportSelectionItem::create(
        ExportJobId::try_new(state.export_job_id)
            .map_err(|error| persisted_domain_error("export job ID", error))?,
        state.manifest_position,
        SelectedPartyId::try_new(state.party_id)
            .map_err(|error| persisted_domain_error("selected Party ID", error))?,
        state.party_resource_version,
        state.created_at_unix_nanos,
    )
    .map_err(|error| persisted_domain_error("export selection item", error))?;
    if item.item_id().as_str() != expected_item_id || item.version() != expected_version {
        return Err(persisted_error(
            "export selection item identity or version is inconsistent".to_owned(),
        ));
    }
    let canonical = encode_export_selection_item_state(&item)?;
    if canonical != bytes {
        return Err(persisted_error(
            "export selection item state is not the strict canonical v1 encoding".to_owned(),
        ));
    }
    Ok(item)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportSelectionItemStateV1 {
    item_id: String,
    export_job_id: String,
    manifest_position: u32,
    party_id: String,
    party_resource_version: i64,
    created_at_unix_nanos: i64,
    version: i64,
}

impl From<&PartyExportSelectionItem> for ExportSelectionItemStateV1 {
    fn from(item: &PartyExportSelectionItem) -> Self {
        Self {
            item_id: item.item_id().as_str().to_owned(),
            export_job_id: item.job_id().as_str().to_owned(),
            manifest_position: item.manifest_position(),
            party_id: item.party_id().as_str().to_owned(),
            party_resource_version: item.party_resource_version(),
            created_at_unix_nanos: item.created_at_unix_nanos(),
            version: item.version(),
        }
    }
}

fn validate_size(bytes: &[u8]) -> Result<(), SdkError> {
    if bytes.len() as u64 > EXPORT_SELECTION_ITEM_STATE_MAXIMUM_BYTES {
        return Err(persisted_error(format!(
            "export selection item state exceeds {EXPORT_SELECTION_ITEM_STATE_MAXIMUM_BYTES} bytes"
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
        "CUSTOMER_DATA_EXPORT_SELECTION_PERSISTED_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "Stored customer export selection state is invalid.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item() -> PartyExportSelectionItem {
        PartyExportSelectionItem::create(
            ExportJobId::try_new("export-selection-persistence-job").unwrap(),
            3,
            SelectedPartyId::try_new("party-selection-persistence").unwrap(),
            17,
            100,
        )
        .unwrap()
    }

    #[test]
    fn round_trips_canonical_immutable_selection_item() {
        let item = item();
        let encoded = encode_export_selection_item_state(&item).unwrap();
        let decoded = decode_export_selection_item_state(&encoded).unwrap();
        assert_eq!(decoded.item_id(), item.item_id());
        assert_eq!(decoded.job_id(), item.job_id());
        assert_eq!(decoded.manifest_position(), item.manifest_position());
        assert_eq!(decoded.party_id(), item.party_id());
        assert_eq!(decoded.party_resource_version(), 17);
        assert_eq!(
            encode_export_selection_item_state(&decoded).unwrap(),
            encoded
        );
    }

    #[test]
    fn rejects_unknown_fields_and_tampered_deterministic_identity() {
        let bytes = encode_export_selection_item_state(&item()).unwrap();
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["unknown"] = serde_json::json!(true);
        assert!(decode_export_selection_item_state(&serde_json::to_vec(&value).unwrap()).is_err());

        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["item_id"] = serde_json::json!("cdo-export-selection-tampered");
        assert!(decode_export_selection_item_state(&serde_json::to_vec(&value).unwrap()).is_err());
    }

    #[test]
    fn descriptor_hash_is_stable_and_non_zero() {
        assert_ne!(export_selection_item_state_descriptor_hash(), [0; 32]);
    }
}
