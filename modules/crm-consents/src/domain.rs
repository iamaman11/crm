use crm_module_sdk::{ErrorCategory, FieldName, FieldViolation, RecordId, SdkError};

const MAX_PURPOSE_CODE_BYTES: usize = 128;
const MAX_LEGAL_BASIS_CODE_BYTES: usize = 96;
const MAX_JURISDICTION_CODE_BYTES: usize = 64;
const MAX_SOURCE_CODE_BYTES: usize = 96;
const MAX_EVIDENCE_REFERENCE_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConsentAuthorizationId(RecordId);

impl ConsentAuthorizationId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, SdkError> {
        RecordId::try_new(value.into()).map(Self).map_err(|error| {
            invalid(
                "CONSENTS_AUTHORIZATION_ID_INVALID",
                "consent_authorization.authorization_id",
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
                "CONSENTS_PARTY_REFERENCE_INVALID",
                "consent_authorization.party_ref.party_id",
                error.to_string(),
            )
        })
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContactPointReference(RecordId);

impl ContactPointReference {
    pub fn try_new(value: impl Into<String>) -> Result<Self, SdkError> {
        RecordId::try_new(value.into()).map(Self).map_err(|error| {
            invalid(
                "CONSENTS_CONTACT_POINT_REFERENCE_INVALID",
                "consent_authorization.contact_point_ref.contact_point_id",
                error.to_string(),
            )
        })
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

macro_rules! semantic_code_type {
    ($name:ident, $maximum:expr, $code:literal, $field:literal, $label:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn try_new(value: impl Into<String>) -> Result<Self, SdkError> {
                normalize_semantic_identifier(
                    &value.into(),
                    $maximum,
                    $code,
                    $field,
                    $label,
                )
                .map(Self)
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

semantic_code_type!(
    PurposeCode,
    MAX_PURPOSE_CODE_BYTES,
    "CONSENTS_PURPOSE_CODE_INVALID",
    "consent_authorization.purpose",
    "purpose code"
);
semantic_code_type!(
    LegalBasisCode,
    MAX_LEGAL_BASIS_CODE_BYTES,
    "CONSENTS_LEGAL_BASIS_CODE_INVALID",
    "consent_authorization.legal_basis",
    "legal-basis code"
);
semantic_code_type!(
    JurisdictionCode,
    MAX_JURISDICTION_CODE_BYTES,
    "CONSENTS_JURISDICTION_CODE_INVALID",
    "consent_authorization.jurisdiction",
    "jurisdiction code"
);
semantic_code_type!(
    SourceCode,
    MAX_SOURCE_CODE_BYTES,
    "CONSENTS_SOURCE_CODE_INVALID",
    "consent_authorization.source",
    "source code"
);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EvidenceReference(String);

impl EvidenceReference {
    pub fn try_new(value: impl Into<String>) -> Result<Self, SdkError> {
        let value = value.into();
        if value.chars().any(char::is_control) {
            return Err(invalid(
                "CONSENTS_EVIDENCE_REFERENCE_INVALID",
                "consent_authorization.evidence_ref",
                "evidence reference must not contain control characters",
            ));
        }
        let canonical = value.trim().to_owned();
        if canonical.is_empty() || canonical.len() > MAX_EVIDENCE_REFERENCE_BYTES {
            return Err(invalid(
                "CONSENTS_EVIDENCE_REFERENCE_INVALID",
                "consent_authorization.evidence_ref",
                format!(
                    "evidence reference must be non-empty and not exceed {MAX_EVIDENCE_REFERENCE_BYTES} UTF-8 bytes"
                ),
            ));
        }
        Ok(Self(canonical))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CommunicationChannel {
    Email,
    Phone,
    Sms,
    Postal,
    Messaging,
    Push,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConsentEffect {
    Grant,
    Deny,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConsentAuthorizationStatus {
    Active,
    Withdrawn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsentDecisionPointEffect {
    Grant,
    Deny,
    Withdrawal,
}

impl ConsentDecisionPointEffect {
    pub const fn allows_communication(self) -> bool {
        matches!(self, Self::Grant)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConsentDecisionPoint {
    pub occurred_at_unix_nanos: i64,
    pub effect: ConsentDecisionPointEffect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsentAuthorization {
    authorization_id: ConsentAuthorizationId,
    party_ref: PartyReference,
    contact_point_ref: Option<ContactPointReference>,
    purpose: PurposeCode,
    channel: CommunicationChannel,
    effect: ConsentEffect,
    legal_basis: LegalBasisCode,
    jurisdiction: JurisdictionCode,
    source: SourceCode,
    evidence_ref: EvidenceReference,
    effective_from_unix_nanos: i64,
    expires_at_unix_nanos: Option<i64>,
    status: ConsentAuthorizationStatus,
    withdrawn_at_unix_nanos: Option<i64>,
    created_at_unix_nanos: i64,
    updated_at_unix_nanos: i64,
    version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsentAuthorizationSnapshot {
    pub authorization_id: ConsentAuthorizationId,
    pub party_ref: PartyReference,
    pub contact_point_ref: Option<ContactPointReference>,
    pub purpose: PurposeCode,
    pub channel: CommunicationChannel,
    pub effect: ConsentEffect,
    pub legal_basis: LegalBasisCode,
    pub jurisdiction: JurisdictionCode,
    pub source: SourceCode,
    pub evidence_ref: EvidenceReference,
    pub effective_from_unix_nanos: i64,
    pub expires_at_unix_nanos: Option<i64>,
    pub status: ConsentAuthorizationStatus,
    pub withdrawn_at_unix_nanos: Option<i64>,
    pub created_at_unix_nanos: i64,
    pub updated_at_unix_nanos: i64,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateConsentAuthorization {
    pub authorization_id: ConsentAuthorizationId,
    pub party_ref: PartyReference,
    pub contact_point_ref: Option<ContactPointReference>,
    pub purpose: PurposeCode,
    pub channel: CommunicationChannel,
    pub effect: ConsentEffect,
    pub legal_basis: LegalBasisCode,
    pub jurisdiction: JurisdictionCode,
    pub source: SourceCode,
    pub evidence_ref: EvidenceReference,
    pub effective_from_unix_nanos: i64,
    pub expires_at_unix_nanos: Option<i64>,
    pub occurred_at_unix_nanos: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WithdrawConsentAuthorization {
    pub expected_version: i64,
    pub occurred_at_unix_nanos: i64,
}

impl ConsentAuthorization {
    pub fn create(command: CreateConsentAuthorization) -> Result<Self, SdkError> {
        validate_timestamp(
            "consent_authorization.occurred_at_unix_nanos",
            command.occurred_at_unix_nanos,
        )?;
        validate_timestamp(
            "consent_authorization.effective_from_unix_nanos",
            command.effective_from_unix_nanos,
        )?;
        if command.effective_from_unix_nanos < command.occurred_at_unix_nanos {
            return Err(invalid(
                "CONSENTS_EFFECTIVE_TIME_INVALID",
                "consent_authorization.effective_from_unix_nanos",
                "effective time cannot precede the authoritative assertion creation time",
            ));
        }
        validate_expiry(
            command.effective_from_unix_nanos,
            command.expires_at_unix_nanos,
        )?;

        Ok(Self {
            authorization_id: command.authorization_id,
            party_ref: command.party_ref,
            contact_point_ref: command.contact_point_ref,
            purpose: command.purpose,
            channel: command.channel,
            effect: command.effect,
            legal_basis: command.legal_basis,
            jurisdiction: command.jurisdiction,
            source: command.source,
            evidence_ref: command.evidence_ref,
            effective_from_unix_nanos: command.effective_from_unix_nanos,
            expires_at_unix_nanos: command.expires_at_unix_nanos,
            status: ConsentAuthorizationStatus::Active,
            withdrawn_at_unix_nanos: None,
            created_at_unix_nanos: command.occurred_at_unix_nanos,
            updated_at_unix_nanos: command.occurred_at_unix_nanos,
            version: 1,
        })
    }

    pub fn rehydrate(snapshot: ConsentAuthorizationSnapshot) -> Result<Self, SdkError> {
        validate_timestamp(
            "consent_authorization.created_at_unix_nanos",
            snapshot.created_at_unix_nanos,
        )?;
        validate_timestamp(
            "consent_authorization.updated_at_unix_nanos",
            snapshot.updated_at_unix_nanos,
        )?;
        validate_timestamp(
            "consent_authorization.effective_from_unix_nanos",
            snapshot.effective_from_unix_nanos,
        )?;
        if snapshot.updated_at_unix_nanos < snapshot.created_at_unix_nanos {
            return Err(invalid(
                "CONSENTS_PERSISTED_TIME_INVALID",
                "consent_authorization.updated_at_unix_nanos",
                "updated time cannot precede created time",
            ));
        }
        if snapshot.effective_from_unix_nanos < snapshot.created_at_unix_nanos {
            return Err(invalid(
                "CONSENTS_PERSISTED_EFFECTIVE_TIME_INVALID",
                "consent_authorization.effective_from_unix_nanos",
                "persisted effective time cannot precede the authoritative assertion creation time",
            ));
        }
        validate_expiry(
            snapshot.effective_from_unix_nanos,
            snapshot.expires_at_unix_nanos,
        )?;
        validate_persisted_lifecycle(&snapshot)?;

        Ok(Self {
            authorization_id: snapshot.authorization_id,
            party_ref: snapshot.party_ref,
            contact_point_ref: snapshot.contact_point_ref,
            purpose: snapshot.purpose,
            channel: snapshot.channel,
            effect: snapshot.effect,
            legal_basis: snapshot.legal_basis,
            jurisdiction: snapshot.jurisdiction,
            source: snapshot.source,
            evidence_ref: snapshot.evidence_ref,
            effective_from_unix_nanos: snapshot.effective_from_unix_nanos,
            expires_at_unix_nanos: snapshot.expires_at_unix_nanos,
            status: snapshot.status,
            withdrawn_at_unix_nanos: snapshot.withdrawn_at_unix_nanos,
            created_at_unix_nanos: snapshot.created_at_unix_nanos,
            updated_at_unix_nanos: snapshot.updated_at_unix_nanos,
            version: snapshot.version,
        })
    }

    pub fn withdraw(&mut self, command: WithdrawConsentAuthorization) -> Result<(), SdkError> {
        self.require_version(command.expected_version)?;
        self.require_strictly_increasing_time(command.occurred_at_unix_nanos)?;
        if self.effect != ConsentEffect::Grant {
            return Err(invalid(
                "CONSENTS_DENY_WITHDRAWAL_INVALID",
                "consent_authorization.effect",
                "a deny assertion cannot use the grant-withdrawal transition",
            ));
        }
        if self.status != ConsentAuthorizationStatus::Active {
            return Err(invalid(
                "CONSENTS_ALREADY_WITHDRAWN",
                "consent_authorization.status",
                "the Consent Authorization grant is already withdrawn",
            ));
        }
        if self
            .expires_at_unix_nanos
            .is_some_and(|expires_at| command.occurred_at_unix_nanos >= expires_at)
        {
            return Err(invalid(
                "CONSENTS_EXPIRED_GRANT_WITHDRAWAL_INVALID",
                "consent_authorization.expires_at_unix_nanos",
                "an already expired grant cannot be withdrawn",
            ));
        }
        let next_version = self.next_version()?;
        self.status = ConsentAuthorizationStatus::Withdrawn;
        self.withdrawn_at_unix_nanos = Some(command.occurred_at_unix_nanos);
        self.updated_at_unix_nanos = command.occurred_at_unix_nanos;
        self.version = next_version;
        Ok(())
    }

    pub fn decision_point_at(&self, evaluation_time_unix_nanos: i64) -> Option<ConsentDecisionPoint> {
        if evaluation_time_unix_nanos <= 0
            || evaluation_time_unix_nanos < self.effective_from_unix_nanos
        {
            return None;
        }

        match self.status {
            ConsentAuthorizationStatus::Active => {
                if self
                    .expires_at_unix_nanos
                    .is_some_and(|expires_at| evaluation_time_unix_nanos >= expires_at)
                {
                    return None;
                }
                Some(ConsentDecisionPoint {
                    occurred_at_unix_nanos: self.effective_from_unix_nanos,
                    effect: match self.effect {
                        ConsentEffect::Grant => ConsentDecisionPointEffect::Grant,
                        ConsentEffect::Deny => ConsentDecisionPointEffect::Deny,
                    },
                })
            }
            ConsentAuthorizationStatus::Withdrawn => {
                let withdrawn_at = self
                    .withdrawn_at_unix_nanos
                    .expect("rehydrated withdrawn Consent Authorization must have withdrawn time");
                if evaluation_time_unix_nanos < withdrawn_at {
                    if self
                        .expires_at_unix_nanos
                        .is_some_and(|expires_at| evaluation_time_unix_nanos >= expires_at)
                    {
                        None
                    } else {
                        Some(ConsentDecisionPoint {
                            occurred_at_unix_nanos: self.effective_from_unix_nanos,
                            effect: ConsentDecisionPointEffect::Grant,
                        })
                    }
                } else {
                    Some(ConsentDecisionPoint {
                        occurred_at_unix_nanos: withdrawn_at,
                        effect: ConsentDecisionPointEffect::Withdrawal,
                    })
                }
            }
        }
    }

    pub fn snapshot(&self) -> ConsentAuthorizationSnapshot {
        ConsentAuthorizationSnapshot {
            authorization_id: self.authorization_id.clone(),
            party_ref: self.party_ref.clone(),
            contact_point_ref: self.contact_point_ref.clone(),
            purpose: self.purpose.clone(),
            channel: self.channel,
            effect: self.effect,
            legal_basis: self.legal_basis.clone(),
            jurisdiction: self.jurisdiction.clone(),
            source: self.source.clone(),
            evidence_ref: self.evidence_ref.clone(),
            effective_from_unix_nanos: self.effective_from_unix_nanos,
            expires_at_unix_nanos: self.expires_at_unix_nanos,
            status: self.status,
            withdrawn_at_unix_nanos: self.withdrawn_at_unix_nanos,
            created_at_unix_nanos: self.created_at_unix_nanos,
            updated_at_unix_nanos: self.updated_at_unix_nanos,
            version: self.version,
        }
    }

    pub fn authorization_id(&self) -> &ConsentAuthorizationId {
        &self.authorization_id
    }

    pub fn party_ref(&self) -> &PartyReference {
        &self.party_ref
    }

    pub fn contact_point_ref(&self) -> Option<&ContactPointReference> {
        self.contact_point_ref.as_ref()
    }

    pub fn purpose(&self) -> &PurposeCode {
        &self.purpose
    }

    pub const fn channel(&self) -> CommunicationChannel {
        self.channel
    }

    pub const fn effect(&self) -> ConsentEffect {
        self.effect
    }

    pub fn legal_basis(&self) -> &LegalBasisCode {
        &self.legal_basis
    }

    pub fn jurisdiction(&self) -> &JurisdictionCode {
        &self.jurisdiction
    }

    pub fn source(&self) -> &SourceCode {
        &self.source
    }

    pub fn evidence_ref(&self) -> &EvidenceReference {
        &self.evidence_ref
    }

    pub const fn effective_from_unix_nanos(&self) -> i64 {
        self.effective_from_unix_nanos
    }

    pub const fn expires_at_unix_nanos(&self) -> Option<i64> {
        self.expires_at_unix_nanos
    }

    pub const fn status(&self) -> ConsentAuthorizationStatus {
        self.status
    }

    pub const fn withdrawn_at_unix_nanos(&self) -> Option<i64> {
        self.withdrawn_at_unix_nanos
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
                "CONSENTS_VERSION_CONFLICT",
                format!(
                    "expected Consent Authorization version {expected_version}, found {}",
                    self.version
                ),
            ));
        }
        Ok(())
    }

    fn require_strictly_increasing_time(
        &self,
        occurred_at_unix_nanos: i64,
    ) -> Result<(), SdkError> {
        validate_timestamp(
            "consent_authorization.occurred_at_unix_nanos",
            occurred_at_unix_nanos,
        )?;
        if occurred_at_unix_nanos <= self.updated_at_unix_nanos {
            return Err(invalid(
                "CONSENTS_TIME_NOT_INCREASING",
                "consent_authorization.occurred_at_unix_nanos",
                "Consent Authorization mutation time must be strictly later than the previous mutation",
            ));
        }
        Ok(())
    }

    fn next_version(&self) -> Result<i64, SdkError> {
        self.version.checked_add(1).ok_or_else(|| {
            conflict(
                "CONSENTS_VERSION_EXHAUSTED",
                "Consent Authorization version cannot be advanced further.",
            )
        })
    }
}

fn validate_persisted_lifecycle(snapshot: &ConsentAuthorizationSnapshot) -> Result<(), SdkError> {
    if snapshot.version <= 0 {
        return Err(invalid(
            "CONSENTS_PERSISTED_VERSION_INVALID",
            "consent_authorization.version",
            "persisted Consent Authorization version must be positive",
        ));
    }

    match snapshot.status {
        ConsentAuthorizationStatus::Active => {
            if snapshot.version != 1
                || snapshot.withdrawn_at_unix_nanos.is_some()
                || snapshot.updated_at_unix_nanos != snapshot.created_at_unix_nanos
            {
                return Err(invalid(
                    "CONSENTS_PERSISTED_ACTIVE_STATE_INVALID",
                    "consent_authorization.status",
                    "v1 active Consent Authorization state must match the immutable create transition",
                ));
            }
        }
        ConsentAuthorizationStatus::Withdrawn => {
            let withdrawn_at = snapshot.withdrawn_at_unix_nanos.ok_or_else(|| {
                invalid(
                    "CONSENTS_PERSISTED_WITHDRAWAL_INVALID",
                    "consent_authorization.withdrawn_at_unix_nanos",
                    "withdrawn Consent Authorization state requires a withdrawal time",
                )
            })?;
            validate_timestamp(
                "consent_authorization.withdrawn_at_unix_nanos",
                withdrawn_at,
            )?;
            if snapshot.effect != ConsentEffect::Grant
                || snapshot.version != 2
                || withdrawn_at != snapshot.updated_at_unix_nanos
                || withdrawn_at <= snapshot.created_at_unix_nanos
                || snapshot
                    .expires_at_unix_nanos
                    .is_some_and(|expires_at| withdrawn_at >= expires_at)
            {
                return Err(invalid(
                    "CONSENTS_PERSISTED_WITHDRAWAL_INVALID",
                    "consent_authorization.status",
                    "persisted withdrawal state does not match the single v1 withdrawal transition",
                ));
            }
        }
    }
    Ok(())
}

fn validate_expiry(
    effective_from_unix_nanos: i64,
    expires_at_unix_nanos: Option<i64>,
) -> Result<(), SdkError> {
    if let Some(expires_at) = expires_at_unix_nanos {
        validate_timestamp("consent_authorization.expires_at_unix_nanos", expires_at)?;
        if expires_at <= effective_from_unix_nanos {
            return Err(invalid(
                "CONSENTS_VALIDITY_INVALID",
                "consent_authorization.expires_at_unix_nanos",
                "expiry time must be strictly later than effective time",
            ));
        }
    }
    Ok(())
}

fn validate_timestamp(field: &'static str, value: i64) -> Result<(), SdkError> {
    if value <= 0 {
        return Err(invalid(
            "CONSENTS_TIME_INVALID",
            field,
            "time must be a positive Unix-nanosecond value",
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
        || bytes
            .iter()
            .any(|byte| !(byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-')))
    {
        return Err(invalid(
            code,
            field,
            format!(
                "{label} must start and end with an ASCII letter or digit and contain only ASCII letters, digits, '.', '_' or '-'"
            ),
        ));
    }
    Ok(normalized)
}

fn invalid(
    code: &'static str,
    field: &'static str,
    internal: impl Into<String>,
) -> SdkError {
    SdkError::new(
        code,
        ErrorCategory::InvalidArgument,
        false,
        "The Consent Authorization data is invalid.",
    )
    .with_internal_reference(internal)
    .with_field_violation(FieldViolation {
        field: FieldName::try_new(field).expect("static Consent Authorization field must be valid"),
        code: "INVALID".to_owned(),
        safe_message: "The Consent Authorization field is invalid.".to_owned(),
    })
}

fn conflict(code: &'static str, internal: impl Into<String>) -> SdkError {
    SdkError::new(
        code,
        ErrorCategory::Conflict,
        false,
        "The Consent Authorization changed before this operation could be applied.",
    )
    .with_internal_reference(internal)
}

trait SdkErrorFieldViolationExt {
    fn with_field_violation(self, violation: FieldViolation) -> Self;
}

impl SdkErrorFieldViolationExt for SdkError {
    fn with_field_violation(mut self, violation: FieldViolation) -> Self {
        self.field_violations.push(violation);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grant(expires_at_unix_nanos: Option<i64>) -> ConsentAuthorization {
        ConsentAuthorization::create(CreateConsentAuthorization {
            authorization_id: ConsentAuthorizationId::try_new("consent-auth-1").unwrap(),
            party_ref: PartyReference::try_new("party-1").unwrap(),
            contact_point_ref: Some(ContactPointReference::try_new("contact-point-1").unwrap()),
            purpose: PurposeCode::try_new(" Marketing.Newsletter ").unwrap(),
            channel: CommunicationChannel::Email,
            effect: ConsentEffect::Grant,
            legal_basis: LegalBasisCode::try_new(" Consent ").unwrap(),
            jurisdiction: JurisdictionCode::try_new(" EU-LT ").unwrap(),
            source: SourceCode::try_new(" Web.Form ").unwrap(),
            evidence_ref: EvidenceReference::try_new(" evidence://consent/1 ").unwrap(),
            effective_from_unix_nanos: 100,
            expires_at_unix_nanos,
            occurred_at_unix_nanos: 100,
        })
        .unwrap()
    }

    #[test]
    fn create_canonicalizes_semantic_codes_and_preserves_evidence_case() {
        let value = grant(Some(1_000));
        assert_eq!(value.purpose().as_str(), "marketing.newsletter");
        assert_eq!(value.legal_basis().as_str(), "consent");
        assert_eq!(value.jurisdiction().as_str(), "eu-lt");
        assert_eq!(value.source().as_str(), "web.form");
        assert_eq!(value.evidence_ref().as_str(), "evidence://consent/1");
        assert_eq!(value.version(), 1);
        assert_eq!(value.status(), ConsentAuthorizationStatus::Active);
    }

    #[test]
    fn semantic_codes_reject_noncanonical_character_sets() {
        for value in ["", "-marketing", "marketing-", "marketing/newsletter"] {
            assert!(PurposeCode::try_new(value).is_err());
        }
    }

    #[test]
    fn create_rejects_effective_time_before_authoritative_creation_and_invalid_expiry() {
        let command = CreateConsentAuthorization {
            authorization_id: ConsentAuthorizationId::try_new("consent-auth-1").unwrap(),
            party_ref: PartyReference::try_new("party-1").unwrap(),
            contact_point_ref: None,
            purpose: PurposeCode::try_new("service.notice").unwrap(),
            channel: CommunicationChannel::Email,
            effect: ConsentEffect::Grant,
            legal_basis: LegalBasisCode::try_new("contract").unwrap(),
            jurisdiction: JurisdictionCode::try_new("eu").unwrap(),
            source: SourceCode::try_new("agent").unwrap(),
            evidence_ref: EvidenceReference::try_new("evidence-1").unwrap(),
            effective_from_unix_nanos: 99,
            expires_at_unix_nanos: Some(99),
            occurred_at_unix_nanos: 100,
        };
        assert_eq!(
            ConsentAuthorization::create(command).unwrap_err().code,
            "CONSENTS_EFFECTIVE_TIME_INVALID"
        );
    }

    #[test]
    fn withdraw_is_exact_versioned_irreversible_and_strictly_monotonic() {
        let mut value = grant(Some(1_000));
        assert_eq!(
            value
                .withdraw(WithdrawConsentAuthorization {
                    expected_version: 2,
                    occurred_at_unix_nanos: 200,
                })
                .unwrap_err()
                .code,
            "CONSENTS_VERSION_CONFLICT"
        );
        assert_eq!(value.version(), 1);
        assert_eq!(value.status(), ConsentAuthorizationStatus::Active);

        assert_eq!(
            value
                .withdraw(WithdrawConsentAuthorization {
                    expected_version: 1,
                    occurred_at_unix_nanos: 100,
                })
                .unwrap_err()
                .code,
            "CONSENTS_TIME_NOT_INCREASING"
        );

        value
            .withdraw(WithdrawConsentAuthorization {
                expected_version: 1,
                occurred_at_unix_nanos: 200,
            })
            .unwrap();
        assert_eq!(value.version(), 2);
        assert_eq!(value.status(), ConsentAuthorizationStatus::Withdrawn);
        assert_eq!(value.withdrawn_at_unix_nanos(), Some(200));

        assert_eq!(
            value
                .withdraw(WithdrawConsentAuthorization {
                    expected_version: 2,
                    occurred_at_unix_nanos: 300,
                })
                .unwrap_err()
                .code,
            "CONSENTS_ALREADY_WITHDRAWN"
        );
    }

    #[test]
    fn deny_assertion_cannot_use_grant_withdrawal_transition() {
        let mut value = ConsentAuthorization::create(CreateConsentAuthorization {
            effect: ConsentEffect::Deny,
            ..grant(None).snapshot().into_create_for_test(101)
        })
        .unwrap();
        assert_eq!(
            value
                .withdraw(WithdrawConsentAuthorization {
                    expected_version: 1,
                    occurred_at_unix_nanos: 200,
                })
                .unwrap_err()
                .code,
            "CONSENTS_DENY_WITHDRAWAL_INVALID"
        );
    }

    #[test]
    fn expired_grant_cannot_be_withdrawn() {
        let mut value = grant(Some(200));
        assert_eq!(
            value
                .withdraw(WithdrawConsentAuthorization {
                    expected_version: 1,
                    occurred_at_unix_nanos: 200,
                })
                .unwrap_err()
                .code,
            "CONSENTS_EXPIRED_GRANT_WITHDRAWAL_INVALID"
        );
        assert_eq!(value.version(), 1);
    }

    #[test]
    fn decision_points_enforce_effective_expiry_and_withdrawal_barrier() {
        let mut value = grant(Some(1_000));
        assert_eq!(value.decision_point_at(99), None);
        assert_eq!(
            value.decision_point_at(100),
            Some(ConsentDecisionPoint {
                occurred_at_unix_nanos: 100,
                effect: ConsentDecisionPointEffect::Grant,
            })
        );
        value
            .withdraw(WithdrawConsentAuthorization {
                expected_version: 1,
                occurred_at_unix_nanos: 200,
            })
            .unwrap();
        assert_eq!(
            value.decision_point_at(199),
            Some(ConsentDecisionPoint {
                occurred_at_unix_nanos: 100,
                effect: ConsentDecisionPointEffect::Grant,
            })
        );
        assert_eq!(
            value.decision_point_at(200),
            Some(ConsentDecisionPoint {
                occurred_at_unix_nanos: 200,
                effect: ConsentDecisionPointEffect::Withdrawal,
            })
        );
        assert_eq!(
            value.decision_point_at(5_000),
            Some(ConsentDecisionPoint {
                occurred_at_unix_nanos: 200,
                effect: ConsentDecisionPointEffect::Withdrawal,
            })
        );
    }

    #[test]
    fn active_assertion_disappears_after_expiry() {
        let value = grant(Some(200));
        assert!(value.decision_point_at(199).is_some());
        assert_eq!(value.decision_point_at(200), None);
    }

    #[test]
    fn version_overflow_failure_is_atomic() {
        let mut value = ConsentAuthorization::rehydrate(ConsentAuthorizationSnapshot {
            version: i64::MAX,
            status: ConsentAuthorizationStatus::Active,
            ..grant(None).snapshot()
        })
        .unwrap_err();
        assert_eq!(value.code, "CONSENTS_PERSISTED_ACTIVE_STATE_INVALID");
    }

    #[test]
    fn strict_rehydrate_rejects_impossible_v1_lifecycle_shapes() {
        let snapshot = grant(None).snapshot();
        let error = ConsentAuthorization::rehydrate(ConsentAuthorizationSnapshot {
            status: ConsentAuthorizationStatus::Withdrawn,
            withdrawn_at_unix_nanos: None,
            version: 2,
            updated_at_unix_nanos: 200,
            ..snapshot.clone()
        })
        .unwrap_err();
        assert_eq!(error.code, "CONSENTS_PERSISTED_WITHDRAWAL_INVALID");

        let error = ConsentAuthorization::rehydrate(ConsentAuthorizationSnapshot {
            version: 2,
            ..snapshot
        })
        .unwrap_err();
        assert_eq!(error.code, "CONSENTS_PERSISTED_ACTIVE_STATE_INVALID");
    }

    trait SnapshotTestExt {
        fn into_create_for_test(self, occurred_at_unix_nanos: i64) -> CreateConsentAuthorization;
    }

    impl SnapshotTestExt for ConsentAuthorizationSnapshot {
        fn into_create_for_test(self, occurred_at_unix_nanos: i64) -> CreateConsentAuthorization {
            CreateConsentAuthorization {
                authorization_id: self.authorization_id,
                party_ref: self.party_ref,
                contact_point_ref: self.contact_point_ref,
                purpose: self.purpose,
                channel: self.channel,
                effect: self.effect,
                legal_basis: self.legal_basis,
                jurisdiction: self.jurisdiction,
                source: self.source,
                evidence_ref: self.evidence_ref,
                effective_from_unix_nanos: self.effective_from_unix_nanos,
                expires_at_unix_nanos: self.expires_at_unix_nanos,
                occurred_at_unix_nanos,
            }
        }
    }
}
