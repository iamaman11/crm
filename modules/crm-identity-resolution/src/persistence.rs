use crate::domain::{
    CanonicalPartyPair, DecisionReasonCode, DuplicateCandidateCase, DuplicateCandidateCaseId,
    DuplicateCandidateCaseSnapshot, DuplicateCandidateCaseStatus, EvidenceReference,
    MatchEvidenceSnapshot, MatchSignal, MatcherProfileCode, PartyReference, SignalKindCode,
    SignalSourceCode,
};
use crm_module_sdk::{ErrorCategory, SdkError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const DUPLICATE_CANDIDATE_CASE_STATE_SCHEMA_ID: &str =
    "crm.identity_resolution.candidate_case.state";
pub const DUPLICATE_CANDIDATE_CASE_STATE_SCHEMA_VERSION: &str = "1.0.0";
pub const DUPLICATE_CANDIDATE_CASE_STATE_MAXIMUM_BYTES: u64 = 512 * 1024;
pub const DUPLICATE_CANDIDATE_CASE_STATE_RETENTION_POLICY_ID: &str =
    "crm.identity_resolution.candidate_evidence";
const DUPLICATE_CANDIDATE_CASE_STATE_DESCRIPTOR: &[u8] = b"crm.identity_resolution.candidate_case.state/v1:case_id,left_party_id,right_party_id,evidence_history[left_party_version,right_party_version,matcher_profile,score_basis_points,signals[kind,source,evidence_ref,contribution_basis_points],generated_at_unix_nanos],status,decision_reason,created_at_unix_nanos,updated_at_unix_nanos,version";

pub fn duplicate_candidate_case_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(DUPLICATE_CANDIDATE_CASE_STATE_DESCRIPTOR).into()
}

pub fn encode_duplicate_candidate_case_state(
    candidate: &DuplicateCandidateCase,
) -> Result<Vec<u8>, SdkError> {
    let bytes = serde_json::to_vec(&DuplicateCandidateCaseStateV1::from(candidate.snapshot()))
        .map_err(|error| {
            persisted_error(format!(
                "Identity Resolution candidate state serialization failed: {error}"
            ))
        })?;
    validate_size(&bytes)?;
    Ok(bytes)
}

pub fn decode_duplicate_candidate_case_state(
    bytes: &[u8],
) -> Result<DuplicateCandidateCase, SdkError> {
    validate_size(bytes)?;
    let state: DuplicateCandidateCaseStateV1 = serde_json::from_slice(bytes).map_err(|error| {
        persisted_error(format!(
            "Identity Resolution candidate state JSON is invalid: {error}"
        ))
    })?;
    DuplicateCandidateCase::rehydrate(state.try_into()?)
        .map_err(|error| persisted_error(format!("{}: {}", error.code, error.safe_message)))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DuplicateCandidateCaseStateV1 {
    case_id: String,
    left_party_id: String,
    right_party_id: String,
    evidence_history: Vec<MatchEvidenceStateV1>,
    status: DuplicateCandidateCaseStatusState,
    decision_reason: Option<String>,
    created_at_unix_nanos: i64,
    updated_at_unix_nanos: i64,
    version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MatchEvidenceStateV1 {
    left_party_version: i64,
    right_party_version: i64,
    matcher_profile: String,
    score_basis_points: u16,
    signals: Vec<MatchSignalStateV1>,
    generated_at_unix_nanos: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MatchSignalStateV1 {
    kind: String,
    source: String,
    evidence_ref: String,
    contribution_basis_points: i16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum DuplicateCandidateCaseStatusState {
    Open,
    Dismissed,
    ConfirmedDuplicate,
}

impl From<DuplicateCandidateCaseSnapshot> for DuplicateCandidateCaseStateV1 {
    fn from(value: DuplicateCandidateCaseSnapshot) -> Self {
        Self {
            case_id: value.case_id.as_str().to_owned(),
            left_party_id: value.pair.left().as_str().to_owned(),
            right_party_id: value.pair.right().as_str().to_owned(),
            evidence_history: value
                .evidence_history
                .into_iter()
                .map(MatchEvidenceStateV1::from)
                .collect(),
            status: value.status.into(),
            decision_reason: value
                .decision_reason
                .as_ref()
                .map(|reason| reason.as_str().to_owned()),
            created_at_unix_nanos: value.created_at_unix_nanos,
            updated_at_unix_nanos: value.updated_at_unix_nanos,
            version: value.version,
        }
    }
}

impl TryFrom<DuplicateCandidateCaseStateV1> for DuplicateCandidateCaseSnapshot {
    type Error = SdkError;

    fn try_from(value: DuplicateCandidateCaseStateV1) -> Result<Self, Self::Error> {
        let left_party_ref = PartyReference::try_new(value.left_party_id.clone())
            .map_err(|error| persisted_error(error.to_string()))?;
        let right_party_ref = PartyReference::try_new(value.right_party_id.clone())
            .map_err(|error| persisted_error(error.to_string()))?;
        if left_party_ref.as_str() != value.left_party_id
            || right_party_ref.as_str() != value.right_party_id
        {
            return Err(persisted_error(
                "persisted Party references are not canonical",
            ));
        }
        let pair = CanonicalPartyPair::try_new(left_party_ref, right_party_ref)
            .map_err(|error| persisted_error(error.to_string()))?;
        if pair.left().as_str() != value.left_party_id || pair.right().as_str() != value.right_party_id
        {
            return Err(persisted_error(
                "persisted Party pair is not in canonical order",
            ));
        }

        let case_id = DuplicateCandidateCaseId::try_new(value.case_id.clone())
            .map_err(|error| persisted_error(error.to_string()))?;
        if case_id.as_str() != value.case_id {
            return Err(persisted_error("persisted case identifier is not canonical"));
        }

        let decision_reason = value
            .decision_reason
            .map(|raw| {
                let parsed = DecisionReasonCode::try_new(raw.clone())
                    .map_err(|error| persisted_error(error.to_string()))?;
                if parsed.as_str() != raw {
                    return Err(persisted_error(
                        "persisted decision reason is not canonical",
                    ));
                }
                Ok(parsed)
            })
            .transpose()?;

        let evidence_history = value
            .evidence_history
            .into_iter()
            .map(|evidence| evidence.into_domain(&pair))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            case_id,
            pair,
            evidence_history,
            status: value.status.into(),
            decision_reason,
            created_at_unix_nanos: value.created_at_unix_nanos,
            updated_at_unix_nanos: value.updated_at_unix_nanos,
            version: value.version,
        })
    }
}

impl From<MatchEvidenceSnapshot> for MatchEvidenceStateV1 {
    fn from(value: MatchEvidenceSnapshot) -> Self {
        Self {
            left_party_version: value.left_party_version(),
            right_party_version: value.right_party_version(),
            matcher_profile: value.matcher_profile().as_str().to_owned(),
            score_basis_points: value.score_basis_points(),
            signals: value
                .signals()
                .iter()
                .cloned()
                .map(MatchSignalStateV1::from)
                .collect(),
            generated_at_unix_nanos: value.generated_at_unix_nanos(),
        }
    }
}

impl MatchEvidenceStateV1 {
    fn into_domain(self, pair: &CanonicalPartyPair) -> Result<MatchEvidenceSnapshot, SdkError> {
        let matcher_profile = MatcherProfileCode::try_new(self.matcher_profile.clone())
            .map_err(|error| persisted_error(error.to_string()))?;
        if matcher_profile.as_str() != self.matcher_profile {
            return Err(persisted_error(
                "persisted matcher profile is not canonical",
            ));
        }
        let signals = self
            .signals
            .into_iter()
            .map(MatchSignalStateV1::into_domain)
            .collect::<Result<Vec<_>, _>>()?;
        MatchEvidenceSnapshot::try_new(
            pair.left().clone(),
            self.left_party_version,
            pair.right().clone(),
            self.right_party_version,
            matcher_profile,
            self.score_basis_points,
            signals,
            self.generated_at_unix_nanos,
        )
        .map_err(|error| persisted_error(error.to_string()))
    }
}

impl From<MatchSignal> for MatchSignalStateV1 {
    fn from(value: MatchSignal) -> Self {
        Self {
            kind: value.kind().as_str().to_owned(),
            source: value.source().as_str().to_owned(),
            evidence_ref: value.evidence_ref().as_str().to_owned(),
            contribution_basis_points: value.contribution_basis_points(),
        }
    }
}

impl MatchSignalStateV1 {
    fn into_domain(self) -> Result<MatchSignal, SdkError> {
        let kind = SignalKindCode::try_new(self.kind.clone())
            .map_err(|error| persisted_error(error.to_string()))?;
        let source = SignalSourceCode::try_new(self.source.clone())
            .map_err(|error| persisted_error(error.to_string()))?;
        let evidence_ref = EvidenceReference::try_new(self.evidence_ref.clone())
            .map_err(|error| persisted_error(error.to_string()))?;
        if kind.as_str() != self.kind
            || source.as_str() != self.source
            || evidence_ref.as_str() != self.evidence_ref
        {
            return Err(persisted_error(
                "persisted evidence signal values are not canonical",
            ));
        }
        MatchSignal::try_new(kind, source, evidence_ref, self.contribution_basis_points)
            .map_err(|error| persisted_error(error.to_string()))
    }
}

impl From<DuplicateCandidateCaseStatus> for DuplicateCandidateCaseStatusState {
    fn from(value: DuplicateCandidateCaseStatus) -> Self {
        match value {
            DuplicateCandidateCaseStatus::Open => Self::Open,
            DuplicateCandidateCaseStatus::Dismissed => Self::Dismissed,
            DuplicateCandidateCaseStatus::ConfirmedDuplicate => Self::ConfirmedDuplicate,
        }
    }
}

impl From<DuplicateCandidateCaseStatusState> for DuplicateCandidateCaseStatus {
    fn from(value: DuplicateCandidateCaseStatusState) -> Self {
        match value {
            DuplicateCandidateCaseStatusState::Open => Self::Open,
            DuplicateCandidateCaseStatusState::Dismissed => Self::Dismissed,
            DuplicateCandidateCaseStatusState::ConfirmedDuplicate => Self::ConfirmedDuplicate,
        }
    }
}

fn validate_size(bytes: &[u8]) -> Result<(), SdkError> {
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX)
        > DUPLICATE_CANDIDATE_CASE_STATE_MAXIMUM_BYTES
    {
        return Err(persisted_error(format!(
            "Identity Resolution candidate state exceeds the maximum of {DUPLICATE_CANDIDATE_CASE_STATE_MAXIMUM_BYTES} bytes"
        )));
    }
    Ok(())
}

fn persisted_error(message: impl Into<String>) -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_PERSISTED_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "The persisted Identity Resolution candidate state is invalid.",
    )
    .with_internal_reference(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        CreateDuplicateCandidateCase, DecideDuplicateCandidateCase, RefreshDuplicateCandidateEvidence,
    };

    fn signal(kind: &str, contribution: i16) -> MatchSignal {
        MatchSignal::try_new(
            SignalKindCode::try_new(kind).unwrap(),
            SignalSourceCode::try_new("party.normalized").unwrap(),
            EvidenceReference::try_new(format!("evidence://{kind}")).unwrap(),
            contribution,
        )
        .unwrap()
    }

    fn evidence(left_version: i64, right_version: i64, generated_at: i64) -> MatchEvidenceSnapshot {
        MatchEvidenceSnapshot::try_new(
            PartyReference::try_new("party-b").unwrap(),
            right_version,
            PartyReference::try_new("party-a").unwrap(),
            left_version,
            MatcherProfileCode::try_new("deterministic.v1").unwrap(),
            8_500,
            vec![signal("name.exact", 6_000), signal("email.exact", 2_500)],
            generated_at,
        )
        .unwrap()
    }

    fn candidate() -> DuplicateCandidateCase {
        let mut candidate = DuplicateCandidateCase::create(CreateDuplicateCandidateCase {
            evidence: evidence(5, 3, 100),
            occurred_at_unix_nanos: 110,
        })
        .unwrap();
        candidate
            .refresh_evidence(RefreshDuplicateCandidateEvidence {
                expected_version: 1,
                evidence: evidence(6, 3, 200),
                occurred_at_unix_nanos: 210,
            })
            .unwrap();
        candidate
    }

    #[test]
    fn round_trip_is_exact_deterministic_and_descriptor_hash_is_nonzero() {
        let value = candidate();
        let first = encode_duplicate_candidate_case_state(&value).unwrap();
        let second = encode_duplicate_candidate_case_state(&value).unwrap();
        let decoded = decode_duplicate_candidate_case_state(&first).unwrap();
        assert_eq!(decoded, value);
        assert_eq!(first, second);
        assert_ne!(duplicate_candidate_case_state_descriptor_hash(), [0; 32]);
    }

    #[test]
    fn terminal_state_round_trip_preserves_decision_evidence() {
        let mut value = candidate();
        value
            .confirm_duplicate(DecideDuplicateCandidateCase {
                expected_version: 2,
                reason: DecisionReasonCode::try_new("review.exact_identity").unwrap(),
                occurred_at_unix_nanos: 300,
            })
            .unwrap();
        let decoded = decode_duplicate_candidate_case_state(
            &encode_duplicate_candidate_case_state(&value).unwrap(),
        )
        .unwrap();
        assert_eq!(decoded, value);
        assert_eq!(decoded.status(), DuplicateCandidateCaseStatus::ConfirmedDuplicate);
        assert_eq!(decoded.version(), 3);
    }

    #[test]
    fn rejects_unknown_fields_and_noncanonical_semantic_values() {
        let canonical = encode_duplicate_candidate_case_state(&candidate()).unwrap();
        let mut value: serde_json::Value = serde_json::from_slice(&canonical).unwrap();
        value["unexpected"] = serde_json::json!(true);
        assert_eq!(
            decode_duplicate_candidate_case_state(&serde_json::to_vec(&value).unwrap())
                .unwrap_err()
                .code,
            "IDENTITY_RESOLUTION_PERSISTED_STATE_INVALID"
        );

        let mut value: serde_json::Value = serde_json::from_slice(&canonical).unwrap();
        value["evidence_history"][0]["matcher_profile"] =
            serde_json::json!(" Deterministic.V1 ");
        assert_eq!(
            decode_duplicate_candidate_case_state(&serde_json::to_vec(&value).unwrap())
                .unwrap_err()
                .code,
            "IDENTITY_RESOLUTION_PERSISTED_STATE_INVALID"
        );
    }

    #[test]
    fn rejects_noncanonical_pair_order_and_impossible_version() {
        let canonical = encode_duplicate_candidate_case_state(&candidate()).unwrap();
        let mut value: serde_json::Value = serde_json::from_slice(&canonical).unwrap();
        let left = value["left_party_id"].clone();
        value["left_party_id"] = value["right_party_id"].clone();
        value["right_party_id"] = left;
        assert_eq!(
            decode_duplicate_candidate_case_state(&serde_json::to_vec(&value).unwrap())
                .unwrap_err()
                .code,
            "IDENTITY_RESOLUTION_PERSISTED_STATE_INVALID"
        );

        let mut value: serde_json::Value = serde_json::from_slice(&canonical).unwrap();
        value["version"] = serde_json::json!(99);
        assert_eq!(
            decode_duplicate_candidate_case_state(&serde_json::to_vec(&value).unwrap())
                .unwrap_err()
                .code,
            "IDENTITY_RESOLUTION_PERSISTED_STATE_INVALID"
        );
    }
}
