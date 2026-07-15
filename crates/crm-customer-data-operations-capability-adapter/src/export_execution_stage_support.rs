use crm_capability_plan_support::{self as support, PersistedPayloadContract};
use crm_customer_data_operations::{
    EXPORT_EXECUTION_STAGE_STATE_MAXIMUM_BYTES, EXPORT_EXECUTION_STAGE_STATE_RETENTION_POLICY_ID,
    EXPORT_EXECUTION_STAGE_STATE_SCHEMA_ID, EXPORT_EXECUTION_STAGE_STATE_SCHEMA_VERSION, ExportJobId,
    PartyExportExecutionStage, PartyExportExecutionStageId, decode_export_execution_stage_state,
    encode_export_execution_stage_state, export_execution_stage_state_descriptor_hash,
};
use crm_module_sdk::{DataClass, RecordSnapshot, SdkError};

use crate::MODULE_ID;

pub const EXPORT_EXECUTION_STAGE_RECORD_TYPE: &str = "customer_data.export_execution_stage";

pub fn export_execution_stage_persisted_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: EXPORT_EXECUTION_STAGE_STATE_SCHEMA_ID,
        schema_version: EXPORT_EXECUTION_STAGE_STATE_SCHEMA_VERSION,
        descriptor_hash: export_execution_stage_state_descriptor_hash(),
        maximum_size_bytes: EXPORT_EXECUTION_STAGE_STATE_MAXIMUM_BYTES,
        retention_policy_id: EXPORT_EXECUTION_STAGE_STATE_RETENTION_POLICY_ID,
    }
}

pub fn export_execution_stage_persisted_payload(
    stage: &PartyExportExecutionStage,
) -> Result<crm_module_sdk::TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        export_execution_stage_persisted_contract(),
        DataClass::Personal,
        encode_export_execution_stage_state(stage)?,
    )
}

pub fn export_execution_stage_record_ref(
    stage: &PartyExportExecutionStage,
) -> Result<crm_module_sdk::RecordRef, SdkError> {
    export_execution_stage_record_ref_for_job_position(stage.job_id(), stage.manifest_position())
}

pub fn export_execution_stage_record_ref_for_job_position(
    job_id: &ExportJobId,
    manifest_position: u32,
) -> Result<crm_module_sdk::RecordRef, SdkError> {
    let stage_id = PartyExportExecutionStageId::for_job_position(job_id, manifest_position)?;
    support::record_ref(
        EXPORT_EXECUTION_STAGE_RECORD_TYPE,
        stage_id.as_str(),
        "customer_data.export.execution_stage_id",
    )
}

pub fn export_execution_stage_from_snapshot(
    snapshot: &RecordSnapshot,
) -> Result<PartyExportExecutionStage, SdkError> {
    let stage = decode_export_execution_stage_state(support::persisted_json_bytes_with_data_class(
        snapshot,
        export_execution_stage_persisted_contract(),
        DataClass::Personal,
    )?)?;
    let expected = export_execution_stage_record_ref(&stage)?;
    if snapshot.reference != expected {
        return Err(support::stored_data_error(
            "CUSTOMER_DATA_EXPORT_EXECUTION_STAGE_REFERENCE_INVALID",
        ));
    }
    Ok(stage)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publishes_strict_personal_execution_stage_contract() {
        let stage = PartyExportExecutionStage::emitted(
            ExportJobId::try_new("execution-stage-support-job").unwrap(),
            1,
            "party-1\n".to_owned(),
            0,
            100,
        )
        .unwrap();
        let reference = export_execution_stage_record_ref(&stage).unwrap();
        let payload = export_execution_stage_persisted_payload(&stage).unwrap();
        assert_eq!(reference.record_type.as_str(), EXPORT_EXECUTION_STAGE_RECORD_TYPE);
        assert_eq!(payload.data_class, DataClass::Personal);
        assert_eq!(
            export_execution_stage_persisted_contract().schema_id,
            EXPORT_EXECUTION_STAGE_STATE_SCHEMA_ID
        );
    }
}
