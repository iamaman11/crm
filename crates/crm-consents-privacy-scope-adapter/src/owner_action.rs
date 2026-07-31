use crm_capability_runtime::CapabilityDefinition;
use crm_consents_capability_adapter::{
    RECORD_TYPE, consent_authorization_from_snapshot,
};
use crm_customer_privacy::PrivacyOwnerActionCommand;
use crm_customer_privacy_owner_scope_support::{
    OwnerPrivacyActionPlanner, OwnerPrivacyActionPolicy, owner_action_definition,
    unsupported_owner_action,
};
use crm_module_sdk::{RecordSnapshot, SdkError, TypedPayload};

pub const OWNER_ACTION_CAPABILITY_ID: &str = "consents.privacy.action.apply";

pub type ConsentsPrivacyActionPlanner = OwnerPrivacyActionPlanner<ConsentsPrivacyActionPolicy>;

#[derive(Debug, Default, Clone, Copy)]
pub struct ConsentsPrivacyActionPolicy;

pub fn consents_privacy_action_definition() -> Result<CapabilityDefinition, SdkError> {
    owner_action_definition(crm_consents::MODULE_ID, OWNER_ACTION_CAPABILITY_ID)
}

pub const fn consents_privacy_action_planner() -> ConsentsPrivacyActionPlanner {
    OwnerPrivacyActionPlanner::new(ConsentsPrivacyActionPolicy)
}

impl OwnerPrivacyActionPolicy for ConsentsPrivacyActionPolicy {
    fn owner_module_id(&self) -> &'static str {
        crm_consents::MODULE_ID
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
        reject_immutable_action(command, current)
    }

    fn deletion_tombstone(
        &self,
        command: &PrivacyOwnerActionCommand,
        current: &RecordSnapshot,
    ) -> Result<TypedPayload, SdkError> {
        reject_immutable_action(command, current)
    }
}

fn reject_immutable_action(
    command: &PrivacyOwnerActionCommand,
    current: &RecordSnapshot,
) -> Result<TypedPayload, SdkError> {
    let _authorization = consent_authorization_from_snapshot(current)?;
    Err(unsupported_owner_action(
        crm_consents::MODULE_ID,
        command.resource_type(),
        command.action_code(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publishes_the_frozen_owner_action_coordinate() {
        let definition = consents_privacy_action_definition().unwrap();
        assert_eq!(definition.owner_module_id.as_str(), crm_consents::MODULE_ID);
        assert_eq!(
            definition.capability_id.as_str(),
            OWNER_ACTION_CAPABILITY_ID
        );
        assert!(definition.mutation);
        assert!(definition.requires_idempotency);
    }

    #[test]
    fn supports_only_the_authoritative_consent_record_type() {
        let policy = ConsentsPrivacyActionPolicy;
        assert!(policy.supports_resource_type(RECORD_TYPE));
        assert!(!policy.supports_resource_type("consents.projection"));
    }
}
