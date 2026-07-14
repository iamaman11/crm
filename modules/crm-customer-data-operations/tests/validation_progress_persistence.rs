use crm_customer_data_operations::{
    CreateImportJob, ImportJob, ImportJobId, ImportJobStatus, ImportParserProfile,
    PartialExecutionPolicy, PartyImportMapping, RecordImportValidationBatch, SourceDescriptor,
    SourceSystemId, decode_import_job_state, encode_import_job_state,
};

#[test]
fn partially_validated_created_job_round_trips_for_resumable_validation() {
    let mut job = ImportJob::create(CreateImportJob {
        job_id: ImportJobId::try_new("import-job-validation-resume-1").unwrap(),
        source: SourceDescriptor::try_new(
            "customers.csv",
            "11".repeat(32),
            3,
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
        partial_execution_policy: PartialExecutionPolicy::AllValidRows,
        occurred_at_unix_nanos: 10,
    })
    .unwrap();

    job.record_validation_batch(RecordImportValidationBatch {
        expected_version: 1,
        valid_rows: 1,
        invalid_rows: 1,
        occurred_at_unix_nanos: 20,
    })
    .unwrap();

    assert_eq!(job.status(), ImportJobStatus::Created);
    assert_eq!(job.version(), 2);
    assert_eq!(job.valid_rows(), 1);
    assert_eq!(job.invalid_rows(), 1);

    let encoded = encode_import_job_state(&job).unwrap();
    let restored = decode_import_job_state(&encoded).unwrap();

    assert_eq!(restored.status(), ImportJobStatus::Created);
    assert_eq!(restored.version(), 2);
    assert_eq!(restored.valid_rows(), 1);
    assert_eq!(restored.invalid_rows(), 1);
    assert_eq!(restored.checkpoint_row_position(), 0);
    assert_eq!(restored.succeeded_rows(), 0);
}
