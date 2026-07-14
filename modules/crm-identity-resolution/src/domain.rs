use crm_module_sdk::{ErrorCategory, FieldName, FieldViolation, RecordId, SdkError};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;

const MAX_SEMANTIC_CODE_BYTES: usize = 128;
const MAX_EVIDENCE_REFERENCE_BYTES: usize = 512;
const MAX_SIGNALS_PER_SNAPSHOT: usize = 64;
const MAX_EVIDENCE_SNAPSHOTS: usize = 64;
const MAX_SCORE_BASIS_POINTS: u16 = 10_000;
const MIN_SIGNAL_CONTRIBUTION_BASIS_POINTS: i16 = -10_000;
const MAX_SIGNAL_CONTRIBUTION_BASIS_POINTS: i16 = 10_000;
const CASE_ID_DOMAIN: &[u8] = b"crm.identity_resolution.candidate_case_id/v1";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PartyReference(RecordId);

impl PartyReference {
    pub fn try_new(value: impl Into<String>) -> Result<Self, SdkError> {
        RecordId::try_new(value.into()).map(Self).map_err(|error| {
            invalid(
                "IDENTITY_RESOLUTION_PARTY_REFERENCE_INVALID",
                "identity_resolution.party_ref.party_id",
                error.to_string(),
            )
        })
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DuplicateCandidateCaseId(RecordId);

impl DuplicateCandidateCaseId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, SdkError> {
        RecordId::try_new(value.into()).map(Self).map_err(|error| {
            invalid(
                "IDENTITY_RESOLUTION_CASE_ID_INVALID",
                "identity_resolution.case_id",
                error.to_string(),
            )
        })
    }

    pub fn for_pair(pair: &CanonicalPartyPair) -> Result<Self, SdkError> {
        let mut hasher = Sha256::new();
        hasher.update(CASE_ID_DOMAIN);
        hasher.update([0]);
        hasher.update(pair.left.as_str().as_bytes());
        hasher.update([0]);
        hasher.update(pair.right.as_str().as_bytes());
        let digest = hasher.finalize();
        let mut hex = String::with_capacity(digest.len() * 2);
        for byte in digest {
            write!(&mut hex, "{byte:02x}").expect("writing to String cannot fail");
        }
        Self::try_new(format!("idrc-{hex}"))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CanonicalPartyPair {
    left: PartyReference,
    right: PartyReference,
}

impl CanonicalPartyPair {
    pub fn try_new(first: PartyReference, second: PartyReference) -> Result<Self, SdkError> {
        if first == second {
            return Err(invalid(
                "IDENTITY_RESOLUTION_SELF_PAIR_INVALID",
                "identity_resolution.party_pair",
                "a Party cannot be compared with itself",
            ));
        }
        let (left, right) = if first < second {
            (first, second)
        } else {
            (second, first)
        };
        Ok(Self { left, right })
    }

    pub fn left(&self) -> &PartyReference {
        &self.left
    }

    pub fn right(&self) -> &PartyReference {
        &self.right
    }

    pub fn contains(&self, party: &PartyReference) -> bool {
        &self.left == party || &self.right == party
    }
}

macro_rules! semantic_code_type {
    ($name:ident, $code:literal, $field:literal, $label:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn try_new(value: impl Into<String>) -> Result<Self, SdkError> {
                normalize_semantic_identifier(
                    &value.into(),
                    MAX_SEMANTIC_CODE_BYTES,
                    $code,
                    $field,
                    $label,
                )
                .map(Self)
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

semantic_code_type!(
    MatcherProfileCode,
    "IDENTITY_RESOLUTION_MATCHER_PROFILE_INVALID",
    "identity_resolution.evidence.matcher_profile",
    "matcher profile"
);
semantic_code_type!(
    SignalKindCode,
    "IDENTITY_RESOLUTION_SIGNAL_KIND_INVALID",
    "identity_resolution.evidence.signal.kind",
    "signal kind"
);
semantic_code_type!(
    SignalSourceCode,
    "IDENTITY_RESOLUTION_SIGNAL_SOURCE_INVALID",
    "identity_resolution.evidence.signal.source",
    "signal source"
);
semantic_code_type!(
    DecisionReasonCode,
    "IDENTITY_RESOLUTION_DECISION_REASON_INVALID",
    "identity_resolution.decision_reason",
    "decision reason"
);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EvidenceReference(String);

impl EvidenceReference {
    pub fn try_new(value: impl Into<String>) -> Result<Self, SdkError> {
        let value = value.into();
        if value.chars().any(char::is_control) {
            return Err(invalid(
                "IDENTITY_RESOLUTION_EVIDENCE_REFERENCE_INVALID",
                "identity_resolution.evidence.signal.evidence_ref",
                "evidence reference must not contain control characters",
            ));
        }
        let canonical = value.trim().to_owned();
        if canonical.is_empty() || canonical.len() > MAX_EVIDENCE_REFERENCE_BYTES {
            return Err(invalid(
                "IDENTITY_RESOLUTION_EVIDENCE_REFERENCE_INVALID",
                "identity_resolution.evidence.signal.evidence_ref",
                format!(
                    "evidence reference must be non-empty and not exceed {MAX_EVIDENCE_REFERENCE_BYTES} UTF-8 bytes"
                ),
            ));
        }
        Ok(Self(canonical))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MatchSignal {
    kind: SignalKindCode,
    source: SignalSourceCode,
    evidence_ref: EvidenceReference,
    contribution_basis_points: i16,
}

impl MatchSignal {
    pub fn try_new(
        kind: SignalKindCode,
        source: SignalSourceCode,
        evidence_ref: EvidenceReference,
        contribution_basis_points: i16,
    ) -> Result<Self, SdkError> {
        if !(MIN_SIGNAL_CONTRIBUTION_BASIS_POINTS..=MAX_SIGNAL_CONTRIBUTION_BASIS_POINTS)
            .contains(&contribution_basis_points)
        {
            return Err(invalid(
                "IDENTITY_RESOLUTION_SIGNAL_CONTRIBUTION_INVALID",
                "identity_resolution.evidence.signal.contribution_basis_points",
                "signal contribution must be between -10000 and 10000 basis points",
            ));
        }
        Ok(Self {
            kind,
            source,
            evidence_ref,
            contribution_basis_points,
        })
    }

    pub fn kind(&self) -> &SignalKindCode {
        &self.kind
    }

    pub fn source(&self) -> &SignalSourceCode {
        &self.source
    }

    pub fn evidence_ref(&self) -> &EvidenceReference {
        &self.evidence_ref
    }

    pub fn contribution_basis_points(&self) -> i16 {
        self.contribution_basis_points
    }

    fn same_identity(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.source == other.source
            && self.evidence_ref == other.evidence_ref
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchEvidenceSnapshot {
    pair: CanonicalPartyPair,
    left_party_version: i64,
    right_party_version: i64,
    matcher_profile: MatcherProfileCode,
    score_basis_points: u16,
    signals: Vec<MatchSignal>,
    generated_at_unix_nanos: i64,
}

impl MatchEvidenceSnapshot {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        first_party_ref: PartyReference,
        first_party_version: i64,
        second_party_ref: PartyReference,
        second_party_version: i64,
        matcher_profile: MatcherProfileCode,
        score_basis_points: u16,
        mut signals: Vec<MatchSignal>,
        generated_at_unix_nanos: i64,
    ) -> Result<Self, SdkError> {
        validate_positive_version(
            "identity_resolution.evidence.first_party_version",
            first_party_version,
        )?;
        validate_positive_version(
            "identity_resolution.evidence.second_party_version",
            second_party_version,
        )?;
        validate_timestamp(
            "identity_resolution.evidence.generated_at_unix_nanos",
            generated_at_unix_nanos,
        )?;
        if score_basis_points > MAX_SCORE_BASIS_POINTS {
            return Err(invalid(
                "IDENTITY_RESOLUTION_SCORE_INVALID",
                "identity_resolution.evidence.score_basis_points",
                "score must be between 0 and 10000 basis points",
            ));
        }
        if signals.is_empty() || signals.len() > MAX_SIGNALS_PER_SNAPSHOT {
            return Err(invalid(
                "IDENTITY_RESOLUTION_SIGNALS_INVALID",
                "identity_resolution.evidence.signals",
                format!("evidence must contain between 1 and {MAX_SIGNALS_PER_SNAPSHOT} signals"),
            ));
        }

        let pair = CanonicalPartyPair::try_new(first_party_ref.clone(), second_party_ref.clone())?;
        let (left_party_version, right_party_version) = if pair.left() == &first_party_ref {
            (first_party_version, second_party_version)
        } else {
            (second_party_version, first_party_version)
        };

        signals.sort();
        if signals
            .windows(2)
            .any(|window| window[0].same_identity(&window[1]))
        {
            return Err(invalid(
                "IDENTITY_RESOLUTION_DUPLICATE_SIGNAL_INVALID",
                "identity_resolution.evidence.signals",
                "one evidence snapshot cannot contain duplicate signal identities",
            ));
        }

        Ok(Self {
            pair,
            left_party_version,
            right_party_version,
            matcher_profile,
            score_basis_points,
            signals,
            generated_at_unix_nanos,
        })
    }

    pub fn pair(&self) -> &CanonicalPartyPair {
        &self.pair
    }

    pub fn left_party_version(&self) -> i64 {
        self.left_party_version
    }

    pub fn right_party_version(&self) -> i64 {
        self.right_party_version
    }

    pub fn matcher_profile(&self) -> &MatcherProfileCode {
        &self.matcher_profile
    }

    pub fn score_basis_points(&self) -> u16 {
        self.score_basis_points
    }

    pub fn signals(&self) -> &[MatchSignal] {
        &self.signals
    }

    pub fn generated_at_unix_nanos(&self) -> i64 {
        self.generated_at_unix_nanos
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DuplicateCandidateCaseStatus {
    Open,
    Dismissed,
    ConfirmedDuplicate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateCandidateCase {
    case_id: DuplicateCandidateCaseId,
    pair: CanonicalPartyPair,
    evidence_history: Vec<MatchEvidenceSnapshot>,
    status: DuplicateCandidateCaseStatus,
    decision_reason: Option<DecisionReasonCode>,
    created_at_unix_nanos: i64,
    updated_at_unix_nanos: i64,
    version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateCandidateCaseSnapshot {
    pub case_id: DuplicateCandidateCaseId,
    pub pair: CanonicalPartyPair,
    pub evidence_history: Vec<MatchEvidenceSnapshot>,
    pub status: DuplicateCandidateCaseStatus,
    pub decision_reason: Option<DecisionReasonCode>,
    pub created_at_unix_nanos: i64,
    pub updated_at_unix_nanos: i64,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateDuplicateCandidateCase {
    pub evidence: MatchEvidenceSnapshot,
    pub occurred_at_unix_nanos: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshDuplicateCandidateEvidence {
    pub expected_version: i64,
    pub evidence: MatchEvidenceSnapshot,
    pub occurred_at_unix_nanos: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecideDuplicateCandidateCase {
    pub expected_version: i64,
    pub reason: DecisionReasonCode,
    pub occurred_at_unix_nanos: i64,
}

impl DuplicateCandidateCase {
    pub fn create(command: CreateDuplicateCandidateCase) -> Result<Self, SdkError> {
        validate_timestamp(
            "identity_resolution.occurred_at_unix_nanos",
            command.occurred_at_unix_nanos,
        )?;
        if command.evidence.generated_at_unix_nanos() > command.occurred_at_unix_nanos {
            return Err(invalid(
                "IDENTITY_RESOLUTION_FUTURE_EVIDENCE_INVALID",
                "identity_resolution.evidence.generated_at_unix_nanos",
                "evidence cannot be generated after the governed registration time",
            ));
        }
        let pair = command.evidence.pair().clone();
        let case_id = DuplicateCandidateCaseId::for_pair(&pair)?;
        Ok(Self {
            case_id,
            pair,
            evidence_history: vec![command.evidence],
            status: DuplicateCandidateCaseStatus::Open,
            decision_reason: None,
            created_at_unix_nanos: command.occurred_at_unix_nanos,
            updated_at_unix_nanos: command.occurred_at_unix_nanos,
            version: 1,
        })
    }

    pub fn rehydrate(snapshot: DuplicateCandidateCaseSnapshot) -> Result<Self, SdkError> {
        validate_timestamp(
            "identity_resolution.created_at_unix_nanos",
            snapshot.created_at_unix_nanos,
        )?;
        validate_timestamp(
            "identity_resolution.updated_at_unix_nanos",
            snapshot.updated_at_unix_nanos,
        )?;
        if snapshot.updated_at_unix_nanos < snapshot.created_at_unix_nanos {
            return Err(invalid(
                "IDENTITY_RESOLUTION_PERSISTED_TIME_INVALID",
                "identity_resolution.updated_at_unix_nanos",
                "updated time cannot precede created time",
            ));
        }
        if snapshot.evidence_history.is_empty()
            || snapshot.evidence_history.len() > MAX_EVIDENCE_SNAPSHOTS
        {
            return Err(invalid(
                "IDENTITY_RESOLUTION_PERSISTED_EVIDENCE_INVALID",
                "identity_resolution.evidence_history",
                "persisted evidence history is empty or exceeds the supported bound",
            ));
        }
        if DuplicateCandidateCaseId::for_pair(&snapshot.pair)? != snapshot.case_id {
            return Err(invalid(
                "IDENTITY_RESOLUTION_PERSISTED_CASE_ID_INVALID",
                "identity_resolution.case_id",
                "persisted case identifier does not match the canonical Party pair",
            ));
        }
        validate_evidence_history(&snapshot.pair, &snapshot.evidence_history)?;

        let history_version = i64::try_from(snapshot.evidence_history.len()).map_err(|_| {
            invalid(
                "IDENTITY_RESOLUTION_PERSISTED_VERSION_INVALID",
                "identity_resolution.version",
                "evidence history length cannot be represented as an aggregate version",
            )
        })?;
        let expected_version = match snapshot.status {
            DuplicateCandidateCaseStatus::Open => {
                if snapshot.decision_reason.is_some() {
                    return Err(invalid(
                        "IDENTITY_RESOLUTION_PERSISTED_DECISION_INVALID",
                        "identity_resolution.decision_reason",
                        "an open candidate case cannot contain a terminal decision reason",
                    ));
                }
                history_version
            }
            DuplicateCandidateCaseStatus::Dismissed
            | DuplicateCandidateCaseStatus::ConfirmedDuplicate => {
                if snapshot.decision_reason.is_none() {
                    return Err(invalid(
                        "IDENTITY_RESOLUTION_PERSISTED_DECISION_INVALID",
                        "identity_resolution.decision_reason",
                        "a terminal candidate case must contain a decision reason",
                    ));
                }
                history_version.checked_add(1).ok_or_else(|| {
                    invalid(
                        "IDENTITY_RESOLUTION_PERSISTED_VERSION_INVALID",
                        "identity_resolution.version",
                        "persisted aggregate version overflow",
                    )
                })?
            }
        };
        if snapshot.version != expected_version {
            return Err(invalid(
                "IDENTITY_RESOLUTION_PERSISTED_VERSION_INVALID",
                "identity_resolution.version",
                format!(
                    "persisted version {} does not match reachable version {expected_version}",
                    snapshot.version
                ),
            ));
        }
        if snapshot.version > 1 && snapshot.updated_at_unix_nanos <= snapshot.created_at_unix_nanos
        {
            return Err(invalid(
                "IDENTITY_RESOLUTION_PERSISTED_TIME_INVALID",
                "identity_resolution.updated_at_unix_nanos",
                "a mutated candidate case must advance the governed mutation time",
            ));
        }

        Ok(Self {
            case_id: snapshot.case_id,
            pair: snapshot.pair,
            evidence_history: snapshot.evidence_history,
            status: snapshot.status,
            decision_reason: snapshot.decision_reason,
            created_at_unix_nanos: snapshot.created_at_unix_nanos,
            updated_at_unix_nanos: snapshot.updated_at_unix_nanos,
            version: snapshot.version,
        })
    }

    pub fn refresh_evidence(
        &mut self,
        command: RefreshDuplicateCandidateEvidence,
    ) -> Result<(), SdkError> {
        self.require_version(command.expected_version)?;
        self.require_open()?;
        self.require_strictly_increasing_time(command.occurred_at_unix_nanos)?;
        if self.evidence_history.len() >= MAX_EVIDENCE_SNAPSHOTS {
            return Err(conflict(
                "IDENTITY_RESOLUTION_EVIDENCE_HISTORY_EXHAUSTED",
                "the candidate case reached the maximum supported evidence snapshot count",
            ));
        }
        if command.evidence.pair() != &self.pair {
            return Err(invalid(
                "IDENTITY_RESOLUTION_EVIDENCE_PAIR_MISMATCH",
                "identity_resolution.evidence.party_pair",
                "refreshed evidence must refer to the same canonical Party pair",
            ));
        }
        if command.evidence.generated_at_unix_nanos() > command.occurred_at_unix_nanos {
            return Err(invalid(
                "IDENTITY_RESOLUTION_FUTURE_EVIDENCE_INVALID",
                "identity_resolution.evidence.generated_at_unix_nanos",
                "evidence cannot be generated after the governed refresh time",
            ));
        }
        let previous = self
            .evidence_history
            .last()
            .expect("candidate case always contains evidence");
        validate_evidence_progression(previous, &command.evidence)?;
        let next_version = self.next_version()?;
        self.evidence_history.push(command.evidence);
        self.updated_at_unix_nanos = command.occurred_at_unix_nanos;
        self.version = next_version;
        Ok(())
    }

    pub fn dismiss(&mut self, command: DecideDuplicateCandidateCase) -> Result<(), SdkError> {
        self.apply_terminal_decision(DuplicateCandidateCaseStatus::Dismissed, command)
    }

    pub fn confirm_duplicate(
        &mut self,
        command: DecideDuplicateCandidateCase,
    ) -> Result<(), SdkError> {
        self.apply_terminal_decision(DuplicateCandidateCaseStatus::ConfirmedDuplicate, command)
    }

    fn apply_terminal_decision(
        &mut self,
        status: DuplicateCandidateCaseStatus,
        command: DecideDuplicateCandidateCase,
    ) -> Result<(), SdkError> {
        self.require_version(command.expected_version)?;
        self.require_open()?;
        self.require_strictly_increasing_time(command.occurred_at_unix_nanos)?;
        let next_version = self.next_version()?;
        self.status = status;
        self.decision_reason = Some(command.reason);
        self.updated_at_unix_nanos = command.occurred_at_unix_nanos;
        self.version = next_version;
        Ok(())
    }

    fn require_open(&self) -> Result<(), SdkError> {
        if self.status != DuplicateCandidateCaseStatus::Open {
            return Err(conflict(
                "IDENTITY_RESOLUTION_CASE_TERMINAL",
                "terminal candidate decisions are irreversible in Phase 8A.5",
            ));
        }
        Ok(())
    }

    fn require_version(&self, expected_version: i64) -> Result<(), SdkError> {
        if expected_version != self.version {
            return Err(conflict(
                "IDENTITY_RESOLUTION_VERSION_CONFLICT",
                format!(
                    "expected version {expected_version} but current version is {}",
                    self.version
                ),
            ));
        }
        Ok(())
    }

    fn require_strictly_increasing_time(
        &self,
        occurred_at_unix_nanos: i64,
    ) -> Result<(), SdkError> {
        validate_timestamp(
            "identity_resolution.occurred_at_unix_nanos",
            occurred_at_unix_nanos,
        )?;
        if occurred_at_unix_nanos <= self.updated_at_unix_nanos {
            return Err(invalid(
                "IDENTITY_RESOLUTION_TIME_NOT_INCREASING",
                "identity_resolution.occurred_at_unix_nanos",
                "governed mutation time must be strictly later than the current aggregate time",
            ));
        }
        Ok(())
    }

    fn next_version(&self) -> Result<i64, SdkError> {
        self.version.checked_add(1).ok_or_else(|| {
            conflict(
                "IDENTITY_RESOLUTION_VERSION_EXHAUSTED",
                "candidate case version cannot advance beyond i64::MAX",
            )
        })
    }

    pub fn snapshot(&self) -> DuplicateCandidateCaseSnapshot {
        DuplicateCandidateCaseSnapshot {
            case_id: self.case_id.clone(),
            pair: self.pair.clone(),
            evidence_history: self.evidence_history.clone(),
            status: self.status,
            decision_reason: self.decision_reason.clone(),
            created_at_unix_nanos: self.created_at_unix_nanos,
            updated_at_unix_nanos: self.updated_at_unix_nanos,
            version: self.version,
        }
    }

    pub fn case_id(&self) -> &DuplicateCandidateCaseId {
        &self.case_id
    }

    pub fn pair(&self) -> &CanonicalPartyPair {
        &self.pair
    }

    pub fn evidence_history(&self) -> &[MatchEvidenceSnapshot] {
        &self.evidence_history
    }

    pub fn current_evidence(&self) -> &MatchEvidenceSnapshot {
        self.evidence_history
            .last()
            .expect("candidate case always contains evidence")
    }

    pub fn status(&self) -> DuplicateCandidateCaseStatus {
        self.status
    }

    pub fn decision_reason(&self) -> Option<&DecisionReasonCode> {
        self.decision_reason.as_ref()
    }

    pub fn created_at_unix_nanos(&self) -> i64 {
        self.created_at_unix_nanos
    }

    pub fn updated_at_unix_nanos(&self) -> i64 {
        self.updated_at_unix_nanos
    }

    pub fn version(&self) -> i64 {
        self.version
    }
}

fn validate_evidence_history(
    pair: &CanonicalPartyPair,
    history: &[MatchEvidenceSnapshot],
) -> Result<(), SdkError> {
    for evidence in history {
        if evidence.pair() != pair {
            return Err(invalid(
                "IDENTITY_RESOLUTION_PERSISTED_EVIDENCE_PAIR_INVALID",
                "identity_resolution.evidence_history",
                "persisted evidence contains a different Party pair",
            ));
        }
    }
    for window in history.windows(2) {
        validate_evidence_progression(&window[0], &window[1])?;
    }
    Ok(())
}

fn validate_evidence_progression(
    previous: &MatchEvidenceSnapshot,
    next: &MatchEvidenceSnapshot,
) -> Result<(), SdkError> {
    if next.generated_at_unix_nanos() <= previous.generated_at_unix_nanos() {
        return Err(invalid(
            "IDENTITY_RESOLUTION_STALE_EVIDENCE_TIME_INVALID",
            "identity_resolution.evidence.generated_at_unix_nanos",
            "refreshed evidence generation time must advance",
        ));
    }
    if next.left_party_version() < previous.left_party_version()
        || next.right_party_version() < previous.right_party_version()
    {
        return Err(invalid(
            "IDENTITY_RESOLUTION_SOURCE_VERSION_REGRESSION",
            "identity_resolution.evidence.party_versions",
            "refreshed evidence cannot regress either authoritative Party source version",
        ));
    }
    if next.left_party_version() == previous.left_party_version()
        && next.right_party_version() == previous.right_party_version()
    {
        return Err(invalid(
            "IDENTITY_RESOLUTION_STALE_EVIDENCE_VERSION",
            "identity_resolution.evidence.party_versions",
            "at least one authoritative Party source version must advance for evidence refresh",
        ));
    }
    Ok(())
}

fn validate_positive_version(field: &'static str, value: i64) -> Result<(), SdkError> {
    if value <= 0 {
        return Err(invalid(
            "IDENTITY_RESOLUTION_SOURCE_VERSION_INVALID",
            field,
            "source version must be positive",
        ));
    }
    Ok(())
}

fn validate_timestamp(field: &'static str, value: i64) -> Result<(), SdkError> {
    if value <= 0 {
        return Err(invalid(
            "IDENTITY_RESOLUTION_TIME_INVALID",
            field,
            "time must be a positive Unix-nanosecond value",
        ));
    }
    Ok(())
}

fn normalize_semantic_identifier(
    value: &str,
    maximum_bytes: usize,
    code: &'static str,
    field: &'static str,
    label: &str,
) -> Result<String, SdkError> {
    if value.chars().any(char::is_control) {
        return Err(invalid(
            code,
            field,
            format!("{label} must not contain control characters"),
        ));
    }
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() || normalized.len() > maximum_bytes {
        return Err(invalid(
            code,
            field,
            format!("{label} must be non-empty and not exceed {maximum_bytes} UTF-8 bytes"),
        ));
    }
    let bytes = normalized.as_bytes();
    if !bytes.first().is_some_and(u8::is_ascii_alphanumeric)
        || !bytes.last().is_some_and(u8::is_ascii_alphanumeric)
        || bytes
            .iter()
            .any(|byte| !(byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-')))
    {
        return Err(invalid(
            code,
            field,
            format!(
                "{label} must start and end with an ASCII letter or digit and contain only ASCII letters, digits, '.', '_' or '-'"
            ),
        ));
    }
    Ok(normalized)
}

fn invalid(code: &'static str, field: &'static str, internal: impl Into<String>) -> SdkError {
    SdkError::new(
        code,
        ErrorCategory::InvalidArgument,
        false,
        "The Identity Resolution candidate data is invalid.",
    )
    .with_internal_reference(internal)
    .with_field_violation(FieldViolation {
        field: FieldName::try_new(field).expect("static Identity Resolution field must be valid"),
        code: "INVALID".to_owned(),
        safe_message: "The Identity Resolution candidate field is invalid.".to_owned(),
    })
}

fn conflict(code: &'static str, internal: impl Into<String>) -> SdkError {
    SdkError::new(
        code,
        ErrorCategory::Conflict,
        false,
        "The Identity Resolution candidate changed before this operation could be applied.",
    )
    .with_internal_reference(internal)
}

trait SdkErrorFieldViolationExt {
    fn with_field_violation(self, violation: FieldViolation) -> Self;
}

impl SdkErrorFieldViolationExt for SdkError {
    fn with_field_violation(mut self, violation: FieldViolation) -> Self {
        self.field_violations.push(violation);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signal(kind: &str, contribution: i16) -> MatchSignal {
        MatchSignal::try_new(
            SignalKindCode::try_new(kind).unwrap(),
            SignalSourceCode::try_new("party.normalized").unwrap(),
            EvidenceReference::try_new(format!("evidence://{kind}")).unwrap(),
            contribution,
        )
        .unwrap()
    }

    fn evidence(
        first: &str,
        first_version: i64,
        second: &str,
        second_version: i64,
        generated_at: i64,
    ) -> MatchEvidenceSnapshot {
        MatchEvidenceSnapshot::try_new(
            PartyReference::try_new(first).unwrap(),
            first_version,
            PartyReference::try_new(second).unwrap(),
            second_version,
            MatcherProfileCode::try_new(" deterministic.v1 ").unwrap(),
            8_500,
            vec![signal("name.exact", 6_000), signal("email.exact", 2_500)],
            generated_at,
        )
        .unwrap()
    }

    fn candidate() -> DuplicateCandidateCase {
        DuplicateCandidateCase::create(CreateDuplicateCandidateCase {
            evidence: evidence("party-b", 3, "party-a", 5, 100),
            occurred_at_unix_nanos: 110,
        })
        .unwrap()
    }

    #[test]
    fn canonical_pair_and_case_id_are_input_order_independent() {
        let first = evidence("party-a", 5, "party-b", 3, 100);
        let second = evidence("party-b", 3, "party-a", 5, 100);
        assert_eq!(first.pair(), second.pair());
        assert_eq!(first.left_party_version(), 5);
        assert_eq!(first.right_party_version(), 3);
        assert_eq!(
            DuplicateCandidateCaseId::for_pair(first.pair()).unwrap(),
            DuplicateCandidateCaseId::for_pair(second.pair()).unwrap()
        );
    }

    #[test]
    fn self_pair_is_rejected() {
        let error = MatchEvidenceSnapshot::try_new(
            PartyReference::try_new("party-a").unwrap(),
            1,
            PartyReference::try_new("party-a").unwrap(),
            1,
            MatcherProfileCode::try_new("deterministic.v1").unwrap(),
            100,
            vec![signal("name.exact", 100)],
            100,
        )
        .unwrap_err();
        assert_eq!(error.code, "IDENTITY_RESOLUTION_SELF_PAIR_INVALID");
    }

    #[test]
    fn evidence_is_canonical_and_duplicate_signal_identity_is_rejected() {
        let value = evidence("party-b", 3, "party-a", 5, 100);
        assert_eq!(value.matcher_profile().as_str(), "deterministic.v1");
        assert_eq!(value.signals()[0].kind().as_str(), "email.exact");
        assert_eq!(value.signals()[1].kind().as_str(), "name.exact");

        let duplicate = MatchEvidenceSnapshot::try_new(
            PartyReference::try_new("party-a").unwrap(),
            1,
            PartyReference::try_new("party-b").unwrap(),
            1,
            MatcherProfileCode::try_new("deterministic.v1").unwrap(),
            100,
            vec![signal("name.exact", 100), signal("name.exact", 200)],
            100,
        )
        .unwrap_err();
        assert_eq!(
            duplicate.code,
            "IDENTITY_RESOLUTION_DUPLICATE_SIGNAL_INVALID"
        );
    }

    #[test]
    fn refresh_appends_evidence_and_requires_source_version_progress() {
        let mut value = candidate();
        value
            .refresh_evidence(RefreshDuplicateCandidateEvidence {
                expected_version: 1,
                evidence: evidence("party-a", 6, "party-b", 3, 200),
                occurred_at_unix_nanos: 210,
            })
            .unwrap();
        assert_eq!(value.version(), 2);
        assert_eq!(value.evidence_history().len(), 2);

        let error = value
            .refresh_evidence(RefreshDuplicateCandidateEvidence {
                expected_version: 2,
                evidence: evidence("party-a", 6, "party-b", 3, 300),
                occurred_at_unix_nanos: 310,
            })
            .unwrap_err();
        assert_eq!(error.code, "IDENTITY_RESOLUTION_STALE_EVIDENCE_VERSION");
    }

    #[test]
    fn refresh_rejects_source_version_regression() {
        let mut value = candidate();
        let error = value
            .refresh_evidence(RefreshDuplicateCandidateEvidence {
                expected_version: 1,
                evidence: evidence("party-a", 4, "party-b", 4, 200),
                occurred_at_unix_nanos: 210,
            })
            .unwrap_err();
        assert_eq!(error.code, "IDENTITY_RESOLUTION_SOURCE_VERSION_REGRESSION");
    }

    #[test]
    fn terminal_decisions_are_exact_versioned_and_irreversible() {
        let mut value = candidate();
        value
            .confirm_duplicate(DecideDuplicateCandidateCase {
                expected_version: 1,
                reason: DecisionReasonCode::try_new("review.exact_identity").unwrap(),
                occurred_at_unix_nanos: 200,
            })
            .unwrap();
        assert_eq!(
            value.status(),
            DuplicateCandidateCaseStatus::ConfirmedDuplicate
        );
        assert_eq!(value.version(), 2);
        assert_eq!(
            value
                .dismiss(DecideDuplicateCandidateCase {
                    expected_version: 2,
                    reason: DecisionReasonCode::try_new("review.not_duplicate").unwrap(),
                    occurred_at_unix_nanos: 300,
                })
                .unwrap_err()
                .code,
            "IDENTITY_RESOLUTION_CASE_TERMINAL"
        );
    }

    #[test]
    fn stale_expected_version_is_conflict_without_mutation() {
        let mut value = candidate();
        let snapshot = value.snapshot();
        assert_eq!(
            value
                .dismiss(DecideDuplicateCandidateCase {
                    expected_version: 2,
                    reason: DecisionReasonCode::try_new("review.not_duplicate").unwrap(),
                    occurred_at_unix_nanos: 200,
                })
                .unwrap_err()
                .code,
            "IDENTITY_RESOLUTION_VERSION_CONFLICT"
        );
        assert_eq!(value.snapshot(), snapshot);
    }

    #[test]
    fn strict_rehydrate_rejects_unreachable_case_id_and_version() {
        let value = candidate();
        let mut snapshot = value.snapshot();
        snapshot.case_id = DuplicateCandidateCaseId::try_new("wrong-case-id").unwrap();
        assert_eq!(
            DuplicateCandidateCase::rehydrate(snapshot)
                .unwrap_err()
                .code,
            "IDENTITY_RESOLUTION_PERSISTED_CASE_ID_INVALID"
        );

        let mut snapshot = value.snapshot();
        snapshot.version = 2;
        assert_eq!(
            DuplicateCandidateCase::rehydrate(snapshot)
                .unwrap_err()
                .code,
            "IDENTITY_RESOLUTION_PERSISTED_VERSION_INVALID"
        );
    }
}
