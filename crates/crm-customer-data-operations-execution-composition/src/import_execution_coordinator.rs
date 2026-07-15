use crate::{
    ExecutionCycleOutcome, ImportExecutionOutcomeSink, ImportExecutionSnapshot,
    validate_partial_execution_policy,
};
use crm_capability_plan_support as support;
use crm_customer_data_operations::{ImportRow, ImportRowStatus, PartyImportKind, TargetPartyId};
use crm_module_sdk::{
    BusinessTransactionId, CapabilityClient, CapabilityId, CapabilityInvocation, CapabilityOutcome,
    CapabilityVersion, DataClass, ErrorCategory, IdempotencyKey, ModuleExecutionContext, ModuleId,
    PortFuture, SdkError,
};
use crm_parties_capability_adapter::{
    CREATE_CAPABILITY as PARTY_CREATE_CAPABILITY,
    CREATE_REQUEST_SCHEMA as PARTY_CREATE_REQUEST_SCHEMA, MODULE_ID as PARTIES_MODULE_ID,
    RECORD_TYPE as PARTY_RECORD_TYPE,
};
use crm_proto_contracts::crm::{customer::v1 as customer, parties::v1 as parties};
use std::sync::Arc;

pub const MODULE_ID: &str = "crm.customer-data-operations";
pub const CONTRACT_VERSION: &str = "1.0.0";

#[derive(Clone)]
pub struct PartyImportExecutionCoordinator {
    target_client: Arc<dyn CapabilityClient>,
    outcomes: Arc<dyn ImportExecutionOutcomeSink>,
}

impl std::fmt::Debug for PartyImportExecutionCoordinator {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PartyImportExecutionCoordinator")
            .field("target_client", &"dyn CapabilityClient")
            .field("outcomes", &"dyn ImportExecutionOutcomeSink")
            .finish()
    }
}

impl PartyImportExecutionCoordinator {
    pub fn new(
        target_client: Arc<dyn CapabilityClient>,
        outcomes: Arc<dyn ImportExecutionOutcomeSink>,
    ) -> Self {
        Self {
            target_client,
            outcomes,
        }
    }

    pub fn execute_next<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        snapshot: &'a ImportExecutionSnapshot,
    ) -> PortFuture<'a, Result<ExecutionCycleOutcome, SdkError>> {
        Box::pin(async move {
            context.validate()?;
            let Some(row) = snapshot.next_row()? else {
                self.outcomes.complete(context, snapshot.job()).await?;
                return Ok(ExecutionCycleOutcome::Completed);
            };

            match row.status() {
                ImportRowStatus::Invalid => {
                    validate_partial_execution_policy(snapshot, row)?;
                    self.outcomes
                        .skip_invalid(context, snapshot.job(), row)
                        .await?;
                    Ok(ExecutionCycleOutcome::SkippedInvalid {
                        row_position: row.row_position(),
                    })
                }
                ImportRowStatus::Valid | ImportRowStatus::FailedRetryable => {
                    let prepared_party = row.prepared_party().ok_or_else(|| {
                        execution_error(
                            "CUSTOMER_DATA_IMPORT_EXECUTION_PREPARED_PARTY_MISSING",
                            ErrorCategory::Internal,
                            false,
                            "Customer-data import execution state is inconsistent.",
                        )
                    })?;
                    let target_context = target_context(context, row)?;
                    let invocation = party_create_invocation(row)?;
                    match self.target_client.invoke(&target_context, invocation).await {
                        Ok(outcome) => {
                            validate_target_outcome(&outcome, prepared_party.party_id())?;
                            self.outcomes
                                .record_success(
                                    context,
                                    snapshot.job(),
                                    row,
                                    prepared_party.party_id(),
                                )
                                .await?;
                            Ok(ExecutionCycleOutcome::PartySucceeded {
                                row_position: row.row_position(),
                                party_id: prepared_party.party_id().as_str().to_owned(),
                            })
                        }
                        Err(error) if error.retryable => {
                            self.outcomes
                                .record_retryable_failure(context, snapshot.job(), row, &error.code)
                                .await?;
                            Ok(ExecutionCycleOutcome::RetryableFailureRecorded {
                                row_position: row.row_position(),
                                error_code: error.code,
                            })
                        }
                        Err(error) => Err(error),
                    }
                }
                ImportRowStatus::Pending => Err(execution_error(
                    "CUSTOMER_DATA_IMPORT_EXECUTION_PENDING_ROW",
                    ErrorCategory::Internal,
                    false,
                    "Customer-data import execution state is inconsistent.",
                )),
                ImportRowStatus::Succeeded => Err(execution_error(
                    "CUSTOMER_DATA_IMPORT_EXECUTION_SUCCEEDED_ROW_AFTER_CHECKPOINT",
                    ErrorCategory::Internal,
                    false,
                    "Customer-data import execution state is inconsistent.",
                )),
            }
        })
    }
}

pub fn party_create_invocation(row: &ImportRow) -> Result<CapabilityInvocation, SdkError> {
    if !matches!(
        row.status(),
        ImportRowStatus::Valid | ImportRowStatus::FailedRetryable
    ) {
        return Err(execution_error(
            "CUSTOMER_DATA_IMPORT_EXECUTION_ROW_NOT_EXECUTABLE",
            ErrorCategory::Conflict,
            false,
            "The import row is not executable.",
        ));
    }
    let prepared = row.prepared_party().ok_or_else(|| {
        execution_error(
            "CUSTOMER_DATA_IMPORT_EXECUTION_PREPARED_PARTY_MISSING",
            ErrorCategory::Internal,
            false,
            "Customer-data import execution state is inconsistent.",
        )
    })?;
    let kind = match prepared.kind() {
        PartyImportKind::Person => parties::PartyKind::Person,
        PartyImportKind::Organization => parties::PartyKind::Organization,
    };
    let input = support::protobuf_payload(
        PARTIES_MODULE_ID,
        PARTY_CREATE_REQUEST_SCHEMA,
        DataClass::Personal,
        &parties::CreatePartyRequest {
            party_ref: Some(customer::PartyRef {
                party_id: prepared.party_id().as_str().to_owned(),
            }),
            kind: kind as i32,
            display_name: prepared.display_name().to_owned(),
        },
    )?;
    Ok(CapabilityInvocation {
        capability_id: configured_capability_id(PARTY_CREATE_CAPABILITY)?,
        capability_version: configured_capability_version(CONTRACT_VERSION)?,
        input,
    })
}

pub fn target_context(
    base: &ModuleExecutionContext,
    row: &ImportRow,
) -> Result<ModuleExecutionContext, SdkError> {
    base.validate()?;
    let mut context = base.clone();
    context.module_id = ModuleId::try_new(MODULE_ID).map_err(configuration_error)?;
    let target_transaction_id = row.target_idempotency_key();
    context.execution.idempotency_key =
        IdempotencyKey::try_new(target_transaction_id.clone()).map_err(configuration_error)?;
    context.execution.business_transaction_id =
        BusinessTransactionId::try_new(target_transaction_id).map_err(configuration_error)?;
    Ok(context)
}

fn validate_target_outcome(
    outcome: &CapabilityOutcome,
    expected_party_id: &TargetPartyId,
) -> Result<(), SdkError> {
    if outcome.affected_resources.iter().any(|resource| {
        resource.resource_type == PARTY_RECORD_TYPE
            && resource.resource_id == expected_party_id.as_str()
    }) {
        Ok(())
    } else {
        Err(execution_error(
            "CUSTOMER_DATA_IMPORT_EXECUTION_TARGET_RESULT_MISMATCH",
            ErrorCategory::Dependency,
            false,
            "The governed Party result did not match the prepared import target.",
        ))
    }
}

fn configured_capability_id(value: &str) -> Result<CapabilityId, SdkError> {
    CapabilityId::try_new(value).map_err(configuration_error)
}

fn configured_capability_version(value: &str) -> Result<CapabilityVersion, SdkError> {
    CapabilityVersion::try_new(value).map_err(configuration_error)
}

fn configuration_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    execution_error(
        "CUSTOMER_DATA_IMPORT_EXECUTION_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "Customer-data import execution is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}

fn execution_error(
    code: &'static str,
    category: ErrorCategory,
    retryable: bool,
    safe_message: &'static str,
) -> SdkError {
    SdkError::new(code, category, retryable, safe_message)
}
