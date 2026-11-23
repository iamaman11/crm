//! Read-only original crm-identity-resolution boundary. The domain crate
//! exclusively decides observed same-email pairs; there is NO merge or identity
//! confirmation. Connect uses these typed D1 reads.
use crm_contact_points::{ContactPointKind, ContactPointStatus};
use crm_identity_resolution::{
    ActiveMergeEdge, CanonicalPartyGraph, CanonicalPartyPair, CreateMergeOperation,
    DecisionReference, LineageDecisionReasonCode, MergeOperation, MergeOperationId,
    MergeOperationStatus, PartyReference, RecordedEmailContact, UnmergeMergeOperation,
    decode_merge_operation_state, discover_shared_recorded_emails, encode_merge_operation_state,
};
use crm_module_sdk::ActorId;
use crm_parties::Party;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use wasm_bindgen::JsValue;
use worker::{D1Database, Result};

use super::{access, bind, contact_points, now_nanos, parties};

#[derive(Deserialize)]
struct ContactRow {
    state: String,
}

pub(super) enum CandidateError {
    ScanLimit,
    SourceChanged,
}
pub(super) struct Candidate {
    pub(super) first: Party,
    pub(super) second: Party,
    pub(super) first_contact_point_id: String,
    pub(super) second_contact_point_id: String,
    pub(super) both_verified: bool,
}

/// The canonical candidate computation is performed ONCE by the original
/// crm-identity-resolution domain. No ContactPoint claim becomes proof.
pub(super) async fn candidates_typed(
    db: &D1Database,
    tenant: &str,
) -> Result<std::result::Result<Vec<Candidate>, CandidateError>> {
    // Refuse a silently truncated scan instead of inventing complete coverage.
    let rows = bind(
        db,
        "SELECT state FROM crm_contact_point_records WHERE tenant = ? ORDER BY id LIMIT 301",
        &[JsValue::from_str(tenant)],
    )?
    .all()
    .await?
    .results::<ContactRow>()?;
    if rows.len() > 300 {
        return Ok(Err(CandidateError::ScanLimit));
    }
    let now = now_nanos();
    let mut contacts = Vec::new();
    for row in rows {
        let point = contact_points::decode(&row.state)?;
        if point.kind() != ContactPointKind::Email
            || point.status() != ContactPointStatus::Active
            || !point.is_valid_at(now)
        {
            continue;
        }
        let party = PartyReference::try_new(point.party_ref().as_str())
            .map_err(|e| worker::Error::RustError(e.code))?;
        contacts.push(RecordedEmailContact {
            party,
            contact_id: point.contact_point_id().as_str().to_owned(),
            normalized_email: point.normalized_value().to_owned(),
            verified: point.verification().is_verified(),
        });
    }
    let observations =
        discover_shared_recorded_emails(&contacts).map_err(|e| worker::Error::RustError(e.code))?;
    let mut found = Vec::with_capacity(observations.len());
    for item in observations {
        let Some(first) = parties::stored(db, tenant, item.pair.left().as_str()).await? else {
            return Ok(Err(CandidateError::SourceChanged));
        };
        let Some(second) = parties::stored(db, tenant, item.pair.right().as_str()).await? else {
            return Ok(Err(CandidateError::SourceChanged));
        };
        found.push(Candidate {
            first,
            second,
            first_contact_point_id: item.left_contact_id,
            second_contact_point_id: item.right_contact_id,
            both_verified: item.both_verified,
        });
    }
    Ok(Ok(found))
}

pub(super) struct PartyComparison {
    pub(super) first: Party,
    pub(super) second: Party,
}

/// Caller must first build canonical pair using the original domain; this
/// reader always returns selected-first order, never claims identity.
pub(super) async fn compare_typed(
    db: &D1Database,
    tenant: &str,
    pair: &CanonicalPartyPair,
    first_id: &str,
) -> Result<Option<PartyComparison>> {
    let Some(left) = parties::stored(db, tenant, pair.left().as_str()).await? else {
        return Ok(None);
    };
    let Some(right) = parties::stored(db, tenant, pair.right().as_str()).await? else {
        return Ok(None);
    };
    let (first, second) = if first_id == pair.left().as_str() {
        (left, right)
    } else {
        (right, left)
    };
    Ok(Some(PartyComparison { first, second }))
}

/** The original merge-lineage owner never deletes/rewires business records.
 * D1 records active canonical edges plus strict original-domain JSON state. */
#[derive(Deserialize)]
struct MergeStateRow {
    state: String,
}
#[derive(Deserialize)]
pub(super) struct MergeReceipt {
    pub(super) digest: String,
    pub(super) state: String,
    pub(super) original_request_id: String,
}
pub(super) struct MergeCommit {
    pub(super) operation: MergeOperation,
    pub(super) replayed: bool,
    pub(super) original_request_id: String,
}
pub(super) use super::d1_write::OwnerFailure as MergeFault;
use connectrpc::ErrorCode;
fn committed_merge(
    receipt: MergeReceipt,
    digest: &str,
    operation_id: &str,
    version: i64,
    request_id: &str,
) -> Result<std::result::Result<MergeCommit, MergeFault>> {
    if receipt.digest != digest {
        return Ok(Err(MergeFault::domain(
            "IDEMPOTENCY_CONFLICT",
            ErrorCode::Aborted,
        )));
    }
    let operation = merge_decode(&receipt.state)?;
    if operation.operation_id().as_str() != operation_id || operation.version() != version {
        return Ok(Err(MergeFault::domain(
            "IDENTITY_RECEIPT_MISMATCH",
            ErrorCode::Unavailable,
        )));
    }
    Ok(Ok(MergeCommit {
        operation,
        replayed: receipt.original_request_id != request_id,
        original_request_id: receipt.original_request_id,
    }))
}
fn merge_decode(state: &str) -> Result<MergeOperation> {
    decode_merge_operation_state(state.as_bytes()).map_err(|e| worker::Error::RustError(e.code))
}
fn merge_encode(operation: &MergeOperation) -> Result<String> {
    String::from_utf8(
        encode_merge_operation_state(operation).map_err(|e| worker::Error::RustError(e.code))?,
    )
    .map_err(|e| worker::Error::RustError(e.to_string()))
}
fn merge_digest(values: &serde_json::Value) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(values).expect("static digest JSON"))
    )
}
pub(super) async fn stored_merge(
    db: &D1Database,
    tenant: &str,
    operation_id: &str,
) -> Result<Option<MergeOperation>> {
    let row = bind(
        db,
        "SELECT state FROM crm_identity_merge_records WHERE tenant=? AND id=?",
        &[JsValue::from_str(tenant), JsValue::from_str(operation_id)],
    )?
    .first::<MergeStateRow>(None)
    .await?;
    row.map(|r| merge_decode(&r.state)).transpose()
}
pub(super) async fn merge_receipt(
    db: &D1Database,
    tenant: &str,
    key: &str,
) -> Result<Option<MergeReceipt>> {
    bind(db,"SELECT digest,state,original_request_id FROM crm_identity_merge_receipts WHERE tenant=? AND key=?",
        &[JsValue::from_str(tenant),JsValue::from_str(key)])?
        .first::<MergeReceipt>(None).await
}
pub(super) async fn canonical_graph(db: &D1Database, tenant: &str) -> Result<CanonicalPartyGraph> {
    // Refuse incomplete topology, never silently resolve a truncated edge set.
    let rows = bind(db,
        "SELECT state FROM crm_identity_merge_records WHERE tenant=? AND status='active' ORDER BY id LIMIT 1001",
        &[JsValue::from_str(tenant)])?.all().await?.results::<MergeStateRow>()?;
    if rows.len() > 1000 {
        return Err(worker::Error::RustError(
            "IDENTITY_MERGE_GRAPH_SCAN_LIMIT".to_owned(),
        ));
    }
    let mut edges = Vec::<ActiveMergeEdge>::new();
    for row in rows {
        let operation = merge_decode(&row.state)?;
        let edge = operation.active_edge().ok_or_else(|| {
            worker::Error::RustError("IDENTITY_MERGE_INDEX_STATE_MISMATCH".to_owned())
        })?;
        edges.push(edge);
    }
    CanonicalPartyGraph::try_new(edges).map_err(|e| worker::Error::RustError(e.code))
}

pub(super) struct CreateMergeInput {
    pub(super) operation_id: String,
    pub(super) source_party_id: String,
    pub(super) source_version: i64,
    pub(super) survivor_party_id: String,
    pub(super) survivor_version: i64,
    pub(super) reason: String,
}
pub(super) async fn create_merge_typed(
    db: &D1Database,
    actor: &access::Actor,
    input: CreateMergeInput,
    key: &str,
    request_id: &str,
) -> Result<std::result::Result<MergeCommit, MergeFault>> {
    let CreateMergeInput {
        operation_id,
        source_party_id,
        source_version,
        survivor_party_id,
        survivor_version,
        reason,
    } = input;
    let tenant = &actor.tenant;
    if !(8..=120).contains(&key.len()) {
        return Ok(Err(MergeFault::domain(
            "IDEMPOTENCY_KEY_REQUIRED",
            ErrorCode::InvalidArgument,
        )));
    }
    let hash = merge_digest(&json!([
        "merge",
        operation_id,
        source_party_id,
        source_version,
        survivor_party_id,
        survivor_version,
        reason
    ]));
    let graph = canonical_graph(db, tenant).await;
    let existing = stored_merge(db, tenant, &operation_id).await;
    if let Some(old) = merge_receipt(db, tenant, key).await? {
        return committed_merge(old, &hash, &operation_id, 1, request_id);
    }
    if existing?.is_some() {
        return Ok(Err(MergeFault::domain(
            "IDENTITY_MERGE_OPERATION_EXISTS",
            ErrorCode::Aborted,
        )));
    }
    let Some(source) = parties::stored(db, tenant, &source_party_id).await? else {
        return Ok(Err(MergeFault::domain(
            "IDENTITY_MERGE_SOURCE_NOT_FOUND",
            ErrorCode::NotFound,
        )));
    };
    let Some(survivor) = parties::stored(db, tenant, &survivor_party_id).await? else {
        return Ok(Err(MergeFault::domain(
            "IDENTITY_MERGE_SURVIVOR_NOT_FOUND",
            ErrorCode::NotFound,
        )));
    };
    if source.version() != source_version || survivor.version() != survivor_version {
        return Ok(Err(MergeFault::domain(
            "IDENTITY_MERGE_STALE_PARTY",
            ErrorCode::Aborted,
        )));
    }
    if source.kind() != survivor.kind() {
        return Ok(Err(MergeFault::domain(
            "IDENTITY_MERGE_KIND_MISMATCH",
            ErrorCode::InvalidArgument,
        )));
    }
    let command = CreateMergeOperation {
        operation_id: match MergeOperationId::try_new(operation_id.clone()) {
            Ok(x) => x,
            Err(e) => return Ok(Err(MergeFault::domain(e.code, ErrorCode::InvalidArgument))),
        },
        source_party_ref: match PartyReference::try_new(source_party_id.clone()) {
            Ok(x) => x,
            Err(e) => return Ok(Err(MergeFault::domain(e.code, ErrorCode::InvalidArgument))),
        },
        source_party_version: source_version,
        survivor_party_ref: match PartyReference::try_new(survivor_party_id.clone()) {
            Ok(x) => x,
            Err(e) => return Ok(Err(MergeFault::domain(e.code, ErrorCode::InvalidArgument))),
        },
        survivor_party_version: survivor_version,
        decision_ref: match DecisionReference::try_new(operation_id.clone()) {
            Ok(x) => x,
            Err(e) => return Ok(Err(MergeFault::domain(e.code, ErrorCode::InvalidArgument))),
        },
        decided_by: match ActorId::try_new(actor.identity.clone()) {
            Ok(x) => x,
            Err(e) => {
                return Ok(Err(MergeFault::domain(
                    e.to_string(),
                    ErrorCode::InvalidArgument,
                )));
            }
        },
        reason: match LineageDecisionReasonCode::try_new(reason) {
            Ok(x) => x,
            Err(e) => return Ok(Err(MergeFault::domain(e.code, ErrorCode::InvalidArgument))),
        },
        survivorship: Vec::new(), // No field is copied or overwritten.
        occurred_at_unix_nanos: now_nanos(),
    };
    let operation = match MergeOperation::create(command) {
        Ok(x) => x,
        Err(e) => return Ok(Err(MergeFault::domain(e.code, ErrorCode::InvalidArgument))),
    };
    let graph = graph?;
    if let Err(e) = graph.with_added_edge(operation.active_edge().expect("new operation active")) {
        return Ok(Err(MergeFault::domain(e.code, ErrorCode::Aborted)));
    }
    let state = merge_encode(&operation)?;
    // The native BEFORE INSERT trigger guards both Party versions and roots
    // atomically. Use an ordinary D1 INSERT, not a conditional SELECT whose
    // changes metadata may be zero with an otherwise successful statement.
    let _acknowledgement = bind(
        db,
        "INSERT INTO crm_identity_merge_records(
            tenant,id,source_party,survivor_party,state,version,status,
            idem_key,idem_digest,request_id,actor)
         VALUES(?,?,?,?,?,1,'active',?,?,?,?)",
        &[
            JsValue::from_str(tenant),
            JsValue::from_str(&operation_id),
            JsValue::from_str(&source_party_id),
            JsValue::from_str(&survivor_party_id),
            JsValue::from_str(&state),
            JsValue::from_str(key),
            JsValue::from_str(&hash),
            JsValue::from_str(request_id),
            JsValue::from_str(&actor.identity),
        ],
    )?
    .run()
    .await;
    // The atomic receipt also covers a lost acknowledgement and concurrent retries.
    let receipt = match super::d1_write::require_receipt(
        merge_receipt(db, tenant, key),
        "IDENTITY_MERGE_COMMIT_UNCONFIRMED",
    )
    .await
    {
        Ok(receipt) => receipt,
        Err(failure) => return Ok(Err(failure)),
    };
    committed_merge(receipt, &hash, &operation_id, 1, request_id)
}

pub(super) struct UnmergeInput<'a> {
    pub(super) operation_id: &'a str,
    pub(super) expected_version: i64,
    pub(super) source_version: i64,
    pub(super) survivor_version: i64,
    pub(super) reason: String,
}
pub(super) async fn unmerge_typed(
    db: &D1Database,
    actor: &access::Actor,
    input: UnmergeInput<'_>,
    key: &str,
    request_id: &str,
) -> Result<std::result::Result<MergeCommit, MergeFault>> {
    let UnmergeInput {
        operation_id,
        expected_version,
        source_version,
        survivor_version,
        reason,
    } = input;
    let tenant = &actor.tenant;
    if !(8..=120).contains(&key.len()) {
        return Ok(Err(MergeFault::domain(
            "IDEMPOTENCY_KEY_REQUIRED",
            ErrorCode::InvalidArgument,
        )));
    }
    let hash = merge_digest(&json!([
        "unmerge",
        operation_id,
        expected_version,
        source_version,
        survivor_version,
        reason
    ]));
    let graph = canonical_graph(db, tenant).await;
    let existing = stored_merge(db, tenant, operation_id).await;
    if let Some(old) = merge_receipt(db, tenant, key).await? {
        return committed_merge(old, &hash, operation_id, 2, request_id);
    }
    let Some(mut operation) = existing? else {
        return Ok(Err(MergeFault::domain(
            "IDENTITY_MERGE_NOT_FOUND",
            ErrorCode::NotFound,
        )));
    };
    let Some(source) = parties::stored(db, tenant, operation.source_party_ref().as_str()).await?
    else {
        return Ok(Err(MergeFault::domain(
            "IDENTITY_MERGE_SOURCE_NOT_FOUND",
            ErrorCode::NotFound,
        )));
    };
    let Some(survivor) =
        parties::stored(db, tenant, operation.survivor_party_ref().as_str()).await?
    else {
        return Ok(Err(MergeFault::domain(
            "IDENTITY_MERGE_SURVIVOR_NOT_FOUND",
            ErrorCode::NotFound,
        )));
    };
    if source.version() != source_version || survivor.version() != survivor_version {
        return Ok(Err(MergeFault::domain(
            "IDENTITY_MERGE_STALE_PARTY",
            ErrorCode::Aborted,
        )));
    }
    let graph = graph?;
    if let Err(e) = graph.without_operation(operation.operation_id()) {
        return Ok(Err(MergeFault::domain(e.code, ErrorCode::Aborted)));
    }
    let input = UnmergeMergeOperation {
        expected_version,
        decision_ref: match DecisionReference::try_new(format!("unmerge-{}", request_id)) {
            Ok(x) => x,
            Err(e) => return Ok(Err(MergeFault::domain(e.code, ErrorCode::InvalidArgument))),
        },
        decided_by: match ActorId::try_new(actor.identity.clone()) {
            Ok(x) => x,
            Err(e) => {
                return Ok(Err(MergeFault::domain(
                    e.to_string(),
                    ErrorCode::InvalidArgument,
                )));
            }
        },
        reason: match LineageDecisionReasonCode::try_new(reason) {
            Ok(x) => x,
            Err(e) => return Ok(Err(MergeFault::domain(e.code, ErrorCode::InvalidArgument))),
        },
        occurred_at_unix_nanos: now_nanos().max(operation.updated_at_unix_nanos() + 1),
    };
    if let Err(e) = operation.unmerge(input) {
        return Ok(Err(MergeFault::domain(e.code, ErrorCode::Aborted)));
    }
    let state = merge_encode(&operation)?;
    let _acknowledgement = bind(
        db,
        "UPDATE crm_identity_merge_records SET state=?,version=2,status='unmerged',
           idem_key=?,idem_digest=?,request_id=?,actor=?
         WHERE tenant=? AND id=? AND version=? AND status='active'
           AND EXISTS(SELECT 1 FROM crm_party_records WHERE tenant=? AND id=? AND version=?)
           AND EXISTS(SELECT 1 FROM crm_party_records WHERE tenant=? AND id=? AND version=?)",
        &[
            JsValue::from_str(&state),
            JsValue::from_str(key),
            JsValue::from_str(&hash),
            JsValue::from_str(request_id),
            JsValue::from_str(&actor.identity),
            JsValue::from_str(tenant),
            JsValue::from_str(operation_id),
            JsValue::from_f64(expected_version as f64),
            JsValue::from_str(tenant),
            JsValue::from_str(operation.source_party_ref().as_str()),
            JsValue::from_f64(source_version as f64),
            JsValue::from_str(tenant),
            JsValue::from_str(operation.survivor_party_ref().as_str()),
            JsValue::from_f64(survivor_version as f64),
        ],
    )?
    .run()
    .await;
    // The atomic receipt also covers a lost acknowledgement and concurrent retries.
    let receipt = match super::d1_write::require_receipt(
        merge_receipt(db, tenant, key),
        "IDENTITY_UNMERGE_COMMIT_UNCONFIRMED",
    )
    .await
    {
        Ok(receipt) => receipt,
        Err(failure) => return Ok(Err(failure)),
    };
    committed_merge(receipt, &hash, operation_id, 2, request_id)
}

pub(super) async fn list_merges(
    db: &D1Database,
    tenant: &str,
    party_id: &str,
    after: &str,
    size: usize,
) -> Result<(Vec<MergeOperation>, Option<String>)> {
    let rows = bind(
        db,
        "SELECT state FROM crm_identity_merge_records WHERE tenant=?
          AND (source_party=? OR survivor_party=?) AND id>? ORDER BY id LIMIT ?",
        &[
            JsValue::from_str(tenant),
            JsValue::from_str(party_id),
            JsValue::from_str(party_id),
            JsValue::from_str(after),
            JsValue::from_f64((size + 1) as f64),
        ],
    )?
    .all()
    .await?
    .results::<MergeStateRow>()?;
    let mut all = rows
        .iter()
        .take(size)
        .map(|r| merge_decode(&r.state))
        .collect::<Result<Vec<_>>>()?;
    let next = if rows.len() > size {
        all.last().map(|x| x.operation_id().as_str().to_owned())
    } else {
        None
    };
    Ok((std::mem::take(&mut all), next))
}
pub(super) async fn resolve_canonical(
    db: &D1Database,
    tenant: &str,
    party_id: &str,
) -> Result<Option<crm_identity_resolution::CanonicalResolution>> {
    let party = parties::stored(db, tenant, party_id).await?;
    let Some(_) = party else {
        return Ok(None);
    };
    let party_ref = PartyReference::try_new(party_id.to_owned())
        .map_err(|e| worker::Error::RustError(e.code))?;
    let resolution = canonical_graph(db, tenant)
        .await?
        .resolve(&party_ref)
        .map_err(|e| worker::Error::RustError(e.code))?;
    if parties::stored(db, tenant, resolution.canonical_party_ref().as_str())
        .await?
        .is_none()
    {
        return Err(worker::Error::RustError(
            "IDENTITY_MERGE_CANONICAL_PARTY_MISSING".to_owned(),
        ));
    }
    Ok(Some(resolution))
}
