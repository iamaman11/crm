use crm_module_sdk::{ErrorCategory, FieldName, FieldViolation, RecordId, SdkError};
use std::collections::BTreeSet;

const MAX_ACCOUNT_NAME_BYTES: usize = 240;
const MAX_PARTY_ASSOCIATIONS: usize = 500;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AccountId(RecordId);

impl AccountId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, SdkError> {
        let value = value.into();
        RecordId::try_new(value).map(Self).map_err(|error| {
            invalid(
                "CUSTOMER_ACCOUNTS_ACCOUNT_ID_INVALID",
                "account.account_id",
                error.to_string(),
            )
        })
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PartyReference(RecordId);

impl PartyReference {
    pub fn try_new(value: impl Into<String>) -> Result<Self, SdkError> {
        let value = value.into();
        RecordId::try_new(value).map(Self).map_err(|error| {
            invalid(
                "CUSTOMER_ACCOUNTS_PARTY_REFERENCE_INVALID",
                "account.party_associations.party_id",
                error.to_string(),
            )
        })
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AccountPartyRole {
    Primary,
    Member,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AccountPartyAssociation {
    party: PartyReference,
    role: AccountPartyRole,
}

impl AccountPartyAssociation {
    pub fn new(party: PartyReference, role: AccountPartyRole) -> Self {
        Self { party, role }
    }

    pub fn party(&self) -> &PartyReference {
        &self.party
    }

    pub const fn role(&self) -> AccountPartyRole {
        self.role
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountStatus {
    Active,
    Inactive,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    account_id: AccountId,
    name: String,
    status: AccountStatus,
    party_associations: Vec<AccountPartyAssociation>,
    created_at_unix_nanos: i64,
    updated_at_unix_nanos: i64,
    version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountSnapshot {
    pub account_id: AccountId,
    pub name: String,
    pub status: AccountStatus,
    pub party_associations: Vec<AccountPartyAssociation>,
    pub created_at_unix_nanos: i64,
    pub updated_at_unix_nanos: i64,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateAccount {
    pub account_id: AccountId,
    pub name: String,
    pub party_associations: Vec<AccountPartyAssociation>,
    pub occurred_at_unix_nanos: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateAccount {
    pub expected_version: i64,
    pub name: String,
    pub status: AccountStatus,
    pub party_associations: Vec<AccountPartyAssociation>,
    pub occurred_at_unix_nanos: i64,
}

impl Account {
    pub fn create(command: CreateAccount) -> Result<Self, SdkError> {
        let name = normalize_name(&command.name)?;
        let party_associations = normalize_associations(command.party_associations)?;
        validate_timestamp(
            "account.occurred_at_unix_nanos",
            command.occurred_at_unix_nanos,
        )?;

        Ok(Self {
            account_id: command.account_id,
            name,
            status: AccountStatus::Active,
            party_associations,
            created_at_unix_nanos: command.occurred_at_unix_nanos,
            updated_at_unix_nanos: command.occurred_at_unix_nanos,
            version: 1,
        })
    }

    pub fn rehydrate(snapshot: AccountSnapshot) -> Result<Self, SdkError> {
        let name = normalize_name(&snapshot.name)?;
        let party_associations = normalize_associations(snapshot.party_associations)?;
        validate_timestamp(
            "account.created_at_unix_nanos",
            snapshot.created_at_unix_nanos,
        )?;
        validate_timestamp(
            "account.updated_at_unix_nanos",
            snapshot.updated_at_unix_nanos,
        )?;
        if snapshot.updated_at_unix_nanos < snapshot.created_at_unix_nanos {
            return Err(invalid(
                "CUSTOMER_ACCOUNTS_PERSISTED_TIME_INVALID",
                "account.updated_at_unix_nanos",
                "updated time cannot precede created time",
            ));
        }
        if snapshot.version <= 0 {
            return Err(invalid(
                "CUSTOMER_ACCOUNTS_PERSISTED_VERSION_INVALID",
                "account.version",
                "persisted Account version must be positive",
            ));
        }

        Ok(Self {
            account_id: snapshot.account_id,
            name,
            status: snapshot.status,
            party_associations,
            created_at_unix_nanos: snapshot.created_at_unix_nanos,
            updated_at_unix_nanos: snapshot.updated_at_unix_nanos,
            version: snapshot.version,
        })
    }

    pub fn apply_update(&mut self, command: UpdateAccount) -> Result<(), SdkError> {
        self.require_version(command.expected_version)?;
        self.require_monotonic_time(command.occurred_at_unix_nanos)?;
        let name = normalize_name(&command.name)?;
        let party_associations = normalize_associations(command.party_associations)?;
        if name == self.name
            && command.status == self.status
            && party_associations == self.party_associations
        {
            return Err(invalid(
                "CUSTOMER_ACCOUNTS_UPDATE_EMPTY",
                "account",
                "Account update must change name, lifecycle status, or Party associations",
            ));
        }

        self.name = name;
        self.status = command.status;
        self.party_associations = party_associations;
        self.updated_at_unix_nanos = command.occurred_at_unix_nanos;
        self.version = self.version.checked_add(1).ok_or_else(|| {
            conflict(
                "CUSTOMER_ACCOUNTS_VERSION_EXHAUSTED",
                "Account version cannot be advanced further.",
            )
        })?;
        Ok(())
    }

    pub fn snapshot(&self) -> AccountSnapshot {
        AccountSnapshot {
            account_id: self.account_id.clone(),
            name: self.name.clone(),
            status: self.status,
            party_associations: self.party_associations.clone(),
            created_at_unix_nanos: self.created_at_unix_nanos,
            updated_at_unix_nanos: self.updated_at_unix_nanos,
            version: self.version,
        }
    }

    pub fn account_id(&self) -> &AccountId {
        &self.account_id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn status(&self) -> AccountStatus {
        self.status
    }

    pub fn party_associations(&self) -> &[AccountPartyAssociation] {
        &self.party_associations
    }

    pub const fn created_at_unix_nanos(&self) -> i64 {
        self.created_at_unix_nanos
    }

    pub const fn updated_at_unix_nanos(&self) -> i64 {
        self.updated_at_unix_nanos
    }

    pub const fn version(&self) -> i64 {
        self.version
    }

    fn require_version(&self, expected_version: i64) -> Result<(), SdkError> {
        if expected_version != self.version {
            return Err(conflict(
                "CUSTOMER_ACCOUNTS_VERSION_CONFLICT",
                format!(
                    "expected Account version {expected_version}, found {}",
                    self.version
                ),
            ));
        }
        Ok(())
    }

    fn require_monotonic_time(&self, occurred_at_unix_nanos: i64) -> Result<(), SdkError> {
        validate_timestamp("account.occurred_at_unix_nanos", occurred_at_unix_nanos)?;
        if occurred_at_unix_nanos < self.updated_at_unix_nanos {
            return Err(invalid(
                "CUSTOMER_ACCOUNTS_TIME_REGRESSION",
                "account.occurred_at_unix_nanos",
                "Account mutation time cannot precede the previous mutation",
            ));
        }
        Ok(())
    }
}

fn normalize_name(value: &str) -> Result<String, SdkError> {
    if value.chars().any(char::is_control) {
        return Err(invalid(
            "CUSTOMER_ACCOUNTS_NAME_INVALID",
            "account.name",
            "Account name must not contain control characters",
        ));
    }

    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() || normalized.len() > MAX_ACCOUNT_NAME_BYTES {
        return Err(invalid(
            "CUSTOMER_ACCOUNTS_NAME_INVALID",
            "account.name",
            format!(
                "Account name must be non-empty and not exceed {MAX_ACCOUNT_NAME_BYTES} UTF-8 bytes"
            ),
        ));
    }
    Ok(normalized)
}

fn normalize_associations(
    mut associations: Vec<AccountPartyAssociation>,
) -> Result<Vec<AccountPartyAssociation>, SdkError> {
    if associations.is_empty() || associations.len() > MAX_PARTY_ASSOCIATIONS {
        return Err(invalid(
            "CUSTOMER_ACCOUNTS_PARTY_ASSOCIATIONS_INVALID",
            "account.party_associations",
            format!(
                "Account must contain between 1 and {MAX_PARTY_ASSOCIATIONS} Party associations"
            ),
        ));
    }

    associations.sort();
    let mut party_ids = BTreeSet::new();
    let mut primary_count = 0usize;
    for association in &associations {
        if !party_ids.insert(association.party().as_str().to_owned()) {
            return Err(invalid(
                "CUSTOMER_ACCOUNTS_PARTY_ASSOCIATION_DUPLICATE",
                "account.party_associations",
                "A Party may appear at most once in an Account",
            ));
        }
        if association.role() == AccountPartyRole::Primary {
            primary_count += 1;
        }
    }
    if primary_count != 1 {
        return Err(invalid(
            "CUSTOMER_ACCOUNTS_PRIMARY_PARTY_INVALID",
            "account.party_associations",
            "Account must contain exactly one primary Party association",
        ));
    }
    Ok(associations)
}

fn validate_timestamp(field: &'static str, value: i64) -> Result<(), SdkError> {
    if value < 0 {
        return Err(invalid(
            "CUSTOMER_ACCOUNTS_TIMESTAMP_INVALID",
            field,
            "timestamp must not be negative",
        ));
    }
    Ok(())
}

fn invalid(code: &'static str, field: &'static str, safe_message: impl Into<String>) -> SdkError {
    let safe_message = safe_message.into();
    let mut error = SdkError::new(
        code,
        ErrorCategory::InvalidArgument,
        false,
        "The Account request contains invalid data.",
    );
    error.field_violations.push(FieldViolation {
        field: FieldName::try_new(field).expect("static field path must be valid"),
        code: code.to_owned(),
        safe_message,
    });
    error
}

fn conflict(code: &'static str, safe_message: impl Into<String>) -> SdkError {
    SdkError::new(code, ErrorCategory::Conflict, false, safe_message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn party(value: &str, role: AccountPartyRole) -> AccountPartyAssociation {
        AccountPartyAssociation::new(PartyReference::try_new(value).unwrap(), role)
    }

    fn account() -> Account {
        Account::create(CreateAccount {
            account_id: AccountId::try_new("account-01J00000000000000000000000").unwrap(),
            name: "  Acme   Corporation  ".to_owned(),
            party_associations: vec![
                party("party-organization-acme", AccountPartyRole::Primary),
                party("party-person-owner", AccountPartyRole::Member),
            ],
            occurred_at_unix_nanos: 10,
        })
        .unwrap()
    }

    #[test]
    fn creates_active_account_with_normalized_name_one_primary_party_and_version_one() {
        let value = account();
        assert_eq!(value.name(), "Acme Corporation");
        assert_eq!(value.status(), AccountStatus::Active);
        assert_eq!(value.party_associations().len(), 2);
        assert_eq!(
            value
                .party_associations()
                .iter()
                .filter(|association| association.role() == AccountPartyRole::Primary)
                .count(),
            1
        );
        assert_eq!(value.version(), 1);
    }

    #[test]
    fn update_preserves_identity_and_advances_exactly_one_version() {
        let mut value = account();
        let original_id = value.account_id().as_str().to_owned();
        value
            .apply_update(UpdateAccount {
                expected_version: 1,
                name: " Acme   Global ".to_owned(),
                status: AccountStatus::Inactive,
                party_associations: vec![
                    party("party-person-success", AccountPartyRole::Member),
                    party("party-organization-acme", AccountPartyRole::Primary),
                ],
                occurred_at_unix_nanos: 20,
            })
            .unwrap();

        assert_eq!(value.account_id().as_str(), original_id);
        assert_eq!(value.name(), "Acme Global");
        assert_eq!(value.status(), AccountStatus::Inactive);
        assert_eq!(value.version(), 2);
        assert_eq!(value.created_at_unix_nanos(), 10);
        assert_eq!(value.updated_at_unix_nanos(), 20);
    }

    #[test]
    fn rejects_missing_multiple_and_duplicate_primary_party_shapes() {
        for associations in [
            vec![],
            vec![party("party-a", AccountPartyRole::Member)],
            vec![
                party("party-a", AccountPartyRole::Primary),
                party("party-b", AccountPartyRole::Primary),
            ],
            vec![
                party("party-a", AccountPartyRole::Primary),
                party("party-a", AccountPartyRole::Member),
            ],
        ] {
            let error = Account::create(CreateAccount {
                account_id: AccountId::try_new("account-invalid-associations").unwrap(),
                name: "Invalid Account".to_owned(),
                party_associations: associations,
                occurred_at_unix_nanos: 1,
            })
            .unwrap_err();
            assert!(
                matches!(
                    error.code.as_str(),
                    "CUSTOMER_ACCOUNTS_PARTY_ASSOCIATIONS_INVALID"
                        | "CUSTOMER_ACCOUNTS_PRIMARY_PARTY_INVALID"
                        | "CUSTOMER_ACCOUNTS_PARTY_ASSOCIATION_DUPLICATE"
                ),
                "unexpected code: {}",
                error.code
            );
        }
    }

    #[test]
    fn rejects_stale_version_time_regression_and_semantic_no_op() {
        let mut value = account();

        let stale = value
            .apply_update(UpdateAccount {
                expected_version: 2,
                name: value.name().to_owned(),
                status: value.status(),
                party_associations: value.party_associations().to_vec(),
                occurred_at_unix_nanos: 20,
            })
            .unwrap_err();
        assert_eq!(stale.code, "CUSTOMER_ACCOUNTS_VERSION_CONFLICT");

        let time_regression = value
            .apply_update(UpdateAccount {
                expected_version: 1,
                name: "Acme Global".to_owned(),
                status: value.status(),
                party_associations: value.party_associations().to_vec(),
                occurred_at_unix_nanos: 9,
            })
            .unwrap_err();
        assert_eq!(time_regression.code, "CUSTOMER_ACCOUNTS_TIME_REGRESSION");

        let no_op = value
            .apply_update(UpdateAccount {
                expected_version: 1,
                name: "  Acme Corporation ".to_owned(),
                status: value.status(),
                party_associations: value.party_associations().to_vec(),
                occurred_at_unix_nanos: 10,
            })
            .unwrap_err();
        assert_eq!(no_op.code, "CUSTOMER_ACCOUNTS_UPDATE_EMPTY");
        assert_eq!(value.version(), 1);
    }

    #[test]
    fn snapshot_round_trip_preserves_exact_domain_state() {
        let value = account();
        let rehydrated = Account::rehydrate(value.snapshot()).unwrap();
        assert_eq!(rehydrated, value);
    }
}
