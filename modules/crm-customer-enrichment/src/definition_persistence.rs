use crate::{
    MappingNormalization, MappingVersion, ProviderProfileVersion, RawPayloadPolicy, TargetField,
};
use crm_module_sdk::{ErrorCategory, SdkError};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};

pub const PROVIDER_PROFILE_VERSION_STATE_SCHEMA_ID: &str =
    "crm.customer-enrichment.provider_profile_version.state";
pub const MAPPING_VERSION_STATE_SCHEMA_ID: &str = "crm.customer-enrichment.mapping_version.state";
pub const DEFINITION_STATE_SCHEMA_VERSION: &str = "1.0.0";
pub const PROVIDER_PROFILE_VERSION_STATE_MAXIMUM_BYTES: u64 = 32 * 1024;
pub const MAPPING_VERSION_STATE_MAXIMUM_BYTES: u64 = 16 * 1024;
pub const DEFINITION_STATE_RETENTION_POLICY_ID: &str = "crm.customer_enrichment.definition";

const PROVIDER_PROFILE_ID_DOMAIN: &[u8] = b"crm.customer-enrichment.provider-profile-version/v1";
const MAPPING_VERSION_ID_DOMAIN: &[u8] = b"crm.customer-enrichment.mapping-version/v1";

const PROVIDER_PROFILE_STATE_DESCRIPTOR: &[u8] = b"crm.customer-enrichment.provider_profile_version.state/v1:version_id,provider_key,adapter_kind,adapter_contract_version,supported_target_fields,purpose_codes,license_id,permitted_use_class,residency_region,retention_days,raw_payload_policy,credential_handle_aliases,effective_at_unix_ms,expires_at_unix_ms";
const MAPPING_STATE_DESCRIPTOR: &[u8] = b"crm.customer-enrichment.mapping_version.state/v1:version_id,mapping_key,provider_profile_version_id,provider_response_field_path,target_field,normalization,maximum_suggestions_per_response,confidence_required";

pub fn provider_profile_version_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(PROVIDER_PROFILE_STATE_DESCRIPTOR).into()
}

pub fn mapping_version_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(MAPPING_STATE_DESCRIPTOR).into()
}

pub fn encode_provider_profile_version_state(
    value: &ProviderProfileVersion,
) -> Result<Vec<u8>, SdkError> {
    encode(
        value,
        PROVIDER_PROFILE_VERSION_STATE_MAXIMUM_BYTES,
        "provider profile version",
    )
}

pub fn decode_provider_profile_version_state(
    bytes: &[u8],
) -> Result<ProviderProfileVersion, SdkError> {
    decode::<ProviderProfileVersion, ProviderProfileVersionStateV1>(
        bytes,
        PROVIDER_PROFILE_VERSION_STATE_MAXIMUM_BYTES,
        "provider profile version",
        ProviderProfileVersionStateV1::validate,
        encode_provider_profile_version_state,
    )
}

pub fn encode_mapping_version_state(value: &MappingVersion) -> Result<Vec<u8>, SdkError> {
    encode(
        value,
        MAPPING_VERSION_STATE_MAXIMUM_BYTES,
        "mapping version",
    )
}

pub fn decode_mapping_version_state(bytes: &[u8]) -> Result<MappingVersion, SdkError> {
    decode::<MappingVersion, MappingVersionStateV1>(
        bytes,
        MAPPING_VERSION_STATE_MAXIMUM_BYTES,
        "mapping version",
        MappingVersionStateV1::validate,
        encode_mapping_version_state,
    )
}

fn encode<T: Serialize>(value: &T, maximum_bytes: u64, label: &str) -> Result<Vec<u8>, SdkError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| persisted_error(format!("{label} serialization failed: {error}")))?;
    validate_size(&bytes, maximum_bytes, label)?;
    Ok(bytes)
}

fn decode<T, S>(
    bytes: &[u8],
    maximum_bytes: u64,
    label: &str,
    validate: fn(&S) -> Result<(), SdkError>,
    encode_domain: fn(&T) -> Result<Vec<u8>, SdkError>,
) -> Result<T, SdkError>
where
    T: DeserializeOwned,
    S: DeserializeOwned,
{
    validate_size(bytes, maximum_bytes, label)?;
    let state: S = serde_json::from_slice(bytes)
        .map_err(|error| persisted_error(format!("{label} JSON is invalid: {error}")))?;
    validate(&state)?;
    let domain: T = serde_json::from_slice(bytes)
        .map_err(|error| persisted_error(format!("{label} domain state is invalid: {error}")))?;
    if encode_domain(&domain)? != bytes {
        return Err(persisted_error(format!(
            "persisted {label} is not the strict canonical v1 encoding"
        )));
    }
    Ok(domain)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderProfileVersionStateV1 {
    version_id: String,
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

#[derive(Serialize)]
struct ProviderProfileIdentityState<'a> {
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

impl ProviderProfileVersionStateV1 {
    fn validate(&self) -> Result<(), SdkError> {
        canonical_key(&self.provider_key, "provider key")?;
        canonical_key(&self.adapter_kind, "adapter kind")?;
        canonical_version(&self.adapter_contract_version, "adapter contract version")?;
        canonical_target_fields(&self.supported_target_fields)?;
        canonical_required_keys(&self.purpose_codes, 32, "purpose codes")?;
        canonical_policy_text(&self.license_id, 160, "license id")?;
        canonical_key(&self.permitted_use_class, "permitted use class")?;
        canonical_key(&self.residency_region, "residency region")?;
        if self.retention_days > 3_650 {
            return Err(persisted_error(
                "persisted provider retention exceeds the bounded policy maximum",
            ));
        }
        canonical_optional_keys(
            &self.credential_handle_aliases,
            8,
            "credential handle aliases",
        )?;
        if self
            .expires_at_unix_ms
            .is_some_and(|expiry| expiry <= self.effective_at_unix_ms)
        {
            return Err(persisted_error(
                "persisted provider profile expiry is not later than its effective timestamp",
            ));
        }
        let identity = ProviderProfileIdentityState {
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
        };
        validate_expected_id(
            &self.version_id,
            "enrichment-provider-profile-",
            PROVIDER_PROFILE_ID_DOMAIN,
            &identity,
            "provider profile version",
        )
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MappingVersionStateV1 {
    version_id: String,
    mapping_key: String,
    provider_profile_version_id: String,
    provider_response_field_path: String,
    target_field: TargetField,
    normalization: MappingNormalization,
    maximum_suggestions_per_response: u32,
    confidence_required: bool,
}

#[derive(Serialize)]
struct MappingIdentityState<'a> {
    semantic_version: &'static str,
    mapping_key: &'a str,
    provider_profile_version_id: &'a str,
    provider_response_field_path: &'a str,
    target_field: TargetField,
    normalization: MappingNormalization,
    maximum_suggestions_per_response: u32,
    confidence_required: bool,
}

impl MappingVersionStateV1 {
    fn validate(&self) -> Result<(), SdkError> {
        canonical_key(&self.mapping_key, "mapping key")?;
        validate_derived_id(
            &self.provider_profile_version_id,
            "enrichment-provider-profile-",
            "provider profile version",
        )?;
        canonical_provider_field_path(&self.provider_response_field_path)?;
        if !(1..=32).contains(&self.maximum_suggestions_per_response) {
            return Err(persisted_error(
                "persisted mapping suggestion count is outside the bounded range",
            ));
        }
        if !matches!(
            (self.target_field, self.normalization),
            (
                TargetField::PartyDisplayName,
                MappingNormalization::CanonicalPartyDisplayNameV1
            )
        ) {
            return Err(persisted_error(
                "persisted mapping normalization does not match its exact target field",
            ));
        }
        let identity = MappingIdentityState {
            semantic_version: "1.0.0",
            mapping_key: &self.mapping_key,
            provider_profile_version_id: &self.provider_profile_version_id,
            provider_response_field_path: &self.provider_response_field_path,
            target_field: self.target_field,
            normalization: self.normalization,
            maximum_suggestions_per_response: self.maximum_suggestions_per_response,
            confidence_required: self.confidence_required,
        };
        validate_expected_id(
            &self.version_id,
            "enrichment-mapping-",
            MAPPING_VERSION_ID_DOMAIN,
            &identity,
            "mapping version",
        )
    }
}

fn canonical_target_fields(values: &[TargetField]) -> Result<(), SdkError> {
    if values.is_empty() || values.len() > 8 {
        return Err(persisted_error(
            "persisted provider target fields must contain 1..=8 entries",
        ));
    }
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(persisted_error(
            "persisted provider target fields are not strict canonical order",
        ));
    }
    Ok(())
}

fn canonical_required_keys(
    values: &[String],
    maximum_items: usize,
    label: &str,
) -> Result<(), SdkError> {
    if values.is_empty() {
        return Err(persisted_error(format!(
            "persisted {label} must not be empty"
        )));
    }
    canonical_optional_keys(values, maximum_items, label)
}

fn canonical_optional_keys(
    values: &[String],
    maximum_items: usize,
    label: &str,
) -> Result<(), SdkError> {
    if values.len() > maximum_items {
        return Err(persisted_error(format!(
            "persisted {label} exceeds the bounded item count"
        )));
    }
    for value in values {
        canonical_key(value, label)?;
    }
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(persisted_error(format!(
            "persisted {label} is not unique canonical order"
        )));
    }
    Ok(())
}

fn canonical_key(value: &str, label: &str) -> Result<(), SdkError> {
    let valid = !value.is_empty()
        && value.len() <= 80
        && value.is_ascii()
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
        && value
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        && value
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_alphanumeric);
    if !valid {
        return Err(persisted_error(format!(
            "persisted {label} is not a canonical key"
        )));
    }
    Ok(())
}

fn canonical_version(value: &str, label: &str) -> Result<(), SdkError> {
    let valid = !value.is_empty()
        && value.len() <= 48
        && value.is_ascii()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'+'));
    if !valid {
        return Err(persisted_error(format!(
            "persisted {label} is not a canonical version"
        )));
    }
    Ok(())
}

fn canonical_policy_text(value: &str, maximum_bytes: usize, label: &str) -> Result<(), SdkError> {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized != value
        || value.is_empty()
        || value.len() > maximum_bytes
        || value.chars().any(char::is_control)
    {
        return Err(persisted_error(format!(
            "persisted {label} is not canonical bounded policy text"
        )));
    }
    Ok(())
}

fn canonical_provider_field_path(value: &str) -> Result<(), SdkError> {
    if value.is_empty()
        || value.len() > 160
        || !value.is_ascii()
        || value.chars().any(char::is_control)
        || value.bytes().any(|byte| byte.is_ascii_whitespace())
    {
        return Err(persisted_error(
            "persisted provider response field path is invalid",
        ));
    }
    Ok(())
}

fn validate_derived_id(value: &str, prefix: &str, label: &str) -> Result<(), SdkError> {
    let Some(digest) = value.strip_prefix(prefix) else {
        return Err(persisted_error(format!(
            "persisted {label} identity has the wrong prefix"
        )));
    };
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(persisted_error(format!(
            "persisted {label} identity is not a lowercase SHA-256 coordinate"
        )));
    }
    Ok(())
}

fn validate_expected_id<T: Serialize>(
    actual: &str,
    prefix: &str,
    domain: &[u8],
    identity: &T,
    label: &str,
) -> Result<(), SdkError> {
    validate_derived_id(actual, prefix, label)?;
    let expected = format!("{prefix}{}", hex(&canonical_digest(domain, identity)));
    if actual != expected {
        return Err(persisted_error(format!(
            "persisted {label} identity does not match its canonical content"
        )));
    }
    Ok(())
}

fn canonical_digest<T: Serialize>(domain: &[u8], value: &T) -> Vec<u8> {
    let encoded = serde_json::to_vec(value)
        .expect("canonical customer-enrichment definition identity must serialize");
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

fn validate_size(bytes: &[u8], maximum_bytes: u64, label: &str) -> Result<(), SdkError> {
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum_bytes {
        return Err(persisted_error(format!(
            "{label} state exceeds the maximum of {maximum_bytes} bytes"
        )));
    }
    Ok(())
}

fn persisted_error(message: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PERSISTED_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "The persisted customer-enrichment state is invalid.",
    )
    .with_internal_reference(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MappingDraft, ProviderProfileDraft};
    use serde_json::Value;

    fn definitions() -> (ProviderProfileVersion, MappingVersion) {
        let provider = ProviderProfileVersion::publish(ProviderProfileDraft {
            provider_key: "company_registry".to_owned(),
            adapter_kind: "registry_http_v1".to_owned(),
            adapter_contract_version: "1.0.0".to_owned(),
            supported_target_fields: vec![TargetField::PartyDisplayName],
            purpose_codes: vec!["customer_profile_enrichment".to_owned()],
            license_id: "Registry licence v3".to_owned(),
            permitted_use_class: "customer_master_review".to_owned(),
            residency_region: "eu".to_owned(),
            retention_days: 30,
            raw_payload_policy: RawPayloadPolicy::DigestOnly,
            credential_handle_aliases: vec!["registry_primary".to_owned()],
            effective_at_unix_ms: 1,
            expires_at_unix_ms: None,
        })
        .unwrap();
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
        (provider, mapping)
    }

    #[test]
    fn definition_states_round_trip_through_strict_canonical_encoding() {
        let (provider, mapping) = definitions();
        let provider_bytes = encode_provider_profile_version_state(&provider).unwrap();
        assert_eq!(
            decode_provider_profile_version_state(&provider_bytes).unwrap(),
            provider
        );
        let mapping_bytes = encode_mapping_version_state(&mapping).unwrap();
        assert_eq!(
            decode_mapping_version_state(&mapping_bytes).unwrap(),
            mapping
        );
    }

    #[test]
    fn changed_definition_identity_and_unknown_fields_are_rejected() {
        let (provider, mapping) = definitions();
        let mut provider_json: Value =
            serde_json::from_slice(&encode_provider_profile_version_state(&provider).unwrap())
                .unwrap();
        provider_json["retention_days"] = Value::from(31_u64);
        assert!(
            decode_provider_profile_version_state(&serde_json::to_vec(&provider_json).unwrap())
                .is_err()
        );

        let mut mapping_json: Value =
            serde_json::from_slice(&encode_mapping_version_state(&mapping).unwrap()).unwrap();
        mapping_json["unexpected"] = Value::Bool(true);
        assert!(decode_mapping_version_state(&serde_json::to_vec(&mapping_json).unwrap()).is_err());
    }

    #[test]
    fn definition_descriptor_hashes_are_stable_and_nonzero() {
        assert!(
            provider_profile_version_state_descriptor_hash()
                .iter()
                .any(|byte| *byte != 0)
        );
        assert!(
            mapping_version_state_descriptor_hash()
                .iter()
                .any(|byte| *byte != 0)
        );
    }
}
