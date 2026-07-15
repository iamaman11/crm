//! Immutable selection-boundary evidence for Party export.
//!
//! The boundary freezes the newest Party creation time eligible for one export job. Selection may
//! resume across process crashes, but it must always use the same cutoff and deterministic owner-side
//! ordering. The finalized manifest digest is additionally bound to this boundary so the same list
//! of Party references under a different cutoff is not the same export intent.

use crate::{
    ExportJobId, ExportSpecificationVersionId, PartyExportSelectionItem,
    party_export_selection_manifest_sha256,
};
use crm_module_sdk::{ErrorCategory, RecordId, SdkError};
use sha2::{Digest, Sha256};

pub const PARTY_EXPORT_SELECTION_BOUNDARY_VERSION_V1: &str =
    "party-export-selection-boundary/v1";

const SELECTION_BOUNDARY_ID_DOMAIN: &[u8] =
    b"crm.customer-data-operations.party-export-selection-boundary/v1";
const BOUNDED_MANIFEST_DIGEST_DOMAIN: &[u8] =
    b"crm.customer-data-operations.party-export-selection-manifest-bounded/v1";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PartyExportSelectionBoundaryId(RecordId);

impl PartyExportSelectionBoundaryId {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyExportSelectionBoundary {
    boundary_id: PartyExportSelectionBoundaryId,
    job_id: ExportJobId,
    export_specification_version_id: ExportSpecificationVersionId,
    selection_cutoff_unix_nanos: i64,
}

impl PartyExportSelectionBoundary {
    pub fn create(
        job_id: ExportJobId,
        export_specification_version_id: ExportSpecificationVersionId,
        selection_cutoff_unix_nanos: i64,
    ) -> Result<Self, SdkError> {
        if selection_cutoff_unix_nanos <= 0 {
            return Err(invalid(
                "CUSTOMER_DATA_EXPORT_SELECTION_CUTOFF_INVALID",
                "customer_data.export.selection_cutoff_unix_nanos",
                "selection cutoff must be positive Unix nanoseconds",
            ));
        }

        let mut hasher = Sha256::new();
        hasher.update(SELECTION_BOUNDARY_ID_DOMAIN);
        hash_part(&mut hasher, job_id.as_str().as_bytes());
        hash_part(
            &mut hasher,
            export_specification_version_id.as_str().as_bytes(),
        );
        hash_part(&mut hasher, &selection_cutoff_unix_nanos.to_be_bytes());

        let boundary_id = RecordId::try_new(format!(
            "cdo-export-boundary-{}",
            hex_digest(hasher.finalize())
        ))
        .map(PartyExportSelectionBoundaryId)
        .map_err(|error| {
            invalid(
                "CUSTOMER_DATA_EXPORT_SELECTION_BOUNDARY_ID_INVALID",
                "customer_data.export.selection_boundary_id",
                error.to_string(),
            )
        })?;

        Ok(Self {
            boundary_id,
            job_id,
            export_specification_version_id,
            selection_cutoff_unix_nanos,
        })
    }

    pub fn boundary_id(&self) -> &PartyExportSelectionBoundaryId {
        &self.boundary_id
    }

    pub fn job_id(&self) -> &ExportJobId {
        &self.job_id
    }

    pub fn export_specification_version_id(&self) -> &ExportSpecificationVersionId {
        &self.export_specification_version_id
    }

    pub const fn selection_cutoff_unix_nanos(&self) -> i64 {
        self.selection_cutoff_unix_nanos
    }

    /// Returns whether an authoritative Party creation timestamp belongs to this immutable export
    /// population. Party creation time is immutable owner state; updates after the cutoff do not
    /// change membership and are handled later by exact resource-version validation.
    pub const fn includes_party_created_at(&self, party_created_at_unix_nanos: i64) -> bool {
        party_created_at_unix_nanos >= 0
            && party_created_at_unix_nanos <= self.selection_cutoff_unix_nanos
    }
}

/// Produces the authoritative v1 manifest digest for a finalized Party export selection.
///
/// The existing manifest validator proves contiguous positions, one job identity and unique Party
/// references. This wrapper additionally binds the digest to the immutable selection boundary.
pub fn bounded_party_export_selection_manifest_sha256(
    boundary: &PartyExportSelectionBoundary,
    items: &[PartyExportSelectionItem],
) -> Result<String, SdkError> {
    let manifest_sha256 = party_export_selection_manifest_sha256(boundary.job_id(), items)?;

    let mut hasher = Sha256::new();
    hasher.update(BOUNDED_MANIFEST_DIGEST_DOMAIN);
    hash_part(
        &mut hasher,
        PARTY_EXPORT_SELECTION_BOUNDARY_VERSION_V1.as_bytes(),
    );
    hash_part(&mut hasher, boundary.boundary_id().as_str().as_bytes());
    hash_part(
        &mut hasher,
        boundary.export_specification_version_id().as_str().as_bytes(),
    );
    hash_part(
        &mut hasher,
        &boundary.selection_cutoff_unix_nanos().to_be_bytes(),
    );
    hash_part(&mut hasher, manifest_sha256.as_bytes());
    Ok(hex_digest(hasher.finalize()))
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
    use crate::{PartyExportSelectionItem, SelectedPartyId};

    fn specification_version(value: &str) -> ExportSpecificationVersionId {
        // Selection-boundary code must accept only the already-validated deterministic
        // specification identity. Tests obtain one through the public Party-export domain API.
        let specification = crate::PartyExportSpecification::try_new(
            crate::PartyExportScope::try_new(None, 10).unwrap(),
            crate::PartyExportProfile::v1(
                vec![crate::PartyExportField::PartyId],
                value,
            )
            .unwrap(),
        )
        .unwrap();
        specification.version_id().clone()
    }

    fn item(job_id: &ExportJobId, position: u32, party_id: &str) -> PartyExportSelectionItem {
        PartyExportSelectionItem::create(
            job_id.clone(),
            position,
            SelectedPartyId::try_new(party_id).unwrap(),
            1,
            10,
        )
        .unwrap()
    }

    #[test]
    fn boundary_identity_is_stable_for_exact_job_specification_and_cutoff() {
        let job_id = ExportJobId::try_new("selection-boundary-job").unwrap();
        let specification_version = specification_version("retention-a");
        let first = PartyExportSelectionBoundary::create(
            job_id.clone(),
            specification_version.clone(),
            100,
        )
        .unwrap();
        let replay = PartyExportSelectionBoundary::create(job_id, specification_version, 100).unwrap();
        assert_eq!(first, replay);
    }

    #[test]
    fn different_cutoff_or_specification_changes_boundary_identity() {
        let job_id = ExportJobId::try_new("selection-boundary-change-job").unwrap();
        let first = PartyExportSelectionBoundary::create(
            job_id.clone(),
            specification_version("retention-a"),
            100,
        )
        .unwrap();
        let different_cutoff = PartyExportSelectionBoundary::create(
            job_id.clone(),
            specification_version("retention-a"),
            101,
        )
        .unwrap();
        let different_specification = PartyExportSelectionBoundary::create(
            job_id,
            specification_version("retention-b"),
            100,
        )
        .unwrap();
        assert_ne!(first.boundary_id(), different_cutoff.boundary_id());
        assert_ne!(first.boundary_id(), different_specification.boundary_id());
    }

    #[test]
    fn cutoff_excludes_parties_created_after_selection_started() {
        let boundary = PartyExportSelectionBoundary::create(
            ExportJobId::try_new("selection-boundary-cutoff-job").unwrap(),
            specification_version("retention-a"),
            100,
        )
        .unwrap();
        assert!(boundary.includes_party_created_at(0));
        assert!(boundary.includes_party_created_at(100));
        assert!(!boundary.includes_party_created_at(101));
        assert!(!boundary.includes_party_created_at(-1));
    }

    #[test]
    fn finalized_manifest_digest_is_bound_to_selection_cutoff() {
        let job_id = ExportJobId::try_new("selection-boundary-digest-job").unwrap();
        let items = vec![item(&job_id, 1, "party-1"), item(&job_id, 2, "party-2")];
        let first = PartyExportSelectionBoundary::create(
            job_id.clone(),
            specification_version("retention-a"),
            100,
        )
        .unwrap();
        let second = PartyExportSelectionBoundary::create(
            job_id,
            specification_version("retention-a"),
            101,
        )
        .unwrap();
        assert_ne!(
            bounded_party_export_selection_manifest_sha256(&first, &items).unwrap(),
            bounded_party_export_selection_manifest_sha256(&second, &items).unwrap()
        );
    }

    #[test]
    fn rejects_non_positive_cutoff() {
        let error = PartyExportSelectionBoundary::create(
            ExportJobId::try_new("selection-boundary-invalid-job").unwrap(),
            specification_version("retention-a"),
            0,
        )
        .unwrap_err();
        assert_eq!(error.code, "CUSTOMER_DATA_EXPORT_SELECTION_CUTOFF_INVALID");
    }
}
