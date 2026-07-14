use crate::domain::{
    ImportJob, ImportJobId, ImportJobSnapshot, ImportJobStatus, ImportRow, ImportRowId,
    ImportRowSnapshot, ImportRowStatus, MappingVersionId, PartialExecutionPolicy, PartyImportKind,
    PartyImportMapping, PreparedPartyRow, RowDiagnostic, RowIdentitySource, SourceDescriptor,
    TargetPartyId,
};
use crm_module_sdk::{ErrorCategory, SdkError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const IMPORT_JOB_STATE_SCHEMA_ID: &str = "crm.customer-data-operations.import_job.state";
pub const IMPORT_JOB_STATE_SCHEMA_VERSION: &str = "1.0.0";
pub const IMPORT_JOB_STATE_MAXIMUM_BYTES: u64 = 512 * 1024;
pub const IMPORT_JOB_STATE_RETENTION_POLICY_ID: &str = "crm.customer_data.import_job";

pub const IMPORT_ROW_STATE_SCHEMA_ID: &str = "crm.customer-data-operations.import_row.state";
pub const IMPORT_ROW_STATE_SCHEMA_VERSION: &str = "1.0.0";
pub const IMPORT_ROW_STATE_MAXIMUM_BYTES: u64 = 128 * 1024;
pub const IMPORT_ROW_STATE_RETENTION_POLICY_ID: &str = "crm.customer_data.import_row";

const IMPORT_JOB_STATE_DESCRIPTOR: &[u8] = b"crm.customer-data-operations.import_job.state/v1:job_id,source[source_name,content_sha256,row_count],mapping[party_id_column,party_kind_column,display_name_column,external_row_key_column],mapping_version_id,partial_execution_policy,status,total_rows,valid_rows,invalid_rows,succeeded_rows,checkpoint_row_position,created_at_unix_nanos,updated_at_unix_nanos,version";
const IMPORT_ROW_STATE_DESCRIPTOR: &[u8] = b"crm.customer-data-operations.import_row.state/v1:row_id,job_id,row_position,identity_source, status,prepared_party[party_id,kind,display_name],diagnostics[code,field],execution_attempts,last_execution_error_code,target_party_id,created_at_unix_nanos,updated_at_unix_nanos,version";

pub fn import_job_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(IMPORT_JOB_STATE_DESCRIPTOR).into()
}

pub fn import_row_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(IMPORT_ROW_STATE_DESCRIPTOR).into()
}

pub fn encode_import_job_state(job: &ImportJob) -> Result<Vec<u8>, SdkError> {
    let bytes = serde_json::to_vec(&ImportJobStateV1::from(job.snapshot())).map_err(|error| {
        persisted_error(format!("import-job state serialization failed: {error}"))
    })?;
    validate_size(&bytes, IMPORT_JOB_STATE_MAXIMUM_BYTES, "import-job")?;
    Ok(bytes)
}

pub fn decode_import_job_state(bytes: &[u8]) -> Result<ImportJob, SdkError> {
    validate_size(bytes, IMPORT_JOB_STATE_MAXIMUM_BYTES, "import-job")?;
    let state: ImportJobStateV1 = serde_json::from_slice(bytes)
        .map_err(|error| persisted_error(format!("import-job state JSON is invalid: {error}")))?;
    require_canonical_json(bytes, &state, "import-job")?;
    ImportJob::rehydrate(state.try_into()?)
        .map_err(|error| persisted_error(format!("{}: {}", error.code, error.safe_message)))
}

pub fn encode_import_row_state(row: &ImportRow) -> Result<Vec<u8>, SdkError> {
    let bytes = serde_json::to_vec(&ImportRowStateV1::from(row.snapshot())).map_err(|error| {
        persisted_error(format!("import-row state serialization failed: {error}"))
    })?;
    validate_size(&bytes, IMPORT_ROW_STATE_MAXIMUM_BYTES, "import-row")?;
    Ok(bytes)
}

pub fn decode_import_row_state(bytes: &[u8]) -> Result<ImportRow, SdkError> {
    validate_size(bytes, IMPORT_ROW_STATE_MAXIMUM_BYTES, "import-row")?;
    let state: ImportRowStateV1 = serde_json::from_slice(bytes)
        .map_err(|error| persisted_error(format!("import-row state JSON is invalid: {error}")))?;
    require_canonical_json(bytes, &state, "import-row")?;
    ImportRow::rehydrate(state.try_into()?)
        .map_err(|error| persisted_error(format!("{}: {}", error.code, error.safe_message)))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ImportJobStateV1 {
    job_id: String,
    source: SourceDescriptorStateV1,
    mapping: PartyImportMappingStateV1,
    mapping_version_id: String,
    partial_execution_policy: PartialExecutionPolicyState,
    status: ImportJobStatusState,
    total_rows: u32,
    valid_rows: u32,
    invalid_rows: u32,
    succeeded_rows: u32,
    checkpoint_row_position: u32,
    created_at_unix_nanos: i64,
    updated_at_unix_nanos: i64,
    version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceDescriptorStateV1 {
    source_name: String,
    content_sha256: String,
    row_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartyImportMappingStateV1 {
    party_id_column: String,
    party_kind_column: String,
    display_name_column: String,
    external_row_key_column: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PartialExecutionPolicyState {
    AllValidRows,
    RequireAllValid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ImportJobStatusState {
    Created,
    Validated,
    Executing,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ImportRowStateV1 {
    row_id: String,
    job_id: String,
    row_position: u32,
    identity_source: RowIdentitySourceState,
    status: ImportRowStatusState,
    prepared_party: Option<PreparedPartyStateV1>,
    diagnostics: Vec<RowDiagnosticStateV1>,
    execution_attempts: u32,
    last_execution_error_code: Option<String>,
    target_party_id: Option<String>,
    created_at_unix_nanos: i64,
    updated_at_unix_nanos: i64,
    version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum RowIdentitySourceState {
    Position(u32),
    ExternalKeySha256(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ImportRowStatusState {
    Pending,
    Valid,
    Invalid,
    FailedRetryable,
    Succeeded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PreparedPartyStateV1 {
    party_id: String,
    kind: PartyImportKindState,
    display_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PartyImportKindState {
    Person,
    Organization,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RowDiagnosticStateV1 {
    code: String,
    field: String,
}

impl From<ImportJobSnapshot> for ImportJobStateV1 {
    fn from(value: ImportJobSnapshot) -> Self {
        Self {
            job_id: value.job_id.as_str().to_owned(),
            source: SourceDescriptorStateV1::from(value.source),
            mapping: PartyImportMappingStateV1::from(value.mapping),
            mapping_version_id: value.mapping_version_id.as_str().to_owned(),
            partial_execution_policy: value.partial_execution_policy.into(),
            status: value.status.into(),
            total_rows: value.total_rows,
            valid_rows: value.valid_rows,
            invalid_rows: value.invalid_rows,
            succeeded_rows: value.succeeded_rows,
            checkpoint_row_position: value.checkpoint_row_position,
            created_at_unix_nanos: value.created_at_unix_nanos,
            updated_at_unix_nanos: value.updated_at_unix_nanos,
            version: value.version,
        }
    }
}

impl TryFrom<ImportJobStateV1> for ImportJobSnapshot {
    type Error = SdkError;

    fn try_from(value: ImportJobStateV1) -> Result<Self, Self::Error> {
        let job_id = parse_canonical_record_id(
            value.job_id,
            ImportJobId::try_new,
            |value| value.as_str(),
            "import-job ID",
        )?;
        let source = value.source.try_into()?;
        let mapping: PartyImportMapping = value.mapping.try_into()?;
        let mapping_version_id = parse_canonical_record_id(
            value.mapping_version_id,
            MappingVersionId::try_new,
            |value| value.as_str(),
            "mapping-version ID",
        )?;

        Ok(Self {
            job_id,
            source,
            mapping,
            mapping_version_id,
            partial_execution_policy: value.partial_execution_policy.into(),
            status: value.status.into(),
            total_rows: value.total_rows,
            valid_rows: value.valid_rows,
            invalid_rows: value.invalid_rows,
            succeeded_rows: value.succeeded_rows,
            checkpoint_row_position: value.checkpoint_row_position,
            created_at_unix_nanos: value.created_at_unix_nanos,
            updated_at_unix_nanos: value.updated_at_unix_nanos,
            version: value.version,
        })
    }
}

impl From<SourceDescriptor> for SourceDescriptorStateV1 {
    fn from(value: SourceDescriptor) -> Self {
        Self {
            source_name: value.source_name().to_owned(),
            content_sha256: value.content_sha256().to_owned(),
            row_count: value.row_count(),
        }
    }
}

impl TryFrom<SourceDescriptorStateV1> for SourceDescriptor {
    type Error = SdkError;

    fn try_from(value: SourceDescriptorStateV1) -> Result<Self, Self::Error> {
        let source = SourceDescriptor::try_new(
            value.source_name.clone(),
            value.content_sha256.clone(),
            value.row_count,
        )
        .map_err(|error| persisted_error(error.to_string()))?;
        if source.source_name() != value.source_name
            || source.content_sha256() != value.content_sha256
        {
            return Err(persisted_error(
                "persisted source descriptor is not canonical",
            ));
        }
        Ok(source)
    }
}

impl From<PartyImportMapping> for PartyImportMappingStateV1 {
    fn from(value: PartyImportMapping) -> Self {
        Self {
            party_id_column: value.party_id_column().to_owned(),
            party_kind_column: value.party_kind_column().to_owned(),
            display_name_column: value.display_name_column().to_owned(),
            external_row_key_column: value.external_row_key_column().map(str::to_owned),
        }
    }
}

impl TryFrom<PartyImportMappingStateV1> for PartyImportMapping {
    type Error = SdkError;

    fn try_from(value: PartyImportMappingStateV1) -> Result<Self, Self::Error> {
        let mapping = PartyImportMapping::try_new(
            value.party_id_column.clone(),
            value.party_kind_column.clone(),
            value.display_name_column.clone(),
            value.external_row_key_column.clone(),
        )
        .map_err(|error| persisted_error(error.to_string()))?;
        if mapping.party_id_column() != value.party_id_column
            || mapping.party_kind_column() != value.party_kind_column
            || mapping.display_name_column() != value.display_name_column
            || mapping.external_row_key_column() != value.external_row_key_column.as_deref()
        {
            return Err(persisted_error(
                "persisted Party import mapping is not canonical",
            ));
        }
        Ok(mapping)
    }
}

impl From<ImportRowSnapshot> for ImportRowStateV1 {
    fn from(value: ImportRowSnapshot) -> Self {
        Self {
            row_id: value.row_id.as_str().to_owned(),
            job_id: value.job_id.as_str().to_owned(),
            row_position: value.row_position,
            identity_source: value.identity_source.into(),
            status: value.status.into(),
            prepared_party: value.prepared_party.map(PreparedPartyStateV1::from),
            diagnostics: value
                .diagnostics
                .into_iter()
                .map(RowDiagnosticStateV1::from)
                .collect(),
            execution_attempts: value.execution_attempts,
            last_execution_error_code: value.last_execution_error_code,
            target_party_id: value.target_party_id.map(|value| value.as_str().to_owned()),
            created_at_unix_nanos: value.created_at_unix_nanos,
            updated_at_unix_nanos: value.updated_at_unix_nanos,
            version: value.version,
        }
    }
}

impl TryFrom<ImportRowStateV1> for ImportRowSnapshot {
    type Error = SdkError;

    fn try_from(value: ImportRowStateV1) -> Result<Self, Self::Error> {
        let row_id = parse_canonical_record_id(
            value.row_id,
            ImportRowId::try_new,
            |value| value.as_str(),
            "import-row ID",
        )?;
        let job_id = parse_canonical_record_id(
            value.job_id,
            ImportJobId::try_new,
            |value| value.as_str(),
            "import-job ID",
        )?;
        let target_party_id = value
            .target_party_id
            .map(|raw| {
                parse_canonical_record_id(
                    raw,
                    TargetPartyId::try_new,
                    |value| value.as_str(),
                    "target Party ID",
                )
            })
            .transpose()?;
        let prepared_party = value
            .prepared_party
            .map(PreparedPartyStateV1::try_into)
            .transpose()?;
        let diagnostics = value
            .diagnostics
            .into_iter()
            .map(RowDiagnosticStateV1::try_into)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            row_id,
            job_id,
            row_position: value.row_position,
            identity_source: value.identity_source.try_into()?,
            status: value.status.into(),
            prepared_party,
            diagnostics,
            execution_attempts: value.execution_attempts,
            last_execution_error_code: value.last_execution_error_code,
            target_party_id,
            created_at_unix_nanos: value.created_at_unix_nanos,
            updated_at_unix_nanos: value.updated_at_unix_nanos,
            version: value.version,
        })
    }
}

impl From<RowIdentitySource> for RowIdentitySourceState {
    fn from(value: RowIdentitySource) -> Self {
        match value {
            RowIdentitySource::Position(position) => Self::Position(position),
            RowIdentitySource::ExternalKeySha256(value) => Self::ExternalKeySha256(value),
        }
    }
}

impl TryFrom<RowIdentitySourceState> for RowIdentitySource {
    type Error = SdkError;

    fn try_from(value: RowIdentitySourceState) -> Result<Self, Self::Error> {
        match value {
            RowIdentitySourceState::Position(position) => Ok(Self::Position(position)),
            RowIdentitySourceState::ExternalKeySha256(value) => {
                if value.len() != 64
                    || value
                        .as_bytes()
                        .iter()
                        .any(|byte| !byte.is_ascii_hexdigit() || byte.is_ascii_uppercase())
                {
                    return Err(persisted_error(
                        "persisted external-row-key digest is not canonical lowercase SHA-256",
                    ));
                }
                Ok(Self::ExternalKeySha256(value))
            }
        }
    }
}

impl From<PreparedPartyRow> for PreparedPartyStateV1 {
    fn from(value: PreparedPartyRow) -> Self {
        Self {
            party_id: value.party_id().as_str().to_owned(),
            kind: value.kind().into(),
            display_name: value.display_name().to_owned(),
        }
    }
}

impl TryFrom<PreparedPartyStateV1> for PreparedPartyRow {
    type Error = SdkError;

    fn try_from(value: PreparedPartyStateV1) -> Result<Self, Self::Error> {
        let party_id = parse_canonical_record_id(
            value.party_id,
            TargetPartyId::try_new,
            |value| value.as_str(),
            "prepared Party ID",
        )?;
        let prepared =
            PreparedPartyRow::try_new(party_id, value.kind.into(), value.display_name.clone())
                .map_err(|error| persisted_error(error.to_string()))?;
        if prepared.display_name() != value.display_name {
            return Err(persisted_error(
                "persisted prepared Party row is not canonical",
            ));
        }
        Ok(prepared)
    }
}

impl From<RowDiagnostic> for RowDiagnosticStateV1 {
    fn from(value: RowDiagnostic) -> Self {
        Self {
            code: value.code().to_owned(),
            field: value.field().to_owned(),
        }
    }
}

impl TryFrom<RowDiagnosticStateV1> for RowDiagnostic {
    type Error = SdkError;

    fn try_from(value: RowDiagnosticStateV1) -> Result<Self, Self::Error> {
        let diagnostic = RowDiagnostic::try_new(value.code.clone(), value.field.clone())
            .map_err(|error| persisted_error(error.to_string()))?;
        if diagnostic.code() != value.code || diagnostic.field() != value.field {
            return Err(persisted_error("persisted row diagnostic is not canonical"));
        }
        Ok(diagnostic)
    }
}

impl From<PartialExecutionPolicy> for PartialExecutionPolicyState {
    fn from(value: PartialExecutionPolicy) -> Self {
        match value {
            PartialExecutionPolicy::AllValidRows => Self::AllValidRows,
            PartialExecutionPolicy::RequireAllValid => Self::RequireAllValid,
        }
    }
}

impl From<PartialExecutionPolicyState> for PartialExecutionPolicy {
    fn from(value: PartialExecutionPolicyState) -> Self {
        match value {
            PartialExecutionPolicyState::AllValidRows => Self::AllValidRows,
            PartialExecutionPolicyState::RequireAllValid => Self::RequireAllValid,
        }
    }
}

impl From<ImportJobStatus> for ImportJobStatusState {
    fn from(value: ImportJobStatus) -> Self {
        match value {
            ImportJobStatus::Created => Self::Created,
            ImportJobStatus::Validated => Self::Validated,
            ImportJobStatus::Executing => Self::Executing,
            ImportJobStatus::Completed => Self::Completed,
            ImportJobStatus::Cancelled => Self::Cancelled,
        }
    }
}

impl From<ImportJobStatusState> for ImportJobStatus {
    fn from(value: ImportJobStatusState) -> Self {
        match value {
            ImportJobStatusState::Created => Self::Created,
            ImportJobStatusState::Validated => Self::Validated,
            ImportJobStatusState::Executing => Self::Executing,
            ImportJobStatusState::Completed => Self::Completed,
            ImportJobStatusState::Cancelled => Self::Cancelled,
        }
    }
}

impl From<ImportRowStatus> for ImportRowStatusState {
    fn from(value: ImportRowStatus) -> Self {
        match value {
            ImportRowStatus::Pending => Self::Pending,
            ImportRowStatus::Valid => Self::Valid,
            ImportRowStatus::Invalid => Self::Invalid,
            ImportRowStatus::FailedRetryable => Self::FailedRetryable,
            ImportRowStatus::Succeeded => Self::Succeeded,
        }
    }
}

impl From<ImportRowStatusState> for ImportRowStatus {
    fn from(value: ImportRowStatusState) -> Self {
        match value {
            ImportRowStatusState::Pending => Self::Pending,
            ImportRowStatusState::Valid => Self::Valid,
            ImportRowStatusState::Invalid => Self::Invalid,
            ImportRowStatusState::FailedRetryable => Self::FailedRetryable,
            ImportRowStatusState::Succeeded => Self::Succeeded,
        }
    }
}

impl From<PartyImportKind> for PartyImportKindState {
    fn from(value: PartyImportKind) -> Self {
        match value {
            PartyImportKind::Person => Self::Person,
            PartyImportKind::Organization => Self::Organization,
        }
    }
}

impl From<PartyImportKindState> for PartyImportKind {
    fn from(value: PartyImportKindState) -> Self {
        match value {
            PartyImportKindState::Person => Self::Person,
            PartyImportKindState::Organization => Self::Organization,
        }
    }
}

fn parse_canonical_record_id<T, Parse, View>(
    raw: String,
    parse: Parse,
    view: View,
    label: &str,
) -> Result<T, SdkError>
where
    Parse: FnOnce(String) -> Result<T, SdkError>,
    View: Fn(&T) -> &str,
{
    let parsed = parse(raw.clone()).map_err(|error| persisted_error(error.to_string()))?;
    if view(&parsed) != raw {
        return Err(persisted_error(format!(
            "persisted {label} is not canonical"
        )));
    }
    Ok(parsed)
}

fn require_canonical_json<T: Serialize>(
    original: &[u8],
    state: &T,
    label: &str,
) -> Result<(), SdkError> {
    let canonical = serde_json::to_vec(state).map_err(|error| {
        persisted_error(format!("{label} canonical serialization failed: {error}"))
    })?;
    if canonical != original {
        return Err(persisted_error(format!(
            "persisted {label} state is not in canonical JSON representation"
        )));
    }
    Ok(())
}

fn validate_size(bytes: &[u8], maximum_bytes: u64, label: &str) -> Result<(), SdkError> {
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum_bytes {
        return Err(persisted_error(format!(
            "{label} state exceeds the maximum of {maximum_bytes} bytes"
        )));
    }
    Ok(())
}

fn persisted_error(message: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_PERSISTED_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "The persisted customer-data import state is invalid.",
    )
    .with_internal_reference(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        CreateImportJob, CreateImportRow, MarkImportJobValidated, PartialExecutionPolicy,
        ValidateImportRowFailure,
    };

    fn source(rows: u32) -> SourceDescriptor {
        SourceDescriptor::try_new(
            "customers.csv",
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            rows,
        )
        .unwrap()
    }

    fn mapping() -> PartyImportMapping {
        PartyImportMapping::try_new(
            "party_id",
            "kind",
            "display_name",
            Some("external_id".to_owned()),
        )
        .unwrap()
    }

    #[test]
    fn import_job_state_round_trip_is_deterministic() {
        let mut job = ImportJob::create(CreateImportJob {
            job_id: ImportJobId::try_new("import-job-1").unwrap(),
            source: source(2),
            mapping: mapping(),
            partial_execution_policy: PartialExecutionPolicy::AllValidRows,
            occurred_at_unix_nanos: 100,
        })
        .unwrap();
        job.mark_validated(MarkImportJobValidated {
            expected_version: 1,
            valid_rows: 1,
            invalid_rows: 1,
            occurred_at_unix_nanos: 200,
        })
        .unwrap();

        let first = encode_import_job_state(&job).unwrap();
        let decoded = decode_import_job_state(&first).unwrap();
        let second = encode_import_job_state(&decoded).unwrap();
        assert_eq!(first, second);
        assert_eq!(decoded.snapshot(), job.snapshot());
    }

    #[test]
    fn import_row_state_round_trip_preserves_invalid_evidence() {
        let mut row = ImportRow::create(CreateImportRow {
            job_id: ImportJobId::try_new("import-job-1").unwrap(),
            row_position: 1,
            external_row_key: Some("customer-42".to_owned()),
            occurred_at_unix_nanos: 100,
        })
        .unwrap();
        row.mark_invalid(ValidateImportRowFailure {
            expected_version: 1,
            diagnostics: vec![RowDiagnostic::try_new("mapping.missing", "display_name").unwrap()],
            occurred_at_unix_nanos: 200,
        })
        .unwrap();

        let first = encode_import_row_state(&row).unwrap();
        let decoded = decode_import_row_state(&first).unwrap();
        let second = encode_import_row_state(&decoded).unwrap();
        assert_eq!(first, second);
        assert_eq!(decoded.snapshot(), row.snapshot());
    }

    #[test]
    fn unknown_fields_and_noncanonical_json_are_rejected() {
        let job = ImportJob::create(CreateImportJob {
            job_id: ImportJobId::try_new("import-job-1").unwrap(),
            source: source(1),
            mapping: mapping(),
            partial_execution_policy: PartialExecutionPolicy::RequireAllValid,
            occurred_at_unix_nanos: 100,
        })
        .unwrap();
        let bytes = encode_import_job_state(&job).unwrap();
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["unexpected"] = serde_json::json!(true);
        assert_eq!(
            decode_import_job_state(&serde_json::to_vec(&value).unwrap())
                .unwrap_err()
                .code,
            "CUSTOMER_DATA_PERSISTED_STATE_INVALID"
        );

        let pretty = serde_json::to_vec_pretty(
            &serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
        )
        .unwrap();
        assert_eq!(
            decode_import_job_state(&pretty).unwrap_err().code,
            "CUSTOMER_DATA_PERSISTED_STATE_INVALID"
        );
    }

    #[test]
    fn semantically_noncanonical_source_is_rejected() {
        let job = ImportJob::create(CreateImportJob {
            job_id: ImportJobId::try_new("import-job-1").unwrap(),
            source: source(1),
            mapping: mapping(),
            partial_execution_policy: PartialExecutionPolicy::RequireAllValid,
            occurred_at_unix_nanos: 100,
        })
        .unwrap();
        let bytes = encode_import_job_state(&job).unwrap();
        let mut state: ImportJobStateV1 = serde_json::from_slice(&bytes).unwrap();
        state.source.source_name = " customers.csv ".to_owned();
        let tampered = serde_json::to_vec(&state).unwrap();
        assert_eq!(
            decode_import_job_state(&tampered).unwrap_err().code,
            "CUSTOMER_DATA_PERSISTED_STATE_INVALID"
        );
    }
}
