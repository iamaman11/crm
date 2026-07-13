use crate::domain::{
    PartyReference, PartyRelationship, PartyRelationshipId, PartyRelationshipSnapshot,
    PartyRelationshipStatus, RelationshipDirectionality, RelationshipType,
};
use crm_module_sdk::{ErrorCategory, SdkError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PARTY_RELATIONSHIP_STATE_SCHEMA_ID: &str =
    "crm.party-relationships.party-relationship.state";
pub const PARTY_RELATIONSHIP_STATE_SCHEMA_VERSION: &str = "1.0.0";
pub const PARTY_RELATIONSHIP_STATE_MAXIMUM_BYTES: u64 = 128 * 1024;
pub const PARTY_RELATIONSHIP_STATE_RETENTION_POLICY_ID: &str =
    "crm.party-relationships.business_record";
const PARTY_RELATIONSHIP_STATE_DESCRIPTOR: &[u8] = b"crm.party-relationships.party-relationship.state/v1:party_relationship_id,from_party_id,to_party_id,relationship_type[code,directionality,from_role,to_role],status,valid_from_unix_nanos,valid_until_unix_nanos,created_at_unix_nanos,updated_at_unix_nanos,version";

pub fn party_relationship_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(PARTY_RELATIONSHIP_STATE_DESCRIPTOR).into()
}

pub fn encode_party_relationship_state(
    party_relationship: &PartyRelationship,
) -> Result<Vec<u8>, SdkError> {
    let bytes = serde_json::to_vec(&PartyRelationshipStateV1::from(
        party_relationship.snapshot(),
    ))
    .map_err(|error| {
        persisted_error(format!(
            "Party Relationship state serialization failed: {error}"
        ))
    })?;
    validate_size(&bytes)?;
    Ok(bytes)
}

pub fn decode_party_relationship_state(bytes: &[u8]) -> Result<PartyRelationship, SdkError> {
    validate_size(bytes)?;
    let state: PartyRelationshipStateV1 = serde_json::from_slice(bytes).map_err(|error| {
        persisted_error(format!(
            "Party Relationship state JSON is invalid: {error}"
        ))
    })?;
    PartyRelationship::rehydrate(state.try_into()?)
        .map_err(|error| persisted_error(format!("{}: {}", error.code, error.safe_message)))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartyRelationshipStateV1 {
    party_relationship_id: String,
    from_party_id: String,
    to_party_id: String,
    relationship_type: RelationshipTypeStateV1,
    status: PartyRelationshipStatusState,
    valid_from_unix_nanos: Option<i64>,
    valid_until_unix_nanos: Option<i64>,
    created_at_unix_nanos: i64,
    updated_at_unix_nanos: i64,
    version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RelationshipTypeStateV1 {
    code: String,
    directionality: RelationshipDirectionalityState,
    from_role: String,
    to_role: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RelationshipDirectionalityState {
    Directional,
    Reciprocal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PartyRelationshipStatusState {
    Active,
    Inactive,
}

impl From<PartyRelationshipSnapshot> for PartyRelationshipStateV1 {
    fn from(value: PartyRelationshipSnapshot) -> Self {
        Self {
            party_relationship_id: value.party_relationship_id.as_str().to_owned(),
            from_party_id: value.from_party_ref.as_str().to_owned(),
            to_party_id: value.to_party_ref.as_str().to_owned(),
            relationship_type: value.relationship_type.into(),
            status: value.status.into(),
            valid_from_unix_nanos: value.valid_from_unix_nanos,
            valid_until_unix_nanos: value.valid_until_unix_nanos,
            created_at_unix_nanos: value.created_at_unix_nanos,
            updated_at_unix_nanos: value.updated_at_unix_nanos,
            version: value.version,
        }
    }
}

impl TryFrom<PartyRelationshipStateV1> for PartyRelationshipSnapshot {
    type Error = SdkError;

    fn try_from(value: PartyRelationshipStateV1) -> Result<Self, Self::Error> {
        Ok(Self {
            party_relationship_id: PartyRelationshipId::try_new(value.party_relationship_id)
                .map_err(|error| persisted_error(error.to_string()))?,
            from_party_ref: PartyReference::try_new(value.from_party_id)
                .map_err(|error| persisted_error(error.to_string()))?,
            to_party_ref: PartyReference::try_new(value.to_party_id)
                .map_err(|error| persisted_error(error.to_string()))?,
            relationship_type: value.relationship_type.try_into()?,
            status: value.status.into(),
            valid_from_unix_nanos: value.valid_from_unix_nanos,
            valid_until_unix_nanos: value.valid_until_unix_nanos,
            created_at_unix_nanos: value.created_at_unix_nanos,
            updated_at_unix_nanos: value.updated_at_unix_nanos,
            version: value.version,
        })
    }
}

impl From<RelationshipType> for RelationshipTypeStateV1 {
    fn from(value: RelationshipType) -> Self {
        Self {
            code: value.code().to_owned(),
            directionality: value.directionality().into(),
            from_role: value.from_role().to_owned(),
            to_role: value.to_role().to_owned(),
        }
    }
}

impl TryFrom<RelationshipTypeStateV1> for RelationshipType {
    type Error = SdkError;

    fn try_from(value: RelationshipTypeStateV1) -> Result<Self, Self::Error> {
        let directionality: RelationshipDirectionality = value.directionality.into();
        let relationship_type = RelationshipType::try_new(
            value.code.clone(),
            directionality,
            value.from_role.clone(),
            value.to_role.clone(),
        )
        .map_err(|error| persisted_error(error.to_string()))?;
        if relationship_type.code() != value.code
            || relationship_type.from_role() != value.from_role
            || relationship_type.to_role() != value.to_role
            || relationship_type.directionality() != directionality
        {
            return Err(persisted_error(
                "persisted relationship type semantics are not canonical",
            ));
        }
        Ok(relationship_type)
    }
}

impl From<RelationshipDirectionality> for RelationshipDirectionalityState {
    fn from(value: RelationshipDirectionality) -> Self {
        match value {
            RelationshipDirectionality::Directional => Self::Directional,
            RelationshipDirectionality::Reciprocal => Self::Reciprocal,
        }
    }
}

impl From<RelationshipDirectionalityState> for RelationshipDirectionality {
    fn from(value: RelationshipDirectionalityState) -> Self {
        match value {
            RelationshipDirectionalityState::Directional => Self::Directional,
            RelationshipDirectionalityState::Reciprocal => Self::Reciprocal,
        }
    }
}

impl From<PartyRelationshipStatus> for PartyRelationshipStatusState {
    fn from(value: PartyRelationshipStatus) -> Self {
        match value {
            PartyRelationshipStatus::Active => Self::Active,
            PartyRelationshipStatus::Inactive => Self::Inactive,
        }
    }
}

impl From<PartyRelationshipStatusState> for PartyRelationshipStatus {
    fn from(value: PartyRelationshipStatusState) -> Self {
        match value {
            PartyRelationshipStatusState::Active => Self::Active,
            PartyRelationshipStatusState::Inactive => Self::Inactive,
        }
    }
}

fn validate_size(bytes: &[u8]) -> Result<(), SdkError> {
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > PARTY_RELATIONSHIP_STATE_MAXIMUM_BYTES {
        return Err(persisted_error(format!(
            "Party Relationship state exceeds the maximum of {PARTY_RELATIONSHIP_STATE_MAXIMUM_BYTES} bytes"
        )));
    }
    Ok(())
}

fn persisted_error(message: impl Into<String>) -> SdkError {
    SdkError::new(
        "PARTY_RELATIONSHIPS_PERSISTED_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "The persisted Party Relationship state is invalid.",
    )
    .with_internal_reference(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{CreatePartyRelationship, RelationshipType};

    fn directional_relationship() -> PartyRelationship {
        PartyRelationship::create(CreatePartyRelationship {
            party_relationship_id: PartyRelationshipId::try_new("party-relationship-persisted-1")
                .unwrap(),
            from_party_ref: PartyReference::try_new("party-organization-acme").unwrap(),
            to_party_ref: PartyReference::try_new("party-person-ada").unwrap(),
            relationship_type: RelationshipType::employment(),
            valid_from_unix_nanos: Some(10),
            valid_until_unix_nanos: Some(1_000),
            occurred_at_unix_nanos: 20,
        })
        .unwrap()
    }

    #[test]
    fn round_trip_preserves_exact_state_schema_hash_and_deterministic_bytes() {
        let value = directional_relationship();
        let first = encode_party_relationship_state(&value).unwrap();
        let second = encode_party_relationship_state(&value).unwrap();
        let decoded = decode_party_relationship_state(&first).unwrap();

        assert_eq!(decoded, value);
        assert_eq!(first, second);
        assert_eq!(
            std::str::from_utf8(&first).unwrap(),
            r#"{"party_relationship_id":"party-relationship-persisted-1","from_party_id":"party-organization-acme","to_party_id":"party-person-ada","relationship_type":{"code":"employment","directionality":"directional","from_role":"employer","to_role":"employee"},"status":"active","valid_from_unix_nanos":10,"valid_until_unix_nanos":1000,"created_at_unix_nanos":20,"updated_at_unix_nanos":20,"version":1}"#
        );
        assert_ne!(party_relationship_state_descriptor_hash(), [0; 32]);
    }

    #[test]
    fn rejects_unknown_fields_and_noncanonical_relationship_type_semantics() {
        let unknown = br#"{"party_relationship_id":"relationship-1","from_party_id":"party-a","to_party_id":"party-b","relationship_type":{"code":"employment","directionality":"directional","from_role":"employer","to_role":"employee"},"status":"active","valid_from_unix_nanos":null,"valid_until_unix_nanos":null,"created_at_unix_nanos":1,"updated_at_unix_nanos":1,"version":1,"unexpected":true}"#;
        assert_eq!(
            decode_party_relationship_state(unknown).unwrap_err().code,
            "PARTY_RELATIONSHIPS_PERSISTED_STATE_INVALID"
        );

        let noncanonical = br#"{"party_relationship_id":"relationship-1","from_party_id":"party-a","to_party_id":"party-b","relationship_type":{"code":" Employment ","directionality":"directional","from_role":"Employer","to_role":"Employee"},"status":"active","valid_from_unix_nanos":null,"valid_until_unix_nanos":null,"created_at_unix_nanos":1,"updated_at_unix_nanos":1,"version":1}"#;
        assert_eq!(
            decode_party_relationship_state(noncanonical)
                .unwrap_err()
                .code,
            "PARTY_RELATIONSHIPS_PERSISTED_STATE_INVALID"
        );
    }

    #[test]
    fn rejects_noncanonical_reciprocal_endpoint_order_and_impossible_initial_state() {
        let noncanonical_order = br#"{"party_relationship_id":"relationship-1","from_party_id":"party-z","to_party_id":"party-a","relationship_type":{"code":"household","directionality":"reciprocal","from_role":"household_member","to_role":"household_member"},"status":"active","valid_from_unix_nanos":null,"valid_until_unix_nanos":null,"created_at_unix_nanos":1,"updated_at_unix_nanos":1,"version":1}"#;
        assert_eq!(
            decode_party_relationship_state(noncanonical_order)
                .unwrap_err()
                .code,
            "PARTY_RELATIONSHIPS_PERSISTED_STATE_INVALID"
        );

        let impossible_initial = br#"{"party_relationship_id":"relationship-1","from_party_id":"party-a","to_party_id":"party-b","relationship_type":{"code":"employment","directionality":"directional","from_role":"employer","to_role":"employee"},"status":"inactive","valid_from_unix_nanos":null,"valid_until_unix_nanos":null,"created_at_unix_nanos":1,"updated_at_unix_nanos":1,"version":1}"#;
        assert_eq!(
            decode_party_relationship_state(impossible_initial)
                .unwrap_err()
                .code,
            "PARTY_RELATIONSHIPS_PERSISTED_STATE_INVALID"
        );
    }
}
