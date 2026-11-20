use crm_customer_data_operations::{
    CreateImportJob, CreateValidatedImportRow, ImportJob, ImportJobId, ImportParserProfile,
    InitialImportRowValidation, MarkImportJobValidated, PartialExecutionPolicy, PartyImportKind,
    PartyImportMapping, PreparedPartyRow, RowDiagnostic, SourceDescriptor, SourceSystemId,
    StartImportExecution, TargetPartyId, create_validated_import_row,
};
use crm_customer_data_operations_execution_composition::{
    CONTRACT_VERSION, ImportExecutionSnapshot, party_create_invocation,
};
use crm_module_sdk::DataClass;
use crm_parties_capability_adapter::{
    CREATE_CAPABILITY as PARTY_CREATE_CAPABILITY,
    CREATE_REQUEST_SCHEMA as PARTY_CREATE_REQUEST_SCHEMA, MODULE_ID as PARTIES_MODULE_ID,
};

#[test]
fn public_composition_surface_builds_only_the_exact_governed_party_create_invocation() {
    let row = valid_row(
        ImportJobId::try_new("import-job-public-surface-1").unwrap(),
        1,
        "party-public-surface-1",
    );

    let invocation = party_create_invocation(&row).unwrap();

    assert_eq!(invocation.capability_id.as_str(), PARTY_CREATE_CAPABILITY);
    assert_eq!(invocation.capability_version.as_str(), CONTRACT_VERSION);
    assert_eq!(invocation.input.owner.as_str(), PARTIES_MODULE_ID);
    assert_eq!(
        invocation.input.schema_id.as_str(),
        PARTY_CREATE_REQUEST_SCHEMA
    );
    assert_eq!(invocation.input.data_class, DataClass::Personal);
    assert_eq!(
        invocation.input.owner.as_str(),
        "crm.parties",
        "execution composition must never own or directly persist Party state"
    );
}

#[test]
fn execution_snapshot_rejects_an_incomplete_authoritative_row_set_before_target_execution() {
    let job_id = ImportJobId::try_new("import-job-incomplete-row-set-1").unwrap();
    let job = executing_job(job_id.clone(), 2);
    let only_first_row = valid_row(job_id, 1, "party-incomplete-row-set-1");

    let error = ImportExecutionSnapshot::try_new(job, vec![only_first_row]).unwrap_err();

    assert_eq!(
        error.code,
        "CUSTOMER_DATA_IMPORT_EXECUTION_ROW_SET_INCOMPLETE"
    );
}

#[test]
fn invalid_row_cannot_build_a_target_party_invocation() {
    let row = create_validated_import_row(CreateValidatedImportRow {
        job_id: ImportJobId::try_new("import-job-invalid-target-1").unwrap(),
        row_position: 1,
        external_row_key: None,
        source_external_id: None,
        outcome: InitialImportRowValidation::Invalid(vec![
            RowDiagnostic::try_new("DISPLAY_NAME_MISSING", "display_name").unwrap(),
        ]),
        occurred_at_unix_nanos: 10,
    })
    .unwrap();

    let error = party_create_invocation(&row).unwrap_err();

    assert_eq!(
        error.code,
        "CUSTOMER_DATA_IMPORT_EXECUTION_ROW_NOT_EXECUTABLE"
    );
}

fn executing_job(job_id: ImportJobId, total_rows: u32) -> ImportJob {
    let mut job = ImportJob::create(CreateImportJob {
        job_id,
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
        partial_execution_policy: PartialExecutionPolicy::AllValidRows,
        occurred_at_unix_nanos: 10,
    })
    .unwrap();
    job.mark_validated(MarkImportJobValidated {
        expected_version: 1,
        valid_rows: total_rows,
        invalid_rows: 0,
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

fn valid_row(
    job_id: ImportJobId,
    row_position: u32,
    party_id: &str,
) -> crm_customer_data_operations::ImportRow {
    create_validated_import_row(CreateValidatedImportRow {
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
