use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::error::Error;
use std::fmt;

pub const PROFILE_ID: &str = crm_canonical_json::PROFILE_ID;
const PROFILE_FIELD: &str = "canonicalization_profile";

#[derive(Debug)]
pub struct DataQualityCanonicalizationError(String);

impl fmt::Display for DataQualityCanonicalizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for DataQualityCanonicalizationError {}

pub fn semantic_to_vec<T: Serialize>(
    value: &T,
) -> Result<Vec<u8>, DataQualityCanonicalizationError> {
    crm_canonical_json::to_vec(value)
        .map_err(|error| DataQualityCanonicalizationError(error.to_string()))
}

pub fn state_to_vec<T: Serialize>(value: &T) -> Result<Vec<u8>, DataQualityCanonicalizationError> {
    let mut value = serde_json::to_value(value)
        .map_err(|error| DataQualityCanonicalizationError(error.to_string()))?;
    let object = value.as_object_mut().ok_or_else(|| {
        DataQualityCanonicalizationError(
            "Data Quality persisted state must be a top-level object".to_owned(),
        )
    })?;
    if object
        .insert(
            PROFILE_FIELD.to_owned(),
            Value::String(PROFILE_ID.to_owned()),
        )
        .is_some()
    {
        return Err(DataQualityCanonicalizationError(
            "Data Quality persisted state must not define the canonicalization profile field"
                .to_owned(),
        ));
    }
    crm_canonical_json::to_vec(&value)
        .map_err(|error| DataQualityCanonicalizationError(error.to_string()))
}

pub fn state_from_slice<T: DeserializeOwned>(
    bytes: &[u8],
) -> Result<T, DataQualityCanonicalizationError> {
    let mut value: Value = serde_json::from_slice(bytes)
        .map_err(|error| DataQualityCanonicalizationError(error.to_string()))?;
    let object = value.as_object_mut().ok_or_else(|| {
        DataQualityCanonicalizationError(
            "Data Quality persisted state must be a top-level object".to_owned(),
        )
    })?;
    match object.remove(PROFILE_FIELD) {
        Some(Value::String(profile)) if profile == PROFILE_ID => {}
        Some(Value::String(profile)) => {
            return Err(DataQualityCanonicalizationError(format!(
                "Data Quality persisted state canonicalization profile is unsupported: {profile}"
            )));
        }
        Some(_) => {
            return Err(DataQualityCanonicalizationError(
                "Data Quality persisted state canonicalization profile must be a string".to_owned(),
            ));
        }
        None => {
            return Err(DataQualityCanonicalizationError(
                "Data Quality persisted state canonicalization profile is missing".to_owned(),
            ));
        }
    }
    serde_json::from_value(value)
        .map_err(|error| DataQualityCanonicalizationError(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Fixture {
        value: u32,
    }

    #[test]
    fn persisted_state_binds_and_validates_exact_profile() {
        let encoded = state_to_vec(&Fixture { value: 7 }).unwrap();
        assert_eq!(
            encoded,
            br#"{"canonicalization_profile":"crm.cjson/v1","value":7}"#
        );
        assert_eq!(
            state_from_slice::<Fixture>(&encoded).unwrap(),
            Fixture { value: 7 }
        );

        let wrong = String::from_utf8(encoded)
            .unwrap()
            .replace("crm.cjson/v1", "crm.cjson/v2");
        assert!(state_from_slice::<Fixture>(wrong.as_bytes()).is_err());
    }
}
