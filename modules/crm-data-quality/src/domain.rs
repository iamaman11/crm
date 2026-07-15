use crm_module_sdk::{ErrorCategory, FieldName, FieldViolation, SdkError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const RULE_SET_ID_DOMAIN: &[u8] = b"crm.data-quality.party-rule-set-version/v1";
const COMPLETENESS_PROFILE_ID_DOMAIN: &[u8] =
    b"crm.data-quality.party-completeness-profile-version/v1";
const TARGET_RESOURCE_TYPE: &str = "parties.party";
const EVALUATOR_SEMANTIC_VERSION: &str = "1.0.0";
const COMPLETENESS_SEMANTIC_VERSION: &str = "1.0.0";

const MAX_RULES_PER_RULE_SET: usize = 64;
const MAX_COMPLETENESS_COMPONENTS: usize = 64;
const MAX_CANONICAL_KEY_BYTES: usize = 80;
const MAX_TITLE_BYTES: usize = 160;
const MAX_REMEDIATION_GUIDANCE_BYTES: usize = 512;
const MAX_PLACEHOLDER_TOKENS: usize = 32;
const MAX_PLACEHOLDER_TOKEN_BYTES: usize = 64;
const MAX_PARTY_DISPLAY_NAME_BYTES: usize = 240;
const TOTAL_BASIS_POINTS: u32 = 10_000;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RuleKey(String);

impl RuleKey {
    pub fn try_new(value: impl Into<String>) -> Result<Self, SdkError> {
        canonical_key(value.into(), "rule.rule_key").map(Self)
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ComponentKey(String);

impl ComponentKey {
    pub fn try_new(value: impl Into<String>) -> Result<Self, SdkError> {
        canonical_key(value.into(), "component.component_key").map(Self)
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PartyRuleSetVersionId(String);

impl PartyRuleSetVersionId {
    fn from_digest(digest: &[u8]) -> Self {
        Self(format!("dq-party-rule-set-{}", hex(digest)))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PartyCompletenessProfileVersionId(String);

impl PartyCompletenessProfileVersionId {
    fn from_digest(digest: &[u8]) -> Self {
        Self(format!("dq-party-completeness-profile-{}", hex(digest)))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualitySeverity {
    Info,
    Warning,
    Error,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvaluatedPartyKind {
    Person,
    Organization,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyQualityInput {
    kind: EvaluatedPartyKind,
    display_name: String,
}

impl PartyQualityInput {
    pub fn try_new(
        kind: EvaluatedPartyKind,
        display_name: impl Into<String>,
    ) -> Result<Self, SdkError> {
        let display_name = display_name.into();
        validate_canonical_party_display_name(&display_name)?;
        Ok(Self { kind, display_name })
    }

    pub const fn kind(&self) -> EvaluatedPartyKind {
        self.kind
    }

    pub fn display_name(&self) -> &str {
        self.display_name.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DisplayNameMinUtf8Bytes {
    minimum_utf8_bytes: u32,
}

impl DisplayNameMinUtf8Bytes {
    pub fn try_new(minimum_utf8_bytes: u32) -> Result<Self, SdkError> {
        if !(2..=64).contains(&minimum_utf8_bytes) {
            return Err(invalid(
                "DATA_QUALITY_MINIMUM_UTF8_BYTES_INVALID",
                "rule.evaluator.minimum_utf8_bytes",
                "minimum UTF-8 bytes must be in the inclusive range 2..=64",
            ));
        }
        Ok(Self { minimum_utf8_bytes })
    }

    pub const fn minimum_utf8_bytes(&self) -> u32 {
        self.minimum_utf8_bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DisplayNamePlaceholderExactAsciiCasefold {
    placeholder_tokens: Vec<String>,
}

impl DisplayNamePlaceholderExactAsciiCasefold {
    pub fn try_new(placeholder_tokens: Vec<String>) -> Result<Self, SdkError> {
        if placeholder_tokens.is_empty() || placeholder_tokens.len() > MAX_PLACEHOLDER_TOKENS {
            return Err(invalid(
                "DATA_QUALITY_PLACEHOLDER_TOKENS_INVALID",
                "rule.evaluator.placeholder_tokens",
                format!(
                    "placeholder token count must be in the inclusive range 1..={MAX_PLACEHOLDER_TOKENS}"
                ),
            ));
        }

        let mut canonical = placeholder_tokens
            .into_iter()
            .map(canonical_placeholder_token)
            .collect::<Result<Vec<_>, _>>()?;
        canonical.sort();

        if canonical.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(invalid(
                "DATA_QUALITY_PLACEHOLDER_TOKENS_DUPLICATE",
                "rule.evaluator.placeholder_tokens",
                "placeholder tokens must be unique after canonical ASCII case folding",
            ));
        }

        Ok(Self {
            placeholder_tokens: canonical,
        })
    }

    pub fn placeholder_tokens(&self) -> &[String] {
        &self.placeholder_tokens
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "parameters", rename_all = "snake_case")]
pub enum PartyQualityEvaluator {
    DisplayNameMinUtf8Bytes(DisplayNameMinUtf8Bytes),
    DisplayNamePlaceholderExactAsciiCasefold(DisplayNamePlaceholderExactAsciiCasefold),
}

impl PartyQualityEvaluator {
    pub fn display_name_min_utf8_bytes(minimum_utf8_bytes: u32) -> Result<Self, SdkError> {
        DisplayNameMinUtf8Bytes::try_new(minimum_utf8_bytes).map(Self::DisplayNameMinUtf8Bytes)
    }

    pub fn display_name_placeholder_exact_ascii_casefold(
        placeholder_tokens: Vec<String>,
    ) -> Result<Self, SdkError> {
        DisplayNamePlaceholderExactAsciiCasefold::try_new(placeholder_tokens)
            .map(Self::DisplayNamePlaceholderExactAsciiCasefold)
    }

    pub fn evaluate(&self, input: &PartyQualityInput) -> PartyEvaluatorResult {
        match self {
            Self::DisplayNameMinUtf8Bytes(parameters) => {
                let passed = input.display_name().len() >= parameters.minimum_utf8_bytes as usize;
                PartyEvaluatorResult {
                    passed,
                    reason_code: if passed {
                        "DATA_QUALITY_RULE_PASSED"
                    } else {
                        "DATA_QUALITY_PARTY_DISPLAY_NAME_TOO_SHORT"
                    },
                }
            }
            Self::DisplayNamePlaceholderExactAsciiCasefold(parameters) => {
                let matches_placeholder = input.display_name().is_ascii()
                    && parameters
                        .placeholder_tokens
                        .binary_search(&input.display_name().to_ascii_lowercase())
                        .is_ok();
                PartyEvaluatorResult {
                    passed: !matches_placeholder,
                    reason_code: if matches_placeholder {
                        "DATA_QUALITY_PARTY_DISPLAY_NAME_PLACEHOLDER"
                    } else {
                        "DATA_QUALITY_RULE_PASSED"
                    },
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PartyEvaluatorResult {
    pub passed: bool,
    pub reason_code: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartyQualityRule {
    rule_key: RuleKey,
    severity: QualitySeverity,
    evaluator: PartyQualityEvaluator,
    title: String,
    remediation_guidance: String,
}

impl PartyQualityRule {
    pub fn try_new(
        rule_key: RuleKey,
        severity: QualitySeverity,
        evaluator: PartyQualityEvaluator,
        title: impl Into<String>,
        remediation_guidance: impl Into<String>,
    ) -> Result<Self, SdkError> {
        Ok(Self {
            rule_key,
            severity,
            evaluator,
            title: canonical_text(
                title.into(),
                MAX_TITLE_BYTES,
                "rule.title",
                "DATA_QUALITY_RULE_TITLE_INVALID",
            )?,
            remediation_guidance: canonical_text(
                remediation_guidance.into(),
                MAX_REMEDIATION_GUIDANCE_BYTES,
                "rule.remediation_guidance",
                "DATA_QUALITY_RULE_REMEDIATION_GUIDANCE_INVALID",
            )?,
        })
    }

    pub fn rule_key(&self) -> &RuleKey {
        &self.rule_key
    }

    pub const fn severity(&self) -> QualitySeverity {
        self.severity
    }

    pub fn evaluator(&self) -> &PartyQualityEvaluator {
        &self.evaluator
    }

    pub fn title(&self) -> &str {
        self.title.as_str()
    }

    pub fn remediation_guidance(&self) -> &str {
        self.remediation_guidance.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyRuleSetVersion {
    version_id: PartyRuleSetVersionId,
    rules: Vec<PartyQualityRule>,
}

#[derive(Serialize)]
struct CanonicalRuleSet<'a> {
    target_resource_type: &'static str,
    evaluator_semantic_version: &'static str,
    rules: &'a [PartyQualityRule],
}

impl PartyRuleSetVersion {
    pub fn publish(mut rules: Vec<PartyQualityRule>) -> Result<Self, SdkError> {
        if rules.is_empty() || rules.len() > MAX_RULES_PER_RULE_SET {
            return Err(invalid(
                "DATA_QUALITY_RULE_SET_SIZE_INVALID",
                "rule_set.rules",
                format!(
                    "rule count must be in the inclusive range 1..={MAX_RULES_PER_RULE_SET}"
                ),
            ));
        }

        rules.sort_by(|left, right| left.rule_key.cmp(&right.rule_key));
        if rules
            .windows(2)
            .any(|pair| pair[0].rule_key == pair[1].rule_key)
        {
            return Err(invalid(
                "DATA_QUALITY_RULE_KEY_DUPLICATE",
                "rule_set.rules",
                "rule keys must be unique within one published rule-set version",
            ));
        }

        let canonical = CanonicalRuleSet {
            target_resource_type: TARGET_RESOURCE_TYPE,
            evaluator_semantic_version: EVALUATOR_SEMANTIC_VERSION,
            rules: &rules,
        };
        let version_id = PartyRuleSetVersionId::from_digest(&canonical_digest(
            RULE_SET_ID_DOMAIN,
            &canonical,
        ));

        Ok(Self { version_id, rules })
    }

    pub fn version_id(&self) -> &PartyRuleSetVersionId {
        &self.version_id
    }

    pub fn rules(&self) -> &[PartyQualityRule] {
        &self.rules
    }

    pub fn rule(&self, rule_key: &RuleKey) -> Option<&PartyQualityRule> {
        self.rules
            .binary_search_by(|rule| rule.rule_key.cmp(rule_key))
            .ok()
            .map(|index| &self.rules[index])
    }

    pub fn evaluate(&self, input: &PartyQualityInput) -> Vec<PartyRuleEvaluation> {
        self.rules
            .iter()
            .map(|rule| {
                let result = rule.evaluator.evaluate(input);
                PartyRuleEvaluation {
                    rule_set_version_id: self.version_id.clone(),
                    rule_key: rule.rule_key.clone(),
                    passed: result.passed,
                    reason_code: result.reason_code.to_owned(),
                }
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyRuleEvaluation {
    rule_set_version_id: PartyRuleSetVersionId,
    rule_key: RuleKey,
    passed: bool,
    reason_code: String,
}

impl PartyRuleEvaluation {
    pub fn rule_set_version_id(&self) -> &PartyRuleSetVersionId {
        &self.rule_set_version_id
    }

    pub fn rule_key(&self) -> &RuleKey {
        &self.rule_key
    }

    pub const fn passed(&self) -> bool {
        self.passed
    }

    pub fn reason_code(&self) -> &str {
        self.reason_code.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartyCompletenessComponent {
    component_key: ComponentKey,
    rule_key: RuleKey,
    weight_basis_points: u32,
}

impl PartyCompletenessComponent {
    pub fn try_new(
        component_key: ComponentKey,
        rule_key: RuleKey,
        weight_basis_points: u32,
    ) -> Result<Self, SdkError> {
        if weight_basis_points == 0 || weight_basis_points > TOTAL_BASIS_POINTS {
            return Err(invalid(
                "DATA_QUALITY_COMPLETENESS_WEIGHT_INVALID",
                "component.weight_basis_points",
                format!(
                    "component weight must be in the inclusive range 1..={TOTAL_BASIS_POINTS}"
                ),
            ));
        }
        Ok(Self {
            component_key,
            rule_key,
            weight_basis_points,
        })
    }

    pub fn component_key(&self) -> &ComponentKey {
        &self.component_key
    }

    pub fn rule_key(&self) -> &RuleKey {
        &self.rule_key
    }

    pub const fn weight_basis_points(&self) -> u32 {
        self.weight_basis_points
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyCompletenessProfileVersion {
    version_id: PartyCompletenessProfileVersionId,
    rule_set_version_id: PartyRuleSetVersionId,
    components: Vec<PartyCompletenessComponent>,
}

#[derive(Serialize)]
struct CanonicalCompletenessProfile<'a> {
    target_resource_type: &'static str,
    completeness_semantic_version: &'static str,
    rule_set_version_id: &'a str,
    components: &'a [PartyCompletenessComponent],
}

impl PartyCompletenessProfileVersion {
    pub fn publish(
        rule_set: &PartyRuleSetVersion,
        mut components: Vec<PartyCompletenessComponent>,
    ) -> Result<Self, SdkError> {
        if components.is_empty() || components.len() > MAX_COMPLETENESS_COMPONENTS {
            return Err(invalid(
                "DATA_QUALITY_COMPLETENESS_COMPONENT_COUNT_INVALID",
                "completeness_profile.components",
                format!(
                    "component count must be in the inclusive range 1..={MAX_COMPLETENESS_COMPONENTS}"
                ),
            ));
        }

        components.sort_by(|left, right| left.component_key.cmp(&right.component_key));
        if components
            .windows(2)
            .any(|pair| pair[0].component_key == pair[1].component_key)
        {
            return Err(invalid(
                "DATA_QUALITY_COMPLETENESS_COMPONENT_KEY_DUPLICATE",
                "completeness_profile.components",
                "component keys must be unique within one completeness-profile version",
            ));
        }

        for component in &components {
            if rule_set.rule(&component.rule_key).is_none() {
                return Err(invalid(
                    "DATA_QUALITY_COMPLETENESS_RULE_REFERENCE_INVALID",
                    "completeness_profile.components",
                    format!(
                        "component {} references unknown rule {}",
                        component.component_key.as_str(),
                        component.rule_key.as_str()
                    ),
                ));
            }
        }

        let total_weight = components.iter().try_fold(0_u32, |total, component| {
            total.checked_add(component.weight_basis_points).ok_or_else(|| {
                invalid(
                    "DATA_QUALITY_COMPLETENESS_WEIGHT_OVERFLOW",
                    "completeness_profile.components",
                    "component weights overflowed the supported integer range",
                )
            })
        })?;
        if total_weight != TOTAL_BASIS_POINTS {
            return Err(invalid(
                "DATA_QUALITY_COMPLETENESS_WEIGHT_TOTAL_INVALID",
                "completeness_profile.components",
                format!(
                    "component weights must sum exactly to {TOTAL_BASIS_POINTS} basis points"
                ),
            ));
        }

        let canonical = CanonicalCompletenessProfile {
            target_resource_type: TARGET_RESOURCE_TYPE,
            completeness_semantic_version: COMPLETENESS_SEMANTIC_VERSION,
            rule_set_version_id: rule_set.version_id.as_str(),
            components: &components,
        };
        let version_id = PartyCompletenessProfileVersionId::from_digest(&canonical_digest(
            COMPLETENESS_PROFILE_ID_DOMAIN,
            &canonical,
        ));

        Ok(Self {
            version_id,
            rule_set_version_id: rule_set.version_id.clone(),
            components,
        })
    }

    pub fn version_id(&self) -> &PartyCompletenessProfileVersionId {
        &self.version_id
    }

    pub fn rule_set_version_id(&self) -> &PartyRuleSetVersionId {
        &self.rule_set_version_id
    }

    pub fn components(&self) -> &[PartyCompletenessComponent] {
        &self.components
    }

    pub fn score(
        &self,
        outcomes: &[PartyRuleEvaluation],
    ) -> Result<PartyCompletenessScore, SdkError> {
        let mut by_rule = BTreeMap::new();
        for outcome in outcomes {
            if outcome.rule_set_version_id != self.rule_set_version_id {
                return Err(invalid(
                    "DATA_QUALITY_COMPLETENESS_OUTCOME_RULE_SET_MISMATCH",
                    "completeness.outcomes",
                    "every rule outcome must belong to the completeness profile rule-set version",
                ));
            }
            if by_rule.insert(outcome.rule_key.clone(), outcome).is_some() {
                return Err(invalid(
                    "DATA_QUALITY_COMPLETENESS_OUTCOME_DUPLICATE",
                    "completeness.outcomes",
                    "rule outcomes must contain at most one outcome for each rule key",
                ));
            }
        }

        let mut score_basis_points = 0_u32;
        let mut awards = Vec::with_capacity(self.components.len());
        for component in &self.components {
            let outcome = by_rule.get(&component.rule_key).ok_or_else(|| {
                invalid(
                    "DATA_QUALITY_COMPLETENESS_OUTCOME_MISSING",
                    "completeness.outcomes",
                    format!(
                        "missing outcome for completeness rule {}",
                        component.rule_key.as_str()
                    ),
                )
            })?;
            let awarded_basis_points = if outcome.passed {
                component.weight_basis_points
            } else {
                0
            };
            score_basis_points = score_basis_points
                .checked_add(awarded_basis_points)
                .ok_or_else(|| {
                    invalid(
                        "DATA_QUALITY_COMPLETENESS_SCORE_OVERFLOW",
                        "completeness.score_basis_points",
                        "completeness score overflowed the supported integer range",
                    )
                })?;
            awards.push(PartyCompletenessAward {
                component_key: component.component_key.clone(),
                rule_key: component.rule_key.clone(),
                awarded_basis_points,
            });
        }

        if score_basis_points > TOTAL_BASIS_POINTS {
            return Err(invalid(
                "DATA_QUALITY_COMPLETENESS_SCORE_INVALID",
                "completeness.score_basis_points",
                "completeness score cannot exceed 10,000 basis points",
            ));
        }

        Ok(PartyCompletenessScore {
            score_basis_points,
            awards,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyCompletenessAward {
    component_key: ComponentKey,
    rule_key: RuleKey,
    awarded_basis_points: u32,
}

impl PartyCompletenessAward {
    pub fn component_key(&self) -> &ComponentKey {
        &self.component_key
    }

    pub fn rule_key(&self) -> &RuleKey {
        &self.rule_key
    }

    pub const fn awarded_basis_points(&self) -> u32 {
        self.awarded_basis_points
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyCompletenessScore {
    score_basis_points: u32,
    awards: Vec<PartyCompletenessAward>,
}

impl PartyCompletenessScore {
    pub const fn score_basis_points(&self) -> u32 {
        self.score_basis_points
    }

    pub fn awards(&self) -> &[PartyCompletenessAward] {
        &self.awards
    }
}

fn canonical_key(value: String, field: &'static str) -> Result<String, SdkError> {
    if value.is_empty()
        || value.len() > MAX_CANONICAL_KEY_BYTES
        || !value.is_ascii()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-'))
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
            "DATA_QUALITY_CANONICAL_KEY_INVALID",
            field,
            "canonical keys must be 1..=80 ASCII bytes, use lowercase letters/digits/._-, and start/end with an alphanumeric character",
        ));
    }
    Ok(value)
}

fn canonical_placeholder_token(value: String) -> Result<String, SdkError> {
    if !value.is_ascii() || value.chars().any(char::is_control) {
        return Err(invalid(
            "DATA_QUALITY_PLACEHOLDER_TOKEN_INVALID",
            "rule.evaluator.placeholder_tokens",
            "placeholder tokens must contain printable ASCII only",
        ));
    }
    let canonical = value
        .split_ascii_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    if canonical.is_empty() || canonical.len() > MAX_PLACEHOLDER_TOKEN_BYTES {
        return Err(invalid(
            "DATA_QUALITY_PLACEHOLDER_TOKEN_INVALID",
            "rule.evaluator.placeholder_tokens",
            format!(
                "each canonical placeholder token must contain 1..={MAX_PLACEHOLDER_TOKEN_BYTES} bytes"
            ),
        ));
    }
    Ok(canonical)
}

fn canonical_text(
    value: String,
    maximum_bytes: usize,
    field: &'static str,
    code: &'static str,
) -> Result<String, SdkError> {
    if value.chars().any(char::is_control) {
        return Err(invalid(code, field, "text must not contain control characters"));
    }
    let canonical = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if canonical.is_empty() || canonical.len() > maximum_bytes {
        return Err(invalid(
            code,
            field,
            format!("text must contain 1..={maximum_bytes} UTF-8 bytes after normalization"),
        ));
    }
    Ok(canonical)
}

fn validate_canonical_party_display_name(value: &str) -> Result<(), SdkError> {
    if value.chars().any(char::is_control)
        || value.is_empty()
        || value.len() > MAX_PARTY_DISPLAY_NAME_BYTES
        || value.split_whitespace().collect::<Vec<_>>().join(" ") != value
    {
        return Err(invalid(
            "DATA_QUALITY_PARTY_INPUT_INVALID",
            "party.display_name",
            "evaluated Party display name must be non-empty, canonical whitespace-normalized text without control characters and at most 240 UTF-8 bytes",
        ));
    }
    Ok(())
}

fn canonical_digest<T: Serialize>(domain: &[u8], value: &T) -> Vec<u8> {
    let encoded = serde_json::to_vec(value).expect("canonical data-quality state must serialize");
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
        "The data-quality definition or evidence is invalid.",
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

    fn key(value: &str) -> RuleKey {
        RuleKey::try_new(value).unwrap()
    }

    fn component_key(value: &str) -> ComponentKey {
        ComponentKey::try_new(value).unwrap()
    }

    fn minimum_rule(rule_key: &str, minimum: u32) -> PartyQualityRule {
        PartyQualityRule::try_new(
            key(rule_key),
            QualitySeverity::Warning,
            PartyQualityEvaluator::display_name_min_utf8_bytes(minimum).unwrap(),
            "Display name length",
            "Replace the display name with a meaningful customer name.",
        )
        .unwrap()
    }

    fn placeholder_rule(rule_key: &str, tokens: &[&str]) -> PartyQualityRule {
        PartyQualityRule::try_new(
            key(rule_key),
            QualitySeverity::Error,
            PartyQualityEvaluator::display_name_placeholder_exact_ascii_casefold(
                tokens.iter().map(|value| (*value).to_owned()).collect(),
            )
            .unwrap(),
            "Placeholder display name",
            "Replace the placeholder with the real customer name.",
        )
        .unwrap()
    }

    fn rule_set() -> PartyRuleSetVersion {
        PartyRuleSetVersion::publish(vec![
            minimum_rule("display_name.minimum", 4),
            placeholder_rule("display_name.placeholder", &["unknown", "n/a"]),
        ])
        .unwrap()
    }

    #[test]
    fn rule_set_identity_is_independent_of_caller_order() {
        let first = PartyRuleSetVersion::publish(vec![
            placeholder_rule("display_name.placeholder", &["N/A", " UNKNOWN "]),
            minimum_rule("display_name.minimum", 4),
        ])
        .unwrap();
        let second = PartyRuleSetVersion::publish(vec![
            minimum_rule("display_name.minimum", 4),
            placeholder_rule("display_name.placeholder", &["unknown", "n/a"]),
        ])
        .unwrap();

        assert_eq!(first.version_id(), second.version_id());
        assert_eq!(first.rules(), second.rules());
    }

    #[test]
    fn changed_evaluator_semantics_change_rule_set_identity() {
        let first = PartyRuleSetVersion::publish(vec![minimum_rule("display_name.minimum", 4)]).unwrap();
        let second = PartyRuleSetVersion::publish(vec![minimum_rule("display_name.minimum", 5)]).unwrap();
        assert_ne!(first.version_id(), second.version_id());
    }

    #[test]
    fn placeholder_tokens_are_canonical_sorted_and_duplicates_are_rejected() {
        let evaluator = DisplayNamePlaceholderExactAsciiCasefold::try_new(vec![
            " UNKNOWN ".to_owned(),
            "N/A".to_owned(),
        ])
        .unwrap();
        assert_eq!(
            evaluator.placeholder_tokens(),
            &["n/a".to_owned(), "unknown".to_owned()]
        );

        assert!(
            DisplayNamePlaceholderExactAsciiCasefold::try_new(vec![
                "UNKNOWN".to_owned(),
                " unknown ".to_owned(),
            ])
            .is_err()
        );
    }

    #[test]
    fn v1_evaluators_are_exact_and_deterministic() {
        let rules = rule_set();
        let placeholder = PartyQualityInput::try_new(EvaluatedPartyKind::Person, "Unknown").unwrap();
        let short = PartyQualityInput::try_new(EvaluatedPartyKind::Organization, "Acme").unwrap();
        let non_ascii = PartyQualityInput::try_new(EvaluatedPartyKind::Person, "Неизвестно").unwrap();

        let placeholder_outcomes = rules.evaluate(&placeholder);
        assert_eq!(placeholder_outcomes.len(), 2);
        assert!(
            placeholder_outcomes
                .iter()
                .any(|outcome| outcome.rule_key().as_str() == "display_name.placeholder"
                    && !outcome.passed()
                    && outcome.reason_code() == "DATA_QUALITY_PARTY_DISPLAY_NAME_PLACEHOLDER")
        );

        assert!(rules.evaluate(&short).iter().all(PartyRuleEvaluation::passed));
        assert!(
            rules
                .evaluate(&non_ascii)
                .iter()
                .find(|outcome| outcome.rule_key().as_str() == "display_name.placeholder")
                .unwrap()
                .passed()
        );
    }

    #[test]
    fn rule_set_rejects_duplicate_keys_and_invalid_bounds() {
        assert!(PartyRuleSetVersion::publish(Vec::new()).is_err());
        assert!(
            PartyRuleSetVersion::publish(vec![
                minimum_rule("display_name.minimum", 4),
                placeholder_rule("display_name.minimum", &["unknown"]),
            ])
            .is_err()
        );
        assert!(PartyQualityEvaluator::display_name_min_utf8_bytes(1).is_err());
        assert!(RuleKey::try_new("Display Name").is_err());
    }

    #[test]
    fn completeness_profile_identity_is_independent_of_component_order() {
        let rules = rule_set();
        let first = PartyCompletenessProfileVersion::publish(
            &rules,
            vec![
                PartyCompletenessComponent::try_new(
                    component_key("name.minimum"),
                    key("display_name.minimum"),
                    4_000,
                )
                .unwrap(),
                PartyCompletenessComponent::try_new(
                    component_key("name.placeholder"),
                    key("display_name.placeholder"),
                    6_000,
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let second = PartyCompletenessProfileVersion::publish(
            &rules,
            vec![
                PartyCompletenessComponent::try_new(
                    component_key("name.placeholder"),
                    key("display_name.placeholder"),
                    6_000,
                )
                .unwrap(),
                PartyCompletenessComponent::try_new(
                    component_key("name.minimum"),
                    key("display_name.minimum"),
                    4_000,
                )
                .unwrap(),
            ],
        )
        .unwrap();

        assert_eq!(first.version_id(), second.version_id());
        assert_eq!(first.components(), second.components());
    }

    #[test]
    fn completeness_profile_requires_exact_total_and_known_rules() {
        let rules = rule_set();
        assert!(
            PartyCompletenessProfileVersion::publish(
                &rules,
                vec![PartyCompletenessComponent::try_new(
                    component_key("name.minimum"),
                    key("display_name.minimum"),
                    9_999,
                )
                .unwrap()],
            )
            .is_err()
        );
        assert!(
            PartyCompletenessProfileVersion::publish(
                &rules,
                vec![PartyCompletenessComponent::try_new(
                    component_key("unknown"),
                    key("unknown.rule"),
                    10_000,
                )
                .unwrap()],
            )
            .is_err()
        );
    }

    #[test]
    fn completeness_score_reconciles_exact_integer_component_awards() {
        let rules = rule_set();
        let profile = PartyCompletenessProfileVersion::publish(
            &rules,
            vec![
                PartyCompletenessComponent::try_new(
                    component_key("name.minimum"),
                    key("display_name.minimum"),
                    4_000,
                )
                .unwrap(),
                PartyCompletenessComponent::try_new(
                    component_key("name.placeholder"),
                    key("display_name.placeholder"),
                    6_000,
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let input = PartyQualityInput::try_new(EvaluatedPartyKind::Person, "N/A").unwrap();
        let score = profile.score(&rules.evaluate(&input)).unwrap();

        assert_eq!(score.score_basis_points(), 0);
        assert_eq!(score.awards().len(), 2);
        assert_eq!(
            score
                .awards()
                .iter()
                .map(PartyCompletenessAward::awarded_basis_points)
                .sum::<u32>(),
            score.score_basis_points()
        );
    }

    #[test]
    fn canonical_party_input_rejects_noncanonical_or_invalid_display_name() {
        assert!(PartyQualityInput::try_new(EvaluatedPartyKind::Person, "  Ada  Lovelace ").is_err());
        assert!(PartyQualityInput::try_new(EvaluatedPartyKind::Person, "").is_err());
        assert!(PartyQualityInput::try_new(EvaluatedPartyKind::Person, "Ada\nLovelace").is_err());
    }

    #[test]
    fn all_rule_keys_remain_unique_after_publication() {
        let rules = rule_set();
        let unique = rules
            .rules()
            .iter()
            .map(|rule| rule.rule_key().clone())
            .collect::<BTreeSet<_>>();
        assert_eq!(unique.len(), rules.rules().len());
    }
}
