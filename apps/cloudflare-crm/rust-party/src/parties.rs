//! Original crm-parties HTTP/D1 adapter. Business invariants belong only to
//! the existing crm-parties crate; this file owns transport/persistence only.
use super::{access, bind, now_nanos};
use crm_parties::{
    CreateParty, Party, PartyId, PartyKind, UpdateParty, decode_party_state, encode_party_state,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use wasm_bindgen::JsValue;
use worker::{D1Database, Result};

#[derive(Deserialize)]
struct Stored {
    state: String,
}
use super::d1_write::Receipt;
#[derive(Serialize, Deserialize)]
struct PartyRow {
    id: String,
    state: String,
}

fn digest(kind: &str, id: &str, name: &str, version: i64) -> String {
    let bytes = serde_json::to_vec(&json!([kind, id, name, version])).expect("static JSON");
    format!("{:x}", Sha256::digest(bytes))
}

pub(super) async fn stored(db: &D1Database, tenant: &str, id: &str) -> Result<Option<Party>> {
    let row = bind(
        db,
        "SELECT state FROM crm_party_records WHERE tenant = ? AND id = ?",
        &[JsValue::from_str(tenant), JsValue::from_str(id)],
    )?
    .first::<Stored>(None)
    .await?;
    row.map(|r| {
        decode_party_state(r.state.as_bytes()).map_err(|e| worker::Error::RustError(e.code))
    })
    .transpose()
}
/// Only this Party adapter persists Party creation, including CSV import.
/// D1 batch is atomic: all Party states, receipts and audits or none.
pub(super) async fn import_receipt_matches(
    db: &D1Database,
    tenant: &str,
    key: &str,
    id: &str,
    kind: PartyKind,
    name: &str,
) -> Result<bool> {
    let Some(old) = previous(db, tenant, key).await? else {
        return Ok(false);
    };
    if old.digest != digest("create", id, name, 0) {
        return Ok(false);
    }
    let party =
        decode_party_state(old.state.as_bytes()).map_err(|e| worker::Error::RustError(e.code))?;
    Ok(party.party_id().as_str() == id && party.kind() == kind)
}

pub(super) struct ImportSaved {
    pub(super) created: usize,
    pub(super) replayed: usize,
    pub(super) operation_replayed: bool,
    pub(super) original_request_id: String,
}
#[derive(Deserialize)]
pub(super) struct ImportReceipt {
    pub(super) source_sha256: String,
    pub(super) row_count: usize,
    pub(super) request_id: String,
}
pub(super) async fn import_operation(
    db: &D1Database,
    tenant: &str,
    key: &str,
) -> Result<Option<ImportReceipt>> {
    bind(
        db,
        "SELECT source_sha256,row_count,request_id FROM crm_party_import_receipts WHERE tenant=? AND key=?",
        &[JsValue::from_str(tenant), JsValue::from_str(key)],
    )?
    .first::<ImportReceipt>(None)
    .await
}
pub(super) async fn import_batch(
    db: &D1Database,
    actor: &access::Actor,
    rows: Vec<(String, PartyKind, String, String)>,
    operation_key: &str,
    source_hash: &str,
    trace: &str,
) -> Result<std::result::Result<ImportSaved, MutationFailure>> {
    let tenant = &actor.tenant;
    if !(8..=120).contains(&operation_key.len()) || rows.is_empty() {
        return Ok(Err(MutationFailure::domain(
            "IMPORT_PRECONDITION_INVALID",
            ErrorCode::InvalidArgument,
        )));
    }
    let replay = |old: ImportReceipt| {
        if old.source_sha256 != source_hash || old.row_count != rows.len() {
            return Ok(Err(MutationFailure::domain(
                "IDEMPOTENCY_CONFLICT",
                ErrorCode::Aborted,
            )));
        }
        Ok(Ok(ImportSaved {
            created: 0,
            replayed: old.row_count,
            operation_replayed: true,
            original_request_id: old.request_id,
        }))
    };
    if let Some(old) = import_operation(db, tenant, operation_key).await? {
        return replay(old);
    }
    let mut statements = Vec::with_capacity(rows.len() + 1);
    let mut ids = BTreeSet::new();
    let mut receipts = 0;
    for (id, kind, name, key) in &rows {
        if !ids.insert(id.clone()) {
            return Ok(Err(MutationFailure::domain(
                "IMPORT_DUPLICATE_TARGET_ID",
                ErrorCode::InvalidArgument,
            )));
        }
        let hash = digest("create", id, name, 0);
        let current = stored(db, tenant, id).await;
        if let Some(old) = previous(db, tenant, key).await? {
            if old.digest != hash {
                return Ok(Err(MutationFailure::domain(
                    "IDEMPOTENCY_CONFLICT",
                    ErrorCode::Aborted,
                )));
            }
            let party = decode_party_state(old.state.as_bytes())
                .map_err(|e| worker::Error::RustError(e.code))?;
            if party.party_id().as_str() != id || party.kind() != *kind {
                return Ok(Err(MutationFailure::domain(
                    "IMPORT_RECEIPT_ID_CONFLICT",
                    ErrorCode::Aborted,
                )));
            }
            receipts += 1;
            continue;
        }
        if current?.is_some() {
            return Ok(Err(MutationFailure::domain(
                "IMPORT_TARGET_PARTY_CONFLICT",
                ErrorCode::Aborted,
            )));
        }
        let party = Party::create(CreateParty {
            party_id: PartyId::try_new(id.clone()).map_err(|e| worker::Error::RustError(e.code))?,
            kind: *kind,
            display_name: name.clone(),
            occurred_at_unix_nanos: now_nanos(),
        })
        .map_err(|e| worker::Error::RustError(e.code))?;
        let state = String::from_utf8(
            encode_party_state(&party).map_err(|e| worker::Error::RustError(e.code))?,
        )
        .map_err(|e| worker::Error::RustError(e.to_string()))?;
        statements.push(bind(db,
            "INSERT INTO crm_party_records(tenant,id,state,version,idem_key,idem_digest,request_id,actor,search_name) VALUES(?,?,?,?,?,?,?,?,?)",
            &[JsValue::from_str(tenant),JsValue::from_str(id),
              JsValue::from_str(&state),JsValue::from_f64(1.0),
              JsValue::from_str(key),JsValue::from_str(&hash),
              JsValue::from_str(trace),JsValue::from_str(&actor.identity),JsValue::from_str(&crm_core_contracts::nfc_text_key(party.display_name()))])?);
    }
    if receipts > 0 && receipts != rows.len() {
        if let Some(old) = import_operation(db, tenant, operation_key).await? {
            return replay(old);
        }
        return Ok(Err(MutationFailure::domain(
            "IMPORT_PARTIAL_REPLAY_CONFLICT",
            ErrorCode::Aborted,
        )));
    }
    // Operation-key receipt and all original Party writes commit in one D1 batch.
    // The UNIQUE(tenant,key) constraint also serializes concurrent retry attempts.
    statements.push(bind(db,
        "INSERT INTO crm_party_import_receipts(tenant,key,source_sha256,row_count,request_id) VALUES(?,?,?,?,?)",
        &[JsValue::from_str(tenant),JsValue::from_str(operation_key),
          JsValue::from_str(source_hash),JsValue::from_f64(rows.len() as f64),
          JsValue::from_str(trace)])?);
    let statement_count = statements.len();
    let acknowledged = db.batch(statements).await.is_ok_and(|results| {
        results.len() == statement_count && results.iter().all(|result| result.success())
    });
    if !acknowledged {
        let old = match super::d1_write::require_receipt(
            import_operation(db, tenant, operation_key),
            "IMPORT_WRITE_UNCONFIRMED",
        )
        .await
        {
            Ok(old) => old,
            Err(failure) => return Ok(Err(failure)),
        };
        return replay(old);
    }
    Ok(Ok(ImportSaved {
        created: rows.len() - receipts,
        replayed: receipts,
        operation_replayed: receipts > 0,
        original_request_id: trace.to_owned(),
    }))
}

async fn previous(db: &D1Database, tenant: &str, key: &str) -> Result<Option<Receipt>> {
    bind(
        db,
        "SELECT digest,state,original_request_id FROM crm_party_receipts WHERE tenant = ? AND key = ?",
        &[JsValue::from_str(tenant), JsValue::from_str(key)],
    )?
    .first::<Receipt>(None)
    .await
}

// The ONLY Party persistence implementation. Both the historical (blocked)
// HTTP call and the generated Connect service use these original Rust owner
// commands. Do not add another Party aggregate or duplicate D1 writes.
pub(super) struct MutationOutcome {
    pub(super) party: Party,
    pub(super) replayed: bool,
    pub(super) original_request_id: Option<String>,
}
pub(super) use super::d1_write::OwnerFailure as MutationFailure;
use connectrpc::ErrorCode;
pub(super) async fn create_typed(
    db: &D1Database,
    actor: &access::Actor,
    id: String,
    kind: PartyKind,
    name: String,
    key: &str,
    request_id: &str,
) -> Result<std::result::Result<MutationOutcome, MutationFailure>> {
    let tenant = &actor.tenant;
    if !(8..=120).contains(&key.len()) {
        return Ok(Err(MutationFailure::domain(
            "IDEMPOTENCY_KEY_REQUIRED",
            ErrorCode::InvalidArgument,
        )));
    }
    let hash = digest("create", &id, &name, 0);
    let replay = |old: Receipt| {
        if old.digest != hash {
            return Ok(Err(MutationFailure::domain(
                "IDEMPOTENCY_CONFLICT",
                ErrorCode::Aborted,
            )));
        }
        let party = decode_party_state(old.state.as_bytes())
            .map_err(|e| worker::Error::RustError(e.code))?;
        if party.party_id().as_str() != id || party.kind() != kind {
            return Ok(Err(MutationFailure::domain(
                "IDEMPOTENCY_CONFLICT",
                ErrorCode::Aborted,
            )));
        }
        Ok(Ok(MutationOutcome {
            party,
            replayed: true,
            original_request_id: old.original_request_id,
        }))
    };
    if let Some(old) = previous(db, tenant, key).await? {
        return replay(old);
    }
    let id = match PartyId::try_new(id.clone()) {
        Ok(value) => value,
        Err(_) => {
            return Ok(Err(MutationFailure::domain(
                "PARTIES_PARTY_ID_INVALID",
                ErrorCode::InvalidArgument,
            )));
        }
    };
    let party = match Party::create(CreateParty {
        party_id: id,
        kind,
        display_name: name,
        occurred_at_unix_nanos: now_nanos(),
    }) {
        Ok(value) => value,
        Err(e) => {
            return Ok(Err(MutationFailure::domain(
                e.code,
                ErrorCode::InvalidArgument,
            )));
        }
    };
    let state = String::from_utf8(
        encode_party_state(&party).map_err(|e| worker::Error::RustError(e.code))?,
    )
    .map_err(|e| worker::Error::RustError(e.to_string()))?;
    // Existing D1 triggers write the audit and idempotency receipt atomically.
    let result = bind(db,
        "INSERT INTO crm_party_records(tenant,id,state,version,idem_key,idem_digest,request_id,actor,search_name) VALUES(?,?,?,?,?,?,?,?,?) ON CONFLICT(tenant,id) DO NOTHING",
        &[JsValue::from_str(tenant), JsValue::from_str(party.party_id().as_str()),
          JsValue::from_str(&state), JsValue::from_f64(1.0),
          JsValue::from_str(key), JsValue::from_str(&hash),
          JsValue::from_str(request_id), JsValue::from_str(&actor.identity), JsValue::from_str(&crm_core_contracts::nfc_text_key(party.display_name()))])?.run().await;
    match super::d1_write::confirm(
        result,
        previous(db, tenant, key),
        "PARTIES_PARTY_CREATE_CONFLICT",
        "PARTIES_WRITE_UNCONFIRMED",
    )
    .await
    {
        Ok(super::d1_write::Confirmation::Applied) => {}
        Ok(super::d1_write::Confirmation::Replayed(saved)) => return replay(saved),
        Err(failure) => return Ok(Err(MutationFailure::Write(failure))),
    }
    Ok(Ok(MutationOutcome {
        party,
        replayed: false,
        original_request_id: Some(request_id.to_owned()),
    }))
}
pub(super) async fn update_typed(
    db: &D1Database,
    actor: &access::Actor,
    id: &str,
    name: String,
    expected_version: i64,
    key: &str,
    request_id: &str,
) -> Result<std::result::Result<MutationOutcome, MutationFailure>> {
    let tenant = &actor.tenant;
    if !(8..=120).contains(&key.len()) {
        return Ok(Err(MutationFailure::domain(
            "IDEMPOTENCY_KEY_REQUIRED",
            ErrorCode::InvalidArgument,
        )));
    }
    let hash = digest("update", id, &name, expected_version);
    let replay = |old: Receipt| {
        if old.digest != hash {
            return Ok(Err(MutationFailure::domain(
                "IDEMPOTENCY_CONFLICT",
                ErrorCode::Aborted,
            )));
        }
        let party = decode_party_state(old.state.as_bytes())
            .map_err(|e| worker::Error::RustError(e.code))?;
        if party.party_id().as_str() != id {
            return Ok(Err(MutationFailure::domain(
                "IDEMPOTENCY_CONFLICT",
                ErrorCode::Aborted,
            )));
        }
        Ok(Ok(MutationOutcome {
            party,
            replayed: true,
            original_request_id: old.original_request_id,
        }))
    };
    let current = stored(db, tenant, id).await;
    if let Some(old) = previous(db, tenant, key).await? {
        return replay(old);
    }
    let Some(mut party) = current? else {
        return Ok(Err(MutationFailure::domain(
            "NOT_FOUND",
            ErrorCode::NotFound,
        )));
    };
    if let Err(e) = party.apply_update(UpdateParty {
        expected_version,
        display_name: name,
        occurred_at_unix_nanos: now_nanos(),
    }) {
        let status = if e.code == "PARTIES_PARTY_VERSION_CONFLICT" {
            ErrorCode::Aborted
        } else {
            ErrorCode::InvalidArgument
        };
        return Ok(Err(MutationFailure::domain(e.code, status)));
    }
    let state = String::from_utf8(
        encode_party_state(&party).map_err(|e| worker::Error::RustError(e.code))?,
    )
    .map_err(|e| worker::Error::RustError(e.to_string()))?;
    let result = bind(db,
        "UPDATE crm_party_records SET state=?,version=?,idem_key=?,idem_digest=?,request_id=?,actor=?,search_name=? WHERE tenant=? AND id=? AND version=?",
        &[JsValue::from_str(&state), JsValue::from_f64(party.version() as f64),
          JsValue::from_str(key), JsValue::from_str(&hash),
          JsValue::from_str(request_id), JsValue::from_str(&actor.identity), JsValue::from_str(&crm_core_contracts::nfc_text_key(party.display_name())), JsValue::from_str(tenant),
          JsValue::from_str(id), JsValue::from_f64(expected_version as f64)])?.run().await;
    match super::d1_write::confirm(
        result,
        previous(db, tenant, key),
        "PARTIES_PARTY_VERSION_CONFLICT",
        "PARTIES_WRITE_UNCONFIRMED",
    )
    .await
    {
        Ok(super::d1_write::Confirmation::Applied) => {}
        Ok(super::d1_write::Confirmation::Replayed(saved)) => return replay(saved),
        Err(failure) => return Ok(Err(MutationFailure::Write(failure))),
    }
    Ok(Ok(MutationOutcome {
        party,
        replayed: false,
        original_request_id: Some(request_id.to_owned()),
    }))
}
pub(super) async fn list_page_typed(
    db: &D1Database,
    tenant: &str,
    after: &str,
    page_size: usize,
    kind: Option<PartyKind>,
    search: &str,
) -> Result<(Vec<Party>, Option<String>)> {
    // The original codec stores kind as top-level snake_case JSON.
    // Filter before LIMIT so a page cursor never skips matching records.
    let filter = kind.map(|value| match value {
        PartyKind::Person => "person",
        PartyKind::Organization => "organization",
    });
    let mut query = "SELECT id,state FROM crm_party_records WHERE tenant=? AND id>?".to_owned();
    let mut params = vec![JsValue::from_str(tenant), JsValue::from_str(after)];
    if let Some(value) = filter {
        query.push_str(" AND json_extract(state,'$.kind')=?");
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
    params.push(JsValue::from_f64((page_size + 1) as f64));
    let rows = bind(db, &query, &params)?
        .all()
        .await?
        .results::<PartyRow>()?;
    let has_more = rows.len() > page_size;
    let next = if has_more {
        rows.get(page_size - 1).map(|r| r.id.clone())
    } else {
        None
    };
    let mut items = Vec::new();
    for row in rows.iter().take(page_size) {
        let party = decode_party_state(row.state.as_bytes())
            .map_err(|e| worker::Error::RustError(e.code))?;
        items.push(party);
    }
    Ok((items, next))
}
