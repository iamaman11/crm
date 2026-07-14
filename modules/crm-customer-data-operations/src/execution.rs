use crate::{ImportRowId, ImportRowStatus};
use crm_module_sdk::{ErrorCategory, SdkError};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionRowReference {
    row_id: ImportRowId,
    row_position: u32,
    status: ImportRowStatus,
}

impl ExecutionRowReference {
    pub fn new(row_id: ImportRowId, row_position: u32, status: ImportRowStatus) -> Self {
        Self {
            row_id,
            row_position,
            status,
        }
    }

    pub fn row_id(&self) -> &ImportRowId {
        &self.row_id
    }

    pub const fn row_position(&self) -> u32 {
        self.row_position
    }

    pub const fn status(&self) -> ImportRowStatus {
        self.status
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionPositionIndex {
    total_rows: u32,
    rows: BTreeMap<u32, ExecutionRowReference>,
}

impl ExecutionPositionIndex {
    pub fn build(
        total_rows: u32,
        rows: impl IntoIterator<Item = ExecutionRowReference>,
    ) -> Result<Self, SdkError> {
        if total_rows == 0 {
            return Err(index_error(
                "CUSTOMER_DATA_IMPORT_EXECUTION_TOTAL_ROWS_INVALID",
                "execution requires a positive immutable source row count",
            ));
        }

        let mut indexed = BTreeMap::new();
        for row in rows {
            if row.row_position == 0 || row.row_position > total_rows {
                return Err(index_error(
                    "CUSTOMER_DATA_IMPORT_EXECUTION_ROW_POSITION_INVALID",
                    "execution row position is outside the immutable source row range",
                ));
            }
            if indexed.insert(row.row_position, row).is_some() {
                return Err(index_error(
                    "CUSTOMER_DATA_IMPORT_EXECUTION_ROW_POSITION_DUPLICATE",
                    "execution row positions must be unique",
                ));
            }
        }

        let indexed_count = u32::try_from(indexed.len()).map_err(|_| {
            index_error(
                "CUSTOMER_DATA_IMPORT_EXECUTION_ROW_COUNT_INVALID",
                "execution row count does not fit the supported range",
            )
        })?;
        if indexed_count != total_rows {
            return Err(index_error(
                "CUSTOMER_DATA_IMPORT_EXECUTION_ROW_SET_INCOMPLETE",
                "execution requires one authoritative row outcome for every immutable source row",
            ));
        }

        for expected_position in 1..=total_rows {
            if !indexed.contains_key(&expected_position) {
                return Err(index_error(
                    "CUSTOMER_DATA_IMPORT_EXECUTION_ROW_SET_INCOMPLETE",
                    "execution row positions must form one complete contiguous source range",
                ));
            }
        }

        Ok(Self {
            total_rows,
            rows: indexed,
        })
    }

    pub const fn total_rows(&self) -> u32 {
        self.total_rows
    }

    pub fn row(&self, row_position: u32) -> Option<&ExecutionRowReference> {
        self.rows.get(&row_position)
    }

    pub fn next_after_checkpoint(
        &self,
        checkpoint_row_position: u32,
    ) -> Result<Option<&ExecutionRowReference>, SdkError> {
        if checkpoint_row_position > self.total_rows {
            return Err(index_error(
                "CUSTOMER_DATA_IMPORT_EXECUTION_CHECKPOINT_INVALID",
                "execution checkpoint exceeds the immutable source row count",
            ));
        }
        if checkpoint_row_position == self.total_rows {
            return Ok(None);
        }
        let next_position = checkpoint_row_position.checked_add(1).ok_or_else(|| {
            index_error(
                "CUSTOMER_DATA_IMPORT_EXECUTION_CHECKPOINT_INVALID",
                "execution checkpoint cannot advance safely",
            )
        })?;
        self.rows.get(&next_position).map(Some).ok_or_else(|| {
            index_error(
                "CUSTOMER_DATA_IMPORT_EXECUTION_ROW_SET_INCOMPLETE",
                "the next execution row is missing from the authoritative position index",
            )
        })
    }
}

fn index_error(code: &'static str, internal: &'static str) -> SdkError {
    SdkError::new(
        code,
        ErrorCategory::Internal,
        false,
        "Customer-data import execution state is inconsistent.",
    )
    .with_internal_reference(internal)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(position: u32, status: ImportRowStatus) -> ExecutionRowReference {
        ExecutionRowReference::new(
            ImportRowId::try_new(format!("import-row-{position}")).unwrap(),
            position,
            status,
        )
    }

    #[test]
    fn relationship_query_order_does_not_affect_execution_order() {
        let index = ExecutionPositionIndex::build(
            3,
            [
                row(3, ImportRowStatus::Valid),
                row(1, ImportRowStatus::Invalid),
                row(2, ImportRowStatus::Valid),
            ],
        )
        .unwrap();

        assert_eq!(
            index
                .next_after_checkpoint(0)
                .unwrap()
                .unwrap()
                .row_position(),
            1
        );
        assert_eq!(
            index
                .next_after_checkpoint(1)
                .unwrap()
                .unwrap()
                .row_position(),
            2
        );
        assert_eq!(
            index
                .next_after_checkpoint(2)
                .unwrap()
                .unwrap()
                .row_position(),
            3
        );
        assert!(index.next_after_checkpoint(3).unwrap().is_none());
    }

    #[test]
    fn duplicate_positions_fail_before_target_execution() {
        let error = ExecutionPositionIndex::build(
            2,
            [
                row(1, ImportRowStatus::Valid),
                row(1, ImportRowStatus::Invalid),
            ],
        )
        .unwrap_err();
        assert_eq!(
            error.code,
            "CUSTOMER_DATA_IMPORT_EXECUTION_ROW_POSITION_DUPLICATE"
        );
    }

    #[test]
    fn missing_positions_fail_before_target_execution() {
        let error = ExecutionPositionIndex::build(
            3,
            [
                row(1, ImportRowStatus::Valid),
                row(3, ImportRowStatus::Valid),
            ],
        )
        .unwrap_err();
        assert_eq!(
            error.code,
            "CUSTOMER_DATA_IMPORT_EXECUTION_ROW_SET_INCOMPLETE"
        );
    }

    #[test]
    fn checkpoint_must_stay_inside_source_range() {
        let index = ExecutionPositionIndex::build(1, [row(1, ImportRowStatus::Valid)]).unwrap();
        let error = index.next_after_checkpoint(2).unwrap_err();
        assert_eq!(
            error.code,
            "CUSTOMER_DATA_IMPORT_EXECUTION_CHECKPOINT_INVALID"
        );
    }
}
