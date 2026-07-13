use crate::domain::{
    CommunicationChannel, ConsentAuthorization, ConsentAuthorizationId,
    ConsentAuthorizationSnapshot, ConsentAuthorizationStatus, ConsentEffect, ContactPointReference,
    EvidenceReference, JurisdictionCode, LegalBasisCode, PartyReference, PurposeCode, SourceCode,
};
use crm_module_sdk::{ErrorCategory, SdkError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const CONSENT_AUTHORIZATION_STATE_SCHEMA_ID: &str = "crm.consents.authorization.state";
pub const CONSENT_AUTHORIZATION_STATE_SCHEMA_VERSION: &str = "1.0.0";
pub const CONSENT_AUTHORIZATION_STATE_MAXIMUM_BYTES: u64 = 128 * 1024;
pub const CONSENT_AUTHORIZATION_STATE_RETENTION_POLICY_ID: &str =
    "crm.consents.authorization_evidence";
const CONSENT_AUTHORIZATION_STATE_DESCRIPTOR: &[u8] = b"crm.consents.authorization.state/v1:authorization_id,party_id,contact_point_id,purpose,channel,effect,legal_basis,jurisdiction,source,evidence_ref,effective_from_unix_nanos,expires_at_unix_nanos,status,withdrawn_at_unix_nanos,created_at_unix_nanos,updated_at_unix_nanos,version";

pub fn consent_authorization_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(CONSENT_AUTHORIZATION_STATE_DESCRIPTOR).into()
}

pub fn encode_consent_authorization_state(
    authorization: &ConsentAuthorization,
) -> Result<Vec<u8>, SdkError> {
    let bytes = serde_json::to_vec(&ConsentAuthorizationStateV1::from(authorization.snapshot()))
        .map_err(|error| {
            persisted_error(format!(
                "Consent Authorization state serialization failed: {error}"
            ))
        })?;
    validate_size(&bytes)?;
    Ok(bytes)
}

pub fn decode_consent_authorization_state(bytes: &[u8]) -> Result<ConsentAuthorization, SdkError> {
    validate_size(bytes)?;
    let state: ConsentAuthorizationStateV1 = serde_json::from_slice(bytes).map_err(|error| {
        persisted_error(format!(
            "Consent Authorization state JSON is invalid: {error}"
        ))
    })?;
    ConsentAuthorization::rehydrate(state.try_into()?)
        .map_err(|error| persisted_error(format!("{}: {}", error.code, error.safe_message)))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConsentAuthorizationStateV1 {
    authorization_id: String,
    party_id: String,
    contact_point_id: Option<String>,
    purpose: String,
    channel: CommunicationChannelState,
    effect: ConsentEffectState,
    legal_basis: String,
    jurisdiction: String,
    source: String,
    evidence_ref: String,
    effective_from_unix_nanos: i64,
    expires_at_unix_nanos: Option<i64>,
    status: ConsentAuthorizationStatusState,
    withdrawn_at_unix_nanos: Option<i64>,
    created_at_unix_nanos: i64,
    updated_at_unix_nanos: i64,
    version: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum CommunicationChannelState {
    Email,
    Phone,
    Sms,
    Postal,
    Messaging,
    Push,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ConsentEffectState {
    Grant,
    Deny,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ConsentAuthorizationStatusState {
    Active,
    Withdrawn,
}

impl From<ConsentAuthorizationSnapshot> for ConsentAuthorizationStateV1 {
    fn from(value: ConsentAuthorizationSnapshot) -> Self {
        Self {
            authorization_id: value.authorization_id.as_str().to_owned(),
            party_id: value.party_ref.as_str().to_owned(),
            contact_point_id: value
                .contact_point_ref
                .as_ref()
                .map(|reference| reference.as_str().to_owned()),
            purpose: value.purpose.as_str().to_owned(),
            channel: value.channel.into(),
            effect: value.effect.into(),
            legal_basis: value.legal_basis.as_str().to_owned(),
            jurisdiction: value.jurisdiction.as_str().to_owned(),
            source: value.source.as_str().to_owned(),
            evidence_ref: value.evidence_ref.as_str().to_owned(),
            effective_from_unix_nanos: value.effective_from_unix_nanos,
            expires_at_unix_nanos: value.expires_at_unix_nanos,
            status: value.status.into(),
            withdrawn_at_unix_nanos: value.withdrawn_at_unix_nanos,
            created_at_unix_nanos: value.created_at_unix_nanos,
            updated_at_unix_nanos: value.updated_at_unix_nanos,
            version: value.version,
        }
    }
}

impl TryFrom<ConsentAuthorizationStateV1> for ConsentAuthorizationSnapshot {
    type Error = SdkError;

    fn try_from(value: ConsentAuthorizationStateV1) -> Result<Self, Self::Error> {
        let purpose = PurposeCode::try_new(value.purpose.clone())
            .map_err(|error| persisted_error(error.to_string()))?;
        let legal_basis = LegalBasisCode::try_new(value.legal_basis.clone())
            .map_err(|error| persisted_error(error.to_string()))?;
        let jurisdiction = JurisdictionCode::try_new(value.jurisdiction.clone())
            .map_err(|error| persisted_error(error.to_string()))?;
        let source = SourceCode::try_new(value.source.clone())
            .map_err(|error| persisted_error(error.to_string()))?;
        let evidence_ref = EvidenceReference::try_new(value.evidence_ref.clone())
            .map_err(|error| persisted_error(error.to_string()))?;

        if purpose.as_str() != value.purpose
            || legal_basis.as_str() != value.legal_basis
            || jurisdiction.as_str() != value.jurisdiction
            || source.as_str() != value.source
            || evidence_ref.as_str() != value.evidence_ref
        {
            return Err(persisted_error(
                "persisted Consent Authorization semantic values are not canonical",
            ));
        }

        Ok(Self {
            authorization_id: ConsentAuthorizationId::try_new(value.authorization_id)
                .map_err(|error| persisted_error(error.to_string()))?,
            party_ref: PartyReference::try_new(value.party_id)
                .map_err(|error| persisted_error(error.to_string()))?,
            contact_point_ref: value
                .contact_point_id
                .map(ContactPointReference::try_new)
                .transpose()
                .map_err(|error| persisted_error(error.to_string()))?,
            purpose,
            channel: value.channel.into(),
            effect: value.effect.into(),
            legal_basis,
            jurisdiction,
            source,
            evidence_ref,
            effective_from_unix_nanos: value.effective_from_unix_nanos,
            expires_at_unix_nanos: value.expires_at_unix_nanos,
            status: value.status.into(),
            withdrawn_at_unix_nanos: value.withdrawn_at_unix_nanos,
            created_at_unix_nanos: value.created_at_unix_nanos,
            updated_at_unix_nanos: value.updated_at_unix_nanos,
            version: value.version,
        })
    }
}

impl From<CommunicationChannel> for CommunicationChannelState {
    fn from(value: CommunicationChannel) -> Self {
        match value {
            CommunicationChannel::Email => Self::Email,
            CommunicationChannel::Phone => Self::Phone,
            CommunicationChannel::Sms => Self::Sms,
            CommunicationChannel::Postal => Self::Postal,
            CommunicationChannel::Messaging => Self::Messaging,
            CommunicationChannel::Push => Self::Push,
        }
    }
}

impl From<CommunicationChannelState> for CommunicationChannel {
    fn from(value: CommunicationChannelState) -> Self {
        match value {
            CommunicationChannelState::Email => Self::Email,
            CommunicationChannelState::Phone => Self::Phone,
            CommunicationChannelState::Sms => Self::Sms,
            CommunicationChannelState::Postal => Self::Postal,
            CommunicationChannelState::Messaging => Self::Messaging,
            CommunicationChannelState::Push => Self::Push,
        }
    }
}

impl From<ConsentEffect> for ConsentEffectState {
    fn from(value: ConsentEffect) -> Self {
        match value {
            ConsentEffect::Grant => Self::Grant,
            ConsentEffect::Deny => Self::Deny,
        }
    }
}

impl From<ConsentEffectState> for ConsentEffect {
    fn from(value: ConsentEffectState) -> Self {
        match value {
            ConsentEffectState::Grant => Self::Grant,
            ConsentEffectState::Deny => Self::Deny,
        }
    }
}

impl From<ConsentAuthorizationStatus> for ConsentAuthorizationStatusState {
    fn from(value: ConsentAuthorizationStatus) -> Self {
        match value {
            ConsentAuthorizationStatus::Active => Self::Active,
            ConsentAuthorizationStatus::Withdrawn => Self::Withdrawn,
        }
    }
}

impl From<ConsentAuthorizationStatusState> for ConsentAuthorizationStatus {
    fn from(value: ConsentAuthorizationStatusState) -> Self {
        match value {
            ConsentAuthorizationStatusState::Active => Self::Active,
            ConsentAuthorizationStatusState::Withdrawn => Self::Withdrawn,
        }
    }
}

fn validate_size(bytes: &[u8]) -> Result<(), SdkError> {
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > CONSENT_AUTHORIZATION_STATE_MAXIMUM_BYTES {
        return Err(persisted_error(format!(
            "Consent Authorization state exceeds the maximum of {CONSENT_AUTHORIZATION_STATE_MAXIMUM_BYTES} bytes"
        )));
    }
    Ok(())
}

fn persisted_error(message: impl Into<String>) -> SdkError {
    SdkError::new(
        "CONSENTS_PERSISTED_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "The persisted Consent Authorization state is invalid.",
    )
    .with_internal_reference(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{CreateConsentAuthorization, WithdrawConsentAuthorization};

    fn authorization() -> ConsentAuthorization {
        ConsentAuthorization::create(CreateConsentAuthorization {
            authorization_id: ConsentAuthorizationId::try_new("consent-auth-persisted-1").unwrap(),
            party_ref: PartyReference::try_new("party-ada").unwrap(),
            contact_point_ref: Some(
                ContactPointReference::try_new("contact-point-email-1").unwrap(),
            ),
            purpose: PurposeCode::try_new("marketing.newsletter").unwrap(),
            channel: CommunicationChannel::Email,
            effect: ConsentEffect::Grant,
            legal_basis: LegalBasisCode::try_new("consent").unwrap(),
            jurisdiction: JurisdictionCode::try_new("eu-lt").unwrap(),
            source: SourceCode::try_new("web.form").unwrap(),
            evidence_ref: EvidenceReference::try_new("evidence://consent/1").unwrap(),
            effective_from_unix_nanos: 100,
            expires_at_unix_nanos: Some(1_000),
            occurred_at_unix_nanos: 100,
        })
        .unwrap()
    }

    #[test]
    fn round_trip_preserves_exact_state_schema_hash_and_deterministic_bytes() {
        let value = authorization();
        let first = encode_consent_authorization_state(&value).unwrap();
        let second = encode_consent_authorization_state(&value).unwrap();
        let decoded = decode_consent_authorization_state(&first).unwrap();

        assert_eq!(decoded, value);
        assert_eq!(first, second);
        assert_eq!(
            std::str::from_utf8(&first).unwrap(),
            r#"{"authorization_id":"consent-auth-persisted-1","party_id":"party-ada","contact_point_id":"contact-point-email-1","purpose":"marketing.newsletter","channel":"email","effect":"grant","legal_basis":"consent","jurisdiction":"eu-lt","source":"web.form","evidence_ref":"evidence://consent/1","effective_from_unix_nanos":100,"expires_at_unix_nanos":1000,"status":"active","withdrawn_at_unix_nanos":null,"created_at_unix_nanos":100,"updated_at_unix_nanos":100,"version":1}"#
        );
        assert_ne!(consent_authorization_state_descriptor_hash(), [0; 32]);
    }

    #[test]
    fn withdrawn_round_trip_preserves_irreversible_lifecycle_evidence() {
        let mut value = authorization();
        value
            .withdraw(WithdrawConsentAuthorization {
                expected_version: 1,
                occurred_at_unix_nanos: 200,
            })
            .unwrap();
        let decoded = decode_consent_authorization_state(
            &encode_consent_authorization_state(&value).unwrap(),
        )
        .unwrap();
        assert_eq!(decoded, value);
        assert_eq!(decoded.withdrawn_at_unix_nanos(), Some(200));
        assert_eq!(decoded.version(), 2);
    }

    #[test]
    fn rejects_unknown_fields_and_noncanonical_semantic_values() {
        let canonical = encode_consent_authorization_state(&authorization()).unwrap();
        let mut value: serde_json::Value = serde_json::from_slice(&canonical).unwrap();
        value["unexpected"] = serde_json::json!(true);
        assert_eq!(
            decode_consent_authorization_state(&serde_json::to_vec(&value).unwrap())
                .unwrap_err()
                .code,
            "CONSENTS_PERSISTED_STATE_INVALID"
        );

        let mut value: serde_json::Value = serde_json::from_slice(&canonical).unwrap();
        value["purpose"] = serde_json::json!(" Marketing.Newsletter ");
        assert_eq!(
            decode_consent_authorization_state(&serde_json::to_vec(&value).unwrap())
                .unwrap_err()
                .code,
            "CONSENTS_PERSISTED_STATE_INVALID"
        );
    }

    #[test]
    fn rejects_impossible_withdrawal_and_oversized_state() {
        let canonical = encode_consent_authorization_state(&authorization()).unwrap();
        let mut value: serde_json::Value = serde_json::from_slice(&canonical).unwrap();
        value["status"] = serde_json::json!("withdrawn");
        value["version"] = serde_json::json!(2);
        value["updated_at_unix_nanos"] = serde_json::json!(200);
        assert_eq!(
            decode_consent_authorization_state(&serde_json::to_vec(&value).unwrap())
                .unwrap_err()
                .code,
            "CONSENTS_PERSISTED_STATE_INVALID"
        );

        let oversized = vec![b'x'; CONSENT_AUTHORIZATION_STATE_MAXIMUM_BYTES as usize + 1];
        assert_eq!(
            decode_consent_authorization_state(&oversized)
                .unwrap_err()
                .code,
            "CONSENTS_PERSISTED_STATE_INVALID"
        );
    }
}
