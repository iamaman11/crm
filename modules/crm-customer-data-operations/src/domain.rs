use crate::profile::{ExternalPartyIdentifierDigest, ImportParserProfile, SourceSystemId};
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
const DERIVED_TARGET_PARTY_ID_DOMAIN: &[u8] =
    b"crm.customer-data-operations.derived-target-party-id/v1";
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
        hash_optional(&mut hasher, mapping.target_party_id_column.as_deref());
        hash_part(&mut hasher, mapping.party_kind_column.as_bytes());
        hash_part(&mut hasher, mapping.display_name_column.as_bytes());
        hash_optional(&mut hasher, mapping.source_external_id_column.as_deref());
        hash_optional(&mut hasher, mapping.external_row_key_column.as_deref());
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

    pub fn derive_for_import_row(
        job_id: &ImportJobId,
        row_id: &ImportRowId,
    ) -> Result<Self, SdkError> {
        let mut hasher = Sha256::new();
        hasher.update(DERIVED_TARGET_PARTY_ID_DOMAIN);
        hash_part(&mut hasher, job_id.as_str().as_bytes());
        hash_part(&mut hasher, row_id.as_str().as_bytes());
        Self::try_new(format!("party-import-{}", hex_digest(hasher.finalize())))
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
    source_system_id: SourceSystemId,
    parser_profile: ImportParserProfile,
}

impl SourceDescriptor {
    pub fn try_new(
        source_name: impl Into<String>,
        content_sha256: impl Into<String>,
        row_count: u32,
        source_system_id: SourceSystemId,
        parser_profile: ImportParserProfile,
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
            source_system_id,
            parser_profile,
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

    pub fn source_system_id(&self) -> &SourceSystemId {
        &self.source_system_id
    }

    pub fn parser_profile(&self) -> &ImportParserProfile {
        &self.parser_profile
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyImportMapping {
    target_party_id_column: Option<String>,
    party_kind_column: String,
    display_name_column: String,
    source_external_id_column: Option<String>,
    external_row_key_column: Option<String>,
}

impl PartyImportMapping {
    pub fn try_new(
        target_party_id_column: Option<String>,
        party_kind_column: impl Into<String>,
        display_name_column: impl Into<String>,
        source_external_id_column: Option<String>,
        external_row_key_column: Option<String>,
    ) -> Result<Self, SdkError> {
        let target_party_id_column = target_party_id_column
            .map(normalize_column_name)
            .transpose()?;
        let party_kind_column = normalize_column_name(party_kind_column.into())?;
        let display_name_column = normalize_column_name(display_name_column.into())?;
        let source_external_id_column = source_external_id_column
            .map(normalize_column_name)
            .transpose()?;
        let external_row_key_column = external_row_key_column
            .map(normalize_column_name)
            .transpose()?;

        Ok(Self {
            target_party_id_column,
            party_kind_column,
            display_name_column,
            source_external_id_column,
            external_row_key_column,
        })
    }

    pub fn version_id(&self) -> Result<MappingVersionId, SdkError> {
        MappingVersionId::for_party_mapping(self)
    }

    pub fn target_party_id_column(&self) -> Option<&str> {
        self.target_party_id_column.as_deref()
    }

    pub fn party_kind_column(&self) -> &str {
        &self.party_kind_column
    }

    pub fn display_name_column(&self) -> &str {
        &self.display_name_column
    }

    pub fn source_external_id_column(&self) -> Option<&str> {
        self.source_external_id_column.as_deref()
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
    source_external_id_sha256: Option<ExternalPartyIdentifierDigest>,
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
    pub source_external_id_sha256: Option<ExternalPartyIdentifierDigest>,
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
    pub source_external_id: Option<String>,
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
        let source_external_id_sha256 = command
            .source_external_id
            .map(ExternalPartyIdentifierDigest::for_identifier)
            .transpose()?;
        Ok(Self {
            row_id,
            job_id: command.job_id,
            row_position: command.row_position,
            identity_source,
            source_external_id_sha256,
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
            source_external_id_sha256: snapshot.source_external_id_sha256,
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

    pub fn target_idempotency_key(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(TARGET_IDEMPOTENCY_DOMAIN);
        hash_part(&mut hasher, self.job_id.as_str().as_bytes());
        hash_part(&mut hasher, self.row_id.as_str().as_bytes());
        hash_part(&mut hasher, b"parties.party.create@1.0.0");
        format!("cdo-party-create-{}", hex_digest(hasher.finalize()))
    }

    pub fn derived_target_party_id(&self) -> Result<TargetPartyId, SdkError> {
        TargetPartyId::derive_for_import_row(&self.job_id, &self.row_id)
    }

    pub fn snapshot(&self) -> ImportRowSnapshot {
        ImportRowSnapshot {
            row_id: self.row_id.clone(),
            job_id: self.job_id.clone(),
            row_position: self.row_position,
            identity_source: self.identity_source.clone(),
            source_external_id_sha256: self.source_external_id_sha256.clone(),
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

    pub fn source_external_id_sha256(&self) -> Option<&ExternalPartyIdentifierDigest> {
        self.source_external_id_sha256.as_ref()
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
            if validated_total != total_rows
                || succeeded_rows != 0
                || checkpoint_row_position != 0
            {
                return Err(invalid_counter(
                    "validated import jobs require complete validation and zero execution counters",
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
                    "require_all_valid jobs cannot persist executing state with invalid rows",
                ));
            }
            let skipped = checkpoint_row_position
                .checked_sub(succeeded_rows)
                .ok_or_else(|| invalid_counter("checkpoint cannot trail successful rows"))?;
            if skipped > invalid_rows {
                return Err(invalid_counter(
                    "checkpoint implies more skipped rows than validated invalid rows",
                ));
            }
        }
        ImportJobStatus::Completed => {
            if validated_total != total_rows
                || checkpoint_row_position != total_rows
                || succeeded_rows != valid_rows
            {
                return Err(invalid_counter(
                    "completed import jobs require every source row checkpointed and every valid row successful",
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
    validate_diagnostics(diagnostics)?;
    match status {
        ImportRowStatus::Pending => {
            if version != 1
                || prepared_party.is_some()
                || !diagnostics.is_empty()
                || execution_attempts != 0
                || last_execution_error_code.is_some()
                || target_party_id.is_some()
            {
                return Err(invalid_row_state(
                    "pending rows must retain initial empty outcome state",
                ));
            }
        }
        ImportRowStatus::Valid => {
            if prepared_party.is_none()
                || !diagnostics.is_empty()
                || execution_attempts != 0
                || last_execution_error_code.is_some()
                || target_party_id.is_some()
            {
                return Err(invalid_row_state(
                    "valid rows require a prepared Party and no execution outcome",
                ));
            }
        }
        ImportRowStatus::Invalid => {
            if prepared_party.is_some()
                || diagnostics.is_empty()
                || execution_attempts != 0
                || last_execution_error_code.is_some()
                || target_party_id.is_some()
            {
                return Err(invalid_row_state(
                    "invalid rows require diagnostics and no prepared target or execution outcome",
                ));
            }
        }
        ImportRowStatus::FailedRetryable => {
            if prepared_party.is_none()
                || !diagnostics.is_empty()
                || execution_attempts == 0
                || last_execution_error_code.is_none()
                || target_party_id.is_some()
            {
                return Err(invalid_row_state(
                    "retryable-failed rows require prepared target and retryable execution evidence",
                ));
            }
        }
        ImportRowStatus::Succeeded => {
            let prepared_party = prepared_party.as_ref().ok_or_else(|| {
                invalid_row_state("succeeded rows require a prepared Party target")
            })?;
            let target_party_id = target_party_id.ok_or_else(|| {
                invalid_row_state("succeeded rows require the authoritative target Party result")
            })?;
            if !diagnostics.is_empty()
                || last_execution_error_code.is_some()
                || prepared_party.party_id() != target_party_id
            {
                return Err(invalid_row_state(
                    "succeeded row state does not match its prepared Party target",
                ));
            }
        }
    }
    Ok(())
}

fn validate_diagnostics(diagnostics: &[RowDiagnostic]) -> Result<(), SdkError> {
    if diagnostics.len() > MAX_DIAGNOSTICS_PER_ROW {
        return Err(invalid(
            "CUSTOMER_DATA_ROW_DIAGNOSTICS_LIMIT_EXCEEDED",
            "customer_data.row.diagnostics",
            format!("at most {MAX_DIAGNOSTICS_PER_ROW} diagnostics are allowed"),
        ));
    }
    for diagnostic in diagnostics {
        if diagnostic.code.is_empty() || diagnostic.field.is_empty() {
            return Err(invalid(
                "CUSTOMER_DATA_ROW_DIAGNOSTIC_INVALID",
                "customer_data.row.diagnostics",
                "diagnostics must retain canonical non-empty code and field values",
            ));
        }
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
            "customer_data.row.position",
            format!("row position must be between 1 and {MAX_IMPORT_ROWS}"),
        ));
    }
    Ok(())
}

fn validate_timestamp(field: &'static str, value: i64) -> Result<(), SdkError> {
    if value <= 0 {
        return Err(invalid(
            "CUSTOMER_DATA_IMPORT_TIME_INVALID",
            field,
            "timestamp must be positive Unix nanoseconds",
        ));
    }
    Ok(())
}

fn normalize_column_name(value: String) -> Result<String, SdkError> {
    normalize_bounded_text(
        value,
        MAX_COLUMN_NAME_BYTES,
        "CUSTOMER_DATA_IMPORT_COLUMN_NAME_INVALID",
        "customer_data.mapping.column",
        "column name",
    )
}

fn normalize_sha256(
    value: String,
    code: &'static str,
    field: &'static str,
) -> Result<String, SdkError> {
    if value.len() != 64
        || value
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
    Ok(value)
}

fn normalize_semantic_identifier(
    value: String,
    maximum_bytes: usize,
    code: &'static str,
    field: &'static str,
    label: &'static str,
) -> Result<String, SdkError> {
    let value = normalize_bounded_text(value, maximum_bytes, code, field, label)?;
    if value
        .as_bytes()
        .iter()
        .any(|byte| !(byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-')))
    {
        return Err(invalid(
            code,
            field,
            format!("{label} may contain only ASCII letters, digits, dot, underscore and hyphen"),
        ));
    }
    Ok(value)
}

fn normalize_bounded_text(
    value: String,
    maximum_bytes: usize,
    code: &'static str,
    field: &'static str,
    label: &'static str,
) -> Result<String, SdkError> {
    let value = value.trim().to_owned();
    if value.is_empty() || value.len() > maximum_bytes || value.chars().any(char::is_control) {
        return Err(invalid(
            code,
            field,
            format!(
                "{label} must be non-empty, contain no control characters and not exceed {maximum_bytes} UTF-8 bytes"
            ),
        ));
    }
    Ok(value)
}

fn hash_part(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn hash_optional(hasher: &mut Sha256, value: Option<&str>) {
    match value {
        Some(value) => {
            hash_part(hasher, b"some");
            hash_part(hasher, value.as_bytes());
        }
        None => hash_part(hasher, b"none"),
    }
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

fn invalid_row_state(internal: impl Into<String>) -> SdkError {
    invalid(
        "CUSTOMER_DATA_IMPORT_ROW_STATE_INVALID",
        "customer_data.import_row.state",
        internal,
    )
}

fn invalid(code: &'static str, field: &'static str, internal: impl Into<String>) -> SdkError {
    let mut error = SdkError::new(
        code,
        ErrorCategory::InvalidArgument,
        false,
        "The customer-data import request is invalid.",
    )
    .with_internal_reference(internal);
    error.field_violations.push(FieldViolation {
        field: FieldName::try_new(field).expect("static customer-data import field must be valid"),
        code: "INVALID".to_owned(),
        safe_message: "The customer-data import field is invalid.".to_owned(),
    });
    error
}

fn conflict(code: &'static str, internal: impl Into<String>) -> SdkError {
    SdkError::new(
        code,
        ErrorCategory::Conflict,
        false,
        "The customer-data import state conflicts with this operation.",
    )
    .with_internal_reference(internal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::ImportParserProfile;

    fn source(rows: u32) -> SourceDescriptor {
        SourceDescriptor::try_new(
            "customers.csv",
            "11".repeat(32),
            rows,
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

    fn job(rows: u32, policy: PartialExecutionPolicy) -> ImportJob {
        ImportJob::create(CreateImportJob {
            job_id: ImportJobId::try_new("import-job-1").unwrap(),
            source: source(rows),
            mapping: mapping(),
            partial_execution_policy: policy,
            occurred_at_unix_nanos: 1,
        })
        .unwrap()
    }

    #[test]
    fn mapping_version_distinguishes_external_identifier_semantics() {
        let first = mapping();
        let second = PartyImportMapping::try_new(
            None,
            "kind",
            "display_name",
            Some("other_external_id".to_owned()),
            Some("row_key".to_owned()),
        )
        .unwrap();
        assert_ne!(first.version_id().unwrap(), second.version_id().unwrap());
    }

    #[test]
    fn source_external_identifier_is_evidence_not_target_party_identity() {
        let row = ImportRow::create(CreateImportRow {
            job_id: ImportJobId::try_new("import-job-1").unwrap(),
            row_position: 1,
            external_row_key: Some("row-1".to_owned()),
            source_external_id: Some("legacy-customer-42".to_owned()),
            occurred_at_unix_nanos: 1,
        })
        .unwrap();
        let derived = row.derived_target_party_id().unwrap();
        assert_ne!(
            row.source_external_id_sha256().unwrap().as_str(),
            derived.as_str()
        );
        assert!(derived.as_str().starts_with("party-import-"));
    }

    #[test]
    fn deterministic_row_identity_and_target_idempotency_survive_replay() {
        let command = CreateImportRow {
            job_id: ImportJobId::try_new("import-job-1").unwrap(),
            row_position: 2,
            external_row_key: Some("external-row-42".to_owned()),
            source_external_id: Some("customer-42".to_owned()),
            occurred_at_unix_nanos: 1,
        };
        let first = ImportRow::create(command.clone()).unwrap();
        let second = ImportRow::create(command).unwrap();
        assert_eq!(first.row_id(), second.row_id());
        assert_eq!(first.target_idempotency_key(), second.target_idempotency_key());
        assert_eq!(
            first.derived_target_party_id().unwrap(),
            second.derived_target_party_id().unwrap()
        );
    }

    #[test]
    fn require_all_valid_blocks_execution_when_validation_found_errors() {
        let mut job = job(2, PartialExecutionPolicy::RequireAllValid);
        job.mark_validated(MarkImportJobValidated {
            expected_version: 1,
            valid_rows: 1,
            invalid_rows: 1,
            occurred_at_unix_nanos: 2,
        })
        .unwrap();
        let error = job
            .start_execution(StartImportExecution {
                expected_version: 2,
                occurred_at_unix_nanos: 3,
            })
            .unwrap_err();
        assert_eq!(
            error.code,
            "CUSTOMER_DATA_IMPORT_JOB_INVALID_ROWS_BLOCK_EXECUTION"
        );
    }

    #[test]
    fn all_valid_rows_policy_can_checkpoint_valid_and_invalid_rows() {
        let mut job = job(2, PartialExecutionPolicy::AllValidRows);
        job.mark_validated(MarkImportJobValidated {
            expected_version: 1,
            valid_rows: 1,
            invalid_rows: 1,
            occurred_at_unix_nanos: 2,
        })
        .unwrap();
        job.start_execution(StartImportExecution {
            expected_version: 2,
            occurred_at_unix_nanos: 3,
        })
        .unwrap();
        job.advance_checkpoint(AdvanceImportCheckpoint {
            expected_version: 3,
            row_position: 1,
            outcome: CheckpointOutcome::SkippedInvalid,
            occurred_at_unix_nanos: 4,
        })
        .unwrap();
        job.advance_checkpoint(AdvanceImportCheckpoint {
            expected_version: 4,
            row_position: 2,
            outcome: CheckpointOutcome::Succeeded,
            occurred_at_unix_nanos: 5,
        })
        .unwrap();
        job.complete(FinishImportJob {
            expected_version: 5,
            occurred_at_unix_nanos: 6,
        })
        .unwrap();
        assert_eq!(job.status(), ImportJobStatus::Completed);
    }

    #[test]
    fn row_success_requires_exact_prepared_target_party() {
        let mut row = ImportRow::create(CreateImportRow {
            job_id: ImportJobId::try_new("import-job-1").unwrap(),
            row_position: 1,
            external_row_key: None,
            source_external_id: None,
            occurred_at_unix_nanos: 1,
        })
        .unwrap();
        let target = row.derived_target_party_id().unwrap();
        row.mark_valid(ValidateImportRowSuccess {
            expected_version: 1,
            prepared_party: PreparedPartyRow::try_new(
                target.clone(),
                PartyImportKind::Person,
                "Ada Lovelace",
            )
            .unwrap(),
            occurred_at_unix_nanos: 2,
        })
        .unwrap();
        let error = row
            .mark_succeeded(MarkImportRowSucceeded {
                expected_version: 2,
                target_party_id: TargetPartyId::try_new("different-party").unwrap(),
                occurred_at_unix_nanos: 3,
            })
            .unwrap_err();
        assert_eq!(error.code, "CUSTOMER_DATA_IMPORT_ROW_TARGET_PARTY_CONFLICT");
        row.mark_succeeded(MarkImportRowSucceeded {
            expected_version: 2,
            target_party_id: target,
            occurred_at_unix_nanos: 3,
        })
        .unwrap();
        assert_eq!(row.status(), ImportRowStatus::Succeeded);
    }
}
