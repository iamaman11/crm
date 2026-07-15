//! Deterministic Party-export CSV canonicalization.
//!
//! The v1 profile is intentionally safe for common spreadsheet consumers. Text cells that could be
//! interpreted as formulas are neutralized before ordinary CSV quoting. These rules are part of the
//! versioned export canonicalization contract and must never change in place after publication.

use crm_module_sdk::SdkError;
use sha2::{Digest, Sha256};

pub const PARTY_EXPORT_CANONICALIZATION_V1: &str = "party-export-csv/v1";
pub const PARTY_EXPORT_CSV_MEDIA_TYPE: &str = "text/csv; charset=utf-8";

/// Encodes one UTF-8 CSV record using the immutable v1 canonicalization rules.
///
/// The returned bytes always end in a single LF (`\n`) and never contain a BOM.
pub fn encode_party_export_csv_record(cells: &[&str]) -> Vec<u8> {
    let mut output = String::new();
    for (index, cell) in cells.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&encode_party_export_csv_cell(cell));
    }
    output.push('\n');
    output.into_bytes()
}

/// Encodes owned canonical cell values without changing their ordering or bytes.
///
/// Execution composition builds row values as owned strings after live field authorization. This
/// bounded adapter keeps that worker path on the same immutable v1 CSV canonicalization function.
pub fn canonical_party_export_csv_record(cells: &[String]) -> Result<Vec<u8>, SdkError> {
    let borrowed = cells.iter().map(String::as_str).collect::<Vec<_>>();
    Ok(encode_party_export_csv_record(&borrowed))
}

/// Encodes one CSV cell after deterministic spreadsheet-formula neutralization.
pub fn encode_party_export_csv_cell(value: &str) -> String {
    let neutralized = neutralize_spreadsheet_formula(value);
    if requires_csv_quotes(&neutralized) {
        let mut quoted = String::with_capacity(neutralized.len() + 2);
        quoted.push('"');
        for character in neutralized.chars() {
            if character == '"' {
                quoted.push('"');
            }
            quoted.push(character);
        }
        quoted.push('"');
        quoted
    } else {
        neutralized
    }
}

/// Returns the exact SHA-256 digest of deterministic export bytes.
pub fn party_export_sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

/// Domain-separated deterministic SHA-256 over length-prefixed parts.
pub fn party_export_hash_parts(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    for part in parts {
        hasher.update((part.len() as u64).to_be_bytes());
        hasher.update(part);
    }
    hasher.finalize().into()
}

/// Lowercase hexadecimal encoding used by immutable export digest evidence.
pub fn party_export_hex(bytes: &[u8]) -> String {
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut value, "{byte:02x}").expect("writing to String cannot fail");
    }
    value
}

fn neutralize_spreadsheet_formula(value: &str) -> String {
    let first_non_whitespace = value.chars().find(|character| !character.is_whitespace());
    if matches!(first_non_whitespace, Some('=' | '+' | '-' | '@')) {
        let mut safe = String::with_capacity(value.len() + 1);
        safe.push('\'');
        safe.push_str(value);
        safe
    } else {
        value.to_owned()
    }
}

fn requires_csv_quotes(value: &str) -> bool {
    value
        .chars()
        .any(|character| matches!(character, ',' | '"' | '\n' | '\r'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_fixed_lf_records_without_bom() {
        let bytes = encode_party_export_csv_record(&["party_id", "display_name"]);
        assert_eq!(bytes, b"party_id,display_name\n");
        assert!(!bytes.starts_with(&[0xEF, 0xBB, 0xBF]));
        assert!(!bytes.windows(2).any(|window| window == b"\r\n"));
    }

    #[test]
    fn owned_cells_use_the_same_canonical_record_encoding() {
        let cells = vec!["party-1".to_owned(), "Ada Lovelace".to_owned()];
        assert_eq!(
            canonical_party_export_csv_record(&cells).unwrap(),
            encode_party_export_csv_record(&["party-1", "Ada Lovelace"])
        );
    }

    #[test]
    fn quotes_commas_quotes_and_line_breaks_deterministically() {
        assert_eq!(
            encode_party_export_csv_cell("Northwind, Ltd"),
            "\"Northwind, Ltd\""
        );
        assert_eq!(
            encode_party_export_csv_cell("Ada \"Countess\""),
            "\"Ada \"\"Countess\"\"\""
        );
        assert_eq!(
            encode_party_export_csv_cell("line1\nline2"),
            "\"line1\nline2\""
        );
    }

    #[test]
    fn neutralizes_formula_like_text_before_csv_escaping() {
        for dangerous in [
            "=HYPERLINK(\"https://example.invalid\")",
            "+SUM(1,1)",
            "-1+2",
            "@SUM(1,1)",
            "   =CMD()",
        ] {
            let encoded = encode_party_export_csv_cell(dangerous);
            let first_payload_character = encoded
                .trim_start_matches('"')
                .chars()
                .next()
                .expect("encoded dangerous cell must not be empty");
            assert_eq!(first_payload_character, '\'');
        }
    }

    #[test]
    fn leaves_safe_text_unchanged_before_ordinary_csv_escaping() {
        assert_eq!(encode_party_export_csv_cell("Ada Lovelace"), "Ada Lovelace");
        assert_eq!(encode_party_export_csv_cell("1"), "1");
        assert_eq!(
            encode_party_export_csv_cell("party-01J00000000000000000000000"),
            "party-01J00000000000000000000000"
        );
    }

    #[test]
    fn same_cells_always_produce_identical_bytes() {
        let cells = ["party-1", "person", "=SUM(1,1)", "7"];
        assert_eq!(
            encode_party_export_csv_record(&cells),
            encode_party_export_csv_record(&cells)
        );
    }

    #[test]
    fn deterministic_hash_helpers_are_domain_separated() {
        assert_eq!(party_export_sha256(b"abc"), party_export_sha256(b"abc"));
        assert_ne!(
            party_export_hash_parts(b"domain-a", &[b"abc"]),
            party_export_hash_parts(b"domain-b", &[b"abc"])
        );
        assert_eq!(party_export_hex(&[0xab, 0xcd]), "abcd");
    }
}
