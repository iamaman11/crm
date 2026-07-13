use crate::domain::{Party, PartyId, PartyKind, PartySnapshot};
use crm_module_sdk::{ErrorCategory, SdkError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PARTY_STATE_SCHEMA_ID: &str = "crm.parties.party.state";
pub const PARTY_STATE_SCHEMA_VERSION: &str = "1.0.0";
pub const PARTY_STATE_MAXIMUM_BYTES: u64 = 64 * 1024;
pub const PARTY_STATE_RETENTION_POLICY_ID: &str = "crm.parties.business_record";
const PARTY_STATE_DESCRIPTOR: &[u8] = b"crm.parties.party.state/v1:party_id,kind,display_name,created_at_unix_nanos,updated_at_unix_nanos,version";

pub fn party_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(PARTY_STATE_DESCRIPTOR).into()
}

pub fn encode_party_state(party: &Party) -> Result<Vec<u8>, SdkError> {
    let bytes = serde_json::to_vec(&PartyStateV1::from(party.snapshot()))
        .map_err(|error| persisted_error(format!("Party state serialization failed: {error}")))?;
    validate_size(&bytes)?;
    Ok(bytes)
}

pub fn decode_party_state(bytes: &[u8]) -> Result<Party, SdkError> {
    validate_size(bytes)?;
    let state: PartyStateV1 = serde_json::from_slice(bytes)
        .map_err(|error| persisted_error(format!("Party state JSON is invalid: {error}")))?;
    Party::rehydrate(state.try_into()?)
        .map_err(|error| persisted_error(format!("{}: {}", error.code, error.safe_message)))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartyStateV1 {
    party_id: String,
    kind: PartyKindState,
    display_name: String,
    created_at_unix_nanos: i64,
    updated_at_unix_nanos: i64,
    version: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PartyKindState {
    Person,
    Organization,
}

impl From<PartySnapshot> for PartyStateV1 {
    fn from(value: PartySnapshot) -> Self {
        Self {
            party_id: value.party_id.as_str().to_owned(),
            kind: value.kind.into(),
            display_name: value.display_name,
            created_at_unix_nanos: value.created_at_unix_nanos,
            updated_at_unix_nanos: value.updated_at_unix_nanos,
            version: value.version,
        }
    }
}

impl TryFrom<PartyStateV1> for PartySnapshot {
    type Error = SdkError;

    fn try_from(value: PartyStateV1) -> Result<Self, Self::Error> {
        Ok(Self {
            party_id: PartyId::try_new(value.party_id)
                .map_err(|error| persisted_error(error.to_string()))?,
            kind: value.kind.into(),
            display_name: value.display_name,
            created_at_unix_nanos: value.created_at_unix_nanos,
            updated_at_unix_nanos: value.updated_at_unix_nanos,
            version: value.version,
        })
    }
}

impl From<PartyKind> for PartyKindState {
    fn from(value: PartyKind) -> Self {
        match value {
            PartyKind::Person => Self::Person,
            PartyKind::Organization => Self::Organization,
        }
    }
}

impl From<PartyKindState> for PartyKind {
    fn from(value: PartyKindState) -> Self {
        match value {
            PartyKindState::Person => Self::Person,
            PartyKindState::Organization => Self::Organization,
        }
    }
}

fn validate_size(bytes: &[u8]) -> Result<(), SdkError> {
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > PARTY_STATE_MAXIMUM_BYTES {
        return Err(persisted_error(format!(
            "Party state exceeds the maximum of {PARTY_STATE_MAXIMUM_BYTES} bytes"
        )));
    }
    Ok(())
}

fn persisted_error(message: impl Into<String>) -> SdkError {
    SdkError::new(
        "PARTIES_PARTY_PERSISTED_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "The persisted Party state is invalid.",
    )
    .with_internal_reference(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::CreateParty;

    fn party() -> Party {
        Party::create(CreateParty {
            party_id: PartyId::try_new("party-persisted-1").unwrap(),
            kind: PartyKind::Organization,
            display_name: "Northwind Holdings".to_owned(),
            occurred_at_unix_nanos: 10,
        })
        .unwrap()
    }

    #[test]
    fn round_trip_preserves_exact_state_and_schema_hash() {
        let value = party();
        let encoded = encode_party_state(&value).unwrap();
        let decoded = decode_party_state(&encoded).unwrap();

        assert_eq!(decoded, value);
        assert_eq!(party_state_descriptor_hash(), party_state_descriptor_hash());
        assert_ne!(party_state_descriptor_hash(), [0; 32]);
    }

    #[test]
    fn rejects_unknown_persisted_fields_and_invalid_domain_state() {
        let unknown = br#"{"party_id":"party-1","kind":"person","display_name":"Ada","created_at_unix_nanos":1,"updated_at_unix_nanos":1,"version":1,"unexpected":true}"#;
        assert_eq!(
            decode_party_state(unknown).unwrap_err().code,
            "PARTIES_PARTY_PERSISTED_STATE_INVALID"
        );

        let invalid_version = br#"{"party_id":"party-1","kind":"person","display_name":"Ada","created_at_unix_nanos":1,"updated_at_unix_nanos":1,"version":0}"#;
        assert_eq!(
            decode_party_state(invalid_version).unwrap_err().code,
            "PARTIES_PARTY_PERSISTED_STATE_INVALID"
        );
    }
}
