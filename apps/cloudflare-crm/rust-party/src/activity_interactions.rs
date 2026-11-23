//! D1 adapter for immutable crm-activities Interaction facts.
//! Original Activities domain validates the record. One insert trigger creates
//! the record, original receipt and audit atomically in the existing D1.
use super::{access, bind, now_nanos};
use crate::d1_write::{self, Confirmation, Receipt, WriteResult, rejected};
use crate::diagnostics::{MutationFailure, ReceiptUncertainty};
use crm_activities::interactions::{CreateInteraction, Interaction};
use crm_module_sdk::ErrorCategory;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use wasm_bindgen::JsValue;
use worker::{D1Database, Result};

#[derive(Deserialize)]
struct Row {
    state: String,
}

#[derive(Deserialize)]
struct PageRow {
    id: String,
    occurred_at_key: String,
    state: String,
}

pub(super) struct Saved {
    pub interaction: Interaction,
    pub replayed: bool,
    pub original_request_id: Option<String>,
}

const UNCONFIRMED: &str = "ACTIVITIES_INTERACTION_WRITE_UNCONFIRMED";

fn uncertain(_: worker::Error) -> MutationFailure {
    MutationFailure::CommitUnconfirmed {
        code: UNCONFIRMED,
        reason: ReceiptUncertainty::StorageUnavailable,
    }
}

fn decode(state: &str) -> Result<Interaction> {
    let item: Interaction = serde_json::from_str(state)
        .map_err(|_| worker::Error::RustError("Invalid Interaction state".into()))?;
    item.verify_stored()
        .map_err(|error| worker::Error::RustError(error.code))?;
    Ok(item)
}

pub(super) async fn receipt(db: &D1Database, tenant: &str, key: &str) -> Result<Option<Receipt>> {
    bind(db,
        "SELECT digest,state,original_request_id FROM crm_activity_interaction_receipts WHERE tenant=? AND key=?",
        &[JsValue::from_str(tenant),JsValue::from_str(key)])?
        .first::<Receipt>(None).await
}

pub(super) async fn get(
    db: &D1Database, tenant: &str, id: &str,
) -> Result<Option<Interaction>> {
    bind(db,
        "SELECT state FROM crm_activity_interactions WHERE tenant=? AND id=?",
        &[JsValue::from_str(tenant),JsValue::from_str(id)])?
        .first::<Row>(None).await?
        .map(|row| decode(&row.state)).transpose()
}

fn replay(row: Receipt, digest: &str, id: &str) -> WriteResult<Saved> {
    if row.digest != digest {
        return Err(rejected("IDEMPOTENCY_CONFLICT", ErrorCategory::Conflict));
    }
    let item = decode(&row.state).map_err(|_| MutationFailure::CommitUnconfirmed {
        code: UNCONFIRMED,
        reason: ReceiptUncertainty::DecodeFailed,
    })?;
    if item.interaction_id != id {
        return Err(rejected("IDEMPOTENCY_CONFLICT", ErrorCategory::Conflict));
    }
    Ok(Saved { interaction:item, replayed:true, original_request_id:row.original_request_id })
}

pub(super) async fn create(
    input: CreateInteraction,
    key: &str,
    db: &D1Database,
    actor: &access::Actor,
    request_id: &str,
) -> WriteResult<Saved> {
    let tenant = &actor.tenant;
    if !(8..=120).contains(&key.len()) {
        return Err(rejected("IDEMPOTENCY_KEY_REQUIRED", ErrorCategory::InvalidArgument));
    }
    let digest = format!("{:x}", Sha256::digest(
        serde_json::to_vec(&json!([
            "create_interaction", &input.interaction_id, &input.account_id,
            &input.deal_id, input.kind, &input.text, input.occurred_at_nanos,
            &actor.identity,
        ])).map_err(|e| uncertain(worker::Error::RustError(e.to_string())))?
    ));
    if let Some(prior) = receipt(db, tenant, key).await.map_err(uncertain)? {
        return replay(prior, &digest, &input.interaction_id);
    }

    let item = Interaction::create(input, &actor.identity, now_nanos())
        .map_err(|error| rejected(error.code, ErrorCategory::InvalidArgument))?;
    let account = bind(db,
        "SELECT id FROM crm_account_records WHERE tenant=? AND id=?",
        &[JsValue::from_str(tenant), JsValue::from_str(&item.account_id)])
        .map_err(uncertain)?.first::<serde_json::Value>(None).await.map_err(uncertain)?;
    if account.is_none() {
        return Err(rejected("ACTIVITIES_INTERACTION_ACCOUNT_NOT_FOUND",ErrorCategory::NotFound));
    }
    if let Some(deal) = item.deal_id.as_deref() {
        #[derive(Deserialize)]
        struct DealAccount { account_id: String }
        let row = bind(db,
            "SELECT account_id FROM crm_sales_deal_records WHERE tenant=? AND id=?",
            &[JsValue::from_str(tenant), JsValue::from_str(deal)])
            .map_err(uncertain)?.first::<DealAccount>(None).await.map_err(uncertain)?;
        match row {
            None => return Err(rejected("ACTIVITIES_INTERACTION_DEAL_NOT_FOUND",ErrorCategory::NotFound)),
            Some(row) if row.account_id != item.account_id =>
                return Err(rejected("ACTIVITIES_INTERACTION_DEAL_ACCOUNT_MISMATCH",ErrorCategory::InvalidArgument)),
            _ => {}
        }
    }

    let state = serde_json::to_string(&item)
        .map_err(|e| uncertain(worker::Error::RustError(e.to_string())))?;
    let stamp = format!("{:020}",item.occurred_at_nanos);
    let result = bind(db,
        concat!("INSERT INTO crm_activity_interactions(",
          "tenant,id,account_id,deal_id,state,occurred_at_key,idem_key,idem_digest,request_id,actor)",
          " VALUES(?,?,?,?,?,?,?,?,?,?) ON CONFLICT(tenant,id) DO NOTHING"),
        &[JsValue::from_str(tenant),JsValue::from_str(&item.interaction_id),
          JsValue::from_str(&item.account_id),
          item.deal_id.as_deref().map(JsValue::from_str).unwrap_or(JsValue::NULL),
          JsValue::from_str(&state),JsValue::from_str(&stamp),
          JsValue::from_str(key),JsValue::from_str(&digest),
          JsValue::from_str(request_id),JsValue::from_str(&actor.identity)])
          .map_err(uncertain)?.run().await;
    match d1_write::confirm(result,receipt(db,tenant,key),
        "ACTIVITIES_INTERACTION_CREATE_CONFLICT",UNCONFIRMED).await?
    {
        Confirmation::Applied => Ok(Saved {
            interaction:item,replayed:false,original_request_id:Some(request_id.to_owned()),
        }),
        Confirmation::Replayed(prior) => replay(prior,&digest,&item.interaction_id),
    }
}

pub(super) async fn list(
    db: &D1Database,
    tenant: &str,
    account_id: &str,
    deal_id: &str,
    cursor: &str,
    size: usize,
) -> Result<(Vec<Interaction>,Option<String>)> {
    let (after_time,after_id) = if cursor.is_empty() {
        (String::new(),String::new())
    } else {
        let (timestamp,id) = cursor.split_once('|')
            .ok_or_else(||worker::Error::RustError("INVALID_INTERACTION_CURSOR".into()))?;
        if timestamp.len()!=20 || !timestamp.bytes().all(|ch|ch.is_ascii_digit()) ||
            crm_activities::interactions::validate_identifier(id).is_err()
        {
            return Err(worker::Error::RustError("INVALID_INTERACTION_CURSOR".into()));
        }
        (timestamp.to_owned(),id.to_owned())
    };
    let rows = bind(db,
      concat!("SELECT id,occurred_at_key,state FROM crm_activity_interactions ",
        "WHERE tenant=? AND account_id=? AND (?='' OR deal_id=?) ",
        "AND (?='' OR occurred_at_key<? OR (occurred_at_key=? AND id<?)) ",
        "ORDER BY occurred_at_key DESC,id DESC LIMIT ?"),
      &[JsValue::from_str(tenant),JsValue::from_str(account_id),
        JsValue::from_str(deal_id),JsValue::from_str(deal_id),
        JsValue::from_str(&after_time),JsValue::from_str(&after_time),
        JsValue::from_str(&after_time),JsValue::from_str(&after_id),
        JsValue::from_f64((size+1) as f64)])?
        .all().await?.results::<PageRow>()?;
    let more=rows.len()>size;
    let mut interactions=Vec::with_capacity(rows.len().min(size));
    let mut next=None;
    for (index,row) in rows.into_iter().enumerate().take(size) {
        if more && index+1==size {
            next=Some(format!("{}|{}",row.occurred_at_key,row.id));
        }
        interactions.push(decode(&row.state)?);
    }
    Ok((interactions,next))
}
