use crm_module_sdk::{ErrorCategory, FieldName, FieldViolation, RecordId, SdkError};

const MAX_RELATIONSHIP_TYPE_CODE_BYTES: usize = 96;
const MAX_RELATIONSHIP_ROLE_CODE_BYTES: usize = 96;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PartyRelationshipId(RecordId);

impl PartyRelationshipId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, SdkError> {
        RecordId::try_new(value.into()).map(Self).map_err(|error| {
            invalid(
                "PARTY_RELATIONSHIPS_RELATIONSHIP_ID_INVALID",
                "party_relationship.party_relationship_id",
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
        RecordId::try_new(value.into()).map(Self).map_err(|error| {
            invalid(
                "PARTY_RELATIONSHIPS_PARTY_REFERENCE_INVALID",
                "party_relationship.party_ref.party_id",
                error.to_string(),
            )
        })
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RelationshipDirectionality {
    Directional,
    Reciprocal,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RelationshipType {
    code: String,
    directionality: RelationshipDirectionality,
    from_role: String,
    to_role: String,
}

impl RelationshipType {
    pub fn try_new(
        code: impl Into<String>,
        directionality: RelationshipDirectionality,
        from_role: impl Into<String>,
        to_role: impl Into<String>,
    ) -> Result<Self, SdkError> {
        let code = normalize_semantic_identifier(
            &code.into(),
            MAX_RELATIONSHIP_TYPE_CODE_BYTES,
            "PARTY_RELATIONSHIPS_TYPE_CODE_INVALID",
            "party_relationship.relationship_type.code",
            "relationship type code",
        )?;
        let from_role = normalize_semantic_identifier(
            &from_role.into(),
            MAX_RELATIONSHIP_ROLE_CODE_BYTES,
            "PARTY_RELATIONSHIPS_FROM_ROLE_INVALID",
            "party_relationship.relationship_type.from_role",
            "from-role code",
        )?;
        let to_role = normalize_semantic_identifier(
            &to_role.into(),
            MAX_RELATIONSHIP_ROLE_CODE_BYTES,
            "PARTY_RELATIONSHIPS_TO_ROLE_INVALID",
            "party_relationship.relationship_type.to_role",
            "to-role code",
        )?;

        match directionality {
            RelationshipDirectionality::Directional if from_role == to_role => {
                return Err(invalid(
                    "PARTY_RELATIONSHIPS_DIRECTIONAL_ROLES_INVALID",
                    "party_relationship.relationship_type",
                    "a directional relationship must use distinct from-role and to-role codes",
                ));
            }
            RelationshipDirectionality::Reciprocal if from_role != to_role => {
                return Err(invalid(
                    "PARTY_RELATIONSHIPS_RECIPROCAL_ROLES_INVALID",
                    "party_relationship.relationship_type",
                    "a reciprocal relationship must use the same role code on both endpoints",
                ));
            }
            _ => {}
        }

        validate_reserved_builtin_semantics(&code, directionality, &from_role, &to_role)?;

        Ok(Self {
            code,
            directionality,
            from_role,
            to_role,
        })
    }

    pub fn employment() -> Self {
        Self::builtin(
            "employment",
            RelationshipDirectionality::Directional,
            "employer",
            "employee",
        )
    }

    pub fn household() -> Self {
        Self::builtin(
            "household",
            RelationshipDirectionality::Reciprocal,
            "household_member",
            "household_member",
        )
    }

    pub fn parent_subsidiary() -> Self {
        Self::builtin(
            "parent_subsidiary",
            RelationshipDirectionality::Directional,
            "parent",
            "subsidiary",
        )
    }

    pub fn partner() -> Self {
        Self::builtin(
            "partner",
            RelationshipDirectionality::Reciprocal,
            "partner",
            "partner",
        )
    }

    pub fn advisor() -> Self {
        Self::builtin(
            "advisor",
            RelationshipDirectionality::Directional,
            "advisor",
            "advisee",
        )
    }

    pub fn guarantor() -> Self {
        Self::builtin(
            "guarantor",
            RelationshipDirectionality::Directional,
            "guarantor",
            "guaranteed_party",
        )
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub const fn directionality(&self) -> RelationshipDirectionality {
        self.directionality
    }

    pub fn from_role(&self) -> &str {
        &self.from_role
    }

    pub fn to_role(&self) -> &str {
        &self.to_role
    }

    fn builtin(
        code: &'static str,
        directionality: RelationshipDirectionality,
        from_role: &'static str,
        to_role: &'static str,
    ) -> Self {
        Self::try_new(code, directionality, from_role, to_role)
            .expect("static Party Relationship semantics must be valid")
    }
}

fn validate_reserved_builtin_semantics(
    code: &str,
    directionality: RelationshipDirectionality,
    from_role: &str,
    to_role: &str,
) -> Result<(), SdkError> {
    let expected = match code {
        "employment" => Some((
            RelationshipDirectionality::Directional,
            "employer",
            "employee",
        )),
        "household" => Some((
            RelationshipDirectionality::Reciprocal,
            "household_member",
            "household_member",
        )),
        "parent_subsidiary" => Some((
            RelationshipDirectionality::Directional,
            "parent",
            "subsidiary",
        )),
        "partner" => Some((
            RelationshipDirectionality::Reciprocal,
            "partner",
            "partner",
        )),
        "advisor" => Some((
            RelationshipDirectionality::Directional,
            "advisor",
            "advisee",
        )),
        "guarantor" => Some((
            RelationshipDirectionality::Directional,
            "guarantor",
            "guaranteed_party",
        )),
        _ => None,
    };
    if let Some((expected_directionality, expected_from_role, expected_to_role)) = expected
        && (directionality != expected_directionality
            || from_role != expected_from_role
            || to_role != expected_to_role)
    {
        return Err(invalid(
            "PARTY_RELATIONSHIPS_RESERVED_TYPE_SEMANTICS_INVALID",
            "party_relationship.relationship_type",
            format!(
                "reserved relationship type '{code}' must use its canonical directionality and endpoint roles"
            ),
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PartyRelationshipStatus {
    Active,
    Inactive,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyRelationship {
    party_relationship_id: PartyRelationshipId,
    from_party_ref: PartyReference,
    to_party_ref: PartyReference,
    relationship_type: RelationshipType,
    status: PartyRelationshipStatus,
    valid_from_unix_nanos: Option<i64>,
    valid_until_unix_nanos: Option<i64>,
    created_at_unix_nanos: i64,
    updated_at_unix_nanos: i64,
    version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyRelationshipSnapshot {
    pub party_relationship_id: PartyRelationshipId,
    pub from_party_ref: PartyReference,
    pub to_party_ref: PartyReference,
    pub relationship_type: RelationshipType,
    pub status: PartyRelationshipStatus,
    pub valid_from_unix_nanos: Option<i64>,
    pub valid_until_unix_nanos: Option<i64>,
    pub created_at_unix_nanos: i64,
    pub updated_at_unix_nanos: i64,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatePartyRelationship {
    pub party_relationship_id: PartyRelationshipId,
    pub from_party_ref: PartyReference,
    pub to_party_ref: PartyReference,
    pub relationship_type: RelationshipType,
    pub valid_from_unix_nanos: Option<i64>,
    pub valid_until_unix_nanos: Option<i64>,
    pub occurred_at_unix_nanos: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdatePartyRelationship {
    pub expected_version: i64,
    pub status: PartyRelationshipStatus,
    pub valid_from_unix_nanos: Option<i64>,
    pub valid_until_unix_nanos: Option<i64>,
    pub occurred_at_unix_nanos: i64,
}

impl PartyRelationship {
    pub fn create(command: CreatePartyRelationship) -> Result<Self, SdkError> {
        validate_timestamp(
            "party_relationship.occurred_at_unix_nanos",
            command.occurred_at_unix_nanos,
        )?;
        validate_validity(
            command.valid_from_unix_nanos,
            command.valid_until_unix_nanos,
        )?;
        let (from_party_ref, to_party_ref) = canonicalize_endpoints(
            command.relationship_type.directionality(),
            command.from_party_ref,
            command.to_party_ref,
        )?;

        Ok(Self {
            party_relationship_id: command.party_relationship_id,
            from_party_ref,
            to_party_ref,
            relationship_type: command.relationship_type,
            status: PartyRelationshipStatus::Active,
            valid_from_unix_nanos: command.valid_from_unix_nanos,
            valid_until_unix_nanos: command.valid_until_unix_nanos,
            created_at_unix_nanos: command.occurred_at_unix_nanos,
            updated_at_unix_nanos: command.occurred_at_unix_nanos,
            version: 1,
        })
    }

    pub fn rehydrate(snapshot: PartyRelationshipSnapshot) -> Result<Self, SdkError> {
        validate_timestamp(
            "party_relationship.created_at_unix_nanos",
            snapshot.created_at_unix_nanos,
        )?;
        validate_timestamp(
            "party_relationship.updated_at_unix_nanos",
            snapshot.updated_at_unix_nanos,
        )?;
        if snapshot.updated_at_unix_nanos < snapshot.created_at_unix_nanos {
            return Err(invalid(
                "PARTY_RELATIONSHIPS_PERSISTED_TIME_INVALID",
                "party_relationship.updated_at_unix_nanos",
                "updated time cannot precede created time",
            ));
        }
        if snapshot.version <= 0 {
            return Err(invalid(
                "PARTY_RELATIONSHIPS_PERSISTED_VERSION_INVALID",
                "party_relationship.version",
                "persisted Party Relationship version must be positive",
            ));
        }
        validate_validity(
            snapshot.valid_from_unix_nanos,
            snapshot.valid_until_unix_nanos,
        )?;
        validate_initial_transition_shape(
            snapshot.status,
            snapshot.created_at_unix_nanos,
            snapshot.updated_at_unix_nanos,
            snapshot.version,
        )?;
        validate_distinct_endpoints(&snapshot.from_party_ref, &snapshot.to_party_ref)?;
        validate_canonical_endpoint_order(
            snapshot.relationship_type.directionality(),
            &snapshot.from_party_ref,
            &snapshot.to_party_ref,
        )?;

        Ok(Self {
            party_relationship_id: snapshot.party_relationship_id,
            from_party_ref: snapshot.from_party_ref,
            to_party_ref: snapshot.to_party_ref,
            relationship_type: snapshot.relationship_type,
            status: snapshot.status,
            valid_from_unix_nanos: snapshot.valid_from_unix_nanos,
            valid_until_unix_nanos: snapshot.valid_until_unix_nanos,
            created_at_unix_nanos: snapshot.created_at_unix_nanos,
            updated_at_unix_nanos: snapshot.updated_at_unix_nanos,
            version: snapshot.version,
        })
    }

    pub fn apply_update(&mut self, command: UpdatePartyRelationship) -> Result<(), SdkError> {
        self.require_version(command.expected_version)?;
        self.require_monotonic_time(command.occurred_at_unix_nanos)?;
        validate_validity(
            command.valid_from_unix_nanos,
            command.valid_until_unix_nanos,
        )?;

        if command.status == self.status
            && command.valid_from_unix_nanos == self.valid_from_unix_nanos
            && command.valid_until_unix_nanos == self.valid_until_unix_nanos
        {
            return Err(invalid(
                "PARTY_RELATIONSHIPS_UPDATE_EMPTY",
                "party_relationship",
                "updated Party Relationship state must differ from the current value",
            ));
        }

        let next_version = self.next_version()?;
        self.status = command.status;
        self.valid_from_unix_nanos = command.valid_from_unix_nanos;
        self.valid_until_unix_nanos = command.valid_until_unix_nanos;
        self.updated_at_unix_nanos = command.occurred_at_unix_nanos;
        self.version = next_version;
        Ok(())
    }

    pub fn snapshot(&self) -> PartyRelationshipSnapshot {
        PartyRelationshipSnapshot {
            party_relationship_id: self.party_relationship_id.clone(),
            from_party_ref: self.from_party_ref.clone(),
            to_party_ref: self.to_party_ref.clone(),
            relationship_type: self.relationship_type.clone(),
            status: self.status,
            valid_from_unix_nanos: self.valid_from_unix_nanos,
            valid_until_unix_nanos: self.valid_until_unix_nanos,
            created_at_unix_nanos: self.created_at_unix_nanos,
            updated_at_unix_nanos: self.updated_at_unix_nanos,
            version: self.version,
        }
    }

    pub fn party_relationship_id(&self) -> &PartyRelationshipId {
        &self.party_relationship_id
    }

    pub fn from_party_ref(&self) -> &PartyReference {
        &self.from_party_ref
    }

    pub fn to_party_ref(&self) -> &PartyReference {
        &self.to_party_ref
    }

    pub fn relationship_type(&self) -> &RelationshipType {
        &self.relationship_type
    }

    pub const fn status(&self) -> PartyRelationshipStatus {
        self.status
    }

    pub const fn valid_from_unix_nanos(&self) -> Option<i64> {
        self.valid_from_unix_nanos
    }

    pub const fn valid_until_unix_nanos(&self) -> Option<i64> {
        self.valid_until_unix_nanos
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

    pub fn is_effective_at(&self, unix_nanos: i64) -> bool {
        self.status == PartyRelationshipStatus::Active
            && self
                .valid_from_unix_nanos
                .is_none_or(|value| unix_nanos >= value)
            && self
                .valid_until_unix_nanos
                .is_none_or(|value| unix_nanos < value)
    }

    fn require_version(&self, expected_version: i64) -> Result<(), SdkError> {
        if expected_version != self.version {
            return Err(conflict(
                "PARTY_RELATIONSHIPS_VERSION_CONFLICT",
                format!(
                    "expected Party Relationship version {expected_version}, found {}",
                    self.version
                ),
            ));
        }
        Ok(())
    }

    fn require_monotonic_time(&self, occurred_at_unix_nanos: i64) -> Result<(), SdkError> {
        validate_timestamp(
            "party_relationship.occurred_at_unix_nanos",
            occurred_at_unix_nanos,
        )?;
        if occurred_at_unix_nanos < self.updated_at_unix_nanos {
            return Err(invalid(
                "PARTY_RELATIONSHIPS_TIME_REGRESSION",
                "party_relationship.occurred_at_unix_nanos",
                "Party Relationship mutation time cannot precede the previous mutation",
            ));
        }
        Ok(())
    }

    fn next_version(&self) -> Result<i64, SdkError> {
        self.version.checked_add(1).ok_or_else(|| {
            conflict(
                "PARTY_RELATIONSHIPS_VERSION_EXHAUSTED",
                "Party Relationship version cannot be advanced further.",
            )
        })
    }
}

fn canonicalize_endpoints(
    directionality: RelationshipDirectionality,
    from_party_ref: PartyReference,
    to_party_ref: PartyReference,
) -> Result<(PartyReference, PartyReference), SdkError> {
    validate_distinct_endpoints(&from_party_ref, &to_party_ref)?;
    if directionality == RelationshipDirectionality::Reciprocal && to_party_ref < from_party_ref {
        Ok((to_party_ref, from_party_ref))
    } else {
        Ok((from_party_ref, to_party_ref))
    }
}

fn validate_distinct_endpoints(
    from_party_ref: &PartyReference,
    to_party_ref: &PartyReference,
) -> Result<(), SdkError> {
    if from_party_ref == to_party_ref {
        return Err(invalid(
            "PARTY_RELATIONSHIPS_SELF_RELATIONSHIP_INVALID",
            "party_relationship.party_refs",
            "a Party Relationship must connect two distinct Parties",
        ));
    }
    Ok(())
}

fn validate_canonical_endpoint_order(
    directionality: RelationshipDirectionality,
    from_party_ref: &PartyReference,
    to_party_ref: &PartyReference,
) -> Result<(), SdkError> {
    if directionality == RelationshipDirectionality::Reciprocal && to_party_ref < from_party_ref {
        return Err(invalid(
            "PARTY_RELATIONSHIPS_PERSISTED_ENDPOINT_ORDER_INVALID",
            "party_relationship.party_refs",
            "persisted reciprocal Party Relationship endpoints are not in canonical order",
        ));
    }
    Ok(())
}

fn validate_initial_transition_shape(
    status: PartyRelationshipStatus,
    created_at_unix_nanos: i64,
    updated_at_unix_nanos: i64,
    version: i64,
) -> Result<(), SdkError> {
    if version == 1
        && (status != PartyRelationshipStatus::Active
            || updated_at_unix_nanos != created_at_unix_nanos)
    {
        return Err(invalid(
            "PARTY_RELATIONSHIPS_PERSISTED_INITIAL_STATE_INVALID",
            "party_relationship.version",
            "version-one Party Relationship state must match the create transition",
        ));
    }
    Ok(())
}

fn validate_validity(
    valid_from_unix_nanos: Option<i64>,
    valid_until_unix_nanos: Option<i64>,
) -> Result<(), SdkError> {
    if let Some(value) = valid_from_unix_nanos {
        validate_timestamp("party_relationship.valid_from_unix_nanos", value)?;
    }
    if let Some(value) = valid_until_unix_nanos {
        validate_timestamp("party_relationship.valid_until_unix_nanos", value)?;
    }
    if matches!(
        (valid_from_unix_nanos, valid_until_unix_nanos),
        (Some(from), Some(until)) if until <= from
    ) {
        return Err(invalid(
            "PARTY_RELATIONSHIPS_VALIDITY_INVALID",
            "party_relationship.validity",
            "valid-until must be later than valid-from",
        ));
    }
    Ok(())
}

fn normalize_semantic_identifier(
    value: &str,
    maximum_bytes: usize,
    code: &'static str,
    field: &'static str,
    label: &str,
) -> Result<String, SdkError> {
    if value.chars().any(char::is_control) {
        return Err(invalid(
            code,
            field,
            format!("{label} must not contain control characters"),
        ));
    }
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() || normalized.len() > maximum_bytes {
        return Err(invalid(
            code,
            field,
            format!("{label} must be non-empty and not exceed {maximum_bytes} UTF-8 bytes"),
        ));
    }
    let bytes = normalized.as_bytes();
    if !bytes.first().is_some_and(u8::is_ascii_alphanumeric)
        || !bytes.last().is_some_and(u8::is_ascii_alphanumeric)
        || !bytes.iter().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(*byte, b'.' | b'-' | b'_')
        })
    {
        return Err(invalid(
            code,
            field,
            format!(
                "{label} must use lowercase ASCII letters, digits, '.', '-' or '_' and start/end with an alphanumeric character"
            ),
        ));
    }
    Ok(normalized)
}

fn validate_timestamp(field: &'static str, value: i64) -> Result<(), SdkError> {
    if value < 0 {
        return Err(invalid(
            "PARTY_RELATIONSHIPS_TIMESTAMP_INVALID",
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
        "The Party Relationship request contains invalid data.",
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

    fn party(value: &str) -> PartyReference {
        PartyReference::try_new(value).unwrap()
    }

    fn relationship() -> PartyRelationship {
        PartyRelationship::create(CreatePartyRelationship {
            party_relationship_id: PartyRelationshipId::try_new("party-relationship-1").unwrap(),
            from_party_ref: party("party-organization-acme"),
            to_party_ref: party("party-person-ada"),
            relationship_type: RelationshipType::employment(),
            valid_from_unix_nanos: Some(10),
            valid_until_unix_nanos: Some(1_000),
            occurred_at_unix_nanos: 20,
        })
        .unwrap()
    }

    #[test]
    fn creates_directional_relationship_with_immutable_semantics_and_version_one() {
        let value = relationship();
        assert_eq!(value.from_party_ref().as_str(), "party-organization-acme");
        assert_eq!(value.to_party_ref().as_str(), "party-person-ada");
        assert_eq!(value.relationship_type().code(), "employment");
        assert_eq!(
            value.relationship_type().directionality(),
            RelationshipDirectionality::Directional
        );
        assert_eq!(value.relationship_type().from_role(), "employer");
        assert_eq!(value.relationship_type().to_role(), "employee");
        assert_eq!(value.status(), PartyRelationshipStatus::Active);
        assert_eq!(value.version(), 1);
    }

    #[test]
    fn reciprocal_relationships_canonicalize_endpoint_order() {
        let value = PartyRelationship::create(CreatePartyRelationship {
            party_relationship_id: PartyRelationshipId::try_new("party-relationship-household")
                .unwrap(),
            from_party_ref: party("party-z"),
            to_party_ref: party("party-a"),
            relationship_type: RelationshipType::household(),
            valid_from_unix_nanos: None,
            valid_until_unix_nanos: None,
            occurred_at_unix_nanos: 1,
        })
        .unwrap();

        assert_eq!(value.from_party_ref().as_str(), "party-a");
        assert_eq!(value.to_party_ref().as_str(), "party-z");
        assert_eq!(
            value.relationship_type().directionality(),
            RelationshipDirectionality::Reciprocal
        );
    }

    #[test]
    fn rejects_self_relationships_and_ambiguous_directionality_roles() {
        let self_error = PartyRelationship::create(CreatePartyRelationship {
            party_relationship_id: PartyRelationshipId::try_new("party-relationship-self").unwrap(),
            from_party_ref: party("party-a"),
            to_party_ref: party("party-a"),
            relationship_type: RelationshipType::partner(),
            valid_from_unix_nanos: None,
            valid_until_unix_nanos: None,
            occurred_at_unix_nanos: 1,
        })
        .unwrap_err();
        assert_eq!(
            self_error.code,
            "PARTY_RELATIONSHIPS_SELF_RELATIONSHIP_INVALID"
        );

        let directional = RelationshipType::try_new(
            "custom_directional",
            RelationshipDirectionality::Directional,
            "member",
            "member",
        )
        .unwrap_err();
        assert_eq!(
            directional.code,
            "PARTY_RELATIONSHIPS_DIRECTIONAL_ROLES_INVALID"
        );

        let reciprocal = RelationshipType::try_new(
            "custom_reciprocal",
            RelationshipDirectionality::Reciprocal,
            "left",
            "right",
        )
        .unwrap_err();
        assert_eq!(
            reciprocal.code,
            "PARTY_RELATIONSHIPS_RECIPROCAL_ROLES_INVALID"
        );
    }

    #[test]
    fn rejects_redefinition_of_reserved_builtin_semantics() {
        let wrong_directionality = RelationshipType::try_new(
            "employment",
            RelationshipDirectionality::Reciprocal,
            "employee",
            "employee",
        )
        .unwrap_err();
        assert_eq!(
            wrong_directionality.code,
            "PARTY_RELATIONSHIPS_RESERVED_TYPE_SEMANTICS_INVALID"
        );

        let wrong_roles = RelationshipType::try_new(
            "parent_subsidiary",
            RelationshipDirectionality::Directional,
            "owner",
            "owned",
        )
        .unwrap_err();
        assert_eq!(
            wrong_roles.code,
            "PARTY_RELATIONSHIPS_RESERVED_TYPE_SEMANTICS_INVALID"
        );
    }

    #[test]
    fn normalizes_bounded_custom_semantics_deterministically() {
        let relationship_type = RelationshipType::try_new(
            "  Strategic_Partner  ",
            RelationshipDirectionality::Reciprocal,
            " PARTNER ",
            "partner",
        )
        .unwrap();
        assert_eq!(relationship_type.code(), "strategic_partner");
        assert_eq!(relationship_type.from_role(), "partner");
        assert_eq!(relationship_type.to_role(), "partner");

        let invalid = RelationshipType::try_new(
            "bad type!",
            RelationshipDirectionality::Directional,
            "source",
            "target",
        )
        .unwrap_err();
        assert_eq!(invalid.code, "PARTY_RELATIONSHIPS_TYPE_CODE_INVALID");
    }

    #[test]
    fn update_preserves_identity_endpoints_and_type_and_advances_exactly_one_version() {
        let mut value = relationship();
        let id = value.party_relationship_id().as_str().to_owned();
        let from = value.from_party_ref().as_str().to_owned();
        let to = value.to_party_ref().as_str().to_owned();
        let relationship_type = value.relationship_type().clone();

        value
            .apply_update(UpdatePartyRelationship {
                expected_version: 1,
                status: PartyRelationshipStatus::Inactive,
                valid_from_unix_nanos: Some(10),
                valid_until_unix_nanos: Some(900),
                occurred_at_unix_nanos: 30,
            })
            .unwrap();

        assert_eq!(value.party_relationship_id().as_str(), id);
        assert_eq!(value.from_party_ref().as_str(), from);
        assert_eq!(value.to_party_ref().as_str(), to);
        assert_eq!(value.relationship_type(), &relationship_type);
        assert_eq!(value.status(), PartyRelationshipStatus::Inactive);
        assert_eq!(value.version(), 2);
        assert_eq!(value.created_at_unix_nanos(), 20);
        assert_eq!(value.updated_at_unix_nanos(), 30);
    }

    #[test]
    fn rejects_invalid_validity_stale_version_time_regression_and_semantic_no_op_atomically() {
        let invalid_validity = PartyRelationship::create(CreatePartyRelationship {
            party_relationship_id: PartyRelationshipId::try_new("party-relationship-invalid")
                .unwrap(),
            from_party_ref: party("party-a"),
            to_party_ref: party("party-b"),
            relationship_type: RelationshipType::parent_subsidiary(),
            valid_from_unix_nanos: Some(20),
            valid_until_unix_nanos: Some(20),
            occurred_at_unix_nanos: 1,
        })
        .unwrap_err();
        assert_eq!(invalid_validity.code, "PARTY_RELATIONSHIPS_VALIDITY_INVALID");

        let mut value = relationship();
        let original = value.clone();
        let stale = value
            .apply_update(UpdatePartyRelationship {
                expected_version: 2,
                status: PartyRelationshipStatus::Inactive,
                valid_from_unix_nanos: Some(10),
                valid_until_unix_nanos: Some(1_000),
                occurred_at_unix_nanos: 30,
            })
            .unwrap_err();
        assert_eq!(stale.code, "PARTY_RELATIONSHIPS_VERSION_CONFLICT");
        assert_eq!(value, original);

        let time_regression = value
            .apply_update(UpdatePartyRelationship {
                expected_version: 1,
                status: PartyRelationshipStatus::Inactive,
                valid_from_unix_nanos: Some(10),
                valid_until_unix_nanos: Some(1_000),
                occurred_at_unix_nanos: 19,
            })
            .unwrap_err();
        assert_eq!(time_regression.code, "PARTY_RELATIONSHIPS_TIME_REGRESSION");
        assert_eq!(value, original);

        let no_op = value
            .apply_update(UpdatePartyRelationship {
                expected_version: 1,
                status: PartyRelationshipStatus::Active,
                valid_from_unix_nanos: Some(10),
                valid_until_unix_nanos: Some(1_000),
                occurred_at_unix_nanos: 20,
            })
            .unwrap_err();
        assert_eq!(no_op.code, "PARTY_RELATIONSHIPS_UPDATE_EMPTY");
        assert_eq!(value, original);
    }

    #[test]
    fn version_exhaustion_is_atomic() {
        let mut value = PartyRelationship::rehydrate(PartyRelationshipSnapshot {
            party_relationship_id: PartyRelationshipId::try_new("party-relationship-max").unwrap(),
            from_party_ref: party("party-a"),
            to_party_ref: party("party-b"),
            relationship_type: RelationshipType::employment(),
            status: PartyRelationshipStatus::Active,
            valid_from_unix_nanos: None,
            valid_until_unix_nanos: None,
            created_at_unix_nanos: 1,
            updated_at_unix_nanos: 2,
            version: i64::MAX,
        })
        .unwrap();
        let original = value.clone();

        let error = value
            .apply_update(UpdatePartyRelationship {
                expected_version: i64::MAX,
                status: PartyRelationshipStatus::Inactive,
                valid_from_unix_nanos: None,
                valid_until_unix_nanos: None,
                occurred_at_unix_nanos: 3,
            })
            .unwrap_err();

        assert_eq!(error.code, "PARTY_RELATIONSHIPS_VERSION_EXHAUSTED");
        assert_eq!(value, original);
    }

    #[test]
    fn rehydrate_rejects_noncanonical_reciprocal_order_and_impossible_version_one_state() {
        let noncanonical = PartyRelationship::rehydrate(PartyRelationshipSnapshot {
            party_relationship_id: PartyRelationshipId::try_new("party-relationship-order")
                .unwrap(),
            from_party_ref: party("party-z"),
            to_party_ref: party("party-a"),
            relationship_type: RelationshipType::household(),
            status: PartyRelationshipStatus::Active,
            valid_from_unix_nanos: None,
            valid_until_unix_nanos: None,
            created_at_unix_nanos: 1,
            updated_at_unix_nanos: 1,
            version: 1,
        })
        .unwrap_err();
        assert_eq!(
            noncanonical.code,
            "PARTY_RELATIONSHIPS_PERSISTED_ENDPOINT_ORDER_INVALID"
        );

        let impossible_initial = PartyRelationship::rehydrate(PartyRelationshipSnapshot {
            party_relationship_id: PartyRelationshipId::try_new("party-relationship-initial")
                .unwrap(),
            from_party_ref: party("party-a"),
            to_party_ref: party("party-b"),
            relationship_type: RelationshipType::employment(),
            status: PartyRelationshipStatus::Inactive,
            valid_from_unix_nanos: None,
            valid_until_unix_nanos: None,
            created_at_unix_nanos: 1,
            updated_at_unix_nanos: 1,
            version: 1,
        })
        .unwrap_err();
        assert_eq!(
            impossible_initial.code,
            "PARTY_RELATIONSHIPS_PERSISTED_INITIAL_STATE_INVALID"
        );
    }

    #[test]
    fn effective_time_is_active_and_uses_half_open_validity_interval() {
        let mut value = relationship();
        assert!(!value.is_effective_at(9));
        assert!(value.is_effective_at(10));
        assert!(value.is_effective_at(999));
        assert!(!value.is_effective_at(1_000));

        value
            .apply_update(UpdatePartyRelationship {
                expected_version: 1,
                status: PartyRelationshipStatus::Inactive,
                valid_from_unix_nanos: Some(10),
                valid_until_unix_nanos: Some(1_000),
                occurred_at_unix_nanos: 30,
            })
            .unwrap();
        assert!(!value.is_effective_at(100));
    }

    #[test]
    fn snapshot_round_trip_preserves_exact_domain_state() {
        let value = relationship();
        let rehydrated = PartyRelationship::rehydrate(value.snapshot()).unwrap();
        assert_eq!(rehydrated, value);
    }
}
