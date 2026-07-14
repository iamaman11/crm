use crm_module_sdk::{ErrorCategory, FieldName, FieldViolation, RecordId, SdkError};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;

const MAX_SOURCE_NAME_BYTES: usize = 240;
const MAX_COLUMN_NAME_BYTES: usize = 160;
const MAX_DISPLAY_NAME_BYTES: usize = 240;
const MAX_EXTERNAL_ROW_KEY_BYTES: usize = 512;
const MAX_DIAGNOSTICS_PER_ROW: usize = 16;
const MAX_IMPORT_ROWS: u32 = 100_000;
const MAPPING_ID_DOMAIN: &[u8] = b"crm.customer-data-operations.party-import-mapping/v1";
const ROW_ID_DOMAIN: &[u8] = b"crm.customer-data-operations.import-row/v1";
const TARGET_IDEMPOTENCY_DOMAIN: &[u8] =
    b"crm.customer-data-operations.party-create-idempotency/v1";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ImportJobId(RecordId);

impl ImportJobId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, SdkError> {
        RecordId::try_new(value.into()).map(Self).map_err(|error| {
            invalid(
                "CUSTOMER_DATA_IMPORT_JOB_ID_INVALID",
                "customer_data.import_job_id",
                error.to_string(),
            )
        })
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ImportRowId(RecordId);

impl ImportRowId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, SdkError> {
        RecordId::try_new(value.into()).map(Self).map_err(|error| {
            invalid(
                "CUSTOMER_DATA_IMPORT_ROW_ID_INVALID",
                "customer_data.import_row_id",
                error.to_string(),
            )
        })
    }

    fn for_identity(job_id: &ImportJobId, identity: &RowIdentitySource) -> Result<Self, SdkError> {
        let mut hasher = Sha256::new();
        hasher.update(ROW_ID_DOMAIN);
        hash_part(&mut hasher, job_id.as_str().as_bytes());
        match identity {
            RowIdentitySource::Position(position) => {
                hash_part(&mut hasher, b"position");
                hash_part(&mut hasher, &position.to_be_bytes());
            }
            RowIdentitySource::ExternalKeySha256(digest) => {
                hash_part(&mut hasher, b"external-key-sha256");
                hash_part(&mut hasher, digest.as_bytes());
            }
        }
        Self::try_new(format!("cdo-row-{}", hex_digest(hasher.finalize())))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MappingVersionId(RecordId);

impl MappingVersionId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, SdkError> {
        RecordId::try_new(value.into()).map(Self).map_err(|error| {
            invalid(
                "CUSTOMER_DATA_MAPPING_VERSION_ID_INVALID",
                "customer_data.mapping_version_id",
                error.to_string(),
            )
        })
    }

    fn for_party_mapping(mapping: &PartyImportMapping) -> Result<Self, SdkError> {
        let mut hasher = Sha256::new();
        hasher.update(MAPPING_ID_DOMAIN);
        hash_part(&mut hasher, mapping.party_id_column.as_bytes());
        hash_part(&mut hasher, mapping.party_kind_column.as_bytes());
        hash_part(&mut hasher, mapping.display_name_column.as_bytes());
        match &mapping.external_row_key_column {
            Some(value) => {
                hash_part(&mut hasher, b"some");
                hash_part(&mut hasher, value.as_bytes());
            }
            None => hash_part(&mut hasher, b"none"),
        }
        Self::try_new(format!("cdo-map-{}", hex_digest(hasher.finalize())))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TargetPartyId(RecordId);

impl TargetPartyId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, SdkError> {
        RecordId::try_new(value.into()).map(Self).map_err(|error| {
            invalid(
                "CUSTOMER_DATA_TARGET_PARTY_ID_INVALID",
                "customer_data.party.party_id",
                error.to_string(),
            )
        })
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceDescriptor {
    source_name: String,
    content_sha256: String,
    row_count: u32,
}

impl SourceDescriptor {
    pub fn try_new(
        source_name: impl Into<String>,
        content_sha256: impl Into<String>,
        row_count: u32,
    ) -> Result<Self, SdkError> {
        let source_name = normalize_bounded_text(
            source_name.into(),
            MAX_SOURCE_NAME_BYTES,
            "CUSTOMER_DATA_SOURCE_NAME_INVALID",
            "customer_data.source.name",
            "source name",
        )?;
        let content_sha256 = normalize_sha256(
            content_sha256.into(),
            "CUSTOMER_DATA_SOURCE_SHA256_INVALID",
            "customer_data.source.content_sha256",
        )?;
        validate_row_count(row_count)?;
        Ok(Self {
            source_name,
            content_sha256,
            row_count,
        })
    }

    pub fn source_name(&self) -> &str {
        &self.source_name
    }

    pub fn content_sha256(&self) -> &str {
        &self.content_sha256
    }

    pub const fn row_count(&self) -> u32 {
        self.row_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyImportMapping {
    party_id_column: String,
    party_kind_column: String,
    display_name_column: String,
    external_row_key_column: Option<String>,
}

impl PartyImportMapping {
    pub fn try_new(
        party_id_column: impl Into<String>,
        party_kind_column: impl Into<String>,
        display_name_column: impl Into<String>,
        external_row_key_column: Option<String>,
    ) -> Result<Self, SdkError> {
        let party_id_column = normalize_column_name(party_id_column.into())?;
        let party_kind_column = normalize_column_name(party_kind_column.into())?;
        let display_name_column = normalize_column_name(display_name_column.into())?;
        let external_row_key_column = external_row_key_column
            .map(normalize_column_name)
            .transpose()?;

        Ok(Self {
            party_id_column,
            party_kind_column,
            display_name_column,
            external_row_key_column,
        })
    }

    pub fn version_id(&self) -> Result<MappingVersionId, SdkError> {
        MappingVersionId::for_party_mapping(self)
    }

    pub fn party_id_column(&self) -> &str {
        &self.party_id_column
    }

    pub fn party_kind_column(&self) -> &str {
        &self.party_kind_column
    }

    pub fn display_name_column(&self) -> &str {
        &self.display_name_column
    }

    pub fn external_row_key_column(&self) -> Option<&str> {
        self.external_row_key_column.as_deref()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartialExecutionPolicy {
    AllValidRows,
    RequireAllValid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartyImportKind {
    Person,
    Organization,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedPartyRow {
    party_id: TargetPartyId,
    kind: PartyImportKind,
    display_name: String,
}

impl PreparedPartyRow {
    pub fn try_new(
        party_id: TargetPartyId,
        kind: PartyImportKind,
        display_name: impl Into<String>,
    ) -> Result<Self, SdkError> {
        let display_name = normalize_bounded_text(
            display_name.into(),
            MAX_DISPLAY_NAME_BYTES,
            "CUSTOMER_DATA_PARTY_DISPLAY_NAME_INVALID",
            "customer_data.party.display_name",
            "Party display name",
        )?;
        Ok(Self {
            party_id,
            kind,
            display_name,
        })
    }

    pub fn party_id(&self) -> &TargetPartyId {
        &self.party_id
    }

    pub const fn kind(&self) -> PartyImportKind {
        self.kind
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowIdentitySource {
    Position(u32),
    ExternalKeySha256(String),
}

impl RowIdentitySource {
    pub fn for_row(row_position: u32, external_row_key: Option<&str>) -> Result<Self, SdkError> {
        validate_row_position(row_position)?;
        match external_row_key {
            Some(value) => {
                let value = normalize_bounded_text(
                    value.to_owned(),
                    MAX_EXTERNAL_ROW_KEY_BYTES,
                    "CUSTOMER_DATA_EXTERNAL_ROW_KEY_INVALID",
                    "customer_data.row.external_key",
                    "external row key",
                )?;
                let mut hasher = Sha256::new();
                hash_part(&mut hasher, value.as_bytes());
                Ok(Self::ExternalKeySha256(hex_digest(hasher.finalize())))
            }
            None => Ok(Self::Position(row_position)),
        }
    }

    pub fn external_key_sha256(&self) -> Option<&str> {
        match self {
            Self::Position(_) => None,
            Self::ExternalKeySha256(value) => Some(value),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowDiagnostic {
    code: String,
    field: String,
}

impl RowDiagnostic {
    pub fn try_new(code: impl Into<String>, field: impl Into<String>) -> Result<Self, SdkError> {
        let code = normalize_semantic_identifier(
            code.into(),
            128,
            "CUSTOMER_DATA_ROW_DIAGNOSTIC_CODE_INVALID",
            "customer_data.row.diagnostic.code",
            "row diagnostic code",
        )?;
        let field = normalize_bounded_text(
            field.into(),
            MAX_COLUMN_NAME_BYTES,
            "CUSTOMER_DATA_ROW_DIAGNOSTIC_FIELD_INVALID",
            "customer_data.row.diagnostic.field",
            "row diagnostic field",
        )?;
        Ok(Self { code, field })
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn field(&self) -> &str {
        &self.field
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportJobStatus {
    Created,
    Validated,
    Executing,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportJob {
    job_id: ImportJobId,
    source: SourceDescriptor,
    mapping: PartyImportMapping,
    mapping_version_id: MappingVersionId,
    partial_execution_policy: PartialExecutionPolicy,
    status: ImportJobStatus,
    total_rows: u32,
    valid_rows: u32,
    invalid_rows: u32,
    succeeded_rows: u32,
    checkpoint_row_position: u32,
    created_at_unix_nanos: i64,
    updated_at_unix_nanos: i64,
    version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportJobSnapshot {
    pub job_id: ImportJobId,
    pub source: SourceDescriptor,
    pub mapping: PartyImportMapping,
    pub mapping_version_id: MappingVersionId,
    pub partial_execution_policy: PartialExecutionPolicy,
    pub status: ImportJobStatus,
    pub total_rows: u32,
    pub valid_rows: u32,
    pub invalid_rows: u32,
    pub succeeded_rows: u32,
    pub checkpoint_row_position: u32,
    pub created_at_unix_nanos: i64,
    pub updated_at_unix_nanos: i64,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateImportJob {
    pub job_id: ImportJobId,
    pub source: SourceDescriptor,
    pub mapping: PartyImportMapping,
    pub partial_execution_policy: PartialExecutionPolicy,
    pub occurred_at_unix_nanos: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarkImportJobValidated {
    pub expected_version: i64,
    pub valid_rows: u32,
    pub invalid_rows: u32,
    pub occurred_at_unix_nanos: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartImportExecution {
    pub expected_version: i64,
    pub occurred_at_unix_nanos: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdvanceImportCheckpoint {
    pub expected_version: i64,
    pub row_position: u32,
    pub outcome: CheckpointOutcome,
    pub occurred_at_unix_nanos: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckpointOutcome {
    Succeeded,
    SkippedInvalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FinishImportJob {
    pub expected_version: i64,
    pub occurred_at_unix_nanos: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CancelImportJob {
    pub expected_version: i64,
    pub occurred_at_unix_nanos: i64,
}

impl ImportJob {
    pub fn create(command: CreateImportJob) -> Result<Self, SdkError> {
        validate_timestamp(
            "customer_data.import_job.occurred_at_unix_nanos",
            command.occurred_at_unix_nanos,
        )?;
        let mapping_version_id = command.mapping.version_id()?;
        Ok(Self {
            total_rows: command.source.row_count(),
            job_id: command.job_id,
            source: command.source,
            mapping: command.mapping,
            mapping_version_id,
            partial_execution_policy: command.partial_execution_policy,
            status: ImportJobStatus::Created,
            valid_rows: 0,
            invalid_rows: 0,
            succeeded_rows: 0,
            checkpoint_row_position: 0,
            created_at_unix_nanos: command.occurred_at_unix_nanos,
            updated_at_unix_nanos: command.occurred_at_unix_nanos,
            version: 1,
        })
    }

    pub fn rehydrate(snapshot: ImportJobSnapshot) -> Result<Self, SdkError> {
        validate_timestamp(
            "customer_data.import_job.created_at_unix_nanos",
            snapshot.created_at_unix_nanos,
        )?;
        validate_timestamp(
            "customer_data.import_job.updated_at_unix_nanos",
            snapshot.updated_at_unix_nanos,
        )?;
        if snapshot.updated_at_unix_nanos < snapshot.created_at_unix_nanos {
            return Err(invalid(
                "CUSTOMER_DATA_IMPORT_JOB_PERSISTED_TIME_INVALID",
                "customer_data.import_job.updated_at_unix_nanos",
                "updated time cannot precede created time",
            ));
        }
        if snapshot.version <= 0 {
            return Err(invalid(
                "CUSTOMER_DATA_IMPORT_JOB_PERSISTED_VERSION_INVALID",
                "customer_data.import_job.version",
                "persisted import-job version must be positive",
            ));
        }
        validate_row_count(snapshot.total_rows)?;
        if snapshot.source.row_count() != snapshot.total_rows {
            return Err(invalid(
                "CUSTOMER_DATA_IMPORT_JOB_SOURCE_COUNT_INVALID",
                "customer_data.import_job.total_rows",
                "persisted total rows must equal the immutable source row count",
            ));
        }
        let expected_mapping_version = snapshot.mapping.version_id()?;
        if expected_mapping_version != snapshot.mapping_version_id {
            return Err(invalid(
                "CUSTOMER_DATA_IMPORT_JOB_MAPPING_VERSION_INVALID",
                "customer_data.import_job.mapping_version_id",
                "persisted mapping version does not match the immutable mapping",
            ));
        }
        validate_job_counters(
            snapshot.status,
            snapshot.partial_execution_policy,
            snapshot.total_rows,
            snapshot.valid_rows,
            snapshot.invalid_rows,
            snapshot.succeeded_rows,
            snapshot.checkpoint_row_position,
            snapshot.version,
        )?;
        Ok(Self {
            job_id: snapshot.job_id,
            source: snapshot.source,
            mapping: snapshot.mapping,
            mapping_version_id: snapshot.mapping_version_id,
            partial_execution_policy: snapshot.partial_execution_policy,
            status: snapshot.status,
            total_rows: snapshot.total_rows,
            valid_rows: snapshot.valid_rows,
            invalid_rows: snapshot.invalid_rows,
            succeeded_rows: snapshot.succeeded_rows,
            checkpoint_row_position: snapshot.checkpoint_row_position,
            created_at_unix_nanos: snapshot.created_at_unix_nanos,
            updated_at_unix_nanos: snapshot.updated_at_unix_nanos,
            version: snapshot.version,
        })
    }

    pub fn mark_validated(&mut self, command: MarkImportJobValidated) -> Result<(), SdkError> {
        self.require_version(command.expected_version)?;
        self.require_monotonic_time(command.occurred_at_unix_nanos)?;
        if self.status != ImportJobStatus::Created {
            return Err(conflict(
                "CUSTOMER_DATA_IMPORT_JOB_NOT_CREATED",
                "only a created import job can accept validation results",
            ));
        }
        let total = command
            .valid_rows
            .checked_add(command.invalid_rows)
            .ok_or_else(|| invalid_counter("validation row counts overflow"))?;
        if total != self.total_rows {
            return Err(invalid_counter(
                "valid plus invalid rows must equal the immutable source row count",
            ));
        }
        self.valid_rows = command.valid_rows;
        self.invalid_rows = command.invalid_rows;
        self.status = ImportJobStatus::Validated;
        self.advance(command.occurred_at_unix_nanos)
    }

    pub fn start_execution(&mut self, command: StartImportExecution) -> Result<(), SdkError> {
        self.require_version(command.expected_version)?;
        self.require_monotonic_time(command.occurred_at_unix_nanos)?;
        if self.status != ImportJobStatus::Validated {
            return Err(conflict(
                "CUSTOMER_DATA_IMPORT_JOB_NOT_VALIDATED",
                "only a validated import job can start execution",
            ));
        }
        if self.partial_execution_policy == PartialExecutionPolicy::RequireAllValid
            && self.invalid_rows != 0
        {
            return Err(conflict(
                "CUSTOMER_DATA_IMPORT_JOB_INVALID_ROWS_BLOCK_EXECUTION",
                "require_all_valid jobs cannot execute while invalid rows exist",
            ));
        }
        self.status = ImportJobStatus::Executing;
        self.advance(command.occurred_at_unix_nanos)
    }

    pub fn advance_checkpoint(&mut self, command: AdvanceImportCheckpoint) -> Result<(), SdkError> {
        self.require_version(command.expected_version)?;
        self.require_monotonic_time(command.occurred_at_unix_nanos)?;
        if self.status != ImportJobStatus::Executing {
            return Err(conflict(
                "CUSTOMER_DATA_IMPORT_JOB_NOT_EXECUTING",
                "only an executing import job can advance its checkpoint",
            ));
        }
        let expected_position = self.checkpoint_row_position.checked_add(1).ok_or_else(|| {
            conflict(
                "CUSTOMER_DATA_IMPORT_CHECKPOINT_EXHAUSTED",
                "checkpoint cannot advance further",
            )
        })?;
        if command.row_position != expected_position || command.row_position > self.total_rows {
            return Err(conflict(
                "CUSTOMER_DATA_IMPORT_CHECKPOINT_SEQUENCE_CONFLICT",
                format!(
                    "expected checkpoint row {expected_position}, got {}",
                    command.row_position
                ),
            ));
        }
        match command.outcome {
            CheckpointOutcome::Succeeded => {
                if self.succeeded_rows >= self.valid_rows {
                    return Err(invalid_counter(
                        "successful row count cannot exceed validated valid rows",
                    ));
                }
                self.succeeded_rows += 1;
            }
            CheckpointOutcome::SkippedInvalid => {
                if self.partial_execution_policy != PartialExecutionPolicy::AllValidRows {
                    return Err(conflict(
                        "CUSTOMER_DATA_IMPORT_INVALID_SKIP_POLICY_CONFLICT",
                        "invalid rows may only be skipped by all_valid_rows policy",
                    ));
                }
                let already_skipped = self
                    .checkpoint_row_position
                    .checked_sub(self.succeeded_rows)
                    .ok_or_else(|| invalid_counter("checkpoint cannot trail successful rows"))?;
                if already_skipped >= self.invalid_rows {
                    return Err(invalid_counter(
                        "skipped invalid rows cannot exceed validated invalid rows",
                    ));
                }
            }
        }
        self.checkpoint_row_position = command.row_position;
        self.advance(command.occurred_at_unix_nanos)
    }

    pub fn complete(&mut self, command: FinishImportJob) -> Result<(), SdkError> {
        self.require_version(command.expected_version)?;
        self.require_monotonic_time(command.occurred_at_unix_nanos)?;
        if self.status != ImportJobStatus::Executing {
            return Err(conflict(
                "CUSTOMER_DATA_IMPORT_JOB_NOT_EXECUTING",
                "only an executing import job can complete",
            ));
        }
        if self.checkpoint_row_position != self.total_rows || self.succeeded_rows != self.valid_rows
        {
            return Err(conflict(
                "CUSTOMER_DATA_IMPORT_JOB_INCOMPLETE",
                "all source rows must be checkpointed and all valid rows must succeed before completion",
            ));
        }
        self.status = ImportJobStatus::Completed;
        self.advance(command.occurred_at_unix_nanos)
    }

    pub fn cancel(&mut self, command: CancelImportJob) -> Result<(), SdkError> {
        self.require_version(command.expected_version)?;
        self.require_monotonic_time(command.occurred_at_unix_nanos)?;
        if matches!(
            self.status,
            ImportJobStatus::Completed | ImportJobStatus::Cancelled
        ) {
            return Err(conflict(
                "CUSTOMER_DATA_IMPORT_JOB_TERMINAL",
                "completed or cancelled import jobs are terminal",
            ));
        }
        self.status = ImportJobStatus::Cancelled;
        self.advance(command.occurred_at_unix_nanos)
    }

    pub fn snapshot(&self) -> ImportJobSnapshot {
        ImportJobSnapshot {
            job_id: self.job_id.clone(),
            source: self.source.clone(),
            mapping: self.mapping.clone(),
            mapping_version_id: self.mapping_version_id.clone(),
            partial_execution_policy: self.partial_execution_policy,
            status: self.status,
            total_rows: self.total_rows,
            valid_rows: self.valid_rows,
            invalid_rows: self.invalid_rows,
            succeeded_rows: self.succeeded_rows,
            checkpoint_row_position: self.checkpoint_row_position,
            created_at_unix_nanos: self.created_at_unix_nanos,
            updated_at_unix_nanos: self.updated_at_unix_nanos,
            version: self.version,
        }
    }

    pub fn job_id(&self) -> &ImportJobId {
        &self.job_id
    }

    pub fn source(&self) -> &SourceDescriptor {
        &self.source
    }

    pub fn mapping(&self) -> &PartyImportMapping {
        &self.mapping
    }

    pub fn mapping_version_id(&self) -> &MappingVersionId {
        &self.mapping_version_id
    }

    pub const fn status(&self) -> ImportJobStatus {
        self.status
    }

    pub const fn version(&self) -> i64 {
        self.version
    }

    pub const fn checkpoint_row_position(&self) -> u32 {
        self.checkpoint_row_position
    }

    pub const fn succeeded_rows(&self) -> u32 {
        self.succeeded_rows
    }

    fn require_version(&self, expected_version: i64) -> Result<(), SdkError> {
        if expected_version != self.version {
            return Err(conflict(
                "CUSTOMER_DATA_IMPORT_JOB_VERSION_CONFLICT",
                format!(
                    "expected version {expected_version}, current version {}",
                    self.version
                ),
            ));
        }
        Ok(())
    }

    fn require_monotonic_time(&self, occurred_at_unix_nanos: i64) -> Result<(), SdkError> {
        validate_timestamp(
            "customer_data.import_job.occurred_at_unix_nanos",
            occurred_at_unix_nanos,
        )?;
        if occurred_at_unix_nanos <= self.updated_at_unix_nanos {
            return Err(conflict(
                "CUSTOMER_DATA_IMPORT_JOB_TIME_CONFLICT",
                "mutation time must be strictly greater than the current update time",
            ));
        }
        Ok(())
    }

    fn advance(&mut self, occurred_at_unix_nanos: i64) -> Result<(), SdkError> {
        self.version = self.version.checked_add(1).ok_or_else(|| {
            conflict(
                "CUSTOMER_DATA_IMPORT_JOB_VERSION_EXHAUSTED",
                "import-job version cannot advance further",
            )
        })?;
        self.updated_at_unix_nanos = occurred_at_unix_nanos;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportRowStatus {
    Pending,
    Valid,
    Invalid,
    FailedRetryable,
    Succeeded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportRow {
    row_id: ImportRowId,
    job_id: ImportJobId,
    row_position: u32,
    identity_source: RowIdentitySource,
    status: ImportRowStatus,
    prepared_party: Option<PreparedPartyRow>,
    diagnostics: Vec<RowDiagnostic>,
    execution_attempts: u32,
    last_execution_error_code: Option<String>,
    target_party_id: Option<TargetPartyId>,
    created_at_unix_nanos: i64,
    updated_at_unix_nanos: i64,
    version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportRowSnapshot {
    pub row_id: ImportRowId,
    pub job_id: ImportJobId,
    pub row_position: u32,
    pub identity_source: RowIdentitySource,
    pub status: ImportRowStatus,
    pub prepared_party: Option<PreparedPartyRow>,
    pub diagnostics: Vec<RowDiagnostic>,
    pub execution_attempts: u32,
    pub last_execution_error_code: Option<String>,
    pub target_party_id: Option<TargetPartyId>,
    pub created_at_unix_nanos: i64,
    pub updated_at_unix_nanos: i64,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateImportRow {
    pub job_id: ImportJobId,
    pub row_position: u32,
    pub external_row_key: Option<String>,
    pub occurred_at_unix_nanos: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidateImportRowSuccess {
    pub expected_version: i64,
    pub prepared_party: PreparedPartyRow,
    pub occurred_at_unix_nanos: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidateImportRowFailure {
    pub expected_version: i64,
    pub diagnostics: Vec<RowDiagnostic>,
    pub occurred_at_unix_nanos: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordImportRowRetryableFailure {
    pub expected_version: i64,
    pub error_code: String,
    pub occurred_at_unix_nanos: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkImportRowSucceeded {
    pub expected_version: i64,
    pub target_party_id: TargetPartyId,
    pub occurred_at_unix_nanos: i64,
}

impl ImportRow {
    pub fn create(command: CreateImportRow) -> Result<Self, SdkError> {
        validate_timestamp(
            "customer_data.import_row.occurred_at_unix_nanos",
            command.occurred_at_unix_nanos,
        )?;
        let identity_source =
            RowIdentitySource::for_row(command.row_position, command.external_row_key.as_deref())?;
        let row_id = ImportRowId::for_identity(&command.job_id, &identity_source)?;
        Ok(Self {
            row_id,
            job_id: command.job_id,
            row_position: command.row_position,
            identity_source,
            status: ImportRowStatus::Pending,
            prepared_party: None,
            diagnostics: Vec::new(),
            execution_attempts: 0,
            last_execution_error_code: None,
            target_party_id: None,
            created_at_unix_nanos: command.occurred_at_unix_nanos,
            updated_at_unix_nanos: command.occurred_at_unix_nanos,
            version: 1,
        })
    }

    pub fn rehydrate(snapshot: ImportRowSnapshot) -> Result<Self, SdkError> {
        validate_row_position(snapshot.row_position)?;
        validate_timestamp(
            "customer_data.import_row.created_at_unix_nanos",
            snapshot.created_at_unix_nanos,
        )?;
        validate_timestamp(
            "customer_data.import_row.updated_at_unix_nanos",
            snapshot.updated_at_unix_nanos,
        )?;
        if snapshot.updated_at_unix_nanos < snapshot.created_at_unix_nanos {
            return Err(invalid(
                "CUSTOMER_DATA_IMPORT_ROW_PERSISTED_TIME_INVALID",
                "customer_data.import_row.updated_at_unix_nanos",
                "updated time cannot precede created time",
            ));
        }
        if snapshot.version <= 0 {
            return Err(invalid(
                "CUSTOMER_DATA_IMPORT_ROW_PERSISTED_VERSION_INVALID",
                "customer_data.import_row.version",
                "persisted row version must be positive",
            ));
        }
        if let RowIdentitySource::Position(position) = snapshot.identity_source
            && position != snapshot.row_position
        {
            return Err(invalid(
                "CUSTOMER_DATA_IMPORT_ROW_IDENTITY_POSITION_INVALID",
                "customer_data.import_row.identity_source",
                "position-based identity must match the source row position",
            ));
        }
        if let RowIdentitySource::ExternalKeySha256(value) = &snapshot.identity_source {
            normalize_sha256(
                value.clone(),
                "CUSTOMER_DATA_IMPORT_ROW_EXTERNAL_KEY_DIGEST_INVALID",
                "customer_data.import_row.identity_source",
            )?;
        }
        let expected_row_id =
            ImportRowId::for_identity(&snapshot.job_id, &snapshot.identity_source)?;
        if expected_row_id != snapshot.row_id {
            return Err(invalid(
                "CUSTOMER_DATA_IMPORT_ROW_PERSISTED_ID_INVALID",
                "customer_data.import_row.row_id",
                "persisted row ID does not match deterministic row identity",
            ));
        }
        validate_row_state_shape(
            snapshot.status,
            &snapshot.prepared_party,
            &snapshot.diagnostics,
            snapshot.execution_attempts,
            snapshot.last_execution_error_code.as_deref(),
            snapshot.target_party_id.as_ref(),
            snapshot.version,
        )?;
        Ok(Self {
            row_id: snapshot.row_id,
            job_id: snapshot.job_id,
            row_position: snapshot.row_position,
            identity_source: snapshot.identity_source,
            status: snapshot.status,
            prepared_party: snapshot.prepared_party,
            diagnostics: snapshot.diagnostics,
            execution_attempts: snapshot.execution_attempts,
            last_execution_error_code: snapshot.last_execution_error_code,
            target_party_id: snapshot.target_party_id,
            created_at_unix_nanos: snapshot.created_at_unix_nanos,
            updated_at_unix_nanos: snapshot.updated_at_unix_nanos,
            version: snapshot.version,
        })
    }

    pub fn mark_valid(&mut self, command: ValidateImportRowSuccess) -> Result<(), SdkError> {
        self.require_pending(command.expected_version, command.occurred_at_unix_nanos)?;
        self.prepared_party = Some(command.prepared_party);
        self.status = ImportRowStatus::Valid;
        self.advance(command.occurred_at_unix_nanos)
    }

    pub fn mark_invalid(&mut self, command: ValidateImportRowFailure) -> Result<(), SdkError> {
        self.require_pending(command.expected_version, command.occurred_at_unix_nanos)?;
        validate_diagnostics(&command.diagnostics)?;
        self.diagnostics = command.diagnostics;
        self.status = ImportRowStatus::Invalid;
        self.advance(command.occurred_at_unix_nanos)
    }

    pub fn record_retryable_failure(
        &mut self,
        command: RecordImportRowRetryableFailure,
    ) -> Result<(), SdkError> {
        self.require_version(command.expected_version)?;
        self.require_monotonic_time(command.occurred_at_unix_nanos)?;
        if !matches!(
            self.status,
            ImportRowStatus::Valid | ImportRowStatus::FailedRetryable
        ) {
            return Err(conflict(
                "CUSTOMER_DATA_IMPORT_ROW_NOT_EXECUTABLE",
                "only valid or retryable-failed rows can record an execution failure",
            ));
        }
        let error_code = normalize_semantic_identifier(
            command.error_code,
            128,
            "CUSTOMER_DATA_IMPORT_ROW_EXECUTION_ERROR_CODE_INVALID",
            "customer_data.import_row.execution_error_code",
            "execution error code",
        )?;
        self.execution_attempts = self.execution_attempts.checked_add(1).ok_or_else(|| {
            conflict(
                "CUSTOMER_DATA_IMPORT_ROW_ATTEMPTS_EXHAUSTED",
                "row execution attempts cannot advance further",
            )
        })?;
        self.last_execution_error_code = Some(error_code);
        self.status = ImportRowStatus::FailedRetryable;
        self.advance(command.occurred_at_unix_nanos)
    }

    pub fn mark_succeeded(&mut self, command: MarkImportRowSucceeded) -> Result<(), SdkError> {
        self.require_version(command.expected_version)?;
        self.require_monotonic_time(command.occurred_at_unix_nanos)?;
        if !matches!(
            self.status,
            ImportRowStatus::Valid | ImportRowStatus::FailedRetryable
        ) {
            return Err(conflict(
                "CUSTOMER_DATA_IMPORT_ROW_NOT_EXECUTABLE",
                "only valid or retryable-failed rows can succeed",
            ));
        }
        let prepared_party = self.prepared_party.as_ref().ok_or_else(|| {
            invalid(
                "CUSTOMER_DATA_IMPORT_ROW_PREPARED_PARTY_MISSING",
                "customer_data.import_row.prepared_party",
                "executable row must retain its prepared Party command",
            )
        })?;
        if prepared_party.party_id() != &command.target_party_id {
            return Err(conflict(
                "CUSTOMER_DATA_IMPORT_ROW_TARGET_PARTY_CONFLICT",
                "target Party result must match the prepared Party identity",
            ));
        }
        self.target_party_id = Some(command.target_party_id);
        self.status = ImportRowStatus::Succeeded;
        self.advance(command.occurred_at_unix_nanos)
    }

    pub fn target_idempotency_key(&self) -> Result<String, SdkError> {
        let mut hasher = Sha256::new();
        hasher.update(TARGET_IDEMPOTENCY_DOMAIN);
        hash_part(&mut hasher, self.job_id.as_str().as_bytes());
        hash_part(&mut hasher, self.row_id.as_str().as_bytes());
        hash_part(&mut hasher, b"parties.party.create@1.0.0");
        Ok(format!(
            "cdo-party-create-{}",
            hex_digest(hasher.finalize())
        ))
    }

    pub fn snapshot(&self) -> ImportRowSnapshot {
        ImportRowSnapshot {
            row_id: self.row_id.clone(),
            job_id: self.job_id.clone(),
            row_position: self.row_position,
            identity_source: self.identity_source.clone(),
            status: self.status,
            prepared_party: self.prepared_party.clone(),
            diagnostics: self.diagnostics.clone(),
            execution_attempts: self.execution_attempts,
            last_execution_error_code: self.last_execution_error_code.clone(),
            target_party_id: self.target_party_id.clone(),
            created_at_unix_nanos: self.created_at_unix_nanos,
            updated_at_unix_nanos: self.updated_at_unix_nanos,
            version: self.version,
        }
    }

    pub fn row_id(&self) -> &ImportRowId {
        &self.row_id
    }

    pub fn job_id(&self) -> &ImportJobId {
        &self.job_id
    }

    pub const fn row_position(&self) -> u32 {
        self.row_position
    }

    pub const fn status(&self) -> ImportRowStatus {
        self.status
    }

    pub fn prepared_party(&self) -> Option<&PreparedPartyRow> {
        self.prepared_party.as_ref()
    }

    pub const fn version(&self) -> i64 {
        self.version
    }

    fn require_pending(
        &self,
        expected_version: i64,
        occurred_at_unix_nanos: i64,
    ) -> Result<(), SdkError> {
        self.require_version(expected_version)?;
        self.require_monotonic_time(occurred_at_unix_nanos)?;
        if self.status != ImportRowStatus::Pending {
            return Err(conflict(
                "CUSTOMER_DATA_IMPORT_ROW_ALREADY_VALIDATED",
                "only a pending row can receive validation results",
            ));
        }
        Ok(())
    }

    fn require_version(&self, expected_version: i64) -> Result<(), SdkError> {
        if expected_version != self.version {
            return Err(conflict(
                "CUSTOMER_DATA_IMPORT_ROW_VERSION_CONFLICT",
                format!(
                    "expected version {expected_version}, current version {}",
                    self.version
                ),
            ));
        }
        Ok(())
    }

    fn require_monotonic_time(&self, occurred_at_unix_nanos: i64) -> Result<(), SdkError> {
        validate_timestamp(
            "customer_data.import_row.occurred_at_unix_nanos",
            occurred_at_unix_nanos,
        )?;
        if occurred_at_unix_nanos <= self.updated_at_unix_nanos {
            return Err(conflict(
                "CUSTOMER_DATA_IMPORT_ROW_TIME_CONFLICT",
                "mutation time must be strictly greater than the current update time",
            ));
        }
        Ok(())
    }

    fn advance(&mut self, occurred_at_unix_nanos: i64) -> Result<(), SdkError> {
        self.version = self.version.checked_add(1).ok_or_else(|| {
            conflict(
                "CUSTOMER_DATA_IMPORT_ROW_VERSION_EXHAUSTED",
                "row version cannot advance further",
            )
        })?;
        self.updated_at_unix_nanos = occurred_at_unix_nanos;
        Ok(())
    }
}

fn validate_job_counters(
    status: ImportJobStatus,
    policy: PartialExecutionPolicy,
    total_rows: u32,
    valid_rows: u32,
    invalid_rows: u32,
    succeeded_rows: u32,
    checkpoint_row_position: u32,
    version: i64,
) -> Result<(), SdkError> {
    let validated_total = valid_rows
        .checked_add(invalid_rows)
        .ok_or_else(|| invalid_counter("validation counters overflow"))?;
    if validated_total > total_rows
        || succeeded_rows > valid_rows
        || checkpoint_row_position > total_rows
    {
        return Err(invalid_counter(
            "persisted import-job counters exceed their bounds",
        ));
    }
    match status {
        ImportJobStatus::Created => {
            if version != 1
                || valid_rows != 0
                || invalid_rows != 0
                || succeeded_rows != 0
                || checkpoint_row_position != 0
            {
                return Err(invalid_counter(
                    "created import jobs must retain the initial version and zero counters",
                ));
            }
        }
        ImportJobStatus::Validated => {
            if validated_total != total_rows || succeeded_rows != 0 || checkpoint_row_position != 0
            {
                return Err(invalid_counter(
                    "validated import jobs require complete validation counters and zero execution progress",
                ));
            }
        }
        ImportJobStatus::Executing => {
            if validated_total != total_rows {
                return Err(invalid_counter(
                    "executing import jobs require complete validation counters",
                ));
            }
            if policy == PartialExecutionPolicy::RequireAllValid && invalid_rows != 0 {
                return Err(invalid_counter(
                    "require_all_valid jobs cannot execute with invalid rows",
                ));
            }
            if checkpoint_row_position < succeeded_rows
                || checkpoint_row_position > succeeded_rows.saturating_add(invalid_rows)
            {
                return Err(invalid_counter(
                    "executing checkpoint must equal successful rows plus a bounded number of skipped invalid rows",
                ));
            }
        }
        ImportJobStatus::Completed => {
            if validated_total != total_rows
                || succeeded_rows != valid_rows
                || checkpoint_row_position != total_rows
            {
                return Err(invalid_counter(
                    "completed import jobs require every source row checkpointed and every valid row succeeded",
                ));
            }
        }
        ImportJobStatus::Cancelled => {}
    }
    Ok(())
}

fn validate_row_state_shape(
    status: ImportRowStatus,
    prepared_party: &Option<PreparedPartyRow>,
    diagnostics: &[RowDiagnostic],
    execution_attempts: u32,
    last_execution_error_code: Option<&str>,
    target_party_id: Option<&TargetPartyId>,
    version: i64,
) -> Result<(), SdkError> {
    validate_diagnostics_allow_empty(diagnostics)?;
    if let Some(code) = last_execution_error_code {
        normalize_semantic_identifier(
            code.to_owned(),
            128,
            "CUSTOMER_DATA_IMPORT_ROW_EXECUTION_ERROR_CODE_INVALID",
            "customer_data.import_row.execution_error_code",
            "execution error code",
        )?;
    }
    match status {
        ImportRowStatus::Pending => {
            if version != 1
                || prepared_party.is_some()
                || !diagnostics.is_empty()
                || execution_attempts != 0
                || last_execution_error_code.is_some()
                || target_party_id.is_some()
            {
                return Err(invalid_row_shape("pending row state is not canonical"));
            }
        }
        ImportRowStatus::Valid => {
            if prepared_party.is_none()
                || !diagnostics.is_empty()
                || execution_attempts != 0
                || last_execution_error_code.is_some()
                || target_party_id.is_some()
            {
                return Err(invalid_row_shape("valid row state is not canonical"));
            }
        }
        ImportRowStatus::Invalid => {
            if prepared_party.is_some()
                || diagnostics.is_empty()
                || execution_attempts != 0
                || last_execution_error_code.is_some()
                || target_party_id.is_some()
            {
                return Err(invalid_row_shape("invalid row state is not canonical"));
            }
        }
        ImportRowStatus::FailedRetryable => {
            if prepared_party.is_none()
                || !diagnostics.is_empty()
                || execution_attempts == 0
                || last_execution_error_code.is_none()
                || target_party_id.is_some()
            {
                return Err(invalid_row_shape(
                    "retryable-failed row state is not canonical",
                ));
            }
        }
        ImportRowStatus::Succeeded => {
            if prepared_party.is_none() || !diagnostics.is_empty() || target_party_id.is_none() {
                return Err(invalid_row_shape("succeeded row state is not canonical"));
            }
            if execution_attempts == 0 && last_execution_error_code.is_some() {
                return Err(invalid_row_shape(
                    "succeeded row cannot retain an execution error without prior attempts",
                ));
            }
        }
    }
    Ok(())
}

fn validate_diagnostics(diagnostics: &[RowDiagnostic]) -> Result<(), SdkError> {
    if diagnostics.is_empty() || diagnostics.len() > MAX_DIAGNOSTICS_PER_ROW {
        return Err(invalid(
            "CUSTOMER_DATA_IMPORT_ROW_DIAGNOSTICS_INVALID",
            "customer_data.import_row.diagnostics",
            format!("row diagnostics must contain between 1 and {MAX_DIAGNOSTICS_PER_ROW} entries"),
        ));
    }
    validate_diagnostics_allow_empty(diagnostics)
}

fn validate_diagnostics_allow_empty(diagnostics: &[RowDiagnostic]) -> Result<(), SdkError> {
    if diagnostics.len() > MAX_DIAGNOSTICS_PER_ROW {
        return Err(invalid(
            "CUSTOMER_DATA_IMPORT_ROW_DIAGNOSTICS_INVALID",
            "customer_data.import_row.diagnostics",
            format!("row diagnostics cannot exceed {MAX_DIAGNOSTICS_PER_ROW} entries"),
        ));
    }
    Ok(())
}

fn validate_row_count(row_count: u32) -> Result<(), SdkError> {
    if row_count == 0 || row_count > MAX_IMPORT_ROWS {
        return Err(invalid(
            "CUSTOMER_DATA_SOURCE_ROW_COUNT_INVALID",
            "customer_data.source.row_count",
            format!("row count must be between 1 and {MAX_IMPORT_ROWS}"),
        ));
    }
    Ok(())
}

fn validate_row_position(row_position: u32) -> Result<(), SdkError> {
    if row_position == 0 || row_position > MAX_IMPORT_ROWS {
        return Err(invalid(
            "CUSTOMER_DATA_IMPORT_ROW_POSITION_INVALID",
            "customer_data.import_row.row_position",
            format!("row position must be between 1 and {MAX_IMPORT_ROWS}"),
        ));
    }
    Ok(())
}

fn normalize_column_name(value: String) -> Result<String, SdkError> {
    normalize_bounded_text(
        value,
        MAX_COLUMN_NAME_BYTES,
        "CUSTOMER_DATA_MAPPING_COLUMN_INVALID",
        "customer_data.mapping.column",
        "mapping column name",
    )
}

fn normalize_sha256(
    value: String,
    code: &'static str,
    field: &'static str,
) -> Result<String, SdkError> {
    let canonical = value.trim().to_owned();
    if canonical.len() != 64
        || canonical
            .as_bytes()
            .iter()
            .any(|byte| !byte.is_ascii_hexdigit() || byte.is_ascii_uppercase())
    {
        return Err(invalid(
            code,
            field,
            "SHA-256 digest must be exactly 64 lowercase hexadecimal characters",
        ));
    }
    Ok(canonical)
}

fn normalize_bounded_text(
    value: String,
    maximum_bytes: usize,
    code: &'static str,
    field: &'static str,
    label: &str,
) -> Result<String, SdkError> {
    if value.chars().any(char::is_control) {
        return Err(invalid(
            code,
            field,
            format!("{label} must not contain control characters"),
        ));
    }
    let canonical = value.trim().to_owned();
    if canonical.is_empty() || canonical.len() > maximum_bytes {
        return Err(invalid(
            code,
            field,
            format!("{label} must be non-empty and not exceed {maximum_bytes} UTF-8 bytes"),
        ));
    }
    Ok(canonical)
}

fn normalize_semantic_identifier(
    value: String,
    maximum_bytes: usize,
    code: &'static str,
    field: &'static str,
    label: &str,
) -> Result<String, SdkError> {
    if value.chars().any(char::is_control) {
        return Err(invalid(
            code,
            field,
            format!("{label} must not contain control characters"),
        ));
    }
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() || normalized.len() > maximum_bytes {
        return Err(invalid(
            code,
            field,
            format!("{label} must be non-empty and not exceed {maximum_bytes} UTF-8 bytes"),
        ));
    }
    let bytes = normalized.as_bytes();
    if !bytes.first().is_some_and(u8::is_ascii_alphanumeric)
        || !bytes.last().is_some_and(u8::is_ascii_alphanumeric)
        || bytes
            .iter()
            .any(|byte| !(byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-')))
    {
        return Err(invalid(
            code,
            field,
            format!(
                "{label} must start and end with an ASCII letter or digit and contain only ASCII letters, digits, '.', '_' or '-'"
            ),
        ));
    }
    Ok(normalized)
}

fn validate_timestamp(field: &'static str, value: i64) -> Result<(), SdkError> {
    if value <= 0 {
        return Err(invalid(
            "CUSTOMER_DATA_TIMESTAMP_INVALID",
            field,
            "time must be a positive Unix-nanosecond value",
        ));
    }
    Ok(())
}

fn hash_part(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}

fn hex_digest(bytes: impl AsRef<[u8]>) -> String {
    let bytes = bytes.as_ref();
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut value, "{byte:02x}").expect("writing to String cannot fail");
    }
    value
}

fn invalid_counter(internal: impl Into<String>) -> SdkError {
    invalid(
        "CUSTOMER_DATA_IMPORT_JOB_COUNTERS_INVALID",
        "customer_data.import_job.counters",
        internal,
    )
}

fn invalid_row_shape(internal: impl Into<String>) -> SdkError {
    invalid(
        "CUSTOMER_DATA_IMPORT_ROW_STATE_INVALID",
        "customer_data.import_row.state",
        internal,
    )
}

fn invalid(code: &'static str, field: &'static str, internal: impl Into<String>) -> SdkError {
    SdkError::new(
        code,
        ErrorCategory::InvalidArgument,
        false,
        "The customer-data import request is invalid.",
    )
    .with_internal_reference(internal)
    .with_field_violation(FieldViolation {
        field: FieldName::try_new(field).expect("static customer-data field must be valid"),
        code: "INVALID".to_owned(),
        safe_message: "The customer-data import field is invalid.".to_owned(),
    })
}

fn conflict(code: &'static str, internal: impl Into<String>) -> SdkError {
    SdkError::new(
        code,
        ErrorCategory::Conflict,
        false,
        "The customer-data import state changed before this operation could be applied.",
    )
    .with_internal_reference(internal)
}

trait SdkErrorFieldViolationExt {
    fn with_field_violation(self, violation: FieldViolation) -> Self;
}

impl SdkErrorFieldViolationExt for SdkError {
    fn with_field_violation(mut self, violation: FieldViolation) -> Self {
        self.field_violations.push(violation);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            Some("external_id".into()),
        )
        .unwrap()
    }

    fn job(policy: PartialExecutionPolicy, rows: u32) -> ImportJob {
        ImportJob::create(CreateImportJob {
            job_id: ImportJobId::try_new("import-job-1").unwrap(),
            source: source(rows),
            mapping: mapping(),
            partial_execution_policy: policy,
            occurred_at_unix_nanos: 100,
        })
        .unwrap()
    }

    fn prepared_party() -> PreparedPartyRow {
        PreparedPartyRow::try_new(
            TargetPartyId::try_new("party-1").unwrap(),
            PartyImportKind::Person,
            "Ada Lovelace",
        )
        .unwrap()
    }

    #[test]
    fn mapping_version_is_deterministic_and_input_normalized() {
        let first = PartyImportMapping::try_new(
            " party_id ",
            "kind",
            "display_name",
            Some("external_id".into()),
        )
        .unwrap();
        let second = mapping();
        assert_eq!(first, second);
        assert_eq!(first.version_id().unwrap(), second.version_id().unwrap());
    }

    #[test]
    fn row_identity_is_deterministic_for_external_key_and_position() {
        let job_id = ImportJobId::try_new("import-job-1").unwrap();
        let first = ImportRow::create(CreateImportRow {
            job_id: job_id.clone(),
            row_position: 1,
            external_row_key: Some(" customer-42 ".into()),
            occurred_at_unix_nanos: 100,
        })
        .unwrap();
        let second = ImportRow::create(CreateImportRow {
            job_id: job_id.clone(),
            row_position: 99,
            external_row_key: Some("customer-42".into()),
            occurred_at_unix_nanos: 100,
        })
        .unwrap();
        assert_eq!(first.row_id(), second.row_id());

        let positioned = ImportRow::create(CreateImportRow {
            job_id,
            row_position: 1,
            external_row_key: None,
            occurred_at_unix_nanos: 100,
        })
        .unwrap();
        assert_ne!(first.row_id(), positioned.row_id());
    }

    #[test]
    fn target_idempotency_is_stable_across_retry_state() {
        let mut row = ImportRow::create(CreateImportRow {
            job_id: ImportJobId::try_new("import-job-1").unwrap(),
            row_position: 1,
            external_row_key: Some("customer-42".into()),
            occurred_at_unix_nanos: 100,
        })
        .unwrap();
        row.mark_valid(ValidateImportRowSuccess {
            expected_version: 1,
            prepared_party: prepared_party(),
            occurred_at_unix_nanos: 200,
        })
        .unwrap();
        let before = row.target_idempotency_key().unwrap();
        row.record_retryable_failure(RecordImportRowRetryableFailure {
            expected_version: 2,
            error_code: "target.temporarily_unavailable".into(),
            occurred_at_unix_nanos: 300,
        })
        .unwrap();
        assert_eq!(before, row.target_idempotency_key().unwrap());
    }

    #[test]
    fn require_all_valid_blocks_execution_when_validation_has_errors() {
        let mut value = job(PartialExecutionPolicy::RequireAllValid, 2);
        value
            .mark_validated(MarkImportJobValidated {
                expected_version: 1,
                valid_rows: 1,
                invalid_rows: 1,
                occurred_at_unix_nanos: 200,
            })
            .unwrap();
        let error = value
            .start_execution(StartImportExecution {
                expected_version: 2,
                occurred_at_unix_nanos: 300,
            })
            .unwrap_err();
        assert_eq!(
            error.code,
            "CUSTOMER_DATA_IMPORT_JOB_INVALID_ROWS_BLOCK_EXECUTION"
        );
    }

    #[test]
    fn all_valid_rows_can_checkpoint_success_and_invalid_rows_then_complete() {
        let mut value = job(PartialExecutionPolicy::AllValidRows, 3);
        value
            .mark_validated(MarkImportJobValidated {
                expected_version: 1,
                valid_rows: 2,
                invalid_rows: 1,
                occurred_at_unix_nanos: 200,
            })
            .unwrap();
        value
            .start_execution(StartImportExecution {
                expected_version: 2,
                occurred_at_unix_nanos: 300,
            })
            .unwrap();
        value
            .advance_checkpoint(AdvanceImportCheckpoint {
                expected_version: 3,
                row_position: 1,
                outcome: CheckpointOutcome::Succeeded,
                occurred_at_unix_nanos: 400,
            })
            .unwrap();
        value
            .advance_checkpoint(AdvanceImportCheckpoint {
                expected_version: 4,
                row_position: 2,
                outcome: CheckpointOutcome::SkippedInvalid,
                occurred_at_unix_nanos: 500,
            })
            .unwrap();
        value
            .advance_checkpoint(AdvanceImportCheckpoint {
                expected_version: 5,
                row_position: 3,
                outcome: CheckpointOutcome::Succeeded,
                occurred_at_unix_nanos: 600,
            })
            .unwrap();
        value
            .complete(FinishImportJob {
                expected_version: 6,
                occurred_at_unix_nanos: 700,
            })
            .unwrap();
        assert_eq!(value.status(), ImportJobStatus::Completed);
        assert_eq!(value.succeeded_rows(), 2);
        assert_eq!(value.checkpoint_row_position(), 3);
    }

    #[test]
    fn row_validation_and_retryable_execution_are_irreversible_but_retryable() {
        let mut row = ImportRow::create(CreateImportRow {
            job_id: ImportJobId::try_new("import-job-1").unwrap(),
            row_position: 1,
            external_row_key: None,
            occurred_at_unix_nanos: 100,
        })
        .unwrap();
        row.mark_valid(ValidateImportRowSuccess {
            expected_version: 1,
            prepared_party: prepared_party(),
            occurred_at_unix_nanos: 200,
        })
        .unwrap();
        row.record_retryable_failure(RecordImportRowRetryableFailure {
            expected_version: 2,
            error_code: "target.timeout".into(),
            occurred_at_unix_nanos: 300,
        })
        .unwrap();
        row.mark_succeeded(MarkImportRowSucceeded {
            expected_version: 3,
            target_party_id: TargetPartyId::try_new("party-1").unwrap(),
            occurred_at_unix_nanos: 400,
        })
        .unwrap();
        assert_eq!(row.status(), ImportRowStatus::Succeeded);
        assert_eq!(row.version(), 4);
    }

    #[test]
    fn strict_job_rehydrate_rejects_mapping_identity_mismatch() {
        let value = job(PartialExecutionPolicy::AllValidRows, 2);
        let mut snapshot = value.snapshot();
        snapshot.mapping_version_id = MappingVersionId::try_new("wrong-mapping").unwrap();
        assert_eq!(
            ImportJob::rehydrate(snapshot).unwrap_err().code,
            "CUSTOMER_DATA_IMPORT_JOB_MAPPING_VERSION_INVALID"
        );
    }

    #[test]
    fn strict_row_rehydrate_rejects_unreachable_shape() {
        let row = ImportRow::create(CreateImportRow {
            job_id: ImportJobId::try_new("import-job-1").unwrap(),
            row_position: 1,
            external_row_key: None,
            occurred_at_unix_nanos: 100,
        })
        .unwrap();
        let mut snapshot = row.snapshot();
        snapshot.status = ImportRowStatus::Succeeded;
        assert_eq!(
            ImportRow::rehydrate(snapshot).unwrap_err().code,
            "CUSTOMER_DATA_IMPORT_ROW_STATE_INVALID"
        );
    }
}
