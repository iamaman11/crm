use crm_capability_plan_support::{self as support, PersistedPayloadContract};
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest};
use crm_core_data::{CapabilityBatchExecutionPlan, RecordMutation};
use crm_customer_data_operations::{
    EXPORT_SELECTION_BOUNDARY_STATE_MAXIMUM_BYTES,
    EXPORT_SELECTION_BOUNDARY_STATE_RETENTION_POLICY_ID, EXPORT_SELECTION_BOUNDARY_STATE_SCHEMA_ID,
    EXPORT_SELECTION_BOUNDARY_STATE_SCHEMA_VERSION, EXPORT_SELECTION_PROGRESS_STATE_MAXIMUM_BYTES,
    EXPORT_SELECTION_PROGRESS_STATE_RETENTION_POLICY_ID, EXPORT_SELECTION_PROGRESS_STATE_SCHEMA_ID,
    EXPORT_SELECTION_PROGRESS_STATE_SCHEMA_VERSION, PartyExportJobStatus,
    PartyExportSelectionBoundary, PartyExportSelectionProgress,
    encode_export_selection_boundary_state, encode_export_selection_progress_state,
    export_selection_boundary_state_descriptor_hash,
    export_selection_progress_state_descriptor_hash,
};
use crm_module_sdk::{DataClass, RecordSnapshot, SdkError};

use crate::{MODULE_ID, START_PARTY_EXPORT_EXECUTION_CAPABILITY, export_job_from_snapshot};

pub const EXPORT_SELECTION_BOUNDARY_RECORD_TYPE: &str = "customer_data.export_selection_boundary";
pub const EXPORT_SELECTION_PROGRESS_RECORD_TYPE: &str = "customer_data.export_selection_progress";

/// Adds the immutable selection boundary and initial durable selection progress to the first public
/// Party-export execution start.
///
/// The underlying export-job planner already validates and plans the authoritative job transition.
/// This production hardening layer adds deterministic `Create` mutations for the one job-bound
/// boundary and progress records to the same batch only when the current job is still `Created`.
/// The job transition, cutoff and initial cursor therefore commit or roll back together.
pub fn harden_party_export_start_plan(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    current: Option<&RecordSnapshot>,
    mut plan: CapabilityBatchExecutionPlan,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    if definition.capability_id.as_str() != START_PARTY_EXPORT_EXECUTION_CAPABILITY {
        return Ok(plan);
    }

    let current = current.ok_or_else(|| {
        support::stored_data_error("CUSTOMER_DATA_EXPORT_SELECTION_BOUNDARY_JOB_MISSING")
    })?;
    let job = export_job_from_snapshot(current)?;
    if job.status() != PartyExportJobStatus::Created {
        return Ok(plan);
    }

    let occurred_at_unix_nanos = request.context.execution.request_started_at_unix_nanos;
    let boundary = PartyExportSelectionBoundary::create(
        job.job_id().clone(),
        job.specification().version_id().clone(),
        occurred_at_unix_nanos,
    )?;
    let progress =
        PartyExportSelectionProgress::create(job.job_id().clone(), occurred_at_unix_nanos)?;

    plan.batch.records.push(RecordMutation::Create {
        reference: export_selection_boundary_record_ref(&boundary)?,
        payload: export_selection_boundary_persisted_payload(&boundary)?,
    });
    plan.batch.records.push(RecordMutation::Create {
        reference: export_selection_progress_record_ref(&progress)?,
        payload: export_selection_progress_persisted_payload(&progress)?,
    });
    Ok(plan)
}

pub fn export_selection_boundary_persisted_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: EXPORT_SELECTION_BOUNDARY_STATE_SCHEMA_ID,
        schema_version: EXPORT_SELECTION_BOUNDARY_STATE_SCHEMA_VERSION,
        descriptor_hash: export_selection_boundary_state_descriptor_hash(),
        maximum_size_bytes: EXPORT_SELECTION_BOUNDARY_STATE_MAXIMUM_BYTES,
        retention_policy_id: EXPORT_SELECTION_BOUNDARY_STATE_RETENTION_POLICY_ID,
    }
}

pub fn export_selection_boundary_persisted_payload(
    boundary: &PartyExportSelectionBoundary,
) -> Result<crm_module_sdk::TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        export_selection_boundary_persisted_contract(),
        DataClass::Personal,
        encode_export_selection_boundary_state(boundary)?,
    )
}

pub fn export_selection_boundary_record_ref(
    boundary: &PartyExportSelectionBoundary,
) -> Result<crm_module_sdk::RecordRef, SdkError> {
    support::record_ref(
        EXPORT_SELECTION_BOUNDARY_RECORD_TYPE,
        boundary.boundary_id().as_str(),
        "customer_data.export.selection_boundary_id",
    )
}

pub fn export_selection_progress_persisted_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: EXPORT_SELECTION_PROGRESS_STATE_SCHEMA_ID,
        schema_version: EXPORT_SELECTION_PROGRESS_STATE_SCHEMA_VERSION,
        descriptor_hash: export_selection_progress_state_descriptor_hash(),
        maximum_size_bytes: EXPORT_SELECTION_PROGRESS_STATE_MAXIMUM_BYTES,
        retention_policy_id: EXPORT_SELECTION_PROGRESS_STATE_RETENTION_POLICY_ID,
    }
}

pub fn export_selection_progress_persisted_payload(
    progress: &PartyExportSelectionProgress,
) -> Result<crm_module_sdk::TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        export_selection_progress_persisted_contract(),
        DataClass::Personal,
        encode_export_selection_progress_state(progress)?,
    )
}

pub fn export_selection_progress_record_ref(
    progress: &PartyExportSelectionProgress,
) -> Result<crm_module_sdk::RecordRef, SdkError> {
    support::record_ref(
        EXPORT_SELECTION_PROGRESS_RECORD_TYPE,
        progress.progress_id().as_str(),
        "customer_data.export.selection_progress_id",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_customer_data_operations::{
        ExportJobId, PartyExportField, PartyExportProfile, PartyExportScope,
        PartyExportSpecification,
    };

    fn specification() -> PartyExportSpecification {
        PartyExportSpecification::try_new(
            PartyExportScope::try_new(None, 10).unwrap(),
            PartyExportProfile::v1(vec![PartyExportField::PartyId], "customer-export-30d").unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn publishes_strict_personal_boundary_and_progress_persistence_contracts() {
        let job_id = ExportJobId::try_new("export-boundary-planner-job").unwrap();
        let specification = specification();
        let boundary = PartyExportSelectionBoundary::create(
            job_id.clone(),
            specification.version_id().clone(),
            100,
        )
        .unwrap();
        let progress = PartyExportSelectionProgress::create(job_id, 100).unwrap();

        let boundary_reference = export_selection_boundary_record_ref(&boundary).unwrap();
        let boundary_payload = export_selection_boundary_persisted_payload(&boundary).unwrap();
        assert_eq!(
            boundary_reference.record_type.as_str(),
            EXPORT_SELECTION_BOUNDARY_RECORD_TYPE
        );
        assert_eq!(
            boundary_reference.record_id.as_str(),
            boundary.boundary_id().as_str()
        );
        assert_eq!(boundary_payload.data_class, DataClass::Personal);
        assert_eq!(
            export_selection_boundary_persisted_contract().schema_id,
            EXPORT_SELECTION_BOUNDARY_STATE_SCHEMA_ID
        );

        let progress_reference = export_selection_progress_record_ref(&progress).unwrap();
        let progress_payload = export_selection_progress_persisted_payload(&progress).unwrap();
        assert_eq!(
            progress_reference.record_type.as_str(),
            EXPORT_SELECTION_PROGRESS_RECORD_TYPE
        );
        assert_eq!(
            progress_reference.record_id.as_str(),
            progress.progress_id().as_str()
        );
        assert_eq!(progress_payload.data_class, DataClass::Personal);
        assert_eq!(
            export_selection_progress_persisted_contract().schema_id,
            EXPORT_SELECTION_PROGRESS_STATE_SCHEMA_ID
        );
    }
}
