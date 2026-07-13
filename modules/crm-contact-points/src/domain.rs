use crm_module_sdk::{ErrorCategory, FieldName, FieldViolation, RecordId, SdkError};
use url::Url;

const MAX_EMAIL_BYTES: usize = 320;
const MAX_PHONE_DISPLAY_BYTES: usize = 64;
const MAX_POSTAL_BYTES: usize = 1_024;
const MAX_WEB_BYTES: usize = 2_048;
const MAX_MESSAGING_BYTES: usize = 320;
const MAX_MESSAGING_NAMESPACE_BYTES: usize = 64;
const MAX_EVIDENCE_REFERENCE_BYTES: usize = 240;
const MAX_DISPLAY_VALUE_BYTES: usize = 2_048;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContactPointId(RecordId);

impl ContactPointId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, SdkError> {
        RecordId::try_new(value.into()).map(Self).map_err(|error| {
            invalid(
                "CONTACT_POINTS_CONTACT_POINT_ID_INVALID",
                "contact_point.contact_point_id",
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
                "CONTACT_POINTS_PARTY_REFERENCE_INVALID",
                "contact_point.party_ref.party_id",
                error.to_string(),
            )
        })
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ContactPointKind {
    Email,
    Phone,
    Postal,
    Web,
    Messaging,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ContactPointStatus {
    Active,
    Inactive,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationEvidence {
    evidence_ref: String,
    verified_at_unix_nanos: i64,
}

impl VerificationEvidence {
    pub fn evidence_ref(&self) -> &str {
        &self.evidence_ref
    }

    pub const fn verified_at_unix_nanos(&self) -> i64 {
        self.verified_at_unix_nanos
    }

    pub(crate) fn from_persisted(evidence_ref: String, verified_at_unix_nanos: i64) -> Self {
        Self {
            evidence_ref,
            verified_at_unix_nanos,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationState {
    Unverified,
    Verified(VerificationEvidence),
}

impl VerificationState {
    pub const fn is_verified(&self) -> bool {
        matches!(self, Self::Verified(_))
    }

    pub fn evidence(&self) -> Option<&VerificationEvidence> {
        match self {
            Self::Unverified => None,
            Self::Verified(evidence) => Some(evidence),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContactPoint {
    contact_point_id: ContactPointId,
    party_ref: PartyReference,
    kind: ContactPointKind,
    normalized_value: String,
    display_value: String,
    status: ContactPointStatus,
    preferred: bool,
    valid_from_unix_nanos: Option<i64>,
    valid_until_unix_nanos: Option<i64>,
    verification: VerificationState,
    created_at_unix_nanos: i64,
    updated_at_unix_nanos: i64,
    version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContactPointSnapshot {
    pub contact_point_id: ContactPointId,
    pub party_ref: PartyReference,
    pub kind: ContactPointKind,
    pub normalized_value: String,
    pub display_value: String,
    pub status: ContactPointStatus,
    pub preferred: bool,
    pub valid_from_unix_nanos: Option<i64>,
    pub valid_until_unix_nanos: Option<i64>,
    pub verification: VerificationState,
    pub created_at_unix_nanos: i64,
    pub updated_at_unix_nanos: i64,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateContactPoint {
    pub contact_point_id: ContactPointId,
    pub party_ref: PartyReference,
    pub kind: ContactPointKind,
    pub value: String,
    pub preferred: bool,
    pub valid_from_unix_nanos: Option<i64>,
    pub valid_until_unix_nanos: Option<i64>,
    pub occurred_at_unix_nanos: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateContactPoint {
    pub expected_version: i64,
    pub value: String,
    pub status: ContactPointStatus,
    pub preferred: bool,
    pub valid_from_unix_nanos: Option<i64>,
    pub valid_until_unix_nanos: Option<i64>,
    pub occurred_at_unix_nanos: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyContactPoint {
    pub expected_version: i64,
    pub evidence_ref: String,
    pub occurred_at_unix_nanos: i64,
}

impl ContactPoint {
    pub fn create(command: CreateContactPoint) -> Result<Self, SdkError> {
        validate_timestamp(
            "contact_point.occurred_at_unix_nanos",
            command.occurred_at_unix_nanos,
        )?;
        validate_validity(
            command.valid_from_unix_nanos,
            command.valid_until_unix_nanos,
        )?;
        let (normalized_value, display_value) = normalize_value(command.kind, &command.value)?;

        Ok(Self {
            contact_point_id: command.contact_point_id,
            party_ref: command.party_ref,
            kind: command.kind,
            normalized_value,
            display_value,
            status: ContactPointStatus::Active,
            preferred: command.preferred,
            valid_from_unix_nanos: command.valid_from_unix_nanos,
            valid_until_unix_nanos: command.valid_until_unix_nanos,
            verification: VerificationState::Unverified,
            created_at_unix_nanos: command.occurred_at_unix_nanos,
            updated_at_unix_nanos: command.occurred_at_unix_nanos,
            version: 1,
        })
    }

    pub fn rehydrate(snapshot: ContactPointSnapshot) -> Result<Self, SdkError> {
        validate_timestamp(
            "contact_point.created_at_unix_nanos",
            snapshot.created_at_unix_nanos,
        )?;
        validate_timestamp(
            "contact_point.updated_at_unix_nanos",
            snapshot.updated_at_unix_nanos,
        )?;
        if snapshot.updated_at_unix_nanos < snapshot.created_at_unix_nanos {
            return Err(invalid(
                "CONTACT_POINTS_PERSISTED_TIME_INVALID",
                "contact_point.updated_at_unix_nanos",
                "updated time cannot precede created time",
            ));
        }
        if snapshot.version <= 0 {
            return Err(invalid(
                "CONTACT_POINTS_PERSISTED_VERSION_INVALID",
                "contact_point.version",
                "persisted Contact Point version must be positive",
            ));
        }
        validate_validity(
            snapshot.valid_from_unix_nanos,
            snapshot.valid_until_unix_nanos,
        )?;
        validate_initial_transition_shape(
            snapshot.status,
            &snapshot.verification,
            snapshot.created_at_unix_nanos,
            snapshot.updated_at_unix_nanos,
            snapshot.version,
        )?;
        validate_preference(snapshot.status, snapshot.preferred)?;

        let (normalized_value, display_value) =
            normalize_value(snapshot.kind, &snapshot.display_value)?;
        if display_value != snapshot.display_value {
            return Err(invalid(
                "CONTACT_POINTS_PERSISTED_DISPLAY_VALUE_INVALID",
                "contact_point.display_value",
                "persisted display value is not canonical",
            ));
        }
        if normalized_value != snapshot.normalized_value {
            return Err(invalid(
                "CONTACT_POINTS_PERSISTED_NORMALIZED_VALUE_INVALID",
                "contact_point.normalized_value",
                "persisted normalized value does not match the canonical display value",
            ));
        }
        validate_verification(
            &snapshot.verification,
            snapshot.created_at_unix_nanos,
            snapshot.updated_at_unix_nanos,
            snapshot.version,
        )?;

        Ok(Self {
            contact_point_id: snapshot.contact_point_id,
            party_ref: snapshot.party_ref,
            kind: snapshot.kind,
            normalized_value,
            display_value,
            status: snapshot.status,
            preferred: snapshot.preferred,
            valid_from_unix_nanos: snapshot.valid_from_unix_nanos,
            valid_until_unix_nanos: snapshot.valid_until_unix_nanos,
            verification: snapshot.verification,
            created_at_unix_nanos: snapshot.created_at_unix_nanos,
            updated_at_unix_nanos: snapshot.updated_at_unix_nanos,
            version: snapshot.version,
        })
    }

    pub fn apply_update(&mut self, command: UpdateContactPoint) -> Result<(), SdkError> {
        self.require_version(command.expected_version)?;
        self.require_monotonic_time(command.occurred_at_unix_nanos)?;
        validate_validity(
            command.valid_from_unix_nanos,
            command.valid_until_unix_nanos,
        )?;
        validate_preference(command.status, command.preferred)?;
        let (normalized_value, display_value) = normalize_value(self.kind, &command.value)?;
        let value_changed = normalized_value != self.normalized_value;
        let next_verification = if value_changed {
            VerificationState::Unverified
        } else {
            self.verification.clone()
        };

        if normalized_value == self.normalized_value
            && display_value == self.display_value
            && command.status == self.status
            && command.preferred == self.preferred
            && command.valid_from_unix_nanos == self.valid_from_unix_nanos
            && command.valid_until_unix_nanos == self.valid_until_unix_nanos
            && next_verification == self.verification
        {
            return Err(invalid(
                "CONTACT_POINTS_UPDATE_EMPTY",
                "contact_point",
                "updated Contact Point state must differ from the current value",
            ));
        }

        let next_version = self.next_version()?;
        self.normalized_value = normalized_value;
        self.display_value = display_value;
        self.status = command.status;
        self.preferred = command.preferred;
        self.valid_from_unix_nanos = command.valid_from_unix_nanos;
        self.valid_until_unix_nanos = command.valid_until_unix_nanos;
        self.verification = next_verification;
        self.updated_at_unix_nanos = command.occurred_at_unix_nanos;
        self.version = next_version;
        Ok(())
    }

    pub fn verify(&mut self, command: VerifyContactPoint) -> Result<(), SdkError> {
        self.require_version(command.expected_version)?;
        self.require_monotonic_time(command.occurred_at_unix_nanos)?;
        if self.status != ContactPointStatus::Active {
            return Err(invalid(
                "CONTACT_POINTS_VERIFY_INACTIVE",
                "contact_point.status",
                "only an active Contact Point can be verified",
            ));
        }
        if !self.is_valid_at(command.occurred_at_unix_nanos) {
            return Err(invalid(
                "CONTACT_POINTS_VERIFY_OUTSIDE_VALIDITY",
                "contact_point.validity",
                "Contact Point verification time must fall inside the validity interval",
            ));
        }
        if self.verification.is_verified() {
            return Err(conflict(
                "CONTACT_POINTS_ALREADY_VERIFIED",
                "The Contact Point is already verified.",
            ));
        }
        let evidence_ref = normalize_evidence_reference(&command.evidence_ref)?;
        let next_version = self.next_version()?;
        self.verification = VerificationState::Verified(VerificationEvidence {
            evidence_ref,
            verified_at_unix_nanos: command.occurred_at_unix_nanos,
        });
        self.updated_at_unix_nanos = command.occurred_at_unix_nanos;
        self.version = next_version;
        Ok(())
    }

    pub fn snapshot(&self) -> ContactPointSnapshot {
        ContactPointSnapshot {
            contact_point_id: self.contact_point_id.clone(),
            party_ref: self.party_ref.clone(),
            kind: self.kind,
            normalized_value: self.normalized_value.clone(),
            display_value: self.display_value.clone(),
            status: self.status,
            preferred: self.preferred,
            valid_from_unix_nanos: self.valid_from_unix_nanos,
            valid_until_unix_nanos: self.valid_until_unix_nanos,
            verification: self.verification.clone(),
            created_at_unix_nanos: self.created_at_unix_nanos,
            updated_at_unix_nanos: self.updated_at_unix_nanos,
            version: self.version,
        }
    }

    pub fn contact_point_id(&self) -> &ContactPointId {
        &self.contact_point_id
    }

    pub fn party_ref(&self) -> &PartyReference {
        &self.party_ref
    }

    pub const fn kind(&self) -> ContactPointKind {
        self.kind
    }

    pub fn normalized_value(&self) -> &str {
        &self.normalized_value
    }

    pub fn display_value(&self) -> &str {
        &self.display_value
    }

    pub const fn status(&self) -> ContactPointStatus {
        self.status
    }

    pub const fn preferred(&self) -> bool {
        self.preferred
    }

    pub const fn valid_from_unix_nanos(&self) -> Option<i64> {
        self.valid_from_unix_nanos
    }

    pub const fn valid_until_unix_nanos(&self) -> Option<i64> {
        self.valid_until_unix_nanos
    }

    pub fn verification(&self) -> &VerificationState {
        &self.verification
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

    pub fn is_valid_at(&self, unix_nanos: i64) -> bool {
        self.valid_from_unix_nanos
            .is_none_or(|value| unix_nanos >= value)
            && self
                .valid_until_unix_nanos
                .is_none_or(|value| unix_nanos < value)
    }

    fn require_version(&self, expected_version: i64) -> Result<(), SdkError> {
        if expected_version != self.version {
            return Err(conflict(
                "CONTACT_POINTS_VERSION_CONFLICT",
                format!(
                    "expected Contact Point version {expected_version}, found {}",
                    self.version
                ),
            ));
        }
        Ok(())
    }

    fn require_monotonic_time(&self, occurred_at_unix_nanos: i64) -> Result<(), SdkError> {
        validate_timestamp(
            "contact_point.occurred_at_unix_nanos",
            occurred_at_unix_nanos,
        )?;
        if occurred_at_unix_nanos < self.updated_at_unix_nanos {
            return Err(invalid(
                "CONTACT_POINTS_TIME_REGRESSION",
                "contact_point.occurred_at_unix_nanos",
                "Contact Point mutation time cannot precede the previous mutation",
            ));
        }
        Ok(())
    }

    fn next_version(&self) -> Result<i64, SdkError> {
        self.version.checked_add(1).ok_or_else(|| {
            conflict(
                "CONTACT_POINTS_VERSION_EXHAUSTED",
                "Contact Point version cannot be advanced further.",
            )
        })
    }
}

fn validate_initial_transition_shape(
    status: ContactPointStatus,
    verification: &VerificationState,
    created_at_unix_nanos: i64,
    updated_at_unix_nanos: i64,
    version: i64,
) -> Result<(), SdkError> {
    if version == 1
        && (status != ContactPointStatus::Active
            || verification.is_verified()
            || updated_at_unix_nanos != created_at_unix_nanos)
    {
        return Err(invalid(
            "CONTACT_POINTS_PERSISTED_INITIAL_STATE_INVALID",
            "contact_point.version",
            "version-one Contact Point state must match the create transition",
        ));
    }
    Ok(())
}

fn validate_preference(status: ContactPointStatus, preferred: bool) -> Result<(), SdkError> {
    if preferred && status != ContactPointStatus::Active {
        return Err(invalid(
            "CONTACT_POINTS_INACTIVE_PREFERRED_INVALID",
            "contact_point.preferred",
            "an inactive Contact Point cannot be preferred",
        ));
    }
    Ok(())
}

fn validate_verification(
    verification: &VerificationState,
    created_at_unix_nanos: i64,
    updated_at_unix_nanos: i64,
    version: i64,
) -> Result<(), SdkError> {
    let VerificationState::Verified(evidence) = verification else {
        return Ok(());
    };
    validate_timestamp(
        "contact_point.verification.verified_at_unix_nanos",
        evidence.verified_at_unix_nanos,
    )?;
    let normalized = normalize_evidence_reference(&evidence.evidence_ref)?;
    if normalized != evidence.evidence_ref {
        return Err(invalid(
            "CONTACT_POINTS_PERSISTED_VERIFICATION_EVIDENCE_INVALID",
            "contact_point.verification.evidence_ref",
            "persisted verification evidence reference is not canonical",
        ));
    }
    if version < 2
        || evidence.verified_at_unix_nanos < created_at_unix_nanos
        || evidence.verified_at_unix_nanos > updated_at_unix_nanos
    {
        return Err(invalid(
            "CONTACT_POINTS_PERSISTED_VERIFICATION_TIME_INVALID",
            "contact_point.verification.verified_at_unix_nanos",
            "verification time must be within the aggregate mutation timeline",
        ));
    }
    Ok(())
}

fn validate_validity(
    valid_from_unix_nanos: Option<i64>,
    valid_until_unix_nanos: Option<i64>,
) -> Result<(), SdkError> {
    if let Some(value) = valid_from_unix_nanos {
        validate_timestamp("contact_point.valid_from_unix_nanos", value)?;
    }
    if let Some(value) = valid_until_unix_nanos {
        validate_timestamp("contact_point.valid_until_unix_nanos", value)?;
    }
    if matches!(
        (valid_from_unix_nanos, valid_until_unix_nanos),
        (Some(from), Some(until)) if until <= from
    ) {
        return Err(invalid(
            "CONTACT_POINTS_VALIDITY_INVALID",
            "contact_point.validity",
            "valid-until must be later than valid-from",
        ));
    }
    Ok(())
}

fn normalize_value(kind: ContactPointKind, value: &str) -> Result<(String, String), SdkError> {
    match kind {
        ContactPointKind::Email => normalize_email(value),
        ContactPointKind::Phone => normalize_phone(value),
        ContactPointKind::Postal => normalize_postal(value),
        ContactPointKind::Web => normalize_web(value),
        ContactPointKind::Messaging => normalize_messaging(value),
    }
}

fn normalize_email(value: &str) -> Result<(String, String), SdkError> {
    let display = normalize_display(value, MAX_EMAIL_BYTES, "email")?;
    if display.chars().any(char::is_whitespace) {
        return Err(value_invalid("email must not contain whitespace"));
    }
    let mut parts = display.rsplitn(2, '@');
    let domain = parts.next().unwrap_or_default();
    let local = parts.next().unwrap_or_default();
    if local.is_empty()
        || domain.is_empty()
        || local.contains('@')
        || local.len() > 64
        || local.starts_with('.')
        || local.ends_with('.')
        || local.contains("..")
    {
        return Err(value_invalid("email address is invalid"));
    }

    let ascii_domain = idna::domain_to_ascii(domain)
        .map_err(|_| value_invalid("email domain is invalid"))?
        .to_ascii_lowercase();
    validate_ascii_domain(&ascii_domain)?;
    let normalized = format!("{local}@{ascii_domain}");
    if normalized.len() > MAX_EMAIL_BYTES {
        return Err(value_invalid("normalized email address is too long"));
    }
    Ok((normalized, display))
}

fn validate_ascii_domain(domain: &str) -> Result<(), SdkError> {
    if domain.is_empty() || domain.len() > 253 || !domain.contains('.') || domain.ends_with('.') {
        return Err(value_invalid("email domain is invalid"));
    }
    if domain.split('.').any(|label| {
        label.is_empty()
            || label.len() > 63
            || label.starts_with('-')
            || label.ends_with('-')
            || !label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    }) {
        return Err(value_invalid("email domain is invalid"));
    }
    Ok(())
}

fn normalize_phone(value: &str) -> Result<(String, String), SdkError> {
    let display = normalize_display(value, MAX_PHONE_DISPLAY_BYTES, "phone")?;
    let mut normalized = String::with_capacity(display.len());
    for (index, character) in display.chars().enumerate() {
        match character {
            '+' if index == 0 => normalized.push(character),
            '0'..='9' => normalized.push(character),
            ' ' | '-' | '(' | ')' | '.' => {}
            _ => {
                return Err(value_invalid(
                    "phone must be an E.164 number with safe separators",
                ));
            }
        }
    }
    if !normalized.starts_with('+') {
        return Err(value_invalid(
            "phone must start with '+' and include a country code",
        ));
    }
    let digit_count = normalized.len().saturating_sub(1);
    if !(8..=15).contains(&digit_count) || normalized.as_bytes().get(1) == Some(&b'0') {
        return Err(value_invalid("phone must contain 8 to 15 E.164 digits"));
    }
    Ok((normalized, display))
}

fn normalize_postal(value: &str) -> Result<(String, String), SdkError> {
    let display = normalize_display(value, MAX_POSTAL_BYTES, "postal address")?;
    Ok((display.clone(), display))
}

fn normalize_web(value: &str) -> Result<(String, String), SdkError> {
    let display = normalize_display(value, MAX_WEB_BYTES, "web address")?;
    let mut parsed = Url::parse(&display).map_err(|_| value_invalid("web address is invalid"))?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err(value_invalid(
            "web address must be an absolute HTTP or HTTPS URL",
        ));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(value_invalid(
            "web address must not contain embedded credentials",
        ));
    }
    parsed.set_fragment(None);
    let normalized = parsed.to_string();
    if normalized.len() > MAX_WEB_BYTES {
        return Err(value_invalid("normalized web address is too long"));
    }
    Ok((normalized, display))
}

fn normalize_messaging(value: &str) -> Result<(String, String), SdkError> {
    let display = normalize_display(value, MAX_MESSAGING_BYTES, "messaging endpoint")?;
    let (namespace, address) = display
        .split_once(':')
        .ok_or_else(|| value_invalid("messaging endpoint must use 'namespace:address' form"))?;
    let namespace = namespace.to_ascii_lowercase();
    if namespace.is_empty()
        || namespace.len() > MAX_MESSAGING_NAMESPACE_BYTES
        || !namespace.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_')
        })
    {
        return Err(value_invalid("messaging namespace is invalid"));
    }
    if address.is_empty() || address.chars().any(char::is_whitespace) {
        return Err(value_invalid("messaging address is invalid"));
    }
    Ok((format!("{namespace}:{address}"), display))
}

fn normalize_display(value: &str, maximum_bytes: usize, label: &str) -> Result<String, SdkError> {
    if value.chars().any(char::is_control) {
        return Err(value_invalid(format!(
            "{label} must not contain control characters"
        )));
    }
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty()
        || normalized.len() > maximum_bytes
        || normalized.len() > MAX_DISPLAY_VALUE_BYTES
    {
        return Err(value_invalid(format!(
            "{label} must be non-empty and within the supported size limit"
        )));
    }
    Ok(normalized)
}

fn normalize_evidence_reference(value: &str) -> Result<String, SdkError> {
    if value.chars().any(char::is_control) {
        return Err(invalid(
            "CONTACT_POINTS_VERIFICATION_EVIDENCE_INVALID",
            "contact_point.verification.evidence_ref",
            "verification evidence reference must not contain control characters",
        ));
    }
    let normalized = value.trim().to_owned();
    if normalized.is_empty() || normalized.len() > MAX_EVIDENCE_REFERENCE_BYTES {
        return Err(invalid(
            "CONTACT_POINTS_VERIFICATION_EVIDENCE_INVALID",
            "contact_point.verification.evidence_ref",
            format!(
                "verification evidence reference must be non-empty and not exceed {MAX_EVIDENCE_REFERENCE_BYTES} UTF-8 bytes"
            ),
        ));
    }
    Ok(normalized)
}

fn value_invalid(message: impl Into<String>) -> SdkError {
    invalid(
        "CONTACT_POINTS_VALUE_INVALID",
        "contact_point.value",
        message,
    )
}

fn validate_timestamp(field: &'static str, value: i64) -> Result<(), SdkError> {
    if value < 0 {
        return Err(invalid(
            "CONTACT_POINTS_TIMESTAMP_INVALID",
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
        "The Contact Point request contains invalid data.",
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
    use std::cmp::Ordering;

    fn create(kind: ContactPointKind, value: &str) -> ContactPoint {
        ContactPoint::create(CreateContactPoint {
            contact_point_id: ContactPointId::try_new("contact-point-1").unwrap(),
            party_ref: PartyReference::try_new("party-1").unwrap(),
            kind,
            value: value.to_owned(),
            preferred: true,
            valid_from_unix_nanos: Some(10),
            valid_until_unix_nanos: Some(1_000),
            occurred_at_unix_nanos: 10,
        })
        .unwrap()
    }

    #[test]
    fn normalizes_all_channel_values_deterministically() {
        let email = create(ContactPointKind::Email, "Ada@BÜCHER.Example");
        assert_eq!(email.normalized_value(), "Ada@xn--bcher-kva.example");
        assert_eq!(email.display_value(), "Ada@BÜCHER.Example");

        let phone = create(ContactPointKind::Phone, "+370 (612) 34-567");
        assert_eq!(phone.normalized_value(), "+37061234567");

        let web = create(
            ContactPointKind::Web,
            "HTTPS://Example.COM:443/a/../path#fragment",
        );
        assert_eq!(web.normalized_value(), "https://example.com/path");

        let messaging = create(ContactPointKind::Messaging, "Telegram:@Ada");
        assert_eq!(messaging.normalized_value(), "telegram:@Ada");

        let postal = create(ContactPointKind::Postal, "  Vilnius   LT  ");
        assert_eq!(postal.normalized_value(), "Vilnius LT");
    }

    #[test]
    fn rejects_ambiguous_or_unsafe_channel_values() {
        for invalid_email in [
            "ada@example",
            ".ada@example.com",
            "ada..lovelace@example.com",
            "ada@example.com.",
        ] {
            assert_eq!(
                ContactPoint::create(CreateContactPoint {
                    contact_point_id: ContactPointId::try_new("contact-point-email").unwrap(),
                    party_ref: PartyReference::try_new("party-1").unwrap(),
                    kind: ContactPointKind::Email,
                    value: invalid_email.to_owned(),
                    preferred: false,
                    valid_from_unix_nanos: None,
                    valid_until_unix_nanos: None,
                    occurred_at_unix_nanos: 1,
                })
                .unwrap_err()
                .code,
                "CONTACT_POINTS_VALUE_INVALID"
            );
        }

        assert_eq!(
            ContactPoint::create(CreateContactPoint {
                contact_point_id: ContactPointId::try_new("contact-point-web").unwrap(),
                party_ref: PartyReference::try_new("party-1").unwrap(),
                kind: ContactPointKind::Web,
                value: "https://user:secret@example.com".to_owned(),
                preferred: false,
                valid_from_unix_nanos: None,
                valid_until_unix_nanos: None,
                occurred_at_unix_nanos: 1,
            })
            .unwrap_err()
            .code,
            "CONTACT_POINTS_VALUE_INVALID"
        );
    }

    #[test]
    fn value_change_resets_verification_but_display_only_change_preserves_it() {
        let mut value = create(ContactPointKind::Phone, "+370 612 34567");
        value
            .verify(VerifyContactPoint {
                expected_version: 1,
                evidence_ref: "verification-1".to_owned(),
                occurred_at_unix_nanos: 20,
            })
            .unwrap();
        assert!(value.verification().is_verified());

        value
            .apply_update(UpdateContactPoint {
                expected_version: 2,
                value: "+370 (612) 34567".to_owned(),
                status: ContactPointStatus::Active,
                preferred: true,
                valid_from_unix_nanos: Some(10),
                valid_until_unix_nanos: Some(1_000),
                occurred_at_unix_nanos: 30,
            })
            .unwrap();
        assert!(value.verification().is_verified());

        value
            .apply_update(UpdateContactPoint {
                expected_version: 3,
                value: "+370 699 99999".to_owned(),
                status: ContactPointStatus::Active,
                preferred: true,
                valid_from_unix_nanos: Some(10),
                valid_until_unix_nanos: Some(1_000),
                occurred_at_unix_nanos: 40,
            })
            .unwrap();
        assert!(!value.verification().is_verified());
    }

    #[test]
    fn lifecycle_and_validity_only_updates_preserve_historical_verification() {
        let mut value = create(ContactPointKind::Email, "ada@example.com");
        value
            .verify(VerifyContactPoint {
                expected_version: 1,
                evidence_ref: "verification-1".to_owned(),
                occurred_at_unix_nanos: 20,
            })
            .unwrap();
        value
            .apply_update(UpdateContactPoint {
                expected_version: 2,
                value: "ada@example.com".to_owned(),
                status: ContactPointStatus::Inactive,
                preferred: false,
                valid_from_unix_nanos: Some(100),
                valid_until_unix_nanos: Some(2_000),
                occurred_at_unix_nanos: 30,
            })
            .unwrap();

        assert!(value.verification().is_verified());
        assert_eq!(value.status(), ContactPointStatus::Inactive);
        assert_eq!(ContactPoint::rehydrate(value.snapshot()).unwrap(), value);
    }

    #[test]
    fn verification_requires_active_current_endpoint_and_canonical_evidence() {
        let mut value = create(ContactPointKind::Email, "ada@example.com");
        let outside = value
            .verify(VerifyContactPoint {
                expected_version: 1,
                evidence_ref: "evidence-1".to_owned(),
                occurred_at_unix_nanos: 1_000,
            })
            .unwrap_err();
        assert_eq!(outside.code, "CONTACT_POINTS_VERIFY_OUTSIDE_VALIDITY");

        let invalid_evidence = value
            .verify(VerifyContactPoint {
                expected_version: 1,
                evidence_ref: "   ".to_owned(),
                occurred_at_unix_nanos: 20,
            })
            .unwrap_err();
        assert_eq!(
            invalid_evidence.code,
            "CONTACT_POINTS_VERIFICATION_EVIDENCE_INVALID"
        );

        value
            .apply_update(UpdateContactPoint {
                expected_version: 1,
                value: "ada@example.com".to_owned(),
                status: ContactPointStatus::Inactive,
                preferred: false,
                valid_from_unix_nanos: Some(10),
                valid_until_unix_nanos: Some(1_000),
                occurred_at_unix_nanos: 20,
            })
            .unwrap();
        let inactive = value
            .verify(VerifyContactPoint {
                expected_version: 2,
                evidence_ref: "evidence-1".to_owned(),
                occurred_at_unix_nanos: 30,
            })
            .unwrap_err();
        assert_eq!(inactive.code, "CONTACT_POINTS_VERIFY_INACTIVE");
    }

    #[test]
    fn rejects_inactive_preferred_invalid_validity_stale_version_and_empty_update() {
        let mut value = create(ContactPointKind::Email, "ada@example.com");
        let inactive_preferred = value
            .apply_update(UpdateContactPoint {
                expected_version: 1,
                value: "ada@example.com".to_owned(),
                status: ContactPointStatus::Inactive,
                preferred: true,
                valid_from_unix_nanos: Some(10),
                valid_until_unix_nanos: Some(1_000),
                occurred_at_unix_nanos: 20,
            })
            .unwrap_err();
        assert_eq!(
            inactive_preferred.code,
            "CONTACT_POINTS_INACTIVE_PREFERRED_INVALID"
        );

        let invalid_validity = ContactPoint::create(CreateContactPoint {
            contact_point_id: ContactPointId::try_new("contact-point-invalid").unwrap(),
            party_ref: PartyReference::try_new("party-1").unwrap(),
            kind: ContactPointKind::Email,
            value: "ada@example.com".to_owned(),
            preferred: false,
            valid_from_unix_nanos: Some(100),
            valid_until_unix_nanos: Some(100),
            occurred_at_unix_nanos: 10,
        })
        .unwrap_err();
        assert_eq!(invalid_validity.code, "CONTACT_POINTS_VALIDITY_INVALID");

        let stale = value
            .apply_update(UpdateContactPoint {
                expected_version: 2,
                value: "new@example.com".to_owned(),
                status: ContactPointStatus::Active,
                preferred: false,
                valid_from_unix_nanos: Some(10),
                valid_until_unix_nanos: Some(1_000),
                occurred_at_unix_nanos: 20,
            })
            .unwrap_err();
        assert_eq!(stale.code, "CONTACT_POINTS_VERSION_CONFLICT");

        let empty = value
            .apply_update(UpdateContactPoint {
                expected_version: 1,
                value: "ada@example.com".to_owned(),
                status: ContactPointStatus::Active,
                preferred: true,
                valid_from_unix_nanos: Some(10),
                valid_until_unix_nanos: Some(1_000),
                occurred_at_unix_nanos: 20,
            })
            .unwrap_err();
        assert_eq!(empty.code, "CONTACT_POINTS_UPDATE_EMPTY");
    }

    #[test]
    fn rejects_time_regression_and_keeps_failed_version_exhaustion_atomic() {
        let mut value = create(ContactPointKind::Email, "ada@example.com");
        let regression = value
            .apply_update(UpdateContactPoint {
                expected_version: 1,
                value: "new@example.com".to_owned(),
                status: ContactPointStatus::Active,
                preferred: false,
                valid_from_unix_nanos: Some(10),
                valid_until_unix_nanos: Some(1_000),
                occurred_at_unix_nanos: 9,
            })
            .unwrap_err();
        assert_eq!(regression.code, "CONTACT_POINTS_TIME_REGRESSION");

        let mut exhausted_snapshot = value.snapshot();
        exhausted_snapshot.version = i64::MAX;
        let mut exhausted = ContactPoint::rehydrate(exhausted_snapshot).unwrap();
        let before = exhausted.clone();
        let error = exhausted
            .apply_update(UpdateContactPoint {
                expected_version: i64::MAX,
                value: "new@example.com".to_owned(),
                status: ContactPointStatus::Active,
                preferred: false,
                valid_from_unix_nanos: Some(10),
                valid_until_unix_nanos: Some(1_000),
                occurred_at_unix_nanos: 20,
            })
            .unwrap_err();
        assert_eq!(error.code, "CONTACT_POINTS_VERSION_EXHAUSTED");
        assert_eq!(exhausted, before);
    }

    #[test]
    fn rehydrate_rejects_noncanonical_and_impossible_persisted_state() {
        let value = create(ContactPointKind::Email, "ada@example.com");

        let mut display = value.snapshot();
        display.display_value = "  ada@example.com  ".to_owned();
        assert_eq!(
            ContactPoint::rehydrate(display).unwrap_err().code,
            "CONTACT_POINTS_PERSISTED_DISPLAY_VALUE_INVALID"
        );

        let mut normalized = value.snapshot();
        normalized.normalized_value = "ADA@example.com".to_owned();
        assert_eq!(
            ContactPoint::rehydrate(normalized).unwrap_err().code,
            "CONTACT_POINTS_PERSISTED_NORMALIZED_VALUE_INVALID"
        );

        let mut impossible = value.snapshot();
        impossible.status = ContactPointStatus::Inactive;
        assert_eq!(
            ContactPoint::rehydrate(impossible).unwrap_err().code,
            "CONTACT_POINTS_PERSISTED_INITIAL_STATE_INVALID"
        );
    }

    #[test]
    fn snapshot_round_trip_preserves_exact_domain_state() {
        let mut value = create(ContactPointKind::Messaging, "Telegram:@Ada");
        value
            .verify(VerifyContactPoint {
                expected_version: 1,
                evidence_ref: "verification-42".to_owned(),
                occurred_at_unix_nanos: 42,
            })
            .unwrap();
        let rehydrated = ContactPoint::rehydrate(value.snapshot()).unwrap();
        assert_eq!(rehydrated, value);
    }

    #[test]
    fn kind_order_is_stable_for_persisted_and_query_normalization() {
        let mut kinds = vec![
            ContactPointKind::Messaging,
            ContactPointKind::Email,
            ContactPointKind::Web,
            ContactPointKind::Phone,
            ContactPointKind::Postal,
        ];
        kinds.sort();
        assert_eq!(
            kinds,
            vec![
                ContactPointKind::Email,
                ContactPointKind::Phone,
                ContactPointKind::Postal,
                ContactPointKind::Web,
                ContactPointKind::Messaging,
            ]
        );
        assert_eq!(
            Ordering::Equal,
            ContactPointKind::Email.cmp(&ContactPointKind::Email)
        );
    }
}
