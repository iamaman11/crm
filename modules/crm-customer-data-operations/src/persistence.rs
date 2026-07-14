use crate::domain::{
    ImportJob, ImportJobId, ImportJobSnapshot, ImportJobStatus, ImportRow, ImportRowId,
    ImportRowSnapshot, ImportRowStatus, MappingVersionId, PartialExecutionPolicy, PartyImportKind,
    PartyImportMapping, PreparedPartyRow, RowDiagnostic, RowIdentitySource, SourceDescriptor,
    TargetPartyId,
};
use crate::profile::{
    ExternalPartyIdentifierDigest, ImportCanonicalizationVersion, ImportHeaderMode,
    ImportParserProfile, ImportParserVersion, ImportSourceFormat, ImportTextEncoding, SourceSystemId,
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

const IMPORT_JOB_STATE_DESCRIPTOR: &[u8] = b"crm.customer-data-operations.import_job.state/v1:job_id,source[source_name,content_sha256,row_count,source_system_id,parser_profile[format,encoding,delimiter_ascii,quote_ascii,header_mode,parser_version,canonicalization_version]],mapping[target_party_id_column,party_kind_column,display_name_column,source_external_id_column,external_row_key_column],mapping_version_id,partial_execution_policy,status,total_rows,valid_rows,invalid_rows,succeeded_rows,checkpoint_row_position,created_at_unix_nanos,updated_at_unix_nanos,version";
const IMPORT_ROW_STATE_DESCRIPTOR: &[u8] = b"crm.customer-data-operations.import_row.state/v1:row_id,job_id,row_position,identity_source,source_external_id_sha256,status,prepared_party[party_id,kind,display_name],diagnostics[code,field],execution_attempts,last_execution_error_code,target_party_id,created_at_unix_nanos,updated_at_unix_nanos,version";

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
    source_system_id: String,
    parser_profile: ImportParserProfileStateV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ImportParserProfileStateV1 {
    format: String,
    encoding: String,
    delimiter_ascii: u8,
    quote_ascii: u8,
    header_mode: String,
    parser_version: String,
    canonicalization_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartyImportMappingStateV1 {
    target_party_id_column: Option<String>,
    party_kind_column: String,
    display_name_column: String,
    source_external_id_column: Option<String>,
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
    source_external_id_sha256: Option<String>,
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
#[serde(tag = "kind", content = "value", rename_all = "snake_case", deny_unknown_fields)]
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
            source: value.source.into(),
            mapping: value.mapping.into(),
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
        let job_id = parse_record_id(value.job_id, ImportJobId::try_new, "import-job ID")?;
        let source = value.source.try_into()?;
        let mapping: PartyImportMapping = value.mapping.try_into()?;
        let mapping_version_id = parse_record_id(
            value.mapping_version_id,
            MappingVersionId::try_new,
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
            source_system_id: value.source_system_id().as_str().to_owned(),
            parser_profile: value.parser_profile().clone().into(),
        }
    }
}

impl TryFrom<SourceDescriptorStateV1> for SourceDescriptor {
    type Error = SdkError;

    fn try_from(value: SourceDescriptorStateV1) -> Result<Self, Self::Error> {
        let source_system_id = SourceSystemId::try_new(value.source_system_id.clone())
            .map_err(|error| persisted_error(error.to_string()))?;
        let parser_profile: ImportParserProfile = value.parser_profile.try_into()?;
        let source = SourceDescriptor::try_new(
            value.source_name.clone(),
            value.content_sha256.clone(),
            value.row_count,
            source_system_id,
            parser_profile,
        )
        .map_err(|error| persisted_error(error.to_string()))?;
        if source.source_name() != value.source_name
            || source.content_sha256() != value.content_sha256
            || source.source_system_id().as_str() != value.source_system_id
        {
            return Err(persisted_error("persisted source descriptor is not canonical"));
        }
        Ok(source)
    }
}

impl From<ImportParserProfile> for ImportParserProfileStateV1 {
    fn from(value: ImportParserProfile) -> Self {
        Self {
            format: value.format().code().to_owned(),
            encoding: value.encoding().code().to_owned(),
            delimiter_ascii: value.delimiter(),
            quote_ascii: value.quote(),
            header_mode: value.header_mode().code().to_owned(),
            parser_version: value.parser_version().code().to_owned(),
            canonicalization_version: value.canonicalization_version().code().to_owned(),
        }
    }
}

impl TryFrom<ImportParserProfileStateV1> for ImportParserProfile {
    type Error = SdkError;

    fn try_from(value: ImportParserProfileStateV1) -> Result<Self, Self::Error> {
        let format = match value.format.as_str() {
            "csv" => ImportSourceFormat::Csv,
            _ => return Err(persisted_error("persisted parser format is unsupported")),
        };
        let encoding = match value.encoding.as_str() {
            "utf-8" => ImportTextEncoding::Utf8,
            _ => return Err(persisted_error("persisted parser encoding is unsupported")),
        };
        let header_mode = match value.header_mode.as_str() {
            "required-first-row" => ImportHeaderMode::RequiredFirstRow,
            _ => return Err(persisted_error("persisted parser header mode is unsupported")),
        };
        let parser_version = match value.parser_version.as_str() {
            "csv-v1" => ImportParserVersion::CsvV1,
            _ => return Err(persisted_error("persisted parser version is unsupported")),
        };
        let canonicalization_version = match value.canonicalization_version.as_str() {
            "customer-import-v1" => ImportCanonicalizationVersion::V1,
            _ => {
                return Err(persisted_error(
                    "persisted parser canonicalization version is unsupported",
                ))
            }
        };
        let profile = ImportParserProfile::try_new(
            format,
            encoding,
            value.delimiter_ascii,
            value.quote_ascii,
            header_mode,
            parser_version,
            canonicalization_version,
        )
        .map_err(|error| persisted_error(error.to_string()))?;
        let canonical: ImportParserProfileStateV1 = profile.clone().into();
        if canonical != value {
            return Err(persisted_error("persisted parser profile is not canonical"));
        }
        Ok(profile)
    }
}

impl From<PartyImportMapping> for PartyImportMappingStateV1 {
    fn from(value: PartyImportMapping) -> Self {
        Self {
            target_party_id_column: value.target_party_id_column().map(str::to_owned),
            party_kind_column: value.party_kind_column().to_owned(),
            display_name_column: value.display_name_column().to_owned(),
            source_external_id_column: value.source_external_id_column().map(str::to_owned),
            external_row_key_column: value.external_row_key_column().map(str::to_owned),
        }
    }
}

impl TryFrom<PartyImportMappingStateV1> for PartyImportMapping {
    type Error = SdkError;

    fn try_from(value: PartyImportMappingStateV1) -> Result<Self, Self::Error> {
        let mapping = PartyImportMapping::try_new(
            value.target_party_id_column.clone(),
            value.party_kind_column.clone(),
            value.display_name_column.clone(),
            value.source_external_id_column.clone(),
            value.external_row_key_column.clone(),
        )
        .map_err(|error| persisted_error(error.to_string()))?;
        if mapping.target_party_id_column() != value.target_party_id_column.as_deref()
            || mapping.party_kind_column() != value.party_kind_column
            || mapping.display_name_column() != value.display_name_column
            || mapping.source_external_id_column() != value.source_external_id_column.as_deref()
            || mapping.external_row_key_column() != value.external_row_key_column.as_deref()
        {
            return Err(persisted_error("persisted Party import mapping is not canonical"));
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
            source_external_id_sha256: value
                .source_external_id_sha256
                .map(|value| value.as_str().to_owned()),
            status: value.status.into(),
            prepared_party: value.prepared_party.map(Into::into),
            diagnostics: value.diagnostics.into_iter().map(Into::into).collect(),
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
        let row_id = parse_record_id(value.row_id, ImportRowId::try_new, "import-row ID")?;
        let job_id = parse_record_id(value.job_id, ImportJobId::try_new, "import-job ID")?;
        let source_external_id_sha256 = value
            .source_external_id_sha256
            .map(ExternalPartyIdentifierDigest::try_from_sha256)
            .transpose()
            .map_err(|error| persisted_error(error.to_string()))?;
        let target_party_id = value
            .target_party_id
            .map(|raw| parse_record_id(raw, TargetPartyId::try_new, "target Party ID"))
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
            source_external_id_sha256,
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
        Ok(match value {
            RowIdentitySourceState::Position(position) => Self::Position(position),
            RowIdentitySourceState::ExternalKeySha256(value) => {
                if value.len() != 64
                    || value
                        .as_bytes()
                        .iter()
                        .any(|byte| !byte.is_ascii_hexdigit() || byte.is_ascii_uppercase())
                {
                    return Err(persisted_error(
                        "persisted external row-key digest is not canonical SHA-256",
                    ));
                }
                Self::ExternalKeySha256(value)
            }
        })
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
        let party_id = parse_record_id(value.party_id, TargetPartyId::try_new, "prepared Party ID")?;
        PreparedPartyRow::try_new(party_id, value.kind.into(), value.display_name)
            .map_err(|error| persisted_error(error.to_string()))
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
        RowDiagnostic::try_new(value.code, value.field)
            .map_err(|error| persisted_error(error.to_string()))
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

fn parse_record_id<T>(
    raw: String,
    parse: impl FnOnce(String) -> Result<T, SdkError>,
    label: &'static str,
) -> Result<T, SdkError> {
    parse(raw).map_err(|error| persisted_error(format!("persisted {label} is invalid: {error}")))
}

fn require_canonical_json<T>(bytes: &[u8], state: &T, label: &str) -> Result<(), SdkError>
where
    T: Serialize,
{
    let canonical = serde_json::to_vec(state)
        .map_err(|error| persisted_error(format!("{label} canonical JSON failed: {error}")))?;
    if canonical != bytes {
        return Err(persisted_error(format!(
            "persisted {label} state is not canonical JSON"
        )));
    }
    Ok(())
}

fn validate_size(bytes: &[u8], maximum: u64, label: &str) -> Result<(), SdkError> {
    let size = u64::try_from(bytes.len())
        .map_err(|_| persisted_error(format!("{label} state size does not fit u64")))?;
    if size > maximum {
        return Err(persisted_error(format!(
            "persisted {label} state exceeds maximum size {maximum}"
        )));
    }
    Ok(())
}

fn persisted_error(internal: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_IMPORT_PERSISTED_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "Persisted customer-data import state is invalid.",
    )
    .with_internal_reference(internal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{CreateImportJob, CreateImportRow};

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
    fn job_state_round_trip_preserves_source_interpretation_identity() {
        let job = ImportJob::create(CreateImportJob {
            job_id: ImportJobId::try_new("import-job-1").unwrap(),
            source: source(),
            mapping: mapping(),
            partial_execution_policy: PartialExecutionPolicy::AllValidRows,
            occurred_at_unix_nanos: 1,
        })
        .unwrap();
        let bytes = encode_import_job_state(&job).unwrap();
        let decoded = decode_import_job_state(&bytes).unwrap();
        assert_eq!(decoded.snapshot(), job.snapshot());
        assert_eq!(decoded.source().source_system_id().as_str(), "legacy-crm");
        assert_eq!(decoded.source().parser_profile().parser_version(), ImportParserVersion::CsvV1);
    }

    #[test]
    fn row_state_round_trip_preserves_external_identifier_digest() {
        let row = ImportRow::create(CreateImportRow {
            job_id: ImportJobId::try_new("import-job-1").unwrap(),
            row_position: 1,
            external_row_key: Some("row-1".to_owned()),
            source_external_id: Some("legacy-customer-42".to_owned()),
            occurred_at_unix_nanos: 1,
        })
        .unwrap();
        let bytes = encode_import_row_state(&row).unwrap();
        let decoded = decode_import_row_state(&bytes).unwrap();
        assert_eq!(decoded.snapshot(), row.snapshot());
        assert!(decoded.source_external_id_sha256().is_some());
    }

    #[test]
    fn decoder_rejects_non_canonical_json() {
        let job = ImportJob::create(CreateImportJob {
            job_id: ImportJobId::try_new("import-job-1").unwrap(),
            source: source(),
            mapping: mapping(),
            partial_execution_policy: PartialExecutionPolicy::AllValidRows,
            occurred_at_unix_nanos: 1,
        })
        .unwrap();
        let mut bytes = encode_import_job_state(&job).unwrap();
        bytes.push(b'\n');
        assert_eq!(
            decode_import_job_state(&bytes).unwrap_err().code,
            "CUSTOMER_DATA_IMPORT_PERSISTED_STATE_INVALID"
        );
    }
}
