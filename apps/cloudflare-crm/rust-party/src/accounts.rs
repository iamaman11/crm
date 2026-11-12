//! Native D1 storage/HTTP boundary for the ORIGINAL crm-customer-accounts owner.
//! Account is a commercial entity associated with a Party, never a second Party.
//! The domain crate alone decides normalization, versions and valid transitions.
use crm_customer_accounts::{
    Account, AccountId, AccountPartyAssociation, AccountPartyRole, AccountStatus, CreateAccount,
    PartyReference, UpdateAccount, decode_account_state, encode_account_state,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use wasm_bindgen::JsValue;
use worker::{D1Database, Method, Request, Response, Result};

use super::{bind, error, now_nanos, response};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CreateInput {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) party_id: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct UpdateInput {
    pub(super) name: String,
    pub(super) status: AccountStatusInput,
    pub(super) expected_version: i64,
}
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum AccountStatusInput {
    Active,
    Inactive,
}
#[derive(Deserialize)]
struct Stored {
    state: String,
}
#[derive(Deserialize)]
struct Row {
    id: String,
    state: String,
}
#[derive(Deserialize)]
struct Receipt {
    digest: String,
    state: String,
}

fn hash_input<T: Serialize>(operation: &str, id: &str, input: &T) -> Result<String> {
    let bytes = serde_json::to_vec(&json!([operation, id, input]))
        .map_err(|e| worker::Error::RustError(e.to_string()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
fn state(account: &Account) -> Result<String> {
    String::from_utf8(encode_account_state(account).map_err(|e| worker::Error::RustError(e.code))?)
        .map_err(|e| worker::Error::RustError(e.to_string()))
}
fn decoded(bytes: &str) -> Result<Account> {
    decode_account_state(bytes.as_bytes()).map_err(|e| worker::Error::RustError(e.code))
}
pub(super) fn view(account: &Account) -> Value {
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
pub(super) async fn load(db: &D1Database, tenant: &str, id: &str) -> Result<Option<Account>> {
    bind(
        db,
        "SELECT state FROM crm_account_records WHERE tenant = ? AND id = ?",
        &[JsValue::from_str(tenant), JsValue::from_str(id)],
    )?
    .first::<Stored>(None)
    .await?
    .map(|row| decoded(&row.state))
    .transpose()
}
async fn prior(db: &D1Database, tenant: &str, key: &str) -> Result<Option<Receipt>> {
    bind(
        db,
        "SELECT digest, state FROM crm_account_receipts WHERE tenant = ? AND key = ?",
        &[JsValue::from_str(tenant), JsValue::from_str(key)],
    )?
    .first::<Receipt>(None)
    .await
}
async fn party_exists(db: &D1Database, tenant: &str, party_id: &str) -> Result<bool> {
    Ok(bind(
        db,
        "SELECT id FROM crm_party_records WHERE tenant = ? AND id = ?",
        &[JsValue::from_str(tenant), JsValue::from_str(party_id)],
    )?
    .first::<Value>(None)
    .await?
    .is_some())
}
fn required_key(req: &Request) -> Result<Option<String>> {
    Ok(req
        .headers()
        .get("idempotency-key")?
        .filter(|key| (8..=120).contains(&key.len())))
}
// ORIGINAL Account owner. The REST facade (blocked in contract-only) and
// generated Connect handlers invoke these same typed domain+D1 commands.
pub(super) struct MutationOutcome {
    pub(super) account: Account,
    pub(super) replayed: bool,
}
pub(super) struct MutationFailure {
    pub(super) code: String,
    pub(super) status: u16,
}
fn fail(code: impl Into<String>, status: u16) -> MutationFailure {
    MutationFailure {
        code: code.into(),
        status,
    }
}
pub(super) async fn create_typed(
    input: CreateInput,
    key: &str,
    db: &D1Database,
    tenant: &str,
    trace: &str,
) -> Result<std::result::Result<MutationOutcome, MutationFailure>> {
    if !(8..=120).contains(&key.len()) {
        return Ok(Err(fail("IDEMPOTENCY_KEY_REQUIRED", 422)));
    }
    let digest = hash_input("create", &input.id, &input)?;
    if let Some(receipt) = prior(db, tenant, key).await? {
        if receipt.digest != digest {
            return Ok(Err(fail("IDEMPOTENCY_CONFLICT", 409)));
        }
        let account = decoded(&receipt.state)?;
        if account.account_id().as_str() != input.id {
            return Ok(Err(fail("IDEMPOTENCY_CONFLICT", 409)));
        }
        return Ok(Ok(MutationOutcome {
            account,
            replayed: true,
        }));
    }
    let id = match AccountId::try_new(&input.id) {
        Ok(v) => v,
        Err(e) => return Ok(Err(fail(e.code, 422))),
    };
    let party = match PartyReference::try_new(&input.party_id) {
        Ok(v) => v,
        Err(e) => return Ok(Err(fail(e.code, 422))),
    };
    if !party_exists(db, tenant, party.as_str()).await? {
        return Ok(Err(fail("CUSTOMER_ACCOUNTS_PARTY_NOT_FOUND", 422)));
    }
    let account = match Account::create(CreateAccount {
        account_id: id,
        name: input.name,
        party_associations: vec![AccountPartyAssociation::new(
            party,
            AccountPartyRole::Primary,
        )],
        occurred_at_unix_nanos: now_nanos(),
    }) {
        Ok(v) => v,
        Err(e) => return Ok(Err(fail(e.code, 422))),
    };
    let snapshot = state(&account)?;
    let stored=bind(db,
        "INSERT INTO crm_account_records(tenant,id,primary_party,state,version,idem_key,idem_digest,request_id) VALUES(?,?,?,?,?,?,?,?)",
        &[JsValue::from_str(tenant),JsValue::from_str(account.account_id().as_str()),
          JsValue::from_str(&input.party_id),JsValue::from_str(&snapshot),
          JsValue::from_f64(account.version() as f64),JsValue::from_str(key),
          JsValue::from_str(&digest),JsValue::from_str(trace)])?.run().await;
    if stored.is_err() {
        return Ok(Err(fail("CUSTOMER_ACCOUNTS_CREATE_CONFLICT", 409)));
    }
    Ok(Ok(MutationOutcome {
        account,
        replayed: false,
    }))
}
pub(super) async fn update_typed(
    input: UpdateInput,
    key: &str,
    db: &D1Database,
    tenant: &str,
    id: &str,
    trace: &str,
) -> Result<std::result::Result<MutationOutcome, MutationFailure>> {
    if !(8..=120).contains(&key.len()) {
        return Ok(Err(fail("IDEMPOTENCY_KEY_REQUIRED", 422)));
    }
    let digest = hash_input("update", id, &input)?;
    if let Some(receipt) = prior(db, tenant, key).await? {
        if receipt.digest != digest {
            return Ok(Err(fail("IDEMPOTENCY_CONFLICT", 409)));
        }
        let account = decoded(&receipt.state)?;
        if account.account_id().as_str() != id {
            return Ok(Err(fail("IDEMPOTENCY_CONFLICT", 409)));
        }
        return Ok(Ok(MutationOutcome {
            account,
            replayed: true,
        }));
    }
    let Some(mut account) = load(db, tenant, id).await? else {
        return Ok(Err(fail("NOT_FOUND", 404)));
    };
    let status = match input.status {
        AccountStatusInput::Active => AccountStatus::Active,
        AccountStatusInput::Inactive => AccountStatus::Inactive,
    };
    if let Err(e) = account.apply_update(UpdateAccount {
        expected_version: input.expected_version,
        name: input.name,
        status,
        party_associations: account.party_associations().to_vec(),
        occurred_at_unix_nanos: now_nanos(),
    }) {
        return Ok(Err(fail(
            e.code.clone(),
            if e.code == "CUSTOMER_ACCOUNTS_VERSION_CONFLICT" {
                409
            } else {
                422
            },
        )));
    }
    let snapshot = state(&account)?;
    let changed=bind(db,
        "UPDATE crm_account_records SET state=?,version=?,idem_key=?,idem_digest=?,request_id=? WHERE tenant=? AND id=? AND version=?",
        &[JsValue::from_str(&snapshot),JsValue::from_f64(account.version() as f64),
          JsValue::from_str(key),JsValue::from_str(&digest),JsValue::from_str(trace),
          JsValue::from_str(tenant),JsValue::from_str(id),
          JsValue::from_f64(input.expected_version as f64)])?.run().await;
    let Ok(result) = changed else {
        return Ok(Err(fail("CUSTOMER_ACCOUNTS_UPDATE_CONFLICT", 409)));
    };
    if result.meta()?.and_then(|m| m.changes).unwrap_or(0) == 0 {
        return Ok(Err(fail("CUSTOMER_ACCOUNTS_VERSION_CONFLICT", 409)));
    }
    Ok(Ok(MutationOutcome {
        account,
        replayed: false,
    }))
}
pub(super) async fn list_page_typed(
    db: &D1Database,
    tenant: &str,
    after: &str,
    size: usize,
    status: Option<AccountStatus>,
) -> Result<(Vec<Account>, Option<String>)> {
    // Original Account persisted state has top-level snake_case status.
    // Select and page the filtered rows in the same tenant-scoped D1 query.
    let filter = status.map(|value| match value {
        AccountStatus::Active => "active",
        AccountStatus::Inactive => "inactive",
    });
    let query = if filter.is_some() {
        "SELECT id,state FROM crm_account_records WHERE tenant=? AND id>? AND json_extract(state,'$.status')=? ORDER BY id LIMIT ?"
    } else {
        "SELECT id,state FROM crm_account_records WHERE tenant=? AND id>? ORDER BY id LIMIT ?"
    };
    let mut params = vec![JsValue::from_str(tenant), JsValue::from_str(after)];
    if let Some(value) = filter {
        params.push(JsValue::from_str(value));
    }
    params.push(JsValue::from_f64((size + 1) as f64));
    let rows = bind(db, query, &params)?.all().await?.results::<Row>()?;
    let has_more = rows.len() > size;
    let next = if has_more {
        rows.get(size - 1).map(|r| r.id.clone())
    } else {
        None
    };
    let mut accounts = Vec::new();
    for row in rows.iter().take(size) {
        let account = decoded(&row.state)?;
        accounts.push(account);
    }
    Ok((accounts, next))
}
async fn create(mut req: Request, db: &D1Database, tenant: &str, trace: &str) -> Result<Response> {
    let Some(key) = required_key(&req)? else {
        return error("IDEMPOTENCY_KEY_REQUIRED", 422);
    };
    let input = match serde_json::from_str::<CreateInput>(&req.text().await?) {
        Ok(v) => v,
        Err(_) => return error("INVALID_INPUT", 422),
    };
    match create_typed(input, &key, db, tenant, trace).await? {
        Ok(out) => response(
            json!({"item":view(&out.account),"replayed":out.replayed}),
            if out.replayed { 200 } else { 201 },
        ),
        Err(f) => error(&f.code, f.status),
    }
}
async fn update(
    mut req: Request,
    db: &D1Database,
    tenant: &str,
    id: &str,
    trace: &str,
) -> Result<Response> {
    let Some(key) = required_key(&req)? else {
        return error("IDEMPOTENCY_KEY_REQUIRED", 422);
    };
    let input = match serde_json::from_str::<UpdateInput>(&req.text().await?) {
        Ok(v) => v,
        Err(_) => return error("INVALID_INPUT", 422),
    };
    match update_typed(input, &key, db, tenant, id, trace).await? {
        Ok(out) => response(
            json!({"item":view(&out.account),"replayed":out.replayed}),
            200,
        ),
        Err(f) => error(&f.code, f.status),
    }
}
async fn read(db: &D1Database, tenant: &str, id: &str) -> Result<Response> {
    match load(db, tenant, id).await? {
        Some(account) => response(json!({"item":view(&account)}), 200),
        None => error("NOT_FOUND", 404),
    }
}
async fn list(db: &D1Database, tenant: &str) -> Result<Response> {
    let (accounts, _) = list_page_typed(db, tenant, "", 100, None).await?;
    response(
        json!({"items":accounts.iter().map(view).collect::<Vec<_>>()}),
        200,
    )
}

pub(super) async fn route(
    req: Request,
    db: D1Database,
    tenant: &str,
    trace: &str,
) -> Result<Response> {
    let path = req.path();
    let method = req.method();
    if path == "/api/accounts" {
        return match method {
            Method::Get => list(&db, tenant).await,
            Method::Post => create(req, &db, tenant, trace).await,
            _ => error("NOT_FOUND", 404),
        };
    }
    let Some(id) = path.strip_prefix("/api/accounts/") else {
        return error("NOT_FOUND", 404);
    };
    if id.is_empty() || id.contains('/') {
        return error("NOT_FOUND", 404);
    }
    match method {
        Method::Get => read(&db, tenant, id).await,
        Method::Patch => update(req, &db, tenant, id, trace).await,
        _ => error("NOT_FOUND", 404),
    }
}
