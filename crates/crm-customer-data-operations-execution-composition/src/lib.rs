#![forbid(unsafe_code)]

//! Governed execution composition for validated customer-data Party imports.
//!
//! The pure customer-data-operations domain owns import coordination state while
//! `crm.parties` remains the only Party owner. This composition selects the next
//! authoritative source position, invokes exact Party creation through a
//! `CapabilityClient`, and delegates import-owned outcome/checkpoint persistence
//! to a private sink. No public bulk-write or direct Party storage path exists here.

pub mod postgres_reader;
pub use postgres_reader::*;
pub mod outcome_plan;
pub use outcome_plan::*;
pub mod postgres_outcome_sink;
pub use postgres_outcome_sink::*;
pub mod worker;
pub use worker::*;

use crm_capability_plan_support as support;
use crm_customer_data_operations::{
    ExecutionPositionIndex, ExecutionRowReference, ImportJob, ImportJobStatus, ImportRow,
    ImportRowStatus, PartialExecutionPolicy, PartyImportKind, TargetPartyId,
};
use crm_module_sdk::{
    CapabilityClient, CapabilityId, CapabilityInvocation, CapabilityOutcome, CapabilityVersion,
    DataClass, ErrorCategory, IdempotencyKey, ModuleExecutionContext, ModuleId, PortFuture,
    SdkError,
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

#[derive(Debug, Clone)]
pub struct ImportExecutionSnapshot {
    job: ImportJob,
    rows: Vec<ImportRow>,
    position_index: ExecutionPositionIndex,
}

impl ImportExecutionSnapshot {
    pub fn try_new(job: ImportJob, rows: Vec<ImportRow>) -> Result<Self, SdkError> {
        if job.status() != ImportJobStatus::Executing {
            return Err(execution_error(
                "CUSTOMER_DATA_IMPORT_EXECUTION_JOB_NOT_EXECUTING",
                ErrorCategory::Conflict,
                false,
                "Only an executing import job can be processed.",
            ));
        }
        if rows.iter().any(|row| row.job_id() != job.job_id()) {
            return Err(execution_error(
                "CUSTOMER_DATA_IMPORT_EXECUTION_ROW_JOB_MISMATCH",
                ErrorCategory::Internal,
                false,
                "Customer-data import execution state is inconsistent.",
            ));
        }
        let position_index = ExecutionPositionIndex::build(
            job.total_rows(),
            rows.iter().map(|row| {
                ExecutionRowReference::new(row.row_id().clone(), row.row_position(), row.status())
            }),
        )?;
        Ok(Self {
            job,
            rows,
            position_index,
        })
    }

    pub fn job(&self) -> &ImportJob {
        &self.job
    }

    pub fn rows(&self) -> &[ImportRow] {
        &self.rows
    }

    fn next_row(&self) -> Result<Option<&ImportRow>, SdkError> {
        let Some(next) = self
            .position_index
            .next_after_checkpoint(self.job.checkpoint_row_position())?
        else {
            return Ok(None);
        };
        self.rows
            .iter()
            .find(|row| row.row_id() == next.row_id())
            .map(Some)
            .ok_or_else(|| {
                execution_error(
                    "CUSTOMER_DATA_IMPORT_EXECUTION_ROW_REFERENCE_MISSING",
                    ErrorCategory::Internal,
                    false,
                    "Customer-data import execution state is inconsistent.",
                )
            })
    }
}

pub trait ImportExecutionOutcomeSink: Send + Sync {
    fn skip_invalid<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        job: &'a ImportJob,
        row: &'a ImportRow,
    ) -> PortFuture<'a, Result<(), SdkError>>;

    fn record_success<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        job: &'a ImportJob,
        row: &'a ImportRow,
        target_party_id: &'a TargetPartyId,
    ) -> PortFuture<'a, Result<(), SdkError>>;

    fn record_retryable_failure<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        job: &'a ImportJob,
        row: &'a ImportRow,
        error_code: &'a str,
    ) -> PortFuture<'a, Result<(), SdkError>>;

    fn complete<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        job: &'a ImportJob,
    ) -> PortFuture<'a, Result<(), SdkError>>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionCycleOutcome {
    Completed,
    SkippedInvalid {
        row_position: u32,
    },
    PartySucceeded {
        row_position: u32,
        party_id: String,
    },
    RetryableFailureRecorded {
        row_position: u32,
        error_code: String,
    },
}

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
                    if snapshot.job().snapshot().partial_execution_policy
                        != PartialExecutionPolicy::AllValidRows
                    {
                        return Err(execution_error(
                            "CUSTOMER_DATA_IMPORT_EXECUTION_INVALID_ROW_POLICY_CONFLICT",
                            ErrorCategory::Internal,
                            false,
                            "Customer-data import execution state is inconsistent.",
                        ));
                    }
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
    context.execution.idempotency_key =
        IdempotencyKey::try_new(row.target_idempotency_key()).map_err(configuration_error)?;
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
