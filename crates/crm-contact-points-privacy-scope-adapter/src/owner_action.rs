use crm_capability_runtime::CapabilityDefinition;
use crm_contact_points_capability_adapter::{
    RECORD_TYPE, contact_point_from_snapshot, persisted_payload,
};
use crm_customer_privacy_owner_scope_support::{
    OwnerPrivacyActionPlanner, OwnerPrivacyActionPolicy, PrivacyOwnerActionCommand,
    owner_action_definition,
};
use crm_module_sdk::{RecordSnapshot, SdkError, TypedPayload};

pub const OWNER_ACTION_CAPABILITY_ID: &str = "contact_points.privacy.action.apply";

pub type ContactPointsPrivacyActionPlanner =
    OwnerPrivacyActionPlanner<ContactPointsPrivacyActionPolicy>;

#[derive(Debug, Default, Clone, Copy)]
pub struct ContactPointsPrivacyActionPolicy;

pub fn contact_points_privacy_action_definition() -> Result<CapabilityDefinition, SdkError> {
    owner_action_definition(crm_contact_points::MODULE_ID, OWNER_ACTION_CAPABILITY_ID)
}

pub const fn contact_points_privacy_action_planner() -> ContactPointsPrivacyActionPlanner {
    OwnerPrivacyActionPlanner::new(ContactPointsPrivacyActionPolicy)
}

impl OwnerPrivacyActionPolicy for ContactPointsPrivacyActionPolicy {
    fn owner_module_id(&self) -> &'static str {
        crm_contact_points::MODULE_ID
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
        minimize_contact_point(command, current)
    }

    fn deletion_tombstone(
        &self,
        command: &PrivacyOwnerActionCommand,
        current: &RecordSnapshot,
    ) -> Result<TypedPayload, SdkError> {
        minimize_contact_point(command, current)
    }
}

fn minimize_contact_point(
    command: &PrivacyOwnerActionCommand,
    current: &RecordSnapshot,
) -> Result<TypedPayload, SdkError> {
    let mut contact_point = contact_point_from_snapshot(current)?;
    contact_point.redact(contact_point.version(), command.planned_at_unix_nanos())?;
    persisted_payload(&contact_point)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publishes_the_frozen_owner_action_coordinate() {
        let definition = contact_points_privacy_action_definition().unwrap();
        assert_eq!(
            definition.owner_module_id.as_str(),
            crm_contact_points::MODULE_ID
        );
        assert_eq!(
            definition.capability_id.as_str(),
            OWNER_ACTION_CAPABILITY_ID
        );
        assert!(definition.mutation);
        assert!(definition.requires_idempotency);
    }
}
