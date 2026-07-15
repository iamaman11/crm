use crm_capability_runtime::CapabilityRisk;
use crm_customer_data_operations::{
    CreateImportJob, CreateImportRow, ImportJob, ImportJobId, ImportParserProfile, ImportRow,
    PartialExecutionPolicy, PartyImportMapping, SourceDescriptor, SourceSystemId,
};
use crm_customer_data_operations_capability_adapter::{
    CANCEL_PARTY_EXPORT_JOB_CAPABILITY, CANCEL_PARTY_IMPORT_JOB_CAPABILITY,
    CREATE_PARTY_EXPORT_JOB_CAPABILITY, CREATE_PARTY_IMPORT_JOB_CAPABILITY,
    FINALIZE_PARTY_IMPORT_VALIDATION_CAPABILITY, IMPORT_JOB_RECORD_TYPE, IMPORT_ROW_RECORD_TYPE,
    START_PARTY_EXPORT_EXECUTION_CAPABILITY, START_PARTY_IMPORT_EXECUTION_CAPABILITY,
    VALIDATE_PARTY_IMPORT_ROWS_CAPABILITY, capability_definitions, import_job_persisted_contract,
    import_row_persisted_contract, import_row_to_wire, job_to_wire,
};

fn source() -> SourceDescriptor {
    SourceDescriptor::try_new(
        "customers.csv",
        "11".repeat(32),
        2,
        SourceSystemId::try_new("legacy-crm").unwrap(),
        ImportParserProfile::csv_v1(b',', b'"').unwrap(),
    )
    .unwrap()
}

fn mapping() -> PartyImportMapping {
    PartyImportMapping::try_new(
        None,
        "kind",
        "display_name",
        Some("legacy_customer_id".to_owned()),
        Some("row_key".to_owned()),
    )
    .unwrap()
}

#[test]
fn publishes_exact_governed_mutation_surface() {
    let definitions = capability_definitions().unwrap();
    let ids = definitions
        .iter()
        .map(|definition| definition.capability_id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        ids,
        vec![
            CREATE_PARTY_IMPORT_JOB_CAPABILITY,
            VALIDATE_PARTY_IMPORT_ROWS_CAPABILITY,
            FINALIZE_PARTY_IMPORT_VALIDATION_CAPABILITY,
            START_PARTY_IMPORT_EXECUTION_CAPABILITY,
            CANCEL_PARTY_IMPORT_JOB_CAPABILITY,
            CREATE_PARTY_EXPORT_JOB_CAPABILITY,
            START_PARTY_EXPORT_EXECUTION_CAPABILITY,
            CANCEL_PARTY_EXPORT_JOB_CAPABILITY,
        ]
    );
    assert!(definitions.iter().all(|definition| definition.mutation));
    assert!(
        definitions
            .iter()
            .all(|definition| definition.requires_idempotency)
    );
    assert_eq!(definitions[3].risk, CapabilityRisk::High);
    assert_eq!(definitions[6].risk, CapabilityRisk::High);
}

#[test]
fn job_and_row_persistence_contracts_are_distinct_and_bounded() {
    let job = import_job_persisted_contract();
    let row = import_row_persisted_contract();

    assert_eq!(IMPORT_JOB_RECORD_TYPE, "customer_data.import_job");
    assert_eq!(IMPORT_ROW_RECORD_TYPE, "customer_data.import_row");
    assert_ne!(job.schema_id, row.schema_id);
    assert_ne!(job.descriptor_hash, row.descriptor_hash);
    assert!(job.maximum_size_bytes > row.maximum_size_bytes);
    assert_ne!(job.retention_policy_id, row.retention_policy_id);
}

#[test]
fn wire_job_preserves_parser_profile_and_source_system_identity() {
    let job = ImportJob::create(CreateImportJob {
        job_id: ImportJobId::try_new("import-job-1").unwrap(),
        source: source(),
        mapping: mapping(),
        partial_execution_policy: PartialExecutionPolicy::AllValidRows,
        occurred_at_unix_nanos: 1,
    })
    .unwrap();

    let wire = job_to_wire(&job).unwrap();
    let source = wire.source.unwrap();
    let profile = source.parser_profile.unwrap();

    assert_eq!(source.source_system_id, "legacy-crm");
    assert_eq!(source.content_sha256.len(), 32);
    assert_eq!(profile.delimiter_ascii, u32::from(b','));
    assert_eq!(profile.quote_ascii, u32::from(b'"'));
    assert_eq!(
        wire.mapping.unwrap().source_external_id_column.as_deref(),
        Some("legacy_customer_id")
    );
}

#[test]
fn wire_row_keeps_external_identifier_as_digest_not_party_identity() {
    let row = ImportRow::create(CreateImportRow {
        job_id: ImportJobId::try_new("import-job-1").unwrap(),
        row_position: 1,
        external_row_key: Some("row-1".to_owned()),
        source_external_id: Some("legacy-customer-42".to_owned()),
        occurred_at_unix_nanos: 1,
    })
    .unwrap();

    let wire = import_row_to_wire(&row).unwrap();
    assert_eq!(wire.source_external_id_sha256.len(), 64);
    assert!(wire.target_party_ref.is_none());
    assert_ne!(
        wire.source_external_id_sha256,
        row.derived_target_party_id().unwrap().as_str()
    );
}
