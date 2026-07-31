use crm_capability_runtime::CapabilityDefinition;
use crm_customer_privacy::PrivacyOwnerActionCommand;
use crm_customer_privacy_owner_scope_support::{
    OwnerPrivacyActionPlanner, OwnerPrivacyActionPolicy, owner_action_definition,
};
use crm_module_sdk::{RecordSnapshot, SdkError, TypedPayload};
use crm_parties::{Party, PartyKind, UpdateParty};
use crm_parties_capability_adapter::{
    RECORD_TYPE, party_from_snapshot, persisted_payload,
};

pub const OWNER_ACTION_CAPABILITY_ID: &str = "parties.privacy.action.apply";

pub type PartiesPrivacyActionPlanner = OwnerPrivacyActionPlanner<PartiesPrivacyActionPolicy>;

#[derive(Debug, Default, Clone, Copy)]
pub struct PartiesPrivacyActionPolicy;

pub fn parties_privacy_action_definition() -> Result<CapabilityDefinition, SdkError> {
    owner_action_definition(crm_parties::MODULE_ID, OWNER_ACTION_CAPABILITY_ID)
}

pub const fn parties_privacy_action_planner() -> PartiesPrivacyActionPlanner {
    OwnerPrivacyActionPlanner::new(PartiesPrivacyActionPolicy)
}

impl OwnerPrivacyActionPolicy for PartiesPrivacyActionPolicy {
    fn owner_module_id(&self) -> &'static str {
        crm_parties::MODULE_ID
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
        minimize_party(command, current, "minimized")
    }

    fn deletion_tombstone(
        &self,
        command: &PrivacyOwnerActionCommand,
        current: &RecordSnapshot,
    ) -> Result<TypedPayload, SdkError> {
        minimize_party(command, current, "deleted")
    }
}

fn minimize_party(
    command: &PrivacyOwnerActionCommand,
    current: &RecordSnapshot,
    state: &str,
) -> Result<TypedPayload, SdkError> {
    let mut party = party_from_snapshot(current)?;
    let expected_version = party.version();
    let display_name = minimized_display_name(&party, command.item_digest(), state);
    party.apply_update(UpdateParty {
        expected_version,
        display_name,
        occurred_at_unix_nanos: command.planned_at_unix_nanos(),
    })?;
    persisted_payload(&party)
}

fn minimized_display_name(party: &Party, digest: &[u8; 32], state: &str) -> String {
    let kind = match party.kind() {
        PartyKind::Person => "person",
        PartyKind::Organization => "organization",
    };
    let suffix = hex_prefix(digest);
    let candidate = format!("{state} {kind} {suffix}");
    if candidate == party.display_name() {
        format!("{state} {kind} {suffix}-v")
    } else {
        candidate
    }
}

fn hex_prefix(bytes: &[u8; 32]) -> String {
    let mut output = String::with_capacity(24);
    for byte in bytes.iter().take(12) {
        use std::fmt::Write as _;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_parties::{CreateParty, PartyId};

    fn party(kind: PartyKind, display_name: &str) -> Party {
        Party::create(CreateParty {
            party_id: PartyId::try_new("party-owner-action-1").unwrap(),
            kind,
            display_name: display_name.to_owned(),
            occurred_at_unix_nanos: 10,
        })
        .unwrap()
    }

    #[test]
    fn publishes_the_frozen_owner_action_coordinate() {
        let definition = parties_privacy_action_definition().unwrap();
        assert_eq!(definition.owner_module_id.as_str(), crm_parties::MODULE_ID);
        assert_eq!(
            definition.capability_id.as_str(),
            OWNER_ACTION_CAPABILITY_ID
        );
        assert!(definition.mutation);
        assert!(definition.requires_idempotency);
    }

    #[test]
    fn minimized_labels_are_deterministic_kind_aware_and_non_identifying() {
        let digest = [0xabu8; 32];
        let person = minimized_display_name(&party(PartyKind::Person, "Ada"), &digest, "minimized");
        let organization = minimized_display_name(
            &party(PartyKind::Organization, "Northwind"),
            &digest,
            "deleted",
        );

        assert_eq!(person, "minimized person abababababababababababab");
        assert_eq!(organization, "deleted organization abababababababababababab");
        assert!(!person.contains("Ada"));
        assert!(!organization.contains("Northwind"));
    }

    #[test]
    fn minimized_label_cannot_be_a_semantic_no_op() {
        let digest = [0x11u8; 32];
        let current = "minimized person 111111111111111111111111";
        let label = minimized_display_name(&party(PartyKind::Person, current), &digest, "minimized");
        assert_eq!(label, "minimized person 111111111111111111111111-v");
    }
}
