use crm_capability_runtime::CapabilityDefinition;
use crm_contact_points::{
    ContactPoint, ContactPointKind, ContactPointStatus, UpdateContactPoint,
};
use crm_contact_points_capability_adapter::{
    RECORD_TYPE, contact_point_from_snapshot, persisted_payload,
};
use crm_customer_privacy::PrivacyOwnerActionCommand;
use crm_customer_privacy_owner_scope_support::{
    OwnerPrivacyActionPlanner, OwnerPrivacyActionPolicy, owner_action_definition,
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
        minimize_contact_point(command, current, false)
    }

    fn deletion_tombstone(
        &self,
        command: &PrivacyOwnerActionCommand,
        current: &RecordSnapshot,
    ) -> Result<TypedPayload, SdkError> {
        minimize_contact_point(command, current, true)
    }
}

fn minimize_contact_point(
    command: &PrivacyOwnerActionCommand,
    current: &RecordSnapshot,
    deleting: bool,
) -> Result<TypedPayload, SdkError> {
    let mut contact_point = contact_point_from_snapshot(current)?;
    let expected_version = contact_point.version();
    let value = minimized_value(
        &contact_point,
        command.item_digest(),
        if deleting { "deleted" } else { "minimized" },
    );
    contact_point.apply_update(UpdateContactPoint {
        expected_version,
        value,
        status: ContactPointStatus::Inactive,
        preferred: false,
        valid_from_unix_nanos: None,
        valid_until_unix_nanos: None,
        occurred_at_unix_nanos: command.planned_at_unix_nanos(),
    })?;
    persisted_payload(&contact_point)
}

fn minimized_value(contact_point: &ContactPoint, digest: &[u8; 32], state: &str) -> String {
    let candidate = value_for_kind(contact_point.kind(), digest, state, false);
    if candidate == contact_point.display_value() {
        value_for_kind(contact_point.kind(), digest, state, true)
    } else {
        candidate
    }
}

fn value_for_kind(
    kind: ContactPointKind,
    digest: &[u8; 32],
    state: &str,
    alternate: bool,
) -> String {
    let suffix = hex_prefix(digest);
    let marker = if alternate { "-v" } else { "" };
    match kind {
        ContactPointKind::Email => {
            format!("{state}{marker}-{suffix}@example.invalid")
        }
        ContactPointKind::Phone => redacted_phone(digest, alternate),
        ContactPointKind::Postal => format!("{state} postal{marker} {suffix}"),
        ContactPointKind::Web => {
            format!("https://redacted.invalid/{state}{marker}/{suffix}")
        }
        ContactPointKind::Messaging => {
            format!("{state}{marker}:{suffix}")
        }
    }
}

fn redacted_phone(digest: &[u8; 32], alternate: bool) -> String {
    let mut output = String::from("+1");
    for byte in digest.iter().take(10) {
        output.push(char::from(b'0' + (byte % 10)));
    }
    if alternate {
        let last = output.pop().unwrap_or('0');
        let digit = last.to_digit(10).unwrap_or(0);
        output.push(char::from(b'0' + u8::try_from((digit + 1) % 10).unwrap_or(0)));
    }
    output
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

    #[test]
    fn every_contact_point_kind_receives_a_non_identifying_valid_shape() {
        let digest = [0xabu8; 32];
        assert_eq!(
            value_for_kind(ContactPointKind::Email, &digest, "minimized", false),
            "minimized-abababababababababababab@example.invalid"
        );
        assert_eq!(
            value_for_kind(ContactPointKind::Phone, &digest, "minimized", false),
            "+11111111111"
        );
        assert_eq!(
            value_for_kind(ContactPointKind::Postal, &digest, "minimized", false),
            "minimized postal abababababababababababab"
        );
        assert_eq!(
            value_for_kind(ContactPointKind::Web, &digest, "minimized", false),
            "https://redacted.invalid/minimized/abababababababababababab"
        );
        assert_eq!(
            value_for_kind(ContactPointKind::Messaging, &digest, "minimized", false),
            "minimized:abababababababababababab"
        );
    }

    #[test]
    fn alternate_phone_stays_e164_shaped_and_differs() {
        let digest = [0x11u8; 32];
        let first = redacted_phone(&digest, false);
        let alternate = redacted_phone(&digest, true);
        assert!(first.starts_with("+1"));
        assert!(alternate.starts_with("+1"));
        assert_eq!(first.len(), alternate.len());
        assert_ne!(first, alternate);
    }
}
