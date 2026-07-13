use crm_module_sdk::{ErrorCategory, FieldName, FieldViolation, RecordId, SdkError};

const MAX_DISPLAY_NAME_BYTES: usize = 240;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PartyId(RecordId);

impl PartyId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, SdkError> {
        let value = value.into();
        RecordId::try_new(value)
            .map(Self)
            .map_err(|error| invalid("PARTIES_PARTY_ID_INVALID", "party.party_id", error.to_string()))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartyKind {
    Person,
    Organization,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Party {
    party_id: PartyId,
    kind: PartyKind,
    display_name: String,
    created_at_unix_nanos: i64,
    updated_at_unix_nanos: i64,
    version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartySnapshot {
    pub party_id: PartyId,
    pub kind: PartyKind,
    pub display_name: String,
    pub created_at_unix_nanos: i64,
    pub updated_at_unix_nanos: i64,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateParty {
    pub party_id: PartyId,
    pub kind: PartyKind,
    pub display_name: String,
    pub occurred_at_unix_nanos: i64,
}

impl Party {
    pub fn create(command: CreateParty) -> Result<Self, SdkError> {
        let display_name = normalize_display_name(&command.display_name)?;
        validate_timestamp(
            "party.occurred_at_unix_nanos",
            command.occurred_at_unix_nanos,
        )?;

        Ok(Self {
            party_id: command.party_id,
            kind: command.kind,
            display_name,
            created_at_unix_nanos: command.occurred_at_unix_nanos,
            updated_at_unix_nanos: command.occurred_at_unix_nanos,
            version: 1,
        })
    }

    pub fn rehydrate(snapshot: PartySnapshot) -> Result<Self, SdkError> {
        let display_name = normalize_display_name(&snapshot.display_name)?;
        validate_timestamp("party.created_at_unix_nanos", snapshot.created_at_unix_nanos)?;
        validate_timestamp("party.updated_at_unix_nanos", snapshot.updated_at_unix_nanos)?;
        if snapshot.updated_at_unix_nanos < snapshot.created_at_unix_nanos {
            return Err(invalid(
                "PARTIES_PARTY_PERSISTED_TIME_INVALID",
                "party.updated_at_unix_nanos",
                "updated time cannot precede created time",
            ));
        }
        if snapshot.version <= 0 {
            return Err(invalid(
                "PARTIES_PARTY_PERSISTED_VERSION_INVALID",
                "party.version",
                "persisted Party version must be positive",
            ));
        }

        Ok(Self {
            party_id: snapshot.party_id,
            kind: snapshot.kind,
            display_name,
            created_at_unix_nanos: snapshot.created_at_unix_nanos,
            updated_at_unix_nanos: snapshot.updated_at_unix_nanos,
            version: snapshot.version,
        })
    }

    pub fn snapshot(&self) -> PartySnapshot {
        PartySnapshot {
            party_id: self.party_id.clone(),
            kind: self.kind,
            display_name: self.display_name.clone(),
            created_at_unix_nanos: self.created_at_unix_nanos,
            updated_at_unix_nanos: self.updated_at_unix_nanos,
            version: self.version,
        }
    }

    pub fn party_id(&self) -> &PartyId {
        &self.party_id
    }

    pub const fn kind(&self) -> PartyKind {
        self.kind
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
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
}

fn normalize_display_name(value: &str) -> Result<String, SdkError> {
    if value.chars().any(char::is_control) {
        return Err(invalid(
            "PARTIES_DISPLAY_NAME_INVALID",
            "party.display_name",
            "display name must not contain control characters",
        ));
    }

    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() || normalized.len() > MAX_DISPLAY_NAME_BYTES {
        return Err(invalid(
            "PARTIES_DISPLAY_NAME_INVALID",
            "party.display_name",
            format!(
                "display name must be non-empty and not exceed {MAX_DISPLAY_NAME_BYTES} UTF-8 bytes"
            ),
        ));
    }

    Ok(normalized)
}

fn validate_timestamp(field: &'static str, value: i64) -> Result<(), SdkError> {
    if value < 0 {
        return Err(invalid(
            "PARTIES_TIMESTAMP_INVALID",
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
        "The Party request contains invalid data.",
    );
    error.field_violations.push(FieldViolation {
        field: FieldName::try_new(field).expect("static field path must be valid"),
        code: code.to_owned(),
        safe_message,
    });
    error
}

#[cfg(test)]
mod tests {
    use super::*;

    fn party(kind: PartyKind, name: &str) -> Party {
        Party::create(CreateParty {
            party_id: PartyId::try_new("party-01J00000000000000000000000").unwrap(),
            kind,
            display_name: name.to_owned(),
            occurred_at_unix_nanos: 10,
        })
        .unwrap()
    }

    #[test]
    fn creates_person_and_organization_with_immutable_identity_and_version_one() {
        let person = party(PartyKind::Person, " Ada   Lovelace ");
        let organization = party(PartyKind::Organization, "Analytical Engine Society");

        assert_eq!(person.display_name(), "Ada Lovelace");
        assert_eq!(person.kind(), PartyKind::Person);
        assert_eq!(person.version(), 1);
        assert_eq!(organization.kind(), PartyKind::Organization);
        assert_eq!(organization.version(), 1);
    }

    #[test]
    fn rejects_empty_control_character_and_oversized_display_names() {
        for value in ["   ", "Ada\nLovelace", &"x".repeat(MAX_DISPLAY_NAME_BYTES + 1)] {
            let error = Party::create(CreateParty {
                party_id: PartyId::try_new("party-invalid-name").unwrap(),
                kind: PartyKind::Person,
                display_name: value.to_owned(),
                occurred_at_unix_nanos: 1,
            })
            .unwrap_err();
            assert_eq!(error.code, "PARTIES_DISPLAY_NAME_INVALID");
        }
    }

    #[test]
    fn rehydrate_rejects_invalid_version_and_time_ordering() {
        let snapshot = party(PartyKind::Organization, "Northwind").snapshot();
        let invalid_version = PartySnapshot {
            version: 0,
            ..snapshot.clone()
        };
        assert_eq!(
            Party::rehydrate(invalid_version).unwrap_err().code,
            "PARTIES_PARTY_PERSISTED_VERSION_INVALID"
        );

        let invalid_time = PartySnapshot {
            created_at_unix_nanos: 20,
            updated_at_unix_nanos: 10,
            ..snapshot
        };
        assert_eq!(
            Party::rehydrate(invalid_time).unwrap_err().code,
            "PARTIES_PARTY_PERSISTED_TIME_INVALID"
        );
    }

    #[test]
    fn snapshot_round_trip_preserves_exact_domain_state() {
        let value = party(PartyKind::Organization, "  Northwind   Holdings  ");
        let rehydrated = Party::rehydrate(value.snapshot()).unwrap();
        assert_eq!(rehydrated, value);
    }
}
