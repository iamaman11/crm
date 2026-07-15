use crate::{
    PartyExportJob, PartyExportJobStatus, PartyExportSelectionBoundary, PartyExportSelectionItem,
    PartyExportSelectionProgress, PartyExportSelectionSummary,
    bounded_party_export_selection_manifest_sha256,
};
use crm_module_sdk::{ErrorCategory, SdkError};

/// Proves the exact immutable selection evidence required before an export job may move from
/// `SELECTING` to `READY`.
///
/// The worker must load the authoritative job, its single immutable boundary, durable selection
/// progress and every deterministic manifest item `1..N`. Selection is terminal when the governed
/// Party source is exhausted or the immutable maximum-resource bound has been reached exactly.
/// This function rejects any mismatch before producing the only valid summary for the internal
/// selection-finalize capability.
pub fn prove_party_export_selection_finalization(
    job: &PartyExportJob,
    boundary: &PartyExportSelectionBoundary,
    progress: &PartyExportSelectionProgress,
    items: &[PartyExportSelectionItem],
) -> Result<PartyExportSelectionSummary, SdkError> {
    if job.status() != PartyExportJobStatus::Selecting
        || boundary.job_id() != job.job_id()
        || progress.job_id() != job.job_id()
        || boundary.export_specification_version_id().as_str()
            != job.specification().version_id().as_str()
        || progress.maximum_resources() != job.specification().scope().maximum_resources()
    {
        return Err(finalization_error());
    }

    let selected_resources = progress
        .next_manifest_position()
        .checked_sub(1)
        .ok_or_else(finalization_error)?;
    let source_exhausted = progress.source_exhausted() && progress.continuation().is_none();
    let maximum_reached = selected_resources == progress.maximum_resources();
    if !source_exhausted && !maximum_reached {
        return Err(finalization_error());
    }
    if usize::try_from(selected_resources).map_err(|_| finalization_error())? != items.len() {
        return Err(finalization_error());
    }

    let manifest_sha256 = bounded_party_export_selection_manifest_sha256(boundary, items)?;
    PartyExportSelectionSummary::try_new(
        manifest_sha256,
        selected_resources,
        progress.maximum_resources(),
    )
}

fn finalization_error() -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_SELECTION_FINALIZATION_INVALID",
        ErrorCategory::Conflict,
        false,
        "The customer export selection cannot be finalized from the supplied durable evidence.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ExportJobId, PartyExportField, PartyExportProfile, PartyExportScope,
        PartyExportSpecification, SelectedPartyId,
    };

    fn specification(maximum_resources: u32) -> PartyExportSpecification {
        PartyExportSpecification::try_new(
            PartyExportScope::try_new(None, maximum_resources).unwrap(),
            PartyExportProfile::v1(vec![PartyExportField::PartyId], "customer-export-30d").unwrap(),
        )
        .unwrap()
    }

    fn item(job_id: &ExportJobId, position: u32, party_id: &str) -> PartyExportSelectionItem {
        PartyExportSelectionItem::create(
            job_id.clone(),
            position,
            SelectedPartyId::try_new(party_id).unwrap(),
            1,
            20,
        )
        .unwrap()
    }

    fn selecting_evidence() -> (
        PartyExportJob,
        PartyExportSelectionBoundary,
        PartyExportSelectionProgress,
        Vec<PartyExportSelectionItem>,
    ) {
        let job_id = ExportJobId::try_new("selection-finalization-job").unwrap();
        let mut job = PartyExportJob::create(job_id.clone(), specification(10), 10).unwrap();
        job.start_or_resume(1, 11).unwrap();
        let boundary = PartyExportSelectionBoundary::create(
            job_id.clone(),
            job.specification().version_id().clone(),
            11,
        )
        .unwrap();
        let mut progress = PartyExportSelectionProgress::create(job_id.clone(), 10, 11).unwrap();
        progress.advance(1, 2, None, 20).unwrap();
        let items = vec![item(&job_id, 1, "party-1"), item(&job_id, 2, "party-2")];
        (job, boundary, progress, items)
    }

    #[test]
    fn proves_exact_terminal_selection_and_returns_boundary_bound_digest() {
        let (job, boundary, progress, items) = selecting_evidence();
        let summary =
            prove_party_export_selection_finalization(&job, &boundary, &progress, &items).unwrap();
        assert_eq!(summary.selected_resources(), 2);
        assert_eq!(
            summary.manifest_sha256(),
            bounded_party_export_selection_manifest_sha256(&boundary, &items).unwrap()
        );
    }

    #[test]
    fn maximum_bound_is_terminal_even_when_source_has_more_records() {
        let job_id = ExportJobId::try_new("selection-finalization-max-job").unwrap();
        let mut job = PartyExportJob::create(job_id.clone(), specification(2), 10).unwrap();
        job.start_or_resume(1, 11).unwrap();
        let boundary = PartyExportSelectionBoundary::create(
            job_id.clone(),
            job.specification().version_id().clone(),
            11,
        )
        .unwrap();
        let mut progress = PartyExportSelectionProgress::create(job_id.clone(), 2, 11).unwrap();
        progress
            .advance(
                1,
                2,
                Some(
                    crate::PartyExportSourceContinuation::try_new(
                        "100",
                        crm_module_sdk::RecordId::try_new("party-2").unwrap(),
                    )
                    .unwrap(),
                ),
                20,
            )
            .unwrap();
        let items = vec![item(&job_id, 1, "party-1"), item(&job_id, 2, "party-2")];
        assert!(
            prove_party_export_selection_finalization(&job, &boundary, &progress, &items).is_ok()
        );
    }

    #[test]
    fn rejects_non_terminal_progress_missing_items_and_wrong_boundary() {
        let (job, boundary, progress, items) = selecting_evidence();
        assert!(
            prove_party_export_selection_finalization(&job, &boundary, &progress, &items[..1])
                .is_err()
        );

        let mut non_terminal = PartyExportSelectionProgress::create(
            job.job_id().clone(),
            job.specification().scope().maximum_resources(),
            11,
        )
        .unwrap();
        non_terminal
            .advance(
                1,
                1,
                Some(
                    crate::PartyExportSourceContinuation::try_new(
                        "100",
                        crm_module_sdk::RecordId::try_new("party-1").unwrap(),
                    )
                    .unwrap(),
                ),
                20,
            )
            .unwrap();
        assert!(
            prove_party_export_selection_finalization(&job, &boundary, &non_terminal, &items)
                .is_err()
        );

        let wrong_boundary = PartyExportSelectionBoundary::create(
            ExportJobId::try_new("selection-finalization-other-job").unwrap(),
            job.specification().version_id().clone(),
            11,
        )
        .unwrap();
        assert!(
            prove_party_export_selection_finalization(&job, &wrong_boundary, &progress, &items)
                .is_err()
        );
    }
}
