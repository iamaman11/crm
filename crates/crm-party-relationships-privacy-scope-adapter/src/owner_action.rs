use crm_capability_runtime::CapabilityDefinition;
use crm_customer_privacy::PrivacyOwnerActionCommand;
use crm_customer_privacy_owner_scope_support::{
    OwnerPrivacyActionPlanner, OwnerPrivacyActionPolicy, owner_action_definition,
};
use crm_module_sdk::{ErrorCategory, RecordSnapshot, SdkError, TypedPayload};
use crm_party_relationships::{PartyRelationship, PartyRelationshipStatus};
use crm_party_relationships_capability_adapter::{
    RECORD_TYPE, party_relationship_from_snapshot, persisted_payload,
};

pub const OWNER_ACTION_CAPABILITY_ID: &str = "party_relationships.privacy.action.apply";

pub type PartyRelationshipsPrivacyActionPlanner =
    OwnerPrivacyActionPlanner<PartyRelationshipsPrivacyActionPolicy>;

#[derive(Debug, Default, Clone, Copy)]
pub struct PartyRelationshipsPrivacyActionPolicy;

pub fn party_relationships_privacy_action_definition() -> Result<CapabilityDefinition, SdkError> {
    owner_action_definition(
        crm_party_relationships::MODULE_ID,
        OWNER_ACTION_CAPABILITY_ID,
    )
}

pub const fn party_relationships_privacy_action_planner() -> PartyRelationshipsPrivacyActionPlanner
{
    OwnerPrivacyActionPlanner::new(PartyRelationshipsPrivacyActionPolicy)
}

impl OwnerPrivacyActionPolicy for PartyRelationshipsPrivacyActionPolicy {
    fn owner_module_id(&self) -> &'static str {
        crm_party_relationships::MODULE_ID
    }

    fn capability_id(&self) -> &'static str {
        OWNER_ACTION_CAPABILITY_ID
    }

    fn supports_resource_type(&self, resource_type: &str) -> bool {
        resource_type == RECORD_TYPE
    }

    fn anonymize(
        &self,
        command: &PrivacyOwnerActionCommand,
        current: &RecordSnapshot,
    ) -> Result<TypedPayload, SdkError> {
        minimize_relationship(command, current)
    }

    fn deletion_tombstone(
        &self,
        command: &PrivacyOwnerActionCommand,
        current: &RecordSnapshot,
    ) -> Result<TypedPayload, SdkError> {
        minimize_relationship(command, current)
    }
}

fn minimize_relationship(
    command: &PrivacyOwnerActionCommand,
    current: &RecordSnapshot,
) -> Result<TypedPayload, SdkError> {
    let relationship = party_relationship_from_snapshot(current)?;
    let minimized = privacy_transition(relationship, command.planned_at_unix_nanos())?;
    persisted_payload(&minimized)
}

fn privacy_transition(
    relationship: PartyRelationship,
    occurred_at_unix_nanos: i64,
) -> Result<PartyRelationship, SdkError> {
    if occurred_at_unix_nanos < relationship.updated_at_unix_nanos() {
        return Err(transition_invalid(
            "owner action time precedes the authoritative relationship state",
        ));
    }
    let next_version = relationship
        .version()
        .checked_add(1)
        .ok_or_else(|| transition_invalid("Party Relationship version overflowed"))?;
    let mut snapshot = relationship.snapshot();
    snapshot.status = PartyRelationshipStatus::Inactive;
    snapshot.valid_from_unix_nanos = None;
    snapshot.valid_until_unix_nanos = None;
    snapshot.updated_at_unix_nanos = occurred_at_unix_nanos;
    snapshot.version = next_version;
    PartyRelationship::rehydrate(snapshot)
}

fn transition_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "PARTY_RELATIONSHIPS_PRIVACY_TRANSITION_INVALID",
        ErrorCategory::Conflict,
        false,
        "The Party Relationship privacy transition could not be applied safely.",
    )
    .with_internal_reference(reference)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_party_relationships::{
        CreatePartyRelationship, PartyReference, PartyRelationshipId, RelationshipType,
    };

    fn relationship() -> PartyRelationship {
        PartyRelationship::create(CreatePartyRelationship {
            party_relationship_id: PartyRelationshipId::try_new("relationship-owner-action-1")
                .unwrap(),
            from_party_ref: PartyReference::try_new("party-owner-action-1").unwrap(),
            to_party_ref: PartyReference::try_new("party-owner-action-2").unwrap(),
            relationship_type: RelationshipType::employment(),
            valid_from_unix_nanos: Some(10),
            valid_until_unix_nanos: Some(100),
            occurred_at_unix_nanos: 10,
        })
        .unwrap()
    }

    #[test]
    fn publishes_the_frozen_owner_action_coordinate() {
        let definition = party_relationships_privacy_action_definition().unwrap();
        assert_eq!(
            definition.owner_module_id.as_str(),
            crm_party_relationships::MODULE_ID
        );
        assert_eq!(
            definition.capability_id.as_str(),
            OWNER_ACTION_CAPABILITY_ID
        );
        assert!(definition.mutation);
        assert!(definition.requires_idempotency);
    }

    #[test]
    fn privacy_transition_preserves_topology_and_semantics() {
        let current = relationship();
        let from = current.from_party_ref().as_str().to_owned();
        let to = current.to_party_ref().as_str().to_owned();
        let relationship_type = current.relationship_type().clone();

        let minimized = privacy_transition(current, 20).unwrap();
        assert_eq!(minimized.from_party_ref().as_str(), from);
        assert_eq!(minimized.to_party_ref().as_str(), to);
        assert_eq!(minimized.relationship_type(), &relationship_type);
        assert_eq!(minimized.status(), PartyRelationshipStatus::Inactive);
        assert_eq!(minimized.valid_from_unix_nanos(), None);
        assert_eq!(minimized.valid_until_unix_nanos(), None);
        assert_eq!(minimized.version(), 2);
    }

    #[test]
    fn repeated_privacy_transition_advances_version_without_reintroducing_state() {
        let first = privacy_transition(relationship(), 20).unwrap();
        let second = privacy_transition(first, 20).unwrap();
        assert_eq!(second.status(), PartyRelationshipStatus::Inactive);
        assert_eq!(second.valid_from_unix_nanos(), None);
        assert_eq!(second.valid_until_unix_nanos(), None);
        assert_eq!(second.version(), 3);
    }

    #[test]
    fn privacy_transition_rejects_time_regression() {
        let error = privacy_transition(relationship(), 9).unwrap_err();
        assert_eq!(error.code, "PARTY_RELATIONSHIPS_PRIVACY_TRANSITION_INVALID");
    }
}
