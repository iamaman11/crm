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
#[derive(Deserialize)]
struct StoredVersioned {
    state: String,
    version: i64,
}
#[derive(Deserialize)]
pub(super) struct ImportSource {
    pub(super) id: String,
    pub(super) display_name: String,
}
#[derive(Deserialize)]
struct ImportSourceRecord {
    id: String,
    display_name: String,
    normalized_name: String,
}
#[derive(Deserialize)]
struct SourceIdentityRecord {
    party_id: String,
}
#[derive(Deserialize)]
struct ImportSourceReceiptRecord {
    source_id: String,
    request_id: String,
}
pub(super) struct SourceImportRow {
    pub(super) party_id: String,
    pub(super) kind: PartyKind,
    pub(super) display_name: String,
    pub(super) row_key: String,
    pub(super) external_id_sha256: String,
    pub(super) expected_version: Option<i64>,
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

pub(super) async fn stored_versioned(
    db: &D1Database,
    tenant: &str,
    id: &str,
) -> Result<Option<(Party, i64)>> {
    let row = bind(
        db,
        "SELECT state,version FROM crm_party_records WHERE tenant = ? AND id = ?",
        &[JsValue::from_str(tenant), JsValue::from_str(id)],
    )?
    .first::<StoredVersioned>(None)
    .await?;
    row.map(|row| {
        decode_party_state(row.state.as_bytes())
            .map(|party| (party, row.version))
            .map_err(|error| worker::Error::RustError(error.code))
    })
    .transpose()
}

pub(super) async fn list_import_sources(
    db: &D1Database,
    tenant: &str,
) -> Result<Vec<ImportSource>> {
    bind(
        db,
        "SELECT id,display_name FROM crm_party_import_sources WHERE tenant=? ORDER BY normalized_name,id",
        &[JsValue::from_str(tenant)],
    )?
    .all()
    .await?
    .results::<ImportSource>()
}

async fn import_source_receipt(
    db: &D1Database,
    tenant: &str,
    key: &str,
) -> Result<Option<ImportSourceReceiptRecord>> {
    bind(
        db,
        "SELECT source_id,request_id FROM crm_party_import_source_receipts WHERE tenant=? AND key=?",
        &[JsValue::from_str(tenant), JsValue::from_str(key)],
    )?
    .first::<ImportSourceReceiptRecord>(None)
    .await
}

async fn import_source_by_id(
    db: &D1Database,
    tenant: &str,
    source_id: &str,
) -> Result<Option<ImportSource>> {
    bind(
        db,
        "SELECT id,display_name FROM crm_party_import_sources WHERE tenant=? AND id=?",
        &[JsValue::from_str(tenant), JsValue::from_str(source_id)],
    )?
    .first::<ImportSource>(None)
    .await
}

pub(super) async fn register_import_source(
    db: &D1Database,
    actor: &access::Actor,
    source_id: &str,
    display_name: &str,
    normalized_name: &str,
    operation_key: &str,
    trace: &str,
) -> Result<std::result::Result<(ImportSource, ImportSaved), MutationFailure>> {
    let tenant = &actor.tenant;
    if !(8..=120).contains(&operation_key.len()) {
        return Ok(Err(MutationFailure::domain(
            "IMPORT_PRECONDITION_INVALID",
            ErrorCode::InvalidArgument,
        )));
    }

    if let Some(old) = import_source_receipt(db, tenant, operation_key).await? {
        if old.source_id != source_id {
            return Ok(Err(MutationFailure::domain(
                "IDEMPOTENCY_CONFLICT",
                ErrorCode::Aborted,
            )));
        }
        let source = import_source_by_id(db, tenant, source_id)
            .await?
            .ok_or_else(|| worker::Error::RustError("CSV source receipt target missing".into()))?;
        return Ok(Ok((
            source,
            ImportSaved {
                created: 0,
                replayed: 1,
                operation_replayed: true,
                original_request_id: old.request_id,
            },
        )));
    }

    if let Some(existing) = bind(
        db,
        "SELECT id,display_name,normalized_name FROM crm_party_import_sources WHERE tenant=? AND normalized_name=?",
        &[JsValue::from_str(tenant), JsValue::from_str(normalized_name)],
    )?
    .first::<ImportSourceRecord>(None)
    .await?
    {
        if existing.id != source_id || existing.normalized_name != normalized_name {
            return Ok(Err(MutationFailure::domain(
                "IMPORT_SOURCE_IDENTITY_MISMATCH",
                ErrorCode::Aborted,
            )));
        }
    }
    if let Some(existing) = bind(
        db,
        "SELECT id,display_name,normalized_name FROM crm_party_import_sources WHERE tenant=? AND id=?",
        &[JsValue::from_str(tenant), JsValue::from_str(source_id)],
    )?
    .first::<ImportSourceRecord>(None)
    .await?
    {
        if existing.normalized_name != normalized_name {
            return Ok(Err(MutationFailure::domain(
                "IMPORT_SOURCE_IDENTITY_MISMATCH",
                ErrorCode::Aborted,
            )));
        }
    }

    let statements = vec![
        bind(
            db,
            "INSERT OR IGNORE INTO crm_party_import_sources(tenant,id,display_name,normalized_name,created_by,request_id) VALUES(?,?,?,?,?,?)",
            &[
                JsValue::from_str(tenant),
                JsValue::from_str(source_id),
                JsValue::from_str(display_name),
                JsValue::from_str(normalized_name),
                JsValue::from_str(&actor.identity),
                JsValue::from_str(trace),
            ],
        )?,
        bind(
            db,
            "INSERT INTO crm_party_import_source_receipts(tenant,key,source_id,request_id) VALUES(?,?,?,?)",
            &[
                JsValue::from_str(tenant),
                JsValue::from_str(operation_key),
                JsValue::from_str(source_id),
                JsValue::from_str(trace),
            ],
        )?,
    ];
    let statement_count = statements.len();
    let acknowledged = db.batch(statements).await.is_ok_and(|results| {
        results.len() == statement_count && results.iter().all(|result| result.success())
    });
    if !acknowledged {
        if let Some(old) = import_source_receipt(db, tenant, operation_key).await? {
            if old.source_id != source_id {
                return Ok(Err(MutationFailure::domain(
                    "IDEMPOTENCY_CONFLICT",
                    ErrorCode::Aborted,
                )));
            }
            let source = import_source_by_id(db, tenant, source_id)
                .await?
                .ok_or_else(|| {
                    worker::Error::RustError("CSV source receipt target missing".into())
                })?;
            return Ok(Ok((
                source,
                ImportSaved {
                    created: 0,
                    replayed: 1,
                    operation_replayed: true,
                    original_request_id: old.request_id,
                },
            )));
        }
        return Ok(Err(MutationFailure::domain(
            "IMPORT_SOURCE_REGISTER_UNCONFIRMED",
            ErrorCode::Unavailable,
        )));
    }

    let inserted = bind(
        db,
        "SELECT id,display_name,normalized_name FROM crm_party_import_sources WHERE tenant=? AND id=?",
        &[JsValue::from_str(tenant), JsValue::from_str(source_id)],
    )?
    .first::<ImportSourceRecord>(None)
    .await?
    .ok_or_else(|| worker::Error::RustError("CSV source registration readback failed".into()))?;
    if inserted.normalized_name != normalized_name {
        return Ok(Err(MutationFailure::domain(
            "IMPORT_SOURCE_IDENTITY_MISMATCH",
            ErrorCode::Aborted,
        )));
    }
    let receipt = import_source_receipt(db, tenant, operation_key)
        .await?
        .ok_or_else(|| worker::Error::RustError("CSV source receipt readback failed".into()))?;
    if receipt.source_id != source_id {
        return Ok(Err(MutationFailure::domain(
            "IDEMPOTENCY_CONFLICT",
            ErrorCode::Aborted,
        )));
    }
    Ok(Ok((
        ImportSource {
            id: inserted.id,
            display_name: inserted.display_name,
        },
        ImportSaved {
            created: 0,
            replayed: 0,
            operation_replayed: false,
            original_request_id: receipt.request_id,
        },
    )))
}

pub(super) async fn import_source_exists(
    db: &D1Database,
    tenant: &str,
    source_id: &str,
) -> Result<bool> {
    #[derive(Deserialize)]
    struct Exists {
        present: i64,
    }
    Ok(bind(
        db,
        "SELECT 1 AS present FROM crm_party_import_sources WHERE tenant=? AND id=?",
        &[JsValue::from_str(tenant), JsValue::from_str(source_id)],
    )?
    .first::<Exists>(None)
    .await?
    .is_some())
}

pub(super) async fn source_identity(
    db: &D1Database,
    tenant: &str,
    source_id: &str,
    external_id_sha256: &str,
) -> Result<Option<String>> {
    bind(
        db,
        "SELECT party_id FROM crm_party_source_identities WHERE tenant=? AND source_id=? AND external_id_sha256=?",
        &[
            JsValue::from_str(tenant),
            JsValue::from_str(source_id),
            JsValue::from_str(external_id_sha256),
        ],
    )?
    .first::<SourceIdentityRecord>(None)
    .await
    .map(|record| record.map(|record| record.party_id))
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
    pub(super) source_id: Option<String>,
    pub(super) preview_fingerprint: Option<String>,
}
pub(super) async fn import_operation(
    db: &D1Database,
    tenant: &str,
    key: &str,
) -> Result<Option<ImportReceipt>> {
    bind(
        db,
        "SELECT source_sha256,row_count,request_id,source_id,preview_fingerprint FROM crm_party_import_receipts WHERE tenant=? AND key=?",
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
        if old.source_sha256 != source_hash
            || old.row_count != rows.len()
            || old.source_id.is_some()
            || old.preview_fingerprint.is_some()
        {
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

pub(super) async fn import_source_batch(
    db: &D1Database,
    actor: &access::Actor,
    source_id: &str,
    rows: Vec<SourceImportRow>,
    operation_key: &str,
    source_hash: &str,
    preview_fingerprint: &str,
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
        if old.source_sha256 != source_hash
            || old.row_count != rows.len()
            || old.source_id.as_deref() != Some(source_id)
            || old.preview_fingerprint.as_deref() != Some(preview_fingerprint)
        {
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
    if !import_source_exists(db, tenant, source_id).await? {
        return Ok(Err(MutationFailure::domain(
            "IMPORT_SOURCE_NOT_FOUND",
            ErrorCode::NotFound,
        )));
    }

    let mut statements = Vec::with_capacity(rows.len() * 2 + 1);
    let mut party_ids = BTreeSet::new();
    let mut external_ids = BTreeSet::new();
    let mut created = 0;
    let mut replayed = 0;
    for row in &rows {
        if !party_ids.insert(row.party_id.clone())
            || !external_ids.insert(row.external_id_sha256.clone())
        {
            return Ok(Err(MutationFailure::domain(
                "IMPORT_DUPLICATE_OR_EMPTY_SOURCE_ID",
                ErrorCode::InvalidArgument,
            )));
        }
        let mapped = source_identity(db, tenant, source_id, &row.external_id_sha256).await?;
        if let Some(mapped_party_id) = mapped {
            let Some(expected_version) = row.expected_version else {
                return Ok(Err(MutationFailure::domain(
                    "IMPORT_PREVIEW_STALE",
                    ErrorCode::Aborted,
                )));
            };
            if mapped_party_id != row.party_id {
                return Ok(Err(MutationFailure::domain(
                    "IMPORT_PREVIEW_STALE",
                    ErrorCode::Aborted,
                )));
            }
            let Some((party, current_version)) =
                stored_versioned(db, tenant, &row.party_id).await?
            else {
                return Ok(Err(MutationFailure::domain(
                    "IMPORT_SOURCE_IDENTITY_TARGET_MISSING",
                    ErrorCode::Aborted,
                )));
            };
            if current_version != expected_version
                || party.kind() != row.kind
                || party.display_name() != row.display_name
            {
                return Ok(Err(MutationFailure::domain(
                    "IMPORT_PREVIEW_STALE",
                    ErrorCode::Aborted,
                )));
            }
            // The migration trigger turns this no-op CAS into a transactional
            // version guard. A Party update after preview aborts the whole batch.
            statements.push(bind(
                db,
                "UPDATE crm_party_source_identities SET party_version=? WHERE tenant=? AND source_id=? AND external_id_sha256=? AND party_id=?",
                &[
                    JsValue::from_f64(expected_version as f64),
                    JsValue::from_str(tenant),
                    JsValue::from_str(source_id),
                    JsValue::from_str(&row.external_id_sha256),
                    JsValue::from_str(&row.party_id),
                ],
            )?);
            replayed += 1;
        } else {
            if row.expected_version.is_some()
                || stored_versioned(db, tenant, &row.party_id).await?.is_some()
            {
                return Ok(Err(MutationFailure::domain(
                    "IMPORT_PREVIEW_STALE",
                    ErrorCode::Aborted,
                )));
            }
            let hash = digest("create", &row.party_id, &row.display_name, 0);
            let party = Party::create(CreateParty {
                party_id: PartyId::try_new(row.party_id.clone())
                    .map_err(|error| worker::Error::RustError(error.code))?,
                kind: row.kind,
                display_name: row.display_name.clone(),
                occurred_at_unix_nanos: now_nanos(),
            })
            .map_err(|error| worker::Error::RustError(error.code))?;
            let state = String::from_utf8(
                encode_party_state(&party).map_err(|error| worker::Error::RustError(error.code))?,
            )
            .map_err(|error| worker::Error::RustError(error.to_string()))?;
            statements.push(bind(
                db,
                "INSERT INTO crm_party_records(tenant,id,state,version,idem_key,idem_digest,request_id,actor,search_name) VALUES(?,?,?,?,?,?,?,?,?)",
                &[
                    JsValue::from_str(tenant),
                    JsValue::from_str(&row.party_id),
                    JsValue::from_str(&state),
                    JsValue::from_f64(1.0),
                    JsValue::from_str(&row.row_key),
                    JsValue::from_str(&hash),
                    JsValue::from_str(trace),
                    JsValue::from_str(&actor.identity),
                    JsValue::from_str(&crm_core_contracts::nfc_text_key(party.display_name())),
                ],
            )?);
            statements.push(bind(
                db,
                "INSERT INTO crm_party_source_identities(tenant,source_id,external_id_sha256,party_id,party_version,request_id,actor) VALUES(?,?,?,?,?,?,?)",
                &[
                    JsValue::from_str(tenant),
                    JsValue::from_str(source_id),
                    JsValue::from_str(&row.external_id_sha256),
                    JsValue::from_str(&row.party_id),
                    JsValue::from_f64(1.0),
                    JsValue::from_str(trace),
                    JsValue::from_str(&actor.identity),
                ],
            )?);
            created += 1;
        }
    }
    statements.push(bind(
        db,
        "INSERT INTO crm_party_import_receipts(tenant,key,source_sha256,row_count,request_id,source_id,preview_fingerprint) VALUES(?,?,?,?,?,?,?)",
        &[
            JsValue::from_str(tenant),
            JsValue::from_str(operation_key),
            JsValue::from_str(source_hash),
            JsValue::from_f64(rows.len() as f64),
            JsValue::from_str(trace),
            JsValue::from_str(source_id),
            JsValue::from_str(preview_fingerprint),
        ],
    )?);
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
        created,
        replayed,
        operation_replayed: replayed > 0,
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
