use crate::domain::{
    ComponentKey, DisplayNameMinUtf8Bytes, DisplayNamePlaceholderExactAsciiCasefold,
    PartyCompletenessComponent, PartyCompletenessProfileVersion, PartyQualityEvaluator,
    PartyQualityRule, PartyRuleSetVersion, QualitySeverity, RuleKey,
};
use crm_module_sdk::{ErrorCategory, SdkError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PARTY_RULE_SET_VERSION_STATE_SCHEMA_ID: &str =
    "crm.data-quality.party_rule_set_version.state";
pub const PARTY_RULE_SET_VERSION_STATE_SCHEMA_VERSION: &str = "1.0.0";
pub const PARTY_RULE_SET_VERSION_STATE_MAXIMUM_BYTES: u64 = 256 * 1024;
pub const PARTY_RULE_SET_VERSION_STATE_RETENTION_POLICY_ID: &str =
    "crm.data_quality.definition";

pub const PARTY_COMPLETENESS_PROFILE_VERSION_STATE_SCHEMA_ID: &str =
    "crm.data-quality.party_completeness_profile_version.state";
pub const PARTY_COMPLETENESS_PROFILE_VERSION_STATE_SCHEMA_VERSION: &str = "1.0.0";
pub const PARTY_COMPLETENESS_PROFILE_VERSION_STATE_MAXIMUM_BYTES: u64 = 128 * 1024;
pub const PARTY_COMPLETENESS_PROFILE_VERSION_STATE_RETENTION_POLICY_ID: &str =
    "crm.data_quality.definition";

const PARTY_RULE_SET_VERSION_STATE_DESCRIPTOR: &[u8] = b"crm.data-quality.party_rule_set_version.state/v1:version_id,rules[rule_key,severity,evaluator[kind,parameters],title,remediation_guidance]";
const PARTY_COMPLETENESS_PROFILE_VERSION_STATE_DESCRIPTOR: &[u8] = b"crm.data-quality.party_completeness_profile_version.state/v1:version_id,rule_set_version_id,components[component_key,rule_key,weight_basis_points]";

pub fn party_rule_set_version_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(PARTY_RULE_SET_VERSION_STATE_DESCRIPTOR).into()
}

pub fn party_completeness_profile_version_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(PARTY_COMPLETENESS_PROFILE_VERSION_STATE_DESCRIPTOR).into()
}

pub fn encode_party_rule_set_version_state(
    rule_set: &PartyRuleSetVersion,
) -> Result<Vec<u8>, SdkError> {
    let bytes = serde_json::to_vec(&PartyRuleSetVersionStateV1::from(rule_set)).map_err(|error| {
        persisted_error(format!(
            "Party rule-set version state serialization failed: {error}"
        ))
    })?;
    validate_size(
        &bytes,
        PARTY_RULE_SET_VERSION_STATE_MAXIMUM_BYTES,
        "Party rule-set version",
    )?;
    Ok(bytes)
}

pub fn decode_party_rule_set_version_state(bytes: &[u8]) -> Result<PartyRuleSetVersion, SdkError> {
    validate_size(
        bytes,
        PARTY_RULE_SET_VERSION_STATE_MAXIMUM_BYTES,
        "Party rule-set version",
    )?;
    let state: PartyRuleSetVersionStateV1 = serde_json::from_slice(bytes).map_err(|error| {
        persisted_error(format!(
            "Party rule-set version state JSON is invalid: {error}"
        ))
    })?;
    let stored_version_id = state.version_id.clone();
    let rules = state
        .rules
        .into_iter()
        .map(PartyQualityRuleStateV1::into_domain)
        .collect::<Result<Vec<_>, _>>()?;
    let rule_set = PartyRuleSetVersion::publish(rules)
        .map_err(|error| persisted_domain_error("Party rule-set version", error))?;
    if rule_set.version_id().as_str() != stored_version_id {
        return Err(persisted_error(
            "persisted Party rule-set version identity does not match canonical content",
        ));
    }
    let canonical = encode_party_rule_set_version_state(&rule_set)?;
    if canonical != bytes {
        return Err(persisted_error(
            "persisted Party rule-set version is not the strict canonical v1 encoding",
        ));
    }
    Ok(rule_set)
}

pub fn encode_party_completeness_profile_version_state(
    profile: &PartyCompletenessProfileVersion,
) -> Result<Vec<u8>, SdkError> {
    let bytes = serde_json::to_vec(&PartyCompletenessProfileVersionStateV1::from(profile)).map_err(
        |error| {
            persisted_error(format!(
                "Party completeness-profile version state serialization failed: {error}"
            ))
        },
    )?;
    validate_size(
        &bytes,
        PARTY_COMPLETENESS_PROFILE_VERSION_STATE_MAXIMUM_BYTES,
        "Party completeness-profile version",
    )?;
    Ok(bytes)
}

pub fn decode_party_completeness_profile_version_state(
    bytes: &[u8],
    rule_set: &PartyRuleSetVersion,
) -> Result<PartyCompletenessProfileVersion, SdkError> {
    validate_size(
        bytes,
        PARTY_COMPLETENESS_PROFILE_VERSION_STATE_MAXIMUM_BYTES,
        "Party completeness-profile version",
    )?;
    let state: PartyCompletenessProfileVersionStateV1 =
        serde_json::from_slice(bytes).map_err(|error| {
            persisted_error(format!(
                "Party completeness-profile version state JSON is invalid: {error}"
            ))
        })?;
    if state.rule_set_version_id != rule_set.version_id().as_str() {
        return Err(persisted_error(
            "persisted Party completeness profile references a different rule-set version",
        ));
    }
    let stored_version_id = state.version_id.clone();
    let components = state
        .components
        .into_iter()
        .map(PartyCompletenessComponentStateV1::into_domain)
        .collect::<Result<Vec<_>, _>>()?;
    let profile = PartyCompletenessProfileVersion::publish(rule_set, components)
        .map_err(|error| persisted_domain_error("Party completeness-profile version", error))?;
    if profile.version_id().as_str() != stored_version_id {
        return Err(persisted_error(
            "persisted Party completeness-profile version identity does not match canonical content",
        ));
    }
    let canonical = encode_party_completeness_profile_version_state(&profile)?;
    if canonical != bytes {
        return Err(persisted_error(
            "persisted Party completeness-profile version is not the strict canonical v1 encoding",
        ));
    }
    Ok(profile)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartyRuleSetVersionStateV1 {
    version_id: String,
    rules: Vec<PartyQualityRuleStateV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartyQualityRuleStateV1 {
    rule_key: String,
    severity: QualitySeverityState,
    evaluator: PartyQualityEvaluatorStateV1,
    title: String,
    remediation_guidance: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum QualitySeverityState {
    Info,
    Warning,
    Error,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "parameters", rename_all = "snake_case")]
enum PartyQualityEvaluatorStateV1 {
    DisplayNameMinUtf8Bytes {
        minimum_utf8_bytes: u32,
    },
    DisplayNamePlaceholderExactAsciiCasefold {
        placeholder_tokens: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartyCompletenessProfileVersionStateV1 {
    version_id: String,
    rule_set_version_id: String,
    components: Vec<PartyCompletenessComponentStateV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartyCompletenessComponentStateV1 {
    component_key: String,
    rule_key: String,
    weight_basis_points: u32,
}

impl From<&PartyRuleSetVersion> for PartyRuleSetVersionStateV1 {
    fn from(rule_set: &PartyRuleSetVersion) -> Self {
        Self {
            version_id: rule_set.version_id().as_str().to_owned(),
            rules: rule_set
                .rules()
                .iter()
                .map(PartyQualityRuleStateV1::from)
                .collect(),
        }
    }
}

impl From<&PartyQualityRule> for PartyQualityRuleStateV1 {
    fn from(rule: &PartyQualityRule) -> Self {
        Self {
            rule_key: rule.rule_key().as_str().to_owned(),
            severity: rule.severity().into(),
            evaluator: rule.evaluator().into(),
            title: rule.title().to_owned(),
            remediation_guidance: rule.remediation_guidance().to_owned(),
        }
    }
}

impl PartyQualityRuleStateV1 {
    fn into_domain(self) -> Result<PartyQualityRule, SdkError> {
        let rule_key = RuleKey::try_new(self.rule_key.clone())
            .map_err(|error| persisted_domain_error("rule key", error))?;
        if rule_key.as_str() != self.rule_key {
            return Err(persisted_error("persisted rule key is not canonical"));
        }
        PartyQualityRule::try_new(
            rule_key,
            self.severity.into(),
            self.evaluator.into_domain()?,
            self.title.clone(),
            self.remediation_guidance.clone(),
        )
        .map_err(|error| persisted_domain_error("Party quality rule", error))
    }
}

impl From<QualitySeverity> for QualitySeverityState {
    fn from(value: QualitySeverity) -> Self {
        match value {
            QualitySeverity::Info => Self::Info,
            QualitySeverity::Warning => Self::Warning,
            QualitySeverity::Error => Self::Error,
            QualitySeverity::Critical => Self::Critical,
        }
    }
}

impl From<QualitySeverityState> for QualitySeverity {
    fn from(value: QualitySeverityState) -> Self {
        match value {
            QualitySeverityState::Info => Self::Info,
            QualitySeverityState::Warning => Self::Warning,
            QualitySeverityState::Error => Self::Error,
            QualitySeverityState::Critical => Self::Critical,
        }
    }
}

impl From<&PartyQualityEvaluator> for PartyQualityEvaluatorStateV1 {
    fn from(value: &PartyQualityEvaluator) -> Self {
        match value {
            PartyQualityEvaluator::DisplayNameMinUtf8Bytes(parameters) => {
                Self::DisplayNameMinUtf8Bytes {
                    minimum_utf8_bytes: parameters.minimum_utf8_bytes(),
                }
            }
            PartyQualityEvaluator::DisplayNamePlaceholderExactAsciiCasefold(parameters) => {
                Self::DisplayNamePlaceholderExactAsciiCasefold {
                    placeholder_tokens: parameters.placeholder_tokens().to_vec(),
                }
            }
        }
    }
}

impl PartyQualityEvaluatorStateV1 {
    fn into_domain(self) -> Result<PartyQualityEvaluator, SdkError> {
        match self {
            Self::DisplayNameMinUtf8Bytes { minimum_utf8_bytes } => {
                DisplayNameMinUtf8Bytes::try_new(minimum_utf8_bytes)
                    .map(PartyQualityEvaluator::DisplayNameMinUtf8Bytes)
                    .map_err(|error| persisted_domain_error("minimum UTF-8 evaluator", error))
            }
            Self::DisplayNamePlaceholderExactAsciiCasefold { placeholder_tokens } => {
                DisplayNamePlaceholderExactAsciiCasefold::try_new(placeholder_tokens)
                    .map(PartyQualityEvaluator::DisplayNamePlaceholderExactAsciiCasefold)
                    .map_err(|error| persisted_domain_error("placeholder evaluator", error))
            }
        }
    }
}

impl From<&PartyCompletenessProfileVersion> for PartyCompletenessProfileVersionStateV1 {
    fn from(profile: &PartyCompletenessProfileVersion) -> Self {
        Self {
            version_id: profile.version_id().as_str().to_owned(),
            rule_set_version_id: profile.rule_set_version_id().as_str().to_owned(),
            components: profile
                .components()
                .iter()
                .map(PartyCompletenessComponentStateV1::from)
                .collect(),
        }
    }
}

impl From<&PartyCompletenessComponent> for PartyCompletenessComponentStateV1 {
    fn from(component: &PartyCompletenessComponent) -> Self {
        Self {
            component_key: component.component_key().as_str().to_owned(),
            rule_key: component.rule_key().as_str().to_owned(),
            weight_basis_points: component.weight_basis_points(),
        }
    }
}

impl PartyCompletenessComponentStateV1 {
    fn into_domain(self) -> Result<PartyCompletenessComponent, SdkError> {
        let component_key = ComponentKey::try_new(self.component_key.clone())
            .map_err(|error| persisted_domain_error("completeness component key", error))?;
        let rule_key = RuleKey::try_new(self.rule_key.clone())
            .map_err(|error| persisted_domain_error("completeness rule key", error))?;
        if component_key.as_str() != self.component_key || rule_key.as_str() != self.rule_key {
            return Err(persisted_error(
                "persisted completeness component keys are not canonical",
            ));
        }
        PartyCompletenessComponent::try_new(component_key, rule_key, self.weight_basis_points)
            .map_err(|error| persisted_domain_error("completeness component", error))
    }
}

fn validate_size(bytes: &[u8], maximum_bytes: u64, label: &str) -> Result<(), SdkError> {
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum_bytes {
        return Err(persisted_error(format!(
            "{label} state exceeds the maximum of {maximum_bytes} bytes"
        )));
    }
    Ok(())
}

fn persisted_domain_error(label: &str, error: SdkError) -> SdkError {
    persisted_error(format!(
        "{label} failed strict persisted-state validation: {}: {}",
        error.code, error.safe_message
    ))
}

fn persisted_error(message: impl Into<String>) -> SdkError {
    SdkError::new(
        "DATA_QUALITY_PERSISTED_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "The persisted Data Quality state is invalid.",
    )
    .with_internal_reference(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule_set() -> PartyRuleSetVersion {
        PartyRuleSetVersion::publish(vec![
            PartyQualityRule::try_new(
                RuleKey::try_new("display_name.minimum").unwrap(),
                QualitySeverity::Warning,
                PartyQualityEvaluator::display_name_min_utf8_bytes(4).unwrap(),
                "Display name length",
                "Replace the display name with a meaningful customer name.",
            )
            .unwrap(),
            PartyQualityRule::try_new(
                RuleKey::try_new("display_name.placeholder").unwrap(),
                QualitySeverity::Error,
                PartyQualityEvaluator::display_name_placeholder_exact_ascii_casefold(vec![
                    "unknown".to_owned(),
                    "n/a".to_owned(),
                ])
                .unwrap(),
                "Placeholder display name",
                "Replace the placeholder with the real customer name.",
            )
            .unwrap(),
        ])
        .unwrap()
    }

    fn profile(rule_set: &PartyRuleSetVersion) -> PartyCompletenessProfileVersion {
        PartyCompletenessProfileVersion::publish(
            rule_set,
            vec![
                PartyCompletenessComponent::try_new(
                    ComponentKey::try_new("name.minimum").unwrap(),
                    RuleKey::try_new("display_name.minimum").unwrap(),
                    4_000,
                )
                .unwrap(),
                PartyCompletenessComponent::try_new(
                    ComponentKey::try_new("name.placeholder").unwrap(),
                    RuleKey::try_new("display_name.placeholder").unwrap(),
                    6_000,
                )
                .unwrap(),
            ],
        )
        .unwrap()
    }

    #[test]
    fn rule_set_state_round_trip_is_exact_and_deterministic() {
        let rule_set = rule_set();
        let first = encode_party_rule_set_version_state(&rule_set).unwrap();
        let second = encode_party_rule_set_version_state(&rule_set).unwrap();
        let decoded = decode_party_rule_set_version_state(&first).unwrap();

        assert_eq!(first, second);
        assert_eq!(decoded, rule_set);
        assert_ne!(party_rule_set_version_state_descriptor_hash(), [0; 32]);
    }

    #[test]
    fn rule_set_state_rejects_forged_identity_and_noncanonical_order() {
        let rule_set = rule_set();
        let mut forged = PartyRuleSetVersionStateV1::from(&rule_set);
        forged.version_id.push('x');
        assert!(
            decode_party_rule_set_version_state(&serde_json::to_vec(&forged).unwrap()).is_err()
        );

        let mut reordered = PartyRuleSetVersionStateV1::from(&rule_set);
        reordered.rules.reverse();
        assert!(
            decode_party_rule_set_version_state(&serde_json::to_vec(&reordered).unwrap()).is_err()
        );
    }

    #[test]
    fn rule_set_state_rejects_unknown_fields() {
        let rule_set = rule_set();
        let encoded = String::from_utf8(encode_party_rule_set_version_state(&rule_set).unwrap())
            .unwrap();
        let injected = encoded.replacen('{', "{\"unknown\":true,", 1);
        assert!(decode_party_rule_set_version_state(injected.as_bytes()).is_err());
    }

    #[test]
    fn completeness_profile_state_round_trip_is_exact_and_bound_to_rule_set() {
        let rule_set = rule_set();
        let profile = profile(&rule_set);
        let first = encode_party_completeness_profile_version_state(&profile).unwrap();
        let second = encode_party_completeness_profile_version_state(&profile).unwrap();
        let decoded = decode_party_completeness_profile_version_state(&first, &rule_set).unwrap();

        assert_eq!(first, second);
        assert_eq!(decoded, profile);
        assert_ne!(
            party_completeness_profile_version_state_descriptor_hash(),
            [0; 32]
        );
    }

    #[test]
    fn completeness_profile_state_rejects_forged_identity_and_wrong_rule_set() {
        let rule_set = rule_set();
        let profile = profile(&rule_set);
        let mut forged = PartyCompletenessProfileVersionStateV1::from(&profile);
        forged.version_id.push('x');
        assert!(decode_party_completeness_profile_version_state(
            &serde_json::to_vec(&forged).unwrap(),
            &rule_set
        )
        .is_err());

        let other_rule_set = PartyRuleSetVersion::publish(vec![
            PartyQualityRule::try_new(
                RuleKey::try_new("display_name.minimum").unwrap(),
                QualitySeverity::Warning,
                PartyQualityEvaluator::display_name_min_utf8_bytes(5).unwrap(),
                "Display name length",
                "Replace the display name with a meaningful customer name.",
            )
            .unwrap(),
        ])
        .unwrap();
        let bytes = encode_party_completeness_profile_version_state(&profile).unwrap();
        assert!(
            decode_party_completeness_profile_version_state(&bytes, &other_rule_set).is_err()
        );
    }
}
