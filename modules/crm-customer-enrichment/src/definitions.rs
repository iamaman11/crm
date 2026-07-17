use crm_module_sdk::{ErrorCategory, FieldName, FieldViolation, SdkError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const PROVIDER_PROFILE_ID_DOMAIN: &[u8] = b"crm.customer-enrichment.provider-profile-version/v1";
const MAPPING_VERSION_ID_DOMAIN: &[u8] = b"crm.customer-enrichment.mapping-version/v1";

const MAX_CANONICAL_KEY_BYTES: usize = 80;
const MAX_VERSION_BYTES: usize = 48;
const MAX_POLICY_TEXT_BYTES: usize = 160;
const MAX_PROVIDER_FIELD_PATH_BYTES: usize = 160;
const MAX_PURPOSE_CODES: usize = 32;
const MAX_CREDENTIAL_HANDLE_ALIASES: usize = 8;
const MAX_RETENTION_DAYS: u32 = 3_650;
const MAX_SUGGESTIONS_PER_RESPONSE: u32 = 32;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ProviderProfileVersionId(String);

impl ProviderProfileVersionId {
    fn from_digest(digest: &[u8]) -> Self {
        Self(format!("enrichment-provider-profile-{}", hex(digest)))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct MappingVersionId(String);

impl MappingVersionId {
    fn from_digest(digest: &[u8]) -> Self {
        Self(format!("enrichment-mapping-{}", hex(digest)))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetField {
    PartyDisplayName,
}

impl TargetField {
    pub const fn owner_module_id(self) -> &'static str {
        match self {
            Self::PartyDisplayName => "crm.parties",
        }
    }

    pub const fn resource_type(self) -> &'static str {
        match self {
            Self::PartyDisplayName => "parties.party",
        }
    }

    pub const fn field_name(self) -> &'static str {
        match self {
            Self::PartyDisplayName => "display_name",
        }
    }

    pub const fn owner_capability_id(self) -> &'static str {
        match self {
            Self::PartyDisplayName => "parties.party.update",
        }
    }

    pub const fn owner_capability_version(self) -> &'static str {
        "1.0.0"
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RawPayloadPolicy {
    DigestOnly,
    GovernedProtectedEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MappingNormalization {
    CanonicalPartyDisplayNameV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderProfileDraft {
    pub provider_key: String,
    pub adapter_kind: String,
    pub adapter_contract_version: String,
    pub supported_target_fields: Vec<TargetField>,
    pub purpose_codes: Vec<String>,
    pub license_id: String,
    pub permitted_use_class: String,
    pub residency_region: String,
    pub retention_days: u32,
    pub raw_payload_policy: RawPayloadPolicy,
    pub credential_handle_aliases: Vec<String>,
    pub effective_at_unix_ms: u64,
    pub expires_at_unix_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderProfileVersion {
    version_id: ProviderProfileVersionId,
    provider_key: String,
    adapter_kind: String,
    adapter_contract_version: String,
    supported_target_fields: Vec<TargetField>,
    purpose_codes: Vec<String>,
    license_id: String,
    permitted_use_class: String,
    residency_region: String,
    retention_days: u32,
    raw_payload_policy: RawPayloadPolicy,
    credential_handle_aliases: Vec<String>,
    effective_at_unix_ms: u64,
    expires_at_unix_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ProviderProfileIdentity<'a> {
    semantic_version: &'static str,
    provider_key: &'a str,
    adapter_kind: &'a str,
    adapter_contract_version: &'a str,
    supported_target_fields: &'a [TargetField],
    purpose_codes: &'a [String],
    license_id: &'a str,
    permitted_use_class: &'a str,
    residency_region: &'a str,
    retention_days: u32,
    raw_payload_policy: RawPayloadPolicy,
    credential_handle_aliases: &'a [String],
    effective_at_unix_ms: u64,
    expires_at_unix_ms: Option<u64>,
}

impl ProviderProfileVersion {
    pub fn publish(draft: ProviderProfileDraft) -> Result<Self, SdkError> {
        let provider_key = canonical_key(draft.provider_key, "provider_profile.provider_key")?;
        let adapter_kind = canonical_key(draft.adapter_kind, "provider_profile.adapter_kind")?;
        let adapter_contract_version = canonical_version(
            draft.adapter_contract_version,
            "provider_profile.adapter_contract_version",
        )?;
        let supported_target_fields = canonical_target_fields(draft.supported_target_fields)?;
        let purpose_codes = canonical_keys(
            draft.purpose_codes,
            MAX_PURPOSE_CODES,
            "provider_profile.purpose_codes",
        )?;
        let license_id = canonical_policy_text(
            draft.license_id,
            "provider_profile.license_id",
            "CUSTOMER_ENRICHMENT_LICENSE_ID_INVALID",
        )?;
        let permitted_use_class = canonical_key(
            draft.permitted_use_class,
            "provider_profile.permitted_use_class",
        )?;
        let residency_region =
            canonical_key(draft.residency_region, "provider_profile.residency_region")?;
        if draft.retention_days > MAX_RETENTION_DAYS {
            return Err(invalid(
                "CUSTOMER_ENRICHMENT_RETENTION_INVALID",
                "provider_profile.retention_days",
                format!("retention days must be in the inclusive range 0..={MAX_RETENTION_DAYS}"),
            ));
        }
        let credential_handle_aliases = canonical_optional_keys(
            draft.credential_handle_aliases,
            MAX_CREDENTIAL_HANDLE_ALIASES,
            "provider_profile.credential_handle_aliases",
        )?;
        if draft
            .expires_at_unix_ms
            .is_some_and(|expiry| expiry <= draft.effective_at_unix_ms)
        {
            return Err(invalid(
                "CUSTOMER_ENRICHMENT_EFFECTIVE_WINDOW_INVALID",
                "provider_profile.expires_at_unix_ms",
                "provider profile expiry must be later than its effective timestamp",
            ));
        }

        let mut profile = Self {
            version_id: ProviderProfileVersionId(String::new()),
            provider_key,
            adapter_kind,
            adapter_contract_version,
            supported_target_fields,
            purpose_codes,
            license_id,
            permitted_use_class,
            residency_region,
            retention_days: draft.retention_days,
            raw_payload_policy: draft.raw_payload_policy,
            credential_handle_aliases,
            effective_at_unix_ms: draft.effective_at_unix_ms,
            expires_at_unix_ms: draft.expires_at_unix_ms,
        };
        profile.version_id = ProviderProfileVersionId::from_digest(&canonical_digest(
            PROVIDER_PROFILE_ID_DOMAIN,
            &profile.identity(),
        ));
        Ok(profile)
    }

    fn identity(&self) -> ProviderProfileIdentity<'_> {
        ProviderProfileIdentity {
            semantic_version: "1.0.0",
            provider_key: &self.provider_key,
            adapter_kind: &self.adapter_kind,
            adapter_contract_version: &self.adapter_contract_version,
            supported_target_fields: &self.supported_target_fields,
            purpose_codes: &self.purpose_codes,
            license_id: &self.license_id,
            permitted_use_class: &self.permitted_use_class,
            residency_region: &self.residency_region,
            retention_days: self.retention_days,
            raw_payload_policy: self.raw_payload_policy,
            credential_handle_aliases: &self.credential_handle_aliases,
            effective_at_unix_ms: self.effective_at_unix_ms,
            expires_at_unix_ms: self.expires_at_unix_ms,
        }
    }

    pub fn version_id(&self) -> &ProviderProfileVersionId {
        &self.version_id
    }

    pub fn provider_key(&self) -> &str {
        self.provider_key.as_str()
    }

    pub fn adapter_kind(&self) -> &str {
        self.adapter_kind.as_str()
    }

    pub fn adapter_contract_version(&self) -> &str {
        self.adapter_contract_version.as_str()
    }

    pub fn supported_target_fields(&self) -> &[TargetField] {
        &self.supported_target_fields
    }

    pub fn purpose_codes(&self) -> &[String] {
        &self.purpose_codes
    }

    pub fn license_id(&self) -> &str {
        self.license_id.as_str()
    }

    pub fn permitted_use_class(&self) -> &str {
        self.permitted_use_class.as_str()
    }

    pub fn residency_region(&self) -> &str {
        self.residency_region.as_str()
    }

    pub const fn retention_days(&self) -> u32 {
        self.retention_days
    }

    pub const fn raw_payload_policy(&self) -> RawPayloadPolicy {
        self.raw_payload_policy
    }

    pub fn credential_handle_aliases(&self) -> &[String] {
        &self.credential_handle_aliases
    }

    pub const fn effective_at_unix_ms(&self) -> u64 {
        self.effective_at_unix_ms
    }

    pub const fn expires_at_unix_ms(&self) -> Option<u64> {
        self.expires_at_unix_ms
    }

    pub fn is_effective_at(&self, timestamp_unix_ms: u64) -> bool {
        timestamp_unix_ms >= self.effective_at_unix_ms
            && self
                .expires_at_unix_ms
                .is_none_or(|expiry| timestamp_unix_ms < expiry)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappingDraft {
    pub mapping_key: String,
    pub provider_profile_version_id: ProviderProfileVersionId,
    pub provider_response_field_path: String,
    pub target_field: TargetField,
    pub normalization: MappingNormalization,
    pub maximum_suggestions_per_response: u32,
    pub confidence_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MappingVersion {
    version_id: MappingVersionId,
    mapping_key: String,
    provider_profile_version_id: ProviderProfileVersionId,
    provider_response_field_path: String,
    target_field: TargetField,
    normalization: MappingNormalization,
    maximum_suggestions_per_response: u32,
    confidence_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct MappingIdentity<'a> {
    semantic_version: &'static str,
    mapping_key: &'a str,
    provider_profile_version_id: &'a ProviderProfileVersionId,
    provider_response_field_path: &'a str,
    target_field: TargetField,
    normalization: MappingNormalization,
    maximum_suggestions_per_response: u32,
    confidence_required: bool,
}

impl MappingVersion {
    pub fn publish(draft: MappingDraft) -> Result<Self, SdkError> {
        let mapping_key = canonical_key(draft.mapping_key, "mapping.mapping_key")?;
        let provider_response_field_path = canonical_provider_field_path(
            draft.provider_response_field_path,
            "mapping.provider_response_field_path",
        )?;
        if !(1..=MAX_SUGGESTIONS_PER_RESPONSE).contains(&draft.maximum_suggestions_per_response) {
            return Err(invalid(
                "CUSTOMER_ENRICHMENT_MAXIMUM_SUGGESTIONS_INVALID",
                "mapping.maximum_suggestions_per_response",
                format!(
                    "maximum suggestions per response must be in the inclusive range 1..={MAX_SUGGESTIONS_PER_RESPONSE}"
                ),
            ));
        }
        if !matches!(
            (draft.target_field, draft.normalization),
            (
                TargetField::PartyDisplayName,
                MappingNormalization::CanonicalPartyDisplayNameV1
            )
        ) {
            return Err(invalid(
                "CUSTOMER_ENRICHMENT_MAPPING_NORMALIZATION_INVALID",
                "mapping.normalization",
                "the mapping normalization must match the exact supported target field",
            ));
        }

        let mut mapping = Self {
            version_id: MappingVersionId(String::new()),
            mapping_key,
            provider_profile_version_id: draft.provider_profile_version_id,
            provider_response_field_path,
            target_field: draft.target_field,
            normalization: draft.normalization,
            maximum_suggestions_per_response: draft.maximum_suggestions_per_response,
            confidence_required: draft.confidence_required,
        };
        mapping.version_id = MappingVersionId::from_digest(&canonical_digest(
            MAPPING_VERSION_ID_DOMAIN,
            &mapping.identity(),
        ));
        Ok(mapping)
    }

    fn identity(&self) -> MappingIdentity<'_> {
        MappingIdentity {
            semantic_version: "1.0.0",
            mapping_key: &self.mapping_key,
            provider_profile_version_id: &self.provider_profile_version_id,
            provider_response_field_path: &self.provider_response_field_path,
            target_field: self.target_field,
            normalization: self.normalization,
            maximum_suggestions_per_response: self.maximum_suggestions_per_response,
            confidence_required: self.confidence_required,
        }
    }

    pub fn version_id(&self) -> &MappingVersionId {
        &self.version_id
    }

    pub fn mapping_key(&self) -> &str {
        self.mapping_key.as_str()
    }

    pub fn provider_profile_version_id(&self) -> &ProviderProfileVersionId {
        &self.provider_profile_version_id
    }

    pub fn provider_response_field_path(&self) -> &str {
        self.provider_response_field_path.as_str()
    }

    pub const fn target_field(&self) -> TargetField {
        self.target_field
    }

    pub const fn normalization(&self) -> MappingNormalization {
        self.normalization
    }

    pub const fn maximum_suggestions_per_response(&self) -> u32 {
        self.maximum_suggestions_per_response
    }

    pub const fn confidence_required(&self) -> bool {
        self.confidence_required
    }
}

fn canonical_target_fields(mut values: Vec<TargetField>) -> Result<Vec<TargetField>, SdkError> {
    if values.is_empty() || values.len() > 8 {
        return Err(invalid(
            "CUSTOMER_ENRICHMENT_TARGET_FIELDS_INVALID",
            "provider_profile.supported_target_fields",
            "provider profile target fields must contain 1..=8 entries",
        ));
    }
    values.sort();
    if values.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(invalid(
            "CUSTOMER_ENRICHMENT_TARGET_FIELDS_DUPLICATE",
            "provider_profile.supported_target_fields",
            "provider profile target fields must be unique",
        ));
    }
    Ok(values)
}

fn canonical_keys(
    values: Vec<String>,
    maximum_items: usize,
    field: &'static str,
) -> Result<Vec<String>, SdkError> {
    if values.is_empty() || values.len() > maximum_items {
        return Err(invalid(
            "CUSTOMER_ENRICHMENT_KEY_SET_INVALID",
            field,
            format!("key set must contain 1..={maximum_items} entries"),
        ));
    }
    canonical_optional_keys(values, maximum_items, field)
}

fn canonical_optional_keys(
    values: Vec<String>,
    maximum_items: usize,
    field: &'static str,
) -> Result<Vec<String>, SdkError> {
    if values.len() > maximum_items {
        return Err(invalid(
            "CUSTOMER_ENRICHMENT_KEY_SET_INVALID",
            field,
            format!("key set must contain at most {maximum_items} entries"),
        ));
    }
    let mut canonical = values
        .into_iter()
        .map(|value| canonical_key(value, field))
        .collect::<Result<Vec<_>, _>>()?;
    canonical.sort();
    if canonical.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(invalid(
            "CUSTOMER_ENRICHMENT_KEY_SET_DUPLICATE",
            field,
            "key set entries must be unique after canonicalization",
        ));
    }
    Ok(canonical)
}

fn canonical_key(value: String, field: &'static str) -> Result<String, SdkError> {
    if value.is_empty()
        || value.len() > MAX_CANONICAL_KEY_BYTES
        || !value.is_ascii()
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
        || !value
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        || !value
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_alphanumeric)
    {
        return Err(invalid(
            "CUSTOMER_ENRICHMENT_CANONICAL_KEY_INVALID",
            field,
            "canonical keys must be 1..=80 ASCII bytes, use lowercase letters/digits/._-, and start/end with an alphanumeric character",
        ));
    }
    Ok(value)
}

fn canonical_version(value: String, field: &'static str) -> Result<String, SdkError> {
    if value.is_empty()
        || value.len() > MAX_VERSION_BYTES
        || !value.is_ascii()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'+'))
    {
        return Err(invalid(
            "CUSTOMER_ENRICHMENT_VERSION_INVALID",
            field,
            "version must be 1..=48 ASCII alphanumeric/dot/hyphen/plus bytes",
        ));
    }
    Ok(value)
}

fn canonical_policy_text(
    value: String,
    field: &'static str,
    code: &'static str,
) -> Result<String, SdkError> {
    if value.chars().any(char::is_control) {
        return Err(invalid(
            code,
            field,
            "policy text must not contain control characters",
        ));
    }
    let canonical = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if canonical.is_empty() || canonical.len() > MAX_POLICY_TEXT_BYTES {
        return Err(invalid(
            code,
            field,
            format!(
                "policy text must contain 1..={MAX_POLICY_TEXT_BYTES} UTF-8 bytes after normalization"
            ),
        ));
    }
    Ok(canonical)
}

fn canonical_provider_field_path(value: String, field: &'static str) -> Result<String, SdkError> {
    if value.is_empty()
        || value.len() > MAX_PROVIDER_FIELD_PATH_BYTES
        || !value.is_ascii()
        || value.chars().any(char::is_control)
        || value.bytes().any(u8::is_ascii_whitespace)
    {
        return Err(invalid(
            "CUSTOMER_ENRICHMENT_PROVIDER_FIELD_PATH_INVALID",
            field,
            "provider field path must be 1..=160 printable non-whitespace ASCII bytes",
        ));
    }
    Ok(value)
}

fn canonical_digest<T: Serialize>(domain: &[u8], value: &T) -> Vec<u8> {
    let encoded =
        serde_json::to_vec(value).expect("canonical customer-enrichment definition must serialize");
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((encoded.len() as u64).to_be_bytes());
    hasher.update(encoded);
    hasher.finalize().to_vec()
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn invalid(code: &'static str, field: &'static str, safe_message: impl Into<String>) -> SdkError {
    let safe_message = safe_message.into();
    let mut error = SdkError::new(
        code,
        ErrorCategory::InvalidArgument,
        false,
        "The customer-enrichment definition is invalid.",
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

    fn draft() -> ProviderProfileDraft {
        ProviderProfileDraft {
            provider_key: "company_registry".to_owned(),
            adapter_kind: "registry_http_v1".to_owned(),
            adapter_contract_version: "1.0.0".to_owned(),
            supported_target_fields: vec![TargetField::PartyDisplayName],
            purpose_codes: vec![
                "customer_profile_enrichment".to_owned(),
                "due_diligence".to_owned(),
            ],
            license_id: "Registry commercial data licence v3".to_owned(),
            permitted_use_class: "customer_master_review".to_owned(),
            residency_region: "eu".to_owned(),
            retention_days: 30,
            raw_payload_policy: RawPayloadPolicy::DigestOnly,
            credential_handle_aliases: vec!["registry_primary".to_owned()],
            effective_at_unix_ms: 1_000,
            expires_at_unix_ms: Some(2_000),
        }
    }

    #[test]
    fn provider_profile_identity_is_independent_of_set_order() {
        let first = ProviderProfileVersion::publish(draft()).unwrap();
        let mut reordered = draft();
        reordered.purpose_codes.reverse();
        let second = ProviderProfileVersion::publish(reordered).unwrap();
        assert_eq!(first.version_id(), second.version_id());
        assert_eq!(
            first.purpose_codes(),
            &[
                "customer_profile_enrichment".to_owned(),
                "due_diligence".to_owned()
            ]
        );
    }

    #[test]
    fn changed_use_policy_changes_immutable_profile_identity() {
        let first = ProviderProfileVersion::publish(draft()).unwrap();
        let mut changed = draft();
        changed.retention_days = 31;
        let second = ProviderProfileVersion::publish(changed).unwrap();
        assert_ne!(first.version_id(), second.version_id());
    }

    #[test]
    fn duplicate_or_secret_like_aliases_are_rejected() {
        let mut duplicate = draft();
        duplicate.credential_handle_aliases =
            vec!["registry_primary".to_owned(), "registry_primary".to_owned()];
        assert!(ProviderProfileVersion::publish(duplicate).is_err());

        let mut secret_like = draft();
        secret_like.credential_handle_aliases = vec!["token=top-secret".to_owned()];
        assert!(ProviderProfileVersion::publish(secret_like).is_err());
    }

    #[test]
    fn provider_profile_effective_window_is_half_open() {
        let profile = ProviderProfileVersion::publish(draft()).unwrap();
        assert!(!profile.is_effective_at(999));
        assert!(profile.is_effective_at(1_000));
        assert!(profile.is_effective_at(1_999));
        assert!(!profile.is_effective_at(2_000));

        let mut invalid_window = draft();
        invalid_window.expires_at_unix_ms = Some(1_000);
        assert!(ProviderProfileVersion::publish(invalid_window).is_err());
    }

    #[test]
    fn mapping_identity_binds_exact_provider_and_target_semantics() {
        let provider = ProviderProfileVersion::publish(draft()).unwrap();
        let mapping = MappingVersion::publish(MappingDraft {
            mapping_key: "party_display_name".to_owned(),
            provider_profile_version_id: provider.version_id().clone(),
            provider_response_field_path: "organization.legal_name".to_owned(),
            target_field: TargetField::PartyDisplayName,
            normalization: MappingNormalization::CanonicalPartyDisplayNameV1,
            maximum_suggestions_per_response: 1,
            confidence_required: true,
        })
        .unwrap();

        assert_eq!(mapping.target_field().owner_module_id(), "crm.parties");
        assert_eq!(
            mapping.target_field().owner_capability_id(),
            "parties.party.update"
        );
        assert_eq!(mapping.target_field().owner_capability_version(), "1.0.0");

        let changed = MappingVersion::publish(MappingDraft {
            mapping_key: "party_display_name".to_owned(),
            provider_profile_version_id: provider.version_id().clone(),
            provider_response_field_path: "organization.trade_name".to_owned(),
            target_field: TargetField::PartyDisplayName,
            normalization: MappingNormalization::CanonicalPartyDisplayNameV1,
            maximum_suggestions_per_response: 1,
            confidence_required: true,
        })
        .unwrap();
        assert_ne!(mapping.version_id(), changed.version_id());
    }

    #[test]
    fn mapping_limits_are_bounded() {
        let provider = ProviderProfileVersion::publish(draft()).unwrap();
        let invalid = MappingVersion::publish(MappingDraft {
            mapping_key: "party_display_name".to_owned(),
            provider_profile_version_id: provider.version_id().clone(),
            provider_response_field_path: "organization.legal_name".to_owned(),
            target_field: TargetField::PartyDisplayName,
            normalization: MappingNormalization::CanonicalPartyDisplayNameV1,
            maximum_suggestions_per_response: 0,
            confidence_required: true,
        });
        assert!(invalid.is_err());
    }
}
