use crate::domain::{EvidenceReference, PartyReference};
use crate::merge_lineage::{
    CreateMergeOperation, DecisionReference, FieldPath, LineageDecisionReasonCode, MergeOperation,
    MergeOperationId, MergeOperationStatus, SourceValueDigest, SurvivorshipSelection,
    UnmergeMergeOperation,
};
use crm_module_sdk::{ActorId, ErrorCategory, SdkError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;

pub const MERGE_OPERATION_STATE_SCHEMA_ID: &str = "crm.identity_resolution.merge_operation.state";
pub const MERGE_OPERATION_STATE_SCHEMA_VERSION: &str = "1.0.0";
pub const MERGE_OPERATION_STATE_MAXIMUM_BYTES: u64 = 512 * 1024;
pub const MERGE_OPERATION_STATE_RETENTION_POLICY_ID: &str = "crm.identity_resolution.merge_lineage";
const MERGE_OPERATION_STATE_DESCRIPTOR: &[u8] = b"crm.identity_resolution.merge_operation.state/v1:operation_id,source_party_id,source_party_version,survivor_party_id,survivor_party_version,decision_ref,decided_by,reason,survivorship[field_path,provenance_party_id,provenance_party_version,source_value_sha256,evidence_ref],status,unmerge_decision[decision_ref,decided_by,reason,occurred_at_unix_nanos],created_at_unix_nanos,updated_at_unix_nanos,version";

pub fn merge_operation_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(MERGE_OPERATION_STATE_DESCRIPTOR).into()
}

pub fn encode_merge_operation_state(operation: &MergeOperation) -> Result<Vec<u8>, SdkError> {
    let bytes = serde_json::to_vec(&MergeOperationStateV1::from(operation)).map_err(|error| {
        persisted_error(format!(
            "Identity Resolution merge-operation state serialization failed: {error}"
        ))
    })?;
    validate_size(&bytes)?;
    Ok(bytes)
}

pub fn decode_merge_operation_state(bytes: &[u8]) -> Result<MergeOperation, SdkError> {
    validate_size(bytes)?;
    let state: MergeOperationStateV1 = serde_json::from_slice(bytes).map_err(|error| {
        persisted_error(format!(
            "Identity Resolution merge-operation state JSON is invalid: {error}"
        ))
    })?;
    let persisted_shape = state.persisted_shape();
    let operation = state.into_domain()?;
    validate_rehydrated_shape(&operation, &persisted_shape)?;
    let canonical = encode_merge_operation_state(&operation)?;
    if canonical != bytes {
        return Err(persisted_error(
            "persisted merge-operation state is not in canonical deterministic representation",
        ));
    }
    Ok(operation)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MergeOperationStateV1 {
    operation_id: String,
    source_party_id: String,
    source_party_version: i64,
    survivor_party_id: String,
    survivor_party_version: i64,
    decision_ref: String,
    decided_by: String,
    reason: String,
    survivorship: Vec<SurvivorshipSelectionStateV1>,
    status: MergeOperationStatusState,
    unmerge_decision: Option<UnmergeDecisionStateV1>,
    created_at_unix_nanos: i64,
    updated_at_unix_nanos: i64,
    version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SurvivorshipSelectionStateV1 {
    field_path: String,
    provenance_party_id: String,
    provenance_party_version: i64,
    source_value_sha256: String,
    evidence_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct UnmergeDecisionStateV1 {
    decision_ref: String,
    decided_by: String,
    reason: String,
    occurred_at_unix_nanos: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum MergeOperationStatusState {
    Active,
    Unmerged,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PersistedShape {
    status: MergeOperationStatusState,
    created_at_unix_nanos: i64,
    updated_at_unix_nanos: i64,
    version: i64,
}

impl MergeOperationStateV1 {
    fn persisted_shape(&self) -> PersistedShape {
        PersistedShape {
            status: self.status,
            created_at_unix_nanos: self.created_at_unix_nanos,
            updated_at_unix_nanos: self.updated_at_unix_nanos,
            version: self.version,
        }
    }

    fn into_domain(self) -> Result<MergeOperation, SdkError> {
        let operation_id = parse_canonical(
            self.operation_id,
            MergeOperationId::try_new,
            "merge operation identifier",
        )?;
        let source_party_ref = parse_canonical(
            self.source_party_id,
            PartyReference::try_new,
            "source Party reference",
        )?;
        let survivor_party_ref = parse_canonical(
            self.survivor_party_id,
            PartyReference::try_new,
            "survivor Party reference",
        )?;
        let decision_ref = parse_canonical(
            self.decision_ref,
            DecisionReference::try_new,
            "merge decision reference",
        )?;
        let decided_by = ActorId::try_new(self.decided_by.clone())
            .map_err(|error| persisted_error(error.to_string()))?;
        if decided_by.as_str() != self.decided_by {
            return Err(persisted_error("persisted merge actor is not canonical"));
        }
        let reason = parse_canonical(
            self.reason,
            LineageDecisionReasonCode::try_new,
            "merge decision reason",
        )?;
        let survivorship = self
            .survivorship
            .into_iter()
            .map(SurvivorshipSelectionStateV1::into_domain)
            .collect::<Result<Vec<_>, _>>()?;

        let mut operation = MergeOperation::create(CreateMergeOperation {
            operation_id,
            source_party_ref,
            source_party_version: self.source_party_version,
            survivor_party_ref,
            survivor_party_version: self.survivor_party_version,
            decision_ref,
            decided_by,
            reason,
            survivorship,
            occurred_at_unix_nanos: self.created_at_unix_nanos,
        })
        .map_err(domain_error)?;

        match (self.status, self.unmerge_decision) {
            (MergeOperationStatusState::Active, None) => {}
            (MergeOperationStatusState::Active, Some(_)) => {
                return Err(persisted_error(
                    "an active persisted merge operation cannot contain unmerge decision evidence",
                ));
            }
            (MergeOperationStatusState::Unmerged, Some(decision)) => {
                operation
                    .unmerge(UnmergeMergeOperation {
                        expected_version: 1,
                        decision_ref: parse_canonical(
                            decision.decision_ref,
                            DecisionReference::try_new,
                            "unmerge decision reference",
                        )?,
                        decided_by: parse_actor(decision.decided_by)?,
                        reason: parse_canonical(
                            decision.reason,
                            LineageDecisionReasonCode::try_new,
                            "unmerge decision reason",
                        )?,
                        occurred_at_unix_nanos: decision.occurred_at_unix_nanos,
                    })
                    .map_err(domain_error)?;
            }
            (MergeOperationStatusState::Unmerged, None) => {
                return Err(persisted_error(
                    "an unmerged persisted merge operation must contain unmerge decision evidence",
                ));
            }
        }
        Ok(operation)
    }
}

impl From<&MergeOperation> for MergeOperationStateV1 {
    fn from(value: &MergeOperation) -> Self {
        Self {
            operation_id: value.operation_id().as_str().to_owned(),
            source_party_id: value.source_party_ref().as_str().to_owned(),
            source_party_version: value.source_party_version(),
            survivor_party_id: value.survivor_party_ref().as_str().to_owned(),
            survivor_party_version: value.survivor_party_version(),
            decision_ref: value.decision_ref().as_str().to_owned(),
            decided_by: value.decided_by().as_str().to_owned(),
            reason: value.reason().as_str().to_owned(),
            survivorship: value
                .survivorship()
                .iter()
                .map(SurvivorshipSelectionStateV1::from)
                .collect(),
            status: value.status().into(),
            unmerge_decision: value.unmerge_decision().map(UnmergeDecisionStateV1::from),
            created_at_unix_nanos: value.created_at_unix_nanos(),
            updated_at_unix_nanos: value.updated_at_unix_nanos(),
            version: value.version(),
        }
    }
}

impl SurvivorshipSelectionStateV1 {
    fn into_domain(self) -> Result<SurvivorshipSelection, SdkError> {
        SurvivorshipSelection::try_new(
            parse_canonical(
                self.field_path,
                FieldPath::try_new,
                "survivorship field path",
            )?,
            parse_canonical(
                self.provenance_party_id,
                PartyReference::try_new,
                "survivorship provenance Party reference",
            )?,
            self.provenance_party_version,
            SourceValueDigest::from_bytes(parse_sha256_hex(&self.source_value_sha256)?),
            parse_canonical(
                self.evidence_ref,
                EvidenceReference::try_new,
                "survivorship evidence reference",
            )?,
        )
        .map_err(domain_error)
    }
}

impl From<&SurvivorshipSelection> for SurvivorshipSelectionStateV1 {
    fn from(value: &SurvivorshipSelection) -> Self {
        Self {
            field_path: value.field_path().as_str().to_owned(),
            provenance_party_id: value.provenance_party_ref().as_str().to_owned(),
            provenance_party_version: value.provenance_party_version(),
            source_value_sha256: sha256_hex(value.source_value_digest().as_bytes()),
            evidence_ref: value.evidence_ref().as_str().to_owned(),
        }
    }
}

impl From<&crate::merge_lineage::UnmergeDecision> for UnmergeDecisionStateV1 {
    fn from(value: &crate::merge_lineage::UnmergeDecision) -> Self {
        Self {
            decision_ref: value.decision_ref().as_str().to_owned(),
            decided_by: value.decided_by().as_str().to_owned(),
            reason: value.reason().as_str().to_owned(),
            occurred_at_unix_nanos: value.occurred_at_unix_nanos(),
        }
    }
}

impl From<MergeOperationStatus> for MergeOperationStatusState {
    fn from(value: MergeOperationStatus) -> Self {
        match value {
            MergeOperationStatus::Active => Self::Active,
            MergeOperationStatus::Unmerged => Self::Unmerged,
        }
    }
}

fn validate_rehydrated_shape(
    operation: &MergeOperation,
    persisted: &PersistedShape,
) -> Result<(), SdkError> {
    if MergeOperationStatusState::from(operation.status()) != persisted.status
        || operation.created_at_unix_nanos() != persisted.created_at_unix_nanos
        || operation.updated_at_unix_nanos() != persisted.updated_at_unix_nanos
        || operation.version() != persisted.version
    {
        return Err(persisted_error(
            "persisted merge-operation lifecycle shape is not reachable from the owner domain",
        ));
    }
    Ok(())
}

fn parse_actor(raw: String) -> Result<ActorId, SdkError> {
    let parsed =
        ActorId::try_new(raw.clone()).map_err(|error| persisted_error(error.to_string()))?;
    if parsed.as_str() != raw {
        return Err(persisted_error(
            "persisted actor identifier is not canonical",
        ));
    }
    Ok(parsed)
}

fn parse_canonical<T>(
    raw: String,
    parser: impl FnOnce(String) -> Result<T, SdkError>,
    label: &str,
) -> Result<T, SdkError>
where
    T: CanonicalString,
{
    let parsed = parser(raw.clone()).map_err(domain_error)?;
    if parsed.canonical_str() != raw {
        return Err(persisted_error(format!(
            "persisted {label} is not canonical"
        )));
    }
    Ok(parsed)
}

trait CanonicalString {
    fn canonical_str(&self) -> &str;
}

macro_rules! canonical_string {
    ($type:ty) => {
        impl CanonicalString for $type {
            fn canonical_str(&self) -> &str {
                self.as_str()
            }
        }
    };
}

canonical_string!(MergeOperationId);
canonical_string!(PartyReference);
canonical_string!(DecisionReference);
canonical_string!(LineageDecisionReasonCode);
canonical_string!(FieldPath);
canonical_string!(EvidenceReference);

fn parse_sha256_hex(value: &str) -> Result<[u8; 32], SdkError> {
    if value.len() != 64
        || value
            .as_bytes()
            .iter()
            .any(|byte| !byte.is_ascii_hexdigit() || byte.is_ascii_uppercase())
    {
        return Err(persisted_error(
            "persisted source-value SHA-256 digest must be exactly 64 lowercase hexadecimal characters",
        ));
    }
    let mut bytes = [0_u8; 32];
    for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
        let high = hex_nibble(chunk[0])?;
        let low = hex_nibble(chunk[1])?;
        bytes[index] = (high << 4) | low;
    }
    Ok(bytes)
}

fn hex_nibble(byte: u8) -> Result<u8, SdkError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(persisted_error(
            "persisted source-value SHA-256 digest contains an invalid hexadecimal character",
        )),
    }
}

fn sha256_hex(bytes: &[u8; 32]) -> String {
    let mut output = String::with_capacity(64);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn validate_size(bytes: &[u8]) -> Result<(), SdkError> {
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MERGE_OPERATION_STATE_MAXIMUM_BYTES {
        return Err(persisted_error(format!(
            "Identity Resolution merge-operation state exceeds the maximum of {MERGE_OPERATION_STATE_MAXIMUM_BYTES} bytes"
        )));
    }
    Ok(())
}

fn domain_error(error: SdkError) -> SdkError {
    persisted_error(format!("{}: {}", error.code, error.safe_message))
}

fn persisted_error(message: impl Into<String>) -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_MERGE_PERSISTED_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "The persisted Identity Resolution merge-operation state is invalid.",
    )
    .with_internal_reference(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actor(value: &str) -> ActorId {
        ActorId::try_new(value).unwrap()
    }

    fn selection(field: &str, party_id: &str, version: i64) -> SurvivorshipSelection {
        SurvivorshipSelection::try_new(
            FieldPath::try_new(field).unwrap(),
            PartyReference::try_new(party_id).unwrap(),
            version,
            SourceValueDigest::sha256(format!("{field}:{party_id}:{version}")),
            EvidenceReference::try_new(format!("evidence://{field}/{party_id}/{version}")).unwrap(),
        )
        .unwrap()
    }

    fn active_operation() -> MergeOperation {
        MergeOperation::create(CreateMergeOperation {
            operation_id: MergeOperationId::try_new("merge-op-persisted-1").unwrap(),
            source_party_ref: PartyReference::try_new("party-a").unwrap(),
            source_party_version: 3,
            survivor_party_ref: PartyReference::try_new("party-b").unwrap(),
            survivor_party_version: 7,
            decision_ref: DecisionReference::try_new("approval://merge/1").unwrap(),
            decided_by: actor("reviewer-a"),
            reason: LineageDecisionReasonCode::try_new("duplicate.confirmed").unwrap(),
            survivorship: vec![
                selection("display_name", "party-b", 7),
                selection("custom.vip_tier", "party-a", 3),
            ],
            occurred_at_unix_nanos: 100,
        })
        .unwrap()
    }

    fn unmerged_operation() -> MergeOperation {
        let mut operation = active_operation();
        operation
            .unmerge(UnmergeMergeOperation {
                expected_version: 1,
                decision_ref: DecisionReference::try_new("approval://unmerge/1").unwrap(),
                decided_by: actor("reviewer-b"),
                reason: LineageDecisionReasonCode::try_new("merge.reversed").unwrap(),
                occurred_at_unix_nanos: 200,
            })
            .unwrap();
        operation
    }

    #[test]
    fn active_round_trip_is_exact_deterministic_and_descriptor_hash_is_nonzero() {
        let value = active_operation();
        let first = encode_merge_operation_state(&value).unwrap();
        let second = encode_merge_operation_state(&value).unwrap();
        let decoded = decode_merge_operation_state(&first).unwrap();
        assert_eq!(decoded, value);
        assert_eq!(first, second);
        assert_ne!(merge_operation_state_descriptor_hash(), [0; 32]);
    }

    #[test]
    fn unmerged_round_trip_preserves_reversal_evidence() {
        let value = unmerged_operation();
        let bytes = encode_merge_operation_state(&value).unwrap();
        let decoded = decode_merge_operation_state(&bytes).unwrap();
        assert_eq!(decoded, value);
        assert_eq!(decoded.status(), MergeOperationStatus::Unmerged);
        assert_eq!(decoded.version(), 2);
        assert!(decoded.unmerge_decision().is_some());
    }

    #[test]
    fn unknown_fields_and_noncanonical_json_are_rejected() {
        let bytes = encode_merge_operation_state(&active_operation()).unwrap();
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("unknown".to_owned(), serde_json::json!(true));
        assert!(decode_merge_operation_state(&serde_json::to_vec(&value).unwrap()).is_err());

        let mut noncanonical = Vec::from(b" ".as_slice());
        noncanonical.extend_from_slice(&bytes);
        assert!(decode_merge_operation_state(&noncanonical).is_err());
    }

    #[test]
    fn uppercase_digest_and_noncanonical_reason_are_rejected() {
        let bytes = encode_merge_operation_state(&active_operation()).unwrap();
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["survivorship"][0]["source_value_sha256"] = serde_json::json!(
            value["survivorship"][0]["source_value_sha256"]
                .as_str()
                .unwrap()
                .to_ascii_uppercase()
        );
        assert!(decode_merge_operation_state(&serde_json::to_vec(&value).unwrap()).is_err());

        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["reason"] = serde_json::json!(" Duplicate.Confirmed ");
        assert!(decode_merge_operation_state(&serde_json::to_vec(&value).unwrap()).is_err());
    }

    #[test]
    fn corrupt_lifecycle_shape_is_rejected() {
        let bytes = encode_merge_operation_state(&active_operation()).unwrap();
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["status"] = serde_json::json!("unmerged");
        value["version"] = serde_json::json!(2);
        value["updated_at_unix_nanos"] = serde_json::json!(200);
        assert!(decode_merge_operation_state(&serde_json::to_vec(&value).unwrap()).is_err());
    }
}
