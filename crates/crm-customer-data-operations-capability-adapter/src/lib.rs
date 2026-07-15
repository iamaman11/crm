#![forbid(unsafe_code)]

//! Governed mutation adapter boundary for customer-data operations.
//!
//! Party remains the authoritative customer identity owner. Import target execution must invoke the
//! existing Party capability rather than reading or writing Party storage directly. Export owns only
//! its job/specification/selection-boundary/manifest/execution-outcome/artifact/reconciliation
//! evidence and must read Party data through governed query composition rather than direct Party
//! storage access.
//!
//! Public import/export mutation planners may atomically mutate only customer-data-operation-owned
//! records. The production combined planner additionally fixes the immutable export selection cutoff
//! in the same transaction as the first export start. Background worker-only selection, checkpoint
//! and completion outcomes remain separate internal capability/composition responsibilities and must
//! not leak into public mutation catalogs.

mod export_boundary_planner;
mod export_planner;
mod export_selection_planner;
mod planner;

pub use export_boundary_planner::*;
pub use export_planner::*;
pub use export_selection_planner::*;
pub use planner::*;

use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest, CapabilityRisk};
use crm_core_data::{AggregateTarget, CapabilityBatchExecutionPlan, TransactionalAggregatePlanner};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, RecordSnapshot, SdkError,
};

pub const MODULE_ID: &str = "crm.customer-data-operations";
pub const IMPORT_JOB_RECORD_TYPE: &str = "customer_data.import_job";
pub const IMPORT_ROW_RECORD_TYPE: &str = "customer_data.import_row";
pub const IMPORT_JOB_ROW_RELATIONSHIP_TYPE: &str = "customer_data.import_job.row";

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
pub const PARTY_IMPORT_ROW_VALIDATED_EVENT_TYPE: &str = "customer_data.import.party.row_validated";
pub const PARTY_IMPORT_ROW_VALIDATED_EVENT_SCHEMA: &str =
    "crm.customer_data_operations.v1.PartyImportRowValidatedEvent";
pub const PARTY_IMPORT_VALIDATION_PROGRESSED_EVENT_TYPE: &str =
    "customer_data.import.party.validation_progressed";
pub const PARTY_IMPORT_VALIDATION_PROGRESSED_EVENT_SCHEMA: &str =
    "crm.customer_data_operations.v1.PartyImportValidationProgressedEvent";
pub const PARTY_IMPORT_VALIDATION_COMPLETED_EVENT_TYPE: &str =
    "customer_data.import.party.validation_completed";
pub const PARTY_IMPORT_VALIDATION_COMPLETED_EVENT_SCHEMA: &str =
    "crm.customer_data_operations.v1.PartyImportValidationCompletedEvent";
pub const PARTY_IMPORT_EXECUTION_STARTED_EVENT_TYPE: &str =
    "customer_data.import.party.execution_started";
pub const PARTY_IMPORT_EXECUTION_STARTED_EVENT_SCHEMA: &str =
    "crm.customer_data_operations.v1.PartyImportExecutionStartedEvent";
pub const PARTY_IMPORT_ROW_SUCCEEDED_EVENT_TYPE: &str = "customer_data.import.party.row_succeeded";
pub const PARTY_IMPORT_ROW_SUCCEEDED_EVENT_SCHEMA: &str =
    "crm.customer_data_operations.v1.PartyImportRowSucceededEvent";
pub const PARTY_IMPORT_ROW_FAILED_EVENT_TYPE: &str = "customer_data.import.party.row_failed";
pub const PARTY_IMPORT_ROW_FAILED_EVENT_SCHEMA: &str =
    "crm.customer_data_operations.v1.PartyImportRowFailedEvent";
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

pub const IMPORT_MUTATION_CAPABILITY_IDS: [&str; 5] = [
    CREATE_PARTY_IMPORT_JOB_CAPABILITY,
    VALIDATE_PARTY_IMPORT_ROWS_CAPABILITY,
    FINALIZE_PARTY_IMPORT_VALIDATION_CAPABILITY,
    START_PARTY_IMPORT_EXECUTION_CAPABILITY,
    CANCEL_PARTY_IMPORT_JOB_CAPABILITY,
];

pub const MUTATION_CAPABILITY_IDS: [&str; 8] = [
    CREATE_PARTY_IMPORT_JOB_CAPABILITY,
    VALIDATE_PARTY_IMPORT_ROWS_CAPABILITY,
    FINALIZE_PARTY_IMPORT_VALIDATION_CAPABILITY,
    START_PARTY_IMPORT_EXECUTION_CAPABILITY,
    CANCEL_PARTY_IMPORT_JOB_CAPABILITY,
    CREATE_PARTY_EXPORT_JOB_CAPABILITY,
    START_PARTY_EXPORT_EXECUTION_CAPABILITY,
    CANCEL_PARTY_EXPORT_JOB_CAPABILITY,
];

pub fn capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    let mut definitions = import_capability_definitions()?;
    let mut export_definitions = export_capability_definitions()?;

    // Bulk export is a high-risk disclosure operation. The production catalog fails closed by
    // requiring approval for execution until a tenant-specific policy layer explicitly introduces a
    // governed lower-friction threshold. Creating or cancelling a job remains approval-free.
    for definition in &mut export_definitions {
        if definition.capability_id.as_str() == START_PARTY_EXPORT_EXECUTION_CAPABILITY {
            definition.requires_approval = true;
        }
    }

    definitions.extend(export_definitions);
    Ok(definitions)
}

pub fn import_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    IMPORT_MUTATION_CAPABILITY_IDS
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

#[derive(Debug, Default, Clone, Copy)]
pub struct CustomerDataOperationsCapabilityPlanner;

impl TransactionalAggregatePlanner for CustomerDataOperationsCapabilityPlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        if IMPORT_MUTATION_CAPABILITY_IDS.contains(&definition.capability_id.as_str()) {
            CustomerDataImportCapabilityPlanner.target(definition, request)
        } else if EXPORT_MUTATION_CAPABILITY_IDS.contains(&definition.capability_id.as_str()) {
            PartyExportCapabilityPlanner.target(definition, request)
        } else {
            Err(routing_error())
        }
    }

    fn plan(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
        current: Option<&RecordSnapshot>,
    ) -> Result<CapabilityBatchExecutionPlan, SdkError> {
        if IMPORT_MUTATION_CAPABILITY_IDS.contains(&definition.capability_id.as_str()) {
            CustomerDataImportCapabilityPlanner.plan(definition, request, current)
        } else if EXPORT_MUTATION_CAPABILITY_IDS.contains(&definition.capability_id.as_str()) {
            let plan = PartyExportCapabilityPlanner.plan(definition, request, current)?;
            harden_party_export_start_plan(definition, request, current, plan)
        } else {
            Err(routing_error())
        }
    }
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

fn routing_error() -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_OPERATION_CAPABILITY_UNSUPPORTED",
        ErrorCategory::Internal,
        false,
        "The customer-data operation capability is not configured.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publishes_exact_customer_data_mutation_coordinates() {
        let definitions = capability_definitions().unwrap();
        assert_eq!(definitions.len(), 8);
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
            if capability_id == START_PARTY_EXPORT_EXECUTION_CAPABILITY {
                assert!(definition.requires_approval);
            } else {
                assert!(!definition.requires_approval);
            }
        }
        assert_eq!(definitions[3].risk, CapabilityRisk::High);
        assert_eq!(definitions[6].risk, CapabilityRisk::High);
    }

    #[test]
    fn import_definition_rejects_unknown_import_coordinate() {
        let error = capability_definition("customer_data.import.party.destroy").unwrap_err();
        assert_eq!(error.code, "CUSTOMER_DATA_IMPORT_CAPABILITY_UNSUPPORTED");
    }
}
