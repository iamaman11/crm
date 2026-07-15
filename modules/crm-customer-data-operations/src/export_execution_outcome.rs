//! Durable per-manifest execution outcomes for Party export.
//!
//! An execution checkpoint is safe only when every manifest position at or below it already has one
//! durable outcome. Emitted outcomes additionally bind the exact deterministic artifact chunk index,
//! SHA-256 and byte size that must have been durably accepted by the file-artifact boundary before
//! the outcome/checkpoint transaction may commit.

use crate::{ExportJobId, PartyExportReconciliation};
use crm_module_sdk::{ErrorCategory, RecordId, SdkError};
use sha2::{Digest, Sha256};

const EXECUTION_OUTCOME_ID_DOMAIN: &[u8] =
    b"crm.customer-data-operations.party-export-execution-outcome/v1";
const MAX_EXECUTION_OUTCOMES: usize = 100_000;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PartyExportExecutionOutcomeId(RecordId);

impl PartyExportExecutionOutcomeId {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartyExportExclusionReason {
    NotVisible,
    VersionChanged,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartyExportExecutionOutcomeKind {
    Emitted {
        artifact_chunk_index: u32,
        chunk_sha256: String,
        chunk_size_bytes: u64,
    },
    Excluded(PartyExportExclusionReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyExportExecutionOutcome {
    outcome_id: PartyExportExecutionOutcomeId,
    job_id: ExportJobId,
    manifest_position: u32,
    kind: PartyExportExecutionOutcomeKind,
    redacted_fields: u32,
    occurred_at_unix_nanos: i64,
}

impl PartyExportExecutionOutcome {
    pub fn emitted(
        job_id: ExportJobId,
        manifest_position: u32,
        artifact_chunk_index: u32,
        chunk_sha256: impl Into<String>,
        chunk_size_bytes: u64,
        redacted_fields: u32,
        occurred_at_unix_nanos: i64,
    ) -> Result<Self, SdkError> {
        if artifact_chunk_index == 0 {
            return Err(invalid(
                "CUSTOMER_DATA_EXPORT_ARTIFACT_CHUNK_INDEX_INVALID",
                "customer_data.export.execution.artifact_chunk_index",
                "artifact chunk index 0 is reserved for the canonical CSV header",
            ));
        }
        if chunk_size_bytes == 0 {
            return Err(invalid(
                "CUSTOMER_DATA_EXPORT_ARTIFACT_CHUNK_SIZE_INVALID",
                "customer_data.export.execution.chunk_size_bytes",
                "an emitted CSV row chunk must contain at least one byte",
            ));
        }
        Self::create(
            job_id,
            manifest_position,
            PartyExportExecutionOutcomeKind::Emitted {
                artifact_chunk_index,
                chunk_sha256: normalize_sha256(chunk_sha256.into())?,
                chunk_size_bytes,
            },
            redacted_fields,
            occurred_at_unix_nanos,
        )
    }

    pub fn excluded(
        job_id: ExportJobId,
        manifest_position: u32,
        reason: PartyExportExclusionReason,
        occurred_at_unix_nanos: i64,
    ) -> Result<Self, SdkError> {
        Self::create(
            job_id,
            manifest_position,
            PartyExportExecutionOutcomeKind::Excluded(reason),
            0,
            occurred_at_unix_nanos,
        )
    }

    fn create(
        job_id: ExportJobId,
        manifest_position: u32,
        kind: PartyExportExecutionOutcomeKind,
        redacted_fields: u32,
        occurred_at_unix_nanos: i64,
    ) -> Result<Self, SdkError> {
        validate_manifest_position(manifest_position)?;
        if occurred_at_unix_nanos <= 0 {
            return Err(invalid(
                "CUSTOMER_DATA_EXPORT_EXECUTION_OUTCOME_TIME_INVALID",
                "customer_data.export.execution.occurred_at_unix_nanos",
                "execution outcome time must be positive Unix nanoseconds",
            ));
        }

        let mut hasher = Sha256::new();
        hasher.update(EXECUTION_OUTCOME_ID_DOMAIN);
        hash_part(&mut hasher, job_id.as_str().as_bytes());
        hash_part(&mut hasher, &manifest_position.to_be_bytes());
        let outcome_id = RecordId::try_new(format!(
            "cdo-export-outcome-{}",
            hex_digest(hasher.finalize())
        ))
        .map(PartyExportExecutionOutcomeId)
        .map_err(|error| {
            invalid(
                "CUSTOMER_DATA_EXPORT_EXECUTION_OUTCOME_ID_INVALID",
                "customer_data.export.execution.outcome_id",
                error.to_string(),
            )
        })?;

        Ok(Self {
            outcome_id,
            job_id,
            manifest_position,
            kind,
            redacted_fields,
            occurred_at_unix_nanos,
        })
    }

    pub fn outcome_id(&self) -> &PartyExportExecutionOutcomeId {
        &self.outcome_id
    }

    pub fn job_id(&self) -> &ExportJobId {
        &self.job_id
    }

    pub const fn manifest_position(&self) -> u32 {
        self.manifest_position
    }

    pub fn kind(&self) -> &PartyExportExecutionOutcomeKind {
        &self.kind
    }

    pub const fn redacted_fields(&self) -> u32 {
        self.redacted_fields
    }

    pub const fn occurred_at_unix_nanos(&self) -> i64 {
        self.occurred_at_unix_nanos
    }
}

/// Verifies a contiguous durable prefix and returns the checkpoint position it safely proves.
pub fn durable_party_export_checkpoint(
    job_id: &ExportJobId,
    outcomes: &[PartyExportExecutionOutcome],
) -> Result<u32, SdkError> {
    validate_durable_outcome_prefix(job_id, outcomes)?;
    u32::try_from(outcomes.len()).map_err(|_| execution_state_error())
}

/// Derives final reconciliation only from a complete contiguous durable outcome set.
pub fn reconcile_durable_party_export_outcomes(
    job_id: &ExportJobId,
    selected_resources: u32,
    outcomes: &[PartyExportExecutionOutcome],
) -> Result<PartyExportReconciliation, SdkError> {
    if usize::try_from(selected_resources).map_err(|_| execution_state_error())? != outcomes.len() {
        return Err(execution_state_error());
    }
    validate_durable_outcome_prefix(job_id, outcomes)?;

    let mut emitted_rows = 0_u32;
    let mut excluded_not_visible = 0_u32;
    let mut excluded_version_changed = 0_u32;
    let mut excluded_unavailable = 0_u32;
    let mut redacted_fields = 0_u32;
    let mut expected_artifact_chunk_index = 1_u32;

    for outcome in outcomes {
        redacted_fields = redacted_fields
            .checked_add(outcome.redacted_fields())
            .ok_or_else(execution_state_error)?;
        match outcome.kind() {
            PartyExportExecutionOutcomeKind::Emitted {
                artifact_chunk_index,
                ..
            } => {
                if *artifact_chunk_index != expected_artifact_chunk_index {
                    return Err(execution_state_error());
                }
                expected_artifact_chunk_index = expected_artifact_chunk_index
                    .checked_add(1)
                    .ok_or_else(execution_state_error)?;
                emitted_rows = emitted_rows
                    .checked_add(1)
                    .ok_or_else(execution_state_error)?;
            }
            PartyExportExecutionOutcomeKind::Excluded(PartyExportExclusionReason::NotVisible) => {
                excluded_not_visible = excluded_not_visible
                    .checked_add(1)
                    .ok_or_else(execution_state_error)?;
            }
            PartyExportExecutionOutcomeKind::Excluded(
                PartyExportExclusionReason::VersionChanged,
            ) => {
                excluded_version_changed = excluded_version_changed
                    .checked_add(1)
                    .ok_or_else(execution_state_error)?;
            }
            PartyExportExecutionOutcomeKind::Excluded(PartyExportExclusionReason::Unavailable) => {
                excluded_unavailable = excluded_unavailable
                    .checked_add(1)
                    .ok_or_else(execution_state_error)?;
            }
        }
    }

    PartyExportReconciliation::try_new(
        selected_resources,
        emitted_rows,
        excluded_not_visible,
        excluded_version_changed,
        excluded_unavailable,
        redacted_fields,
    )
}

fn validate_durable_outcome_prefix(
    job_id: &ExportJobId,
    outcomes: &[PartyExportExecutionOutcome],
) -> Result<(), SdkError> {
    if outcomes.len() > MAX_EXECUTION_OUTCOMES {
        return Err(execution_state_error());
    }
    for (index, outcome) in outcomes.iter().enumerate() {
        let expected_position = u32::try_from(index + 1).map_err(|_| execution_state_error())?;
        if outcome.job_id() != job_id || outcome.manifest_position() != expected_position {
            return Err(execution_state_error());
        }
        let expected_id = expected_outcome_id(job_id, expected_position)?;
        if outcome.outcome_id() != &expected_id {
            return Err(execution_state_error());
        }
    }
    Ok(())
}

fn expected_outcome_id(
    job_id: &ExportJobId,
    manifest_position: u32,
) -> Result<PartyExportExecutionOutcomeId, SdkError> {
    validate_manifest_position(manifest_position)?;
    let mut hasher = Sha256::new();
    hasher.update(EXECUTION_OUTCOME_ID_DOMAIN);
    hash_part(&mut hasher, job_id.as_str().as_bytes());
    hash_part(&mut hasher, &manifest_position.to_be_bytes());
    RecordId::try_new(format!(
        "cdo-export-outcome-{}",
        hex_digest(hasher.finalize())
    ))
    .map(PartyExportExecutionOutcomeId)
    .map_err(|error| {
        invalid(
            "CUSTOMER_DATA_EXPORT_EXECUTION_OUTCOME_ID_INVALID",
            "customer_data.export.execution.outcome_id",
            error.to_string(),
        )
    })
}

fn validate_manifest_position(value: u32) -> Result<(), SdkError> {
    if value == 0 || value as usize > MAX_EXECUTION_OUTCOMES {
        return Err(invalid(
            "CUSTOMER_DATA_EXPORT_EXECUTION_POSITION_INVALID",
            "customer_data.export.execution.manifest_position",
            format!("manifest position must be between 1 and {MAX_EXECUTION_OUTCOMES}"),
        ));
    }
    Ok(())
}

fn normalize_sha256(value: String) -> Result<String, SdkError> {
    let value = value.to_ascii_lowercase();
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(invalid(
            "CUSTOMER_DATA_EXPORT_ARTIFACT_CHUNK_SHA256_INVALID",
            "customer_data.export.execution.chunk_sha256",
            "artifact chunk SHA-256 must be exactly 64 hexadecimal characters",
        ));
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

fn invalid(code: &'static str, field: &'static str, message: impl Into<String>) -> SdkError {
    let mut error = SdkError::invalid_argument(field, message.into());
    error.code = code.to_owned();
    error
}

fn execution_state_error() -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_EXECUTION_OUTCOMES_INCONSISTENT",
        ErrorCategory::Conflict,
        false,
        "The durable Party export execution outcomes are inconsistent.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job_id() -> ExportJobId {
        ExportJobId::try_new("export-execution-outcome-job").unwrap()
    }

    fn emitted(
        job_id: &ExportJobId,
        position: u32,
        chunk_index: u32,
    ) -> PartyExportExecutionOutcome {
        PartyExportExecutionOutcome::emitted(
            job_id.clone(),
            position,
            chunk_index,
            "11".repeat(32),
            12,
            1,
            100,
        )
        .unwrap()
    }

    #[test]
    fn outcome_identity_is_deterministic_for_job_and_manifest_position() {
        let job_id = job_id();
        let first = emitted(&job_id, 1, 1);
        let replay = emitted(&job_id, 1, 1);
        assert_eq!(first.outcome_id(), replay.outcome_id());
    }

    #[test]
    fn contiguous_durable_outcomes_define_the_only_safe_checkpoint() {
        let job_id = job_id();
        let outcomes = vec![
            emitted(&job_id, 1, 1),
            PartyExportExecutionOutcome::excluded(
                job_id.clone(),
                2,
                PartyExportExclusionReason::NotVisible,
                101,
            )
            .unwrap(),
        ];
        assert_eq!(
            durable_party_export_checkpoint(&job_id, &outcomes).unwrap(),
            2
        );
    }

    #[test]
    fn checkpoint_rejects_gaps_and_cross_job_outcomes() {
        let job_id = job_id();
        let other_job = ExportJobId::try_new("export-execution-other-job").unwrap();
        assert!(durable_party_export_checkpoint(&job_id, &[emitted(&job_id, 2, 1)]).is_err());
        assert!(durable_party_export_checkpoint(&job_id, &[emitted(&other_job, 1, 1)]).is_err());
    }

    #[test]
    fn reconciliation_is_derived_from_complete_durable_outcomes() {
        let job_id = job_id();
        let outcomes = vec![
            emitted(&job_id, 1, 1),
            PartyExportExecutionOutcome::excluded(
                job_id.clone(),
                2,
                PartyExportExclusionReason::VersionChanged,
                101,
            )
            .unwrap(),
            emitted(&job_id, 3, 2),
            PartyExportExecutionOutcome::excluded(
                job_id.clone(),
                4,
                PartyExportExclusionReason::Unavailable,
                102,
            )
            .unwrap(),
        ];
        let reconciliation =
            reconcile_durable_party_export_outcomes(&job_id, 4, &outcomes).unwrap();
        assert_eq!(reconciliation.selected_resources(), 4);
        assert_eq!(reconciliation.emitted_rows(), 2);
        assert_eq!(reconciliation.excluded_version_changed(), 1);
        assert_eq!(reconciliation.excluded_unavailable(), 1);
        assert_eq!(reconciliation.redacted_fields(), 2);
    }

    #[test]
    fn reconciliation_rejects_non_sequential_artifact_chunk_indexes() {
        let job_id = job_id();
        let outcomes = vec![emitted(&job_id, 1, 2)];
        assert!(reconcile_durable_party_export_outcomes(&job_id, 1, &outcomes).is_err());
    }

    #[test]
    fn emitted_outcome_rejects_header_chunk_zero_and_invalid_digest() {
        let job_id = job_id();
        assert!(
            PartyExportExecutionOutcome::emitted(job_id.clone(), 1, 0, "11".repeat(32), 1, 0, 1,)
                .is_err()
        );
        assert!(
            PartyExportExecutionOutcome::emitted(job_id, 1, 1, "not-a-digest", 1, 0, 1).is_err()
        );
    }
}
