//! Read-only multi-owner Account card composed by the ONE Worker.
//! This is not an Account business capability or a second authoritative state:
//! each original owner keeps its own codec/aggregate, and every D1 query is
//! constrained to the verified tenant. No other owner's table is modified.
use crm_customer_accounts::AccountPartyRole;
use serde::Deserialize;
use serde_json::json;
use wasm_bindgen::JsValue;
use worker::{D1Database, Response, Result};
use super::{accounts, bind, error, response};

#[derive(Deserialize)]
struct OverviewState { state: String }

/*
 * Read-only original Account view. Existing original owners retain all
 * validation and persisted codecs; this is only a tenant-scoped D1 projection.
 * No second aggregate, no mirrored tables and no computed consent grants.
 */
const OVERVIEW_LIMIT: usize = 100;
async fn overview_states(db: &D1Database, sql: &str, bindings: &[JsValue])
    -> Result<(Vec<String>, bool)> {
    // Fetch one extra row so a truncated card cannot masquerade as complete.
    let mut rows = bind(db, sql, bindings)?.all().await?.results::<OverviewState>()?;
    let has_more = rows.len() > OVERVIEW_LIMIT;
    rows.truncate(OVERVIEW_LIMIT);
    Ok((rows.into_iter().map(|row| row.state).collect(), has_more))
}
pub(super) async fn overview(db: &D1Database, tenant: &str, id: &str) -> Result<Response> {
    let Some(account) = accounts::load(db, tenant, id).await? else {
        return error("NOT_FOUND", 404);
    };
    let Some(primary) = account.party_associations().iter()
        .find(|association| matches!(association.role(), AccountPartyRole::Primary)) else {
        return error("CUSTOMER_ACCOUNTS_PRIMARY_PARTY_MISSING", 503);
    };
    let primary_id = primary.party().as_str();
    let Some(party) = super::stored(db, tenant, primary_id).await? else {
        return error("CUSTOMER_ACCOUNTS_PRIMARY_PARTY_MISSING", 503);
    };
    let (contact_states, contacts_truncated) = overview_states(db,
        "SELECT state FROM crm_contact_point_records WHERE tenant=? AND party_id=? ORDER BY id LIMIT 101",
        &[JsValue::from_str(tenant), JsValue::from_str(primary_id)]).await?;
    let (relationship_states, relationships_truncated) = overview_states(db,
        "SELECT state FROM crm_party_relationship_records WHERE tenant=? AND (from_party=? OR to_party=?) ORDER BY id LIMIT 101",
        &[JsValue::from_str(tenant), JsValue::from_str(primary_id),
            JsValue::from_str(primary_id)]).await?;
    let (deal_states, deals_truncated) = overview_states(db,
        "SELECT state FROM crm_sales_deal_records WHERE tenant=? AND account_id=? ORDER BY id LIMIT 101",
        &[JsValue::from_str(tenant), JsValue::from_str(id)]).await?;
    let (task_states, tasks_truncated) = overview_states(db,
        "SELECT state FROM crm_activity_task_records WHERE tenant=? AND account_id=? ORDER BY id LIMIT 101",
        &[JsValue::from_str(tenant), JsValue::from_str(id)]).await?;
    let contact_points = contact_states.iter().map(|state|
        super::contact_points::decode(state).map(|item|super::contact_points::view(&item)))
        .collect::<Result<Vec<_>>>()?;
    let relationships = relationship_states.iter().map(|state|
        super::party_relationships::decoded(state).map(|item|super::party_relationships::view(&item)))
        .collect::<Result<Vec<_>>>()?;
    let deals = deal_states.iter().map(|state|
        super::sales_deals::decode(state).map(|item|super::sales_deals::view(&item)))
        .collect::<Result<Vec<_>>>()?;
    let tasks = task_states.iter().map(|state|
        super::activity_tasks::decode(state).map(|item|super::activity_tasks::view(&item)))
        .collect::<Result<Vec<_>>>()?;
    response(json!({
        "account":accounts::view(&account), "primary_party":super::view(&party),
        "contact_points":contact_points, "party_relationships":relationships,
        "deals":deals, "tasks":tasks,
        "scope":{"contact_points":"primary_party_only",
                 "party_relationships":"primary_party_only",
                 "limit_per_collection":OVERVIEW_LIMIT},
        "truncated":{"contact_points":contacts_truncated,
                     "party_relationships":relationships_truncated,
                     "deals":deals_truncated,"tasks":tasks_truncated}
    }), 200)
}

