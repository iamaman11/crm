use crm_core_data::{PostgresDataStore, RecordGetQuery};
use crm_customer_data_operations::{
    ExportJobId, PartyExportExclusionReason, PartyExportExecutionOutcome,
    PartyExportExecutionStage, PartyExportExecutionStageId,
};
use crm_customer_data_operations_capability_adapter::{
    EXPORT_EXECUTION_OUTCOME_RECORD_TYPE, EXPORT_EXECUTION_STAGE_RECORD_TYPE, MODULE_ID,
    export_execution_outcome_from_snapshot, export_execution_stage_from_snapshot,
};
use crm_module_sdk::{
    ErrorCategory, ModuleId, PortFuture, RecordId, RecordType, SdkError, TenantId,
};

#[derive(Debug, Clone)]
pub struct PostgresPartyExportExecutionReader {
    store: PostgresDataStore,
}

impl PostgresPartyExportExecutionReader {
    pub fn new(store: PostgresDataStore) -> Self {
        Self { store }
    }

    pub fn load_stage<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        job_id: &'a ExportJobId,
        manifest_position: u32,
    ) -> PortFuture<'a, Result<Option<PartyExportExecutionStage>, SdkError>> {
        Box::pin(async move {
            let stage_id =
                PartyExportExecutionStageId::for_job_position(job_id, manifest_position)?;
            let snapshot = self
                .store
                .get_record_for_query(&RecordGetQuery {
                    tenant_id: tenant_id.clone(),
                    owner_module_id: module_id()?,
                    record_type: RecordType::try_new(EXPORT_EXECUTION_STAGE_RECORD_TYPE)
                        .map_err(configuration_error)?,
                    record_id: RecordId::try_new(stage_id.as_str()).map_err(configuration_error)?,
                })
                .await?;
            snapshot
                .as_ref()
                .map(export_execution_stage_from_snapshot)
                .transpose()
        })
    }

    pub fn load_outcome<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        job_id: &'a ExportJobId,
        manifest_position: u32,
    ) -> PortFuture<'a, Result<Option<PartyExportExecutionOutcome>, SdkError>> {
        Box::pin(async move {
            let outcome_id = PartyExportExecutionOutcome::excluded(
                job_id.clone(),
                manifest_position,
                PartyExportExclusionReason::Unavailable,
                1,
            )?
            .outcome_id()
            .clone();
            let snapshot = self
                .store
                .get_record_for_query(&RecordGetQuery {
                    tenant_id: tenant_id.clone(),
                    owner_module_id: module_id()?,
                    record_type: RecordType::try_new(EXPORT_EXECUTION_OUTCOME_RECORD_TYPE)
                        .map_err(configuration_error)?,
                    record_id: RecordId::try_new(outcome_id.as_str())
                        .map_err(configuration_error)?,
                })
                .await?;
            snapshot
                .as_ref()
                .map(export_execution_outcome_from_snapshot)
                .transpose()
        })
    }

    pub fn load_outcomes<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        job_id: &'a ExportJobId,
        selected_resources: u32,
    ) -> PortFuture<'a, Result<Vec<PartyExportExecutionOutcome>, SdkError>> {
        Box::pin(async move {
            let mut outcomes = Vec::with_capacity(
                usize::try_from(selected_resources).map_err(|_| execution_evidence_invalid())?,
            );
            for position in 1..=selected_resources {
                let outcome = self
                    .load_outcome(tenant_id, job_id, position)
                    .await?
                    .ok_or_else(execution_evidence_missing)?;
                if outcome.job_id() != job_id || outcome.manifest_position() != position {
                    return Err(execution_evidence_invalid());
                }
                outcomes.push(outcome);
            }
            Ok(outcomes)
        })
    }
}

fn module_id() -> Result<ModuleId, SdkError> {
    ModuleId::try_new(MODULE_ID).map_err(configuration_error)
}

fn execution_evidence_missing() -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_EXECUTION_EVIDENCE_MISSING",
        ErrorCategory::Unavailable,
        true,
        "Required customer export execution evidence is temporarily unavailable.",
    )
}

fn execution_evidence_invalid() -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_EXECUTION_EVIDENCE_INVALID",
        ErrorCategory::Internal,
        false,
        "Stored customer export execution evidence is invalid.",
    )
}

fn configuration_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_EXECUTION_READER_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The customer export execution reader is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_customer_data_operations_capability_adapter::{
        export_execution_outcome_persisted_contract, export_execution_stage_persisted_contract,
    };
    use crm_module_sdk::DataClass;

    #[test]
    fn exact_stage_and_outcome_record_contracts_are_personal() {
        assert_eq!(
            export_execution_stage_persisted_contract().data_class,
            DataClass::Personal
        );
        assert_eq!(
            export_execution_outcome_persisted_contract().data_class,
            DataClass::Personal
        );
    }
}
