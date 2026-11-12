//! Read-only original crm-data-quality Party name review. Evaluation lives
//! in the ORIGINAL domain crate; Connect calls one typed owner boundary.
use super::parties;
use crm_data_quality::{
    EvaluatedPartyKind, PartyQualityInput, QualitySeverity, builtin_party_name_review_rules_v1,
};
use crm_parties::{Party, PartyKind};
use worker::{D1Database, Result};

pub(super) struct RuleCheck {
    pub(super) rule_key: String,
    pub(super) title: String,
    pub(super) remediation: String,
    pub(super) severity_code: &'static str,
    pub(super) passed: bool,
    pub(super) reason_code: String,
}
pub(super) struct Evaluation {
    pub(super) party: Party,
    pub(super) rule_set_version: String,
    pub(super) checks: Vec<RuleCheck>,
    pub(super) failed_count: u32,
}

/// The one owner boundary, used by generated Connect handlers.
/// No new persisted finding, score, or alternate evaluation is introduced.
pub(super) async fn evaluate_typed(
    db: &D1Database,
    tenant: &str,
    id: &str,
) -> Result<Option<Evaluation>> {
    let Some(party) = parties::stored(db, tenant, id).await? else {
        return Ok(None);
    };
    let kind = match party.kind() {
        PartyKind::Person => EvaluatedPartyKind::Person,
        PartyKind::Organization => EvaluatedPartyKind::Organization,
    };
    let input = PartyQualityInput::try_new(kind, party.display_name().to_owned())
        .map_err(|e| worker::Error::RustError(e.code))?;
    let rules =
        builtin_party_name_review_rules_v1().map_err(|e| worker::Error::RustError(e.code))?;
    let results = rules.evaluate(&input);
    let failed_count = results.iter().filter(|result| !result.passed()).count() as u32;
    let checks = rules
        .rules()
        .iter()
        .zip(results.iter())
        .map(|(rule, result)| RuleCheck {
            rule_key: result.rule_key().as_str().to_owned(),
            title: rule.title().to_owned(),
            remediation: rule.remediation_guidance().to_owned(),
            severity_code: match rule.severity() {
                QualitySeverity::Info => "info",
                QualitySeverity::Warning => "warning",
                QualitySeverity::Error => "error",
                QualitySeverity::Critical => "critical",
            },
            passed: result.passed(),
            reason_code: result.reason_code().to_owned(),
        })
        .collect();
    Ok(Some(Evaluation {
        party,
        rule_set_version: rules.version_id().as_str().to_owned(),
        checks,
        failed_count,
    }))
}
