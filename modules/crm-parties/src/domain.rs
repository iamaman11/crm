use crm_module_sdk::{ErrorCategory, FieldName, FieldViolation, RecordId, SdkError};

const MAX_DISPLAY_NAME_BYTES: usize = 240;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PartyId(RecordId);

impl PartyId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, SdkError> {
        let value = value.into();
        RecordId::try_new(value).map(Self).map_err(|error| {
            invalid(
                "PARTIES_PARTY_ID_INVALID",
                "party.party_id",
                error.to_string(),
            )
        })
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MergeLineageReference(RecordId);

impl MergeLineageReference {
    pub fn try_new(value: impl Into<String>) -> Result<Self, SdkError> {
        RecordId::try_new(value.into()).map(Self).map_err(|error| {
            invalid(
                "PARTIES_MERGE_LINEAGE_REFERENCE_INVALID",
                "party.lifecycle.merge_lineage_id",
                error.to_string(),
            )
        })
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
pub enum PartyLifecycle {
    Active,
    Merged {
        survivor_party_id: PartyId,
        merge_lineage_ref: MergeLineageReference,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Party {
    party_id: PartyId,
    kind: PartyKind,
    display_name: String,
    lifecycle: PartyLifecycle,
    created_at_unix_nanos: i64,
    updated_at_unix_nanos: i64,
    version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartySnapshot {
    pub party_id: PartyId,
    pub kind: PartyKind,
    pub display_name: String,
    pub lifecycle: PartyLifecycle,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateParty {
    pub expected_version: i64,
    pub display_name: String,
    pub occurred_at_unix_nanos: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyMergeDisplayName {
    pub expected_version: i64,
    pub display_name: String,
    pub occurred_at_unix_nanos: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkPartyMerged {
    pub expected_version: i64,
    pub survivor_party_id: PartyId,
    pub merge_lineage_ref: MergeLineageReference,
    pub occurred_at_unix_nanos: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReactivatePartyFromMerge {
    pub expected_version: i64,
    pub expected_survivor_party_id: PartyId,
    pub expected_merge_lineage_ref: MergeLineageReference,
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
            lifecycle: PartyLifecycle::Active,
            created_at_unix_nanos: command.occurred_at_unix_nanos,
            updated_at_unix_nanos: command.occurred_at_unix_nanos,
            version: 1,
        })
    }

    pub fn rehydrate(snapshot: PartySnapshot) -> Result<Self, SdkError> {
        let display_name = normalize_display_name(&snapshot.display_name)?;
        validate_timestamp(
            "party.created_at_unix_nanos",
            snapshot.created_at_unix_nanos,
        )?;
        validate_timestamp(
            "party.updated_at_unix_nanos",
            snapshot.updated_at_unix_nanos,
        )?;
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
        validate_persisted_lifecycle(
            &snapshot.party_id,
            &snapshot.lifecycle,
            snapshot.created_at_unix_nanos,
            snapshot.updated_at_unix_nanos,
            snapshot.version,
        )?;

        Ok(Self {
            party_id: snapshot.party_id,
            kind: snapshot.kind,
            display_name,
            lifecycle: snapshot.lifecycle,
            created_at_unix_nanos: snapshot.created_at_unix_nanos,
            updated_at_unix_nanos: snapshot.updated_at_unix_nanos,
            version: snapshot.version,
        })
    }

    pub fn apply_update(&mut self, command: UpdateParty) -> Result<(), SdkError> {
        self.require_version(command.expected_version)?;
        self.require_active()?;
        self.require_monotonic_time(command.occurred_at_unix_nanos)?;
        let display_name = normalize_display_name(&command.display_name)?;
        if display_name == self.display_name {
            return Err(invalid(
                "PARTIES_PARTY_UPDATE_EMPTY",
                "party.display_name",
                "updated display name must differ from the current value",
            ));
        }

        self.display_name = display_name;
        self.updated_at_unix_nanos = command.occurred_at_unix_nanos;
        self.version = self.next_version()?;
        Ok(())
    }

    pub fn apply_merge_display_name(
        &mut self,
        command: ApplyMergeDisplayName,
    ) -> Result<(), SdkError> {
        self.require_version(command.expected_version)?;
        self.require_active()?;
        self.require_strictly_increasing_time(command.occurred_at_unix_nanos)?;
        let display_name = normalize_display_name(&command.display_name)?;
        if display_name == self.display_name {
            return Err(invalid(
                "PARTIES_MERGE_DISPLAY_NAME_UNCHANGED",
                "party.display_name",
                "merge survivorship display name must differ from the current Party value",
            ));
        }
        self.display_name = display_name;
        self.updated_at_unix_nanos = command.occurred_at_unix_nanos;
        self.version = self.next_version()?;
        Ok(())
    }

    pub fn mark_merged(&mut self, command: MarkPartyMerged) -> Result<(), SdkError> {
        self.require_version(command.expected_version)?;
        self.require_active()?;
        self.require_strictly_increasing_time(command.occurred_at_unix_nanos)?;
        if command.survivor_party_id == self.party_id {
            return Err(invalid(
                "PARTIES_SELF_MERGE_INVALID",
                "party.lifecycle.survivor_party_id",
                "a Party cannot be merged into itself",
            ));
        }

        self.lifecycle = PartyLifecycle::Merged {
            survivor_party_id: command.survivor_party_id,
            merge_lineage_ref: command.merge_lineage_ref,
        };
        self.updated_at_unix_nanos = command.occurred_at_unix_nanos;
        self.version = self.next_version()?;
        Ok(())
    }

    pub fn reactivate_from_merge(
        &mut self,
        command: ReactivatePartyFromMerge,
    ) -> Result<(), SdkError> {
        self.require_version(command.expected_version)?;
        self.require_strictly_increasing_time(command.occurred_at_unix_nanos)?;
        match &self.lifecycle {
            PartyLifecycle::Active => {
                return Err(conflict(
                    "PARTIES_PARTY_NOT_MERGED",
                    "the Party is not currently merged",
                ));
            }
            PartyLifecycle::Merged {
                survivor_party_id,
                merge_lineage_ref,
            } if survivor_party_id == &command.expected_survivor_party_id
                && merge_lineage_ref == &command.expected_merge_lineage_ref => {}
            PartyLifecycle::Merged { .. } => {
                return Err(conflict(
                    "PARTIES_MERGE_LINEAGE_CONFLICT",
                    "the Party merge redirect does not match the requested lineage",
                ));
            }
        }

        self.lifecycle = PartyLifecycle::Active;
        self.updated_at_unix_nanos = command.occurred_at_unix_nanos;
        self.version = self.next_version()?;
        Ok(())
    }

    pub fn snapshot(&self) -> PartySnapshot {
        PartySnapshot {
            party_id: self.party_id.clone(),
            kind: self.kind,
            display_name: self.display_name.clone(),
            lifecycle: self.lifecycle.clone(),
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

    pub fn lifecycle(&self) -> &PartyLifecycle {
        &self.lifecycle
    }

    pub fn canonical_party_id(&self) -> &PartyId {
        match &self.lifecycle {
            PartyLifecycle::Active => &self.party_id,
            PartyLifecycle::Merged {
                survivor_party_id, ..
            } => survivor_party_id,
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self.lifecycle, PartyLifecycle::Active)
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
                "PARTIES_PARTY_VERSION_CONFLICT",
                format!(
                    "expected Party version {expected_version}, found {}",
                    self.version
                ),
            ));
        }
        Ok(())
    }

    fn require_active(&self) -> Result<(), SdkError> {
        if !self.is_active() {
            return Err(conflict(
                "PARTIES_PARTY_MERGED_READ_ONLY",
                "a merged Party cannot be changed through the normal Party mutation path",
            ));
        }
        Ok(())
    }

    fn require_monotonic_time(&self, occurred_at_unix_nanos: i64) -> Result<(), SdkError> {
        validate_timestamp("party.occurred_at_unix_nanos", occurred_at_unix_nanos)?;
        if occurred_at_unix_nanos < self.updated_at_unix_nanos {
            return Err(invalid(
                "PARTIES_PARTY_TIME_REGRESSION",
                "party.occurred_at_unix_nanos",
                "Party mutation time cannot precede the previous mutation",
            ));
        }
        Ok(())
    }

    fn require_strictly_increasing_time(
        &self,
        occurred_at_unix_nanos: i64,
    ) -> Result<(), SdkError> {
        validate_timestamp("party.occurred_at_unix_nanos", occurred_at_unix_nanos)?;
        if occurred_at_unix_nanos <= self.updated_at_unix_nanos {
            return Err(invalid(
                "PARTIES_PARTY_TIME_NOT_ADVANCED",
                "party.occurred_at_unix_nanos",
                "merge lifecycle mutation time must be strictly later than the previous Party mutation",
            ));
        }
        Ok(())
    }

    fn next_version(&self) -> Result<i64, SdkError> {
        self.version.checked_add(1).ok_or_else(|| {
            conflict(
                "PARTIES_PARTY_VERSION_EXHAUSTED",
                "Party version cannot be advanced further.",
            )
        })
    }
}

fn validate_persisted_lifecycle(
    party_id: &PartyId,
    lifecycle: &PartyLifecycle,
    created_at_unix_nanos: i64,
    updated_at_unix_nanos: i64,
    version: i64,
) -> Result<(), SdkError> {
    if let PartyLifecycle::Merged {
        survivor_party_id, ..
    } = lifecycle
    {
        if survivor_party_id == party_id {
            return Err(invalid(
                "PARTIES_PARTY_PERSISTED_LIFECYCLE_INVALID",
                "party.lifecycle.survivor_party_id",
                "a persisted merged Party cannot redirect to itself",
            ));
        }
        if version < 2 || updated_at_unix_nanos <= created_at_unix_nanos {
            return Err(invalid(
                "PARTIES_PARTY_PERSISTED_LIFECYCLE_INVALID",
                "party.lifecycle",
                "a persisted merged Party must reflect a later versioned lifecycle mutation",
            ));
        }
    }
    Ok(())
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

fn conflict(code: &'static str, safe_message: impl Into<String>) -> SdkError {
    SdkError::new(code, ErrorCategory::Conflict, false, safe_message)
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

    fn merge_ref() -> MergeLineageReference {
        MergeLineageReference::try_new(
            "idrm-0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        )
        .unwrap()
    }

    fn survivor_id() -> PartyId {
        PartyId::try_new("party-survivor").unwrap()
    }

    #[test]
    fn creates_person_and_organization_with_immutable_identity_and_version_one() {
        let person = party(PartyKind::Person, " Ada   Lovelace ");
        let organization = party(PartyKind::Organization, "Analytical Engine Society");

        assert_eq!(person.display_name(), "Ada Lovelace");
        assert_eq!(person.kind(), PartyKind::Person);
        assert_eq!(person.lifecycle(), &PartyLifecycle::Active);
        assert_eq!(person.version(), 1);
        assert_eq!(organization.kind(), PartyKind::Organization);
        assert_eq!(organization.version(), 1);
    }

    #[test]
    fn applies_normalized_update_without_changing_identity_or_kind() {
        let mut value = party(PartyKind::Person, "Ada Lovelace");
        let original_id = value.party_id().as_str().to_owned();
        value
            .apply_update(UpdateParty {
                expected_version: 1,
                display_name: "  Augusta   Ada   Lovelace  ".to_owned(),
                occurred_at_unix_nanos: 20,
            })
            .unwrap();

        assert_eq!(value.party_id().as_str(), original_id);
        assert_eq!(value.kind(), PartyKind::Person);
        assert_eq!(value.display_name(), "Augusta Ada Lovelace");
        assert_eq!(value.created_at_unix_nanos(), 10);
        assert_eq!(value.updated_at_unix_nanos(), 20);
        assert_eq!(value.version(), 2);
    }

    #[test]
    fn merge_lifecycle_preserves_identity_and_redirects_canonical_resolution() {
        let mut value = party(PartyKind::Person, "Ada Lovelace");
        value
            .mark_merged(MarkPartyMerged {
                expected_version: 1,
                survivor_party_id: survivor_id(),
                merge_lineage_ref: merge_ref(),
                occurred_at_unix_nanos: 20,
            })
            .unwrap();

        assert_eq!(
            value.party_id().as_str(),
            "party-01J00000000000000000000000"
        );
        assert_eq!(value.canonical_party_id(), &survivor_id());
        assert!(!value.is_active());
        assert_eq!(value.version(), 2);
        assert_eq!(value.updated_at_unix_nanos(), 20);
    }

    #[test]
    fn merged_party_rejects_normal_updates_and_conflicting_reactivation() {
        let mut value = party(PartyKind::Organization, "Northwind");
        value
            .mark_merged(MarkPartyMerged {
                expected_version: 1,
                survivor_party_id: survivor_id(),
                merge_lineage_ref: merge_ref(),
                occurred_at_unix_nanos: 20,
            })
            .unwrap();

        assert_eq!(
            value
                .apply_update(UpdateParty {
                    expected_version: 2,
                    display_name: "Northwind Holdings".to_owned(),
                    occurred_at_unix_nanos: 30,
                })
                .unwrap_err()
                .code,
            "PARTIES_PARTY_MERGED_READ_ONLY"
        );
        assert_eq!(
            value
                .reactivate_from_merge(ReactivatePartyFromMerge {
                    expected_version: 2,
                    expected_survivor_party_id: PartyId::try_new("party-other").unwrap(),
                    expected_merge_lineage_ref: merge_ref(),
                    occurred_at_unix_nanos: 30,
                })
                .unwrap_err()
                .code,
            "PARTIES_MERGE_LINEAGE_CONFLICT"
        );
        assert_eq!(value.version(), 2);
    }

    #[test]
    fn exact_merge_reactivation_restores_active_state_without_changing_identity() {
        let mut value = party(PartyKind::Person, "Ada Lovelace");
        value
            .mark_merged(MarkPartyMerged {
                expected_version: 1,
                survivor_party_id: survivor_id(),
                merge_lineage_ref: merge_ref(),
                occurred_at_unix_nanos: 20,
            })
            .unwrap();
        value
            .reactivate_from_merge(ReactivatePartyFromMerge {
                expected_version: 2,
                expected_survivor_party_id: survivor_id(),
                expected_merge_lineage_ref: merge_ref(),
                occurred_at_unix_nanos: 30,
            })
            .unwrap();

        assert_eq!(value.lifecycle(), &PartyLifecycle::Active);
        assert_eq!(value.canonical_party_id(), value.party_id());
        assert_eq!(value.version(), 3);
    }

    #[test]
    fn merge_survivorship_update_requires_active_party_and_strict_time() {
        let mut value = party(PartyKind::Person, "Ada Lovelace");
        assert_eq!(
            value
                .apply_merge_display_name(ApplyMergeDisplayName {
                    expected_version: 1,
                    display_name: "Augusta Ada Lovelace".to_owned(),
                    occurred_at_unix_nanos: 10,
                })
                .unwrap_err()
                .code,
            "PARTIES_PARTY_TIME_NOT_ADVANCED"
        );
        value
            .apply_merge_display_name(ApplyMergeDisplayName {
                expected_version: 1,
                display_name: "Augusta Ada Lovelace".to_owned(),
                occurred_at_unix_nanos: 20,
            })
            .unwrap();
        assert_eq!(value.display_name(), "Augusta Ada Lovelace");
        assert_eq!(value.version(), 2);
    }

    #[test]
    fn rejects_stale_version_time_regression_and_semantic_no_op() {
        let mut value = party(PartyKind::Organization, "Northwind");

        let stale = value
            .apply_update(UpdateParty {
                expected_version: 2,
                display_name: "Northwind Holdings".to_owned(),
                occurred_at_unix_nanos: 20,
            })
            .unwrap_err();
        assert_eq!(stale.code, "PARTIES_PARTY_VERSION_CONFLICT");

        let time_regression = value
            .apply_update(UpdateParty {
                expected_version: 1,
                display_name: "Northwind Holdings".to_owned(),
                occurred_at_unix_nanos: 9,
            })
            .unwrap_err();
        assert_eq!(time_regression.code, "PARTIES_PARTY_TIME_REGRESSION");

        let no_op = value
            .apply_update(UpdateParty {
                expected_version: 1,
                display_name: "  Northwind  ".to_owned(),
                occurred_at_unix_nanos: 10,
            })
            .unwrap_err();
        assert_eq!(no_op.code, "PARTIES_PARTY_UPDATE_EMPTY");
        assert_eq!(value.version(), 1);
    }

    #[test]
    fn rejects_empty_control_character_and_oversized_display_names() {
        for value in [
            "   ",
            "Ada\nLovelace",
            &"x".repeat(MAX_DISPLAY_NAME_BYTES + 1),
        ] {
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
    fn rehydrate_rejects_invalid_version_time_ordering_and_self_redirect() {
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
            ..snapshot.clone()
        };
        assert_eq!(
            Party::rehydrate(invalid_time).unwrap_err().code,
            "PARTIES_PARTY_PERSISTED_TIME_INVALID"
        );

        let self_redirect = PartySnapshot {
            lifecycle: PartyLifecycle::Merged {
                survivor_party_id: snapshot.party_id.clone(),
                merge_lineage_ref: merge_ref(),
            },
            updated_at_unix_nanos: 20,
            version: 2,
            ..snapshot
        };
        assert_eq!(
            Party::rehydrate(self_redirect).unwrap_err().code,
            "PARTIES_PARTY_PERSISTED_LIFECYCLE_INVALID"
        );
    }

    #[test]
    fn snapshot_round_trip_preserves_exact_domain_state() {
        let mut value = party(PartyKind::Organization, "  Northwind   Holdings  ");
        value
            .mark_merged(MarkPartyMerged {
                expected_version: 1,
                survivor_party_id: survivor_id(),
                merge_lineage_ref: merge_ref(),
                occurred_at_unix_nanos: 20,
            })
            .unwrap();
        let rehydrated = Party::rehydrate(value.snapshot()).unwrap();
        assert_eq!(rehydrated, value);
    }
}
