//! Read-only multi-owner Account card composed by the ONE Worker.
//! This is not an Account business capability or a second authoritative state:
//! each original owner keeps its own codec/aggregate, and every D1 query is
//! constrained to the verified tenant. No other owner's table is modified.
use crm_customer_accounts::AccountPartyRole;
use serde::Deserialize;
use wasm_bindgen::JsValue;
use worker::{D1Database, Result};
use super::{accounts, bind};
use crm_customer_360_query_adapter::account_overview::{AccountOverview,Sources};

#[derive(Deserialize)]
struct OverviewState { state: String }

/*
 * Read-only original Account view. Existing original owners retain all
 * validation and persisted codecs; this is only a tenant-scoped D1 projection.
 * No second aggregate, no mirrored tables and no computed consent grants.
 */
async fn overview_states(db: &D1Database, sql: &str, bindings: &[JsValue])
    -> Result<Vec<String>> {
    // Fetch one extra row so a truncated card cannot masquerade as complete.
    let rows = bind(db, sql, bindings)?.all().await?.results::<OverviewState>()?;
    Ok(rows.into_iter().map(|row|row.state).collect())
}
pub(super) async fn overview(db: &D1Database, tenant: &str, id: &str) -> Result<Option<AccountOverview>> {
    let Some(account) = accounts::load(db, tenant, id).await? else {
        return Ok(None);
    };
    let Some(primary) = account.party_associations().iter()
        .find(|association| matches!(association.role(), AccountPartyRole::Primary)) else {
        return Err(worker::Error::RustError("CUSTOMER_ACCOUNTS_PRIMARY_PARTY_MISSING".into()));
    };
    let primary_id = primary.party().as_str();
    let Some(party) = super::parties::stored(db, tenant, primary_id).await? else {
        return Err(worker::Error::RustError("CUSTOMER_ACCOUNTS_PRIMARY_PARTY_MISSING".into()));
    };
    let contact_states = overview_states(db,
        "SELECT state FROM crm_contact_point_records WHERE tenant=? AND party_id=? ORDER BY id LIMIT 101",
        &[JsValue::from_str(tenant), JsValue::from_str(primary_id)]).await?;
    let relationship_states = overview_states(db,
        "SELECT state FROM crm_party_relationship_records WHERE tenant=? AND (from_party=? OR to_party=?) ORDER BY id LIMIT 101",
        &[JsValue::from_str(tenant), JsValue::from_str(primary_id),
            JsValue::from_str(primary_id)]).await?;
    let deal_states = overview_states(db,
        "SELECT state FROM crm_sales_deal_records WHERE tenant=? AND account_id=? ORDER BY id LIMIT 101",
        &[JsValue::from_str(tenant), JsValue::from_str(id)]).await?;
    let task_states = overview_states(db,
        "SELECT state FROM crm_activity_task_records WHERE tenant=? AND account_id=? ORDER BY id LIMIT 101",
        &[JsValue::from_str(tenant), JsValue::from_str(id)]).await?;
    let contact_points = contact_states.iter().map(|state|
        super::contact_points::decode(state))
        .collect::<Result<Vec<_>>>()?;
    let relationships = relationship_states.iter().map(|state|
        super::party_relationships::decoded(state))
        .collect::<Result<Vec<_>>>()?;
    let deals = deal_states.iter().map(|state|
        super::sales_deals::decode(state))
        .collect::<Result<Vec<_>>>()?;
    let tasks = task_states.iter().map(|state|
        super::activity_tasks::decode(state))
        .collect::<Result<Vec<_>>>()?;
    AccountOverview::compose(Sources{account,primary_party:party,contact_points,relationships,deals,tasks})
        .map(Some).map_err(|e|worker::Error::RustError(e.code))
}
