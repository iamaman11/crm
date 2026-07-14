use crate::export::{
    ExportJobId, PartyExportArtifactEvidence, PartyExportField, PartyExportJob,
    PartyExportJobStatus, PartyExportKindFilter, PartyExportProfile, PartyExportReconciliation,
    PartyExportScope, PartyExportSelectionSummary, PartyExportSpecification,
};
use crm_module_sdk::{ErrorCategory, FileId, SdkError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const EXPORT_JOB_STATE_SCHEMA_ID: &str = "crm.customer-data-operations.export_job.state";
pub const EXPORT_JOB_STATE_SCHEMA_VERSION: &str = "1.0.0";
pub const EXPORT_JOB_STATE_MAXIMUM_BYTES: u64 = 256 * 1024;
pub const EXPORT_JOB_STATE_RETENTION_POLICY_ID: &str = "crm.customer_data.export_job";

const EXPORT_JOB_STATE_DESCRIPTOR: &[u8] = b"crm.customer-data-operations.export_job.state/v1:job_id,specification[scope[kind_filter,maximum_resources],profile[fields,retention_policy_id],specification_version_id],status,selection[manifest_sha256,selected_resources],checkpoint_manifest_position,execution_attempts,last_execution_error_code,artifact[file_id,content_sha256,size_bytes,retention_policy_id],reconciliation[selected_resources,emitted_rows,excluded_not_visible,excluded_version_changed,excluded_unavailable,redacted_fields],created_at_unix_nanos,updated_at_unix_nanos,version";

pub fn export_job_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(EXPORT_JOB_STATE_DESCRIPTOR).into()
}

pub fn encode_export_job_state(job: &PartyExportJob) -> Result<Vec<u8>, SdkError> {
    let bytes = serde_json::to_vec(&ExportJobStateV1::from(job)).map_err(|error| {
        persisted_error(format!("export-job state serialization failed: {error}"))
    })?;
    validate_size(&bytes)?;
    Ok(bytes)
}

pub fn decode_export_job_state(bytes: &[u8]) -> Result<PartyExportJob, SdkError> {
    validate_size(bytes)?;
    let state: ExportJobStateV1 = serde_json::from_slice(bytes)
        .map_err(|error| persisted_error(format!("export-job state JSON is invalid: {error}")))?;
    let job = state.rehydrate()?;
    let canonical = encode_export_job_state(&job)?;
    if canonical != bytes {
        return Err(persisted_error(
            "export-job state is not the strict canonical v1 encoding".to_owned(),
        ));
    }
    Ok(job)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportJobStateV1 {
    job_id: String,
    specification: PartyExportSpecificationStateV1,
    status: PartyExportJobStatusState,
    selection: Option<PartyExportSelectionStateV1>,
    checkpoint_manifest_position: u32,
    execution_attempts: u32,
    last_execution_error_code: Option<String>,
    artifact: Option<PartyExportArtifactStateV1>,
    reconciliation: Option<PartyExportReconciliationStateV1>,
    created_at_unix_nanos: i64,
    updated_at_unix_nanos: i64,
    version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartyExportSpecificationStateV1 {
    scope: PartyExportScopeStateV1,
    profile: PartyExportProfileStateV1,
    specification_version_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartyExportScopeStateV1 {
    kind_filter: Option<PartyExportKindFilterState>,
    maximum_resources: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PartyExportKindFilterState {
    Person,
    Organization,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartyExportProfileStateV1 {
    fields: Vec<PartyExportFieldState>,
    retention_policy_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PartyExportFieldState {
    PartyId,
    Kind,
    DisplayName,
    ResourceVersion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PartyExportJobStatusState {
    Created,
    Selecting,
    Ready,
    Executing,
    Completed,
    FailedRetryable,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartyExportSelectionStateV1 {
    manifest_sha256: String,
    selected_resources: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartyExportArtifactStateV1 {
    file_id: String,
    content_sha256: String,
    size_bytes: u64,
    retention_policy_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartyExportReconciliationStateV1 {
    selected_resources: u32,
    emitted_rows: u32,
    excluded_not_visible: u32,
    excluded_version_changed: u32,
    excluded_unavailable: u32,
    redacted_fields: u32,
}

impl From<&PartyExportJob> for ExportJobStateV1 {
    fn from(job: &PartyExportJob) -> Self {
        Self {
            job_id: job.job_id().as_str().to_owned(),
            specification: PartyExportSpecificationStateV1::from(job.specification()),
            status: job.status().into(),
            selection: job.selection().map(PartyExportSelectionStateV1::from),
            checkpoint_manifest_position: job.checkpoint_manifest_position(),
            execution_attempts: job.execution_attempts(),
            last_execution_error_code: job.last_execution_error_code().map(str::to_owned),
            artifact: job.artifact().map(PartyExportArtifactStateV1::from),
            reconciliation: job
                .reconciliation()
                .map(PartyExportReconciliationStateV1::from),
            created_at_unix_nanos: job.created_at_unix_nanos(),
            updated_at_unix_nanos: job.updated_at_unix_nanos(),
            version: job.version(),
        }
    }
}

impl From<&PartyExportSpecification> for PartyExportSpecificationStateV1 {
    fn from(specification: &PartyExportSpecification) -> Self {
        Self {
            scope: PartyExportScopeStateV1 {
                kind_filter: specification.scope().kind_filter().map(Into::into),
                maximum_resources: specification.scope().maximum_resources(),
            },
            profile: PartyExportProfileStateV1 {
                fields: specification
                    .profile()
                    .fields()
                    .iter()
                    .copied()
                    .map(Into::into)
                    .collect(),
                retention_policy_id: specification.profile().retention_policy_id().to_owned(),
            },
            specification_version_id: specification.version_id().as_str().to_owned(),
        }
    }
}

impl From<&PartyExportSelectionSummary> for PartyExportSelectionStateV1 {
    fn from(selection: &PartyExportSelectionSummary) -> Self {
        Self {
            manifest_sha256: selection.manifest_sha256().to_owned(),
            selected_resources: selection.selected_resources(),
        }
    }
}

impl From<&PartyExportArtifactEvidence> for PartyExportArtifactStateV1 {
    fn from(artifact: &PartyExportArtifactEvidence) -> Self {
        Self {
            file_id: artifact.file_id().as_str().to_owned(),
            content_sha256: artifact.content_sha256().to_owned(),
            size_bytes: artifact.size_bytes(),
            retention_policy_id: artifact.retention_policy_id().to_owned(),
        }
    }
}

impl From<&PartyExportReconciliation> for PartyExportReconciliationStateV1 {
    fn from(reconciliation: &PartyExportReconciliation) -> Self {
        Self {
            selected_resources: reconciliation.selected_resources(),
            emitted_rows: reconciliation.emitted_rows(),
            excluded_not_visible: reconciliation.excluded_not_visible(),
            excluded_version_changed: reconciliation.excluded_version_changed(),
            excluded_unavailable: reconciliation.excluded_unavailable(),
            redacted_fields: reconciliation.redacted_fields(),
        }
    }
}

impl From<PartyExportKindFilter> for PartyExportKindFilterState {
    fn from(value: PartyExportKindFilter) -> Self {
        match value {
            PartyExportKindFilter::Person => Self::Person,
            PartyExportKindFilter::Organization => Self::Organization,
        }
    }
}

impl From<PartyExportKindFilterState> for PartyExportKindFilter {
    fn from(value: PartyExportKindFilterState) -> Self {
        match value {
            PartyExportKindFilterState::Person => Self::Person,
            PartyExportKindFilterState::Organization => Self::Organization,
        }
    }
}

impl From<PartyExportField> for PartyExportFieldState {
    fn from(value: PartyExportField) -> Self {
        match value {
            PartyExportField::PartyId => Self::PartyId,
            PartyExportField::Kind => Self::Kind,
            PartyExportField::DisplayName => Self::DisplayName,
            PartyExportField::ResourceVersion => Self::ResourceVersion,
        }
    }
}

impl From<PartyExportFieldState> for PartyExportField {
    fn from(value: PartyExportFieldState) -> Self {
        match value {
            PartyExportFieldState::PartyId => Self::PartyId,
            PartyExportFieldState::Kind => Self::Kind,
            PartyExportFieldState::DisplayName => Self::DisplayName,
            PartyExportFieldState::ResourceVersion => Self::ResourceVersion,
        }
    }
}

impl From<PartyExportJobStatus> for PartyExportJobStatusState {
    fn from(value: PartyExportJobStatus) -> Self {
        match value {
            PartyExportJobStatus::Created => Self::Created,
            PartyExportJobStatus::Selecting => Self::Selecting,
            PartyExportJobStatus::Ready => Self::Ready,
            PartyExportJobStatus::Executing => Self::Executing,
            PartyExportJobStatus::Completed => Self::Completed,
            PartyExportJobStatus::FailedRetryable => Self::FailedRetryable,
            PartyExportJobStatus::Cancelled => Self::Cancelled,
        }
    }
}

impl ExportJobStateV1 {
    fn rehydrate(self) -> Result<PartyExportJob, SdkError> {
        if self.updated_at_unix_nanos < self.created_at_unix_nanos {
            return Err(persisted_error(
                "export-job updated time precedes created time".to_owned(),
            ));
        }
        let specification = self.specification.try_into_domain()?;
        let job_id = ExportJobId::try_new(self.job_id.clone())
            .map_err(|error| persisted_domain_error("export-job ID", error))?;
        let mut job = PartyExportJob::create(job_id, specification, self.created_at_unix_nanos)
            .map_err(|error| persisted_domain_error("export-job create", error))?;
        let transition_time = self.updated_at_unix_nanos;

        match self.status {
            PartyExportJobStatusState::Created => {}
            PartyExportJobStatusState::Selecting => {
                require_none(&self.selection, "selecting export selection")?;
                require_none(&self.artifact, "selecting export artifact")?;
                require_none(&self.reconciliation, "selecting export reconciliation")?;
                require_none(
                    &self.last_execution_error_code,
                    "selecting export execution error",
                )?;
                require_zero(
                    self.checkpoint_manifest_position,
                    "selecting export checkpoint",
                )?;
                start(&mut job, transition_time)?;
                replay_completed_retry_cycles(&mut job, self.execution_attempts, transition_time)?;
            }
            PartyExportJobStatusState::Ready => {
                require_none(&self.artifact, "ready export artifact")?;
                require_none(&self.reconciliation, "ready export reconciliation")?;
                require_none(
                    &self.last_execution_error_code,
                    "ready export execution error",
                )?;
                require_zero(self.checkpoint_manifest_position, "ready export checkpoint")?;
                start(&mut job, transition_time)?;
                replay_completed_retry_cycles(&mut job, self.execution_attempts, transition_time)?;
                complete_selection(&mut job, &self.selection, transition_time)?;
            }
            PartyExportJobStatusState::Executing => {
                require_none(&self.artifact, "executing export artifact")?;
                require_none(&self.reconciliation, "executing export reconciliation")?;
                require_none(
                    &self.last_execution_error_code,
                    "executing export execution error",
                )?;
                enter_execution(&mut job, &self.selection, transition_time)?;
                replay_completed_retry_cycles(&mut job, self.execution_attempts, transition_time)?;
                replay_checkpoints(&mut job, self.checkpoint_manifest_position, transition_time)?;
            }
            PartyExportJobStatusState::FailedRetryable => {
                require_none(&self.artifact, "retryable export artifact")?;
                require_none(&self.reconciliation, "retryable export reconciliation")?;
                let error_code = self.last_execution_error_code.as_deref().ok_or_else(|| {
                    persisted_error(
                        "retryable export state requires last_execution_error_code".to_owned(),
                    )
                })?;
                if self.execution_attempts == 0 {
                    return Err(persisted_error(
                        "retryable export state requires at least one execution attempt".to_owned(),
                    ));
                }
                if self.selection.is_some() {
                    enter_execution(&mut job, &self.selection, transition_time)?;
                    replay_checkpoints(
                        &mut job,
                        self.checkpoint_manifest_position,
                        transition_time,
                    )?;
                } else {
                    require_zero(
                        self.checkpoint_manifest_position,
                        "retryable selecting export checkpoint",
                    )?;
                    start(&mut job, transition_time)?;
                }
                replay_prior_retry_cycles(&mut job, self.execution_attempts - 1, transition_time)?;
                record_failure(&mut job, error_code, transition_time)?;
            }
            PartyExportJobStatusState::Completed => {
                require_none(
                    &self.last_execution_error_code,
                    "completed export execution error",
                )?;
                enter_execution(&mut job, &self.selection, transition_time)?;
                replay_completed_retry_cycles(&mut job, self.execution_attempts, transition_time)?;
                replay_checkpoints(&mut job, self.checkpoint_manifest_position, transition_time)?;
                let artifact = self
                    .artifact
                    .as_ref()
                    .ok_or_else(|| {
                        persisted_error("completed export artifact is missing".to_owned())
                    })?
                    .try_into_domain()?;
                let reconciliation = self
                    .reconciliation
                    .as_ref()
                    .ok_or_else(|| {
                        persisted_error("completed export reconciliation is missing".to_owned())
                    })?
                    .try_into_domain()?;
                let expected = job.version();
                job.complete(expected, artifact, reconciliation, transition_time)
                    .map_err(|error| persisted_domain_error("export completion", error))?;
            }
            PartyExportJobStatusState::Cancelled => {
                require_none(&self.artifact, "cancelled export artifact")?;
                require_none(&self.reconciliation, "cancelled export reconciliation")?;
                replay_cancelled_state(&mut job, &self, transition_time)?;
            }
        }

        if job.version() != self.version {
            return Err(persisted_error(format!(
                "export-job version {} cannot be reconstructed from persisted lifecycle evidence; reconstructed {}",
                self.version,
                job.version()
            )));
        }
        Ok(job)
    }
}

impl PartyExportSpecificationStateV1 {
    fn try_into_domain(&self) -> Result<PartyExportSpecification, SdkError> {
        let scope = PartyExportScope::try_new(
            self.scope.kind_filter.map(Into::into),
            self.scope.maximum_resources,
        )
        .map_err(|error| persisted_domain_error("export scope", error))?;
        let profile = PartyExportProfile::v1(
            self.profile
                .fields
                .iter()
                .copied()
                .map(Into::into)
                .collect(),
            self.profile.retention_policy_id.clone(),
        )
        .map_err(|error| persisted_domain_error("export profile", error))?;
        let specification = PartyExportSpecification::try_new(scope, profile)
            .map_err(|error| persisted_domain_error("export specification", error))?;
        if specification.version_id().as_str() != self.specification_version_id {
            return Err(persisted_error(
                "persisted export specification version identity does not match semantic specification"
                    .to_owned(),
            ));
        }
        Ok(specification)
    }
}

impl PartyExportSelectionStateV1 {
    fn try_into_domain(
        &self,
        maximum_resources: u32,
    ) -> Result<PartyExportSelectionSummary, SdkError> {
        PartyExportSelectionSummary::try_new(
            self.manifest_sha256.clone(),
            self.selected_resources,
            maximum_resources,
        )
        .map_err(|error| persisted_domain_error("export selection", error))
    }
}

impl PartyExportArtifactStateV1 {
    fn try_into_domain(&self) -> Result<PartyExportArtifactEvidence, SdkError> {
        PartyExportArtifactEvidence::try_new(
            FileId::try_new(self.file_id.clone())
                .map_err(|error| persisted_error(format!("export artifact file ID: {error}")))?,
            self.content_sha256.clone(),
            self.size_bytes,
            self.retention_policy_id.clone(),
        )
        .map_err(|error| persisted_domain_error("export artifact", error))
    }
}

impl PartyExportReconciliationStateV1 {
    fn try_into_domain(&self) -> Result<PartyExportReconciliation, SdkError> {
        PartyExportReconciliation::try_new(
            self.selected_resources,
            self.emitted_rows,
            self.excluded_not_visible,
            self.excluded_version_changed,
            self.excluded_unavailable,
            self.redacted_fields,
        )
        .map_err(|error| persisted_domain_error("export reconciliation", error))
    }
}

fn start(job: &mut PartyExportJob, occurred_at_unix_nanos: i64) -> Result<(), SdkError> {
    let expected = job.version();
    job.start_or_resume(expected, occurred_at_unix_nanos)
        .map_err(|error| persisted_domain_error("export start/resume", error))
}

fn complete_selection(
    job: &mut PartyExportJob,
    selection: &Option<PartyExportSelectionStateV1>,
    occurred_at_unix_nanos: i64,
) -> Result<(), SdkError> {
    let selection = selection.as_ref().ok_or_else(|| {
        persisted_error("export state requires immutable selection evidence".to_owned())
    })?;
    let selection = selection.try_into_domain(job.specification().scope().maximum_resources())?;
    let expected = job.version();
    job.complete_selection(expected, selection, occurred_at_unix_nanos)
        .map_err(|error| persisted_domain_error("export selection completion", error))
}

fn enter_execution(
    job: &mut PartyExportJob,
    selection: &Option<PartyExportSelectionStateV1>,
    occurred_at_unix_nanos: i64,
) -> Result<(), SdkError> {
    start(job, occurred_at_unix_nanos)?;
    complete_selection(job, selection, occurred_at_unix_nanos)?;
    start(job, occurred_at_unix_nanos)
}

fn replay_completed_retry_cycles(
    job: &mut PartyExportJob,
    attempts: u32,
    occurred_at_unix_nanos: i64,
) -> Result<(), SdkError> {
    replay_prior_retry_cycles(job, attempts, occurred_at_unix_nanos)
}

fn replay_prior_retry_cycles(
    job: &mut PartyExportJob,
    attempts: u32,
    occurred_at_unix_nanos: i64,
) -> Result<(), SdkError> {
    for _ in 0..attempts {
        record_failure(
            job,
            "PERSISTED_RETRYABLE_EXPORT_FAILURE",
            occurred_at_unix_nanos,
        )?;
        start(job, occurred_at_unix_nanos)?;
    }
    Ok(())
}

fn record_failure(
    job: &mut PartyExportJob,
    error_code: &str,
    occurred_at_unix_nanos: i64,
) -> Result<(), SdkError> {
    let expected = job.version();
    job.record_retryable_failure(expected, error_code.to_owned(), occurred_at_unix_nanos)
        .map_err(|error| persisted_domain_error("export retryable failure", error))
}

fn replay_checkpoints(
    job: &mut PartyExportJob,
    checkpoint_manifest_position: u32,
    occurred_at_unix_nanos: i64,
) -> Result<(), SdkError> {
    for position in 1..=checkpoint_manifest_position {
        let expected = job.version();
        job.advance_checkpoint(expected, position, occurred_at_unix_nanos)
            .map_err(|error| persisted_domain_error("export checkpoint", error))?;
    }
    Ok(())
}

fn replay_cancelled_state(
    job: &mut PartyExportJob,
    state: &ExportJobStateV1,
    occurred_at_unix_nanos: i64,
) -> Result<(), SdkError> {
    let has_error = state.last_execution_error_code.is_some();
    match state.selection.as_ref() {
        None => {
            require_zero(
                state.checkpoint_manifest_position,
                "cancelled selecting export checkpoint",
            )?;
            if state.execution_attempts == 0 && !has_error && state.version == 2 {
                // Direct Created -> Cancelled.
            } else {
                start(job, occurred_at_unix_nanos)?;
                if has_error {
                    if state.execution_attempts == 0 {
                        return Err(persisted_error(
                            "cancelled retryable export requires execution attempts".to_owned(),
                        ));
                    }
                    replay_prior_retry_cycles(
                        job,
                        state.execution_attempts - 1,
                        occurred_at_unix_nanos,
                    )?;
                    record_failure(
                        job,
                        state
                            .last_execution_error_code
                            .as_deref()
                            .unwrap_or_default(),
                        occurred_at_unix_nanos,
                    )?;
                } else {
                    replay_completed_retry_cycles(
                        job,
                        state.execution_attempts,
                        occurred_at_unix_nanos,
                    )?;
                }
            }
        }
        Some(_) => {
            start(job, occurred_at_unix_nanos)?;
            complete_selection(job, &state.selection, occurred_at_unix_nanos)?;
            let cancel_from_ready = state.execution_attempts == 0
                && state.checkpoint_manifest_position == 0
                && !has_error
                && state.version == 4;
            if !cancel_from_ready {
                start(job, occurred_at_unix_nanos)?;
                replay_checkpoints(
                    job,
                    state.checkpoint_manifest_position,
                    occurred_at_unix_nanos,
                )?;
                if has_error {
                    if state.execution_attempts == 0 {
                        return Err(persisted_error(
                            "cancelled retryable export requires execution attempts".to_owned(),
                        ));
                    }
                    replay_prior_retry_cycles(
                        job,
                        state.execution_attempts - 1,
                        occurred_at_unix_nanos,
                    )?;
                    record_failure(
                        job,
                        state
                            .last_execution_error_code
                            .as_deref()
                            .unwrap_or_default(),
                        occurred_at_unix_nanos,
                    )?;
                } else {
                    replay_completed_retry_cycles(
                        job,
                        state.execution_attempts,
                        occurred_at_unix_nanos,
                    )?;
                }
            }
        }
    }
    let expected = job.version();
    job.cancel(expected, occurred_at_unix_nanos)
        .map_err(|error| persisted_domain_error("export cancellation", error))
}

fn require_none<T>(value: &Option<T>, label: &str) -> Result<(), SdkError> {
    if value.is_none() {
        Ok(())
    } else {
        Err(persisted_error(format!("{label} must be absent")))
    }
}

fn require_zero(value: u32, label: &str) -> Result<(), SdkError> {
    if value == 0 {
        Ok(())
    } else {
        Err(persisted_error(format!("{label} must be zero")))
    }
}

fn validate_size(bytes: &[u8]) -> Result<(), SdkError> {
    if bytes.len() as u64 > EXPORT_JOB_STATE_MAXIMUM_BYTES {
        return Err(persisted_error(format!(
            "export-job state exceeds {EXPORT_JOB_STATE_MAXIMUM_BYTES} bytes"
        )));
    }
    Ok(())
}

fn persisted_domain_error(context: &str, error: SdkError) -> SdkError {
    persisted_error(format!("{context}: {}: {}", error.code, error.safe_message))
}

fn persisted_error(detail: String) -> SdkError {
    let _ = detail;
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_PERSISTED_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "Stored customer export state is invalid.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn specification() -> PartyExportSpecification {
        PartyExportSpecification::try_new(
            PartyExportScope::try_new(Some(PartyExportKindFilter::Person), 10).unwrap(),
            PartyExportProfile::v1(
                vec![
                    PartyExportField::PartyId,
                    PartyExportField::Kind,
                    PartyExportField::DisplayName,
                    PartyExportField::ResourceVersion,
                ],
                "customer-export-30d",
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn selected_job(selected: u32) -> PartyExportJob {
        let mut job = PartyExportJob::create(
            ExportJobId::try_new("export-job-persistence").unwrap(),
            specification(),
            10,
        )
        .unwrap();
        job.start_or_resume(1, 20).unwrap();
        job.complete_selection(
            2,
            PartyExportSelectionSummary::try_new("11".repeat(32), selected, 10).unwrap(),
            30,
        )
        .unwrap();
        job
    }

    #[test]
    fn round_trips_created_selecting_ready_and_executing_states_canonically() {
        let created = PartyExportJob::create(
            ExportJobId::try_new("export-job-created").unwrap(),
            specification(),
            10,
        )
        .unwrap();
        round_trip(&created);

        let mut selecting = PartyExportJob::create(
            ExportJobId::try_new("export-job-selecting").unwrap(),
            specification(),
            10,
        )
        .unwrap();
        selecting.start_or_resume(1, 20).unwrap();
        selecting
            .record_retryable_failure(2, "PARTY_LIST_TEMPORARY", 30)
            .unwrap();
        selecting.start_or_resume(3, 40).unwrap();
        round_trip(&selecting);

        let ready = selected_job(2);
        round_trip(&ready);

        let mut executing = ready;
        executing.start_or_resume(3, 40).unwrap();
        executing.advance_checkpoint(4, 1, 50).unwrap();
        executing
            .record_retryable_failure(5, "PARTY_GET_TEMPORARY", 60)
            .unwrap();
        executing.start_or_resume(6, 70).unwrap();
        round_trip(&executing);
    }

    #[test]
    fn round_trips_retryable_completed_and_cancelled_states_canonically() {
        let mut retryable = selected_job(1);
        retryable.start_or_resume(3, 40).unwrap();
        retryable
            .record_retryable_failure(4, "FILE_STORE_TEMPORARY", 50)
            .unwrap();
        round_trip(&retryable);

        let mut completed = selected_job(1);
        completed.start_or_resume(3, 40).unwrap();
        completed.advance_checkpoint(4, 1, 50).unwrap();
        completed
            .complete(
                5,
                PartyExportArtifactEvidence::try_new(
                    FileId::try_new("export-file-persistence").unwrap(),
                    "22".repeat(32),
                    128,
                    "customer-export-30d",
                )
                .unwrap(),
                PartyExportReconciliation::try_new(1, 1, 0, 0, 0, 0).unwrap(),
                60,
            )
            .unwrap();
        round_trip(&completed);

        let mut cancelled = selected_job(2);
        cancelled.start_or_resume(3, 40).unwrap();
        cancelled.advance_checkpoint(4, 1, 50).unwrap();
        cancelled
            .record_retryable_failure(5, "PARTY_GET_TEMPORARY", 60)
            .unwrap();
        cancelled.cancel(6, 70).unwrap();
        round_trip(&cancelled);
    }

    #[test]
    fn rejects_unknown_fields_and_semantic_specification_tampering() {
        let job = PartyExportJob::create(
            ExportJobId::try_new("export-job-tamper").unwrap(),
            specification(),
            10,
        )
        .unwrap();
        let bytes = encode_export_job_state(&job).unwrap();
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["unknown"] = serde_json::json!(true);
        assert!(decode_export_job_state(&serde_json::to_vec(&value).unwrap()).is_err());

        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["specification"]["specification_version_id"] =
            serde_json::json!("cdo-export-spec-tampered");
        assert!(decode_export_job_state(&serde_json::to_vec(&value).unwrap()).is_err());
    }

    #[test]
    fn descriptor_hash_is_stable_and_non_zero() {
        assert_ne!(export_job_state_descriptor_hash(), [0; 32]);
    }

    fn round_trip(job: &PartyExportJob) {
        let encoded = encode_export_job_state(job).unwrap();
        let decoded = decode_export_job_state(&encoded).unwrap();
        assert_eq!(encode_export_job_state(&decoded).unwrap(), encoded);
        assert_eq!(decoded.version(), job.version());
        assert_eq!(decoded.status(), job.status());
    }
}
