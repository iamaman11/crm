//! Native HTTP/D1 adapter for the four existing *test-pilot* CRM screens.
//! One Rust fetch entrypoint owns this transport. No JS routing, auth or D1
//! mutations; the original crm-parties domain continues to own Party rules.
//! These pilot CRUD records are not substitutes for the expert owner modules.
use serde_json::{json, Map, Value};
use wasm_bindgen::JsValue;
use worker::{D1Database, Env, Method, Request, Response, Result};
use super::{bind, error, native_request_id, response};

#[derive(Clone, Copy)]
struct Model {
    table: &'static str,
    fields: &'static [&'static str],
}

fn model(name: &str) -> Option<Model> {
    match name {
        "customers" => Some(Model { table: "crm_customers", fields: &["name", "kind"] }),
        "contacts" => Some(Model { table: "crm_contacts", fields: &["customer_id", "name", "email", "phone"] }),
        "deals" => Some(Model { table: "crm_deals", fields: &["customer_id", "title", "stage", "amount"] }),
        "tasks" => Some(Model { table: "crm_tasks", fields: &["customer_id", "title", "due_date", "status"] }),
        _ => None,
    }
}

fn uuid(id: &str) -> bool {
    id.len() == 36 && id.bytes().enumerate().all(|(i, c)|
        if [8, 13, 18, 23].contains(&i) { c == b'-' }
        else { c.is_ascii_hexdigit() })
}
fn text(data: &Value, key: &str, limit: usize) -> String {
    data.get(key).and_then(Value::as_str).unwrap_or("").trim()
        .chars().take(limit).collect()
}
fn js(value: &Value) -> JsValue {
    match value {
        Value::String(s) => JsValue::from_str(s),
        Value::Number(n) => JsValue::from_f64(n.as_f64().unwrap_or(0.0)),
        _ => JsValue::NULL,
    }
}
fn params(values: &[Value]) -> Vec<JsValue> { values.iter().map(js).collect() }
fn s(value: &str) -> Value { Value::String(value.to_owned()) }
fn stamp() -> String {
    js_sys::Date::new_0().to_iso_string().as_string().unwrap_or_default()
}
async fn first(db: &D1Database, sql: &str, args: &[Value]) -> Result<Option<Value>> {
    bind(db, sql, &params(args))?.first::<Value>(None).await
}
async fn rows(db: &D1Database, sql: &str, args: &[Value]) -> Result<Vec<Value>> {
    bind(db, sql, &params(args))?.all().await?.results::<Value>()
}
fn valid_date(date: &str) -> bool {
    date.len() == 10 && date.bytes().enumerate().all(|(i,c)|
        if i == 4 || i == 7 { c == b'-' } else { c.is_ascii_digit() })
}
fn email_ok(email: &str) -> bool {
    if email.is_empty() { return true; }
    if email.bytes().any(|c| c.is_ascii_whitespace()) { return false; }
    let mut parts = email.split('@');
    let (Some(before), Some(after), None) = (parts.next(), parts.next(), parts.next())
        else { return false; };
    !before.is_empty() && after.split('.').all(|x| !x.is_empty()) && after.contains('.')
}
fn validate(name: &str, input: &Value) -> Option<Map<String, Value>> {
    if !input.is_object() { return None; }
    let mut data = Map::new();
    if name == "customers" {
        let n = text(input, "name", 120);
        let kind = text(input, "kind", 120);
        if n.is_empty() || !["person", "organization"].contains(&kind.as_str()) { return None; }
        data.insert("name".into(), s(&n));
        data.insert("kind".into(), s(&kind));
        return Some(data);
    }
    let customer = text(input, "customer_id", 36);
    if !uuid(&customer) { return None; }
    data.insert("customer_id".into(), s(&customer));
    match name {
        "contacts" => {
            let name = text(input, "name", 120);
            let email = text(input, "email", 180);
            let phone = text(input, "phone", 60);
            if name.is_empty() || !email_ok(&email) { return None; }
            data.insert("name".into(), s(&name));
            data.insert("email".into(), s(&email));
            data.insert("phone".into(), s(&phone));
        }
        "deals" => {
            let title = text(input, "title", 120);
            let stage = text(input, "stage", 120);
            let amount = match input.get("amount") {
                Some(Value::Number(n)) => n.as_f64(),
                Some(Value::String(x)) => x.parse::<f64>().ok(),
                Some(Value::Null) => Some(0.0),
                _ => None,
            }?;
            if title.is_empty() || !["new", "qualified", "proposal", "won", "lost"].contains(&stage.as_str())
                || !amount.is_finite() || !(0.0..=1_000_000_000.0).contains(&amount) { return None; }
            data.insert("title".into(), s(&title));
            data.insert("stage".into(), s(&stage));
            data.insert("amount".into(), json!(amount));
        }
        "tasks" => {
            let title = text(input, "title", 120);
            let status = text(input, "status", 120);
            let date = text(input, "due_date", 10);
            if title.is_empty() || !["open", "done"].contains(&status.as_str())
                || (!date.is_empty() && !valid_date(&date)) { return None; }
            data.insert("title".into(), s(&title));
            data.insert("status".into(), s(&status));
            data.insert("due_date".into(), s(&date));
        }
        _ => return None,
    };
    Some(data)
}
async fn body(req: &mut Request) -> Result<Option<Value>> {
    if req.headers().get("content-length")?.and_then(|x| x.parse::<usize>().ok()).unwrap_or(0) > 8192 {
        return Ok(None);
    }
    let content = req.text().await?;
    if content.len() > 8192 { return Ok(None); }
    Ok(serde_json::from_str(&content).ok())
}
async fn customer_exists(db: &D1Database, tenant: &str, id: &str) -> Result<bool> {
    Ok(first(db, "SELECT id FROM crm_customers WHERE tenant_id = ? AND id = ?",
        &[s(tenant), s(id)]).await?.is_some())
}
async fn records(mut req: Request, db: &D1Database, tenant: &str, request_id: &str,
    name: &str, id: Option<&str>, spec: Model) -> Result<Response> {
    let method = req.method();
    if let Some(id) = id { if !uuid(id) { return error("NOT_FOUND", 404); } }
    if method == Method::Get {
        if let Some(id) = id {
            let sql = format!("SELECT * FROM {} WHERE id = ? AND tenant_id = ?", spec.table);
            return match first(db, &sql, &[s(id), s(tenant)]).await? {
                Some(item) => response(json!({"item":item}), 200),
                None => error("NOT_FOUND", 404),
            };
        }
        let sql = format!("SELECT * FROM {} WHERE tenant_id = ? ORDER BY updated_at DESC LIMIT 300", spec.table);
        return response(json!({"items":rows(db, &sql, &[s(tenant)]).await?}), 200);
    }
    if (method == Method::Post && id.is_none()) || (method == Method::Patch && id.is_some()) {
        let input = match body(&mut req).await? { Some(value) => value, None => return error("INVALID_INPUT", 422) };
        let Some(data) = validate(name, &input) else { return error("INVALID_INPUT", 422); };
        if let Some(customer) = data.get("customer_id").and_then(Value::as_str) {
            if !customer_exists(db, tenant, customer).await? { return error("CUSTOMER_NOT_FOUND", 422); }
        }
        if let Some(id) = id {
            let sql = format!("SELECT id FROM {} WHERE id = ? AND tenant_id = ?", spec.table);
            if first(db, &sql, &[s(id), s(tenant)]).await?.is_none() { return error("NOT_FOUND", 404); }
        }
        let new_id = match id { Some(id) => id.to_owned(), None => native_request_id()? };
        let at = stamp();
        let fields: Vec<&str> = spec.fields.to_vec();
        let mut values: Vec<Value> = fields.iter().map(|f| data.get(*f).cloned().unwrap_or(Value::Null)).collect();
        let sql = if id.is_some() {
            let sets = fields.iter().map(|f| format!("{f} = ?")).collect::<Vec<_>>().join(", ");
            values.extend([s(&at), s(&new_id), s(tenant)]);
            format!("UPDATE {} SET {}, updated_at = ? WHERE id = ? AND tenant_id = ?", spec.table, sets)
        } else {
            let columns = fields.join(", ");
            let placeholders = vec!["?"; fields.len() + 3].join(", ");
            values.splice(0..0, [s(&new_id), s(tenant)]);
            values.push(s(&at));
            format!("INSERT INTO {} (id, tenant_id, {}, updated_at) VALUES ({})",
                spec.table, columns, placeholders)
        };
        let sql_audit = "INSERT INTO crm_audit (id, tenant_id, action, subject_id, created_at, request_id) VALUES (?,?,?,?,?,?)";
        db.batch(vec![
            bind(db, &sql, &params(&values))?,
            bind(db, sql_audit, &params(&[
                s(&native_request_id()?), s(tenant),
                s(&format!("{name}.{}", if id.is_some() { "update" } else { "create" })),
                s(&new_id), s(&at), s(request_id),
            ]))?,
        ]).await?;
        let mut item = data;
        item.insert("id".into(), s(&new_id));
        item.insert("tenant_id".into(), s(tenant));
        item.insert("updated_at".into(), s(&at));
        return response(json!({"item": Value::Object(item)}), if id.is_some() {200} else {201});
    }
    if method == Method::Delete {
        let Some(id) = id else { return error("NOT_FOUND", 404); };
        let sql = format!("SELECT id FROM {} WHERE id = ? AND tenant_id = ?", spec.table);
        if first(db, &sql, &[s(id), s(tenant)]).await?.is_none() { return error("NOT_FOUND", 404); }
        let del = format!("DELETE FROM {} WHERE id = ? AND tenant_id = ?", spec.table);
        db.batch(vec![
            bind(db, &del, &params(&[s(id), s(tenant)]))?,
            bind(db, "INSERT INTO crm_audit (id, tenant_id, action, subject_id, created_at, request_id) VALUES (?,?,?,?,?,?)",
                &params(&[s(&native_request_id()?), s(tenant),
                    s(&format!("{name}.delete")), s(id), s(&stamp()), s(request_id)]))?,
        ]).await?;
        return Ok(Response::empty()?.with_status(204));
    }
    error("NOT_FOUND", 404)
}

async fn customer360(db: &D1Database, tenant: &str, id: &str) -> Result<Response> {
    if !uuid(id) { return error("NOT_FOUND", 404); }
    let Some(customer) = first(db,
        "SELECT id, name, kind, updated_at FROM crm_customers WHERE id = ? AND tenant_id = ?",
        &[s(id), s(tenant)]).await? else { return error("NOT_FOUND", 404); };
    const LIMIT: i64 = 100;
    let today = stamp().chars().take(10).collect::<String>();
    let mut related = Map::new();
    let mut counts = Map::new();
    for (key, table) in [("contacts","crm_contacts"), ("deals","crm_deals"), ("tasks","crm_tasks")] {
        let count_sql = format!("SELECT COUNT(*) AS total FROM {table} WHERE tenant_id = ? AND customer_id = ?");
        let total = first(db, &count_sql, &[s(tenant), s(id)]).await?
            .and_then(|x| x.get("total").and_then(Value::as_i64)).unwrap_or(0);
        let query = format!("SELECT * FROM {table} WHERE tenant_id = ? AND customer_id = ? ORDER BY updated_at DESC LIMIT ?");
        let list = rows(db, &query, &[s(tenant), s(id), json!(LIMIT)]).await?;
        counts.insert(key.into(), json!(total));
        related.insert(key.into(), json!({"total":total, "items":list, "limit":LIMIT}));
    }
    let contacts = counts["contacts"].as_i64().unwrap_or(0);
    let deals = counts["deals"].as_i64().unwrap_or(0);
    let tasks = counts["tasks"].as_i64().unwrap_or(0);
    let task_list = related["tasks"]["items"].as_array().cloned().unwrap_or_default();
    let deal_list = related["deals"]["items"].as_array().cloned().unwrap_or_default();
    let open = task_list.iter().filter(|x| x["status"] == "open").count();
    let overdue = task_list.iter().filter(|x| x["status"] == "open"
        && x["due_date"].as_str().is_some_and(|d| !d.is_empty() && d < today.as_str())).count();
    let amount: f64 = deal_list.iter().map(|x| x["amount"].as_f64().unwrap_or(0.0)).sum();
    response(json!({
        "customer": customer, "related": related, "as_of": today,
        "summary": {"contacts":contacts, "deals":deals, "tasks":tasks,
            "open_tasks": if tasks <= LIMIT {json!(open)} else {Value::Null},
            "overdue_tasks": if tasks <= LIMIT {json!(overdue)} else {Value::Null},
            "deal_amount": if deals <= LIMIT {json!(amount)} else {Value::Null}}
    }), 200)
}

pub(super) async fn route(req: Request, env: &Env, tenant: &str,
    role: &str, request_id: &str) -> Result<Response> {
    let path = req.path();
    let method = req.method();
    if path == "/api/health" && method == Method::Get {
        return response(json!({"service":"crm-dev","status":"ok"}), 200);
    }
    if path == "/api/session" && method == Method::Get {
        return response(json!({"authenticated":true,"role":role}), 200);
    }
    if path == "/api/version" && method == Method::Get {
        return response(json!({"source":env.var("GIT_SHA").map(|x| x.to_string()).unwrap_or_default(),
            "database":"D1","api":"crm.dev/v1"}), 200);
    }
    let db = env.d1("DB")?;
    if let Some(id) = path.strip_prefix("/api/customer360/") {
        return if method == Method::Get {customer360(&db, tenant, id).await}
            else {error("NOT_FOUND", 404)};
    }
    let Some(rest) = path.strip_prefix("/api/") else { return error("NOT_FOUND", 404); };
    let mut parts = rest.split('/');
    let (Some(name), id, None) = (parts.next(), parts.next(), parts.next()) else {
        return error("NOT_FOUND", 404);
    };
    let Some(spec) = model(name) else { return error("NOT_FOUND", 404); };
    records(req, &db, tenant, request_id, name, id, spec).await
}
