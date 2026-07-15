use crate::ExportJobId;
use crm_module_sdk::{ErrorCategory, RecordId, SdkError};
use sha2::{Digest, Sha256};

const SELECTION_PROGRESS_ID_DOMAIN: &[u8] =
    b"crm.customer-data-operations.party-export-selection-progress/v1";
const MAXIMUM_SOURCE_SORT_VALUE_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PartyExportSelectionProgressId(RecordId);

impl PartyExportSelectionProgressId {
    pub fn for_job(job_id: &ExportJobId) -> Result<Self, SdkError> {
        let mut hasher = Sha256::new();
        hasher.update(SELECTION_PROGRESS_ID_DOMAIN);
        hash_part(&mut hasher, job_id.as_str().as_bytes());
        RecordId::try_new(format!(
            "cdo-export-selection-progress-{}",
            hex_digest(hasher.finalize())
        ))
        .map(Self)
        .map_err(|error| {
            invalid(
                "CUSTOMER_DATA_EXPORT_SELECTION_PROGRESS_ID_INVALID",
                "customer_data.export.selection_progress_id",
                error.to_string(),
            )
        })
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyExportSourceContinuation {
    sort_value: String,
    record_id: RecordId,
}

impl PartyExportSourceContinuation {
    pub fn try_new(sort_value: impl Into<String>, record_id: RecordId) -> Result<Self, SdkError> {
        let sort_value = sort_value.into();
        if sort_value.is_empty()
            || sort_value.len() > MAXIMUM_SOURCE_SORT_VALUE_BYTES
            || sort_value.chars().any(char::is_control)
        {
            return Err(invalid(
                "CUSTOMER_DATA_EXPORT_SELECTION_CONTINUATION_INVALID",
                "customer_data.export.selection_progress.source_after_sort_value",
                "selection continuation sort value is invalid",
            ));
        }
        Ok(Self {
            sort_value,
            record_id,
        })
    }

    pub fn sort_value(&self) -> &str {
        &self.sort_value
    }

    pub fn record_id(&self) -> &RecordId {
        &self.record_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyExportSelectionProgress {
    progress_id: PartyExportSelectionProgressId,
    job_id: ExportJobId,
    next_manifest_position: u32,
    continuation: Option<PartyExportSourceContinuation>,
    source_exhausted: bool,
    created_at_unix_nanos: i64,
    updated_at_unix_nanos: i64,
    version: i64,
}

impl PartyExportSelectionProgress {
    pub fn create(job_id: ExportJobId, occurred_at_unix_nanos: i64) -> Result<Self, SdkError> {
        validate_time(occurred_at_unix_nanos)?;
        Ok(Self {
            progress_id: PartyExportSelectionProgressId::for_job(&job_id)?,
            job_id,
            next_manifest_position: 1,
            continuation: None,
            source_exhausted: false,
            created_at_unix_nanos: occurred_at_unix_nanos,
            updated_at_unix_nanos: occurred_at_unix_nanos,
            version: 1,
        })
    }

    pub fn advance(
        &mut self,
        expected_version: i64,
        committed_items: u32,
        continuation: Option<PartyExportSourceContinuation>,
        occurred_at_unix_nanos: i64,
    ) -> Result<(), SdkError> {
        self.require_version(expected_version)?;
        if self.source_exhausted {
            return Err(conflict(
                "CUSTOMER_DATA_EXPORT_SELECTION_PROGRESS_FINAL",
                "A completed export selection scan cannot advance.",
            ));
        }
        validate_time(occurred_at_unix_nanos)?;
        if occurred_at_unix_nanos < self.updated_at_unix_nanos {
            return Err(invalid(
                "CUSTOMER_DATA_EXPORT_SELECTION_PROGRESS_TIME_REGRESSION",
                "customer_data.export.selection_progress.updated_at",
                "selection progress time cannot move backwards",
            ));
        }
        if committed_items == 0
            && continuation.is_some()
            && continuation == self.continuation
        {
            return Err(conflict(
                "CUSTOMER_DATA_EXPORT_SELECTION_PROGRESS_NOOP",
                "Selection progress must commit items, advance the source cursor or finish the source scan.",
            ));
        }

        self.next_manifest_position = self
            .next_manifest_position
            .checked_add(committed_items)
            .ok_or_else(|| {
                conflict(
                    "CUSTOMER_DATA_EXPORT_SELECTION_POSITION_EXHAUSTED",
                    "The export selection cannot advance another manifest position.",
                )
            })?;
        self.source_exhausted = continuation.is_none();
        self.continuation = continuation;
        self.version = self.version.checked_add(1).ok_or_else(|| {
            conflict(
                "CUSTOMER_DATA_EXPORT_SELECTION_PROGRESS_VERSION_EXHAUSTED",
                "The export selection progress cannot advance another version.",
            )
        })?;
        self.updated_at_unix_nanos = occurred_at_unix_nanos;
        Ok(())
    }

    pub fn progress_id(&self) -> &PartyExportSelectionProgressId {
        &self.progress_id
    }

    pub fn job_id(&self) -> &ExportJobId {
        &self.job_id
    }

    pub const fn next_manifest_position(&self) -> u32 {
        self.next_manifest_position
    }

    pub fn continuation(&self) -> Option<&PartyExportSourceContinuation> {
        self.continuation.as_ref()
    }

    pub const fn source_exhausted(&self) -> bool {
        self.source_exhausted
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
        if self.version == expected_version {
            Ok(())
        } else {
            Err(conflict(
                "CUSTOMER_DATA_EXPORT_SELECTION_PROGRESS_VERSION_CONFLICT",
                "The export selection progress version is stale.",
            ))
        }
    }
}

fn validate_time(value: i64) -> Result<(), SdkError> {
    if value <= 0 {
        Err(invalid(
            "CUSTOMER_DATA_EXPORT_SELECTION_PROGRESS_TIME_INVALID",
            "customer_data.export.selection_progress.time",
            "selection progress time must be positive Unix nanoseconds",
        ))
    } else {
        Ok(())
    }
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

fn conflict(code: &'static str, safe_message: &'static str) -> SdkError {
    SdkError::new(code, ErrorCategory::Conflict, false, safe_message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn continuation(sort: &str, record_id: &str) -> PartyExportSourceContinuation {
        PartyExportSourceContinuation::try_new(sort, RecordId::try_new(record_id).unwrap()).unwrap()
    }

    #[test]
    fn progress_identity_is_deterministic_per_job() {
        let job_id = ExportJobId::try_new("selection-progress-job").unwrap();
        let first = PartyExportSelectionProgress::create(job_id.clone(), 10).unwrap();
        let replay = PartyExportSelectionProgress::create(job_id, 10).unwrap();
        assert_eq!(first.progress_id(), replay.progress_id());
    }

    #[test]
    fn advances_manifest_position_and_opaque_source_continuation() {
        let mut progress = PartyExportSelectionProgress::create(
            ExportJobId::try_new("selection-progress-advance").unwrap(),
            10,
        )
        .unwrap();
        progress
            .advance(
                1,
                3,
                Some(continuation("2026-07-15T00:00:00Z", "party-3")),
                20,
            )
            .unwrap();
        assert_eq!(progress.next_manifest_position(), 4);
        assert_eq!(progress.version(), 2);
        assert!(!progress.source_exhausted());
    }

    #[test]
    fn zero_item_page_can_advance_cursor_and_final_page_can_finish() {
        let mut progress = PartyExportSelectionProgress::create(
            ExportJobId::try_new("selection-progress-empty-pages").unwrap(),
            10,
        )
        .unwrap();
        progress
            .advance(
                1,
                0,
                Some(continuation(
                    "2026-07-15T00:00:00Z",
                    "party-hidden",
                )),
                20,
            )
            .unwrap();
        progress.advance(2, 0, None, 30).unwrap();
        assert!(progress.source_exhausted());
        assert_eq!(progress.next_manifest_position(), 1);
    }

    #[test]
    fn rejects_stale_repeated_cursor_and_advance_after_source_exhaustion() {
        let mut progress = PartyExportSelectionProgress::create(
            ExportJobId::try_new("selection-progress-conflict").unwrap(),
            10,
        )
        .unwrap();
        assert!(progress.advance(2, 1, None, 20).is_err());
        let next = continuation("2026-07-15T00:00:00Z", "party-hidden");
        progress.advance(1, 0, Some(next.clone()), 20).unwrap();
        assert!(progress.advance(2, 0, Some(next), 30).is_err());
        progress.advance(2, 1, None, 30).unwrap();
        assert!(progress.advance(3, 1, None, 40).is_err());
    }
}
