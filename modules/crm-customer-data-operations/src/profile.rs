//! Immutable source-system identity, parser-profile and source-identifier evidence types.
//!
//! These types keep source interpretation and source-system identifiers explicit and prevent
//! external identifiers from being treated as canonical CRM Party identities.

use crm_module_sdk::{ErrorCategory, FieldName, FieldViolation, RecordId, SdkError};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;

const MAX_EXTERNAL_PARTY_IDENTIFIER_BYTES: usize = 512;

macro_rules! record_identifier_type {
    ($name:ident, $code:literal, $field:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(RecordId);

        impl $name {
            pub fn try_new(value: impl Into<String>) -> Result<Self, SdkError> {
                RecordId::try_new(value.into()).map(Self).map_err(|error| {
                    invalid($code, $field, error.to_string())
                })
            }

            pub fn as_str(&self) -> &str {
                self.0.as_str()
            }
        }
    };
}

record_identifier_type!(
    SourceSystemId,
    "CUSTOMER_DATA_SOURCE_SYSTEM_ID_INVALID",
    "customer_data.source.source_system_id"
);
record_identifier_type!(
    ParserProfileId,
    "CUSTOMER_DATA_PARSER_PROFILE_ID_INVALID",
    "customer_data.source.parser_profile.parser_profile_id"
);
record_identifier_type!(
    ParserProfileVersion,
    "CUSTOMER_DATA_PARSER_PROFILE_VERSION_INVALID",
    "customer_data.source.parser_profile.parser_profile_version"
);
record_identifier_type!(
    CanonicalizationProfileId,
    "CUSTOMER_DATA_CANONICALIZATION_PROFILE_ID_INVALID",
    "customer_data.source.parser_profile.canonicalization_profile_id"
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportSourceFormat {
    DelimitedText,
}

impl ImportSourceFormat {
    pub const fn code(self) -> &'static str {
        match self {
            Self::DelimitedText => "delimited-text",
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
    FirstRow,
}

impl ImportHeaderMode {
    pub const fn code(self) -> &'static str {
        match self {
            Self::FirstRow => "first-row",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportParserProfile {
    parser_profile_id: ParserProfileId,
    parser_profile_version: ParserProfileVersion,
    format: ImportSourceFormat,
    encoding: ImportTextEncoding,
    delimiter: char,
    quote_character: char,
    header_mode: ImportHeaderMode,
    canonicalization_profile_id: CanonicalizationProfileId,
}

impl ImportParserProfile {
    pub fn try_new(
        parser_profile_id: ParserProfileId,
        parser_profile_version: ParserProfileVersion,
        format: ImportSourceFormat,
        encoding: ImportTextEncoding,
        delimiter: char,
        quote_character: char,
        header_mode: ImportHeaderMode,
        canonicalization_profile_id: CanonicalizationProfileId,
    ) -> Result<Self, SdkError> {
        validate_dialect_character(
            delimiter,
            "CUSTOMER_DATA_IMPORT_DELIMITER_INVALID",
            "customer_data.source.parser_profile.delimiter",
            "delimiter",
        )?;
        validate_dialect_character(
            quote_character,
            "CUSTOMER_DATA_IMPORT_QUOTE_INVALID",
            "customer_data.source.parser_profile.quote_character",
            "quote character",
        )?;
        if delimiter == quote_character {
            return Err(invalid(
                "CUSTOMER_DATA_IMPORT_DIALECT_INVALID",
                "customer_data.source.parser_profile",
                "delimiter and quote character must be distinct",
            ));
        }
        Ok(Self {
            parser_profile_id,
            parser_profile_version,
            format,
            encoding,
            delimiter,
            quote_character,
            header_mode,
            canonicalization_profile_id,
        })
    }

    pub fn delimited_text_v1(delimiter: char, quote_character: char) -> Result<Self, SdkError> {
        Self::try_new(
            ParserProfileId::try_new("crm.import.delimited-text")?,
            ParserProfileVersion::try_new("1.0.0")?,
            ImportSourceFormat::DelimitedText,
            ImportTextEncoding::Utf8,
            delimiter,
            quote_character,
            ImportHeaderMode::FirstRow,
            CanonicalizationProfileId::try_new("crm.import.canonicalization-v1")?,
        )
    }

    pub fn parser_profile_id(&self) -> &ParserProfileId {
        &self.parser_profile_id
    }

    pub fn parser_profile_version(&self) -> &ParserProfileVersion {
        &self.parser_profile_version
    }

    pub const fn format(&self) -> ImportSourceFormat {
        self.format
    }

    pub const fn encoding(&self) -> ImportTextEncoding {
        self.encoding
    }

    pub const fn delimiter(&self) -> char {
        self.delimiter
    }

    pub const fn quote_character(&self) -> char {
        self.quote_character
    }

    pub const fn header_mode(&self) -> ImportHeaderMode {
        self.header_mode
    }

    pub fn canonicalization_profile_id(&self) -> &CanonicalizationProfileId {
        &self.canonicalization_profile_id
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
                "customer_data.row.source_external_id",
                "source external identifier must not contain control characters",
            ));
        }
        let canonical = value.trim();
        if canonical.is_empty() || canonical.len() > MAX_EXTERNAL_PARTY_IDENTIFIER_BYTES {
            return Err(invalid(
                "CUSTOMER_DATA_EXTERNAL_PARTY_IDENTIFIER_INVALID",
                "customer_data.row.source_external_id",
                format!(
                    "source external identifier must be non-empty and not exceed {MAX_EXTERNAL_PARTY_IDENTIFIER_BYTES} UTF-8 bytes"
                ),
            ));
        }
        Ok(Self(hex_digest(Sha256::digest(canonical.as_bytes()))))
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
                "customer_data.row.source_external_id_sha256",
                "source external identifier digest must be exactly 64 lowercase hexadecimal characters",
            ));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn validate_dialect_character(
    value: char,
    code: &'static str,
    field: &'static str,
    label: &str,
) -> Result<(), SdkError> {
    if value == '\0' || value == '\r' || value == '\n' || value.is_control() {
        return Err(invalid(
            code,
            field,
            format!("{label} must be one non-control Unicode scalar other than NUL, CR or LF"),
        ));
    }
    Ok(())
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
    fn parser_profile_rejects_ambiguous_dialect() {
        assert_eq!(
            ImportParserProfile::delimited_text_v1(',', ',')
                .unwrap_err()
                .code,
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
