//! Read-only original crm-data-quality Party name review. The preset and
//! evaluation live in the ORIGINAL crate; this is transport + tenant-bound D1.
//! No persisted finding, unconfigured quality score or second data owner.
use crm_data_quality::{
    EvaluatedPartyKind, PartyQualityInput, builtin_party_name_review_rules_v1,
};
use crm_parties::PartyKind;
use serde_json::{Value, json};
use worker::{D1Database, Method, Request, Response, Result};
use super::{error, parties, response};

pub(super) async fn route(req: Request, db: D1Database, tenant: &str) -> Result<Response> {
    if req.method() != Method::Get { return error("NOT_FOUND", 404); }
    let path = req.path();
    let Some(id) = path.strip_prefix("/api/data-quality/parties/")
        .and_then(|tail| tail.strip_suffix("/evaluate")) else {
        return error("NOT_FOUND", 404);
    };
    if id.is_empty() || id.contains('/') { return error("NOT_FOUND", 404); }
    // Authoritative Party is read within the signed Access tenant only.
    let Some(party) = parties::stored(&db, tenant, id).await? else {
        return error("NOT_FOUND", 404);
    };
    let kind = match party.kind() {
        PartyKind::Person => EvaluatedPartyKind::Person,
        PartyKind::Organization => EvaluatedPartyKind::Organization,
    };
    let input = PartyQualityInput::try_new(kind, party.display_name().to_owned())
        .map_err(|e| worker::Error::RustError(e.code))?;
    let rules = builtin_party_name_review_rules_v1()
        .map_err(|e| worker::Error::RustError(e.code))?;
    let results = rules.evaluate(&input);
    let failed = results.iter().filter(|outcome| !outcome.passed()).count();
    let checks: Vec<Value> = rules.rules().iter().zip(results.iter())
        .map(|(rule, result)| json!({
            "rule_key": result.rule_key().as_str(),
            "title": rule.title(),
            "remediation": rule.remediation_guidance(),
            "severity": rule.severity(),
            "passed": result.passed(),
            "reason_code": result.reason_code()
        })).collect();
    response(json!({
        "party": parties::view(&party),
        "rule_set_version": rules.version_id().as_str(),
        "policy": "builtin_party_name_review_v1",
        "checks": checks,
        "failed": failed,
        "checked": results.len(),
        "evaluation": "READ_ONLY_CURRENT_PARTY",
        "persisted_findings": "NOT_CONNECTED",
        "completeness_score": "NOT_COMPUTED"
    }), 200)
}
