//! Native D1 storage/HTTP boundary for the ORIGINAL crm-customer-accounts owner.
//! Account is a commercial entity associated with a Party, never a second Party.
//! The domain crate alone decides normalization, versions and valid transitions.
use super::d1_write::{self, Confirmation, Receipt, WriteResult, rejected};
use super::diagnostics::{MutationFailure, ReceiptUncertainty};
use crm_customer_accounts::{
    Account, AccountId, AccountPartyAssociation, AccountPartyRole, AccountStatus, CreateAccount,
    PartyReference, UpdateAccount, decode_account_state, encode_account_state,
};
use crm_module_sdk::ErrorCategory;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use wasm_bindgen::JsValue;
use worker::{D1Database, Result};

use super::{access, bind, now_nanos};

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
        "SELECT digest, state, original_request_id FROM crm_account_receipts WHERE tenant = ? AND key = ?",
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
pub(super) struct MutationOutcome {
    pub(super) account: Account,
    pub(super) replayed: bool,
    pub(super) original_request_id: Option<String>,
}
const UNCONFIRMED: &str = "CUSTOMER_ACCOUNTS_WRITE_UNCONFIRMED";

fn storage_failure(_: worker::Error) -> MutationFailure {
    MutationFailure::CommitUnconfirmed {
        code: UNCONFIRMED,
        reason: ReceiptUncertainty::StorageUnavailable,
    }
}

fn replay(receipt: Receipt, digest: &str, id: &str) -> WriteResult<MutationOutcome> {
    if receipt.digest != digest {
        return Err(rejected("IDEMPOTENCY_CONFLICT", ErrorCategory::Conflict));
    }
    let account = decoded(&receipt.state).map_err(|_| MutationFailure::CommitUnconfirmed {
        code: UNCONFIRMED,
        reason: ReceiptUncertainty::DecodeFailed,
    })?;
    if account.account_id().as_str() != id {
        return Err(rejected("IDEMPOTENCY_CONFLICT", ErrorCategory::Conflict));
    }
    Ok(MutationOutcome {
        account,
        replayed: true,
        original_request_id: receipt.original_request_id,
    })
}
pub(super) async fn create_typed(
    input: CreateInput,
    key: &str,
    db: &D1Database,
    actor: &access::Actor,
    trace: &str,
) -> WriteResult<MutationOutcome> {
    let tenant = &actor.tenant;
    if !(8..=120).contains(&key.len()) {
        return Err(rejected(
            "IDEMPOTENCY_KEY_REQUIRED",
            ErrorCategory::InvalidArgument,
        ));
    }
    let digest = hash_input("create", &input.id, &input).map_err(storage_failure)?;
    if let Some(receipt) = prior(db, tenant, key).await.map_err(storage_failure)? {
        return replay(receipt, &digest, &input.id);
    }
    let id = match AccountId::try_new(&input.id) {
        Ok(v) => v,
        Err(e) => return Err(rejected(e.code, ErrorCategory::InvalidArgument)),
    };
    let party = match PartyReference::try_new(&input.party_id) {
        Ok(v) => v,
        Err(e) => return Err(rejected(e.code, ErrorCategory::InvalidArgument)),
    };
    if !party_exists(db, tenant, party.as_str())
        .await
        .map_err(storage_failure)?
    {
        return Err(rejected(
            "CUSTOMER_ACCOUNTS_PARTY_NOT_FOUND",
            ErrorCategory::InvalidArgument,
        ));
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
        Err(e) => return Err(rejected(e.code, ErrorCategory::InvalidArgument)),
    };
    let snapshot = state(&account).map_err(storage_failure)?;
    let stored=bind(db,
        "INSERT INTO crm_account_records(tenant,id,primary_party,state,version,idem_key,idem_digest,request_id,actor,search_name) VALUES(?,?,?,?,?,?,?,?,?,?) ON CONFLICT(tenant,id) DO NOTHING",
        &[JsValue::from_str(tenant),JsValue::from_str(account.account_id().as_str()),
          JsValue::from_str(&input.party_id),JsValue::from_str(&snapshot),
          JsValue::from_f64(account.version() as f64),JsValue::from_str(key),
          JsValue::from_str(&digest),JsValue::from_str(trace),
           JsValue::from_str(&actor.identity),JsValue::from_str(&crm_core_contracts::nfc_text_key(account.name()))]).map_err(storage_failure)?.run().await;
    match d1_write::confirm(
        stored,
        prior(db, tenant, key),
        "CUSTOMER_ACCOUNTS_CREATE_CONFLICT",
        UNCONFIRMED,
    )
    .await?
    {
        Confirmation::Applied => Ok(MutationOutcome {
            account,
            replayed: false,
            original_request_id: Some(trace.to_owned()),
        }),
        Confirmation::Replayed(receipt) => replay(receipt, &digest, &input.id),
    }
}
pub(super) async fn update_typed(
    input: UpdateInput,
    key: &str,
    db: &D1Database,
    actor: &access::Actor,
    id: &str,
    trace: &str,
) -> WriteResult<MutationOutcome> {
    let tenant = &actor.tenant;
    if !(8..=120).contains(&key.len()) {
        return Err(rejected(
            "IDEMPOTENCY_KEY_REQUIRED",
            ErrorCategory::InvalidArgument,
        ));
    }
    let digest = hash_input("update", id, &input).map_err(storage_failure)?;
    // Read the receipt after the snapshot so a concurrent replay wins over stale state.
    let current = load(db, tenant, id).await;
    if let Some(receipt) = prior(db, tenant, key).await.map_err(storage_failure)? {
        return replay(receipt, &digest, id);
    }
    let Some(mut account) = current.map_err(storage_failure)? else {
        return Err(rejected("NOT_FOUND", ErrorCategory::NotFound));
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
        return Err(rejected(
            e.code.clone(),
            if e.code == "CUSTOMER_ACCOUNTS_VERSION_CONFLICT" {
                ErrorCategory::Conflict
            } else {
                ErrorCategory::InvalidArgument
            },
        ));
    }
    let snapshot = state(&account).map_err(storage_failure)?;
    let changed=bind(db,
        "UPDATE crm_account_records SET state=?,version=?,idem_key=?,idem_digest=?,request_id=?,actor=?,search_name=? WHERE tenant=? AND id=? AND version=?",
        &[JsValue::from_str(&snapshot),JsValue::from_f64(account.version() as f64),
          JsValue::from_str(key),JsValue::from_str(&digest),JsValue::from_str(trace),
          JsValue::from_str(&actor.identity),JsValue::from_str(&crm_core_contracts::nfc_text_key(account.name())),JsValue::from_str(tenant),JsValue::from_str(id),
          JsValue::from_f64(input.expected_version as f64)]).map_err(storage_failure)?.run().await;
    match d1_write::confirm(
        changed,
        prior(db, tenant, key),
        "CUSTOMER_ACCOUNTS_VERSION_CONFLICT",
        UNCONFIRMED,
    )
    .await?
    {
        Confirmation::Applied => Ok(MutationOutcome {
            account,
            replayed: false,
            original_request_id: Some(trace.to_owned()),
        }),
        Confirmation::Replayed(receipt) => replay(receipt, &digest, id),
    }
}
pub(super) async fn list_page_typed(
    db: &D1Database,
    tenant: &str,
    after: &str,
    size: usize,
    status: Option<AccountStatus>,
    search: &str,
) -> Result<(Vec<Account>, Option<String>)> {
    // Original Account persisted state has top-level snake_case status.
    // Select and page the filtered rows in the same tenant-scoped D1 query.
    let filter = status.map(|value| match value {
        AccountStatus::Active => "active",
        AccountStatus::Inactive => "inactive",
    });
    let mut query = "SELECT id,state FROM crm_account_records WHERE tenant=? AND id>?".to_owned();
    let mut params = vec![JsValue::from_str(tenant), JsValue::from_str(after)];
    if let Some(value) = filter {
        query.push_str(" AND json_extract(state,'$.status')=?");
        params.push(JsValue::from_str(value));
    }
    if !search.is_empty() {
        query.push_str(" AND (instr(search_name,?)>0 OR instr(id,?)>0)");
        params.extend([
            JsValue::from_str(&crm_core_contracts::nfc_text_key(search)),
            JsValue::from_str(search),
        ]);
    }
    query.push_str(" ORDER BY id LIMIT ?");
    params.push(JsValue::from_f64((size + 1) as f64));
    let rows = bind(db, &query, &params)?.all().await?.results::<Row>()?;
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
