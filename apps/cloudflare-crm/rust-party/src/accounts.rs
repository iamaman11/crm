//! Native D1 storage/HTTP boundary for the ORIGINAL crm-customer-accounts owner.
//! Account is a commercial entity associated with a Party, never a second Party.
//! The domain crate alone decides normalization, versions and valid transitions.
use crm_customer_accounts::{
    Account, AccountId, AccountPartyAssociation, AccountPartyRole, AccountStatus,
    CreateAccount, PartyReference, UpdateAccount,
    decode_account_state, encode_account_state,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use wasm_bindgen::JsValue;
use worker::{D1Database, Method, Request, Response, Result};

use super::{bind, error, now_nanos, response};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateInput {
    id: String,
    name: String,
    party_id: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateInput {
    name: String,
    status: AccountStatusInput,
    expected_version: i64,
}
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum AccountStatusInput { Active, Inactive }
#[derive(Deserialize)]
struct Stored { state: String }
#[derive(Deserialize)]
struct Row { state: String }
#[derive(Deserialize)]
struct Receipt { digest: String, state: String }

fn hash_input<T: Serialize>(operation: &str, id: &str, input: &T) -> Result<String> {
    let bytes = serde_json::to_vec(&json!([operation, id, input]))
        .map_err(|e| worker::Error::RustError(e.to_string()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
fn state(account: &Account) -> Result<String> {
    String::from_utf8(encode_account_state(account).map_err(|e|
        worker::Error::RustError(e.code))?)
        .map_err(|e| worker::Error::RustError(e.to_string()))
}
fn decoded(bytes: &str) -> Result<Account> {
    decode_account_state(bytes.as_bytes())
        .map_err(|e| worker::Error::RustError(e.code))
}
fn view(account: &Account) -> Value {
    json!({
        "id": account.account_id().as_str(),
        "name": account.name(),
        "status": match account.status() {
            AccountStatus::Active => "active", AccountStatus::Inactive => "inactive"
        },
        "version": account.version(),
        "parties": account.party_associations().iter().map(|a| json!({
            "id": a.party().as_str(),
            "role": match a.role() {
                AccountPartyRole::Primary => "primary",
                AccountPartyRole::Member => "member",
            }
        })).collect::<Vec<_>>()
    })
}
async fn load(db: &D1Database, tenant: &str, id: &str) -> Result<Option<Account>> {
    bind(db, "SELECT state FROM crm_account_records WHERE tenant = ? AND id = ?",
        &[JsValue::from_str(tenant), JsValue::from_str(id)])?
        .first::<Stored>(None).await?
        .map(|row| decoded(&row.state)).transpose()
}
async fn prior(db: &D1Database, tenant: &str, key: &str) -> Result<Option<Receipt>> {
    bind(db, "SELECT digest, state FROM crm_account_receipts WHERE tenant = ? AND key = ?",
        &[JsValue::from_str(tenant), JsValue::from_str(key)])?
        .first::<Receipt>(None).await
}
async fn party_exists(db: &D1Database, tenant: &str, party_id: &str) -> Result<bool> {
    Ok(bind(db, "SELECT id FROM crm_party_records WHERE tenant = ? AND id = ?",
        &[JsValue::from_str(tenant), JsValue::from_str(party_id)])?
        .first::<Value>(None).await?.is_some())
}
fn required_key(req: &Request) -> Result<Option<String>> {
    Ok(req.headers().get("idempotency-key")?
        .filter(|key| (8..=120).contains(&key.len())))
}
async fn create(mut req: Request, db: &D1Database, tenant: &str, trace: &str) -> Result<Response> {
    let Some(key) = required_key(&req)? else {
        return error("IDEMPOTENCY_KEY_REQUIRED", 422);
    };
    let input = match req.json::<CreateInput>().await {
        Ok(v) => v, Err(_) => return error("INVALID_INPUT", 422),
    };
    let digest = hash_input("create", &input.id, &input)?;
    if let Some(receipt) = prior(db, tenant, &key).await? {
        if receipt.digest != digest { return error("IDEMPOTENCY_CONFLICT", 409); }
        return response(json!({"item":view(&decoded(&receipt.state)?),"replayed":true}),200);
    }
    let id = match AccountId::try_new(&input.id) {
        Ok(x) => x, Err(e) => return error(&e.code, 422),
    };
    let party = match PartyReference::try_new(&input.party_id) {
        Ok(x) => x, Err(e) => return error(&e.code, 422),
    };
    if !party_exists(db, tenant, party.as_str()).await? {
        return error("CUSTOMER_ACCOUNTS_PARTY_NOT_FOUND", 422);
    }
    let account = match Account::create(CreateAccount {
        account_id: id, name: input.name,
        party_associations: vec![AccountPartyAssociation::new(party, AccountPartyRole::Primary)],
        occurred_at_unix_nanos: now_nanos(),
    }) { Ok(x) => x, Err(e) => return error(&e.code, 422) };
    let snapshot = state(&account)?;
    let stmt = bind(db, "INSERT INTO crm_account_records(tenant,id,primary_party,state,version,idem_key,idem_digest,request_id) VALUES(?,?,?,?,?,?,?,?)",
        &[
            JsValue::from_str(tenant), JsValue::from_str(account.account_id().as_str()),
            JsValue::from_str(&input.party_id), JsValue::from_str(&snapshot),
            JsValue::from_f64(account.version() as f64), JsValue::from_str(&key),
            JsValue::from_str(&digest), JsValue::from_str(trace),
        ])?;
    // Triggers insert the receipt and the request-correlated audit atomically.
    if stmt.run().await.is_err() {
        return error("CUSTOMER_ACCOUNTS_CREATE_CONFLICT", 409);
    }
    response(json!({"item":view(&account),"replayed":false}),201)
}
async fn update(mut req: Request, db: &D1Database, tenant: &str, id: &str, trace: &str) -> Result<Response> {
    let Some(key) = required_key(&req)? else {
        return error("IDEMPOTENCY_KEY_REQUIRED", 422);
    };
    let input = match req.json::<UpdateInput>().await {
        Ok(v) => v, Err(_) => return error("INVALID_INPUT", 422),
    };
    let digest = hash_input("update", id, &input)?;
    if let Some(receipt) = prior(db, tenant, &key).await? {
        if receipt.digest != digest { return error("IDEMPOTENCY_CONFLICT", 409); }
        return response(json!({"item":view(&decoded(&receipt.state)?),"replayed":true}),200);
    }
    let Some(mut account) = load(db, tenant, id).await? else {
        return error("NOT_FOUND", 404);
    };
    let status = match input.status {
        AccountStatusInput::Active => AccountStatus::Active,
        AccountStatusInput::Inactive => AccountStatus::Inactive,
    };
    if let Err(e) = account.apply_update(UpdateAccount {
        expected_version: input.expected_version, name: input.name, status,
        party_associations: account.party_associations().to_vec(),
        occurred_at_unix_nanos: now_nanos(),
    }) {
        return error(&e.code, if e.code == "CUSTOMER_ACCOUNTS_VERSION_CONFLICT" {409} else {422});
    }
    let snapshot = state(&account)?;
    let result = bind(db, "UPDATE crm_account_records SET state = ?, version = ?, idem_key = ?, idem_digest = ?, request_id = ? WHERE tenant = ? AND id = ? AND version = ?",
        &[
            JsValue::from_str(&snapshot), JsValue::from_f64(account.version() as f64),
            JsValue::from_str(&key), JsValue::from_str(&digest), JsValue::from_str(trace),
            JsValue::from_str(tenant), JsValue::from_str(id),
            JsValue::from_f64(input.expected_version as f64),
        ])?.run().await;
    let Ok(result) = result else {return error("CUSTOMER_ACCOUNTS_UPDATE_CONFLICT", 409)};
    if result.meta()?.and_then(|m|m.changes).unwrap_or(0) == 0 {
        return error("CUSTOMER_ACCOUNTS_VERSION_CONFLICT", 409);
    }
    response(json!({"item":view(&account),"replayed":false}),200)
}
async fn read(db: &D1Database, tenant: &str, id: &str) -> Result<Response> {
    match load(db, tenant, id).await? {
        Some(account) => response(json!({"item":view(&account)}), 200),
        None => error("NOT_FOUND", 404),
    }
}
async fn list(db: &D1Database, tenant: &str) -> Result<Response> {
    let rows = bind(db, "SELECT state FROM crm_account_records WHERE tenant = ? ORDER BY id LIMIT 300",
        &[JsValue::from_str(tenant)])?.all().await?.results::<Row>()?;
    let items = rows.iter().map(|row| decoded(&row.state).map(|account|view(&account)))
        .collect::<Result<Vec<_>>>()?;
    response(json!({"items":items}),200)
}
pub(super) async fn route(req: Request, db: D1Database, tenant: &str, trace: &str) -> Result<Response> {
    let path = req.path();
    let method = req.method();
    if path == "/api/accounts" {
        return match method {
            Method::Get => list(&db, tenant).await,
            Method::Post => create(req, &db, tenant, trace).await,
            _ => error("NOT_FOUND", 404)
        };
    }
    let Some(id) = path.strip_prefix("/api/accounts/") else {return error("NOT_FOUND", 404)};
    if id.is_empty() || id.contains('/') {return error("NOT_FOUND", 404);}
    match method {
        Method::Get => read(&db, tenant, id).await,
        Method::Patch => update(req, &db, tenant, id, trace).await,
        _ => error("NOT_FOUND", 404)
    }
}
