use crate::domain::{
    ContactPoint, ContactPointId, ContactPointKind, ContactPointSnapshot, ContactPointStatus,
    PartyReference, VerificationEvidence, VerificationState,
};
use crm_module_sdk::{ErrorCategory, SdkError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const CONTACT_POINT_STATE_SCHEMA_ID: &str = "crm.contact-points.contact-point.state";
pub const CONTACT_POINT_STATE_SCHEMA_VERSION: &str = "1.0.0";
pub const CONTACT_POINT_STATE_MAXIMUM_BYTES: u64 = 128 * 1024;
pub const CONTACT_POINT_STATE_RETENTION_POLICY_ID: &str = "crm.contact-points.business_record";
const CONTACT_POINT_STATE_DESCRIPTOR: &[u8] = b"crm.contact-points.contact-point.state/v1:contact_point_id,party_id,kind,normalized_value,display_value,status,preferred,valid_from_unix_nanos,valid_until_unix_nanos,verification[state,evidence_ref,verified_at_unix_nanos],created_at_unix_nanos,updated_at_unix_nanos,version";

pub fn contact_point_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(CONTACT_POINT_STATE_DESCRIPTOR).into()
}

pub fn encode_contact_point_state(contact_point: &ContactPoint) -> Result<Vec<u8>, SdkError> {
    let bytes = serde_json::to_vec(&ContactPointStateV1::from(contact_point.snapshot())).map_err(
        |error| persisted_error(format!("Contact Point state serialization failed: {error}")),
    )?;
    validate_size(&bytes)?;
    Ok(bytes)
}

pub fn decode_contact_point_state(bytes: &[u8]) -> Result<ContactPoint, SdkError> {
    validate_size(bytes)?;
    let state: ContactPointStateV1 = serde_json::from_slice(bytes)
        .map_err(|error| persisted_error(format!("Contact Point state JSON is invalid: {error}")))?;
    ContactPoint::rehydrate(state.try_into()?)
        .map_err(|error| persisted_error(format!("{}: {}", error.code, error.safe_message)))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContactPointStateV1 {
    contact_point_id: String,
    party_id: String,
    kind: ContactPointKindState,
    normalized_value: String,
    display_value: String,
    status: ContactPointStatusState,
    preferred: bool,
    valid_from_unix_nanos: Option<i64>,
    valid_until_unix_nanos: Option<i64>,
    verification: VerificationStateV1,
    created_at_unix_nanos: i64,
    updated_at_unix_nanos: i64,
    version: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ContactPointKindState {
    Email,
    Phone,
    Postal,
    Web,
    Messaging,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ContactPointStatusState {
    Active,
    Inactive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
enum VerificationStateV1 {
    Unverified,
    Verified {
        evidence_ref: String,
        verified_at_unix_nanos: i64,
    },
}

impl From<ContactPointSnapshot> for ContactPointStateV1 {
    fn from(value: ContactPointSnapshot) -> Self {
        Self {
            contact_point_id: value.contact_point_id.as_str().to_owned(),
            party_id: value.party_ref.as_str().to_owned(),
            kind: value.kind.into(),
            normalized_value: value.normalized_value,
            display_value: value.display_value,
            status: value.status.into(),
            preferred: value.preferred,
            valid_from_unix_nanos: value.valid_from_unix_nanos,
            valid_until_unix_nanos: value.valid_until_unix_nanos,
            verification: value.verification.into(),
            created_at_unix_nanos: value.created_at_unix_nanos,
            updated_at_unix_nanos: value.updated_at_unix_nanos,
            version: value.version,
        }
    }
}

impl TryFrom<ContactPointStateV1> for ContactPointSnapshot {
    type Error = SdkError;

    fn try_from(value: ContactPointStateV1) -> Result<Self, Self::Error> {
        Ok(Self {
            contact_point_id: ContactPointId::try_new(value.contact_point_id)
                .map_err(|error| persisted_error(error.to_string()))?,
            party_ref: PartyReference::try_new(value.party_id)
                .map_err(|error| persisted_error(error.to_string()))?,
            kind: value.kind.into(),
            normalized_value: value.normalized_value,
            display_value: value.display_value,
            status: value.status.into(),
            preferred: value.preferred,
            valid_from_unix_nanos: value.valid_from_unix_nanos,
            valid_until_unix_nanos: value.valid_until_unix_nanos,
            verification: value.verification.into(),
            created_at_unix_nanos: value.created_at_unix_nanos,
            updated_at_unix_nanos: value.updated_at_unix_nanos,
            version: value.version,
        })
    }
}

impl From<ContactPointKind> for ContactPointKindState {
    fn from(value: ContactPointKind) -> Self {
        match value {
            ContactPointKind::Email => Self::Email,
            ContactPointKind::Phone => Self::Phone,
            ContactPointKind::Postal => Self::Postal,
            ContactPointKind::Web => Self::Web,
            ContactPointKind::Messaging => Self::Messaging,
        }
    }
}

impl From<ContactPointKindState> for ContactPointKind {
    fn from(value: ContactPointKindState) -> Self {
        match value {
            ContactPointKindState::Email => Self::Email,
            ContactPointKindState::Phone => Self::Phone,
            ContactPointKindState::Postal => Self::Postal,
            ContactPointKindState::Web => Self::Web,
            ContactPointKindState::Messaging => Self::Messaging,
        }
    }
}

impl From<ContactPointStatus> for ContactPointStatusState {
    fn from(value: ContactPointStatus) -> Self {
        match value {
            ContactPointStatus::Active => Self::Active,
            ContactPointStatus::Inactive => Self::Inactive,
        }
    }
}

impl From<ContactPointStatusState> for ContactPointStatus {
    fn from(value: ContactPointStatusState) -> Self {
        match value {
            ContactPointStatusState::Active => Self::Active,
            ContactPointStatusState::Inactive => Self::Inactive,
        }
    }
}

impl From<VerificationState> for VerificationStateV1 {
    fn from(value: VerificationState) -> Self {
        match value {
            VerificationState::Unverified => Self::Unverified,
            VerificationState::Verified(evidence) => Self::Verified {
                evidence_ref: evidence.evidence_ref().to_owned(),
                verified_at_unix_nanos: evidence.verified_at_unix_nanos(),
            },
        }
    }
}

impl From<VerificationStateV1> for VerificationState {
    fn from(value: VerificationStateV1) -> Self {
        match value {
            VerificationStateV1::Unverified => Self::Unverified,
            VerificationStateV1::Verified {
                evidence_ref,
                verified_at_unix_nanos,
            } => Self::Verified(VerificationEvidence::from_persisted(
                evidence_ref,
                verified_at_unix_nanos,
            )),
        }
    }
}

fn validate_size(bytes: &[u8]) -> Result<(), SdkError> {
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > CONTACT_POINT_STATE_MAXIMUM_BYTES {
        return Err(persisted_error(format!(
            "Contact Point state exceeds the maximum of {CONTACT_POINT_STATE_MAXIMUM_BYTES} bytes"
        )));
    }
    Ok(())
}

fn persisted_error(message: impl Into<String>) -> SdkError {
    SdkError::new(
        "CONTACT_POINTS_PERSISTED_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "The persisted Contact Point state is invalid.",
    )
    .with_internal_reference(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{CreateContactPoint, VerifyContactPoint};

    fn verified_contact_point() -> ContactPoint {
        let mut value = ContactPoint::create(CreateContactPoint {
            contact_point_id: ContactPointId::try_new("contact-point-persisted-1").unwrap(),
            party_ref: PartyReference::try_new("party-1").unwrap(),
            kind: ContactPointKind::Email,
            value: "Ada@EXAMPLE.COM".to_owned(),
            preferred: true,
            valid_from_unix_nanos: Some(10),
            valid_until_unix_nanos: Some(1_000),
            occurred_at_unix_nanos: 10,
        })
        .unwrap();
        value
            .verify(VerifyContactPoint {
                expected_version: 1,
                evidence_ref: "verification-42".to_owned(),
                occurred_at_unix_nanos: 42,
            })
            .unwrap();
        value
    }

    #[test]
    fn round_trip_preserves_exact_state_schema_hash_and_deterministic_bytes() {
        let value = verified_contact_point();
        let first = encode_contact_point_state(&value).unwrap();
        let second = encode_contact_point_state(&value).unwrap();
        let decoded = decode_contact_point_state(&first).unwrap();

        assert_eq!(decoded, value);
        assert_eq!(first, second);
        assert_eq!(
            std::str::from_utf8(&first).unwrap(),
            r#"{"contact_point_id":"contact-point-persisted-1","party_id":"party-1","kind":"email","normalized_value":"Ada@example.com","display_value":"Ada@EXAMPLE.COM","status":"active","preferred":true,"valid_from_unix_nanos":10,"valid_until_unix_nanos":1000,"verification":{"state":"verified","evidence_ref":"verification-42","verified_at_unix_nanos":42},"created_at_unix_nanos":10,"updated_at_unix_nanos":42,"version":2}"#
        );
        assert_ne!(contact_point_state_descriptor_hash(), [0; 32]);
    }

    #[test]
    fn rejects_unknown_fields_noncanonical_values_and_impossible_verification_time() {
        let unknown = br#"{"contact_point_id":"contact-point-1","party_id":"party-1","kind":"email","normalized_value":"ada@example.com","display_value":"ada@example.com","status":"active","preferred":false,"valid_from_unix_nanos":null,"valid_until_unix_nanos":null,"verification":{"state":"unverified"},"created_at_unix_nanos":1,"updated_at_unix_nanos":1,"version":1,"unexpected":true}"#;
        assert_eq!(
            decode_contact_point_state(unknown).unwrap_err().code,
            "CONTACT_POINTS_PERSISTED_STATE_INVALID"
        );

        let noncanonical = br#"{"contact_point_id":"contact-point-1","party_id":"party-1","kind":"email","normalized_value":"ada@example.com","display_value":"  ada@example.com  ","status":"active","preferred":false,"valid_from_unix_nanos":null,"valid_until_unix_nanos":null,"verification":{"state":"unverified"},"created_at_unix_nanos":1,"updated_at_unix_nanos":1,"version":1}"#;
        assert_eq!(
            decode_contact_point_state(noncanonical).unwrap_err().code,
            "CONTACT_POINTS_PERSISTED_STATE_INVALID"
        );

        let impossible_verification = br#"{"contact_point_id":"contact-point-1","party_id":"party-1","kind":"email","normalized_value":"ada@example.com","display_value":"ada@example.com","status":"active","preferred":false,"valid_from_unix_nanos":null,"valid_until_unix_nanos":null,"verification":{"state":"verified","evidence_ref":"evidence-1","verified_at_unix_nanos":9},"created_at_unix_nanos":1,"updated_at_unix_nanos":8,"version":2}"#;
        assert_eq!(
            decode_contact_point_state(impossible_verification)
                .unwrap_err()
                .code,
            "CONTACT_POINTS_PERSISTED_STATE_INVALID"
        );
    }
}
