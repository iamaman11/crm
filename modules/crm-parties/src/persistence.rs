use crate::domain::{
    MergeLineageReference, Party, PartyId, PartyKind, PartyLifecycle, PartySnapshot,
};
use crm_module_sdk::{ErrorCategory, SdkError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PARTY_STATE_SCHEMA_ID: &str = "crm.parties.party.state";
pub const PARTY_STATE_SCHEMA_VERSION: &str = "2.0.0";
pub const PARTY_STATE_V1_SCHEMA_VERSION: &str = "1.0.0";
pub const PARTY_STATE_MAXIMUM_BYTES: u64 = 64 * 1024;
pub const PARTY_STATE_RETENTION_POLICY_ID: &str = "crm.parties.business_record";
const PARTY_STATE_V1_DESCRIPTOR: &[u8] =
    b"crm.parties.party.state/v1:party_id,kind,display_name,created_at_unix_nanos,updated_at_unix_nanos,version";
const PARTY_STATE_V2_DESCRIPTOR: &[u8] = b"crm.parties.party.state/v2:party_id,kind,display_name,lifecycle[status,survivor_party_id,merge_lineage_id],created_at_unix_nanos,updated_at_unix_nanos,version";

pub fn party_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(PARTY_STATE_V2_DESCRIPTOR).into()
}

pub fn party_state_v1_descriptor_hash() -> [u8; 32] {
    Sha256::digest(PARTY_STATE_V1_DESCRIPTOR).into()
}

pub fn encode_party_state(party: &Party) -> Result<Vec<u8>, SdkError> {
    let bytes = serde_json::to_vec(&PartyStateV2::from(party.snapshot()))
        .map_err(|error| persisted_error(format!("Party state serialization failed: {error}")))?;
    validate_size(&bytes)?;
    Ok(bytes)
}

pub fn decode_party_state(bytes: &[u8]) -> Result<Party, SdkError> {
    validate_size(bytes)?;

    if let Ok(state) = serde_json::from_slice::<PartyStateV2>(bytes) {
        let party = Party::rehydrate(state.try_into()?)
            .map_err(|error| persisted_error(format!("{}: {}", error.code, error.safe_message)))?;
        let canonical = encode_party_state(&party)?;
        if canonical != bytes {
            return Err(persisted_error(
                "persisted Party v2 representation is not canonical",
            ));
        }
        return Ok(party);
    }

    let state: PartyStateV1 = serde_json::from_slice(bytes)
        .map_err(|error| persisted_error(format!("Party state JSON is invalid: {error}")))?;
    let party = Party::rehydrate(state.try_into()?)
        .map_err(|error| persisted_error(format!("{}: {}", error.code, error.safe_message)))?;
    let canonical = serde_json::to_vec(&PartyStateV1::from(party.snapshot())).map_err(|error| {
        persisted_error(format!(
            "legacy Party state canonical serialization failed: {error}"
        ))
    })?;
    if canonical != bytes {
        return Err(persisted_error(
            "persisted Party v1 representation is not canonical",
        ));
    }
    Ok(party)
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartyStateV2 {
    party_id: String,
    kind: PartyKindState,
    display_name: String,
    lifecycle: PartyLifecycleStateV2,
    created_at_unix_nanos: i64,
    updated_at_unix_nanos: i64,
    version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
enum PartyLifecycleStateV2 {
    Active,
    Merged {
        survivor_party_id: String,
        merge_lineage_id: String,
    },
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
            party_id: canonical_party_id(value.party_id)?,
            kind: value.kind.into(),
            display_name: value.display_name,
            lifecycle: PartyLifecycle::Active,
            created_at_unix_nanos: value.created_at_unix_nanos,
            updated_at_unix_nanos: value.updated_at_unix_nanos,
            version: value.version,
        })
    }
}

impl From<PartySnapshot> for PartyStateV2 {
    fn from(value: PartySnapshot) -> Self {
        Self {
            party_id: value.party_id.as_str().to_owned(),
            kind: value.kind.into(),
            display_name: value.display_name,
            lifecycle: value.lifecycle.into(),
            created_at_unix_nanos: value.created_at_unix_nanos,
            updated_at_unix_nanos: value.updated_at_unix_nanos,
            version: value.version,
        }
    }
}

impl TryFrom<PartyStateV2> for PartySnapshot {
    type Error = SdkError;

    fn try_from(value: PartyStateV2) -> Result<Self, Self::Error> {
        Ok(Self {
            party_id: canonical_party_id(value.party_id)?,
            kind: value.kind.into(),
            display_name: value.display_name,
            lifecycle: value.lifecycle.try_into()?,
            created_at_unix_nanos: value.created_at_unix_nanos,
            updated_at_unix_nanos: value.updated_at_unix_nanos,
            version: value.version,
        })
    }
}

impl From<PartyLifecycle> for PartyLifecycleStateV2 {
    fn from(value: PartyLifecycle) -> Self {
        match value {
            PartyLifecycle::Active => Self::Active,
            PartyLifecycle::Merged {
                survivor_party_id,
                merge_lineage_ref,
            } => Self::Merged {
                survivor_party_id: survivor_party_id.as_str().to_owned(),
                merge_lineage_id: merge_lineage_ref.as_str().to_owned(),
            },
        }
    }
}

impl TryFrom<PartyLifecycleStateV2> for PartyLifecycle {
    type Error = SdkError;

    fn try_from(value: PartyLifecycleStateV2) -> Result<Self, Self::Error> {
        match value {
            PartyLifecycleStateV2::Active => Ok(Self::Active),
            PartyLifecycleStateV2::Merged {
                survivor_party_id,
                merge_lineage_id,
            } => Ok(Self::Merged {
                survivor_party_id: canonical_party_id(survivor_party_id)?,
                merge_lineage_ref: canonical_merge_lineage_ref(merge_lineage_id)?,
            }),
        }
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

fn canonical_party_id(raw: String) -> Result<PartyId, SdkError> {
    let parsed =
        PartyId::try_new(raw.clone()).map_err(|error| persisted_error(error.to_string()))?;
    if parsed.as_str() != raw {
        return Err(persisted_error("persisted Party ID is not canonical"));
    }
    Ok(parsed)
}

fn canonical_merge_lineage_ref(raw: String) -> Result<MergeLineageReference, SdkError> {
    let parsed = MergeLineageReference::try_new(raw.clone())
        .map_err(|error| persisted_error(error.to_string()))?;
    if parsed.as_str() != raw {
        return Err(persisted_error(
            "persisted merge-lineage reference is not canonical",
        ));
    }
    Ok(parsed)
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
    use crate::domain::{CreateParty, MarkPartyMerged};

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
    fn v2_round_trip_preserves_exact_active_state_and_schema_hash() {
        let value = party();
        let encoded = encode_party_state(&value).unwrap();
        let decoded = decode_party_state(&encoded).unwrap();

        assert_eq!(decoded, value);
        assert_eq!(decoded.lifecycle(), &PartyLifecycle::Active);
        assert_ne!(party_state_descriptor_hash(), [0; 32]);
        assert_ne!(
            party_state_descriptor_hash(),
            party_state_v1_descriptor_hash()
        );
    }

    #[test]
    fn v2_round_trip_preserves_merge_redirect_state() {
        let mut value = party();
        value
            .mark_merged(MarkPartyMerged {
                expected_version: 1,
                survivor_party_id: PartyId::try_new("party-survivor").unwrap(),
                merge_lineage_ref: MergeLineageReference::try_new(
                    "idrm-0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                )
                .unwrap(),
                occurred_at_unix_nanos: 20,
            })
            .unwrap();
        let encoded = encode_party_state(&value).unwrap();
        assert_eq!(decode_party_state(&encoded).unwrap(), value);
    }

    #[test]
    fn legacy_v1_state_decodes_as_active_without_mutating_the_published_contract() {
        let legacy = br#"{"party_id":"party-legacy-1","kind":"person","display_name":"Ada Lovelace","created_at_unix_nanos":1,"updated_at_unix_nanos":1,"version":1}"#;
        let decoded = decode_party_state(legacy).unwrap();
        assert_eq!(decoded.party_id().as_str(), "party-legacy-1");
        assert_eq!(decoded.lifecycle(), &PartyLifecycle::Active);
        assert_eq!(PARTY_STATE_V1_SCHEMA_VERSION, "1.0.0");
    }

    #[test]
    fn rejects_unknown_persisted_fields_and_invalid_domain_state() {
        let unknown = br#"{"party_id":"party-1","kind":"person","display_name":"Ada","lifecycle":{"status":"active"},"created_at_unix_nanos":1,"updated_at_unix_nanos":1,"version":1,"unexpected":true}"#;
        assert_eq!(
            decode_party_state(unknown).unwrap_err().code,
            "PARTIES_PARTY_PERSISTED_STATE_INVALID"
        );

        let invalid_version = br#"{"party_id":"party-1","kind":"person","display_name":"Ada","lifecycle":{"status":"active"},"created_at_unix_nanos":1,"updated_at_unix_nanos":1,"version":0}"#;
        assert_eq!(
            decode_party_state(invalid_version).unwrap_err().code,
            "PARTIES_PARTY_PERSISTED_STATE_INVALID"
        );
    }
}
