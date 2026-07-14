use crm_customer_data_operations::{
    CreateImportJob, ImportJob, ImportJobId, ImportParserProfile, PartialExecutionPolicy,
    PartyImportMapping, SourceDescriptor, SourceSystemId, decode_import_job_state,
    encode_import_job_state,
};
use crm_module_sdk::FileId;

#[test]
fn bound_source_artifact_identity_survives_strict_import_job_persistence() {
    let source = SourceDescriptor::try_new_bound(
        FileId::try_new("source-artifact-001").unwrap(),
        "customers.csv",
        "11".repeat(32),
        2,
        SourceSystemId::try_new("legacy-crm").unwrap(),
        ImportParserProfile::csv_v1(b',', b'"').unwrap(),
    )
    .unwrap();
    let job = ImportJob::create(CreateImportJob {
        job_id: ImportJobId::try_new("import-job-bound-source-001").unwrap(),
        source,
        mapping: PartyImportMapping::try_new(
            None,
            "kind",
            "display_name",
            Some("legacy_id".to_owned()),
            None,
        )
        .unwrap(),
        partial_execution_policy: PartialExecutionPolicy::AllValidRows,
        occurred_at_unix_nanos: 10,
    })
    .unwrap();

    let encoded = encode_import_job_state(&job).unwrap();
    let restored = decode_import_job_state(&encoded).unwrap();

    assert_eq!(
        restored.source().source_artifact_id().unwrap().as_str(),
        "source-artifact-001"
    );
    assert_eq!(restored.source().content_sha256(), "11".repeat(32));
}
