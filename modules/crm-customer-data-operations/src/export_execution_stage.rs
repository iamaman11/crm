use crate::{ExportJobId, PartyExportExclusionReason};
use crm_module_sdk::{ErrorCategory, RecordId, SdkError};
use sha2::{Digest, Sha256};

pub const MAXIMUM_PARTY_EXPORT_ROW_BYTES: usize = 512 * 1024;
const EXECUTION_STAGE_ID_DOMAIN: &[u8] =
    b"crm.customer-data-operations.party-export-execution-stage/v1";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PartyExportExecutionStageId(RecordId);

impl PartyExportExecutionStageId {
    pub fn for_job_position(
        job_id: &ExportJobId,
        manifest_position: u32,
    ) -> Result<Self, SdkError> {
        if manifest_position == 0 {
            return Err(invalid_position());
        }
        let mut hasher = Sha256::new();
        hasher.update(EXECUTION_STAGE_ID_DOMAIN);
        hash_part(&mut hasher, job_id.as_str().as_bytes());
        hash_part(&mut hasher, &manifest_position.to_be_bytes());
        RecordId::try_new(format!(
            "cdo-export-stage-{}",
            hex_digest(hasher.finalize())
        ))
        .map(Self)
        .map_err(|error| invalid("CUSTOMER_DATA_EXPORT_EXECUTION_STAGE_ID_INVALID", error.to_string()))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartyExportExecutionStageKind {
    Emitted {
        row_utf8: String,
        row_sha256: String,
        redacted_fields: u32,
    },
    Excluded(PartyExportExclusionReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyExportExecutionStage {
    stage_id: PartyExportExecutionStageId,
    job_id: ExportJobId,
    manifest_position: u32,
    kind: PartyExportExecutionStageKind,
    occurred_at_unix_nanos: i64,
}

impl PartyExportExecutionStage {
    pub fn emitted(
        job_id: ExportJobId,
        manifest_position: u32,
        row_utf8: String,
        redacted_fields: u32,
        occurred_at_unix_nanos: i64,
    ) -> Result<Self, SdkError> {
        if row_utf8.is_empty() || row_utf8.len() > MAXIMUM_PARTY_EXPORT_ROW_BYTES {
            return Err(SdkError::invalid_argument(
                "customer_data.export.execution_stage.row_utf8",
                format!(
                    "staged export row must contain between 1 and {MAXIMUM_PARTY_EXPORT_ROW_BYTES} UTF-8 bytes"
                ),
            ));
        }
        if !row_utf8.ends_with('\n') {
            return Err(SdkError::invalid_argument(
                "customer_data.export.execution_stage.row_utf8",
                "staged export row must end with the canonical LF newline",
            ));
        }
        let row_sha256 = hex_digest(Sha256::digest(row_utf8.as_bytes()));
        Self::create(
            job_id,
            manifest_position,
            PartyExportExecutionStageKind::Emitted {
                row_utf8,
                row_sha256,
                redacted_fields,
            },
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
            PartyExportExecutionStageKind::Excluded(reason),
            occurred_at_unix_nanos,
        )
    }

    fn create(
        job_id: ExportJobId,
        manifest_position: u32,
        kind: PartyExportExecutionStageKind,
        occurred_at_unix_nanos: i64,
    ) -> Result<Self, SdkError> {
        if occurred_at_unix_nanos <= 0 {
            return Err(SdkError::invalid_argument(
                "customer_data.export.execution_stage.occurred_at_unix_nanos",
                "execution-stage time must be positive Unix nanoseconds",
            ));
        }
        Ok(Self {
            stage_id: PartyExportExecutionStageId::for_job_position(&job_id, manifest_position)?,
            job_id,
            manifest_position,
            kind,
            occurred_at_unix_nanos,
        })
    }

    pub fn stage_id(&self) -> &PartyExportExecutionStageId {
        &self.stage_id
    }

    pub fn job_id(&self) -> &ExportJobId {
        &self.job_id
    }

    pub const fn manifest_position(&self) -> u32 {
        self.manifest_position
    }

    pub fn kind(&self) -> &PartyExportExecutionStageKind {
        &self.kind
    }

    pub const fn occurred_at_unix_nanos(&self) -> i64 {
        self.occurred_at_unix_nanos
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

fn invalid(code: &'static str, detail: String) -> SdkError {
    SdkError::new(
        code,
        ErrorCategory::InvalidArgument,
        false,
        "The customer export execution stage is invalid.",
    )
    .with_internal_reference(detail)
}

fn invalid_position() -> SdkError {
    SdkError::invalid_argument(
        "customer_data.export.execution_stage.manifest_position",
        "manifest position must be positive",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emitted_stage_freezes_exact_canonical_row_bytes() {
        let job_id = ExportJobId::try_new("stage-job").unwrap();
        let stage = PartyExportExecutionStage::emitted(
            job_id.clone(),
            1,
            "party-1,person,Alice\n".to_owned(),
            1,
            100,
        )
        .unwrap();
        let replay = PartyExportExecutionStage::emitted(
            job_id,
            1,
            "party-1,person,Alice\n".to_owned(),
            1,
            101,
        )
        .unwrap();
        assert_eq!(stage.stage_id(), replay.stage_id());
        assert!(matches!(stage.kind(), PartyExportExecutionStageKind::Emitted { .. }));
    }

    #[test]
    fn rejects_noncanonical_or_oversized_rows() {
        let job_id = ExportJobId::try_new("stage-row-validation-job").unwrap();
        assert!(
            PartyExportExecutionStage::emitted(job_id.clone(), 1, "row".to_owned(), 0, 100)
                .is_err()
        );
        assert!(
            PartyExportExecutionStage::emitted(
                job_id,
                1,
                format!("{}\n", "x".repeat(MAXIMUM_PARTY_EXPORT_ROW_BYTES)),
                0,
                100,
            )
            .is_err()
        );
    }
}
