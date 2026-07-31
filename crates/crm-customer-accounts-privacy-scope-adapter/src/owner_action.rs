use crm_capability_runtime::CapabilityDefinition;
use crm_customer_accounts::{Account, AccountStatus, UpdateAccount};
use crm_customer_accounts_capability_adapter::{
    RECORD_TYPE, account_from_snapshot, persisted_payload,
};
use crm_customer_privacy_owner_scope_support::{
    OwnerPrivacyActionPlanner, OwnerPrivacyActionPolicy, PrivacyOwnerActionCommand,
    owner_action_definition,
};
use crm_module_sdk::{RecordSnapshot, SdkError, TypedPayload};

pub const OWNER_ACTION_CAPABILITY_ID: &str = "customer_accounts.privacy.action.apply";

pub type CustomerAccountsPrivacyActionPlanner =
    OwnerPrivacyActionPlanner<CustomerAccountsPrivacyActionPolicy>;

#[derive(Debug, Default, Clone, Copy)]
pub struct CustomerAccountsPrivacyActionPolicy;

pub fn customer_accounts_privacy_action_definition() -> Result<CapabilityDefinition, SdkError> {
    owner_action_definition(crm_customer_accounts::MODULE_ID, OWNER_ACTION_CAPABILITY_ID)
}

pub const fn customer_accounts_privacy_action_planner() -> CustomerAccountsPrivacyActionPlanner {
    OwnerPrivacyActionPlanner::new(CustomerAccountsPrivacyActionPolicy)
}

impl OwnerPrivacyActionPolicy for CustomerAccountsPrivacyActionPolicy {
    fn owner_module_id(&self) -> &'static str {
        crm_customer_accounts::MODULE_ID
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
        minimize_account(command, current, false)
    }

    fn deletion_tombstone(
        &self,
        command: &PrivacyOwnerActionCommand,
        current: &RecordSnapshot,
    ) -> Result<TypedPayload, SdkError> {
        minimize_account(command, current, true)
    }
}

fn minimize_account(
    command: &PrivacyOwnerActionCommand,
    current: &RecordSnapshot,
    deleting: bool,
) -> Result<TypedPayload, SdkError> {
    let mut account = account_from_snapshot(current)?;
    let expected_version = account.version();
    let status = if deleting {
        AccountStatus::Inactive
    } else {
        account.status()
    };
    let state = if deleting { "deleted" } else { "minimized" };
    let name = minimized_account_name(&account, command.item_digest(), state, status);
    account.apply_update(UpdateAccount {
        expected_version,
        name,
        status,
        party_associations: account.party_associations().to_vec(),
        occurred_at_unix_nanos: command.planned_at_unix_nanos(),
    })?;
    persisted_payload(&account)
}

fn minimized_account_name(
    account: &Account,
    digest: &[u8; 32],
    state: &str,
    target_status: AccountStatus,
) -> String {
    let suffix = hex_prefix(digest);
    let candidate = format!("{state} account {suffix}");
    if candidate == account.name() && target_status == account.status() {
        format!("{state} account {suffix}-v")
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
    use crm_customer_accounts::{
        AccountId, AccountPartyAssociation, AccountPartyRole, CreateAccount, PartyReference,
    };

    fn account(name: &str) -> Account {
        Account::create(CreateAccount {
            account_id: AccountId::try_new("account-owner-action-1").unwrap(),
            name: name.to_owned(),
            party_associations: vec![AccountPartyAssociation::new(
                PartyReference::try_new("party-owner-action-1").unwrap(),
                AccountPartyRole::Primary,
            )],
            occurred_at_unix_nanos: 10,
        })
        .unwrap()
    }

    #[test]
    fn publishes_the_frozen_owner_action_coordinate() {
        let definition = customer_accounts_privacy_action_definition().unwrap();
        assert_eq!(
            definition.owner_module_id.as_str(),
            crm_customer_accounts::MODULE_ID
        );
        assert_eq!(
            definition.capability_id.as_str(),
            OWNER_ACTION_CAPABILITY_ID
        );
        assert!(definition.mutation);
        assert!(definition.requires_idempotency);
    }

    #[test]
    fn minimized_name_is_deterministic_and_non_identifying() {
        let value = account("Northwind");
        let name =
            minimized_account_name(&value, &[0xabu8; 32], "minimized", AccountStatus::Active);
        assert_eq!(name, "minimized account abababababababababababab");
        assert!(!name.contains("Northwind"));
    }

    #[test]
    fn delete_changes_lifecycle_even_when_name_is_already_minimized() {
        let current = "deleted account 111111111111111111111111";
        let value = account(current);
        let name =
            minimized_account_name(&value, &[0x11u8; 32], "deleted", AccountStatus::Inactive);
        assert_eq!(name, current);
        assert_ne!(value.status(), AccountStatus::Inactive);
    }
}
