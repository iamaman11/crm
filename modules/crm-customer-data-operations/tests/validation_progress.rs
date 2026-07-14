use crm_customer_data_operations::{
    CreateImportJob, FinalizeImportValidation, ImportJob, ImportJobId, ImportJobStatus,
    ImportParserProfile, PartialExecutionPolicy, PartyImportMapping, RecordImportValidationBatch,
    SourceDescriptor, SourceSystemId,
};

fn job() -> ImportJob {
    ImportJob::create(CreateImportJob {
        job_id: ImportJobId::try_new("import-job-validation-progress").unwrap(),
        source: SourceDescriptor::try_new(
            "customers.csv",
            "11".repeat(32),
            2,
            SourceSystemId::try_new("legacy-crm").unwrap(),
            ImportParserProfile::csv_v1(b',', b'"').unwrap(),
        )
        .unwrap(),
        mapping: PartyImportMapping::try_new(
            None,
            "kind",
            "display_name",
            Some("legacy_customer_id".to_owned()),
            Some("row_key".to_owned()),
        )
        .unwrap(),
        partial_execution_policy: PartialExecutionPolicy::AllValidRows,
        occurred_at_unix_nanos: 1,
    })
    .unwrap()
}

#[test]
fn validation_progress_is_server_accumulated_before_exact_version_finalization() {
    let mut job = job();

    job.record_validation_batch(RecordImportValidationBatch {
        expected_version: 1,
        valid_rows: 1,
        invalid_rows: 0,
        occurred_at_unix_nanos: 2,
    })
    .unwrap();

    assert_eq!(job.status(), ImportJobStatus::Created);
    assert_eq!(job.valid_rows(), 1);
    assert_eq!(job.invalid_rows(), 0);
    assert_eq!(job.version(), 2);

    let incomplete = job
        .finalize_validation(FinalizeImportValidation {
            expected_version: 2,
            occurred_at_unix_nanos: 3,
        })
        .unwrap_err();
    assert_eq!(
        incomplete.code,
        "CUSTOMER_DATA_IMPORT_VALIDATION_INCOMPLETE"
    );

    job.record_validation_batch(RecordImportValidationBatch {
        expected_version: 2,
        valid_rows: 0,
        invalid_rows: 1,
        occurred_at_unix_nanos: 3,
    })
    .unwrap();

    assert_eq!(job.valid_rows(), 1);
    assert_eq!(job.invalid_rows(), 1);
    assert_eq!(job.version(), 3);

    job.finalize_validation(FinalizeImportValidation {
        expected_version: 3,
        occurred_at_unix_nanos: 4,
    })
    .unwrap();

    assert_eq!(job.status(), ImportJobStatus::Validated);
    assert_eq!(job.version(), 4);
}
