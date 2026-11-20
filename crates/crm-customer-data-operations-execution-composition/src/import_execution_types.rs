use crm_customer_data_operations::{
    ExecutionPositionIndex, ExecutionRowReference, ImportJob, ImportJobStatus, ImportRow,
    ImportRowStatus, PartialExecutionPolicy, TargetPartyId,
};
use crm_module_sdk::{ErrorCategory, ModuleExecutionContext, PortFuture, SdkError};

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
                "Only an executing import job can be processed.",
            ));
        }
        if rows.iter().any(|row| row.job_id() != job.job_id()) {
            return Err(execution_error(
                "CUSTOMER_DATA_IMPORT_EXECUTION_ROW_JOB_MISMATCH",
                ErrorCategory::Internal,
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

    pub(crate) fn next_row(&self) -> Result<Option<&ImportRow>, SdkError> {
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

pub(crate) fn validate_partial_execution_policy(
    snapshot: &ImportExecutionSnapshot,
    row: &ImportRow,
) -> Result<(), SdkError> {
    if row.status() == ImportRowStatus::Invalid
        && snapshot.job().snapshot().partial_execution_policy
            != PartialExecutionPolicy::AllValidRows
    {
        return Err(execution_error(
            "CUSTOMER_DATA_IMPORT_EXECUTION_INVALID_ROW_POLICY_CONFLICT",
            ErrorCategory::Internal,
            "Customer-data import execution state is inconsistent.",
        ));
    }
    Ok(())
}

fn execution_error(
    code: &'static str,
    category: ErrorCategory,
    safe_message: &'static str,
) -> SdkError {
    SdkError::new(code, category, false, safe_message)
}
