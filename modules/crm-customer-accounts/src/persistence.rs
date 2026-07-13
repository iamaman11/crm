use crate::domain::{
    Account, AccountId, AccountPartyAssociation, AccountPartyRole, AccountSnapshot, AccountStatus,
    PartyReference,
};
use crm_module_sdk::{ErrorCategory, SdkError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const ACCOUNT_STATE_SCHEMA_ID: &str = "crm.customer-accounts.account.state";
pub const ACCOUNT_STATE_SCHEMA_VERSION: &str = "1.0.0";
pub const ACCOUNT_STATE_MAXIMUM_BYTES: u64 = 256 * 1024;
pub const ACCOUNT_STATE_RETENTION_POLICY_ID: &str = "crm.customer-accounts.business_record";
const ACCOUNT_STATE_DESCRIPTOR: &[u8] = b"crm.customer-accounts.account.state/v1:account_id,name,status,party_associations[party_id,role],created_at_unix_nanos,updated_at_unix_nanos,version";

pub fn account_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(ACCOUNT_STATE_DESCRIPTOR).into()
}

pub fn encode_account_state(account: &Account) -> Result<Vec<u8>, SdkError> {
    let bytes = serde_json::to_vec(&AccountStateV1::from(account.snapshot())).map_err(|error| {
        persisted_error(format!("Account state serialization failed: {error}"))
    })?;
    validate_size(&bytes)?;
    Ok(bytes)
}

pub fn decode_account_state(bytes: &[u8]) -> Result<Account, SdkError> {
    validate_size(bytes)?;
    let state: AccountStateV1 = serde_json::from_slice(bytes)
        .map_err(|error| persisted_error(format!("Account state JSON is invalid: {error}")))?;
    Account::rehydrate(state.try_into()?)
        .map_err(|error| persisted_error(format!("{}: {}", error.code, error.safe_message)))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AccountStateV1 {
    account_id: String,
    name: String,
    status: AccountStatusState,
    party_associations: Vec<AccountPartyAssociationState>,
    created_at_unix_nanos: i64,
    updated_at_unix_nanos: i64,
    version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AccountPartyAssociationState {
    party_id: String,
    role: AccountPartyRoleState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum AccountStatusState {
    Active,
    Inactive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum AccountPartyRoleState {
    Primary,
    Member,
}

impl From<AccountSnapshot> for AccountStateV1 {
    fn from(value: AccountSnapshot) -> Self {
        Self {
            account_id: value.account_id.as_str().to_owned(),
            name: value.name,
            status: value.status.into(),
            party_associations: value
                .party_associations
                .into_iter()
                .map(AccountPartyAssociationState::from)
                .collect(),
            created_at_unix_nanos: value.created_at_unix_nanos,
            updated_at_unix_nanos: value.updated_at_unix_nanos,
            version: value.version,
        }
    }
}

impl TryFrom<AccountStateV1> for AccountSnapshot {
    type Error = SdkError;

    fn try_from(value: AccountStateV1) -> Result<Self, Self::Error> {
        Ok(Self {
            account_id: AccountId::try_new(value.account_id)
                .map_err(|error| persisted_error(error.to_string()))?,
            name: value.name,
            status: value.status.into(),
            party_associations: value
                .party_associations
                .into_iter()
                .map(AccountPartyAssociation::try_from)
                .collect::<Result<Vec<_>, _>>()?,
            created_at_unix_nanos: value.created_at_unix_nanos,
            updated_at_unix_nanos: value.updated_at_unix_nanos,
            version: value.version,
        })
    }
}

impl From<AccountPartyAssociation> for AccountPartyAssociationState {
    fn from(value: AccountPartyAssociation) -> Self {
        Self {
            party_id: value.party().as_str().to_owned(),
            role: value.role().into(),
        }
    }
}

impl TryFrom<AccountPartyAssociationState> for AccountPartyAssociation {
    type Error = SdkError;

    fn try_from(value: AccountPartyAssociationState) -> Result<Self, Self::Error> {
        Ok(AccountPartyAssociation::new(
            PartyReference::try_new(value.party_id)
                .map_err(|error| persisted_error(error.to_string()))?,
            value.role.into(),
        ))
    }
}

impl From<AccountStatus> for AccountStatusState {
    fn from(value: AccountStatus) -> Self {
        match value {
            AccountStatus::Active => Self::Active,
            AccountStatus::Inactive => Self::Inactive,
        }
    }
}

impl From<AccountStatusState> for AccountStatus {
    fn from(value: AccountStatusState) -> Self {
        match value {
            AccountStatusState::Active => Self::Active,
            AccountStatusState::Inactive => Self::Inactive,
        }
    }
}

impl From<AccountPartyRole> for AccountPartyRoleState {
    fn from(value: AccountPartyRole) -> Self {
        match value {
            AccountPartyRole::Primary => Self::Primary,
            AccountPartyRole::Member => Self::Member,
        }
    }
}

impl From<AccountPartyRoleState> for AccountPartyRole {
    fn from(value: AccountPartyRoleState) -> Self {
        match value {
            AccountPartyRoleState::Primary => Self::Primary,
            AccountPartyRoleState::Member => Self::Member,
        }
    }
}

fn validate_size(bytes: &[u8]) -> Result<(), SdkError> {
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > ACCOUNT_STATE_MAXIMUM_BYTES {
        return Err(persisted_error(format!(
            "Account state exceeds the maximum of {ACCOUNT_STATE_MAXIMUM_BYTES} bytes"
        )));
    }
    Ok(())
}

fn persisted_error(message: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ACCOUNTS_PERSISTED_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "The persisted Account state is invalid.",
    )
    .with_internal_reference(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::CreateAccount;

    fn account() -> Account {
        Account::create(CreateAccount {
            account_id: AccountId::try_new("account-persisted-1").unwrap(),
            name: "Northwind Customer Group".to_owned(),
            party_associations: vec![
                AccountPartyAssociation::new(
                    PartyReference::try_new("party-org-northwind").unwrap(),
                    AccountPartyRole::Primary,
                ),
                AccountPartyAssociation::new(
                    PartyReference::try_new("party-person-buyer").unwrap(),
                    AccountPartyRole::Member,
                ),
            ],
            occurred_at_unix_nanos: 10,
        })
        .unwrap()
    }

    #[test]
    fn round_trip_preserves_exact_state_and_schema_hash() {
        let value = account();
        let encoded = encode_account_state(&value).unwrap();
        let decoded = decode_account_state(&encoded).unwrap();

        assert_eq!(decoded, value);
        assert_ne!(account_state_descriptor_hash(), [0; 32]);
    }

    #[test]
    fn rejects_unknown_persisted_fields_and_invalid_domain_state() {
        let unknown = br#"{"account_id":"account-1","name":"Acme","status":"active","party_associations":[{"party_id":"party-1","role":"primary"}],"created_at_unix_nanos":1,"updated_at_unix_nanos":1,"version":1,"unexpected":true}"#;
        assert_eq!(
            decode_account_state(unknown).unwrap_err().code,
            "CUSTOMER_ACCOUNTS_PERSISTED_STATE_INVALID"
        );

        let no_primary = br#"{"account_id":"account-1","name":"Acme","status":"active","party_associations":[{"party_id":"party-1","role":"member"}],"created_at_unix_nanos":1,"updated_at_unix_nanos":1,"version":1}"#;
        assert_eq!(
            decode_account_state(no_primary).unwrap_err().code,
            "CUSTOMER_ACCOUNTS_PERSISTED_STATE_INVALID"
        );
    }
}
