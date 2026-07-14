use crm_customer_data_operations::{
    CreateValidatedImportRow, ImportJobId, InitialImportRowValidation, PartyImportKind,
    PreparedPartyRow, TargetPartyId, create_validated_import_row,
};
use crm_customer_data_operations_execution_composition::{
    CONTRACT_VERSION, party_create_invocation,
};
use crm_module_sdk::DataClass;
use crm_parties_capability_adapter::{
    CREATE_CAPABILITY as PARTY_CREATE_CAPABILITY, CREATE_REQUEST_SCHEMA as PARTY_CREATE_REQUEST_SCHEMA,
    MODULE_ID as PARTIES_MODULE_ID,
};

#[test]
fn public_composition_surface_builds_only_the_exact_governed_party_create_invocation() {
    let row = create_validated_import_row(CreateValidatedImportRow {
        job_id: ImportJobId::try_new("import-job-public-surface-1").unwrap(),
        row_position: 1,
        external_row_key: Some("row-1".to_owned()),
        source_external_id: Some("legacy-1".to_owned()),
        outcome: InitialImportRowValidation::Valid(
            PreparedPartyRow::try_new(
                TargetPartyId::try_new("party-public-surface-1").unwrap(),
                PartyImportKind::Person,
                "Ada Lovelace",
            )
            .unwrap(),
        ),
        occurred_at_unix_nanos: 10,
    })
    .unwrap();

    let invocation = party_create_invocation(&row).unwrap();

    assert_eq!(invocation.capability_id.as_str(), PARTY_CREATE_CAPABILITY);
    assert_eq!(invocation.capability_version.as_str(), CONTRACT_VERSION);
    assert_eq!(invocation.input.owner.as_str(), PARTIES_MODULE_ID);
    assert_eq!(invocation.input.schema_id.as_str(), PARTY_CREATE_REQUEST_SCHEMA);
    assert_eq!(invocation.input.data_class, DataClass::Personal);
    assert_eq!(
        invocation.input.owner.as_str(),
        "crm.parties",
        "execution composition must never own or directly persist Party state"
    );
}
