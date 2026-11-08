//! Native workers-rs proof using the *original* crm-parties owner.
//! Not a second deployed CRM: deliberately denies all requests unless built
//! for the loopback-only CI test with the ci-local feature.
//! No JS reproduction of Party::create, apply_update or encode/decode.
use crm_parties::{
    CreateParty, Party, PartyId, PartyKind, UpdateParty,
    decode_party_state, encode_party_state,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use wasm_bindgen::JsValue;
use worker::*;

#[cfg(feature = "native")]
mod access;

#[cfg(feature = "native")]
fn native_request_id() -> Result<String> {
    use js_sys::{Function, Reflect};
    use wasm_bindgen::JsCast;
    let crypto = Reflect::get(&js_sys::global(), &JsValue::from_str("crypto"))
        .map_err(|_| worker::Error::RustError("crypto unavailable".into()))?;
    let func = Reflect::get(&crypto, &JsValue::from_str("randomUUID"))
        .map_err(|_| worker::Error::RustError("randomUUID unavailable".into()))?
        .dyn_into::<Function>()
        .map_err(|_| worker::Error::RustError("crypto.randomUUID unavailable".into()))?;
    func.call0(&crypto).map_err(|_| worker::Error::RustError("request_id generation failed".into()))?.as_string()
        .ok_or_else(|| worker::Error::RustError("invalid request_id".into()))
}

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

fn response(body: Value, status: u16) -> Result<Response> {
    Ok(Response::from_json(&body)?.with_status(status))
}
fn error(code: &str, status: u16) -> Result<Response> {
    let mut reply = response(json!({"code": code, "error": code}), status)?;
    reply.headers_mut().set("x-crm-error-code", code)?;
    Ok(reply)
}
fn view(party: &Party) -> Value {
    json!({
        "id": party.party_id().as_str(),
        "kind": match party.kind() { PartyKind::Person => "person", PartyKind::Organization => "organization" },
        "name": party.display_name(),
        "version": party.version(),
    })
}
fn now_nanos() -> i64 {
    (Date::now().as_millis() as i64).saturating_mul(1_000_000)
}
fn digest(kind: &str, id: &str, name: &str, version: i64) -> String {
    let bytes = serde_json::to_vec(&json!([kind,id,name,version])).expect("static JSON");
    format!("{:x}", Sha256::digest(bytes))
}
fn bind(db: &D1Database, sql: &str, values: &[JsValue]) -> Result<worker::d1::D1PreparedStatement> {
    db.prepare(sql).bind(values)
}
async fn stored(db: &D1Database, tenant: &str, id: &str) -> Result<Option<Party>> {
    let row = bind(db, "SELECT state FROM crm_party_records WHERE tenant = ? AND id = ?",
        &[JsValue::from_str(tenant), JsValue::from_str(id)])?
        .first::<Stored>(None).await?;
    row.map(|r| decode_party_state(r.state.as_bytes())
        .map_err(|e| worker::Error::RustError(e.code)))
        .transpose()
}
async fn previous(db: &D1Database, tenant: &str, key: &str) -> Result<Option<Receipt>> {
    bind(db, "SELECT digest, state FROM crm_party_receipts WHERE tenant = ? AND key = ?",
        &[JsValue::from_str(tenant), JsValue::from_str(key)])?
        .first::<Receipt>(None).await
}
async fn create(mut req: Request, db: D1Database, tenant: &str) -> Result<Response> {
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
    let r = bind(&db, "INSERT INTO crm_party_records(tenant,id,state,version,idem_key,idem_digest) VALUES(?,?,?,?,?,?)",
        &[JsValue::from_str(tenant), JsValue::from_str(party.party_id().as_str()),
          JsValue::from_str(&state), JsValue::from_f64(1.0),
          JsValue::from_str(&key), JsValue::from_str(&hash)])?.run().await;
    if r.is_err() { return error("PARTIES_PARTY_CREATE_CONFLICT", 409); }
    response(json!({"item":view(&party), "replayed":false}), 201)
}
async fn update(mut req: Request, db: D1Database, tenant: &str, id: &str) -> Result<Response> {
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
    let updated = bind(&db, "UPDATE crm_party_records SET state = ?, version = ?, idem_key = ?, idem_digest = ? WHERE tenant = ? AND id = ? AND version = ?",
        &[JsValue::from_str(&state), JsValue::from_f64(party.version() as f64),
          JsValue::from_str(&key), JsValue::from_str(&hash),
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
async fn dispatch(req: Request, env: Env, tenant: &str) -> Result<Response> {
    let path = req.path();
    let method = req.method();
    let db = env.d1("DB")?;
    if path == "/api/parties" {
        return match method {
            Method::Post => create(req, db, tenant).await,
            Method::Get => list(db, tenant).await,
            _ => error("NOT_FOUND", 404),
        };
    }
    if let Some(id) = path.strip_prefix("/api/parties/") {
        if id.is_empty() || id.contains('/') {
            return error("NOT_FOUND", 404);
        }
        return match method {
            Method::Get => read(db, tenant, id).await,
            Method::Patch => update(req, db, tenant, id).await,
            _ => error("NOT_FOUND", 404),
        };
    }
    error("NOT_FOUND", 404)
}

#[event(fetch)]
pub async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    #[cfg(any(all(feature = "ci-local", feature = "integrated"),
              all(feature = "ci-local", feature = "native"),
              all(feature = "integrated", feature = "native")))]
    compile_error!("ci-local, integrated and native are mutually exclusive");

    // Only the compiled-in test mode trusts a loopback CI-only header.
    #[cfg(feature = "ci-local")]
    {
        if env.var("CRM_CI_LOCAL_ONLY").ok().map(|v| v.to_string()) != Some("yes".into()) {
            return error("AUTH_REQUIRED", 403);
        }
        let Some(tenant) = req.headers().get("x-crm-ci-tenant")?
            .filter(|s| s == "tenant-a" || s == "tenant-b") else {
            return error("AUTH_REQUIRED", 403);
        };
        return dispatch(req, env, &tenant).await;
    }

    // The `integrated` build is import-only, called inside the single trusted
    // Worker module. The public JS entrypoint verifies Cloudflare Access,
    // deletes user-supplied internal headers and assigns this tenant value.
    // Never publish the generated Rust shim as an independent Worker.
    #[cfg(feature = "integrated")]
    {
        let Some(tenant) = req.headers().get("x-crm-native-tenant")?
            .filter(|s| !s.is_empty() && s.len() <= 120 &&
                s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')) else {
            return error("AUTH_REQUIRED", 403);
        };
        return dispatch(req, env, &tenant).await;
    }

    #[cfg(feature = "native")]
    {
        // One correlation boundary, shared by Access, native domain and D1.
        // Never trust a caller-supplied request identifier or tenant header.
        let request_id = native_request_id()?;
        let started = Date::now().as_millis();
        let path = req.path();
        let method = req.method();
        let operation = match (path.as_str(), method) {
            ("/api/parties", Method::Get) => "parties.list",
            ("/api/parties", Method::Post) => "parties.create",
            (p, Method::Get) if p.starts_with("/api/parties/") => "parties.read",
            (p, Method::Patch) if p.starts_with("/api/parties/") => "parties.update",
            _ => "parties.unmatched",
        };
        let (result, phase) = match access::tenant(&req, &env).await {
            Ok(Some(tenant)) => (dispatch(req, env, &tenant).await, "domain_d1"),
            Ok(None) => (error("AUTH_REQUIRED", 401), "access"),
            Err(_) => (error("ACCESS_INTERNAL_ERROR", 503), "access"),
        };
        let mut reply = match result {
            Ok(reply) => reply,
            Err(_) => error("PARTIES_INTERNAL_ERROR", 500)?,
        };
        reply.headers_mut().set("x-request-id", &request_id)?;
        reply.headers_mut().set("x-crm-module", "crm.parties")?;
        let elapsed = Date::now().as_millis().saturating_sub(started);
        console_log!("{}", json!({
            "event":"crm.native.request", "request_id":request_id,
            "module":"crm.parties", "operation":operation, "phase":phase,
            "status":reply.status_code(), "duration_ms":elapsed
        }));
        return Ok(reply);
    }

    #[cfg(not(any(feature = "ci-local", feature = "integrated", feature = "native")))]
    {
        let _ = (req, env);
        error("AUTH_REQUIRED", 403)
    }
}
