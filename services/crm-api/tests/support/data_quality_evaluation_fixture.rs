use crm_proto_contracts::crm::data_quality::v1 as data_quality;
use std::time::{SystemTime, UNIX_EPOCH};

pub const TENANT: &str = "tenant-a";
pub const API_ACTOR: &str = "actor-a";
pub const TOKEN: &str = "data-quality-evaluation-process-token-0123456789abcdef0123456789abcdef";
pub const APPROVAL_KEY: &str = "data-quality-evaluation-process-approval-key-0123456789abcdef";
pub const PUBLISH_RULE_SET: &str = "data_quality.party.rule_set.publish";
pub const PUBLISH_PROFILE: &str = "data_quality.party.completeness_profile.publish";
pub const REQUEST_EVALUATION: &str = "data_quality.party.evaluation.request";
pub const PARTY_CREATE: &str = "parties.party.create";
pub const INTERNAL_STAGE: &str = "data_quality.party.evaluation.internal.stage";
pub const WORKER_ACTOR: &str = "crm-api-data-quality-evaluation-worker";

pub fn rule_set_input() -> data_quality::PartyRuleSetDefinition {
    data_quality::PartyRuleSetDefinition {
        evaluator_semantic_version: data_quality::PartyQualityEvaluatorSemanticVersion::V1 as i32,
        rules: vec![
            data_quality::PartyQualityRule {
                rule_key: "display_name.evaluation_process_minimum".to_owned(),
                severity: data_quality::QualitySeverity::Warning as i32,
                evaluator: Some(
                    data_quality::party_quality_rule::Evaluator::DisplayNameMinUtf8Bytes(
                        data_quality::PartyDisplayNameMinUtf8BytesEvaluator {
                            minimum_utf8_bytes: 7,
                        },
                    ),
                ),
                title: "Evaluation process display name length".to_owned(),
                remediation_guidance: "Use a meaningful customer display name.".to_owned(),
            },
            data_quality::PartyQualityRule {
                rule_key: "display_name.evaluation_process_placeholder".to_owned(),
                severity: data_quality::QualitySeverity::Error as i32,
                evaluator: Some(
                    data_quality::party_quality_rule::Evaluator::DisplayNamePlaceholderExactAsciiCasefold(
                        data_quality::PartyDisplayNamePlaceholderExactAsciiCasefoldEvaluator {
                            placeholder_tokens: vec!["unknown".to_owned()],
                        },
                    ),
                ),
                title: "Evaluation process placeholder".to_owned(),
                remediation_guidance: "Replace the placeholder customer name.".to_owned(),
            },
        ],
    }
}

pub fn profile_input(
    rule_set_version_id: &str,
) -> data_quality::PartyCompletenessProfileDefinition {
    data_quality::PartyCompletenessProfileDefinition {
        completeness_semantic_version: data_quality::PartyCompletenessSemanticVersion::V1 as i32,
        rule_set_version_ref: Some(data_quality::PartyRuleSetVersionRef {
            rule_set_version_id: rule_set_version_id.to_owned(),
        }),
        components: vec![
            data_quality::PartyCompletenessComponent {
                component_key: "name.minimum".to_owned(),
                rule_key: "display_name.evaluation_process_minimum".to_owned(),
                weight_basis_points: 4_000,
            },
            data_quality::PartyCompletenessComponent {
                component_key: "name.placeholder".to_owned(),
                rule_key: "display_name.evaluation_process_placeholder".to_owned(),
                weight_basis_points: 6_000,
            },
        ],
    }
}

pub fn unique_id(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after epoch")
        .as_nanos();
    format!("{prefix}-{nanos}")
}
