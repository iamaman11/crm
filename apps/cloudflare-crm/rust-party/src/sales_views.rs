//! D1 adapter for the ORIGINAL crm_sales::SalesDealView state machine.
//! The signed transport supplies tenant/actor; D1 trigger commits audit and
//! replay receipt atomically with exactly one insert/update.
use super::bind;
use crm_sales::{SalesDealView, SalesViewStatus};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use wasm_bindgen::JsValue;
use worker::{D1Database, Result};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CreateInput {
    pub view_id: String,
    pub name: String,
    pub pipeline_id: Option<String>,
    pub status: Option<SalesViewStatus>,
    pub account_id: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct UpdateInput {
    pub view_id: String,
    pub expected_version: i64,
    pub name: String,
    pub pipeline_id: Option<String>,
    pub status: Option<SalesViewStatus>,
    pub account_id: Option<String>,
    pub archived: bool,
}
#[derive(Deserialize)]
struct StateRow {
    state: String,
}
#[derive(Deserialize)]
struct PageRow {
    id: String,
    state: String,
}
#[derive(Deserialize)]
struct Receipt {
    digest: String,
    state: String,
}
pub(super) struct Saved {
    pub value: SalesDealView,
    pub replayed: bool,
}
pub(super) struct Failure {
    pub code: String,
    pub status: u16,
}
fn fail(code: impl Into<String>, status: u16) -> Failure {
    Failure {
        code: code.into(),
        status,
    }
}
fn digest<T: Serialize>(kind: &str, input: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&json!([kind, input])).expect("typed saved-view request")
        )
    )
}
fn decode(state: &str) -> Result<SalesDealView> {
    SalesDealView::decode(state.as_bytes()).map_err(|e| worker::Error::RustError(e.code))
}
async fn receipt(db: &D1Database, tenant: &str, key: &str) -> Result<Option<Receipt>> {
    bind(
        db,
        "SELECT digest,state FROM crm_sales_saved_view_receipts WHERE tenant=? AND key=?",
        &[JsValue::from_str(tenant), JsValue::from_str(key)],
    )?
    .first::<Receipt>(None)
    .await
}
pub(super) async fn get(db: &D1Database, tenant: &str, id: &str) -> Result<Option<SalesDealView>> {
    bind(
        db,
        "SELECT state FROM crm_sales_saved_view_records WHERE tenant=? AND id=?",
        &[JsValue::from_str(tenant), JsValue::from_str(id)],
    )?
    .first::<StateRow>(None)
    .await?
    .map(|row| decode(&row.state))
    .transpose()
}
async fn check_pipeline(
    db: &D1Database,
    tenant: &str,
    pipeline: Option<&str>,
) -> Result<std::result::Result<(), Failure>> {
    if let Some(id) = pipeline
        && super::sales_pipelines::get(db, tenant, id).await?.is_none()
    {
        return Ok(Err(fail("SALES_VIEW_PIPELINE_NOT_FOUND", 404)));
    }
    Ok(Ok(()))
}
pub(super) async fn create(
    input: CreateInput,
    key: &str,
    db: &D1Database,
    tenant: &str,
    actor: &str,
    request_id: &str,
) -> Result<std::result::Result<Saved, Failure>> {
    let hash = digest("sales_view.create", &input);
    if let Some(prior) = receipt(db, tenant, key).await? {
        if prior.digest != hash {
            return Ok(Err(fail("IDEMPOTENCY_CONFLICT", 409)));
        }
        return Ok(Ok(Saved {
            value: decode(&prior.state)?,
            replayed: true,
        }));
    }
    let view = match SalesDealView::create(
        input.view_id,
        input.name,
        input.pipeline_id,
        input.status,
        input.account_id,
    ) {
        Ok(view) => view,
        Err(e) => return Ok(Err(fail(e.code, 422))),
    };
    if let Err(e) = check_pipeline(db, tenant, view.pipeline_id.as_deref()).await? {
        return Ok(Err(e));
    }
    let state = view
        .encode()
        .map_err(|e| worker::Error::RustError(e.code))?;
    let inserted = bind(db,
        "INSERT INTO crm_sales_saved_view_records(tenant,id,state,version,idem_key,idem_digest,request_id,actor,last_action) VALUES(?,?,?,?,?,?,?,?,?)",
        &[
            JsValue::from_str(tenant), JsValue::from_str(&view.view_id),
            JsValue::from_str(&state), JsValue::from_f64(view.version as f64),
            JsValue::from_str(key), JsValue::from_str(&hash),
            JsValue::from_str(request_id), JsValue::from_str(actor),
            JsValue::from_str("sales_view.create"),
        ],
    )?.run().await;
    if inserted.is_err() {
        if let Some(prior) = receipt(db, tenant, key).await?
            && prior.digest == hash
        {
            return Ok(Ok(Saved {
                value: decode(&prior.state)?,
                replayed: true,
            }));
        }
        return Ok(Err(fail("SALES_VIEW_CREATE_CONFLICT", 409)));
    }
    Ok(Ok(Saved {
        value: view,
        replayed: false,
    }))
}
pub(super) async fn update(
    input: UpdateInput,
    key: &str,
    db: &D1Database,
    tenant: &str,
    actor: &str,
    request_id: &str,
) -> Result<std::result::Result<Saved, Failure>> {
    let hash = digest("sales_view.update", &input);
    if let Some(prior) = receipt(db, tenant, key).await? {
        if prior.digest != hash {
            return Ok(Err(fail("IDEMPOTENCY_CONFLICT", 409)));
        }
        return Ok(Ok(Saved {
            value: decode(&prior.state)?,
            replayed: true,
        }));
    }
    let Some(existing) = get(db, tenant, &input.view_id).await? else {
        return Ok(Err(fail("SALES_VIEW_NOT_FOUND", 404)));
    };
    let changed = match existing.reconfigure(
        input.expected_version,
        input.name,
        input.pipeline_id,
        input.status,
        input.account_id,
        input.archived,
    ) {
        Ok(view) => view,
        Err(e) => {
            return Ok(Err(fail(
                e.code,
                if e.category == crm_module_sdk::ErrorCategory::Conflict {
                    409
                } else {
                    422
                },
            )));
        }
    };
    if let Err(e) = check_pipeline(db, tenant, changed.pipeline_id.as_deref()).await? {
        return Ok(Err(e));
    }
    let state = changed
        .encode()
        .map_err(|e| worker::Error::RustError(e.code))?;
    let result = bind(db,
        "UPDATE crm_sales_saved_view_records SET state=?,version=?,idem_key=?,idem_digest=?,request_id=?,actor=?,last_action='sales_view.update' WHERE tenant=? AND id=? AND version=?",
        &[
            JsValue::from_str(&state),
            JsValue::from_f64(changed.version as f64),
            JsValue::from_str(key), JsValue::from_str(&hash),
            JsValue::from_str(request_id), JsValue::from_str(actor),
            JsValue::from_str(tenant), JsValue::from_str(&changed.view_id),
            JsValue::from_f64(input.expected_version as f64),
        ]
    )?.run().await;
    let Ok(done) = result else {
        return Ok(Err(fail("SALES_VIEW_WRITE_CONFLICT", 409)));
    };
    if done.meta()?.and_then(|meta| meta.changes).unwrap_or(0) == 0 {
        return Ok(Err(fail("SALES_VIEW_VERSION_CONFLICT", 409)));
    }
    Ok(Ok(Saved {
        value: changed,
        replayed: false,
    }))
}
pub(super) async fn list(
    db: &D1Database,
    tenant: &str,
    after: &str,
    limit: usize,
    include_archived: bool,
) -> Result<(Vec<SalesDealView>, Option<String>)> {
    let mut sql =
        "SELECT id,state FROM crm_sales_saved_view_records WHERE tenant=? AND id>?".to_owned();
    if !include_archived {
        sql.push_str(" AND json_extract(state,'$.archived')=0");
    }
    sql.push_str(" ORDER BY id LIMIT ?");
    let mut rows = bind(
        db,
        &sql,
        &[
            JsValue::from_str(tenant),
            JsValue::from_str(after),
            JsValue::from_f64((limit + 1) as f64),
        ],
    )?
    .all()
    .await?
    .results::<PageRow>()?;
    let has_more = rows.len() > limit;
    rows.truncate(limit);
    let next = if has_more {
        rows.last().map(|row| row.id.clone())
    } else {
        None
    };
    let views = rows
        .iter()
        .map(|row| decode(&row.state))
        .collect::<Result<Vec<_>>>()?;
    Ok((views, next))
}
