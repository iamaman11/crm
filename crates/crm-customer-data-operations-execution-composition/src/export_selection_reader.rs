use crm_capability_plan_support as support;
use crm_core_data::{PostgresDataStore, RecordGetQuery};
use crm_customer_data_operations::{
    PartyExportJob, PartyExportSelectionBoundary, PartyExportSelectionBoundaryId,
    PartyExportSelectionItem, PartyExportSelectionItemId, PartyExportSelectionProgress,
    PartyExportSelectionProgressId, decode_export_selection_boundary_state,
    decode_export_selection_item_state, decode_export_selection_progress_state,
    prove_party_export_selection_finalization,
};
use crm_customer_data_operations_capability_adapter::{
    EXPORT_SELECTION_BOUNDARY_RECORD_TYPE, EXPORT_SELECTION_PROGRESS_RECORD_TYPE, MODULE_ID,
    export_selection_boundary_persisted_contract, export_selection_item_persisted_contract,
    export_selection_progress_persisted_contract,
};
use crm_module_sdk::{
    DataClass, ErrorCategory, ModuleId, PortFuture, RecordId, RecordType, SdkError, TenantId,
};

const EXPORT_SELECTION_ITEM_RECORD_TYPE: &str = "customer_data.export_selection_item";

#[derive(Debug, Clone)]
pub struct PartyExportSelectionEvidence {
    pub boundary: PartyExportSelectionBoundary,
    pub progress: PartyExportSelectionProgress,
}

#[derive(Debug, Clone)]
pub struct PostgresPartyExportSelectionReader {
    store: PostgresDataStore,
}

impl PostgresPartyExportSelectionReader {
    pub fn new(store: PostgresDataStore) -> Self {
        Self { store }
    }

    pub fn load_evidence<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        job: &'a PartyExportJob,
    ) -> PortFuture<'a, Result<PartyExportSelectionEvidence, SdkError>> {
        Box::pin(async move {
            let owner_module_id = module_id()?;
            let boundary_id = PartyExportSelectionBoundaryId::for_job(job.job_id())?;
            let boundary_snapshot = self
                .store
                .get_record_for_query(&RecordGetQuery {
                    tenant_id: tenant_id.clone(),
                    owner_module_id: owner_module_id.clone(),
                    record_type: RecordType::try_new(EXPORT_SELECTION_BOUNDARY_RECORD_TYPE)
                        .map_err(configuration_error)?,
                    record_id: RecordId::try_new(boundary_id.as_str())
                        .map_err(configuration_error)?,
                })
                .await?
                .ok_or_else(selection_evidence_missing)?;
            let boundary = decode_export_selection_boundary_state(
                support::persisted_json_bytes_with_data_class(
                    &boundary_snapshot,
                    export_selection_boundary_persisted_contract(),
                    DataClass::Personal,
                )?,
                job.job_id(),
                job.specification().version_id(),
            )?;

            let progress_id = PartyExportSelectionProgressId::for_job(job.job_id())?;
            let progress_snapshot = self
                .store
                .get_record_for_query(&RecordGetQuery {
                    tenant_id: tenant_id.clone(),
                    owner_module_id,
                    record_type: RecordType::try_new(EXPORT_SELECTION_PROGRESS_RECORD_TYPE)
                        .map_err(configuration_error)?,
                    record_id: RecordId::try_new(progress_id.as_str())
                        .map_err(configuration_error)?,
                })
                .await?
                .ok_or_else(selection_evidence_missing)?;
            let progress = decode_export_selection_progress_state(
                support::persisted_json_bytes_with_data_class(
                    &progress_snapshot,
                    export_selection_progress_persisted_contract(),
                    DataClass::Personal,
                )?,
            )?;

            if boundary.job_id() != job.job_id()
                || boundary.export_specification_version_id().as_str()
                    != job.specification().version_id().as_str()
                || progress.job_id() != job.job_id()
                || progress.maximum_resources() != job.specification().scope().maximum_resources()
                || progress.version() != progress_snapshot.version
            {
                return Err(selection_evidence_invalid());
            }

            Ok(PartyExportSelectionEvidence { boundary, progress })
        })
    }

    pub fn load_manifest<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        job: &'a PartyExportJob,
        progress: &'a PartyExportSelectionProgress,
    ) -> PortFuture<'a, Result<Vec<PartyExportSelectionItem>, SdkError>> {
        Box::pin(async move {
            let selected_resources = progress
                .next_manifest_position()
                .checked_sub(1)
                .ok_or_else(selection_evidence_invalid)?;
            if selected_resources > progress.maximum_resources() {
                return Err(selection_evidence_invalid());
            }

            let owner_module_id = module_id()?;
            let record_type = RecordType::try_new(EXPORT_SELECTION_ITEM_RECORD_TYPE)
                .map_err(configuration_error)?;
            let mut items = Vec::with_capacity(
                usize::try_from(selected_resources).map_err(|_| selection_evidence_invalid())?,
            );
            for position in 1..=selected_resources {
                let item_id = PartyExportSelectionItemId::for_job_position(job.job_id(), position)?;
                let snapshot = self
                    .store
                    .get_record_for_query(&RecordGetQuery {
                        tenant_id: tenant_id.clone(),
                        owner_module_id: owner_module_id.clone(),
                        record_type: record_type.clone(),
                        record_id: RecordId::try_new(item_id.as_str())
                            .map_err(configuration_error)?,
                    })
                    .await?
                    .ok_or_else(selection_evidence_missing)?;
                let item = decode_export_selection_item_state(
                    support::persisted_json_bytes_with_data_class(
                        &snapshot,
                        export_selection_item_persisted_contract(),
                        DataClass::Personal,
                    )?,
                )?;
                if item.item_id() != &item_id
                    || item.job_id() != job.job_id()
                    || item.manifest_position() != position
                    || item.version() != snapshot.version
                {
                    return Err(selection_evidence_invalid());
                }
                items.push(item);
            }
            Ok(items)
        })
    }

    pub fn prove_finalization<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        job: &'a PartyExportJob,
        evidence: &'a PartyExportSelectionEvidence,
    ) -> PortFuture<'a, Result<crm_customer_data_operations::PartyExportSelectionSummary, SdkError>>
    {
        Box::pin(async move {
            let items = self
                .load_manifest(tenant_id, job, &evidence.progress)
                .await?;
            prove_party_export_selection_finalization(
                job,
                &evidence.boundary,
                &evidence.progress,
                &items,
            )
        })
    }
}

fn module_id() -> Result<ModuleId, SdkError> {
    ModuleId::try_new(MODULE_ID).map_err(configuration_error)
}

fn selection_evidence_missing() -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_SELECTION_EVIDENCE_MISSING",
        ErrorCategory::Unavailable,
        true,
        "Required customer export selection evidence is temporarily unavailable.",
    )
}

fn selection_evidence_invalid() -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_SELECTION_EVIDENCE_INVALID",
        ErrorCategory::Internal,
        false,
        "Stored customer export selection evidence is invalid.",
    )
}

fn configuration_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_SELECTION_READER_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The customer export selection reader is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_customer_data_operations::ExportJobId;

    #[test]
    fn deterministic_progress_and_manifest_ids_are_job_bound() {
        let job_id = ExportJobId::try_new("selection-reader-id-test").unwrap();
        let progress_id = PartyExportSelectionProgressId::for_job(&job_id).unwrap();
        let first_item = PartyExportSelectionItemId::for_job_position(&job_id, 1).unwrap();
        let second_item = PartyExportSelectionItemId::for_job_position(&job_id, 2).unwrap();
        assert_ne!(progress_id.as_str(), first_item.as_str());
        assert_ne!(first_item, second_item);
    }
}
