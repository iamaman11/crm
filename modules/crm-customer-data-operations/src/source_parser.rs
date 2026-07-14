use crate::ImportParserProfile;
use crm_module_sdk::{ErrorCategory, FieldName, FieldViolation, SdkError};
use std::collections::{BTreeMap, BTreeSet};

pub const MAXIMUM_IMPORT_SOURCE_ROWS: usize = 100_000;
pub const MAXIMUM_IMPORT_SOURCE_COLUMNS: usize = 256;
pub const MAXIMUM_IMPORT_SOURCE_COLUMN_NAME_BYTES: usize = 160;
pub const MAXIMUM_IMPORT_SOURCE_CELL_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedImportSourceRow {
    pub row_position: u32,
    pub columns: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedImportSource {
    headers: Vec<String>,
    rows: Vec<ParsedImportSourceRow>,
}

impl ParsedImportSource {
    pub fn headers(&self) -> &[String] {
        &self.headers
    }

    pub fn rows(&self) -> &[ParsedImportSourceRow] {
        &self.rows
    }

    pub fn row_count(&self) -> u32 {
        u32::try_from(self.rows.len()).expect("bounded import row count must fit u32")
    }

    pub fn rows_inclusive_from(
        &self,
        start_row_position: u32,
        maximum_rows: usize,
    ) -> Result<&[ParsedImportSourceRow], SdkError> {
        if start_row_position == 0 || maximum_rows == 0 {
            return Err(SdkError::invalid_argument(
                "customer_data.import.source_batch",
                "Source validation batch position and size must be positive",
            ));
        }
        let start = usize::try_from(start_row_position - 1).map_err(|_| parser_error(
            "CUSTOMER_DATA_IMPORT_SOURCE_BATCH_POSITION_INVALID",
            "customer_data.import.source_batch.start_row_position",
            "Source validation batch position is invalid.",
        ))?;
        if start > self.rows.len() {
            return Err(SdkError::invalid_argument(
                "customer_data.import.source_batch.start_row_position",
                "Source validation batch starts beyond the immutable source row range",
            ));
        }
        let end = start.saturating_add(maximum_rows).min(self.rows.len());
        Ok(&self.rows[start..end])
    }
}

pub fn parse_import_source(
    bytes: &[u8],
    profile: &ImportParserProfile,
) -> Result<ParsedImportSource, SdkError> {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .delimiter(profile.delimiter())
        .quote(profile.quote())
        .double_quote(true)
        .from_reader(bytes);

    let raw_headers = reader
        .headers()
        .map_err(|error| csv_error("CUSTOMER_DATA_IMPORT_SOURCE_HEADER_INVALID", error))?
        .clone();
    if raw_headers.is_empty() || raw_headers.len() > MAXIMUM_IMPORT_SOURCE_COLUMNS {
        return Err(parser_error(
            "CUSTOMER_DATA_IMPORT_SOURCE_COLUMN_COUNT_INVALID",
            "customer_data.import.source.headers",
            "Import source column count is invalid.",
        ));
    }

    let mut seen_headers = BTreeSet::new();
    let mut headers = Vec::with_capacity(raw_headers.len());
    for raw in &raw_headers {
        let canonical = raw.trim();
        if canonical.is_empty()
            || canonical.len() > MAXIMUM_IMPORT_SOURCE_COLUMN_NAME_BYTES
            || canonical.chars().any(char::is_control)
        {
            return Err(parser_error(
                "CUSTOMER_DATA_IMPORT_SOURCE_HEADER_INVALID",
                "customer_data.import.source.headers",
                "Import source header is invalid.",
            ));
        }
        if !seen_headers.insert(canonical.to_owned()) {
            return Err(parser_error(
                "CUSTOMER_DATA_IMPORT_SOURCE_HEADER_DUPLICATE",
                "customer_data.import.source.headers",
                "Import source contains duplicate canonical column names.",
            ));
        }
        headers.push(canonical.to_owned());
    }

    let mut rows = Vec::new();
    for result in reader.records() {
        if rows.len() >= MAXIMUM_IMPORT_SOURCE_ROWS {
            return Err(parser_error(
                "CUSTOMER_DATA_IMPORT_SOURCE_ROW_LIMIT_EXCEEDED",
                "customer_data.import.source.rows",
                "Import source exceeds the maximum supported row count.",
            ));
        }
        let record = result
            .map_err(|error| csv_error("CUSTOMER_DATA_IMPORT_SOURCE_ROW_INVALID", error))?;
        if record.len() != headers.len() {
            return Err(parser_error(
                "CUSTOMER_DATA_IMPORT_SOURCE_ROW_WIDTH_INVALID",
                "customer_data.import.source.rows",
                "Import source row width does not match the immutable header width.",
            ));
        }
        let mut columns = BTreeMap::new();
        for (header, value) in headers.iter().zip(record.iter()) {
            if value.len() > MAXIMUM_IMPORT_SOURCE_CELL_BYTES {
                return Err(parser_error(
                    "CUSTOMER_DATA_IMPORT_SOURCE_CELL_TOO_LARGE",
                    "customer_data.import.source.rows",
                    "Import source cell exceeds the maximum supported size.",
                ));
            }
            columns.insert(header.clone(), value.to_owned());
        }
        let row_position = u32::try_from(rows.len() + 1).map_err(|_| parser_error(
            "CUSTOMER_DATA_IMPORT_SOURCE_ROW_POSITION_INVALID",
            "customer_data.import.source.rows",
            "Import source row position is invalid.",
        ))?;
        rows.push(ParsedImportSourceRow {
            row_position,
            columns,
        });
    }

    if rows.is_empty() {
        return Err(parser_error(
            "CUSTOMER_DATA_IMPORT_SOURCE_EMPTY",
            "customer_data.import.source.rows",
            "Import source must contain at least one data row.",
        ));
    }

    Ok(ParsedImportSource { headers, rows })
}

fn csv_error(code: &'static str, error: csv::Error) -> SdkError {
    parser_error(
        code,
        "customer_data.import.source",
        "Import source bytes are not valid for the immutable CSV parser profile.",
    )
    .with_internal_reference(error.to_string())
}

fn parser_error(
    code: &'static str,
    field: &'static str,
    safe_message: &'static str,
) -> SdkError {
    let mut error = SdkError::new(code, ErrorCategory::InvalidArgument, false, safe_message);
    error.field_violations.push(FieldViolation {
        field: FieldName::try_new(field).expect("static import source parser field must be valid"),
        code: "INVALID".to_owned(),
        safe_message: safe_message.to_owned(),
    });
    error
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_quoted_csv_with_canonical_headers_and_one_based_positions() {
        let profile = ImportParserProfile::csv_v1(b',', b'"').unwrap();
        let parsed = parse_import_source(
            b" kind ,display_name\r\nperson,\"Ada, Countess of Lovelace\"\r\norganization,Analytical Engines Ltd\r\n",
            &profile,
        )
        .unwrap();

        assert_eq!(parsed.headers(), &["kind", "display_name"]);
        assert_eq!(parsed.row_count(), 2);
        assert_eq!(parsed.rows()[0].row_position, 1);
        assert_eq!(
            parsed.rows()[0].columns.get("display_name").unwrap(),
            "Ada, Countess of Lovelace"
        );
    }

    #[test]
    fn rejects_duplicate_headers_after_canonicalization() {
        let profile = ImportParserProfile::csv_v1(b',', b'"').unwrap();
        let error = parse_import_source(b"kind, kind \nperson,person\n", &profile).unwrap_err();
        assert_eq!(error.code, "CUSTOMER_DATA_IMPORT_SOURCE_HEADER_DUPLICATE");
    }
}
