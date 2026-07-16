use crate::canonicalization::semantic_to_vec;
use crm_module_sdk::{ErrorCategory, FieldName, FieldViolation, SdkError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

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
