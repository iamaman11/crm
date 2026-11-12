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
/// Only this Party adapter persists Party creation, including CSV import.
/// D1 batch is atomic: all Party states, receipts and audits or none.
pub(super) async fn import_receipt_matches(
    db:&D1Database,tenant:&str,key:&str,id:&str,kind:PartyKind,name:&str
)->Result<bool>{
    let Some(old)=previous(db,tenant,key).await? else {return Ok(false);};
    if old.digest!=digest("create",id,name,0){return Ok(false);}
    let party=decode_party_state(old.state.as_bytes())
        .map_err(|e|worker::Error::RustError(e.code))?;
    Ok(party.party_id().as_str()==id&&party.kind()==kind)
}

pub(super) struct ImportSaved {
    pub(super) created:usize,
    pub(super) replayed:usize,
    pub(super) operation_replayed:bool,
}
#[derive(Deserialize)]
struct ImportReceipt {source_sha256:String,row_count:usize}
async fn import_operation(db:&D1Database,tenant:&str,key:&str)->Result<Option<ImportReceipt>>{
    bind(db,"SELECT source_sha256,row_count FROM crm_party_import_receipts WHERE tenant=? AND key=?",
        &[JsValue::from_str(tenant),JsValue::from_str(key)])?.first::<ImportReceipt>(None).await
}
pub(super) async fn import_batch(
    db:&D1Database,tenant:&str,rows:Vec<(String,PartyKind,String,String)>,
    operation_key:&str,source_hash:&str,trace:&str,
)->Result<std::result::Result<ImportSaved,MutationFailure>>{
    if !(8..=120).contains(&operation_key.len())||rows.is_empty(){
        return Ok(Err(failed("IMPORT_PRECONDITION_INVALID",422)));
    }
    if let Some(old)=import_operation(db,tenant,operation_key).await?{
        if old.source_sha256!=source_hash||old.row_count!=rows.len(){
            return Ok(Err(failed("IDEMPOTENCY_CONFLICT",409)));
        }
        return Ok(Ok(ImportSaved{created:0,replayed:old.row_count,operation_replayed:true}));
    }
    let mut statements=Vec::with_capacity(rows.len()+1);
    let mut ids=BTreeSet::new();
    let mut receipts=0;
    for (id,kind,name,key) in &rows{
        if !ids.insert(id.clone()){return Ok(Err(failed("IMPORT_DUPLICATE_TARGET_ID",422)));}
        let hash=digest("create",id,name,0);
        if let Some(old)=previous(db,tenant,key).await?{
            if old.digest!=hash{return Ok(Err(failed("IDEMPOTENCY_CONFLICT",409)));}
            let party=decode_party_state(old.state.as_bytes())
                .map_err(|e|worker::Error::RustError(e.code))?;
            if party.party_id().as_str()!=id||party.kind()!=*kind{
                return Ok(Err(failed("IMPORT_RECEIPT_ID_CONFLICT",409)));
            }
            receipts+=1;
            continue;
        }
        if stored(db,tenant,id).await?.is_some(){
            return Ok(Err(failed("IMPORT_TARGET_PARTY_CONFLICT",409)));
        }
        let party=Party::create(CreateParty{
            party_id:PartyId::try_new(id.clone())
                .map_err(|e|worker::Error::RustError(e.code))?,
            kind:*kind,display_name:name.clone(),occurred_at_unix_nanos:now_nanos(),
        }).map_err(|e|worker::Error::RustError(e.code))?;
        let state=String::from_utf8(encode_party_state(&party)
            .map_err(|e|worker::Error::RustError(e.code))?)
            .map_err(|e|worker::Error::RustError(e.to_string()))?;
        statements.push(bind(db,
            "INSERT INTO crm_party_records(tenant,id,state,version,idem_key,idem_digest,request_id) VALUES(?,?,?,?,?,?,?)",
            &[JsValue::from_str(tenant),JsValue::from_str(id),
              JsValue::from_str(&state),JsValue::from_f64(1.0),
              JsValue::from_str(key),JsValue::from_str(&hash),
              JsValue::from_str(trace)])?);
    }
    if receipts>0&&receipts!=rows.len(){
        return Ok(Err(failed("IMPORT_PARTIAL_REPLAY_CONFLICT",409)));
    }
    // Operation-key receipt and all original Party writes commit in one D1 batch.
    // The UNIQUE(tenant,key) constraint also serializes concurrent retry attempts.
    statements.push(bind(db,
        "INSERT INTO crm_party_import_receipts(tenant,key,source_sha256,row_count,request_id) VALUES(?,?,?,?,?)",
        &[JsValue::from_str(tenant),JsValue::from_str(operation_key),
          JsValue::from_str(source_hash),JsValue::from_f64(rows.len() as f64),
          JsValue::from_str(trace)])?);
    if db.batch(statements).await.is_err(){
        if let Some(old)=import_operation(db,tenant,operation_key).await?{
            if old.source_sha256==source_hash&&old.row_count==rows.len(){
                return Ok(Ok(ImportSaved{created:0,replayed:old.row_count,operation_replayed:true}));
            }
        }
        return Ok(Err(failed("IMPORT_ATOMIC_WRITE_CONFLICT",409)));
    }
    Ok(Ok(ImportSaved{created:rows.len()-receipts,replayed:receipts,operation_replayed:receipts>0}))
}

async fn previous(db: &D1Database, tenant: &str, key: &str) -> Result<Option<Receipt>> {
    bind(db, "SELECT digest, state FROM crm_party_receipts WHERE tenant = ? AND key = ?",
        &[JsValue::from_str(tenant), JsValue::from_str(key)])?
        .first::<Receipt>(None).await
}

// The ONLY Party persistence implementation. Both the historical (blocked)
// HTTP call and the generated Connect service use these original Rust owner
// commands. Do not add another Party aggregate or duplicate D1 writes.
pub(super) struct MutationOutcome {
    pub(super) party: Party,
    pub(super) replayed: bool,
}
pub(super) struct MutationFailure {
    pub(super) code: String,
    pub(super) status: u16,
}
fn failed(code: impl Into<String>, status: u16) -> MutationFailure {
    MutationFailure { code: code.into(), status }
}
pub(super) async fn create_typed(
    db: &D1Database, tenant: &str, id: String, kind: PartyKind,
    name: String, key: &str, request_id: &str,
) -> Result<std::result::Result<MutationOutcome, MutationFailure>> {
    if !(8..=120).contains(&key.len()) {
        return Ok(Err(failed("IDEMPOTENCY_KEY_REQUIRED", 422)));
    }
    let hash = digest("create", &id, &name, 0);
    if let Some(old) = previous(db, tenant, key).await? {
        if old.digest != hash { return Ok(Err(failed("IDEMPOTENCY_CONFLICT", 409))); }
        let party = decode_party_state(old.state.as_bytes())
            .map_err(|e| worker::Error::RustError(e.code))?;
        if party.party_id().as_str()!=id || party.kind()!=kind {
            return Ok(Err(failed("IDEMPOTENCY_CONFLICT", 409)));
        }
        return Ok(Ok(MutationOutcome { party, replayed: true }));
    }
    let id = match PartyId::try_new(id) {
        Ok(value) => value,
        Err(_) => return Ok(Err(failed("PARTIES_PARTY_ID_INVALID", 422))),
    };
    let party = match Party::create(CreateParty {
        party_id: id, kind, display_name: name, occurred_at_unix_nanos: now_nanos(),
    }) {
        Ok(value) => value,
        Err(e) => return Ok(Err(failed(e.code, 422))),
    };
    let state = String::from_utf8(encode_party_state(&party)
        .map_err(|e| worker::Error::RustError(e.code))?)
        .map_err(|e| worker::Error::RustError(e.to_string()))?;
    // Existing D1 triggers write the audit and idempotency receipt atomically.
    let result = bind(db,
        "INSERT INTO crm_party_records(tenant,id,state,version,idem_key,idem_digest,request_id) VALUES(?,?,?,?,?,?,?)",
        &[JsValue::from_str(tenant), JsValue::from_str(party.party_id().as_str()),
          JsValue::from_str(&state), JsValue::from_f64(1.0),
          JsValue::from_str(key), JsValue::from_str(&hash),
          JsValue::from_str(request_id)])?.run().await;
    if result.is_err() { return Ok(Err(failed("PARTIES_PARTY_CREATE_CONFLICT", 409))); }
    Ok(Ok(MutationOutcome { party, replayed: false }))
}
pub(super) async fn update_typed(
    db: &D1Database, tenant: &str, id: &str, name: String,
    expected_version: i64, key: &str, request_id: &str,
) -> Result<std::result::Result<MutationOutcome, MutationFailure>> {
    if !(8..=120).contains(&key.len()) {
        return Ok(Err(failed("IDEMPOTENCY_KEY_REQUIRED", 422)));
    }
    let hash = digest("update", id, &name, expected_version);
    if let Some(old) = previous(db, tenant, key).await? {
        if old.digest != hash { return Ok(Err(failed("IDEMPOTENCY_CONFLICT", 409))); }
        let party = decode_party_state(old.state.as_bytes())
            .map_err(|e| worker::Error::RustError(e.code))?;
        if party.party_id().as_str()!=id {
            return Ok(Err(failed("IDEMPOTENCY_CONFLICT", 409)));
        }
        return Ok(Ok(MutationOutcome { party, replayed: true }));
    }
    let Some(mut party) = stored(db, tenant, id).await? else {
        return Ok(Err(failed("NOT_FOUND", 404)));
    };
    if let Err(e) = party.apply_update(UpdateParty {
        expected_version, display_name: name, occurred_at_unix_nanos: now_nanos(),
    }) {
        let status = if e.code == "PARTIES_PARTY_VERSION_CONFLICT" { 409 } else { 422 };
        return Ok(Err(failed(e.code, status)));
    }
    let state = String::from_utf8(encode_party_state(&party)
        .map_err(|e| worker::Error::RustError(e.code))?)
        .map_err(|e| worker::Error::RustError(e.to_string()))?;
    let result = bind(db,
        "UPDATE crm_party_records SET state=?,version=?,idem_key=?,idem_digest=?,request_id=? WHERE tenant=? AND id=? AND version=?",
        &[JsValue::from_str(&state), JsValue::from_f64(party.version() as f64),
          JsValue::from_str(key), JsValue::from_str(&hash),
          JsValue::from_str(request_id), JsValue::from_str(tenant),
          JsValue::from_str(id), JsValue::from_f64(expected_version as f64)])?.run().await;
    let Ok(changed) = result else {
        return Ok(Err(failed("PARTIES_PARTY_UPDATE_CONFLICT", 409)));
    };
    if changed.meta()?.and_then(|meta| meta.changes).unwrap_or(0) == 0 {
        return Ok(Err(failed("PARTIES_PARTY_VERSION_CONFLICT", 409)));
    }
    Ok(Ok(MutationOutcome { party, replayed: false }))
}
pub(super) async fn list_page_typed(
    db: &D1Database, tenant: &str, after: &str,
    page_size: usize, kind: Option<PartyKind>,
) -> Result<(Vec<Party>, Option<String>)> {
    // The original codec stores kind as top-level snake_case JSON.
    // Filter before LIMIT so a page cursor never skips matching records.
    let filter = kind.map(|value| match value {
        PartyKind::Person => "person",
        PartyKind::Organization => "organization",
    });
    let query = if filter.is_some() {
        "SELECT id,state FROM crm_party_records WHERE tenant=? AND id>? AND json_extract(state,'$.kind')=? ORDER BY id LIMIT ?"
    } else {
        "SELECT id,state FROM crm_party_records WHERE tenant=? AND id>? ORDER BY id LIMIT ?"
    };
    let mut params = vec![JsValue::from_str(tenant), JsValue::from_str(after)];
    if let Some(value) = filter { params.push(JsValue::from_str(value)); }
    params.push(JsValue::from_f64((page_size + 1) as f64));
    let rows = bind(db, query, &params)?.all().await?.results::<PartyRow>()?;
    let has_more = rows.len() > page_size;
    let next = if has_more { rows.get(page_size-1).map(|r| r.id.clone()) } else { None };
    let mut items = Vec::new();
    for row in rows.iter().take(page_size) {
        let party = decode_party_state(row.state.as_bytes())
            .map_err(|e| worker::Error::RustError(e.code))?;
        items.push(party);
    }
    Ok((items, next))
}
async fn create(mut req: Request, db: D1Database, tenant: &str, trace: &str) -> Result<Response> {
    let key = req.headers().get("idempotency-key")?.unwrap_or_default();
    let input = match req.json::<CreateInput>().await {
        Ok(value) => value, Err(_) => return error("INVALID_INPUT", 422),
    };
    let kind = match input.kind.as_str() {
        "person" => PartyKind::Person,
        "organization" => PartyKind::Organization,
        _ => return error("INVALID_KIND", 422),
    };
    match create_typed(&db, tenant, input.id, kind, input.name, &key, trace).await? {
        Ok(result) => response(json!({"item":view(&result.party),
            "replayed":result.replayed}), if result.replayed { 200 } else { 201 }),
        Err(failure) => error(&failure.code, failure.status),
    }
}
async fn update(mut req: Request, db: D1Database, tenant: &str,
    id: &str, trace: &str) -> Result<Response> {
    let key = req.headers().get("idempotency-key")?.unwrap_or_default();
    let input = match req.json::<UpdateInput>().await {
        Ok(value) => value, Err(_) => return error("INVALID_INPUT", 422),
    };
    match update_typed(&db, tenant, id, input.name, input.expected_version,
        &key, trace).await? {
        Ok(result) => response(json!({"item":view(&result.party),
            "replayed":result.replayed}), 200),
        Err(failure) => error(&failure.code, failure.status),
    }
}
async fn read(db: D1Database, tenant: &str, id: &str) -> Result<Response> {
    match stored(&db, tenant, id).await? {
        Some(party) => response(json!({"item":view(&party)}), 200),
        None => error("NOT_FOUND", 404),
    }
}
async fn list(db: D1Database, tenant: &str) -> Result<Response> {
    let (parties, _) = list_page_typed(&db, tenant, "", 100, None).await?;
    response(json!({"items":parties.iter().map(view).collect::<Vec<_>>()}), 200)
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
