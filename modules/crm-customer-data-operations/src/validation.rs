use crate::{
    CreateImportRow, ImportJobId, ImportRow, ImportRowStatus, PreparedPartyRow, RowDiagnostic,
};
use crm_module_sdk::SdkError;

/// Initial validation outcome used when a validation batch first persists an import row.
///
/// The row is created directly in authoritative `Valid` or `Invalid` state at resource
/// version 1. This avoids manufacturing a pre-persistence `Pending v1 -> Valid/Invalid v2`
/// transition that never existed as a durable record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InitialImportRowValidation {
    Valid(PreparedPartyRow),
    Invalid(Vec<RowDiagnostic>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateValidatedImportRow {
    pub job_id: ImportJobId,
    pub row_position: u32,
    pub external_row_key: Option<String>,
    pub source_external_id: Option<String>,
    pub outcome: InitialImportRowValidation,
    pub occurred_at_unix_nanos: i64,
}

/// Creates the first durable representation of a source row after deterministic dry-run
/// validation. The resulting aggregate is already `Valid` or `Invalid` at version 1.
pub fn create_validated_import_row(
    command: CreateValidatedImportRow,
) -> Result<ImportRow, SdkError> {
    let pending = ImportRow::create(CreateImportRow {
        job_id: command.job_id,
        row_position: command.row_position,
        external_row_key: command.external_row_key,
        source_external_id: command.source_external_id,
        occurred_at_unix_nanos: command.occurred_at_unix_nanos,
    })?;
    let mut snapshot = pending.snapshot();
    match command.outcome {
        InitialImportRowValidation::Valid(prepared_party) => {
            snapshot.status = ImportRowStatus::Valid;
            snapshot.prepared_party = Some(prepared_party);
        }
        InitialImportRowValidation::Invalid(diagnostics) => {
            snapshot.status = ImportRowStatus::Invalid;
            snapshot.diagnostics = diagnostics;
        }
    }
    ImportRow::rehydrate(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PartyImportKind, TargetPartyId};

    #[test]
    fn valid_row_is_first_persisted_at_version_one() {
        let row = create_validated_import_row(CreateValidatedImportRow {
            job_id: ImportJobId::try_new("import-job-1").unwrap(),
            row_position: 1,
            external_row_key: Some("row-1".to_owned()),
            source_external_id: Some("legacy-42".to_owned()),
            outcome: InitialImportRowValidation::Valid(
                PreparedPartyRow::try_new(
                    TargetPartyId::try_new("party-1").unwrap(),
                    PartyImportKind::Person,
                    "Ada Lovelace",
                )
                .unwrap(),
            ),
            occurred_at_unix_nanos: 10,
        })
        .unwrap();

        assert_eq!(row.version(), 1);
        assert_eq!(row.status(), ImportRowStatus::Valid);
        assert!(row.source_external_id_sha256().is_some());
    }

    #[test]
    fn invalid_row_is_first_persisted_at_version_one() {
        let row = create_validated_import_row(CreateValidatedImportRow {
            job_id: ImportJobId::try_new("import-job-1").unwrap(),
            row_position: 2,
            external_row_key: None,
            source_external_id: None,
            outcome: InitialImportRowValidation::Invalid(vec![
                RowDiagnostic::try_new("PARTY_KIND_INVALID", "kind").unwrap(),
            ]),
            occurred_at_unix_nanos: 10,
        })
        .unwrap();

        assert_eq!(row.version(), 1);
        assert_eq!(row.status(), ImportRowStatus::Invalid);
    }
}
