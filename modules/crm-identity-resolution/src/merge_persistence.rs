use crate::merge::{
    CreatePartyMergeLineage, DisplayNameSurvivorship, MergeActorReference, MergeLineageId,
    MergeLineageStatus, MergePartyKind, PartyMergeLineage, SurvivorshipSource, UnmergePartyLineage,
    UnmergeReasonCode,
};
use crate::{CanonicalPartyPair, DuplicateCandidateCaseId, PartyReference};
use crm_module_sdk::{ErrorCategory, SdkError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PARTY_MERGE_LINEAGE_STATE_SCHEMA_ID: &str = "crm.identity_resolution.merge_lineage.state";
pub const PARTY_MERGE_LINEAGE_STATE_SCHEMA_VERSION: &str = "1.0.0";
pub const PARTY_MERGE_LINEAGE_STATE_MAXIMUM_BYTES: u64 = 256 * 1024;
pub const PARTY_MERGE_LINEAGE_STATE_RETENTION_POLICY_ID: &str =
    "crm.identity_resolution.merge_lineage";
const PARTY_MERGE_LINEAGE_STATE_DESCRIPTOR: &[u8] = b"crm.identity_resolution.merge_lineage.state/v1:merge_id,candidate_case_id,candidate_case_version,left_party_id,right_party_id,survivor_party_id,absorbed_party_id,party_kind,survivor_pre_merge_version,absorbed_pre_merge_version,survivor_post_merge_version,absorbed_post_merge_version,display_name_survivorship[chosen_source,survivor_value,absorbed_value],merge_actor_ref,merged_at_unix_nanos,status,unmerge_decision[actor_ref,reason,survivor_pre_unmerge_version,absorbed_pre_unmerge_version,survivor_post_unmerge_version,absorbed_post_unmerge_version,occurred_at_unix_nanos],updated_at_unix_nanos,version";

pub fn party_merge_lineage_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(PARTY_MERGE_LINEAGE_STATE_DESCRIPTOR).into()
}

pub fn encode_party_merge_lineage_state(lineage: &PartyMergeLineage) -> Result<Vec<u8>, SdkError> {
    let bytes = serde_json::to_vec(&PartyMergeLineageStateV1::from(lineage)).map_err(|error| {
        persisted_error(format!(
            "Identity Resolution merge-lineage serialization failed: {error}"
        ))
    })?;
    validate_size(&bytes)?;
    Ok(bytes)
}

pub fn decode_party_merge_lineage_state(bytes: &[u8]) -> Result<PartyMergeLineage, SdkError> {
    validate_size(bytes)?;
    let state: PartyMergeLineageStateV1 = serde_json::from_slice(bytes).map_err(|error| {
        persisted_error(format!(
            "Identity Resolution merge-lineage JSON is invalid: {error}"
        ))
    })?;
    let lineage = state.into_domain()?;
    let canonical = encode_party_merge_lineage_state(&lineage)?;
    if canonical != bytes {
        return Err(persisted_error(
            "persisted merge-lineage representation is not canonical",
        ));
    }
    Ok(lineage)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartyMergeLineageStateV1 {
    merge_id: String,
    candidate_case_id: String,
    candidate_case_version: i64,
    left_party_id: String,
    right_party_id: String,
    survivor_party_id: String,
    absorbed_party_id: String,
    party_kind: MergePartyKindState,
    survivor_pre_merge_version: i64,
    absorbed_pre_merge_version: i64,
    survivor_post_merge_version: i64,
    absorbed_post_merge_version: i64,
    display_name_survivorship: DisplayNameSurvivorshipStateV1,
    merge_actor_ref: String,
    merged_at_unix_nanos: i64,
    status: MergeLineageStatusState,
    unmerge_decision: Option<UnmergeDecisionStateV1>,
    updated_at_unix_nanos: i64,
    version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DisplayNameSurvivorshipStateV1 {
    chosen_source: SurvivorshipSourceState,
    survivor_value: String,
    absorbed_value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct UnmergeDecisionStateV1 {
    actor_ref: String,
    reason: String,
    survivor_pre_unmerge_version: i64,
    absorbed_pre_unmerge_version: i64,
    survivor_post_unmerge_version: i64,
    absorbed_post_unmerge_version: i64,
    occurred_at_unix_nanos: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum MergePartyKindState {
    Person,
    Organization,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SurvivorshipSourceState {
    Survivor,
    Absorbed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum MergeLineageStatusState {
    Active,
    Unmerged,
}

impl From<&PartyMergeLineage> for PartyMergeLineageStateV1 {
    fn from(value: &PartyMergeLineage) -> Self {
        Self {
            merge_id: value.merge_id().as_str().to_owned(),
            candidate_case_id: value.candidate_case_id().as_str().to_owned(),
            candidate_case_version: value.candidate_case_version(),
            left_party_id: value.pair().left().as_str().to_owned(),
            right_party_id: value.pair().right().as_str().to_owned(),
            survivor_party_id: value.survivor_party_ref().as_str().to_owned(),
            absorbed_party_id: value.absorbed_party_ref().as_str().to_owned(),
            party_kind: value.party_kind().into(),
            survivor_pre_merge_version: value.survivor_pre_merge_version(),
            absorbed_pre_merge_version: value.absorbed_pre_merge_version(),
            survivor_post_merge_version: value.survivor_post_merge_version(),
            absorbed_post_merge_version: value.absorbed_post_merge_version(),
            display_name_survivorship: value.display_name_survivorship().into(),
            merge_actor_ref: value.merge_actor_ref().as_str().to_owned(),
            merged_at_unix_nanos: value.merged_at_unix_nanos(),
            status: value.status().into(),
            unmerge_decision: value.unmerge_decision().map(Into::into),
            updated_at_unix_nanos: value.updated_at_unix_nanos(),
            version: value.version(),
        }
    }
}

impl PartyMergeLineageStateV1 {
    fn into_domain(self) -> Result<PartyMergeLineage, SdkError> {
        let merge_id = canonical_merge_id(self.merge_id)?;
        let candidate_case_id = canonical_case_id(self.candidate_case_id)?;
        let left_party_ref = canonical_party_ref(self.left_party_id.clone())?;
        let right_party_ref = canonical_party_ref(self.right_party_id.clone())?;
        let pair = CanonicalPartyPair::try_new(left_party_ref, right_party_ref)
            .map_err(|error| persisted_error(error.to_string()))?;
        if pair.left().as_str() != self.left_party_id
            || pair.right().as_str() != self.right_party_id
        {
            return Err(persisted_error(
                "persisted merge-lineage Party pair is not in canonical order",
            ));
        }

        let survivor_party_ref = canonical_party_ref(self.survivor_party_id)?;
        let absorbed_party_ref = canonical_party_ref(self.absorbed_party_id)?;
        if survivor_party_ref == absorbed_party_ref
            || !pair.contains(&survivor_party_ref)
            || !pair.contains(&absorbed_party_ref)
        {
            return Err(persisted_error(
                "persisted survivor/absorbed Party roles do not match the canonical pair",
            ));
        }

        let mut lineage = PartyMergeLineage::create(CreatePartyMergeLineage {
            candidate_case_id,
            candidate_case_version: self.candidate_case_version,
            survivor_party_ref,
            absorbed_party_ref,
            party_kind: self.party_kind.into(),
            survivor_pre_merge_version: self.survivor_pre_merge_version,
            absorbed_pre_merge_version: self.absorbed_pre_merge_version,
            survivor_post_merge_version: self.survivor_post_merge_version,
            absorbed_post_merge_version: self.absorbed_post_merge_version,
            display_name_survivorship: self.display_name_survivorship.into_domain()?,
            merge_actor_ref: canonical_actor_ref(self.merge_actor_ref)?,
            occurred_at_unix_nanos: self.merged_at_unix_nanos,
        })
        .map_err(|error| persisted_error(format!("{}: {}", error.code, error.safe_message)))?;

        if lineage.merge_id() != &merge_id {
            return Err(persisted_error(
                "persisted merge ID does not match the immutable operation coordinates",
            ));
        }

        match self.status {
            MergeLineageStatusState::Active => {
                if self.unmerge_decision.is_some() {
                    return Err(persisted_error(
                        "an active merge lineage cannot contain unmerge evidence",
                    ));
                }
            }
            MergeLineageStatusState::Unmerged => {
                let command = self
                    .unmerge_decision
                    .ok_or_else(|| {
                        persisted_error("unmerged lineage is missing reversal evidence")
                    })?
                    .into_command()?;
                lineage.unmerge(command).map_err(|error| {
                    persisted_error(format!("{}: {}", error.code, error.safe_message))
                })?;
            }
        }

        if lineage.updated_at_unix_nanos() != self.updated_at_unix_nanos
            || lineage.version() != self.version
        {
            return Err(persisted_error(
                "persisted merge-lineage lifecycle coordinates are not reachable",
            ));
        }
        Ok(lineage)
    }
}

impl From<&DisplayNameSurvivorship> for DisplayNameSurvivorshipStateV1 {
    fn from(value: &DisplayNameSurvivorship) -> Self {
        Self {
            chosen_source: value.chosen_source().into(),
            survivor_value: value.survivor_value().to_owned(),
            absorbed_value: value.absorbed_value().to_owned(),
        }
    }
}

impl DisplayNameSurvivorshipStateV1 {
    fn into_domain(self) -> Result<DisplayNameSurvivorship, SdkError> {
        let domain = DisplayNameSurvivorship::try_new(
            self.chosen_source.into(),
            self.survivor_value.clone(),
            self.absorbed_value.clone(),
        )
        .map_err(|error| persisted_error(error.to_string()))?;
        if domain.survivor_value() != self.survivor_value
            || domain.absorbed_value() != self.absorbed_value
        {
            return Err(persisted_error(
                "persisted display-name survivorship values are not canonical",
            ));
        }
        Ok(domain)
    }
}

impl From<&crate::merge::UnmergeDecision> for UnmergeDecisionStateV1 {
    fn from(value: &crate::merge::UnmergeDecision) -> Self {
        Self {
            actor_ref: value.actor_ref().as_str().to_owned(),
            reason: value.reason().as_str().to_owned(),
            survivor_pre_unmerge_version: value.survivor_pre_unmerge_version(),
            absorbed_pre_unmerge_version: value.absorbed_pre_unmerge_version(),
            survivor_post_unmerge_version: value.survivor_post_unmerge_version(),
            absorbed_post_unmerge_version: value.absorbed_post_unmerge_version(),
            occurred_at_unix_nanos: value.occurred_at_unix_nanos(),
        }
    }
}

impl UnmergeDecisionStateV1 {
    fn into_command(self) -> Result<UnmergePartyLineage, SdkError> {
        Ok(UnmergePartyLineage {
            expected_version: 1,
            survivor_pre_unmerge_version: self.survivor_pre_unmerge_version,
            absorbed_pre_unmerge_version: self.absorbed_pre_unmerge_version,
            survivor_post_unmerge_version: self.survivor_post_unmerge_version,
            absorbed_post_unmerge_version: self.absorbed_post_unmerge_version,
            actor_ref: canonical_actor_ref(self.actor_ref)?,
            reason: canonical_unmerge_reason(self.reason)?,
            occurred_at_unix_nanos: self.occurred_at_unix_nanos,
        })
    }
}

impl From<MergePartyKind> for MergePartyKindState {
    fn from(value: MergePartyKind) -> Self {
        match value {
            MergePartyKind::Person => Self::Person,
            MergePartyKind::Organization => Self::Organization,
        }
    }
}

impl From<MergePartyKindState> for MergePartyKind {
    fn from(value: MergePartyKindState) -> Self {
        match value {
            MergePartyKindState::Person => Self::Person,
            MergePartyKindState::Organization => Self::Organization,
        }
    }
}

impl From<SurvivorshipSource> for SurvivorshipSourceState {
    fn from(value: SurvivorshipSource) -> Self {
        match value {
            SurvivorshipSource::Survivor => Self::Survivor,
            SurvivorshipSource::Absorbed => Self::Absorbed,
        }
    }
}

impl From<SurvivorshipSourceState> for SurvivorshipSource {
    fn from(value: SurvivorshipSourceState) -> Self {
        match value {
            SurvivorshipSourceState::Survivor => Self::Survivor,
            SurvivorshipSourceState::Absorbed => Self::Absorbed,
        }
    }
}

impl From<MergeLineageStatus> for MergeLineageStatusState {
    fn from(value: MergeLineageStatus) -> Self {
        match value {
            MergeLineageStatus::Active => Self::Active,
            MergeLineageStatus::Unmerged => Self::Unmerged,
        }
    }
}

fn canonical_merge_id(raw: String) -> Result<MergeLineageId, SdkError> {
    let parsed =
        MergeLineageId::try_new(raw.clone()).map_err(|error| persisted_error(error.to_string()))?;
    if parsed.as_str() != raw {
        return Err(persisted_error("persisted merge ID is not canonical"));
    }
    Ok(parsed)
}

fn canonical_case_id(raw: String) -> Result<DuplicateCandidateCaseId, SdkError> {
    let parsed = DuplicateCandidateCaseId::try_new(raw.clone())
        .map_err(|error| persisted_error(error.to_string()))?;
    if parsed.as_str() != raw {
        return Err(persisted_error(
            "persisted candidate case ID is not canonical",
        ));
    }
    Ok(parsed)
}

fn canonical_party_ref(raw: String) -> Result<PartyReference, SdkError> {
    let parsed =
        PartyReference::try_new(raw.clone()).map_err(|error| persisted_error(error.to_string()))?;
    if parsed.as_str() != raw {
        return Err(persisted_error(
            "persisted Party reference is not canonical",
        ));
    }
    Ok(parsed)
}

fn canonical_actor_ref(raw: String) -> Result<MergeActorReference, SdkError> {
    let parsed = MergeActorReference::try_new(raw.clone())
        .map_err(|error| persisted_error(error.to_string()))?;
    if parsed.as_str() != raw {
        return Err(persisted_error(
            "persisted merge actor reference is not canonical",
        ));
    }
    Ok(parsed)
}

fn canonical_unmerge_reason(raw: String) -> Result<UnmergeReasonCode, SdkError> {
    let parsed = UnmergeReasonCode::try_new(raw.clone())
        .map_err(|error| persisted_error(error.to_string()))?;
    if parsed.as_str() != raw {
        return Err(persisted_error("persisted unmerge reason is not canonical"));
    }
    Ok(parsed)
}

fn validate_size(bytes: &[u8]) -> Result<(), SdkError> {
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > PARTY_MERGE_LINEAGE_STATE_MAXIMUM_BYTES {
        return Err(persisted_error(format!(
            "Identity Resolution merge-lineage state exceeds the maximum of {PARTY_MERGE_LINEAGE_STATE_MAXIMUM_BYTES} bytes"
        )));
    }
    Ok(())
}

fn persisted_error(message: impl Into<String>) -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_MERGE_PERSISTED_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "The persisted Identity Resolution merge-lineage state is invalid.",
    )
    .with_internal_reference(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lineage() -> PartyMergeLineage {
        let survivor = PartyReference::try_new("party-a").unwrap();
        let absorbed = PartyReference::try_new("party-b").unwrap();
        let pair = CanonicalPartyPair::try_new(survivor.clone(), absorbed.clone()).unwrap();
        PartyMergeLineage::create(CreatePartyMergeLineage {
            candidate_case_id: DuplicateCandidateCaseId::for_pair(&pair).unwrap(),
            candidate_case_version: 2,
            survivor_party_ref: survivor,
            absorbed_party_ref: absorbed,
            party_kind: MergePartyKind::Person,
            survivor_pre_merge_version: 4,
            absorbed_pre_merge_version: 7,
            survivor_post_merge_version: 5,
            absorbed_post_merge_version: 8,
            display_name_survivorship: DisplayNameSurvivorship::try_new(
                SurvivorshipSource::Absorbed,
                "Alpha Person",
                "Beta Person",
            )
            .unwrap(),
            merge_actor_ref: MergeActorReference::try_new("reviewer-1").unwrap(),
            occurred_at_unix_nanos: 100,
        })
        .unwrap()
    }

    #[test]
    fn round_trip_is_exact_deterministic_and_descriptor_hash_is_nonzero() {
        let value = lineage();
        let first = encode_party_merge_lineage_state(&value).unwrap();
        let second = encode_party_merge_lineage_state(&value).unwrap();
        let decoded = decode_party_merge_lineage_state(&first).unwrap();
        assert_eq!(decoded, value);
        assert_eq!(first, second);
        assert_ne!(party_merge_lineage_state_descriptor_hash(), [0; 32]);
    }

    #[test]
    fn unmerged_round_trip_preserves_reversal_evidence() {
        let mut value = lineage();
        value
            .unmerge(UnmergePartyLineage {
                expected_version: 1,
                survivor_pre_unmerge_version: 5,
                absorbed_pre_unmerge_version: 8,
                survivor_post_unmerge_version: 6,
                absorbed_post_unmerge_version: 9,
                actor_ref: MergeActorReference::try_new("reviewer-2").unwrap(),
                reason: UnmergeReasonCode::try_new("review.false-positive").unwrap(),
                occurred_at_unix_nanos: 200,
            })
            .unwrap();
        let bytes = encode_party_merge_lineage_state(&value).unwrap();
        assert_eq!(decode_party_merge_lineage_state(&bytes).unwrap(), value);
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let bytes = encode_party_merge_lineage_state(&lineage()).unwrap();
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("unknown".to_owned(), serde_json::json!(true));
        let corrupt = serde_json::to_vec(&value).unwrap();
        let error = decode_party_merge_lineage_state(&corrupt).unwrap_err();
        assert_eq!(
            error.code.as_str(),
            "IDENTITY_RESOLUTION_MERGE_PERSISTED_STATE_INVALID"
        );
    }

    #[test]
    fn noncanonical_display_name_is_rejected() {
        let bytes = encode_party_merge_lineage_state(&lineage()).unwrap();
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["display_name_survivorship"]["absorbed_value"] =
            serde_json::json!("  Beta   Person  ");
        let corrupt = serde_json::to_vec(&value).unwrap();
        assert!(decode_party_merge_lineage_state(&corrupt).is_err());
    }

    #[test]
    fn corrupt_directional_merge_id_is_rejected() {
        let bytes = encode_party_merge_lineage_state(&lineage()).unwrap();
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let survivor = value["survivor_party_id"].as_str().unwrap().to_owned();
        let absorbed = value["absorbed_party_id"].as_str().unwrap().to_owned();
        value["survivor_party_id"] = serde_json::json!(absorbed);
        value["absorbed_party_id"] = serde_json::json!(survivor);
        let corrupt = serde_json::to_vec(&value).unwrap();
        assert!(decode_party_merge_lineage_state(&corrupt).is_err());
    }

    #[test]
    fn unreachable_persisted_version_is_rejected() {
        let bytes = encode_party_merge_lineage_state(&lineage()).unwrap();
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["version"] = serde_json::json!(2);
        let corrupt = serde_json::to_vec(&value).unwrap();
        assert!(decode_party_merge_lineage_state(&corrupt).is_err());
    }
}
