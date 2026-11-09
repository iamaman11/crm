//! Original crm-parties HTTP/D1 adapter. Business invariants belong only to
//! the existing crm-parties crate; this file owns transport/persistence only.
use super::{bind, error, now_nanos, response};
use crm_parties::{CreateParty, Party, PartyId, PartyKind, UpdateParty,
    decode_party_state, encode_party_state};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use sha2::{Digest, Sha256};
use wasm_bindgen::JsValue;
use worker::{D1Database, Method, Request, Response, Result};

#[derive(Deserialize)]
struct CreateInput {
    id: String,
    kind: String,
    name: String,
}
#[derive(Deserialize)]
struct UpdateInput {
    expected_version: i64,
    name: String,
}
#[derive(Deserialize)]
struct Stored {
    state: String,
}
#[derive(Deserialize)]
struct Receipt {
    digest: String,
    state: String,
}
#[derive(Serialize, Deserialize)]
struct PartyRow {
    id: String,
    state: String,
}


pub(super) fn view(party: &Party) -> Value {
    json!({
        "id": party.party_id().as_str(),
        "kind": match party.kind() { PartyKind::Person => "person", PartyKind::Organization => "organization" },
        "name": party.display_name(),
        "version": party.version(),
    })
}

fn digest(kind: &str, id: &str, name: &str, version: i64) -> String {
    let bytes = serde_json::to_vec(&json!([kind,id,name,version])).expect("static JSON");
    format!("{:x}", Sha256::digest(bytes))
}

pub(super) async fn stored(db: &D1Database, tenant: &str, id: &str) -> Result<Option<Party>> {
    let row = bind(db, "SELECT state FROM crm_party_records WHERE tenant = ? AND id = ?",
        &[JsValue::from_str(tenant), JsValue::from_str(id)])?
        .first::<Stored>(None).await?;
    row.map(|r| decode_party_state(r.state.as_bytes())
        .map_err(|e| worker::Error::RustError(e.code)))
        .transpose()
}
/// Only the original Party adapter writes Party records. This atomic D1
/// transaction uses the exact same Party::create and receipt schema as normal
/// creation. Any conflict aborts the entire batch without partial imports.
pub(super) async fn import_batch(db: &D1Database, tenant: &str,
    rows: Vec<(String, PartyKind, String)>, source_key: &str,
    trace: &str) -> Result<Response> {
    let mut statements=Vec::with_capacity(rows.len());
    let mut ids=BTreeSet::new();
    let mut receipts=0;
    for (index,(id,kind,name)) in rows.iter().enumerate() {
        if !ids.insert(id.clone()) { return error("IMPORT_DUPLICATE_ID",422); }
        let key=format!("cdo-{source_key}-{index}");
        let hash=digest("create",id,name,0);
        if let Some(old)=previous(db,tenant,&key).await? {
            if old.digest!=hash {return error("IDEMPOTENCY_CONFLICT",409);}
            receipts+=1;
            continue;
        }
        if stored(db,tenant,id).await?.is_some() {
            return error("IMPORT_PARTY_ALREADY_EXISTS",409);
        }
        let party=Party::create(CreateParty {
            party_id: PartyId::try_new(id.clone())
                .map_err(|e| worker::Error::RustError(e.code))?,
            kind:*kind, display_name:name.clone(), occurred_at_unix_nanos:now_nanos(),
        }).map_err(|e| worker::Error::RustError(e.code))?;
        let state=String::from_utf8(encode_party_state(&party)
            .map_err(|e| worker::Error::RustError(e.code))?)
            .map_err(|e| worker::Error::RustError(e.to_string()))?;
        statements.push(bind(db,
            "INSERT INTO crm_party_records(tenant,id,state,version,idem_key,idem_digest,request_id) VALUES(?,?,?,?,?,?,?)",
            &[JsValue::from_str(tenant),JsValue::from_str(id),
              JsValue::from_str(&state),JsValue::from_f64(1.0),
              JsValue::from_str(&key),JsValue::from_str(&hash),
              JsValue::from_str(trace)])?);
    }
    if receipts>0 {
        return if receipts==rows.len() {
            response(json!({"created":0,"replayed":rows.len(),"source":"original_crm_parties"}),200)
        } else {error("IMPORT_PARTIAL_REPLAY_CONFLICT",409)};
    }
    if db.batch(statements).await.is_err() {
        return error("IMPORT_ATOMIC_WRITE_CONFLICT",409);
    }
    response(json!({"created":rows.len(),"replayed":0,"source":"original_crm_parties"}),201)
}

async fn previous(db: &D1Database, tenant: &str, key: &str) -> Result<Option<Receipt>> {
    bind(db, "SELECT digest, state FROM crm_party_receipts WHERE tenant = ? AND key = ?",
        &[JsValue::from_str(tenant), JsValue::from_str(key)])?
        .first::<Receipt>(None).await
}

async fn create(mut req: Request, db: D1Database, tenant: &str, request_id: &str) -> Result<Response> {
    let key = req.headers().get("idempotency-key")?.unwrap_or_default();
    if key.len() < 8 || key.len() > 120 { return error("IDEMPOTENCY_KEY_REQUIRED", 422); }
    let input = match req.json::<CreateInput>().await {
        Ok(value) => value, Err(_) => return error("INVALID_INPUT", 422),
    };
    let kind = match input.kind.as_str() {
        "person" => PartyKind::Person,
        "organization" => PartyKind::Organization,
        _ => return error("INVALID_KIND", 422),
    };
    let hash = digest("create", &input.id, &input.name, 0);
    if let Some(old) = previous(&db, tenant, &key).await? {
        if old.digest != hash { return error("IDEMPOTENCY_CONFLICT", 409); }
        let p = decode_party_state(old.state.as_bytes())
            .map_err(|e| worker::Error::RustError(e.code))?;
        return response(json!({"item":view(&p), "replayed":true}), 200);
    }
    let id = match PartyId::try_new(input.id) {
        Ok(id) => id, Err(_) => return error("PARTIES_PARTY_ID_INVALID", 422),
    };
    let party = match Party::create(CreateParty {
        party_id: id, kind, display_name: input.name, occurred_at_unix_nanos: now_nanos(),
    }) {
        Ok(p) => p, Err(e) => return error(&e.code, 422),
    };
    let state = String::from_utf8(encode_party_state(&party)
        .map_err(|e| worker::Error::RustError(e.code))?)
        .map_err(|e| worker::Error::RustError(e.to_string()))?;
    // SQLite triggers atomically create the audit and idempotency receipt.
    // Any unique-key failure aborts the *entire* insert and both trigger effects.
    let r = bind(&db, "INSERT INTO crm_party_records(tenant,id,state,version,idem_key,idem_digest,request_id) VALUES(?,?,?,?,?,?,?)",
        &[JsValue::from_str(tenant), JsValue::from_str(party.party_id().as_str()),
          JsValue::from_str(&state), JsValue::from_f64(1.0),
          JsValue::from_str(&key), JsValue::from_str(&hash), JsValue::from_str(request_id)])?.run().await;
    if r.is_err() { return error("PARTIES_PARTY_CREATE_CONFLICT", 409); }
    response(json!({"item":view(&party), "replayed":false}), 201)
}
async fn update(mut req: Request, db: D1Database, tenant: &str, id: &str, request_id: &str) -> Result<Response> {
    let key = req.headers().get("idempotency-key")?.unwrap_or_default();
    if key.len() < 8 || key.len() > 120 { return error("IDEMPOTENCY_KEY_REQUIRED", 422); }
    let input = match req.json::<UpdateInput>().await {
        Ok(value) => value, Err(_) => return error("INVALID_INPUT", 422),
    };
    let hash = digest("update", id, &input.name, input.expected_version);
    if let Some(old) = previous(&db, tenant, &key).await? {
        if old.digest != hash { return error("IDEMPOTENCY_CONFLICT", 409); }
        let p = decode_party_state(old.state.as_bytes())
            .map_err(|e| worker::Error::RustError(e.code))?;
        return response(json!({"item":view(&p), "replayed":true}), 200);
    }
    let Some(mut party) = stored(&db, tenant, id).await? else {
        return error("NOT_FOUND", 404);
    };
    if let Err(e) = party.apply_update(UpdateParty {
        expected_version: input.expected_version,
        display_name: input.name,
        occurred_at_unix_nanos: now_nanos(),
    }) { return error(&e.code, if e.code == "PARTIES_PARTY_VERSION_CONFLICT" { 409 } else { 422 }); }
    let state = String::from_utf8(encode_party_state(&party)
        .map_err(|e| worker::Error::RustError(e.code))?)
        .map_err(|e| worker::Error::RustError(e.to_string()))?;
    let updated = bind(&db, "UPDATE crm_party_records SET state = ?, version = ?, idem_key = ?, idem_digest = ?, request_id = ? WHERE tenant = ? AND id = ? AND version = ?",
        &[JsValue::from_str(&state), JsValue::from_f64(party.version() as f64),
          JsValue::from_str(&key), JsValue::from_str(&hash), JsValue::from_str(request_id),
          JsValue::from_str(tenant), JsValue::from_str(id),
          JsValue::from_f64(input.expected_version as f64)])?.run().await;
    let Ok(result) = updated else { return error("PARTIES_PARTY_UPDATE_CONFLICT", 409); };
    // D1's changes count may include the audit and receipt triggers. A versioned
    // UPDATE is successful when it changed at least one row, not exactly one.
    // A missed optimistic-version predicate changes zero rows and fires no triggers.
    if result.meta()?.and_then(|m| m.changes).unwrap_or(0) == 0 {
        return error("PARTIES_PARTY_VERSION_CONFLICT", 409);
    }
    response(json!({"item":view(&party), "replayed":false}), 200)
}
async fn read(db: D1Database, tenant: &str, id: &str) -> Result<Response> {
    match stored(&db, tenant, id).await? {
        Some(party) => response(json!({"item":view(&party)}), 200),
        None => error("NOT_FOUND", 404),
    }
}
async fn list(db: D1Database, tenant: &str) -> Result<Response> {
    let rows = bind(&db,
        "SELECT id,state FROM crm_party_records WHERE tenant = ? ORDER BY id LIMIT 100",
        &[JsValue::from_str(tenant)])?.all().await?.results::<PartyRow>()?;
    let mut items = Vec::new();
    for row in rows {
        let p = decode_party_state(row.state.as_bytes())
            .map_err(|e| worker::Error::RustError(e.code))?;
        items.push(view(&p));
    }
    response(json!({"items":items}), 200)
}

pub(super) async fn route(req:Request, db:D1Database, tenant:&str, trace:&str)->Result<Response>{
    let path=req.path();
    let method=req.method();
    if path == "/api/parties" {
        return match method {
            Method::Post => create(req, db, tenant, trace).await,
            Method::Get => list(db, tenant).await,
            _ => error("NOT_FOUND", 404),
        };
    }
    if let Some(id)=path.strip_prefix("/api/parties/") {
        if id.is_empty() || id.contains('/') { return error("NOT_FOUND",404); }
        return match method {
            Method::Get => read(db,tenant,id).await,
            Method::Patch => update(req,db,tenant,id,trace).await,
            _ => error("NOT_FOUND",404),
        };
    }
    error("NOT_FOUND",404)
}
