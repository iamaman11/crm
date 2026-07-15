//! Deterministic Party-export CSV canonicalization.
//!
//! The v1 profile is intentionally safe for common spreadsheet consumers. Text cells that could be
//! interpreted as formulas are neutralized before ordinary CSV quoting. These rules are part of the
//! versioned export canonicalization contract and must never change in place after publication.

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
    fn quotes_commas_quotes_and_line_breaks_deterministically() {
        assert_eq!(encode_party_export_csv_cell("Northwind, Ltd"), "\"Northwind, Ltd\"");
        assert_eq!(encode_party_export_csv_cell("Ada \"Countess\""), "\"Ada \"\"Countess\"\"\"");
        assert_eq!(encode_party_export_csv_cell("line1\nline2"), "\"line1\nline2\"");
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
            assert!(encoded.contains('\''));
            let unquoted = encoded.trim_matches('"');
            assert!(unquoted.starts_with('\''));
        }
    }

    #[test]
    fn leaves_safe_text_unchanged_before_ordinary_csv_escaping() {
        assert_eq!(encode_party_export_csv_cell("Ada Lovelace"), "Ada Lovelace");
        assert_eq!(encode_party_export_csv_cell("1"), "1");
        assert_eq!(encode_party_export_csv_cell("party-01J00000000000000000000000"), "party-01J00000000000000000000000");
    }

    #[test]
    fn same_cells_always_produce_identical_bytes() {
        let cells = ["party-1", "person", "=SUM(1,1)", "7"];
        assert_eq!(
            encode_party_export_csv_record(&cells),
            encode_party_export_csv_record(&cells)
        );
    }
}
