use crate::domain::{EvidenceReference, PartyReference};
use crm_module_sdk::{ActorId, ErrorCategory, FieldName, FieldViolation, RecordId, SdkError};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const MAX_DECISION_REFERENCE_BYTES: usize = 512;
const MAX_REASON_CODE_BYTES: usize = 128;
const MAX_FIELD_PATH_BYTES: usize = 256;
const MAX_SURVIVORSHIP_SELECTIONS: usize = 128;
const MAX_CANONICAL_RESOLUTION_HOPS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MergeOperationId(RecordId);

impl MergeOperationId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, SdkError> {
        RecordId::try_new(value.into()).map(Self).map_err(|error| {
            invalid(
                "IDENTITY_RESOLUTION_MERGE_OPERATION_ID_INVALID",
                "identity_resolution.merge_operation_id",
                error.to_string(),
            )
        })
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DecisionReference(String);

impl DecisionReference {
    pub fn try_new(value: impl Into<String>) -> Result<Self, SdkError> {
        let value = value.into();
        if value.chars().any(char::is_control) {
            return Err(invalid(
                "IDENTITY_RESOLUTION_DECISION_REFERENCE_INVALID",
                "identity_resolution.merge.decision_ref",
                "decision reference must not contain control characters",
            ));
        }
        let canonical = value.trim().to_owned();
        if canonical.is_empty() || canonical.len() > MAX_DECISION_REFERENCE_BYTES {
            return Err(invalid(
                "IDENTITY_RESOLUTION_DECISION_REFERENCE_INVALID",
                "identity_resolution.merge.decision_ref",
                format!(
                    "decision reference must be non-empty and not exceed {MAX_DECISION_REFERENCE_BYTES} UTF-8 bytes"
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
pub struct LineageDecisionReasonCode(String);

impl LineageDecisionReasonCode {
    pub fn try_new(value: impl Into<String>) -> Result<Self, SdkError> {
        normalize_semantic_identifier(
            &value.into(),
            MAX_REASON_CODE_BYTES,
            "IDENTITY_RESOLUTION_LINEAGE_REASON_INVALID",
            "identity_resolution.merge.reason",
            "lineage decision reason",
        )
        .map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FieldPath(String);

impl FieldPath {
    pub fn try_new(value: impl Into<String>) -> Result<Self, SdkError> {
        normalize_semantic_identifier(
            &value.into(),
            MAX_FIELD_PATH_BYTES,
            "IDENTITY_RESOLUTION_FIELD_PATH_INVALID",
            "identity_resolution.merge.survivorship.field_path",
            "field path",
        )
        .map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceValueDigest([u8; 32]);

impl SourceValueDigest {
    pub fn sha256(bytes: impl AsRef<[u8]>) -> Self {
        Self(Sha256::digest(bytes.as_ref()).into())
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurvivorshipSelection {
    field_path: FieldPath,
    provenance_party_ref: PartyReference,
    provenance_party_version: i64,
    source_value_digest: SourceValueDigest,
    evidence_ref: EvidenceReference,
}

impl SurvivorshipSelection {
    pub fn try_new(
        field_path: FieldPath,
        provenance_party_ref: PartyReference,
        provenance_party_version: i64,
        source_value_digest: SourceValueDigest,
        evidence_ref: EvidenceReference,
    ) -> Result<Self, SdkError> {
        validate_positive_party_version(
            "identity_resolution.merge.survivorship.provenance_party_version",
            provenance_party_version,
        )?;
        Ok(Self {
            field_path,
            provenance_party_ref,
            provenance_party_version,
            source_value_digest,
            evidence_ref,
        })
    }

    pub fn field_path(&self) -> &FieldPath {
        &self.field_path
    }

    pub fn provenance_party_ref(&self) -> &PartyReference {
        &self.provenance_party_ref
    }

    pub fn provenance_party_version(&self) -> i64 {
        self.provenance_party_version
    }

    pub fn source_value_digest(&self) -> SourceValueDigest {
        self.source_value_digest
    }

    pub fn evidence_ref(&self) -> &EvidenceReference {
        &self.evidence_ref
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MergeOperationStatus {
    Active,
    Unmerged,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnmergeDecision {
    decision_ref: DecisionReference,
    decided_by: ActorId,
    reason: LineageDecisionReasonCode,
    occurred_at_unix_nanos: i64,
}

impl UnmergeDecision {
    pub fn decision_ref(&self) -> &DecisionReference {
        &self.decision_ref
    }

    pub fn decided_by(&self) -> &ActorId {
        &self.decided_by
    }

    pub fn reason(&self) -> &LineageDecisionReasonCode {
        &self.reason
    }

    pub fn occurred_at_unix_nanos(&self) -> i64 {
        self.occurred_at_unix_nanos
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeOperation {
    operation_id: MergeOperationId,
    source_party_ref: PartyReference,
    source_party_version: i64,
    survivor_party_ref: PartyReference,
    survivor_party_version: i64,
    decision_ref: DecisionReference,
    decided_by: ActorId,
    reason: LineageDecisionReasonCode,
    survivorship: Vec<SurvivorshipSelection>,
    status: MergeOperationStatus,
    unmerge_decision: Option<UnmergeDecision>,
    created_at_unix_nanos: i64,
    updated_at_unix_nanos: i64,
    version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeOperationSnapshot {
    pub operation_id: MergeOperationId,
    pub source_party_ref: PartyReference,
    pub source_party_version: i64,
    pub survivor_party_ref: PartyReference,
    pub survivor_party_version: i64,
    pub decision_ref: DecisionReference,
    pub decided_by: ActorId,
    pub reason: LineageDecisionReasonCode,
    pub survivorship: Vec<SurvivorshipSelection>,
    pub status: MergeOperationStatus,
    pub unmerge_decision: Option<UnmergeDecision>,
    pub created_at_unix_nanos: i64,
    pub updated_at_unix_nanos: i64,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateMergeOperation {
    pub operation_id: MergeOperationId,
    pub source_party_ref: PartyReference,
    pub source_party_version: i64,
    pub survivor_party_ref: PartyReference,
    pub survivor_party_version: i64,
    pub decision_ref: DecisionReference,
    pub decided_by: ActorId,
    pub reason: LineageDecisionReasonCode,
    pub survivorship: Vec<SurvivorshipSelection>,
    pub occurred_at_unix_nanos: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnmergeMergeOperation {
    pub expected_version: i64,
    pub decision_ref: DecisionReference,
    pub decided_by: ActorId,
    pub reason: LineageDecisionReasonCode,
    pub occurred_at_unix_nanos: i64,
}

impl MergeOperation {
    pub fn create(mut command: CreateMergeOperation) -> Result<Self, SdkError> {
        validate_distinct_parties(&command.source_party_ref, &command.survivor_party_ref)?;
        validate_positive_party_version(
            "identity_resolution.merge.source_party_version",
            command.source_party_version,
        )?;
        validate_positive_party_version(
            "identity_resolution.merge.survivor_party_version",
            command.survivor_party_version,
        )?;
        validate_timestamp(
            "identity_resolution.merge.occurred_at_unix_nanos",
            command.occurred_at_unix_nanos,
        )?;
        normalize_survivorship(&mut command.survivorship)?;

        Ok(Self {
            operation_id: command.operation_id,
            source_party_ref: command.source_party_ref,
            source_party_version: command.source_party_version,
            survivor_party_ref: command.survivor_party_ref,
            survivor_party_version: command.survivor_party_version,
            decision_ref: command.decision_ref,
            decided_by: command.decided_by,
            reason: command.reason,
            survivorship: command.survivorship,
            status: MergeOperationStatus::Active,
            unmerge_decision: None,
            created_at_unix_nanos: command.occurred_at_unix_nanos,
            updated_at_unix_nanos: command.occurred_at_unix_nanos,
            version: 1,
        })
    }

    pub fn rehydrate(mut snapshot: MergeOperationSnapshot) -> Result<Self, SdkError> {
        validate_distinct_parties(&snapshot.source_party_ref, &snapshot.survivor_party_ref)?;
        validate_positive_party_version(
            "identity_resolution.merge.source_party_version",
            snapshot.source_party_version,
        )?;
        validate_positive_party_version(
            "identity_resolution.merge.survivor_party_version",
            snapshot.survivor_party_version,
        )?;
        validate_timestamp(
            "identity_resolution.merge.created_at_unix_nanos",
            snapshot.created_at_unix_nanos,
        )?;
        validate_timestamp(
            "identity_resolution.merge.updated_at_unix_nanos",
            snapshot.updated_at_unix_nanos,
        )?;
        normalize_survivorship(&mut snapshot.survivorship)?;

        match snapshot.status {
            MergeOperationStatus::Active => {
                if snapshot.unmerge_decision.is_some()
                    || snapshot.version != 1
                    || snapshot.updated_at_unix_nanos != snapshot.created_at_unix_nanos
                {
                    return Err(invalid(
                        "IDENTITY_RESOLUTION_MERGE_PERSISTED_STATE_INVALID",
                        "identity_resolution.merge.status",
                        "an active merge operation must be version 1 with no unmerge decision and unchanged mutation time",
                    ));
                }
            }
            MergeOperationStatus::Unmerged => {
                let decision = snapshot.unmerge_decision.as_ref().ok_or_else(|| {
                    invalid(
                        "IDENTITY_RESOLUTION_MERGE_PERSISTED_STATE_INVALID",
                        "identity_resolution.merge.unmerge_decision",
                        "an unmerged merge operation must contain unmerge decision evidence",
                    )
                })?;
                validate_timestamp(
                    "identity_resolution.merge.unmerge_decision.occurred_at_unix_nanos",
                    decision.occurred_at_unix_nanos,
                )?;
                if snapshot.version != 2
                    || decision.occurred_at_unix_nanos != snapshot.updated_at_unix_nanos
                    || snapshot.updated_at_unix_nanos <= snapshot.created_at_unix_nanos
                {
                    return Err(invalid(
                        "IDENTITY_RESOLUTION_MERGE_PERSISTED_STATE_INVALID",
                        "identity_resolution.merge.version",
                        "an unmerged merge operation must be version 2 and advance exactly to the recorded unmerge decision time",
                    ));
                }
            }
        }

        Ok(Self {
            operation_id: snapshot.operation_id,
            source_party_ref: snapshot.source_party_ref,
            source_party_version: snapshot.source_party_version,
            survivor_party_ref: snapshot.survivor_party_ref,
            survivor_party_version: snapshot.survivor_party_version,
            decision_ref: snapshot.decision_ref,
            decided_by: snapshot.decided_by,
            reason: snapshot.reason,
            survivorship: snapshot.survivorship,
            status: snapshot.status,
            unmerge_decision: snapshot.unmerge_decision,
            created_at_unix_nanos: snapshot.created_at_unix_nanos,
            updated_at_unix_nanos: snapshot.updated_at_unix_nanos,
            version: snapshot.version,
        })
    }

    pub fn unmerge(&mut self, command: UnmergeMergeOperation) -> Result<(), SdkError> {
        if command.expected_version != self.version {
            return Err(conflict(
                "IDENTITY_RESOLUTION_MERGE_VERSION_CONFLICT",
                format!(
                    "expected merge operation version {}, current version is {}",
                    command.expected_version, self.version
                ),
            ));
        }
        if self.status != MergeOperationStatus::Active {
            return Err(conflict(
                "IDENTITY_RESOLUTION_MERGE_ALREADY_UNMERGED",
                "an unmerged operation cannot be unmerged again or reactivated",
            ));
        }
        validate_timestamp(
            "identity_resolution.merge.unmerge.occurred_at_unix_nanos",
            command.occurred_at_unix_nanos,
        )?;
        if command.occurred_at_unix_nanos <= self.updated_at_unix_nanos {
            return Err(conflict(
                "IDENTITY_RESOLUTION_MERGE_TIME_CONFLICT",
                "unmerge time must be strictly later than the previous governed mutation time",
            ));
        }

        self.status = MergeOperationStatus::Unmerged;
        self.unmerge_decision = Some(UnmergeDecision {
            decision_ref: command.decision_ref,
            decided_by: command.decided_by,
            reason: command.reason,
            occurred_at_unix_nanos: command.occurred_at_unix_nanos,
        });
        self.updated_at_unix_nanos = command.occurred_at_unix_nanos;
        self.version = 2;
        Ok(())
    }

    pub fn operation_id(&self) -> &MergeOperationId {
        &self.operation_id
    }

    pub fn source_party_ref(&self) -> &PartyReference {
        &self.source_party_ref
    }

    pub fn source_party_version(&self) -> i64 {
        self.source_party_version
    }

    pub fn survivor_party_ref(&self) -> &PartyReference {
        &self.survivor_party_ref
    }

    pub fn survivor_party_version(&self) -> i64 {
        self.survivor_party_version
    }

    pub fn decision_ref(&self) -> &DecisionReference {
        &self.decision_ref
    }

    pub fn decided_by(&self) -> &ActorId {
        &self.decided_by
    }

    pub fn reason(&self) -> &LineageDecisionReasonCode {
        &self.reason
    }

    pub fn survivorship(&self) -> &[SurvivorshipSelection] {
        &self.survivorship
    }

    pub fn status(&self) -> MergeOperationStatus {
        self.status
    }

    pub fn unmerge_decision(&self) -> Option<&UnmergeDecision> {
        self.unmerge_decision.as_ref()
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

    pub fn snapshot(&self) -> MergeOperationSnapshot {
        MergeOperationSnapshot {
            operation_id: self.operation_id.clone(),
            source_party_ref: self.source_party_ref.clone(),
            source_party_version: self.source_party_version,
            survivor_party_ref: self.survivor_party_ref.clone(),
            survivor_party_version: self.survivor_party_version,
            decision_ref: self.decision_ref.clone(),
            decided_by: self.decided_by.clone(),
            reason: self.reason.clone(),
            survivorship: self.survivorship.clone(),
            status: self.status,
            unmerge_decision: self.unmerge_decision.clone(),
            created_at_unix_nanos: self.created_at_unix_nanos,
            updated_at_unix_nanos: self.updated_at_unix_nanos,
            version: self.version,
        }
    }

    pub fn active_edge(&self) -> Option<ActiveMergeEdge> {
        (self.status == MergeOperationStatus::Active).then(|| ActiveMergeEdge {
            operation_id: self.operation_id.clone(),
            source_party_ref: self.source_party_ref.clone(),
            survivor_party_ref: self.survivor_party_ref.clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveMergeEdge {
    operation_id: MergeOperationId,
    source_party_ref: PartyReference,
    survivor_party_ref: PartyReference,
}

impl ActiveMergeEdge {
    pub fn try_new(
        operation_id: MergeOperationId,
        source_party_ref: PartyReference,
        survivor_party_ref: PartyReference,
    ) -> Result<Self, SdkError> {
        validate_distinct_parties(&source_party_ref, &survivor_party_ref)?;
        Ok(Self {
            operation_id,
            source_party_ref,
            survivor_party_ref,
        })
    }

    pub fn operation_id(&self) -> &MergeOperationId {
        &self.operation_id
    }

    pub fn source_party_ref(&self) -> &PartyReference {
        &self.source_party_ref
    }

    pub fn survivor_party_ref(&self) -> &PartyReference {
        &self.survivor_party_ref
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalResolution {
    requested_party_ref: PartyReference,
    canonical_party_ref: PartyReference,
    party_path: Vec<PartyReference>,
    merge_operation_path: Vec<MergeOperationId>,
}

impl CanonicalResolution {
    pub fn requested_party_ref(&self) -> &PartyReference {
        &self.requested_party_ref
    }

    pub fn canonical_party_ref(&self) -> &PartyReference {
        &self.canonical_party_ref
    }

    pub fn party_path(&self) -> &[PartyReference] {
        &self.party_path
    }

    pub fn merge_operation_path(&self) -> &[MergeOperationId] {
        &self.merge_operation_path
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CanonicalPartyGraph {
    edges_by_source: BTreeMap<PartyReference, ActiveMergeEdge>,
    operation_ids: BTreeSet<MergeOperationId>,
}

impl CanonicalPartyGraph {
    pub fn try_new(edges: impl IntoIterator<Item = ActiveMergeEdge>) -> Result<Self, SdkError> {
        let mut graph = Self::default();
        for edge in edges {
            if !graph.operation_ids.insert(edge.operation_id.clone()) {
                return Err(invalid(
                    "IDENTITY_RESOLUTION_MERGE_GRAPH_DUPLICATE_OPERATION",
                    "identity_resolution.merge_graph.operation_id",
                    "an active merge operation may appear only once in the canonical graph",
                ));
            }
            if graph
                .edges_by_source
                .insert(edge.source_party_ref.clone(), edge)
                .is_some()
            {
                return Err(invalid(
                    "IDENTITY_RESOLUTION_MERGE_GRAPH_DUPLICATE_SOURCE",
                    "identity_resolution.merge_graph.source_party_ref",
                    "a Party may have at most one active outgoing merge edge",
                ));
            }
        }

        for source in graph.edges_by_source.keys() {
            graph.resolve(source)?;
        }
        Ok(graph)
    }

    pub fn resolve(&self, party_ref: &PartyReference) -> Result<CanonicalResolution, SdkError> {
        let requested_party_ref = party_ref.clone();
        let mut current = party_ref.clone();
        let mut party_path = vec![current.clone()];
        let mut merge_operation_path = Vec::new();
        let mut visited = BTreeSet::from([current.clone()]);

        while let Some(edge) = self.edges_by_source.get(&current) {
            if merge_operation_path.len() >= MAX_CANONICAL_RESOLUTION_HOPS {
                return Err(invalid(
                    "IDENTITY_RESOLUTION_MERGE_GRAPH_DEPTH_EXCEEDED",
                    "identity_resolution.merge_graph",
                    format!(
                        "canonical resolution exceeds the supported {MAX_CANONICAL_RESOLUTION_HOPS}-hop bound"
                    ),
                ));
            }
            merge_operation_path.push(edge.operation_id.clone());
            current = edge.survivor_party_ref.clone();
            if !visited.insert(current.clone()) {
                return Err(invalid(
                    "IDENTITY_RESOLUTION_MERGE_GRAPH_CYCLE",
                    "identity_resolution.merge_graph",
                    "active merge edges must not contain a cycle",
                ));
            }
            party_path.push(current.clone());
        }

        Ok(CanonicalResolution {
            requested_party_ref,
            canonical_party_ref: current,
            party_path,
            merge_operation_path,
        })
    }

    pub fn validate_new_merge(
        &self,
        source_party_ref: &PartyReference,
        survivor_party_ref: &PartyReference,
    ) -> Result<(), SdkError> {
        validate_distinct_parties(source_party_ref, survivor_party_ref)?;

        let source_resolution = self.resolve(source_party_ref)?;
        if source_resolution.canonical_party_ref() != source_party_ref {
            return Err(conflict(
                "IDENTITY_RESOLUTION_MERGE_SOURCE_ALREADY_REDIRECTED",
                "the proposed merge source already redirects to another canonical Party",
            ));
        }

        let survivor_resolution = self.resolve(survivor_party_ref)?;
        if survivor_resolution.canonical_party_ref() != survivor_party_ref {
            return Err(conflict(
                "IDENTITY_RESOLUTION_MERGE_SURVIVOR_NOT_CANONICAL",
                "the proposed survivor already redirects to another canonical Party",
            ));
        }
        Ok(())
    }

    pub fn with_added_edge(&self, edge: ActiveMergeEdge) -> Result<Self, SdkError> {
        self.validate_new_merge(&edge.source_party_ref, &edge.survivor_party_ref)?;
        if self.operation_ids.contains(&edge.operation_id) {
            return Err(conflict(
                "IDENTITY_RESOLUTION_MERGE_OPERATION_ALREADY_ACTIVE",
                "the merge operation is already active in the canonical graph",
            ));
        }
        let mut edges = self.edges_by_source.values().cloned().collect::<Vec<_>>();
        edges.push(edge);
        Self::try_new(edges)
    }

    pub fn without_operation(&self, operation_id: &MergeOperationId) -> Result<Self, SdkError> {
        let mut removed = false;
        let edges = self
            .edges_by_source
            .values()
            .filter_map(|edge| {
                if &edge.operation_id == operation_id {
                    removed = true;
                    None
                } else {
                    Some(edge.clone())
                }
            })
            .collect::<Vec<_>>();
        if !removed {
            return Err(conflict(
                "IDENTITY_RESOLUTION_MERGE_OPERATION_NOT_ACTIVE",
                "the requested merge operation is not active in the canonical graph",
            ));
        }
        Self::try_new(edges)
    }

    pub fn active_edge_for_source(&self, party_ref: &PartyReference) -> Option<&ActiveMergeEdge> {
        self.edges_by_source.get(party_ref)
    }

    pub fn active_edge_count(&self) -> usize {
        self.edges_by_source.len()
    }
}

fn normalize_survivorship(selections: &mut Vec<SurvivorshipSelection>) -> Result<(), SdkError> {
    if selections.len() > MAX_SURVIVORSHIP_SELECTIONS {
        return Err(invalid(
            "IDENTITY_RESOLUTION_SURVIVORSHIP_LIMIT_EXCEEDED",
            "identity_resolution.merge.survivorship",
            format!(
                "a merge operation supports at most {MAX_SURVIVORSHIP_SELECTIONS} field survivorship selections"
            ),
        ));
    }
    selections.sort_by(|left, right| left.field_path.cmp(&right.field_path));
    for window in selections.windows(2) {
        if window[0].field_path == window[1].field_path {
            return Err(invalid(
                "IDENTITY_RESOLUTION_SURVIVORSHIP_DUPLICATE_FIELD",
                "identity_resolution.merge.survivorship.field_path",
                "a field may have at most one survivorship selection in one merge operation",
            ));
        }
    }
    Ok(())
}

fn validate_distinct_parties(
    source_party_ref: &PartyReference,
    survivor_party_ref: &PartyReference,
) -> Result<(), SdkError> {
    if source_party_ref == survivor_party_ref {
        return Err(invalid(
            "IDENTITY_RESOLUTION_SELF_MERGE_INVALID",
            "identity_resolution.merge.party_refs",
            "a Party cannot be merged into itself",
        ));
    }
    Ok(())
}

fn validate_positive_party_version(field: &'static str, value: i64) -> Result<(), SdkError> {
    if value <= 0 {
        return Err(invalid(
            "IDENTITY_RESOLUTION_MERGE_PARTY_VERSION_INVALID",
            field,
            "Party version must be positive",
        ));
    }
    Ok(())
}

fn validate_timestamp(field: &'static str, value: i64) -> Result<(), SdkError> {
    if value <= 0 {
        return Err(invalid(
            "IDENTITY_RESOLUTION_MERGE_TIME_INVALID",
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
        "The Identity Resolution merge-lineage data is invalid.",
    )
    .with_internal_reference(internal)
    .with_field_violation(FieldViolation {
        field: FieldName::try_new(field).expect("static Identity Resolution merge field must be valid"),
        code: "INVALID".to_owned(),
        safe_message: "The Identity Resolution merge-lineage field is invalid.".to_owned(),
    })
}

fn conflict(code: &'static str, internal: impl Into<String>) -> SdkError {
    SdkError::new(
        code,
        ErrorCategory::Conflict,
        false,
        "The Identity Resolution merge lineage changed before this operation could be applied.",
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

    fn party(value: &str) -> PartyReference {
        PartyReference::try_new(value).unwrap()
    }

    fn actor(value: &str) -> ActorId {
        ActorId::try_new(value).unwrap()
    }

    fn operation_id(value: &str) -> MergeOperationId {
        MergeOperationId::try_new(value).unwrap()
    }

    fn selection(field: &str, provenance_party: &str, version: i64) -> SurvivorshipSelection {
        SurvivorshipSelection::try_new(
            FieldPath::try_new(field).unwrap(),
            party(provenance_party),
            version,
            SourceValueDigest::sha256(format!("{field}:{provenance_party}:{version}")),
            EvidenceReference::try_new(format!("evidence://{field}/{provenance_party}/{version}"))
                .unwrap(),
        )
        .unwrap()
    }

    fn merge_operation() -> MergeOperation {
        MergeOperation::create(CreateMergeOperation {
            operation_id: operation_id("merge-op-1"),
            source_party_ref: party("party-a"),
            source_party_version: 3,
            survivor_party_ref: party("party-b"),
            survivor_party_version: 5,
            decision_ref: DecisionReference::try_new("approval://merge/1").unwrap(),
            decided_by: actor("reviewer-a"),
            reason: LineageDecisionReasonCode::try_new("duplicate.confirmed").unwrap(),
            survivorship: vec![
                selection("display_name", "party-b", 5),
                selection("custom.vip_tier", "party-a", 3),
            ],
            occurred_at_unix_nanos: 100,
        })
        .unwrap()
    }

    #[test]
    fn merge_creation_is_deterministic_and_preserves_provenance() {
        let operation = merge_operation();
        assert_eq!(operation.status(), MergeOperationStatus::Active);
        assert_eq!(operation.version(), 1);
        assert_eq!(operation.source_party_ref().as_str(), "party-a");
        assert_eq!(operation.survivor_party_ref().as_str(), "party-b");
        assert_eq!(operation.survivorship()[0].field_path().as_str(), "custom.vip_tier");
        assert_eq!(operation.survivorship()[1].field_path().as_str(), "display_name");
        assert!(operation.active_edge().is_some());
    }

    #[test]
    fn self_merge_and_duplicate_survivorship_fields_are_rejected() {
        let self_merge = MergeOperation::create(CreateMergeOperation {
            operation_id: operation_id("merge-op-self"),
            source_party_ref: party("party-a"),
            source_party_version: 1,
            survivor_party_ref: party("party-a"),
            survivor_party_version: 1,
            decision_ref: DecisionReference::try_new("approval://merge/self").unwrap(),
            decided_by: actor("reviewer-a"),
            reason: LineageDecisionReasonCode::try_new("duplicate.confirmed").unwrap(),
            survivorship: Vec::new(),
            occurred_at_unix_nanos: 100,
        })
        .unwrap_err();
        assert_eq!(self_merge.code.as_str(), "IDENTITY_RESOLUTION_SELF_MERGE_INVALID");

        let duplicate_field = MergeOperation::create(CreateMergeOperation {
            operation_id: operation_id("merge-op-duplicate-field"),
            source_party_ref: party("party-a"),
            source_party_version: 1,
            survivor_party_ref: party("party-b"),
            survivor_party_version: 1,
            decision_ref: DecisionReference::try_new("approval://merge/duplicate-field").unwrap(),
            decided_by: actor("reviewer-a"),
            reason: LineageDecisionReasonCode::try_new("duplicate.confirmed").unwrap(),
            survivorship: vec![
                selection("display_name", "party-a", 1),
                selection("display_name", "party-b", 1),
            ],
            occurred_at_unix_nanos: 100,
        })
        .unwrap_err();
        assert_eq!(
            duplicate_field.code.as_str(),
            "IDENTITY_RESOLUTION_SURVIVORSHIP_DUPLICATE_FIELD"
        );
    }

    #[test]
    fn unmerge_is_exact_versioned_and_irreversible() {
        let mut operation = merge_operation();
        let stale = operation
            .unmerge(UnmergeMergeOperation {
                expected_version: 2,
                decision_ref: DecisionReference::try_new("approval://unmerge/stale").unwrap(),
                decided_by: actor("reviewer-b"),
                reason: LineageDecisionReasonCode::try_new("merge.reversed").unwrap(),
                occurred_at_unix_nanos: 200,
            })
            .unwrap_err();
        assert_eq!(stale.code.as_str(), "IDENTITY_RESOLUTION_MERGE_VERSION_CONFLICT");

        operation
            .unmerge(UnmergeMergeOperation {
                expected_version: 1,
                decision_ref: DecisionReference::try_new("approval://unmerge/1").unwrap(),
                decided_by: actor("reviewer-b"),
                reason: LineageDecisionReasonCode::try_new("merge.reversed").unwrap(),
                occurred_at_unix_nanos: 200,
            })
            .unwrap();
        assert_eq!(operation.status(), MergeOperationStatus::Unmerged);
        assert_eq!(operation.version(), 2);
        assert!(operation.active_edge().is_none());

        let repeated = operation
            .unmerge(UnmergeMergeOperation {
                expected_version: 2,
                decision_ref: DecisionReference::try_new("approval://unmerge/repeat").unwrap(),
                decided_by: actor("reviewer-b"),
                reason: LineageDecisionReasonCode::try_new("merge.reversed").unwrap(),
                occurred_at_unix_nanos: 300,
            })
            .unwrap_err();
        assert_eq!(repeated.code.as_str(), "IDENTITY_RESOLUTION_MERGE_ALREADY_UNMERGED");
    }

    #[test]
    fn canonical_graph_resolves_chains_and_unmerge_restores_prior_root() {
        let graph = CanonicalPartyGraph::try_new([
            ActiveMergeEdge::try_new(operation_id("merge-a-b"), party("party-a"), party("party-b"))
                .unwrap(),
            ActiveMergeEdge::try_new(operation_id("merge-b-c"), party("party-b"), party("party-c"))
                .unwrap(),
        ])
        .unwrap();

        let resolution = graph.resolve(&party("party-a")).unwrap();
        assert_eq!(resolution.canonical_party_ref().as_str(), "party-c");
        assert_eq!(
            resolution
                .party_path()
                .iter()
                .map(PartyReference::as_str)
                .collect::<Vec<_>>(),
            vec!["party-a", "party-b", "party-c"]
        );

        let restored = graph.without_operation(&operation_id("merge-b-c")).unwrap();
        assert_eq!(
            restored
                .resolve(&party("party-a"))
                .unwrap()
                .canonical_party_ref()
                .as_str(),
            "party-b"
        );
        assert_eq!(
            restored
                .resolve(&party("party-c"))
                .unwrap()
                .canonical_party_ref()
                .as_str(),
            "party-c"
        );
    }

    #[test]
    fn canonical_graph_rejects_duplicate_sources_and_cycles() {
        let duplicate_source = CanonicalPartyGraph::try_new([
            ActiveMergeEdge::try_new(operation_id("merge-a-b"), party("party-a"), party("party-b"))
                .unwrap(),
            ActiveMergeEdge::try_new(operation_id("merge-a-c"), party("party-a"), party("party-c"))
                .unwrap(),
        ])
        .unwrap_err();
        assert_eq!(
            duplicate_source.code.as_str(),
            "IDENTITY_RESOLUTION_MERGE_GRAPH_DUPLICATE_SOURCE"
        );

        let cycle = CanonicalPartyGraph::try_new([
            ActiveMergeEdge::try_new(operation_id("merge-a-b"), party("party-a"), party("party-b"))
                .unwrap(),
            ActiveMergeEdge::try_new(operation_id("merge-b-a"), party("party-b"), party("party-a"))
                .unwrap(),
        ])
        .unwrap_err();
        assert_eq!(cycle.code.as_str(), "IDENTITY_RESOLUTION_MERGE_GRAPH_CYCLE");
    }

    #[test]
    fn new_merge_requires_both_endpoints_to_be_current_roots() {
        let graph = CanonicalPartyGraph::try_new([
            ActiveMergeEdge::try_new(operation_id("merge-a-b"), party("party-a"), party("party-b"))
                .unwrap(),
        ])
        .unwrap();

        let redirected_source = graph
            .validate_new_merge(&party("party-a"), &party("party-c"))
            .unwrap_err();
        assert_eq!(
            redirected_source.code.as_str(),
            "IDENTITY_RESOLUTION_MERGE_SOURCE_ALREADY_REDIRECTED"
        );

        let redirected_survivor = graph
            .validate_new_merge(&party("party-c"), &party("party-a"))
            .unwrap_err();
        assert_eq!(
            redirected_survivor.code.as_str(),
            "IDENTITY_RESOLUTION_MERGE_SURVIVOR_NOT_CANONICAL"
        );

        graph
            .validate_new_merge(&party("party-b"), &party("party-c"))
            .unwrap();
    }

    #[test]
    fn persisted_lifecycle_shape_is_strict() {
        let operation = merge_operation();
        let mut invalid_snapshot = operation.snapshot();
        invalid_snapshot.status = MergeOperationStatus::Unmerged;
        let error = MergeOperation::rehydrate(invalid_snapshot).unwrap_err();
        assert_eq!(
            error.code.as_str(),
            "IDENTITY_RESOLUTION_MERGE_PERSISTED_STATE_INVALID"
        );
    }

    #[test]
    fn source_value_digest_is_deterministic() {
        assert_eq!(
            SourceValueDigest::sha256("same-value"),
            SourceValueDigest::sha256(b"same-value")
        );
        assert_ne!(
            SourceValueDigest::sha256("same-value"),
            SourceValueDigest::sha256("different-value")
        );
    }
}
