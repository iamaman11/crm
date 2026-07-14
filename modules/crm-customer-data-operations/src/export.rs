use crm_module_sdk::{ErrorCategory, FileId, RecordId, SdkError};
use sha2::{Digest, Sha256};

const MAX_PARTY_EXPORT_RESOURCES: u32 = 100_000;
const MAX_RETENTION_POLICY_ID_BYTES: usize = 128;
const MAX_EXECUTION_ERROR_CODE_BYTES: usize = 160;
const EXPORT_SPECIFICATION_ID_DOMAIN: &[u8] =
    b"crm.customer-data-operations.party-export-specification/v1";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExportJobId(RecordId);

impl ExportJobId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, SdkError> {
        RecordId::try_new(value.into()).map(Self).map_err(|error| {
            invalid(
                "CUSTOMER_DATA_EXPORT_JOB_ID_INVALID",
                "customer_data.export_job_id",
                error.to_string(),
            )
        })
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExportSpecificationVersionId(RecordId);

impl ExportSpecificationVersionId {
    fn derive(specification: &PartyExportSpecification) -> Result<Self, SdkError> {
        let mut hasher = Sha256::new();
        hasher.update(EXPORT_SPECIFICATION_ID_DOMAIN);
        hash_part(
            &mut hasher,
            match specification.scope.kind_filter {
                None => b"all".as_slice(),
                Some(PartyExportKindFilter::Person) => b"person".as_slice(),
                Some(PartyExportKindFilter::Organization) => b"organization".as_slice(),
            },
        );
        hash_part(
            &mut hasher,
            &specification.scope.maximum_resources.to_be_bytes(),
        );
        hash_part(&mut hasher, b"party-export-profile-v1");
        hash_part(&mut hasher, b"csv-utf8");
        hash_part(&mut hasher, b"canonicalization-v1");
        for field in &specification.profile.fields {
            hash_part(&mut hasher, field.canonical_name().as_bytes());
        }
        hash_part(
            &mut hasher,
            specification.profile.retention_policy_id.as_bytes(),
        );
        RecordId::try_new(format!("cdo-export-spec-{}", hex_digest(hasher.finalize())))
            .map(Self)
            .map_err(|error| {
                invalid(
                    "CUSTOMER_DATA_EXPORT_SPECIFICATION_VERSION_ID_INVALID",
                    "customer_data.export.specification_version_id",
                    error.to_string(),
                )
            })
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PartyExportKindFilter {
    Person,
    Organization,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PartyExportField {
    PartyId,
    Kind,
    DisplayName,
    ResourceVersion,
}

impl PartyExportField {
    pub const fn canonical_name(self) -> &'static str {
        match self {
            Self::PartyId => "party_id",
            Self::Kind => "kind",
            Self::DisplayName => "display_name",
            Self::ResourceVersion => "resource_version",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyExportScope {
    kind_filter: Option<PartyExportKindFilter>,
    maximum_resources: u32,
}

impl PartyExportScope {
    pub fn try_new(
        kind_filter: Option<PartyExportKindFilter>,
        maximum_resources: u32,
    ) -> Result<Self, SdkError> {
        if maximum_resources == 0 || maximum_resources > MAX_PARTY_EXPORT_RESOURCES {
            return Err(invalid(
                "CUSTOMER_DATA_EXPORT_MAXIMUM_RESOURCES_INVALID",
                "customer_data.export.scope.maximum_resources",
                format!(
                    "maximum resources must be between 1 and {MAX_PARTY_EXPORT_RESOURCES}"
                ),
            ));
        }
        Ok(Self {
            kind_filter,
            maximum_resources,
        })
    }

    pub const fn kind_filter(&self) -> Option<PartyExportKindFilter> {
        self.kind_filter
    }

    pub const fn maximum_resources(&self) -> u32 {
        self.maximum_resources
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyExportProfile {
    fields: Vec<PartyExportField>,
    retention_policy_id: String,
}

impl PartyExportProfile {
    pub fn v1(
        mut fields: Vec<PartyExportField>,
        retention_policy_id: impl Into<String>,
    ) -> Result<Self, SdkError> {
        if fields.is_empty() {
            return Err(invalid(
                "CUSTOMER_DATA_EXPORT_FIELDS_REQUIRED",
                "customer_data.export.profile.fields",
                "at least one Party export field is required",
            ));
        }
        fields.sort_unstable();
        if fields.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(invalid(
                "CUSTOMER_DATA_EXPORT_FIELD_DUPLICATE",
                "customer_data.export.profile.fields",
                "Party export fields must be unique",
            ));
        }
        if !fields.contains(&PartyExportField::PartyId) {
            return Err(invalid(
                "CUSTOMER_DATA_EXPORT_PARTY_ID_REQUIRED",
                "customer_data.export.profile.fields",
                "Party export must include party_id",
            ));
        }
        let retention_policy_id = normalize_identifier(
            retention_policy_id.into(),
            MAX_RETENTION_POLICY_ID_BYTES,
            "CUSTOMER_DATA_EXPORT_RETENTION_POLICY_INVALID",
            "customer_data.export.profile.retention_policy_id",
        )?;
        Ok(Self {
            fields,
            retention_policy_id,
        })
    }

    pub fn fields(&self) -> &[PartyExportField] {
        &self.fields
    }

    pub fn retention_policy_id(&self) -> &str {
        &self.retention_policy_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyExportSpecification {
    scope: PartyExportScope,
    profile: PartyExportProfile,
    version_id: ExportSpecificationVersionId,
}

impl PartyExportSpecification {
    pub fn try_new(
        scope: PartyExportScope,
        profile: PartyExportProfile,
    ) -> Result<Self, SdkError> {
        let mut specification = Self {
            scope,
            profile,
            version_id: ExportSpecificationVersionId(
                RecordId::try_new("cdo-export-spec-placeholder").map_err(|error| {
                    invalid(
                        "CUSTOMER_DATA_EXPORT_SPECIFICATION_VERSION_ID_INVALID",
                        "customer_data.export.specification_version_id",
                        error.to_string(),
                    )
                })?,
            ),
        };
        specification.version_id = ExportSpecificationVersionId::derive(&specification)?;
        Ok(specification)
    }

    pub fn scope(&self) -> &PartyExportScope {
        &self.scope
    }

    pub fn profile(&self) -> &PartyExportProfile {
        &self.profile
    }

    pub fn version_id(&self) -> &ExportSpecificationVersionId {
        &self.version_id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartyExportJobStatus {
    Created,
    Selecting,
    Ready,
    Executing,
    Completed,
    FailedRetryable,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyExportSelectionSummary {
    manifest_sha256: String,
    selected_resources: u32,
}

impl PartyExportSelectionSummary {
    pub fn try_new(
        manifest_sha256: impl Into<String>,
        selected_resources: u32,
        maximum_resources: u32,
    ) -> Result<Self, SdkError> {
        if selected_resources > maximum_resources {
            return Err(invalid(
                "CUSTOMER_DATA_EXPORT_SELECTION_LIMIT_EXCEEDED",
                "customer_data.export.selection.selected_resources",
                "selected resource count exceeds immutable export scope limit",
            ));
        }
        Ok(Self {
            manifest_sha256: normalize_sha256(
                manifest_sha256.into(),
                "CUSTOMER_DATA_EXPORT_SELECTION_SHA256_INVALID",
                "customer_data.export.selection.manifest_sha256",
            )?,
            selected_resources,
        })
    }

    pub fn manifest_sha256(&self) -> &str {
        &self.manifest_sha256
    }

    pub const fn selected_resources(&self) -> u32 {
        self.selected_resources
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyExportArtifactEvidence {
    file_id: FileId,
    content_sha256: String,
    size_bytes: u64,
    retention_policy_id: String,
}

impl PartyExportArtifactEvidence {
    pub fn try_new(
        file_id: FileId,
        content_sha256: impl Into<String>,
        size_bytes: u64,
        retention_policy_id: impl Into<String>,
    ) -> Result<Self, SdkError> {
        Ok(Self {
            file_id,
            content_sha256: normalize_sha256(
                content_sha256.into(),
                "CUSTOMER_DATA_EXPORT_ARTIFACT_SHA256_INVALID",
                "customer_data.export.artifact.content_sha256",
            )?,
            size_bytes,
            retention_policy_id: normalize_identifier(
                retention_policy_id.into(),
                MAX_RETENTION_POLICY_ID_BYTES,
                "CUSTOMER_DATA_EXPORT_RETENTION_POLICY_INVALID",
                "customer_data.export.artifact.retention_policy_id",
            )?,
        })
    }

    pub fn file_id(&self) -> &FileId {
        &self.file_id
    }

    pub fn content_sha256(&self) -> &str {
        &self.content_sha256
    }

    pub const fn size_bytes(&self) -> u64 {
        self.size_bytes
    }

    pub fn retention_policy_id(&self) -> &str {
        &self.retention_policy_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyExportReconciliation {
    selected_resources: u32,
    emitted_rows: u32,
    excluded_not_visible: u32,
    excluded_version_changed: u32,
    excluded_unavailable: u32,
    redacted_fields: u32,
}

impl PartyExportReconciliation {
    pub fn try_new(
        selected_resources: u32,
        emitted_rows: u32,
        excluded_not_visible: u32,
        excluded_version_changed: u32,
        excluded_unavailable: u32,
        redacted_fields: u32,
    ) -> Result<Self, SdkError> {
        let accounted = emitted_rows
            .checked_add(excluded_not_visible)
            .and_then(|value| value.checked_add(excluded_version_changed))
            .and_then(|value| value.checked_add(excluded_unavailable))
            .ok_or_else(|| {
                export_error(
                    "CUSTOMER_DATA_EXPORT_RECONCILIATION_OVERFLOW",
                    ErrorCategory::Conflict,
                    "Export reconciliation counters overflowed.",
                )
            })?;
        if accounted != selected_resources {
            return Err(invalid(
                "CUSTOMER_DATA_EXPORT_RECONCILIATION_INVALID",
                "customer_data.export.reconciliation",
                "selected resources must equal emitted plus excluded resources",
            ));
        }
        Ok(Self {
            selected_resources,
            emitted_rows,
            excluded_not_visible,
            excluded_version_changed,
            excluded_unavailable,
            redacted_fields,
        })
    }

    pub const fn selected_resources(&self) -> u32 {
        self.selected_resources
    }

    pub const fn emitted_rows(&self) -> u32 {
        self.emitted_rows
    }

    pub const fn excluded_not_visible(&self) -> u32 {
        self.excluded_not_visible
    }

    pub const fn excluded_version_changed(&self) -> u32 {
        self.excluded_version_changed
    }

    pub const fn excluded_unavailable(&self) -> u32 {
        self.excluded_unavailable
    }

    pub const fn redacted_fields(&self) -> u32 {
        self.redacted_fields
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyExportJob {
    job_id: ExportJobId,
    specification: PartyExportSpecification,
    status: PartyExportJobStatus,
    selection: Option<PartyExportSelectionSummary>,
    checkpoint_manifest_position: u32,
    execution_attempts: u32,
    last_execution_error_code: Option<String>,
    artifact: Option<PartyExportArtifactEvidence>,
    reconciliation: Option<PartyExportReconciliation>,
    created_at_unix_nanos: i64,
    updated_at_unix_nanos: i64,
    version: i64,
}

impl PartyExportJob {
    pub fn create(
        job_id: ExportJobId,
        specification: PartyExportSpecification,
        occurred_at_unix_nanos: i64,
    ) -> Result<Self, SdkError> {
        validate_timestamp(occurred_at_unix_nanos)?;
        Ok(Self {
            job_id,
            specification,
            status: PartyExportJobStatus::Created,
            selection: None,
            checkpoint_manifest_position: 0,
            execution_attempts: 0,
            last_execution_error_code: None,
            artifact: None,
            reconciliation: None,
            created_at_unix_nanos: occurred_at_unix_nanos,
            updated_at_unix_nanos: occurred_at_unix_nanos,
            version: 1,
        })
    }

    pub fn start_or_resume(
        &mut self,
        expected_version: i64,
        occurred_at_unix_nanos: i64,
    ) -> Result<(), SdkError> {
        self.require_version(expected_version)?;
        self.status = match self.status {
            PartyExportJobStatus::Created => PartyExportJobStatus::Selecting,
            PartyExportJobStatus::Ready => PartyExportJobStatus::Executing,
            PartyExportJobStatus::FailedRetryable if self.selection.is_some() => {
                PartyExportJobStatus::Executing
            }
            PartyExportJobStatus::FailedRetryable => PartyExportJobStatus::Selecting,
            _ => {
                return Err(export_error(
                    "CUSTOMER_DATA_EXPORT_JOB_NOT_STARTABLE",
                    ErrorCategory::Conflict,
                    "The export job cannot start or resume from its current state.",
                ));
            }
        };
        self.last_execution_error_code = None;
        self.advance(occurred_at_unix_nanos)
    }

    pub fn complete_selection(
        &mut self,
        expected_version: i64,
        selection: PartyExportSelectionSummary,
        occurred_at_unix_nanos: i64,
    ) -> Result<(), SdkError> {
        self.require_version(expected_version)?;
        if self.status != PartyExportJobStatus::Selecting || self.selection.is_some() {
            return Err(export_error(
                "CUSTOMER_DATA_EXPORT_SELECTION_STATE_INVALID",
                ErrorCategory::Conflict,
                "The export selection cannot be completed from its current state.",
            ));
        }
        if selection.selected_resources() > self.specification.scope.maximum_resources() {
            return Err(export_error(
                "CUSTOMER_DATA_EXPORT_SELECTION_LIMIT_EXCEEDED",
                ErrorCategory::Conflict,
                "The export selection exceeds its immutable scope limit.",
            ));
        }
        self.selection = Some(selection);
        self.checkpoint_manifest_position = 0;
        self.status = PartyExportJobStatus::Ready;
        self.advance(occurred_at_unix_nanos)
    }

    pub fn advance_checkpoint(
        &mut self,
        expected_version: i64,
        next_manifest_position: u32,
        occurred_at_unix_nanos: i64,
    ) -> Result<(), SdkError> {
        self.require_version(expected_version)?;
        if self.status != PartyExportJobStatus::Executing {
            return Err(export_error(
                "CUSTOMER_DATA_EXPORT_CHECKPOINT_STATE_INVALID",
                ErrorCategory::Conflict,
                "The export checkpoint can advance only while executing.",
            ));
        }
        let selected = self
            .selection
            .as_ref()
            .ok_or_else(|| inconsistent("executing export job is missing selection evidence"))?
            .selected_resources();
        let expected_next = self
            .checkpoint_manifest_position
            .checked_add(1)
            .ok_or_else(|| inconsistent("export checkpoint overflowed"))?;
        if next_manifest_position != expected_next || next_manifest_position > selected {
            return Err(export_error(
                "CUSTOMER_DATA_EXPORT_CHECKPOINT_NON_SEQUENTIAL",
                ErrorCategory::Conflict,
                "The export checkpoint must advance exactly one selected manifest position.",
            ));
        }
        self.checkpoint_manifest_position = next_manifest_position;
        self.advance(occurred_at_unix_nanos)
    }

    pub fn record_retryable_failure(
        &mut self,
        expected_version: i64,
        error_code: impl Into<String>,
        occurred_at_unix_nanos: i64,
    ) -> Result<(), SdkError> {
        self.require_version(expected_version)?;
        if !matches!(
            self.status,
            PartyExportJobStatus::Selecting | PartyExportJobStatus::Executing
        ) {
            return Err(export_error(
                "CUSTOMER_DATA_EXPORT_RETRYABLE_FAILURE_STATE_INVALID",
                ErrorCategory::Conflict,
                "Retryable export failure can be recorded only during selection or execution.",
            ));
        }
        self.execution_attempts = self.execution_attempts.checked_add(1).ok_or_else(|| {
            export_error(
                "CUSTOMER_DATA_EXPORT_EXECUTION_ATTEMPTS_EXHAUSTED",
                ErrorCategory::Conflict,
                "The export job cannot record another execution attempt.",
            )
        })?;
        self.last_execution_error_code = Some(normalize_identifier(
            error_code.into(),
            MAX_EXECUTION_ERROR_CODE_BYTES,
            "CUSTOMER_DATA_EXPORT_EXECUTION_ERROR_CODE_INVALID",
            "customer_data.export.last_execution_error_code",
        )?);
        self.status = PartyExportJobStatus::FailedRetryable;
        self.advance(occurred_at_unix_nanos)
    }

    pub fn complete(
        &mut self,
        expected_version: i64,
        artifact: PartyExportArtifactEvidence,
        reconciliation: PartyExportReconciliation,
        occurred_at_unix_nanos: i64,
    ) -> Result<(), SdkError> {
        self.require_version(expected_version)?;
        if self.status != PartyExportJobStatus::Executing {
            return Err(export_error(
                "CUSTOMER_DATA_EXPORT_COMPLETION_STATE_INVALID",
                ErrorCategory::Conflict,
                "The export job can complete only while executing.",
            ));
        }
        let selection = self
            .selection
            .as_ref()
            .ok_or_else(|| inconsistent("executing export job is missing selection evidence"))?;
        if self.checkpoint_manifest_position != selection.selected_resources()
            || reconciliation.selected_resources() != selection.selected_resources()
        {
            return Err(export_error(
                "CUSTOMER_DATA_EXPORT_COMPLETION_RECONCILIATION_INVALID",
                ErrorCategory::Conflict,
                "The export job cannot complete before every selected resource is reconciled.",
            ));
        }
        if artifact.retention_policy_id() != self.specification.profile.retention_policy_id() {
            return Err(export_error(
                "CUSTOMER_DATA_EXPORT_ARTIFACT_RETENTION_CONFLICT",
                ErrorCategory::Conflict,
                "The finalized export artifact retention policy does not match the immutable specification.",
            ));
        }
        self.artifact = Some(artifact);
        self.reconciliation = Some(reconciliation);
        self.last_execution_error_code = None;
        self.status = PartyExportJobStatus::Completed;
        self.advance(occurred_at_unix_nanos)
    }

    pub fn cancel(
        &mut self,
        expected_version: i64,
        occurred_at_unix_nanos: i64,
    ) -> Result<(), SdkError> {
        self.require_version(expected_version)?;
        if matches!(
            self.status,
            PartyExportJobStatus::Completed | PartyExportJobStatus::Cancelled
        ) {
            return Err(export_error(
                "CUSTOMER_DATA_EXPORT_JOB_TERMINAL",
                ErrorCategory::Conflict,
                "A terminal export job cannot be cancelled.",
            ));
        }
        self.status = PartyExportJobStatus::Cancelled;
        self.advance(occurred_at_unix_nanos)
    }

    pub fn job_id(&self) -> &ExportJobId {
        &self.job_id
    }

    pub fn specification(&self) -> &PartyExportSpecification {
        &self.specification
    }

    pub const fn status(&self) -> PartyExportJobStatus {
        self.status
    }

    pub fn selection(&self) -> Option<&PartyExportSelectionSummary> {
        self.selection.as_ref()
    }

    pub const fn checkpoint_manifest_position(&self) -> u32 {
        self.checkpoint_manifest_position
    }

    pub const fn execution_attempts(&self) -> u32 {
        self.execution_attempts
    }

    pub fn last_execution_error_code(&self) -> Option<&str> {
        self.last_execution_error_code.as_deref()
    }

    pub fn artifact(&self) -> Option<&PartyExportArtifactEvidence> {
        self.artifact.as_ref()
    }

    pub fn reconciliation(&self) -> Option<&PartyExportReconciliation> {
        self.reconciliation.as_ref()
    }

    pub const fn created_at_unix_nanos(&self) -> i64 {
        self.created_at_unix_nanos
    }

    pub const fn updated_at_unix_nanos(&self) -> i64 {
        self.updated_at_unix_nanos
    }

    pub const fn version(&self) -> i64 {
        self.version
    }

    fn require_version(&self, expected_version: i64) -> Result<(), SdkError> {
        if expected_version == self.version {
            Ok(())
        } else {
            Err(export_error(
                "CUSTOMER_DATA_EXPORT_VERSION_CONFLICT",
                ErrorCategory::Conflict,
                "The export job version is stale.",
            ))
        }
    }

    fn advance(&mut self, occurred_at_unix_nanos: i64) -> Result<(), SdkError> {
        validate_timestamp(occurred_at_unix_nanos)?;
        if occurred_at_unix_nanos < self.updated_at_unix_nanos {
            return Err(invalid(
                "CUSTOMER_DATA_EXPORT_TIME_REGRESSION",
                "customer_data.export.updated_at",
                "export job time cannot move backwards",
            ));
        }
        self.version = self.version.checked_add(1).ok_or_else(|| {
            export_error(
                "CUSTOMER_DATA_EXPORT_VERSION_EXHAUSTED",
                ErrorCategory::Conflict,
                "The export job cannot advance another version.",
            )
        })?;
        self.updated_at_unix_nanos = occurred_at_unix_nanos;
        Ok(())
    }
}

fn validate_timestamp(value: i64) -> Result<(), SdkError> {
    if value <= 0 {
        return Err(invalid(
            "CUSTOMER_DATA_EXPORT_TIME_INVALID",
            "customer_data.export.time",
            "timestamp must be positive Unix nanoseconds",
        ));
    }
    Ok(())
}

fn normalize_identifier(
    value: String,
    maximum_bytes: usize,
    code: &'static str,
    field: &'static str,
) -> Result<String, SdkError> {
    let value = value.trim().to_owned();
    if value.is_empty()
        || value.len() > maximum_bytes
        || value.chars().any(|character| character.is_control())
    {
        return Err(invalid(code, field, "identifier is empty, too long or contains control characters"));
    }
    Ok(value)
}

fn normalize_sha256(
    value: String,
    code: &'static str,
    field: &'static str,
) -> Result<String, SdkError> {
    let value = value.to_ascii_lowercase();
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(invalid(code, field, "SHA-256 must be exactly 64 hexadecimal characters"));
    }
    Ok(value)
}

fn hash_part(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}

fn hex_digest(bytes: impl AsRef<[u8]>) -> String {
    let bytes = bytes.as_ref();
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn invalid(
    code: &'static str,
    field: &'static str,
    message: impl Into<String>,
) -> SdkError {
    let mut error = SdkError::invalid_argument(field, message.into());
    error.code = code.to_owned();
    error
}

fn export_error(
    code: &'static str,
    category: ErrorCategory,
    safe_message: &'static str,
) -> SdkError {
    SdkError::new(code, category, false, safe_message)
}

fn inconsistent(detail: &'static str) -> SdkError {
    let _ = detail;
    export_error(
        "CUSTOMER_DATA_EXPORT_STATE_INCONSISTENT",
        ErrorCategory::Internal,
        "Stored customer export state is inconsistent.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn specification(fields: Vec<PartyExportField>) -> PartyExportSpecification {
        PartyExportSpecification::try_new(
            PartyExportScope::try_new(None, 10).unwrap(),
            PartyExportProfile::v1(fields, "customer-export-30d").unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn specification_identity_is_stable_under_input_field_order() {
        let left = specification(vec![
            PartyExportField::DisplayName,
            PartyExportField::PartyId,
            PartyExportField::Kind,
        ]);
        let right = specification(vec![
            PartyExportField::Kind,
            PartyExportField::PartyId,
            PartyExportField::DisplayName,
        ]);
        assert_eq!(left.profile().fields(), right.profile().fields());
        assert_eq!(left.version_id(), right.version_id());
    }

    #[test]
    fn rejects_duplicate_fields_and_missing_party_id() {
        assert!(
            PartyExportProfile::v1(
                vec![PartyExportField::PartyId, PartyExportField::PartyId],
                "customer-export-30d"
            )
            .is_err()
        );
        assert!(
            PartyExportProfile::v1(
                vec![PartyExportField::DisplayName],
                "customer-export-30d"
            )
            .is_err()
        );
    }

    #[test]
    fn completes_only_after_every_selected_resource_is_reconciled() {
        let mut job = PartyExportJob::create(
            ExportJobId::try_new("export-job-1").unwrap(),
            specification(vec![PartyExportField::PartyId, PartyExportField::DisplayName]),
            10,
        )
        .unwrap();
        job.start_or_resume(1, 20).unwrap();
        job.complete_selection(
            2,
            PartyExportSelectionSummary::try_new("11".repeat(32), 2, 10).unwrap(),
            30,
        )
        .unwrap();
        job.start_or_resume(3, 40).unwrap();
        job.advance_checkpoint(4, 1, 50).unwrap();
        job.advance_checkpoint(5, 2, 60).unwrap();
        job.complete(
            6,
            PartyExportArtifactEvidence::try_new(
                FileId::try_new("export-file-1").unwrap(),
                "22".repeat(32),
                128,
                "customer-export-30d",
            )
            .unwrap(),
            PartyExportReconciliation::try_new(2, 1, 0, 1, 0, 0).unwrap(),
            70,
        )
        .unwrap();
        assert_eq!(job.status(), PartyExportJobStatus::Completed);
        assert_eq!(job.version(), 7);
    }

    #[test]
    fn retry_resumes_selection_or_execution_without_changing_intent() {
        let mut selecting = PartyExportJob::create(
            ExportJobId::try_new("export-job-select-retry").unwrap(),
            specification(vec![PartyExportField::PartyId]),
            10,
        )
        .unwrap();
        selecting.start_or_resume(1, 20).unwrap();
        selecting
            .record_retryable_failure(2, "PARTY_LIST_TEMPORARY", 30)
            .unwrap();
        selecting.start_or_resume(3, 40).unwrap();
        assert_eq!(selecting.status(), PartyExportJobStatus::Selecting);

        selecting
            .complete_selection(
                4,
                PartyExportSelectionSummary::try_new("33".repeat(32), 1, 10).unwrap(),
                50,
            )
            .unwrap();
        selecting.start_or_resume(5, 60).unwrap();
        selecting
            .record_retryable_failure(6, "FILE_STORE_TEMPORARY", 70)
            .unwrap();
        selecting.start_or_resume(7, 80).unwrap();
        assert_eq!(selecting.status(), PartyExportJobStatus::Executing);
        assert_eq!(selecting.specification().version_id(), specification(vec![PartyExportField::PartyId]).version_id());
    }

    #[test]
    fn reconciliation_requires_exact_selected_accounting() {
        assert!(PartyExportReconciliation::try_new(3, 1, 1, 0, 0, 0).is_err());
        assert!(PartyExportReconciliation::try_new(3, 1, 1, 1, 0, 2).is_ok());
    }
}
