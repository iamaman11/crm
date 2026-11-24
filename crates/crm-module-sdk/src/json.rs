use serde::de::DeserializeOwned;
use serde_json::error::Category;
use serde_path_to_error::{Path, Segment};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonDecodeKind {
    Io,
    Syntax,
    Data,
    Eof,
}

/// Contains only trusted structural coordinates, never a raw Serde error,
/// payload value, untrusted object key or enum variant.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("JSON {kind:?} at {path} (line {line}, column {column})")]
pub struct JsonDecodeDiagnostic {
    pub kind: JsonDecodeKind,
    pub path: String,
    pub line: usize,
    pub column: usize,
}

impl JsonDecodeDiagnostic {
    fn new(error: &serde_json::Error, path: String) -> Self {
        let kind = match error.classify() {
            Category::Io => JsonDecodeKind::Io,
            Category::Syntax => JsonDecodeKind::Syntax,
            Category::Data => JsonDecodeKind::Data,
            Category::Eof => JsonDecodeKind::Eof,
        };
        Self {
            kind,
            path,
            line: error.line(),
            column: error.column(),
        }
    }
}

// Only emit literals approved by the calling schema owner. These are
// diagnostic labels, not another validation schema. Unknown keys are masked.
fn safe_path(path: &Path, known_fields: &[&'static str]) -> String {
    use std::fmt::Write as _;
    let mut safe = String::from("$");
    for segment in path.iter().take(16) {
        match segment {
            Segment::Seq { index } => {
                let _ = write!(safe, "[{index}]");
            }
            Segment::Map { key } => {
                safe.push('.');
                safe.push_str(
                    known_fields
                        .iter()
                        .copied()
                        .find(|field| *field == key)
                        .unwrap_or("?"),
                );
            }
            Segment::Enum { .. } | Segment::Unknown => safe.push_str(".?"),
        }
        if safe.len() > 256 {
            return "$.?".to_owned();
        }
    }
    if path.iter().len() > 16 {
        safe.push_str(".?");
    }
    safe
}

/// Deserialize a bounded, owner-validated byte buffer using Serde's actual
/// schema; report a safe structural error without retaining the raw error.
/// The caller owns size limits, domain invariants and its stable error code.
pub fn decode_json<T: DeserializeOwned>(
    bytes: &[u8],
    known_fields: &[&'static str],
) -> Result<T, JsonDecodeDiagnostic> {
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    let value = serde_path_to_error::deserialize(&mut decoder).map_err(|error| {
        JsonDecodeDiagnostic::new(error.inner(), safe_path(error.path(), known_fields))
    })?;
    decoder
        .end()
        .map_err(|error| JsonDecodeDiagnostic::new(&error, "$".to_owned()))?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use std::collections::BTreeMap;

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Fixture {
        items: Vec<Item>,
        metadata: BTreeMap<String, u8>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Item {
        quantity: u8,
    }

    const FIELDS: &[&str] = &["items", "quantity", "metadata"];

    #[test]
    fn nested_path_and_trailing_content_are_precise_and_safe() {
        let valid =
            decode_json::<Fixture>(br#"{"items":[{"quantity":42}],"metadata":{}}  "#, FIELDS)
                .unwrap();
        assert_eq!(valid.items[0].quantity, 42);
        assert!(valid.metadata.is_empty());
        let error = decode_json::<Fixture>(
            br#"{"items":[{"quantity":"private-payload"}],"metadata":{}}"#,
            FIELDS,
        )
        .unwrap_err();
        assert_eq!(error.kind, JsonDecodeKind::Data);
        assert_eq!(error.path, "$.items[0].quantity");
        assert_eq!(error.line, 1);
        assert!(error.column > 0);
        assert!(!format!("{error:?} {error}").contains("private-payload"));
        let error =
            decode_json::<Fixture>(br#"{"items":[],"metadata":{}} true"#, FIELDS).unwrap_err();
        assert_eq!(error.kind, JsonDecodeKind::Syntax);
        assert_eq!(error.path, "$");
    }

    #[test]
    fn untrusted_keys_values_and_syntax_are_never_diagnostic_labels() {
        let error = decode_json::<Fixture>(
            br#"{"items":[],"metadata":{"private-key":"private-value"}}"#,
            FIELDS,
        )
        .unwrap_err();
        assert_eq!(error.path, "$.metadata.?");
        let diagnostic = format!("{error:?} {error}");
        assert!(!diagnostic.contains("private-key"));
        assert!(!diagnostic.contains("private-value"));
        let error =
            decode_json::<Fixture>(br#"{"items":[],"metadata":{},"private-field":1}"#, FIELDS)
                .unwrap_err();
        assert!(!format!("{error:?} {error}").contains("private-field"));
        assert_eq!(
            decode_json::<Fixture>(b"{", FIELDS).unwrap_err().kind,
            JsonDecodeKind::Eof
        );
    }
}
