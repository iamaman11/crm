use crate::canonical_json::{self, CanonicalJsonError};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::error::Error;
use std::fmt;

pub const PROFILE_ID: &str = canonical_json::PROFILE_ID;
const PROFILE_FIELD: &str = "canonicalization_profile";

pub mod persisted_state_json {
    use super::*;

    pub fn to_vec<T: Serialize>(value: &T) -> Result<Vec<u8>, ProfiledStateError> {
        let mut value = serde_json::to_value(value).map_err(ProfiledStateError::Json)?;
        let object = value
            .as_object_mut()
            .ok_or(ProfiledStateError::RootNotObject)?;
        if object
            .insert(
                PROFILE_FIELD.to_owned(),
                Value::String(PROFILE_ID.to_owned()),
            )
            .is_some()
        {
            return Err(ProfiledStateError::ReservedProfileField);
        }
        canonical_json::to_vec(&value).map_err(ProfiledStateError::Canonical)
    }

    pub fn from_slice<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, ProfiledStateError> {
        let mut value: Value = serde_json::from_slice(bytes).map_err(ProfiledStateError::Json)?;
        let object = value
            .as_object_mut()
            .ok_or(ProfiledStateError::RootNotObject)?;
        match object.remove(PROFILE_FIELD) {
            Some(Value::String(profile)) if profile == PROFILE_ID => {}
            Some(_) => return Err(ProfiledStateError::ProfileMismatch),
            None => return Err(ProfiledStateError::ProfileMissing),
        }
        serde_json::from_value(value).map_err(ProfiledStateError::Json)
    }
}

#[derive(Debug)]
pub enum ProfiledStateError {
    Json(serde_json::Error),
    Canonical(CanonicalJsonError),
    RootNotObject,
    ReservedProfileField,
    ProfileMissing,
    ProfileMismatch,
}

impl fmt::Display for ProfiledStateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(formatter, "JSON processing failed: {error}"),
            Self::Canonical(error) => {
                write!(formatter, "canonical JSON processing failed: {error}")
            }
            Self::RootNotObject => formatter.write_str("profiled state root must be an object"),
            Self::ReservedProfileField => formatter
                .write_str("profiled state used the reserved canonicalization profile field"),
            Self::ProfileMissing => {
                formatter.write_str("profiled state is missing the canonicalization profile")
            }
            Self::ProfileMismatch => {
                formatter.write_str("profiled state canonicalization profile is unsupported")
            }
        }
    }
}

impl Error for ProfiledStateError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            Self::Canonical(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::persisted_state_json;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(deny_unknown_fields)]
    struct Fixture {
        version_id: String,
        count: u32,
    }

    #[test]
    fn persisted_state_stores_and_requires_exact_profile() {
        let fixture = Fixture {
            version_id: "version-1".to_owned(),
            count: 7,
        };
        let bytes = persisted_state_json::to_vec(&fixture).unwrap();
        assert_eq!(
            bytes,
            br#"{"canonicalization_profile":"crm.cjson/v1","count":7,"version_id":"version-1"}"#
        );
        assert_eq!(
            persisted_state_json::from_slice::<Fixture>(&bytes).unwrap(),
            fixture
        );
        assert!(
            persisted_state_json::from_slice::<Fixture>(br#"{"count":7,"version_id":"version-1"}"#)
                .is_err()
        );
        assert!(
            persisted_state_json::from_slice::<Fixture>(
                br#"{"canonicalization_profile":"crm.cjson/v2","count":7,"version_id":"version-1"}"#
            )
            .is_err()
        );
    }
}
