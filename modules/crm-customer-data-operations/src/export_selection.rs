use crate::export::ExportJobId;
use crm_module_sdk::{ErrorCategory, RecordId, SdkError};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

const MAX_PARTY_EXPORT_SELECTION_ITEMS: usize = 100_000;
const SELECTION_ITEM_ID_DOMAIN: &[u8] =
    b"crm.customer-data-operations.party-export-selection-item/v1";
const SELECTION_MANIFEST_DIGEST_DOMAIN: &[u8] =
    b"crm.customer-data-operations.party-export-selection-manifest/v1";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PartyExportSelectionItemId(RecordId);

impl PartyExportSelectionItemId {
    fn derive(job_id: &ExportJobId, manifest_position: u32) -> Result<Self, SdkError> {
        let mut hasher = Sha256::new();
        hasher.update(SELECTION_ITEM_ID_DOMAIN);
        hash_part(&mut hasher, job_id.as_str().as_bytes());
        hash_part(&mut hasher, &manifest_position.to_be_bytes());
        RecordId::try_new(format!(
            "cdo-export-selection-{}",
            hex_digest(hasher.finalize())
        ))
        .map(Self)
        .map_err(|error| {
            invalid(
                "CUSTOMER_DATA_EXPORT_SELECTION_ITEM_ID_INVALID",
                "customer_data.export.selection_item_id",
                error.to_string(),
            )
        })
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SelectedPartyId(RecordId);

impl SelectedPartyId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, SdkError> {
        RecordId::try_new(value.into()).map(Self).map_err(|error| {
            invalid(
                "CUSTOMER_DATA_EXPORT_SELECTED_PARTY_ID_INVALID",
                "customer_data.export.selection.party_id",
                error.to_string(),
            )
        })
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyExportSelectionItem {
    item_id: PartyExportSelectionItemId,
    job_id: ExportJobId,
    manifest_position: u32,
    party_id: SelectedPartyId,
    party_resource_version: i64,
    created_at_unix_nanos: i64,
    version: i64,
}

impl PartyExportSelectionItem {
    pub fn create(
        job_id: ExportJobId,
        manifest_position: u32,
        party_id: SelectedPartyId,
        party_resource_version: i64,
        occurred_at_unix_nanos: i64,
    ) -> Result<Self, SdkError> {
        validate_manifest_position(manifest_position)?;
        if party_resource_version <= 0 {
            return Err(invalid(
                "CUSTOMER_DATA_EXPORT_SELECTED_PARTY_VERSION_INVALID",
                "customer_data.export.selection.party_resource_version",
                "selected Party resource version must be positive",
            ));
        }
        if occurred_at_unix_nanos <= 0 {
            return Err(invalid(
                "CUSTOMER_DATA_EXPORT_SELECTION_TIME_INVALID",
                "customer_data.export.selection.created_at",
                "selection item timestamp must be positive Unix nanoseconds",
            ));
        }
        let item_id = PartyExportSelectionItemId::derive(&job_id, manifest_position)?;
        Ok(Self {
            item_id,
            job_id,
            manifest_position,
            party_id,
            party_resource_version,
            created_at_unix_nanos: occurred_at_unix_nanos,
            version: 1,
        })
    }

    pub fn item_id(&self) -> &PartyExportSelectionItemId {
        &self.item_id
    }

    pub fn job_id(&self) -> &ExportJobId {
        &self.job_id
    }

    pub const fn manifest_position(&self) -> u32 {
        self.manifest_position
    }

    pub fn party_id(&self) -> &SelectedPartyId {
        &self.party_id
    }

    pub const fn party_resource_version(&self) -> i64 {
        self.party_resource_version
    }

    pub const fn created_at_unix_nanos(&self) -> i64 {
        self.created_at_unix_nanos
    }

    pub const fn version(&self) -> i64 {
        self.version
    }
}

pub fn party_export_selection_manifest_sha256(
    job_id: &ExportJobId,
    items: &[PartyExportSelectionItem],
) -> Result<String, SdkError> {
    if items.len() > MAX_PARTY_EXPORT_SELECTION_ITEMS {
        return Err(invalid(
            "CUSTOMER_DATA_EXPORT_SELECTION_LIMIT_EXCEEDED",
            "customer_data.export.selection.items",
            format!(
                "selection manifest cannot contain more than {MAX_PARTY_EXPORT_SELECTION_ITEMS} items"
            ),
        ));
    }

    let mut seen_parties = BTreeSet::new();
    let mut hasher = Sha256::new();
    hasher.update(SELECTION_MANIFEST_DIGEST_DOMAIN);
    hash_part(&mut hasher, job_id.as_str().as_bytes());
    hash_part(&mut hasher, &(items.len() as u64).to_be_bytes());

    for (index, item) in items.iter().enumerate() {
        let expected_position = u32::try_from(index + 1).map_err(|_| manifest_error())?;
        if item.job_id() != job_id || item.manifest_position() != expected_position {
            return Err(manifest_error());
        }
        let expected_item_id = PartyExportSelectionItemId::derive(job_id, expected_position)?;
        if item.item_id() != &expected_item_id || !seen_parties.insert(item.party_id().as_str()) {
            return Err(manifest_error());
        }
        hash_part(&mut hasher, &expected_position.to_be_bytes());
        hash_part(&mut hasher, item.party_id().as_str().as_bytes());
        hash_part(&mut hasher, &item.party_resource_version().to_be_bytes());
    }

    Ok(hex_digest(hasher.finalize()))
}

fn validate_manifest_position(value: u32) -> Result<(), SdkError> {
    if value == 0 || value as usize > MAX_PARTY_EXPORT_SELECTION_ITEMS {
        return Err(invalid(
            "CUSTOMER_DATA_EXPORT_SELECTION_POSITION_INVALID",
            "customer_data.export.selection.manifest_position",
            format!("manifest position must be between 1 and {MAX_PARTY_EXPORT_SELECTION_ITEMS}"),
        ));
    }
    Ok(())
}

fn manifest_error() -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_SELECTION_MANIFEST_INVALID",
        ErrorCategory::Conflict,
        false,
        "The Party export selection manifest is not contiguous, deterministic and unique.",
    )
}

fn hash_part(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}

fn hex_digest(bytes: impl AsRef<[u8]>) -> String {
    let bytes = bytes.as_ref();
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn invalid(code: &'static str, field: &'static str, message: impl Into<String>) -> SdkError {
    let mut error = SdkError::invalid_argument(field, message.into());
    error.code = code.to_owned();
    error
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(
        job_id: &ExportJobId,
        position: u32,
        party_id: &str,
        version: i64,
    ) -> PartyExportSelectionItem {
        PartyExportSelectionItem::create(
            job_id.clone(),
            position,
            SelectedPartyId::try_new(party_id).unwrap(),
            version,
            10,
        )
        .unwrap()
    }

    #[test]
    fn item_identity_is_deterministic_for_job_and_position() {
        let job_id = ExportJobId::try_new("export-selection-job").unwrap();
        let first = item(&job_id, 1, "party-1", 7);
        let replay = item(&job_id, 1, "party-1", 7);
        assert_eq!(first.item_id(), replay.item_id());
        assert_eq!(first.version(), 1);
    }

    #[test]
    fn manifest_digest_is_stable_for_exact_party_refs_and_versions() {
        let job_id = ExportJobId::try_new("export-selection-job-digest").unwrap();
        let items = vec![
            item(&job_id, 1, "party-1", 7),
            item(&job_id, 2, "party-2", 9),
        ];
        assert_eq!(
            party_export_selection_manifest_sha256(&job_id, &items).unwrap(),
            party_export_selection_manifest_sha256(&job_id, &items).unwrap()
        );
    }

    #[test]
    fn manifest_digest_rejects_gaps_cross_job_items_and_duplicate_parties() {
        let job_id = ExportJobId::try_new("export-selection-job-invalid").unwrap();
        let other_job = ExportJobId::try_new("export-selection-job-other").unwrap();
        assert!(
            party_export_selection_manifest_sha256(&job_id, &[item(&job_id, 2, "party-2", 1)])
                .is_err()
        );
        assert!(
            party_export_selection_manifest_sha256(&job_id, &[item(&other_job, 1, "party-1", 1)])
                .is_err()
        );
        assert!(
            party_export_selection_manifest_sha256(
                &job_id,
                &[
                    item(&job_id, 1, "party-1", 1),
                    item(&job_id, 2, "party-1", 1),
                ]
            )
            .is_err()
        );
    }

    #[test]
    fn empty_manifest_has_stable_job_bound_digest() {
        let first = ExportJobId::try_new("export-selection-empty-a").unwrap();
        let second = ExportJobId::try_new("export-selection-empty-b").unwrap();
        assert_ne!(
            party_export_selection_manifest_sha256(&first, &[]).unwrap(),
            party_export_selection_manifest_sha256(&second, &[]).unwrap()
        );
    }
}
