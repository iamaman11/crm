//! Sales routing persistence adapter. SQL supplies facts and checks snapshots;
//! the original Rust Sales domain is the only rule matcher and dispatcher.
use super::{bind, sales_deals, sales_pipelines, sales_territories};
use crm_module_sdk::{ErrorCategory, SdkError};
use crm_sales::{
    SalesDispatchMode, SalesOwnerScope, SalesRoutingCandidate, SalesRoutingDecision,
    SalesRoutingFacts, SalesRoutingRule, select_sales_route,
};
use sales_territories::{Kind, Record};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use wasm_bindgen::JsValue;
use worker::{D1Database, Result};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AutomaticRouting {
    pub territory_id: Option<String>,
    pub expected_decision: Option<String>,
}

fn conflict(code: &'static str) -> SdkError {
    SdkError::new(
        code,
        ErrorCategory::Conflict,
        false,
        "Sales routing changed.",
    )
}

fn json_error(e: serde_json::Error) -> worker::Error {
    worker::Error::RustError(e.to_string())
}

// Ordered JSON of stored rule states. The read and commit guard share the
// exact projection; there is no duplicated rule matching in SQL.
const RULES: &str = "SELECT COALESCE(json_group_array(state),'[]') AS snapshot FROM (SELECT state FROM crm_sales_routing_rule_records WHERE tenant=? AND json_extract(state,'$.archived')=0 ORDER BY id)";
const LOAD: &str = "SELECT COALESCE(json_group_array(json_object('email',email,'open_deals',open_deals)),'[]') AS snapshot FROM (SELECT u.email AS email,(SELECT COUNT(*) FROM crm_sales_deal_records d WHERE d.tenant=u.tenant_id AND json_extract(d.state,'$.status')='open' AND json_extract(d.state,'$.owner.actor_id')=u.email) AS open_deals FROM crm_access_teams tm JOIN json_each(tm.state,'$.member_emails') m JOIN crm_users u ON u.tenant_id=tm.tenant AND u.email=m.value AND u.active=1 WHERE tm.tenant=? AND tm.id=? AND json_extract(tm.state,'$.active')=1 AND u.email NOT LIKE 'service:%' ORDER BY u.email)";

#[derive(Deserialize)]
struct SnapshotRow {
    snapshot: String,
}

async fn snapshot(db: &D1Database, sql: &str, args: &[JsValue]) -> Result<String> {
    bind(db, sql, args)?
        .first::<SnapshotRow>(None)
        .await?
        .map(|row| row.snapshot)
        .ok_or_else(|| worker::Error::RustError("SALES_ROUTING_SNAPSHOT_MISSING".into()))
}

pub(super) struct Resolution {
    pub decision: SalesRoutingDecision,
    pub fingerprint: String,
    rules: String,
    assignment_version: i64,
    territory: Option<(String, i64)>,
    pipeline_id: String,
    pipeline_version: i64,
    account_id: String,
    workload: Option<(String, String)>,
}

impl Resolution {
    pub(super) fn append_guard(&self, sql: &mut String, values: &mut Vec<JsValue>, tenant: &str) {
        sql.push_str(&format!(" AND ({RULES})=?"));
        values.extend([JsValue::from_str(tenant), JsValue::from_str(&self.rules)]);
        sql.push_str(" AND COALESCE((SELECT version FROM crm_sales_account_assignment_records WHERE tenant=? AND id=?),0)=?");
        values.extend([
            JsValue::from_str(tenant),
            JsValue::from_str(&self.account_id),
            JsValue::from_f64(self.assignment_version as f64),
        ]);
        if let Some((id, version)) = &self.territory {
            sql.push_str(" AND EXISTS (SELECT 1 FROM crm_sales_territory_records WHERE tenant=? AND id=? AND version=? AND json_extract(state,'$.archived')=0)");
            values.extend([
                JsValue::from_str(tenant),
                JsValue::from_str(id),
                JsValue::from_f64(*version as f64),
            ]);
        }
        sql.push_str(" AND EXISTS (SELECT 1 FROM crm_sales_pipeline_records WHERE tenant=? AND id=? AND version=? AND json_extract(state,'$.archived')=0)");
        values.extend([
            JsValue::from_str(tenant),
            JsValue::from_str(&self.pipeline_id),
            JsValue::from_f64(self.pipeline_version as f64),
        ]);
        if let Some((team, load)) = &self.workload {
            sql.push_str(&format!(" AND ({LOAD})=?"));
            values.extend([
                JsValue::from_str(tenant),
                JsValue::from_str(team),
                JsValue::from_str(load),
            ]);
        }
        sql.push_str(&sales_deals::owner_guard(Some(&self.decision.target)));
        sales_deals::owner_parameters(values, tenant, Some(&self.decision.target));
    }
}

pub(super) async fn resolve(
    db: &D1Database,
    tenant: &str,
    facts: &SalesRoutingFacts,
) -> Result<std::result::Result<Resolution, SdkError>> {
    if let Err(error) = facts.validate() {
        return Ok(Err(error));
    }
    if super::accounts::load(db, tenant, &facts.account_id)
        .await?
        .is_none()
    {
        return Ok(Err(conflict("SALES_ACCOUNT_NOT_FOUND")));
    }
    let Some(pipeline) = sales_pipelines::get(db, tenant, &facts.pipeline_id).await? else {
        return Ok(Err(conflict("SALES_STAGE_NOT_IN_CATALOG")));
    };
    if pipeline.archived || !pipeline.stage(&facts.stage_id).is_some_and(|s| !s.archived) {
        return Ok(Err(conflict("SALES_STAGE_NOT_IN_CATALOG")));
    }
    let assignment =
        match sales_territories::get(db, tenant, Kind::Assignment, &facts.account_id).await? {
            Some(Record::Assignment(value)) => Some(value),
            None => None,
            _ => {
                return Err(worker::Error::RustError(
                    "SALES_ROUTING_ASSIGNMENT_INVALID".into(),
                ));
            }
        };
    let effective = match facts.effective_territory(assignment.as_ref()) {
        Ok(value) => value,
        Err(error) => return Ok(Err(error)),
    };
    let territory = if let Some(id) = effective {
        match sales_territories::get(db, tenant, Kind::Territory, id).await? {
            Some(Record::Territory(value)) => Some(value),
            None => None,
            _ => {
                return Err(worker::Error::RustError(
                    "SALES_ROUTING_TERRITORY_INVALID".into(),
                ));
            }
        }
    } else {
        None
    };
    let rules = snapshot(db, RULES, &[JsValue::from_str(tenant)]).await?;
    let raw: Vec<String> = serde_json::from_str(&rules).map_err(json_error)?;
    let rules_typed = raw
        .iter()
        .map(|s| SalesRoutingRule::decode(s))
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| worker::Error::RustError(e.code))?;
    let selection =
        match select_sales_route(facts, &rules_typed, assignment.as_ref(), territory.as_ref()) {
            Ok(value) => value,
            Err(error) => return Ok(Err(error)),
        };
    let eligible = sales_deals::eligible_owner(db, tenant, &selection.decision.target).await?;
    let workload = match (&selection.decision.target, selection.decision.dispatch) {
        (SalesOwnerScope::Team(team), SalesDispatchMode::LeastOpenDeals) => Some((
            team.clone(),
            snapshot(
                db,
                LOAD,
                &[JsValue::from_str(tenant), JsValue::from_str(team)],
            )
            .await?,
        )),
        _ => None,
    };
    let candidates: Vec<SalesRoutingCandidate> = match &workload {
        Some((_, raw)) => serde_json::from_str(raw).map_err(json_error)?,
        None => Vec::new(),
    };
    let decision = match selection.dispatch(eligible, &candidates) {
        Ok(value) => value,
        Err(error) => return Ok(Err(error)),
    };
    let assignment_version = assignment.as_ref().map_or(0, |a| a.version);
    let territory = territory.map(|t| (t.territory_id, t.version));
    let fingerprint = format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&(
                facts,
                &decision,
                &rules,
                assignment_version,
                &territory,
                pipeline.version,
                &workload
            ))
            .map_err(json_error)?
        )
    );
    Ok(Ok(Resolution {
        decision,
        fingerprint,
        rules,
        assignment_version,
        territory,
        pipeline_id: facts.pipeline_id.clone(),
        pipeline_version: pipeline.version,
        account_id: facts.account_id.clone(),
        workload,
    }))
}
