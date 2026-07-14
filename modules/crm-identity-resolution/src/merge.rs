use crate::{CanonicalPartyPair, DuplicateCandidateCaseId, PartyReference};
use crm_module_sdk::{ErrorCategory, RecordId, SdkError};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;

const MERGE_ID_DOMAIN: &[u8] = b"crm.identity_resolution.merge_lineage_id/v1";
const MAX_DISPLAY_NAME_BYTES: usize = 240;
const MAX_ACTOR_REFERENCE_BYTES: usize = 256;
const MAX_REASON_CODE_BYTES: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MergeLineageId(RecordId);

impl MergeLineageId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, SdkError> {
        RecordId::try_new(value.into()).map(Self).map_err(|error| {
            invalid(
                "IDENTITY_RESOLUTION_MERGE_ID_INVALID",
                "identity_resolution.merge.merge_id",
                error.to_string(),
            )
        })
    }

    pub fn for_operation(
        candidate_case_id: &DuplicateCandidateCaseId,
        candidate_case_version: i64,
        survivor_party_ref: &PartyReference,
        absorbed_party_ref: &PartyReference,
    ) -> Result<Self, SdkError> {
        validate_positive_version(
            "identity_resolution.merge.candidate_case_version",
            candidate_case_version,
        )?;
        if survivor_party_ref == absorbed_party_ref {
            return Err(invalid(
                "IDENTITY_RESOLUTION_MERGE_SELF_INVALID",
                "identity_resolution.merge.party_pair",
                "a Party cannot be merged into itself",
            ));
        }

        let mut hasher = Sha256::new();
        hasher.update(MERGE_ID_DOMAIN);
        hash_field(&mut hasher, candidate_case_id.as_str().as_bytes());
        hash_field(&mut hasher, candidate_case_version.to_string().as_bytes());
        hash_field(&mut hasher, survivor_party_ref.as_str().as_bytes());
        hash_field(&mut hasher, absorbed_party_ref.as_str().as_bytes());
        let digest = hasher.finalize();
        let mut hex = String::with_capacity(digest.len() * 2);
        for byte in digest {
            write!(&mut hex, "{byte:02x}").expect("writing to String cannot fail");
        }
        Self::try_new(format!("idrm-{hex}"))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MergePartyKind {
    Person,
    Organization,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SurvivorshipSource {
    Survivor,
    Absorbed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayNameSurvivorship {
    chosen_source: SurvivorshipSource,
    survivor_value: String,
    absorbed_value: String,
}

impl DisplayNameSurvivorship {
    pub fn try_new(
        chosen_source: SurvivorshipSource,
        survivor_value: impl Into<String>,
        absorbed_value: impl Into<String>,
    ) -> Result<Self, SdkError> {
        Ok(Self {
            chosen_source,
            survivor_value: normalize_display_name(
                survivor_value.into(),
                "identity_resolution.merge.survivorship.display_name.survivor_value",
            )?,
            absorbed_value: normalize_display_name(
                absorbed_value.into(),
                "identity_resolution.merge.survivorship.display_name.absorbed_value",
            )?,
        })
    }

    pub const fn chosen_source(&self) -> SurvivorshipSource {
        self.chosen_source
    }

    pub fn survivor_value(&self) -> &str {
        &self.survivor_value
    }

    pub fn absorbed_value(&self) -> &str {
        &self.absorbed_value
    }

    pub fn chosen_value(&self) -> &str {
        match self.chosen_source {
            SurvivorshipSource::Survivor => &self.survivor_value,
            SurvivorshipSource::Absorbed => &self.absorbed_value,
        }
    }

    pub fn changes_survivor(&self) -> bool {
        self.chosen_value() != self.survivor_value
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MergeActorReference(String);

impl MergeActorReference {
    pub fn try_new(value: impl Into<String>) -> Result<Self, SdkError> {
        let value = value.into();
        if value.chars().any(char::is_control) {
            return Err(invalid(
                "IDENTITY_RESOLUTION_MERGE_ACTOR_INVALID",
                "identity_resolution.merge.actor_ref",
                "actor reference must not contain control characters",
            ));
        }
        let canonical = value.trim().to_owned();
        if canonical.is_empty() || canonical.len() > MAX_ACTOR_REFERENCE_BYTES {
            return Err(invalid(
                "IDENTITY_RESOLUTION_MERGE_ACTOR_INVALID",
                "identity_resolution.merge.actor_ref",
                format!(
                    "actor reference must be non-empty and not exceed {MAX_ACTOR_REFERENCE_BYTES} UTF-8 bytes"
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
pub struct UnmergeReasonCode(String);

impl UnmergeReasonCode {
    pub fn try_new(value: impl Into<String>) -> Result<Self, SdkError> {
        normalize_reason_code(value.into()).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MergeLineageStatus {
    Active,
    Unmerged,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnmergeDecision {
    actor_ref: MergeActorReference,
    reason: UnmergeReasonCode,
    survivor_pre_unmerge_version: i64,
    absorbed_pre_unmerge_version: i64,
    survivor_post_unmerge_version: i64,
    absorbed_post_unmerge_version: i64,
    occurred_at_unix_nanos: i64,
}

impl UnmergeDecision {
    pub fn actor_ref(&self) -> &MergeActorReference {
        &self.actor_ref
    }

    pub fn reason(&self) -> &UnmergeReasonCode {
        &self.reason
    }

    pub fn survivor_pre_unmerge_version(&self) -> i64 {
        self.survivor_pre_unmerge_version
    }

    pub fn absorbed_pre_unmerge_version(&self) -> i64 {
        self.absorbed_pre_unmerge_version
    }

    pub fn survivor_post_unmerge_version(&self) -> i64 {
        self.survivor_post_unmerge_version
    }

    pub fn absorbed_post_unmerge_version(&self) -> i64 {
        self.absorbed_post_unmerge_version
    }

    pub fn occurred_at_unix_nanos(&self) -> i64 {
        self.occurred_at_unix_nanos
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyMergeLineage {
    merge_id: MergeLineageId,
    candidate_case_id: DuplicateCandidateCaseId,
    candidate_case_version: i64,
    pair: CanonicalPartyPair,
    survivor_party_ref: PartyReference,
    absorbed_party_ref: PartyReference,
    party_kind: MergePartyKind,
    survivor_pre_merge_version: i64,
    absorbed_pre_merge_version: i64,
    survivor_post_merge_version: i64,
    absorbed_post_merge_version: i64,
    display_name_survivorship: DisplayNameSurvivorship,
    merge_actor_ref: MergeActorReference,
    merged_at_unix_nanos: i64,
    status: MergeLineageStatus,
    unmerge_decision: Option<UnmergeDecision>,
    updated_at_unix_nanos: i64,
    version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyMergeLineageSnapshot {
    pub merge_id: MergeLineageId,
    pub candidate_case_id: DuplicateCandidateCaseId,
    pub candidate_case_version: i64,
    pub pair: CanonicalPartyPair,
    pub survivor_party_ref: PartyReference,
    pub absorbed_party_ref: PartyReference,
    pub party_kind: MergePartyKind,
    pub survivor_pre_merge_version: i64,
    pub absorbed_pre_merge_version: i64,
    pub survivor_post_merge_version: i64,
    pub absorbed_post_merge_version: i64,
    pub display_name_survivorship: DisplayNameSurvivorship,
    pub merge_actor_ref: MergeActorReference,
    pub merged_at_unix_nanos: i64,
    pub status: MergeLineageStatus,
    pub unmerge_decision: Option<UnmergeDecision>,
    pub updated_at_unix_nanos: i64,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatePartyMergeLineage {
    pub candidate_case_id: DuplicateCandidateCaseId,
    pub candidate_case_version: i64,
    pub survivor_party_ref: PartyReference,
    pub absorbed_party_ref: PartyReference,
    pub party_kind: MergePartyKind,
    pub survivor_pre_merge_version: i64,
    pub absorbed_pre_merge_version: i64,
    pub survivor_post_merge_version: i64,
    pub absorbed_post_merge_version: i64,
    pub display_name_survivorship: DisplayNameSurvivorship,
    pub merge_actor_ref: MergeActorReference,
    pub occurred_at_unix_nanos: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnmergePartyLineage {
    pub expected_version: i64,
    pub survivor_pre_unmerge_version: i64,
    pub absorbed_pre_unmerge_version: i64,
    pub survivor_post_unmerge_version: i64,
    pub absorbed_post_unmerge_version: i64,
    pub actor_ref: MergeActorReference,
    pub reason: UnmergeReasonCode,
    pub occurred_at_unix_nanos: i64,
}

impl PartyMergeLineage {
    pub fn create(command: CreatePartyMergeLineage) -> Result<Self, SdkError> {
        validate_positive_version(
            "identity_resolution.merge.candidate_case_version",
            command.candidate_case_version,
        )?;
        validate_positive_version(
            "identity_resolution.merge.survivor_pre_merge_version",
            command.survivor_pre_merge_version,
        )?;
        validate_positive_version(
            "identity_resolution.merge.absorbed_pre_merge_version",
            command.absorbed_pre_merge_version,
        )?;
        validate_positive_version(
            "identity_resolution.merge.survivor_post_merge_version",
            command.survivor_post_merge_version,
        )?;
        validate_positive_version(
            "identity_resolution.merge.absorbed_post_merge_version",
            command.absorbed_post_merge_version,
        )?;
        validate_timestamp(
            "identity_resolution.merge.occurred_at_unix_nanos",
            command.occurred_at_unix_nanos,
        )?;

        let pair = CanonicalPartyPair::try_new(
            command.survivor_party_ref.clone(),
            command.absorbed_party_ref.clone(),
        )?;
        if DuplicateCandidateCaseId::for_pair(&pair)? != command.candidate_case_id {
            return Err(invalid(
                "IDENTITY_RESOLUTION_MERGE_CASE_PAIR_MISMATCH",
                "identity_resolution.merge.candidate_case_id",
                "candidate case identifier does not match the survivor/absorbed Party pair",
            ));
        }
        validate_merge_version_shape(
            command.survivor_pre_merge_version,
            command.absorbed_pre_merge_version,
            command.survivor_post_merge_version,
            command.absorbed_post_merge_version,
            &command.display_name_survivorship,
        )?;

        let merge_id = MergeLineageId::for_operation(
            &command.candidate_case_id,
            command.candidate_case_version,
            &command.survivor_party_ref,
            &command.absorbed_party_ref,
        )?;

        Ok(Self {
            merge_id,
            candidate_case_id: command.candidate_case_id,
            candidate_case_version: command.candidate_case_version,
            pair,
            survivor_party_ref: command.survivor_party_ref,
            absorbed_party_ref: command.absorbed_party_ref,
            party_kind: command.party_kind,
            survivor_pre_merge_version: command.survivor_pre_merge_version,
            absorbed_pre_merge_version: command.absorbed_pre_merge_version,
            survivor_post_merge_version: command.survivor_post_merge_version,
            absorbed_post_merge_version: command.absorbed_post_merge_version,
            display_name_survivorship: command.display_name_survivorship,
            merge_actor_ref: command.merge_actor_ref,
            merged_at_unix_nanos: command.occurred_at_unix_nanos,
            status: MergeLineageStatus::Active,
            unmerge_decision: None,
            updated_at_unix_nanos: command.occurred_at_unix_nanos,
            version: 1,
        })
    }

    pub fn rehydrate(snapshot: PartyMergeLineageSnapshot) -> Result<Self, SdkError> {
        validate_positive_version(
            "identity_resolution.merge.candidate_case_version",
            snapshot.candidate_case_version,
        )?;
        for (field, version) in [
            (
                "identity_resolution.merge.survivor_pre_merge_version",
                snapshot.survivor_pre_merge_version,
            ),
            (
                "identity_resolution.merge.absorbed_pre_merge_version",
                snapshot.absorbed_pre_merge_version,
            ),
            (
                "identity_resolution.merge.survivor_post_merge_version",
                snapshot.survivor_post_merge_version,
            ),
            (
                "identity_resolution.merge.absorbed_post_merge_version",
                snapshot.absorbed_post_merge_version,
            ),
        ] {
            validate_positive_version(field, version)?;
        }
        validate_timestamp(
            "identity_resolution.merge.merged_at_unix_nanos",
            snapshot.merged_at_unix_nanos,
        )?;
        validate_timestamp(
            "identity_resolution.merge.updated_at_unix_nanos",
            snapshot.updated_at_unix_nanos,
        )?;
        if snapshot.updated_at_unix_nanos < snapshot.merged_at_unix_nanos {
            return Err(invalid(
                "IDENTITY_RESOLUTION_MERGE_PERSISTED_TIME_INVALID",
                "identity_resolution.merge.updated_at_unix_nanos",
                "updated time cannot precede merge time",
            ));
        }

        let pair = CanonicalPartyPair::try_new(
            snapshot.survivor_party_ref.clone(),
            snapshot.absorbed_party_ref.clone(),
        )?;
        if pair != snapshot.pair {
            return Err(invalid(
                "IDENTITY_RESOLUTION_MERGE_PERSISTED_PAIR_INVALID",
                "identity_resolution.merge.party_pair",
                "persisted Party pair is not the canonical survivor/absorbed pair",
            ));
        }
        if DuplicateCandidateCaseId::for_pair(&pair)? != snapshot.candidate_case_id {
            return Err(invalid(
                "IDENTITY_RESOLUTION_MERGE_PERSISTED_CASE_INVALID",
                "identity_resolution.merge.candidate_case_id",
                "persisted candidate case identifier does not match the Party pair",
            ));
        }
        if MergeLineageId::for_operation(
            &snapshot.candidate_case_id,
            snapshot.candidate_case_version,
            &snapshot.survivor_party_ref,
            &snapshot.absorbed_party_ref,
        )? != snapshot.merge_id
        {
            return Err(invalid(
                "IDENTITY_RESOLUTION_MERGE_PERSISTED_ID_INVALID",
                "identity_resolution.merge.merge_id",
                "persisted merge identifier does not match the immutable operation coordinates",
            ));
        }
        validate_merge_version_shape(
            snapshot.survivor_pre_merge_version,
            snapshot.absorbed_pre_merge_version,
            snapshot.survivor_post_merge_version,
            snapshot.absorbed_post_merge_version,
            &snapshot.display_name_survivorship,
        )?;
        validate_lineage_lifecycle(&snapshot)?;

        Ok(Self {
            merge_id: snapshot.merge_id,
            candidate_case_id: snapshot.candidate_case_id,
            candidate_case_version: snapshot.candidate_case_version,
            pair: snapshot.pair,
            survivor_party_ref: snapshot.survivor_party_ref,
            absorbed_party_ref: snapshot.absorbed_party_ref,
            party_kind: snapshot.party_kind,
            survivor_pre_merge_version: snapshot.survivor_pre_merge_version,
            absorbed_pre_merge_version: snapshot.absorbed_pre_merge_version,
            survivor_post_merge_version: snapshot.survivor_post_merge_version,
            absorbed_post_merge_version: snapshot.absorbed_post_merge_version,
            display_name_survivorship: snapshot.display_name_survivorship,
            merge_actor_ref: snapshot.merge_actor_ref,
            merged_at_unix_nanos: snapshot.merged_at_unix_nanos,
            status: snapshot.status,
            unmerge_decision: snapshot.unmerge_decision,
            updated_at_unix_nanos: snapshot.updated_at_unix_nanos,
            version: snapshot.version,
        })
    }

    pub fn unmerge(&mut self, command: UnmergePartyLineage) -> Result<(), SdkError> {
        self.require_version(command.expected_version)?;
        if self.status != MergeLineageStatus::Active {
            return Err(conflict(
                "IDENTITY_RESOLUTION_MERGE_ALREADY_UNMERGED",
                "the merge lineage is already unmerged",
            ));
        }
        validate_timestamp(
            "identity_resolution.merge.unmerge.occurred_at_unix_nanos",
            command.occurred_at_unix_nanos,
        )?;
        if command.occurred_at_unix_nanos <= self.updated_at_unix_nanos {
            return Err(invalid(
                "IDENTITY_RESOLUTION_MERGE_TIME_REGRESSION",
                "identity_resolution.merge.unmerge.occurred_at_unix_nanos",
                "unmerge time must be strictly later than the merge mutation time",
            ));
        }
        validate_unmerge_version_shape(self, &command)?;

        self.unmerge_decision = Some(UnmergeDecision {
            actor_ref: command.actor_ref,
            reason: command.reason,
            survivor_pre_unmerge_version: command.survivor_pre_unmerge_version,
            absorbed_pre_unmerge_version: command.absorbed_pre_unmerge_version,
            survivor_post_unmerge_version: command.survivor_post_unmerge_version,
            absorbed_post_unmerge_version: command.absorbed_post_unmerge_version,
            occurred_at_unix_nanos: command.occurred_at_unix_nanos,
        });
        self.status = MergeLineageStatus::Unmerged;
        self.updated_at_unix_nanos = command.occurred_at_unix_nanos;
        self.version = self.version.checked_add(1).ok_or_else(|| {
            conflict(
                "IDENTITY_RESOLUTION_MERGE_VERSION_EXHAUSTED",
                "merge lineage version cannot be advanced further",
            )
        })?;
        Ok(())
    }

    pub fn snapshot(&self) -> PartyMergeLineageSnapshot {
        PartyMergeLineageSnapshot {
            merge_id: self.merge_id.clone(),
            candidate_case_id: self.candidate_case_id.clone(),
            candidate_case_version: self.candidate_case_version,
            pair: self.pair.clone(),
            survivor_party_ref: self.survivor_party_ref.clone(),
            absorbed_party_ref: self.absorbed_party_ref.clone(),
            party_kind: self.party_kind,
            survivor_pre_merge_version: self.survivor_pre_merge_version,
            absorbed_pre_merge_version: self.absorbed_pre_merge_version,
            survivor_post_merge_version: self.survivor_post_merge_version,
            absorbed_post_merge_version: self.absorbed_post_merge_version,
            display_name_survivorship: self.display_name_survivorship.clone(),
            merge_actor_ref: self.merge_actor_ref.clone(),
            merged_at_unix_nanos: self.merged_at_unix_nanos,
            status: self.status,
            unmerge_decision: self.unmerge_decision.clone(),
            updated_at_unix_nanos: self.updated_at_unix_nanos,
            version: self.version,
        }
    }

    pub fn merge_id(&self) -> &MergeLineageId {
        &self.merge_id
    }

    pub fn candidate_case_id(&self) -> &DuplicateCandidateCaseId {
        &self.candidate_case_id
    }

    pub fn candidate_case_version(&self) -> i64 {
        self.candidate_case_version
    }

    pub fn pair(&self) -> &CanonicalPartyPair {
        &self.pair
    }

    pub fn survivor_party_ref(&self) -> &PartyReference {
        &self.survivor_party_ref
    }

    pub fn absorbed_party_ref(&self) -> &PartyReference {
        &self.absorbed_party_ref
    }

    pub const fn party_kind(&self) -> MergePartyKind {
        self.party_kind
    }

    pub fn survivor_pre_merge_version(&self) -> i64 {
        self.survivor_pre_merge_version
    }

    pub fn absorbed_pre_merge_version(&self) -> i64 {
        self.absorbed_pre_merge_version
    }

    pub fn survivor_post_merge_version(&self) -> i64 {
        self.survivor_post_merge_version
    }

    pub fn absorbed_post_merge_version(&self) -> i64 {
        self.absorbed_post_merge_version
    }

    pub fn display_name_survivorship(&self) -> &DisplayNameSurvivorship {
        &self.display_name_survivorship
    }

    pub fn merge_actor_ref(&self) -> &MergeActorReference {
        &self.merge_actor_ref
    }

    pub fn merged_at_unix_nanos(&self) -> i64 {
        self.merged_at_unix_nanos
    }

    pub const fn status(&self) -> MergeLineageStatus {
        self.status
    }

    pub fn unmerge_decision(&self) -> Option<&UnmergeDecision> {
        self.unmerge_decision.as_ref()
    }

    pub fn updated_at_unix_nanos(&self) -> i64 {
        self.updated_at_unix_nanos
    }

    pub fn version(&self) -> i64 {
        self.version
    }

    fn require_version(&self, expected_version: i64) -> Result<(), SdkError> {
        if expected_version != self.version {
            return Err(conflict(
                "IDENTITY_RESOLUTION_MERGE_VERSION_CONFLICT",
                format!(
                    "expected merge lineage version {expected_version}, found {}",
                    self.version
                ),
            ));
        }
        Ok(())
    }
}

fn validate_merge_version_shape(
    survivor_pre_merge_version: i64,
    absorbed_pre_merge_version: i64,
    survivor_post_merge_version: i64,
    absorbed_post_merge_version: i64,
    display_name_survivorship: &DisplayNameSurvivorship,
) -> Result<(), SdkError> {
    let expected_survivor_post = if display_name_survivorship.changes_survivor() {
        survivor_pre_merge_version.checked_add(1)
    } else {
        Some(survivor_pre_merge_version)
    }
    .ok_or_else(|| {
        conflict(
            "IDENTITY_RESOLUTION_MERGE_PARTY_VERSION_EXHAUSTED",
            "survivor Party version cannot be advanced further",
        )
    })?;
    if survivor_post_merge_version != expected_survivor_post {
        return Err(invalid(
            "IDENTITY_RESOLUTION_MERGE_SURVIVOR_VERSION_INVALID",
            "identity_resolution.merge.survivor_post_merge_version",
            format!(
                "survivor post-merge version must be {expected_survivor_post} for the selected survivorship decision"
            ),
        ));
    }

    let expected_absorbed_post = absorbed_pre_merge_version.checked_add(1).ok_or_else(|| {
        conflict(
            "IDENTITY_RESOLUTION_MERGE_PARTY_VERSION_EXHAUSTED",
            "absorbed Party version cannot be advanced further",
        )
    })?;
    if absorbed_post_merge_version != expected_absorbed_post {
        return Err(invalid(
            "IDENTITY_RESOLUTION_MERGE_ABSORBED_VERSION_INVALID",
            "identity_resolution.merge.absorbed_post_merge_version",
            format!("absorbed post-merge version must be {expected_absorbed_post}"),
        ));
    }
    Ok(())
}

fn validate_unmerge_version_shape(
    lineage: &PartyMergeLineage,
    command: &UnmergePartyLineage,
) -> Result<(), SdkError> {
    for (field, version) in [
        (
            "identity_resolution.merge.unmerge.survivor_pre_unmerge_version",
            command.survivor_pre_unmerge_version,
        ),
        (
            "identity_resolution.merge.unmerge.absorbed_pre_unmerge_version",
            command.absorbed_pre_unmerge_version,
        ),
        (
            "identity_resolution.merge.unmerge.survivor_post_unmerge_version",
            command.survivor_post_unmerge_version,
        ),
        (
            "identity_resolution.merge.unmerge.absorbed_post_unmerge_version",
            command.absorbed_post_unmerge_version,
        ),
    ] {
        validate_positive_version(field, version)?;
    }

    if command.survivor_pre_unmerge_version != lineage.survivor_post_merge_version
        || command.absorbed_pre_unmerge_version != lineage.absorbed_post_merge_version
    {
        return Err(conflict(
            "IDENTITY_RESOLUTION_MERGE_UNMERGE_PARTY_VERSION_CONFLICT",
            "current Party versions do not match the exact merge result and cannot be safely unmerged",
        ));
    }

    let expected_survivor_post = if lineage.display_name_survivorship.changes_survivor() {
        command.survivor_pre_unmerge_version.checked_add(1)
    } else {
        Some(command.survivor_pre_unmerge_version)
    }
    .ok_or_else(|| {
        conflict(
            "IDENTITY_RESOLUTION_MERGE_PARTY_VERSION_EXHAUSTED",
            "survivor Party version cannot be advanced further during unmerge",
        )
    })?;
    if command.survivor_post_unmerge_version != expected_survivor_post {
        return Err(invalid(
            "IDENTITY_RESOLUTION_MERGE_UNMERGE_SURVIVOR_VERSION_INVALID",
            "identity_resolution.merge.unmerge.survivor_post_unmerge_version",
            format!(
                "survivor post-unmerge version must be {expected_survivor_post} for this lineage"
            ),
        ));
    }

    let expected_absorbed_post = command
        .absorbed_pre_unmerge_version
        .checked_add(1)
        .ok_or_else(|| {
            conflict(
                "IDENTITY_RESOLUTION_MERGE_PARTY_VERSION_EXHAUSTED",
                "absorbed Party version cannot be advanced further during unmerge",
            )
        })?;
    if command.absorbed_post_unmerge_version != expected_absorbed_post {
        return Err(invalid(
            "IDENTITY_RESOLUTION_MERGE_UNMERGE_ABSORBED_VERSION_INVALID",
            "identity_resolution.merge.unmerge.absorbed_post_unmerge_version",
            format!("absorbed post-unmerge version must be {expected_absorbed_post}"),
        ));
    }
    Ok(())
}

fn validate_lineage_lifecycle(snapshot: &PartyMergeLineageSnapshot) -> Result<(), SdkError> {
    match snapshot.status {
        MergeLineageStatus::Active => {
            if snapshot.version != 1 || snapshot.unmerge_decision.is_some() {
                return Err(invalid(
                    "IDENTITY_RESOLUTION_MERGE_PERSISTED_LIFECYCLE_INVALID",
                    "identity_resolution.merge.status",
                    "an active merge lineage must be version 1 without unmerge evidence",
                ));
            }
            if snapshot.updated_at_unix_nanos != snapshot.merged_at_unix_nanos {
                return Err(invalid(
                    "IDENTITY_RESOLUTION_MERGE_PERSISTED_TIME_INVALID",
                    "identity_resolution.merge.updated_at_unix_nanos",
                    "an active merge lineage must retain its original merge mutation time",
                ));
            }
        }
        MergeLineageStatus::Unmerged => {
            if snapshot.version != 2 {
                return Err(invalid(
                    "IDENTITY_RESOLUTION_MERGE_PERSISTED_LIFECYCLE_INVALID",
                    "identity_resolution.merge.version",
                    "an unmerged lineage must be version 2",
                ));
            }
            let decision = snapshot.unmerge_decision.as_ref().ok_or_else(|| {
                invalid(
                    "IDENTITY_RESOLUTION_MERGE_PERSISTED_LIFECYCLE_INVALID",
                    "identity_resolution.merge.unmerge_decision",
                    "an unmerged lineage must retain unmerge evidence",
                )
            })?;
            if decision.occurred_at_unix_nanos <= snapshot.merged_at_unix_nanos
                || snapshot.updated_at_unix_nanos != decision.occurred_at_unix_nanos
            {
                return Err(invalid(
                    "IDENTITY_RESOLUTION_MERGE_PERSISTED_TIME_INVALID",
                    "identity_resolution.merge.updated_at_unix_nanos",
                    "unmerge time must be strictly later than merge time and equal updated time",
                ));
            }
            validate_unmerge_snapshot_shape(snapshot, decision)?;
        }
    }
    Ok(())
}

fn validate_unmerge_snapshot_shape(
    snapshot: &PartyMergeLineageSnapshot,
    decision: &UnmergeDecision,
) -> Result<(), SdkError> {
    if decision.survivor_pre_unmerge_version != snapshot.survivor_post_merge_version
        || decision.absorbed_pre_unmerge_version != snapshot.absorbed_post_merge_version
    {
        return Err(invalid(
            "IDENTITY_RESOLUTION_MERGE_PERSISTED_UNMERGE_VERSION_INVALID",
            "identity_resolution.merge.unmerge_decision",
            "persisted unmerge pre-versions do not match the exact merge result",
        ));
    }
    let expected_survivor_post = if snapshot.display_name_survivorship.changes_survivor() {
        decision.survivor_pre_unmerge_version.checked_add(1)
    } else {
        Some(decision.survivor_pre_unmerge_version)
    }
    .ok_or_else(|| {
        invalid(
            "IDENTITY_RESOLUTION_MERGE_PERSISTED_UNMERGE_VERSION_INVALID",
            "identity_resolution.merge.unmerge_decision",
            "persisted survivor unmerge version overflow",
        )
    })?;
    let expected_absorbed_post = decision
        .absorbed_pre_unmerge_version
        .checked_add(1)
        .ok_or_else(|| {
            invalid(
                "IDENTITY_RESOLUTION_MERGE_PERSISTED_UNMERGE_VERSION_INVALID",
                "identity_resolution.merge.unmerge_decision",
                "persisted absorbed unmerge version overflow",
            )
        })?;
    if decision.survivor_post_unmerge_version != expected_survivor_post
        || decision.absorbed_post_unmerge_version != expected_absorbed_post
    {
        return Err(invalid(
            "IDENTITY_RESOLUTION_MERGE_PERSISTED_UNMERGE_VERSION_INVALID",
            "identity_resolution.merge.unmerge_decision",
            "persisted unmerge post-versions are not reachable from the exact merge result",
        ));
    }
    Ok(())
}

fn normalize_display_name(value: String, field: &'static str) -> Result<String, SdkError> {
    if value.chars().any(char::is_control) {
        return Err(invalid(
            "IDENTITY_RESOLUTION_MERGE_DISPLAY_NAME_INVALID",
            field,
            "display name must not contain control characters",
        ));
    }
    let canonical = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if canonical.is_empty() || canonical.len() > MAX_DISPLAY_NAME_BYTES {
        return Err(invalid(
            "IDENTITY_RESOLUTION_MERGE_DISPLAY_NAME_INVALID",
            field,
            format!(
                "display name must be non-empty and not exceed {MAX_DISPLAY_NAME_BYTES} UTF-8 bytes"
            ),
        ));
    }
    Ok(canonical)
}

fn normalize_reason_code(value: String) -> Result<String, SdkError> {
    let canonical = value.trim().to_ascii_lowercase();
    if canonical.is_empty()
        || canonical.len() > MAX_REASON_CODE_BYTES
        || !canonical.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
    {
        return Err(invalid(
            "IDENTITY_RESOLUTION_UNMERGE_REASON_INVALID",
            "identity_resolution.merge.unmerge.reason",
            "unmerge reason must be a non-empty lowercase semantic code",
        ));
    }
    Ok(canonical)
}

fn validate_positive_version(field: &'static str, version: i64) -> Result<(), SdkError> {
    if version <= 0 {
        return Err(invalid(
            "IDENTITY_RESOLUTION_MERGE_VERSION_INVALID",
            field,
            "version must be positive",
        ));
    }
    Ok(())
}

fn validate_timestamp(field: &'static str, value: i64) -> Result<(), SdkError> {
    if value < 0 {
        return Err(invalid(
            "IDENTITY_RESOLUTION_MERGE_TIMESTAMP_INVALID",
            field,
            "timestamp must not be negative",
        ));
    }
    Ok(())
}

fn hash_field(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}

fn invalid(code: &'static str, field: &'static str, safe_message: impl Into<String>) -> SdkError {
    SdkError::invalid_argument(field, safe_message.into()).with_internal_reference(code)
}

fn conflict(code: &'static str, safe_message: impl Into<String>) -> SdkError {
    SdkError::new(code, ErrorCategory::Conflict, false, safe_message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn party(value: &str) -> PartyReference {
        PartyReference::try_new(value).unwrap()
    }

    fn case_for(first: &PartyReference, second: &PartyReference) -> DuplicateCandidateCaseId {
        DuplicateCandidateCaseId::for_pair(
            &CanonicalPartyPair::try_new(first.clone(), second.clone()).unwrap(),
        )
        .unwrap()
    }

    fn actor() -> MergeActorReference {
        MergeActorReference::try_new("reviewer-1").unwrap()
    }

    fn reason() -> UnmergeReasonCode {
        UnmergeReasonCode::try_new("review.false-positive").unwrap()
    }

    fn create_lineage(chosen_source: SurvivorshipSource) -> PartyMergeLineage {
        let survivor = party("party-a");
        let absorbed = party("party-b");
        let survivorship =
            DisplayNameSurvivorship::try_new(chosen_source, "Alpha Person", "Beta Person").unwrap();
        let survivor_post = if survivorship.changes_survivor() {
            8
        } else {
            7
        };
        PartyMergeLineage::create(CreatePartyMergeLineage {
            candidate_case_id: case_for(&survivor, &absorbed),
            candidate_case_version: 3,
            survivor_party_ref: survivor,
            absorbed_party_ref: absorbed,
            party_kind: MergePartyKind::Person,
            survivor_pre_merge_version: 7,
            absorbed_pre_merge_version: 4,
            survivor_post_merge_version: survivor_post,
            absorbed_post_merge_version: 5,
            display_name_survivorship: survivorship,
            merge_actor_ref: actor(),
            occurred_at_unix_nanos: 100,
        })
        .unwrap()
    }

    #[test]
    fn merge_identity_is_deterministic_and_directional() {
        let first = party("party-a");
        let second = party("party-b");
        let case_id = case_for(&first, &second);
        let left = MergeLineageId::for_operation(&case_id, 3, &first, &second).unwrap();
        let same = MergeLineageId::for_operation(&case_id, 3, &first, &second).unwrap();
        let reversed = MergeLineageId::for_operation(&case_id, 3, &second, &first).unwrap();
        assert_eq!(left, same);
        assert_ne!(left, reversed);
    }

    #[test]
    fn candidate_case_must_match_the_merge_pair() {
        let survivor = party("party-a");
        let absorbed = party("party-b");
        let unrelated = party("party-c");
        let error = PartyMergeLineage::create(CreatePartyMergeLineage {
            candidate_case_id: case_for(&survivor, &unrelated),
            candidate_case_version: 2,
            survivor_party_ref: survivor,
            absorbed_party_ref: absorbed,
            party_kind: MergePartyKind::Person,
            survivor_pre_merge_version: 1,
            absorbed_pre_merge_version: 1,
            survivor_post_merge_version: 1,
            absorbed_post_merge_version: 2,
            display_name_survivorship: DisplayNameSurvivorship::try_new(
                SurvivorshipSource::Survivor,
                "Alpha",
                "Beta",
            )
            .unwrap(),
            merge_actor_ref: actor(),
            occurred_at_unix_nanos: 10,
        })
        .unwrap_err();
        assert!(error.internal_reference.is_some());
    }

    #[test]
    fn survivorship_controls_the_reachable_survivor_version() {
        let retained = create_lineage(SurvivorshipSource::Survivor);
        assert_eq!(retained.survivor_pre_merge_version(), 7);
        assert_eq!(retained.survivor_post_merge_version(), 7);

        let changed = create_lineage(SurvivorshipSource::Absorbed);
        assert_eq!(changed.survivor_pre_merge_version(), 7);
        assert_eq!(changed.survivor_post_merge_version(), 8);
        assert_eq!(
            changed.display_name_survivorship().chosen_value(),
            "Beta Person"
        );
    }

    #[test]
    fn unmerge_is_exact_versioned_and_irreversible() {
        let mut lineage = create_lineage(SurvivorshipSource::Absorbed);
        lineage
            .unmerge(UnmergePartyLineage {
                expected_version: 1,
                survivor_pre_unmerge_version: 8,
                absorbed_pre_unmerge_version: 5,
                survivor_post_unmerge_version: 9,
                absorbed_post_unmerge_version: 6,
                actor_ref: actor(),
                reason: reason(),
                occurred_at_unix_nanos: 200,
            })
            .unwrap();
        assert_eq!(lineage.status(), MergeLineageStatus::Unmerged);
        assert_eq!(lineage.version(), 2);
        assert_eq!(lineage.unmerge_decision().unwrap().reason(), &reason());

        let error = lineage
            .unmerge(UnmergePartyLineage {
                expected_version: 2,
                survivor_pre_unmerge_version: 9,
                absorbed_pre_unmerge_version: 6,
                survivor_post_unmerge_version: 10,
                absorbed_post_unmerge_version: 7,
                actor_ref: actor(),
                reason: reason(),
                occurred_at_unix_nanos: 300,
            })
            .unwrap_err();
        assert_eq!(
            error.code.as_str(),
            "IDENTITY_RESOLUTION_MERGE_ALREADY_UNMERGED"
        );
    }

    #[test]
    fn unmerge_rejects_party_versions_that_moved_after_merge() {
        let mut lineage = create_lineage(SurvivorshipSource::Absorbed);
        let error = lineage
            .unmerge(UnmergePartyLineage {
                expected_version: 1,
                survivor_pre_unmerge_version: 9,
                absorbed_pre_unmerge_version: 5,
                survivor_post_unmerge_version: 10,
                absorbed_post_unmerge_version: 6,
                actor_ref: actor(),
                reason: reason(),
                occurred_at_unix_nanos: 200,
            })
            .unwrap_err();
        assert_eq!(
            error.code.as_str(),
            "IDENTITY_RESOLUTION_MERGE_UNMERGE_PARTY_VERSION_CONFLICT"
        );
        assert_eq!(lineage.status(), MergeLineageStatus::Active);
        assert_eq!(lineage.version(), 1);
    }

    #[test]
    fn rehydrate_rejects_corrupt_lifecycle_shape() {
        let lineage = create_lineage(SurvivorshipSource::Survivor);
        let mut snapshot = lineage.snapshot();
        snapshot.status = MergeLineageStatus::Unmerged;
        snapshot.version = 2;
        snapshot.updated_at_unix_nanos = 200;
        let error = PartyMergeLineage::rehydrate(snapshot).unwrap_err();
        assert!(error.internal_reference.is_some());
    }
}
