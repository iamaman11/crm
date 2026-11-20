use crm_capability_runtime::CapabilityDefinition;
use crm_customer_privacy_owner_scope_support::{
    OwnerPrivacyActionPlanner, OwnerPrivacyActionPolicy, PrivacyOwnerActionCommand,
    owner_action_definition, unsupported_owner_action,
};
use crm_identity_resolution::{
    DecisionReasonCode, DuplicateCandidateCase, DuplicateCandidateCaseStatus, EvidenceReference,
    MatchEvidenceSnapshot, MatchSignal, MatcherProfileCode, SignalKindCode, SignalSourceCode,
};
use crm_identity_resolution_capability_adapter::{
    MERGE_OPERATION_RECORD_TYPE, RECORD_TYPE, duplicate_candidate_case_from_snapshot,
    merge_operation_from_snapshot, persisted_payload,
};
use crm_module_sdk::{ErrorCategory, RecordSnapshot, SdkError, TypedPayload};

const MAX_EVIDENCE_HISTORY: usize = 64;

pub const OWNER_ACTION_CAPABILITY_ID: &str = "identity_resolution.privacy.action.apply";

pub type IdentityResolutionPrivacyActionPlanner =
    OwnerPrivacyActionPlanner<IdentityResolutionPrivacyActionPolicy>;

#[derive(Debug, Default, Clone, Copy)]
pub struct IdentityResolutionPrivacyActionPolicy;

pub fn identity_resolution_privacy_action_definition() -> Result<CapabilityDefinition, SdkError> {
    owner_action_definition(
        crm_identity_resolution::MODULE_ID,
        OWNER_ACTION_CAPABILITY_ID,
    )
}

pub const fn identity_resolution_privacy_action_planner() -> IdentityResolutionPrivacyActionPlanner
{
    OwnerPrivacyActionPlanner::new(IdentityResolutionPrivacyActionPolicy)
}

impl OwnerPrivacyActionPolicy for IdentityResolutionPrivacyActionPolicy {
    fn owner_module_id(&self) -> &'static str {
        crm_identity_resolution::MODULE_ID
    }

    fn capability_id(&self) -> &'static str {
        OWNER_ACTION_CAPABILITY_ID
    }

    fn supports_resource_type(&self, resource_type: &str) -> bool {
        matches!(resource_type, RECORD_TYPE | MERGE_OPERATION_RECORD_TYPE)
    }

    fn anonymize(
        &self,
        command: &PrivacyOwnerActionCommand,
        current: &RecordSnapshot,
    ) -> Result<TypedPayload, SdkError> {
        plan_resource_action(command, current)
    }

    fn deletion_tombstone(
        &self,
        command: &PrivacyOwnerActionCommand,
        current: &RecordSnapshot,
    ) -> Result<TypedPayload, SdkError> {
        plan_resource_action(command, current)
    }
}

fn plan_resource_action(
    command: &PrivacyOwnerActionCommand,
    current: &RecordSnapshot,
) -> Result<TypedPayload, SdkError> {
    match command.resource_type() {
        RECORD_TYPE => minimize_candidate_case(command, current),
        MERGE_OPERATION_RECORD_TYPE => {
            let _operation = merge_operation_from_snapshot(current)?;
            Err(unsupported_owner_action(
                crm_identity_resolution::MODULE_ID,
                command.resource_type(),
                command.action_code(),
            ))
        }
        _ => Err(transition_invalid(
            "unsupported Identity Resolution resource type reached the owner policy",
        )),
    }
}

fn minimize_candidate_case(
    command: &PrivacyOwnerActionCommand,
    current: &RecordSnapshot,
) -> Result<TypedPayload, SdkError> {
    let case = duplicate_candidate_case_from_snapshot(current)?;
    let minimized =
        candidate_privacy_transition(case, command.item_digest(), command.planned_at_unix_nanos())?;
    persisted_payload(&minimized)
}

fn candidate_privacy_transition(
    case: DuplicateCandidateCase,
    digest: &[u8; 32],
    occurred_at_unix_nanos: i64,
) -> Result<DuplicateCandidateCase, SdkError> {
    let mut snapshot = case.snapshot();
    if occurred_at_unix_nanos < snapshot.updated_at_unix_nanos {
        return Err(transition_invalid(
            "owner action time precedes the authoritative candidate case",
        ));
    }

    let was_terminal = snapshot.status != DuplicateCandidateCaseStatus::Open;
    let original = std::mem::take(&mut snapshot.evidence_history);
    snapshot.evidence_history = original
        .iter()
        .enumerate()
        .map(|(index, evidence)| minimized_evidence(evidence, digest, index))
        .collect::<Result<Vec<_>, _>>()?;

    if was_terminal {
        if snapshot.evidence_history.len() >= MAX_EVIDENCE_HISTORY {
            return Err(transition_invalid(
                "terminal candidate evidence history is already at its supported bound",
            ));
        }
        let current = snapshot
            .evidence_history
            .last()
            .ok_or_else(|| transition_invalid("candidate evidence history is empty"))?;
        snapshot.evidence_history.push(minimized_evidence_at(
            current,
            digest,
            snapshot.evidence_history.len(),
            occurred_at_unix_nanos,
        )?);
    } else {
        snapshot.status = DuplicateCandidateCaseStatus::Dismissed;
    }

    snapshot.decision_reason = Some(DecisionReasonCode::try_new("privacy_minimized")?);
    snapshot.updated_at_unix_nanos = occurred_at_unix_nanos;
    snapshot.version = snapshot
        .version
        .checked_add(1)
        .ok_or_else(|| transition_invalid("candidate case version overflowed"))?;
    DuplicateCandidateCase::rehydrate(snapshot)
}

fn minimized_evidence(
    evidence: &MatchEvidenceSnapshot,
    digest: &[u8; 32],
    index: usize,
) -> Result<MatchEvidenceSnapshot, SdkError> {
    minimized_evidence_at(evidence, digest, index, evidence.generated_at_unix_nanos())
}

fn minimized_evidence_at(
    evidence: &MatchEvidenceSnapshot,
    digest: &[u8; 32],
    index: usize,
    generated_at_unix_nanos: i64,
) -> Result<MatchEvidenceSnapshot, SdkError> {
    MatchEvidenceSnapshot::try_new(
        evidence.pair().left().clone(),
        evidence.left_party_version(),
        evidence.pair().right().clone(),
        evidence.right_party_version(),
        MatcherProfileCode::try_new("privacy_minimized")?,
        0,
        vec![MatchSignal::try_new(
            SignalKindCode::try_new("privacy_minimized")?,
            SignalSourceCode::try_new("customer_privacy")?,
            EvidenceReference::try_new(format!(
                "privacy-minimized-{}-{index}",
                hex_prefix(digest)
            ))?,
            0,
        )?],
        generated_at_unix_nanos,
    )
}

fn hex_prefix(bytes: &[u8; 32]) -> String {
    let mut output = String::with_capacity(24);
    for byte in bytes.iter().take(12) {
        use std::fmt::Write as _;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

fn transition_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_PRIVACY_TRANSITION_INVALID",
        ErrorCategory::Conflict,
        false,
        "The Identity Resolution privacy transition could not be applied safely.",
    )
    .with_internal_reference(reference)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publishes_the_frozen_owner_action_coordinate() {
        let definition = identity_resolution_privacy_action_definition().unwrap();
        assert_eq!(
            definition.owner_module_id.as_str(),
            crm_identity_resolution::MODULE_ID
        );
        assert_eq!(
            definition.capability_id.as_str(),
            OWNER_ACTION_CAPABILITY_ID
        );
        assert!(definition.mutation);
        assert!(definition.requires_idempotency);
    }

    #[test]
    fn supports_both_authoritative_identity_resolution_record_types() {
        let policy = IdentityResolutionPrivacyActionPolicy;
        assert!(policy.supports_resource_type(RECORD_TYPE));
        assert!(policy.supports_resource_type(MERGE_OPERATION_RECORD_TYPE));
        assert!(!policy.supports_resource_type("identity_resolution.projection"));
    }

    #[test]
    fn privacy_evidence_reference_is_deterministic_and_bounded() {
        let reference = format!("privacy-minimized-{}-63", hex_prefix(&[0xabu8; 32]));
        assert_eq!(reference, "privacy-minimized-abababababababababababab-63");
        assert!(reference.len() < 512);
    }
}
