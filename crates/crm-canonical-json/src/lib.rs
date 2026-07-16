#![forbid(unsafe_code)]

//! Versioned canonical JSON for stable CRM identities and semantic digests.

use serde::Serialize;
use serde_json::Value;
use std::error::Error;
use std::fmt;

pub const PROFILE_ID: &str = "crm.cjson/v1";
pub const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Debug)]
pub enum CanonicalJsonError {
    Serialization(serde_json::Error),
    InvalidObjectKey(String),
    IntegerOutOfRange(String),
    FloatingPointForbidden,
}

impl fmt::Display for CanonicalJsonError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Serialization(error) => {
                write!(formatter, "canonical JSON serialization failed: {error}")
            }
            Self::InvalidObjectKey(key) => write!(
                formatter,
                "canonical JSON object key is not an ASCII identifier: {key}"
            ),
            Self::IntegerOutOfRange(value) => write!(
                formatter,
                "canonical JSON integer exceeds the safe range: {value}"
            ),
            Self::FloatingPointForbidden => {
                formatter.write_str("floating-point values are forbidden by crm.cjson/v1")
            }
        }
    }
}

impl Error for CanonicalJsonError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Serialization(error) => Some(error),
            _ => None,
        }
    }
}

impl From<serde_json::Error> for CanonicalJsonError {
    fn from(error: serde_json::Error) -> Self {
        Self::Serialization(error)
    }
}

pub fn to_vec<T: Serialize>(value: &T) -> Result<Vec<u8>, CanonicalJsonError> {
    let value = serde_json::to_value(value)?;
    let mut output = Vec::new();
    write_value(&value, &mut output)?;
    Ok(output)
}

fn write_value(value: &Value, output: &mut Vec<u8>) -> Result<(), CanonicalJsonError> {
    match value {
        Value::Null => output.extend_from_slice(b"null"),
        Value::Bool(true) => output.extend_from_slice(b"true"),
        Value::Bool(false) => output.extend_from_slice(b"false"),
        Value::String(value) => output.extend_from_slice(serde_json::to_string(value)?.as_bytes()),
        Value::Number(number) => {
            if let Some(value) = number.as_u64() {
                if value > MAX_SAFE_INTEGER {
                    return Err(CanonicalJsonError::IntegerOutOfRange(value.to_string()));
                }
                output.extend_from_slice(value.to_string().as_bytes());
            } else if let Some(value) = number.as_i64() {
                if value.unsigned_abs() > MAX_SAFE_INTEGER {
                    return Err(CanonicalJsonError::IntegerOutOfRange(value.to_string()));
                }
                output.extend_from_slice(value.to_string().as_bytes());
            } else {
                return Err(CanonicalJsonError::FloatingPointForbidden);
            }
        }
        Value::Array(values) => {
            output.push(b'[');
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    output.push(b',');
                }
                write_value(value, output)?;
            }
            output.push(b']');
        }
        Value::Object(values) => {
            let mut keys: Vec<_> = values.keys().collect();
            for key in &keys {
                if !is_ascii_identifier(key) {
                    return Err(CanonicalJsonError::InvalidObjectKey((*key).clone()));
                }
            }
            keys.sort_unstable();
            output.push(b'{');
            for (index, key) in keys.into_iter().enumerate() {
                if index > 0 {
                    output.push(b',');
                }
                output.extend_from_slice(serde_json::to_string(key)?.as_bytes());
                output.push(b':');
                write_value(&values[key], output)?;
            }
            output.push(b'}');
        }
    }
    Ok(())
}

fn is_ascii_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.is_ascii()
        && value
            .as_bytes()
            .first()
            .is_some_and(|byte| byte.is_ascii_alphabetic() || *byte == b'_')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;
    use std::collections::BTreeMap;

    #[derive(Serialize)]
    struct Fixture {
        z_value: u64,
        a_value: BTreeMap<String, String>,
    }

    #[test]
    fn sorts_ascii_identifier_keys_and_removes_whitespace() {
        let fixture = Fixture {
            z_value: 7,
            a_value: BTreeMap::from([
                ("second".to_owned(), "b".to_owned()),
                ("first".to_owned(), "a".to_owned()),
            ]),
        };
        assert_eq!(
            to_vec(&fixture).unwrap(),
            br#"{"a_value":{"first":"a","second":"b"},"z_value":7}"#
        );
    }

    #[test]
    fn rejects_floats_invalid_keys_and_unsafe_integers() {
        assert!(matches!(
            to_vec(&serde_json::json!({"value": 1.5})),
            Err(CanonicalJsonError::FloatingPointForbidden)
        ));
        assert!(matches!(
            to_vec(&serde_json::json!({"not a key": 1})),
            Err(CanonicalJsonError::InvalidObjectKey(_))
        ));
        assert!(matches!(
            to_vec(&serde_json::json!({"value": MAX_SAFE_INTEGER + 1})),
            Err(CanonicalJsonError::IntegerOutOfRange(_))
        ));
    }
}
