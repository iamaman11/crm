use crm_module_sdk::{ErrorCategory, FieldName, FieldViolation, RecordId, SdkError};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;

const MAX_EXTERNAL_PARTY_IDENTIFIER_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceSystemId(RecordId);

impl SourceSystemId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, SdkError> {
        RecordId::try_new(value.into()).map(Self).map_err(|error| {
            invalid(
                "CUSTOMER_DATA_SOURCE_SYSTEM_ID_INVALID",
                "customer_data.source.source_system_id",
                error.to_string(),
            )
        })
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportSourceFormat {
    Csv,
}

impl ImportSourceFormat {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Csv => "csv",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportTextEncoding {
    Utf8,
}

impl ImportTextEncoding {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Utf8 => "utf-8",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportHeaderMode {
    RequiredFirstRow,
}

impl ImportHeaderMode {
    pub const fn code(self) -> &'static str {
        match self {
            Self::RequiredFirstRow => "required-first-row",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportParserVersion {
    CsvV1,
}

impl ImportParserVersion {
    pub const fn code(self) -> &'static str {
        match self {
            Self::CsvV1 => "csv-v1",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportCanonicalizationVersion {
    V1,
}

impl ImportCanonicalizationVersion {
    pub const fn code(self) -> &'static str {
        match self {
            Self::V1 => "customer-import-v1",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportParserProfile {
    format: ImportSourceFormat,
    encoding: ImportTextEncoding,
    delimiter: u8,
    quote: u8,
    header_mode: ImportHeaderMode,
    parser_version: ImportParserVersion,
    canonicalization_version: ImportCanonicalizationVersion,
}

impl ImportParserProfile {
    pub fn try_new(
        format: ImportSourceFormat,
        encoding: ImportTextEncoding,
        delimiter: u8,
        quote: u8,
        header_mode: ImportHeaderMode,
        parser_version: ImportParserVersion,
        canonicalization_version: ImportCanonicalizationVersion,
    ) -> Result<Self, SdkError> {
        if matches!(delimiter, 0 | b'\r' | b'\n') || !delimiter.is_ascii() {
            return Err(invalid(
                "CUSTOMER_DATA_IMPORT_DELIMITER_INVALID",
                "customer_data.source.parser_profile.delimiter",
                "delimiter must be one non-zero ASCII byte other than CR or LF",
            ));
        }
        if matches!(quote, 0 | b'\r' | b'\n') || !quote.is_ascii() {
            return Err(invalid(
                "CUSTOMER_DATA_IMPORT_QUOTE_INVALID",
                "customer_data.source.parser_profile.quote",
                "quote must be one non-zero ASCII byte other than CR or LF",
            ));
        }
        if delimiter == quote {
            return Err(invalid(
                "CUSTOMER_DATA_IMPORT_DIALECT_INVALID",
                "customer_data.source.parser_profile",
                "delimiter and quote bytes must be distinct",
            ));
        }
        Ok(Self {
            format,
            encoding,
            delimiter,
            quote,
            header_mode,
            parser_version,
            canonicalization_version,
        })
    }

    pub fn csv_v1(delimiter: u8, quote: u8) -> Result<Self, SdkError> {
        Self::try_new(
            ImportSourceFormat::Csv,
            ImportTextEncoding::Utf8,
            delimiter,
            quote,
            ImportHeaderMode::RequiredFirstRow,
            ImportParserVersion::CsvV1,
            ImportCanonicalizationVersion::V1,
        )
    }

    pub const fn format(&self) -> ImportSourceFormat {
        self.format
    }

    pub const fn encoding(&self) -> ImportTextEncoding {
        self.encoding
    }

    pub const fn delimiter(&self) -> u8 {
        self.delimiter
    }

    pub const fn quote(&self) -> u8 {
        self.quote
    }

    pub const fn header_mode(&self) -> ImportHeaderMode {
        self.header_mode
    }

    pub const fn parser_version(&self) -> ImportParserVersion {
        self.parser_version
    }

    pub const fn canonicalization_version(&self) -> ImportCanonicalizationVersion {
        self.canonicalization_version
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExternalPartyIdentifierDigest(String);

impl ExternalPartyIdentifierDigest {
    pub fn for_identifier(value: impl Into<String>) -> Result<Self, SdkError> {
        let value = value.into();
        if value.chars().any(char::is_control) {
            return Err(invalid(
                "CUSTOMER_DATA_EXTERNAL_PARTY_IDENTIFIER_INVALID",
                "customer_data.row.external_party_identifier",
                "external Party identifier must not contain control characters",
            ));
        }
        let canonical = value.trim();
        if canonical.is_empty() || canonical.len() > MAX_EXTERNAL_PARTY_IDENTIFIER_BYTES {
            return Err(invalid(
                "CUSTOMER_DATA_EXTERNAL_PARTY_IDENTIFIER_INVALID",
                "customer_data.row.external_party_identifier",
                format!(
                    "external Party identifier must be non-empty and not exceed {MAX_EXTERNAL_PARTY_IDENTIFIER_BYTES} UTF-8 bytes"
                ),
            ));
        }
        let digest = Sha256::digest(canonical.as_bytes());
        Ok(Self(hex_digest(digest)))
    }

    pub fn try_from_sha256(value: impl Into<String>) -> Result<Self, SdkError> {
        let value = value.into();
        if value.len() != 64
            || value
                .as_bytes()
                .iter()
                .any(|byte| !byte.is_ascii_hexdigit() || byte.is_ascii_uppercase())
        {
            return Err(invalid(
                "CUSTOMER_DATA_EXTERNAL_PARTY_IDENTIFIER_DIGEST_INVALID",
                "customer_data.row.external_party_identifier_sha256",
                "external Party identifier digest must be exactly 64 lowercase hexadecimal characters",
            ));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
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

fn invalid(code: &'static str, field: &'static str, internal: impl Into<String>) -> SdkError {
    let mut error = SdkError::new(
        code,
        ErrorCategory::InvalidArgument,
        false,
        "The customer-data import profile is invalid.",
    )
    .with_internal_reference(internal);
    error.field_violations.push(FieldViolation {
        field: FieldName::try_new(field).expect("static customer-data profile field must be valid"),
        code: "INVALID".to_owned(),
        safe_message: "The customer-data import profile field is invalid.".to_owned(),
    });
    error
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_profile_rejects_ambiguous_csv_dialect() {
        assert_eq!(
            ImportParserProfile::csv_v1(b',', b',').unwrap_err().code,
            "CUSTOMER_DATA_IMPORT_DIALECT_INVALID"
        );
    }

    #[test]
    fn external_identifier_digest_preserves_case_semantics() {
        let first = ExternalPartyIdentifierDigest::for_identifier("Customer-42").unwrap();
        let second = ExternalPartyIdentifierDigest::for_identifier("customer-42").unwrap();
        assert_ne!(first, second);
        assert_eq!(first.as_str().len(), 64);
    }
}
