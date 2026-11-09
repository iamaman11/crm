//! Sole native workers-rs CRM entrypoint, linked to the original crm-parties owner.
//! ci-local is a strictly isolated workerd acceptance fixture, never a public
//! authentication mechanism. No JS router, duplicate Party rules or second Worker.
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
mod accounts;
#[cfg(feature = "native")]
mod pilot;
#[cfg(feature = "native-ci")]
mod original_domain_probe;

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
fn correlation_id(req: &Request, trusted: Option<&str>) -> Result<String> {
    // Native's independently generated id takes precedence over any client header.
    if let Some(id) = trusted { return Ok(id.to_owned()); }
    // The public native and integrated entries overwrite this internal header.
    // The isolated CI build generates its own identifier if the header is absent.
    let id = req.headers().get("x-crm-internal-request-id")?
        .filter(|value| value.len() == 36 &&
            value.bytes().all(|byte| byte.is_ascii_hexdigit() || byte == b'-'));
    match id { Some(id) => Ok(id), None => native_request_id() }
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
#[cfg(feature = "native")]
#[derive(Deserialize)]
struct SchemaCount { present: i64 }

#[cfg(feature = "native")]
async fn native_doctor(env: &Env, tenant: &str) -> Result<Response> {
    // A truthful live, read-only diagnostic: this probes the active Rust
    // runtime and current D1 tables. CI/browser/Logs proof remains a separate
    // protected release artifact, never an invented live health result.
    const EXPECTED: [&str; 11] = [
        "crm_tenants", "crm_users", "crm_customers", "crm_contacts",
        "crm_deals", "crm_tasks", "crm_audit", "crm_party_records", "crm_party_receipts",
        "crm_account_records", "crm_account_receipts",
    ];
    let db = env.d1("DB")?;
    let bindings: Vec<JsValue> = EXPECTED.iter().map(|name| JsValue::from_str(name)).collect();
    let row = bind(&db,
        "SELECT COUNT(*) AS present FROM sqlite_master WHERE type='table' AND name IN (?,?,?,?,?,?,?,?,?,?,?)",
        &bindings)?.first::<SchemaCount>(None).await?;
    let present = row.map_or(0, |value| value.present);
    let schema_ok = present == EXPECTED.len() as i64;
    let mut read_checks = serde_json::Map::new();
    let mut reads_ok = schema_ok;
    // Query each deployed UI owner's tenant scope directly, with real D1 reads.
    // Zero records is a valid empty workspace, not a failed health check.
    if schema_ok {
        for (module, table, column) in [
            ("customers", "crm_customers", "tenant_id"),
            ("contacts", "crm_contacts", "tenant_id"),
            ("deals", "crm_deals", "tenant_id"),
            ("tasks", "crm_tasks", "tenant_id"),
            ("parties", "crm_party_records", "tenant"),
            ("accounts", "crm_account_records", "tenant"),
        ] {
            // Table and column names are compiled-in constants, not user input.
            let query = format!("SELECT COUNT(*) AS present FROM {table} WHERE {column} = ?");
            let count = match bind(&db, &query, &[JsValue::from_str(tenant)]) {
                Ok(statement) => statement.first::<SchemaCount>(None).await.ok().flatten().map(|x|x.present),
                Err(_) => None,
            };
            if count.is_none() { reads_ok = false; }
            read_checks.insert(module.into(), json!({
                "status": if count.is_some() { "PASS" } else { "FAIL" },
                "visible_records": count,
                "scope": "verified_access_tenant"
            }));
        }
    }
    // Real audit-schema probe; if request_id is absent, the doctor must fail,
    // even when all nine table names still exist.
    let audit_trace = if schema_ok {
        match bind(&db,
            "SELECT COUNT(request_id) AS present FROM crm_audit WHERE tenant_id = ?",
            &[JsValue::from_str(tenant)]) {
            Ok(statement) => statement.first::<SchemaCount>(None).await.ok().flatten().map(|x|x.present),
            Err(_) => None,
        }
    } else { None };
    let trace_ready = audit_trace.is_some();
    let worker_version = env.object_var::<Value>("CF_VERSION_METADATA").ok()
        .and_then(|metadata| metadata.get("id").and_then(Value::as_str).map(str::to_owned));
    let source = env.var("GIT_SHA").ok().map(|value| value.to_string());
    let identity_ok = source.as_deref().is_some_and(|value| value.len()==40
        && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        && worker_version.as_deref().is_some_and(|value| value.len()==36);
    let healthy = schema_ok && reads_ok && trace_ready && identity_ok;
    response(json!({
        "application": "crm-dev",
        "runtime": "workers-rs",
        "entrypoint": "native-workers-rs",
        "source": source.unwrap_or_else(|| "NOT_AVAILABLE".into()),
        "worker_version": worker_version.unwrap_or_else(|| "NOT_AVAILABLE".into()),
        "release": { "source_version": if identity_ok {"PASS"} else {"FAIL"},
            "traffic_percentage": "EXTERNAL_CLOUDFLARE_DEPLOYMENTS_CHECK_REQUIRED",
            "native_only": true },
        "access": {"status": "PASS", "check": "signed JWT and enrolled tenant",
            "principal_scope": "verified_access_tenant"},
        "database": {"status": if schema_ok && reads_ok && trace_ready { "PASS" } else { "FAIL" },
            "tables": {"present": present, "expected": EXPECTED.len()},
            "tenant_reads": read_checks.clone(),
            "audit": {"request_id_column": if trace_ready { "PASS" } else { "FAIL" },
                "tenant_audit_rows_with_id": audit_trace }},
        "modules": {
            "crm.parties": {"read": read_checks.get("parties"), "business_owner": "original-crm-parties"},
            "crm.customer-accounts": {"read":read_checks.get("accounts"),
                "business_owner":"original-crm-customer-accounts",
                "api":"native-workers-rs", "ui":"NOT_CONNECTED",
                "functional_cycle":"EXTERNAL_PROTECTED_CI_CHECK_REQUIRED"},
            "crm.customers": {"read": read_checks.get("customers"), "business_owner": "native-pilot"},
            "crm.contacts": {"read": read_checks.get("contacts"), "business_owner": "native-pilot"},
            "crm.deals": {"read": read_checks.get("deals"), "business_owner": "native-pilot"},
            "crm.tasks": {"read": read_checks.get("tasks"), "business_owner": "native-pilot"},
            "crm.customer360": {"read": "EXTERNAL_BROWSER_CHECK_REQUIRED"}
        },
        "functional_cycle": {
            "browser_buttons": "EXTERNAL_PROTECTED_CI_CHECK_REQUIRED",
            "crud_writes": "EXTERNAL_PROTECTED_CI_CHECK_REQUIRED",
            "d1_request_id": "EXTERNAL_PROTECTED_CI_CHECK_REQUIRED",
            "cloudflare_logs": "EXTERNAL_PROTECTED_CI_CHECK_REQUIRED"
        }
    }), if healthy {200} else {503})
}

async fn dispatch(req: Request, env: Env, tenant: &str, trusted_id: Option<&str>, role: Option<&str>) -> Result<Response> {
    let request_id = correlation_id(&req, trusted_id)?;
    let path = req.path();
    let method = req.method();
    let db = env.d1("DB")?;
    // Explicitly CI-only, signed-Access-gated native workerd proof. This gate
    // performs no D1 writes and makes no claims about module persistence.
    #[cfg(feature = "native-ci")]
    if path == "/api/_ci/original-domain-check" && method == Method::Get {
        let code = original_domain_probe::check();
        return response(json!({
            "result": if code == 0 { "PASS" } else { "FAIL" },
            "code": code,
            "scope": "in-memory original owner domain contracts",
            "owners_executed": ["crm.customer-accounts", "crm.contact-points",
                "crm.sales", "crm.activities"],
            "original_modules_compiled": 13,
            "d1_owner_persistence": "NOT_TESTED",
            "remaining_owners": "NOT_TESTED"
        }), if code == 0 { 200 } else { 503 });
    }
    #[cfg(feature = "native")]
    if path == "/api/accounts" || path.starts_with("/api/accounts/") {
        return accounts::route(req, db, tenant, &request_id).await;
    }
    if path == "/api/parties" {
        return match method {
            Method::Post => create(req, db, tenant, &request_id).await,
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
            Method::Patch => update(req, db, tenant, id, &request_id).await,
            _ => error("NOT_FOUND", 404),
        };
    }
    #[cfg(feature = "native")]
    {
        return pilot::route(req, &env, tenant, role.unwrap_or("admin"), &request_id).await;
    }
    #[cfg(not(feature = "native"))]
    {
        let _ = role;
        error("NOT_FOUND", 404)
    }
}

#[event(fetch)]
pub async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    #[cfg(all(feature = "ci-local", feature = "native"))]
    compile_error!("ci-local and native are mutually exclusive");

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
        return dispatch(req, env, &tenant, None, None).await;
    }

    #[cfg(feature = "native")]
    {
        // One correlation boundary, shared by Access, native domain and D1.
        // Never trust a caller-supplied request identifier or tenant header.
        let request_id = native_request_id()?;
        let started = Date::now().as_millis();
        let path = req.path();
        let method = req.method();
        // Read version binding before dispatch takes ownership of Env.
        let source_sha = env.var("GIT_SHA").ok().map(|value| value.to_string());
        let operation = match (path.as_str(), method.clone()) {
            ("/api/accounts", Method::Get) => "accounts.list",
            ("/api/accounts", Method::Post) => "accounts.create",
            (p, Method::Get) if p.starts_with("/api/accounts/") => "accounts.read",
            (p, Method::Patch) if p.starts_with("/api/accounts/") => "accounts.update",
            ("/api/parties", Method::Get) => "parties.list",
            ("/api/parties", Method::Post) => "parties.create",
            (p, Method::Get) if p.starts_with("/api/parties/") => "parties.read",
            (p, Method::Patch) if p.starts_with("/api/parties/") => "parties.update",
            ("/api/doctor", Method::Get) => "doctor.read",
            ("/api/health", Method::Get) => "health.read",
            ("/api/session", Method::Get) => "session.read",
            ("/api/version", Method::Get) => "version.read",
            (p, Method::Get) if p.starts_with("/api/customer360/") => "customer360.read",
            (p, Method::Get) if p.starts_with("/api/customers") => "customers.read",
            (p, Method::Post) if p.starts_with("/api/customers") => "customers.create",
            (p, Method::Patch) if p.starts_with("/api/customers/") => "customers.update",
            (p, Method::Delete) if p.starts_with("/api/customers/") => "customers.delete",
            (p, Method::Get) if p.starts_with("/api/contacts") => "contacts.read",
            (p, Method::Post) if p.starts_with("/api/contacts") => "contacts.create",
            (p, Method::Patch) if p.starts_with("/api/contacts/") => "contacts.update",
            (p, Method::Delete) if p.starts_with("/api/contacts/") => "contacts.delete",
            (p, Method::Get) if p.starts_with("/api/deals") => "deals.read",
            (p, Method::Post) if p.starts_with("/api/deals") => "deals.create",
            (p, Method::Patch) if p.starts_with("/api/deals/") => "deals.update",
            (p, Method::Delete) if p.starts_with("/api/deals/") => "deals.delete",
            (p, Method::Get) if p.starts_with("/api/tasks") => "tasks.read",
            (p, Method::Post) if p.starts_with("/api/tasks") => "tasks.create",
            (p, Method::Patch) if p.starts_with("/api/tasks/") => "tasks.update",
            (p, Method::Delete) if p.starts_with("/api/tasks/") => "tasks.delete",
            _ => "unmatched",
        };
        let (result, phase) = match access::actor(&req, &env).await {
            Ok(Some(actor)) if path == "/api/doctor" && method == Method::Get =>
                (native_doctor(&env, &actor.tenant).await, "diagnostics"),
            Ok(Some(actor)) =>
                (dispatch(req, env, &actor.tenant, Some(&request_id), Some(&actor.role)).await, "domain_d1"),
            Ok(None) => (error("AUTH_REQUIRED", 401), "access"),
            Err(_) => (error("ACCESS_INTERNAL_ERROR", 503), "access"),
        };
        let mut reply = match result {
            Ok(reply) => reply,
            Err(_) => error("INTERNAL_ERROR", 500)?,
        };
        reply.headers_mut().set("x-request-id", &request_id)?;
        let module = if path == "/api/doctor" { "crm.diagnostics" }
            else if path.starts_with("/api/parties") { "crm.parties" }
            else if path.starts_with("/api/accounts") { "crm.customer-accounts" }
            else if path.starts_with("/api/customer360") { "crm.customer360" }
            else if path.starts_with("/api/customers") { "crm.customers" }
            else if path.starts_with("/api/contacts") { "crm.contacts" }
            else if path.starts_with("/api/deals") { "crm.deals" }
            else if path.starts_with("/api/tasks") { "crm.tasks" }
            else { "crm.platform" };
        reply.headers_mut().set("x-crm-module", module)?;
        reply.headers_mut().set("x-crm-runtime", "native-workers-rs")?;
        if let Some(value) = source_sha {
            if value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                reply.headers_mut().set("x-crm-source-sha", &value)?;
            }
        }
        let elapsed = Date::now().as_millis().saturating_sub(started);
        let error_code = reply.headers().get("x-crm-error-code").ok().flatten();
        console_log!("{}", json!({
            "event":"crm.native.request", "request_id":request_id,
            "module":module,
            "operation":operation, "phase":phase,
            "status":reply.status_code(), "error_code":error_code, "duration_ms":elapsed
        }));
        return Ok(reply);
    }

    #[cfg(not(any(feature = "ci-local", feature = "native")))]
    {
        let _ = (req, env);
        error("AUTH_REQUIRED", 403)
    }
}
