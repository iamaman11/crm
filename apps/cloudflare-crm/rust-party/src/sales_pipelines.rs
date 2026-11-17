//! One Cloudflare D1 boundary for the original crm_sales::SalesPipeline owner.
use super::bind;
use crm_sales::{PipelineStage, PipelineStageInput, SalesPipeline};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use wasm_bindgen::JsValue;
use worker::{D1Database, Result};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CreateInput {
    pub pipeline_id: String,
    pub name: String,
    pub stages: Vec<PipelineStageInput>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct UpdateInput {
    pub pipeline_id: String,
    pub expected_version: i64,
    pub name: String,
    pub stages: Vec<PipelineStageInput>,
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
    pub value: SalesPipeline,
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
        Sha256::digest(serde_json::to_vec(&json!([kind, input])).expect("typed pipeline request"))
    )
}
async fn receipt(db: &D1Database, tenant: &str, key: &str) -> Result<Option<Receipt>> {
    bind(
        db,
        "SELECT digest,state FROM crm_sales_pipeline_receipts WHERE tenant=? AND key=?",
        &[JsValue::from_str(tenant), JsValue::from_str(key)],
    )?
    .first::<Receipt>(None)
    .await
}
pub(super) async fn get(db: &D1Database, tenant: &str, id: &str) -> Result<Option<SalesPipeline>> {
    bind(
        db,
        "SELECT state FROM crm_sales_pipeline_records WHERE tenant=? AND id=?",
        &[JsValue::from_str(tenant), JsValue::from_str(id)],
    )?
    .first::<StateRow>(None)
    .await?
    .map(|row| SalesPipeline::decode(row.state.as_bytes()))
    .transpose()
    .map_err(|e| worker::Error::RustError(e.code))
}
/// The original Rust sales catalog alone assigns the current stage rank.
pub(super) async fn active_stage(
    db: &D1Database,
    tenant: &str,
    pipeline_id: &str,
    stage_id: &str,
) -> Result<Option<PipelineStage>> {
    let Some(pipeline) = get(db, tenant, pipeline_id).await? else {
        return Ok(None);
    };
    if pipeline.archived {
        return Ok(None);
    }
    Ok(pipeline
        .stage(stage_id)
        .filter(|stage| !stage.archived)
        .cloned())
}
pub(super) async fn active_stage_ordinal(
    db: &D1Database,
    tenant: &str,
    pipeline_id: &str,
    stage_id: &str,
) -> Result<Option<u16>> {
    Ok(active_stage(db, tenant, pipeline_id, stage_id)
        .await?
        .map(|stage| stage.ordinal))
}
pub(super) async fn create(
    input: CreateInput,
    key: &str,
    db: &D1Database,
    tenant: &str,
    actor: &str,
    request_id: &str,
) -> Result<std::result::Result<Saved, Failure>> {
    let hash = digest("pipeline.create", &input);
    if let Some(prior) = receipt(db, tenant, key).await? {
        if prior.digest != hash {
            return Ok(Err(fail("IDEMPOTENCY_CONFLICT", 409)));
        }
        return Ok(Ok(Saved {
            value: SalesPipeline::decode(prior.state.as_bytes())
                .map_err(|e| worker::Error::RustError(e.code))?,
            replayed: true,
        }));
    }
    let pipeline = match SalesPipeline::create(input.pipeline_id, input.name, input.stages) {
        Ok(pipeline) => pipeline,
        Err(err) => return Ok(Err(fail(err.code, 422))),
    };
    let state = pipeline
        .encode()
        .map_err(|e| worker::Error::RustError(e.code))?;
    let inserted = bind(
        db,
        "INSERT INTO crm_sales_pipeline_records(tenant,id,state,version,idem_key,idem_digest,request_id,actor,last_action) VALUES(?,?,?,?,?,?,?,?,?)",
        &[
            JsValue::from_str(tenant),
            JsValue::from_str(&pipeline.pipeline_id),
            JsValue::from_str(&state),
            JsValue::from_f64(pipeline.version as f64),
            JsValue::from_str(key),
            JsValue::from_str(&hash),
            JsValue::from_str(request_id),
            JsValue::from_str(actor),
            JsValue::from_str("sales_pipeline.create"),
        ],
    )?
    .run()
    .await;
    if inserted.is_err() {
        if let Some(prior) = receipt(db, tenant, key).await?
            && prior.digest == hash
        {
            return Ok(Ok(Saved {
                value: SalesPipeline::decode(prior.state.as_bytes())
                    .map_err(|e| worker::Error::RustError(e.code))?,
                replayed: true,
            }));
        }
        return Ok(Err(fail("SALES_PIPELINE_CREATE_CONFLICT", 409)));
    }
    Ok(Ok(Saved {
        value: pipeline,
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
    let hash = digest("pipeline.update", &input);
    if let Some(prior) = receipt(db, tenant, key).await? {
        if prior.digest != hash {
            return Ok(Err(fail("IDEMPOTENCY_CONFLICT", 409)));
        }
        return Ok(Ok(Saved {
            value: SalesPipeline::decode(prior.state.as_bytes())
                .map_err(|e| worker::Error::RustError(e.code))?,
            replayed: true,
        }));
    }
    let Some(existing) = get(db, tenant, &input.pipeline_id).await? else {
        return Ok(Err(fail("SALES_PIPELINE_NOT_FOUND", 404)));
    };
    let changed = match existing.reconfigure(
        input.expected_version,
        input.name,
        input.archived,
        input.stages,
    ) {
        Ok(pipeline) => pipeline,
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
    let state = changed
        .encode()
        .map_err(|e| worker::Error::RustError(e.code))?;
    let result = bind(
        db,
        "UPDATE crm_sales_pipeline_records SET state=?,version=?,idem_key=?,idem_digest=?,request_id=?,actor=?,last_action='sales_pipeline.update' WHERE tenant=? AND id=? AND version=?",
        &[
            JsValue::from_str(&state),
            JsValue::from_f64(changed.version as f64),
            JsValue::from_str(key),
            JsValue::from_str(&hash),
            JsValue::from_str(request_id),
            JsValue::from_str(actor),
            JsValue::from_str(tenant),
            JsValue::from_str(&changed.pipeline_id),
            JsValue::from_f64(input.expected_version as f64),
        ],
    )?
    .run()
    .await;
    let Ok(done) = result else {
        // D1 prohibits removal/archive of referenced stages and atomically
        // rebases occupied ranks inside the same catalog update.
        return Ok(Err(fail(
            "SALES_PIPELINE_REFERENCED_STAGE_OR_WRITE_CONFLICT",
            409,
        )));
    };
    if done.meta()?.and_then(|meta| meta.changes).unwrap_or(0) == 0 {
        return Ok(Err(fail("SALES_PIPELINE_VERSION_CONFLICT", 409)));
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
) -> Result<(Vec<SalesPipeline>, Option<String>)> {
    let mut sql =
        "SELECT id,state FROM crm_sales_pipeline_records WHERE tenant=? AND id>?".to_owned();
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
    let cursor = if has_more {
        rows.last().map(|row| row.id.clone())
    } else {
        None
    };
    let pipelines = rows
        .iter()
        .map(|row| {
            SalesPipeline::decode(row.state.as_bytes())
                .map_err(|e| worker::Error::RustError(e.code))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok((pipelines, cursor))
}
