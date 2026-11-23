//! D1 adapter for the ORIGINAL crm_sales::SalesDealView state machine.
//! The signed transport supplies tenant/actor; D1 trigger commits audit and
//! replay receipt atomically with exactly one insert/update.
use super::bind;
use crm_sales::{DealStatus, SalesDealView, SalesOwnerScope};
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
    pub status: Option<DealStatus>,
    pub account_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_filter: Option<SalesOwnerScope>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct UpdateInput {
    pub view_id: String,
    pub expected_version: i64,
    pub name: String,
    pub pipeline_id: Option<String>,
    pub status: Option<DealStatus>,
    pub account_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_filter: Option<SalesOwnerScope>,
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
use super::d1_write::Receipt;
pub(super) struct Saved {
    pub value: SalesDealView,
    pub replayed: bool,
    pub(super) original_request_id: Option<String>,
}
pub(super) use super::d1_write::OwnerFailure as Failure;
use connectrpc::ErrorCode;
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
        "SELECT digest,state,original_request_id FROM crm_sales_saved_view_receipts WHERE tenant=? AND key=?",
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
        return Ok(Err(Failure::domain(
            "SALES_VIEW_PIPELINE_NOT_FOUND",
            ErrorCode::NotFound,
        )));
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
    let replay = |prior: Receipt| {
        if prior.digest != hash {
            return Ok(Err(Failure::domain(
                "IDEMPOTENCY_CONFLICT",
                ErrorCode::Aborted,
            )));
        }
        Ok(Ok(Saved {
            value: decode(&prior.state)?,
            replayed: true,
            original_request_id: prior.original_request_id,
        }))
    };
    if let Some(prior) = receipt(db, tenant, key).await? {
        return replay(prior);
    }
    let view = match SalesDealView::create(
        input.view_id,
        input.name,
        input.pipeline_id,
        input.status,
        input.account_id,
    )
    .and_then(|view| view.with_owner_filter(input.owner_filter))
    {
        Ok(view) => view,
        Err(e) => return Ok(Err(Failure::domain(e.code, ErrorCode::InvalidArgument))),
    };
    if let Err(e) = check_pipeline(db, tenant, view.pipeline_id.as_deref()).await? {
        return Ok(Err(e));
    }
    let state = view
        .encode()
        .map_err(|e| worker::Error::RustError(e.code))?;
    let inserted = bind(db,
        "INSERT INTO crm_sales_saved_view_records(tenant,id,state,version,idem_key,idem_digest,request_id,actor,last_action) VALUES(?,?,?,?,?,?,?,?,?) ON CONFLICT(tenant,id) DO NOTHING",
        &[
            JsValue::from_str(tenant), JsValue::from_str(&view.view_id),
            JsValue::from_str(&state), JsValue::from_f64(view.version as f64),
            JsValue::from_str(key), JsValue::from_str(&hash),
            JsValue::from_str(request_id), JsValue::from_str(actor),
            JsValue::from_str("sales_view.create"),
        ],
    )?.run().await;
    match super::d1_write::confirm(
        inserted,
        receipt(db, tenant, key),
        "SALES_VIEW_CREATE_CONFLICT",
        "SALES_VIEW_WRITE_UNCONFIRMED",
    )
    .await
    {
        Ok(super::d1_write::Confirmation::Applied) => {}
        Ok(super::d1_write::Confirmation::Replayed(saved)) => return replay(saved),
        Err(failure) => return Ok(Err(Failure::Write(failure))),
    }
    Ok(Ok(Saved {
        value: view,
        replayed: false,
        original_request_id: Some(request_id.to_owned()),
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
    let replay = |prior: Receipt| {
        if prior.digest != hash {
            return Ok(Err(Failure::domain(
                "IDEMPOTENCY_CONFLICT",
                ErrorCode::Aborted,
            )));
        }
        Ok(Ok(Saved {
            value: decode(&prior.state)?,
            replayed: true,
            original_request_id: prior.original_request_id,
        }))
    };
    let current = get(db, tenant, &input.view_id).await;
    if let Some(prior) = receipt(db, tenant, key).await? {
        return replay(prior);
    }
    let Some(existing) = current? else {
        return Ok(Err(Failure::domain(
            "SALES_VIEW_NOT_FOUND",
            ErrorCode::NotFound,
        )));
    };
    let changed = match existing
        .reconfigure(
            input.expected_version,
            input.name,
            input.pipeline_id,
            input.status,
            input.account_id,
            input.archived,
        )
        .and_then(|view| view.with_owner_filter(input.owner_filter))
    {
        Ok(view) => view,
        Err(e) => {
            return Ok(Err(Failure::domain(
                e.code,
                if e.category == crm_module_sdk::ErrorCategory::Conflict {
                    ErrorCode::Aborted
                } else {
                    ErrorCode::InvalidArgument
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
    match super::d1_write::confirm(
        result,
        receipt(db, tenant, key),
        "SALES_VIEW_VERSION_CONFLICT",
        "SALES_VIEW_WRITE_UNCONFIRMED",
    )
    .await
    {
        Ok(super::d1_write::Confirmation::Applied) => {}
        Ok(super::d1_write::Confirmation::Replayed(saved)) => return replay(saved),
        Err(failure) => return Ok(Err(Failure::Write(failure))),
    }
    Ok(Ok(Saved {
        value: changed,
        replayed: false,
        original_request_id: Some(request_id.to_owned()),
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
