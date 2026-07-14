#![forbid(unsafe_code)]

//! Governed mutation adapter boundary for customer import jobs.
//!
//! Party remains the authoritative target owner. Import execution composition must invoke the
//! existing Party capability rather than reading or writing Party storage directly.

mod planner;

pub use planner::*;

use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRisk};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, SdkError,
};

pub const MODULE_ID: &str = "crm.customer-data-operations";
pub const IMPORT_JOB_RECORD_TYPE: &str = "customer_data.import_job";
pub const IMPORT_ROW_RECORD_TYPE: &str = "customer_data.import_row";

pub const CREATE_PARTY_IMPORT_JOB_CAPABILITY: &str = "customer_data.import.party.create";
pub const VALIDATE_PARTY_IMPORT_ROWS_CAPABILITY: &str = "customer_data.import.party.rows.validate";
pub const FINALIZE_PARTY_IMPORT_VALIDATION_CAPABILITY: &str =
    "customer_data.import.party.validation.finalize";
pub const START_PARTY_IMPORT_EXECUTION_CAPABILITY: &str =
    "customer_data.import.party.execution.start";
pub const CANCEL_PARTY_IMPORT_JOB_CAPABILITY: &str = "customer_data.import.party.cancel";

pub const CREATE_PARTY_IMPORT_JOB_REQUEST_SCHEMA: &str =
    "crm.customer_data_operations.v1.CreatePartyImportJobRequest";
pub const CREATE_PARTY_IMPORT_JOB_RESPONSE_SCHEMA: &str =
    "crm.customer_data_operations.v1.CreatePartyImportJobResponse";
pub const VALIDATE_PARTY_IMPORT_ROWS_REQUEST_SCHEMA: &str =
    "crm.customer_data_operations.v1.ValidatePartyImportRowsRequest";
pub const VALIDATE_PARTY_IMPORT_ROWS_RESPONSE_SCHEMA: &str =
    "crm.customer_data_operations.v1.ValidatePartyImportRowsResponse";
pub const FINALIZE_PARTY_IMPORT_VALIDATION_REQUEST_SCHEMA: &str =
    "crm.customer_data_operations.v1.FinalizePartyImportValidationRequest";
pub const FINALIZE_PARTY_IMPORT_VALIDATION_RESPONSE_SCHEMA: &str =
    "crm.customer_data_operations.v1.FinalizePartyImportValidationResponse";
pub const START_PARTY_IMPORT_EXECUTION_REQUEST_SCHEMA: &str =
    "crm.customer_data_operations.v1.StartPartyImportExecutionRequest";
pub const START_PARTY_IMPORT_EXECUTION_RESPONSE_SCHEMA: &str =
    "crm.customer_data_operations.v1.StartPartyImportExecutionResponse";
pub const CANCEL_PARTY_IMPORT_JOB_REQUEST_SCHEMA: &str =
    "crm.customer_data_operations.v1.CancelPartyImportJobRequest";
pub const CANCEL_PARTY_IMPORT_JOB_RESPONSE_SCHEMA: &str =
    "crm.customer_data_operations.v1.CancelPartyImportJobResponse";

pub const PARTY_IMPORT_JOB_CREATED_EVENT_TYPE: &str = "customer_data.import.party.created";
pub const PARTY_IMPORT_JOB_CREATED_EVENT_SCHEMA: &str =
    "crm.customer_data_operations.v1.PartyImportJobCreatedEvent";
pub const PARTY_IMPORT_ROW_VALIDATED_EVENT_TYPE: &str =
    "customer_data.import.party.row.validated";
pub const PARTY_IMPORT_ROW_VALIDATED_EVENT_SCHEMA: &str =
    "crm.customer_data_operations.v1.PartyImportRowValidatedEvent";
pub const PARTY_IMPORT_VALIDATION_COMPLETED_EVENT_TYPE: &str =
    "customer_data.import.party.validation_completed";
pub const PARTY_IMPORT_VALIDATION_COMPLETED_EVENT_SCHEMA: &str =
    "crm.customer_data_operations.v1.PartyImportValidationCompletedEvent";
pub const PARTY_IMPORT_EXECUTION_STARTED_EVENT_TYPE: &str =
    "customer_data.import.party.execution_started";
pub const PARTY_IMPORT_EXECUTION_STARTED_EVENT_SCHEMA: &str =
    "crm.customer_data_operations.v1.PartyImportExecutionStartedEvent";
pub const PARTY_IMPORT_ROW_EXECUTION_UPDATED_EVENT_TYPE: &str =
    "customer_data.import.party.row.execution_updated";
pub const PARTY_IMPORT_ROW_EXECUTION_UPDATED_EVENT_SCHEMA: &str =
    "crm.customer_data_operations.v1.PartyImportRowExecutionUpdatedEvent";
pub const PARTY_IMPORT_CHECKPOINT_ADVANCED_EVENT_TYPE: &str =
    "customer_data.import.party.checkpoint_advanced";
pub const PARTY_IMPORT_CHECKPOINT_ADVANCED_EVENT_SCHEMA: &str =
    "crm.customer_data_operations.v1.PartyImportCheckpointAdvancedEvent";
pub const PARTY_IMPORT_COMPLETED_EVENT_TYPE: &str = "customer_data.import.party.completed";
pub const PARTY_IMPORT_COMPLETED_EVENT_SCHEMA: &str =
    "crm.customer_data_operations.v1.PartyImportCompletedEvent";
pub const PARTY_IMPORT_CANCELLED_EVENT_TYPE: &str = "customer_data.import.party.cancelled";
pub const PARTY_IMPORT_CANCELLED_EVENT_SCHEMA: &str =
    "crm.customer_data_operations.v1.PartyImportCancelledEvent";

pub const MUTATION_CAPABILITY_IDS: [&str; 5] = [
    CREATE_PARTY_IMPORT_JOB_CAPABILITY,
    VALIDATE_PARTY_IMPORT_ROWS_CAPABILITY,
    FINALIZE_PARTY_IMPORT_VALIDATION_CAPABILITY,
    START_PARTY_IMPORT_EXECUTION_CAPABILITY,
    CANCEL_PARTY_IMPORT_JOB_CAPABILITY,
];

pub fn capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    MUTATION_CAPABILITY_IDS
        .into_iter()
        .map(capability_definition)
        .collect()
}

pub fn capability_definition(capability_id: &str) -> Result<CapabilityDefinition, SdkError> {
    let (input_schema, output_schema, risk) = match capability_id {
        CREATE_PARTY_IMPORT_JOB_CAPABILITY => (
            CREATE_PARTY_IMPORT_JOB_REQUEST_SCHEMA,
            CREATE_PARTY_IMPORT_JOB_RESPONSE_SCHEMA,
            CapabilityRisk::Medium,
        ),
        VALIDATE_PARTY_IMPORT_ROWS_CAPABILITY => (
            VALIDATE_PARTY_IMPORT_ROWS_REQUEST_SCHEMA,
            VALIDATE_PARTY_IMPORT_ROWS_RESPONSE_SCHEMA,
            CapabilityRisk::Medium,
        ),
        FINALIZE_PARTY_IMPORT_VALIDATION_CAPABILITY => (
            FINALIZE_PARTY_IMPORT_VALIDATION_REQUEST_SCHEMA,
            FINALIZE_PARTY_IMPORT_VALIDATION_RESPONSE_SCHEMA,
            CapabilityRisk::Medium,
        ),
        START_PARTY_IMPORT_EXECUTION_CAPABILITY => (
            START_PARTY_IMPORT_EXECUTION_REQUEST_SCHEMA,
            START_PARTY_IMPORT_EXECUTION_RESPONSE_SCHEMA,
            CapabilityRisk::High,
        ),
        CANCEL_PARTY_IMPORT_JOB_CAPABILITY => (
            CANCEL_PARTY_IMPORT_JOB_REQUEST_SCHEMA,
            CANCEL_PARTY_IMPORT_JOB_RESPONSE_SCHEMA,
            CapabilityRisk::Medium,
        ),
        _ => {
            return Err(configuration_error(
                "CUSTOMER_DATA_IMPORT_CAPABILITY_UNSUPPORTED",
                "The customer-data import mutation capability is unsupported.",
            ));
        }
    };

    Ok(CapabilityDefinition {
        capability_id: configured(CapabilityId::try_new(capability_id))?,
        capability_version: configured(CapabilityVersion::try_new(support::CONTRACT_VERSION))?,
        owner_module_id: configured(ModuleId::try_new(MODULE_ID))?,
        input_contract: support::protobuf_contract(
            MODULE_ID,
            input_schema,
            vec![DataClass::Personal],
        )?,
        output_contract: Some(support::protobuf_contract(
            MODULE_ID,
            output_schema,
            vec![DataClass::Personal],
        )?),
        risk,
        mutation: true,
        requires_idempotency: true,
        requires_approval: false,
        authorization_policy_id: capability_id.to_owned(),
        rate_limit_policy_id: None,
    })
}

fn configured<T>(value: Result<T, crm_module_sdk::IdentifierError>) -> Result<T, SdkError> {
    value.map_err(|error| {
        configuration_error(
            "CUSTOMER_DATA_IMPORT_CONFIGURATION_INVALID",
            "The customer-data import capability configuration is invalid.",
        )
        .with_internal_reference(error.to_string())
    })
}

fn configuration_error(code: &'static str, safe_message: &'static str) -> SdkError {
    SdkError::new(code, ErrorCategory::Internal, false, safe_message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publishes_exact_import_mutation_coordinates() {
        let definitions = capability_definitions().unwrap();
        assert_eq!(definitions.len(), 5);
        for (definition, capability_id) in definitions.iter().zip(MUTATION_CAPABILITY_IDS) {
            assert_eq!(definition.capability_id.as_str(), capability_id);
            assert_eq!(definition.owner_module_id.as_str(), MODULE_ID);
            assert_eq!(
                definition.capability_version.as_str(),
                support::CONTRACT_VERSION
            );
            assert_eq!(
                definition.input_contract.allowed_data_classes,
                vec![DataClass::Personal]
            );
            assert!(definition.mutation);
            assert!(definition.requires_idempotency);
            assert!(!definition.requires_approval);
        }
        assert_eq!(definitions[3].risk, CapabilityRisk::High);
    }

    #[test]
    fn rejects_unknown_import_mutation_coordinate() {
        let error = capability_definition("customer_data.import.party.destroy").unwrap_err();
        assert_eq!(error.code, "CUSTOMER_DATA_IMPORT_CAPABILITY_UNSUPPORTED");
    }
}
