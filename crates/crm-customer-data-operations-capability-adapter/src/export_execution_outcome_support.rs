use crm_capability_plan_support::{self as support, PersistedPayloadContract};
use crm_customer_data_operations::{
    EXPORT_EXECUTION_OUTCOME_STATE_MAXIMUM_BYTES, EXPORT_EXECUTION_OUTCOME_STATE_RETENTION_POLICY_ID,
    EXPORT_EXECUTION_OUTCOME_STATE_SCHEMA_ID, EXPORT_EXECUTION_OUTCOME_STATE_SCHEMA_VERSION,
    PartyExportExecutionOutcome, decode_export_execution_outcome_state,
    encode_export_execution_outcome_state, export_execution_outcome_state_descriptor_hash,
};
use crm_module_sdk::{DataClass, RecordSnapshot, SdkError};

use crate::MODULE_ID;

pub const EXPORT_EXECUTION_OUTCOME_RECORD_TYPE: &str = "customer_data.export_execution_outcome";

pub fn export_execution_outcome_persisted_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: EXPORT_EXECUTION_OUTCOME_STATE_SCHEMA_ID,
        schema_version: EXPORT_EXECUTION_OUTCOME_STATE_SCHEMA_VERSION,
        descriptor_hash: export_execution_outcome_state_descriptor_hash(),
        maximum_size_bytes: EXPORT_EXECUTION_OUTCOME_STATE_MAXIMUM_BYTES,
        retention_policy_id: EXPORT_EXECUTION_OUTCOME_STATE_RETENTION_POLICY_ID,
    }
}

pub fn export_execution_outcome_persisted_payload(
    outcome: &PartyExportExecutionOutcome,
) -> Result<crm_module_sdk::TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        export_execution_outcome_persisted_contract(),
        DataClass::Personal,
        encode_export_execution_outcome_state(outcome)?,
    )
}

pub fn export_execution_outcome_record_ref(
    outcome: &PartyExportExecutionOutcome,
) -> Result<crm_module_sdk::RecordRef, SdkError> {
    support::record_ref(
        EXPORT_EXECUTION_OUTCOME_RECORD_TYPE,
        outcome.outcome_id().as_str(),
        "customer_data.export.execution_outcome_id",
    )
}

pub fn export_execution_outcome_from_snapshot(
    snapshot: &RecordSnapshot,
) -> Result<PartyExportExecutionOutcome, SdkError> {
    let outcome = decode_export_execution_outcome_state(
        support::persisted_json_bytes_with_data_class(
            snapshot,
            export_execution_outcome_persisted_contract(),
            DataClass::Personal,
        )?,
    )?;
    let expected = export_execution_outcome_record_ref(&outcome)?;
    if snapshot.reference != expected {
        return Err(support::stored_data_error(
            "CUSTOMER_DATA_EXPORT_EXECUTION_OUTCOME_REFERENCE_INVALID",
        ));
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_customer_data_operations::ExportJobId;

    #[test]
    fn publishes_strict_personal_execution_outcome_contract() {
        let outcome = PartyExportExecutionOutcome::emitted(
            ExportJobId::try_new("execution-outcome-support-job").unwrap(),
            1,
            1,
            "11".repeat(32),
            12,
            0,
            100,
        )
        .unwrap();
        let reference = export_execution_outcome_record_ref(&outcome).unwrap();
        let payload = export_execution_outcome_persisted_payload(&outcome).unwrap();
        assert_eq!(reference.record_type.as_str(), EXPORT_EXECUTION_OUTCOME_RECORD_TYPE);
        assert_eq!(payload.data_class, DataClass::Personal);
    }
}
