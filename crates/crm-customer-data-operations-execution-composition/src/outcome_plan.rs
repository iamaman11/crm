use crm_customer_data_operations::{
    AdvanceImportCheckpoint, CheckpointOutcome, FinishImportJob, ImportJob, ImportRow, ImportRowStatus,
    MarkImportRowSucceeded, RecordImportRowRetryableFailure, TargetPartyId,
};
use crm_module_sdk::{ErrorCategory, SdkError};

#[derive(Debug, Clone)]
pub struct PlannedImportJobUpdate {
    expected_version: i64,
    after: ImportJob,
}

impl PlannedImportJobUpdate {
    pub const fn expected_version(&self) -> i64 {
        self.expected_version
    }

    pub fn after(&self) -> &ImportJob {
        &self.after
    }
}

#[derive(Debug, Clone)]
pub struct PlannedImportRowUpdate {
    expected_version: i64,
    after: ImportRow,
}

impl PlannedImportRowUpdate {
    pub const fn expected_version(&self) -> i64 {
        self.expected_version
    }

    pub fn after(&self) -> &ImportRow {
        &self.after
    }
}

#[derive(Debug, Clone)]
pub enum ImportExecutionOutcomePlan {
    SkippedInvalid {
        job: PlannedImportJobUpdate,
        row_position: u32,
    },
    Succeeded {
        job: PlannedImportJobUpdate,
        row: PlannedImportRowUpdate,
        target_party_id: TargetPartyId,
    },
    RetryableFailure {
        row: PlannedImportRowUpdate,
        error_code: String,
    },
    Completed {
        job: PlannedImportJobUpdate,
    },
}

pub fn plan_skip_invalid(
    job: &ImportJob,
    row: &ImportRow,
    occurred_at_unix_nanos: i64,
) -> Result<ImportExecutionOutcomePlan, SdkError> {
    require_same_job(job, row)?;
    if row.status() != ImportRowStatus::Invalid {
        return Err(outcome_error(
            "CUSTOMER_DATA_IMPORT_EXECUTION_SKIP_ROW_NOT_INVALID",
            ErrorCategory::Conflict,
            "Only an invalid import row can be skipped.",
        ));
    }

    let expected_version = job.version();
    let mut after = job.clone();
    after.advance_checkpoint(AdvanceImportCheckpoint {
        expected_version,
        row_position: row.row_position(),
        outcome: CheckpointOutcome::SkippedInvalid,
        occurred_at_unix_nanos,
    })?;

    Ok(ImportExecutionOutcomePlan::SkippedInvalid {
        job: PlannedImportJobUpdate {
            expected_version,
            after,
        },
        row_position: row.row_position(),
    })
}

pub fn plan_success(
    job: &ImportJob,
    row: &ImportRow,
    target_party_id: TargetPartyId,
    occurred_at_unix_nanos: i64,
) -> Result<ImportExecutionOutcomePlan, SdkError> {
    require_same_job(job, row)?;

    let expected_row_version = row.version();
    let mut row_after = row.clone();
    row_after.mark_succeeded(MarkImportRowSucceeded {
        expected_version: expected_row_version,
        target_party_id: target_party_id.clone(),
        occurred_at_unix_nanos,
    })?;

    let expected_job_version = job.version();
    let mut job_after = job.clone();
    job_after.advance_checkpoint(AdvanceImportCheckpoint {
        expected_version: expected_job_version,
        row_position: row.row_position(),
        outcome: CheckpointOutcome::Succeeded,
        occurred_at_unix_nanos,
    })?;

    Ok(ImportExecutionOutcomePlan::Succeeded {
        job: PlannedImportJobUpdate {
            expected_version: expected_job_version,
            after: job_after,
        },
        row: PlannedImportRowUpdate {
            expected_version: expected_row_version,
            after: row_after,
        },
        target_party_id,
    })
}

pub fn plan_retryable_failure(
    job: &ImportJob,
    row: &ImportRow,
    error_code: String,
    occurred_at_unix_nanos: i64,
) -> Result<ImportExecutionOutcomePlan, SdkError> {
    require_same_job(job, row)?;

    let expected_version = row.version();
    let mut after = row.clone();
    after.record_retryable_failure(RecordImportRowRetryableFailure {
        expected_version,
        error_code,
        occurred_at_unix_nanos,
    })?;
    let persisted_error_code = after
        .snapshot()
        .last_execution_error_code
        .clone()
        .ok_or_else(|| {
            outcome_error(
                "CUSTOMER_DATA_IMPORT_EXECUTION_FAILURE_CODE_MISSING",
                ErrorCategory::Internal,
                "Customer-data import execution state is inconsistent.",
            )
        })?;

    Ok(ImportExecutionOutcomePlan::RetryableFailure {
        row: PlannedImportRowUpdate {
            expected_version,
            after,
        },
        error_code: persisted_error_code,
    })
}

pub fn plan_completion(
    job: &ImportJob,
    occurred_at_unix_nanos: i64,
) -> Result<ImportExecutionOutcomePlan, SdkError> {
    let expected_version = job.version();
    let mut after = job.clone();
    after.complete(FinishImportJob {
        expected_version,
        occurred_at_unix_nanos,
    })?;
    Ok(ImportExecutionOutcomePlan::Completed {
        job: PlannedImportJobUpdate {
            expected_version,
            after,
        },
    })
}

fn require_same_job(job: &ImportJob, row: &ImportRow) -> Result<(), SdkError> {
    if row.job_id() == job.job_id() {
        Ok(())
    } else {
        Err(outcome_error(
            "CUSTOMER_DATA_IMPORT_EXECUTION_OUTCOME_JOB_MISMATCH",
            ErrorCategory::Internal,
            "Customer-data import execution state is inconsistent.",
        ))
    }
}

fn outcome_error(
    code: &'static str,
    category: ErrorCategory,
    safe_message: &'static str,
) -> SdkError {
    SdkError::new(code, category, false, safe_message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_customer_data_operations::{
        CreateImportJob, CreateValidatedImportRow, ImportJobId, ImportParserProfile,
        InitialImportRowValidation, MarkImportJobValidated, PartialExecutionPolicy, PartyImportKind,
        PartyImportMapping, PreparedPartyRow, RowDiagnostic, SourceDescriptor, SourceSystemId,
        StartImportExecution,
    };

    #[test]
    fn success_plans_row_and_job_updates_from_the_same_authoritative_versions() {
        let job = executing_job(1, 1, 0, PartialExecutionPolicy::AllValidRows);
        let row = valid_row(job.job_id().clone(), 1, "party-success-1");

        let plan = plan_success(
            &job,
            &row,
            TargetPartyId::try_new("party-success-1").unwrap(),
            40,
        )
        .unwrap();

        let ImportExecutionOutcomePlan::Succeeded { job, row, .. } = plan else {
            panic!("expected succeeded plan");
        };
        assert_eq!(job.expected_version(), 3);
        assert_eq!(job.after().checkpoint_row_position(), 1);
        assert_eq!(job.after().succeeded_rows(), 1);
        assert_eq!(row.expected_version(), 1);
        assert_eq!(row.after().status(), ImportRowStatus::Succeeded);
        assert_eq!(row.after().version(), 2);
    }

    #[test]
    fn retryable_failure_updates_only_the_row_and_preserves_the_job_checkpoint() {
        let job = executing_job(1, 1, 0, PartialExecutionPolicy::AllValidRows);
        let row = valid_row(job.job_id().clone(), 1, "party-retry-1");

        let plan = plan_retryable_failure(&job, &row, "PARTY_TEMPORARY".to_owned(), 40).unwrap();

        let ImportExecutionOutcomePlan::RetryableFailure { row, error_code } = plan else {
            panic!("expected retryable failure plan");
        };
        assert_eq!(job.checkpoint_row_position(), 0);
        assert_eq!(row.expected_version(), 1);
        assert_eq!(row.after().status(), ImportRowStatus::FailedRetryable);
        assert_eq!(row.after().version(), 2);
        assert_eq!(error_code, "PARTY_TEMPORARY");
    }

    #[test]
    fn invalid_skip_advances_only_the_next_sequential_checkpoint() {
        let job = executing_job(2, 1, 1, PartialExecutionPolicy::AllValidRows);
        let row = invalid_row(job.job_id().clone(), 1);

        let plan = plan_skip_invalid(&job, &row, 40).unwrap();

        let ImportExecutionOutcomePlan::SkippedInvalid { job, row_position } = plan else {
            panic!("expected skipped-invalid plan");
        };
        assert_eq!(row_position, 1);
        assert_eq!(job.expected_version(), 3);
        assert_eq!(job.after().checkpoint_row_position(), 1);
        assert_eq!(job.after().succeeded_rows(), 0);
    }

    fn executing_job(
        total_rows: u32,
        valid_rows: u32,
        invalid_rows: u32,
        policy: PartialExecutionPolicy,
    ) -> ImportJob {
        let mut job = ImportJob::create(CreateImportJob {
            job_id: ImportJobId::try_new("import-job-outcome-plan-1").unwrap(),
            source: SourceDescriptor::try_new(
                "customers.csv",
                "11".repeat(32),
                total_rows,
                SourceSystemId::try_new("legacy-crm").unwrap(),
                ImportParserProfile::csv_v1(b',', b'"').unwrap(),
            )
            .unwrap(),
            mapping: PartyImportMapping::try_new(
                None,
                "kind",
                "display_name",
                Some("legacy_id".to_owned()),
                Some("row_key".to_owned()),
            )
            .unwrap(),
            partial_execution_policy: policy,
            occurred_at_unix_nanos: 10,
        })
        .unwrap();
        job.mark_validated(MarkImportJobValidated {
            expected_version: 1,
            valid_rows,
            invalid_rows,
            occurred_at_unix_nanos: 20,
        })
        .unwrap();
        job.start_execution(StartImportExecution {
            expected_version: 2,
            occurred_at_unix_nanos: 30,
        })
        .unwrap();
        job
    }

    fn valid_row(job_id: ImportJobId, row_position: u32, party_id: &str) -> ImportRow {
        crm_customer_data_operations::create_validated_import_row(CreateValidatedImportRow {
            job_id,
            row_position,
            external_row_key: Some(format!("row-{row_position}")),
            source_external_id: Some(format!("legacy-{row_position}")),
            outcome: InitialImportRowValidation::Valid(
                PreparedPartyRow::try_new(
                    TargetPartyId::try_new(party_id).unwrap(),
                    PartyImportKind::Person,
                    format!("Party {row_position}"),
                )
                .unwrap(),
            ),
            occurred_at_unix_nanos: 15,
        })
        .unwrap()
    }

    fn invalid_row(job_id: ImportJobId, row_position: u32) -> ImportRow {
        crm_customer_data_operations::create_validated_import_row(CreateValidatedImportRow {
            job_id,
            row_position,
            external_row_key: None,
            source_external_id: None,
            outcome: InitialImportRowValidation::Invalid(vec![
                RowDiagnostic::try_new("DISPLAY_NAME_MISSING", "display_name").unwrap(),
            ]),
            occurred_at_unix_nanos: 15,
        })
        .unwrap()
    }
}
