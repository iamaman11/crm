//! Native Worker HTTP/D1 boundary for ORIGINAL crm.contact-points.
//! Contact Point is a Party-owned endpoint, not a customer or a consent.
//! Domain invariants and strict persisted codecs remain in crm-contact-points.
use super::d1_write::{self, Confirmation, Receipt, WriteResult, rejected};
use super::diagnostics::{MutationFailure, ReceiptUncertainty};
use super::{access, bind, now_nanos};
use crm_contact_points::{
    ContactPoint, ContactPointId, ContactPointKind, ContactPointStatus, CreateContactPoint,
    PartyReference, UpdateContactPoint, VerifyContactPoint, decode_contact_point_state,
    encode_contact_point_state,
};
use crm_module_sdk::ErrorCategory;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use wasm_bindgen::JsValue;
use worker::{D1Database, Result};

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum KindInput {
    Email,
    Phone,
    Postal,
    Web,
    Messaging,
}
impl From<KindInput> for ContactPointKind {
    fn from(value: KindInput) -> Self {
        match value {
            KindInput::Email => Self::Email,
            KindInput::Phone => Self::Phone,
            KindInput::Postal => Self::Postal,
            KindInput::Web => Self::Web,
            KindInput::Messaging => Self::Messaging,
        }
    }
}
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum StatusInput {
    Active,
    Inactive,
}
impl From<StatusInput> for ContactPointStatus {
    fn from(value: StatusInput) -> Self {
        match value {
            StatusInput::Active => Self::Active,
            StatusInput::Inactive => Self::Inactive,
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CreateInput {
    pub(super) id: String,
    pub(super) party_id: String,
    pub(super) kind: KindInput,
    pub(super) value: String,
    pub(super) preferred: bool,
    pub(super) valid_from_unix_nanos: Option<i64>,
    pub(super) valid_until_unix_nanos: Option<i64>,
}
// Hash patch INTENT in the existing receipt, never mutable D1 values.
// Keep, Clear and Set serialize distinctly (nested Options do not).
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum TimestampChange {
    Keep,
    Clear,
    Set(i64),
}
impl TimestampChange {
    fn resolve(self, current: Option<i64>) -> Option<i64> {
        match self {
            Self::Keep => current,
            Self::Clear => None,
            Self::Set(value) => Some(value),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct UpdateInput {
    pub(super) expected_version: i64,
    pub(super) value: String,
    pub(super) status: StatusInput,
    pub(super) preferred: bool,
    pub(super) valid_from_unix_nanos: TimestampChange,
    pub(super) valid_until_unix_nanos: TimestampChange,
}
#[derive(Deserialize)]
struct Row {
    id: String,
    state: String,
}
#[derive(Deserialize)]
struct Stored {
    state: String,
}

// Only Cloudflare Access signed human identities, not service tokens, can
// assert ownership of the matching email address.
pub(super) fn signed_human_email(identity: &str) -> Option<&str> {
    if identity.starts_with("service:")
        || identity.len() > 254
        || identity.trim() != identity
        || !identity.contains('@')
        || identity.chars().any(char::is_whitespace)
    {
        None
    } else {
        Some(identity)
    }
}

fn digest<T: Serialize>(operation: &str, id: &str, input: &T) -> Result<String> {
    let bytes = serde_json::to_vec(&json!([operation, id, input]))
        .map_err(|e| worker::Error::RustError(e.to_string()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
fn encode(point: &ContactPoint) -> Result<String> {
    String::from_utf8(
        encode_contact_point_state(point).map_err(|e| worker::Error::RustError(e.code))?,
    )
    .map_err(|e| worker::Error::RustError(e.to_string()))
}
pub(super) fn decode(value: &str) -> Result<ContactPoint> {
    decode_contact_point_state(value.as_bytes()).map_err(|e| worker::Error::RustError(e.code))
}
async fn previous(db: &D1Database, tenant: &str, idem: &str) -> Result<Option<Receipt>> {
    bind(
        db,
        "SELECT digest, state, original_request_id FROM crm_contact_point_receipts WHERE tenant = ? AND key = ?",
        &[JsValue::from_str(tenant), JsValue::from_str(idem)],
    )?
    .first::<Receipt>(None)
    .await
}
pub(super) async fn load(db: &D1Database, tenant: &str, id: &str) -> Result<Option<ContactPoint>> {
    bind(
        db,
        "SELECT state FROM crm_contact_point_records WHERE tenant = ? AND id = ?",
        &[JsValue::from_str(tenant), JsValue::from_str(id)],
    )?
    .first::<Stored>(None)
    .await?
    .map(|row| decode(&row.state))
    .transpose()
}
async fn party_exists(db: &D1Database, tenant: &str, id: &str) -> Result<bool> {
    Ok(bind(
        db,
        "SELECT id FROM crm_party_records WHERE tenant = ? AND id = ?",
        &[JsValue::from_str(tenant), JsValue::from_str(id)],
    )?
    .first::<Value>(None)
    .await?
    .is_some())
}
// Native D1 commands owned ONLY by the ORIGINAL ContactPoint aggregate.
// Connect and the blocked old HTTP wrappers call these same functions.
pub(super) struct MutationOutcome {
    pub(super) contact_point: ContactPoint,
    pub(super) replayed: bool,
    pub(super) original_request_id: Option<String>,
}
const UNCONFIRMED: &str = "CONTACT_POINTS_WRITE_UNCONFIRMED";

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
    let contact_point = decode(&receipt.state).map_err(|_| MutationFailure::CommitUnconfirmed {
        code: UNCONFIRMED,
        reason: ReceiptUncertainty::DecodeFailed,
    })?;
    if contact_point.contact_point_id().as_str() != id {
        return Err(rejected("IDEMPOTENCY_CONFLICT", ErrorCategory::Conflict));
    }
    Ok(MutationOutcome {
        contact_point,
        replayed: true,
        original_request_id: receipt.original_request_id,
    })
}
pub(super) async fn create_typed(
    input: CreateInput,
    idem: &str,
    db: &D1Database,
    actor: &access::Actor,
    trace: &str,
) -> WriteResult<MutationOutcome> {
    let tenant = &actor.tenant;
    if !(8..=120).contains(&idem.len()) {
        return Err(rejected(
            "IDEMPOTENCY_KEY_REQUIRED",
            ErrorCategory::InvalidArgument,
        ));
    }
    let hash = digest("create", &input.id, &input).map_err(storage_failure)?;
    if let Some(receipt) = previous(db, tenant, idem).await.map_err(storage_failure)? {
        return replay(receipt, &hash, &input.id);
    }
    let id = match ContactPointId::try_new(&input.id) {
        Ok(x) => x,
        Err(e) => return Err(rejected(e.code, ErrorCategory::InvalidArgument)),
    };
    let party = match PartyReference::try_new(&input.party_id) {
        Ok(x) => x,
        Err(e) => return Err(rejected(e.code, ErrorCategory::InvalidArgument)),
    };
    if !party_exists(db, tenant, party.as_str())
        .await
        .map_err(storage_failure)?
    {
        return Err(rejected(
            "CONTACT_POINTS_PARTY_NOT_FOUND",
            ErrorCategory::InvalidArgument,
        ));
    }
    let point = match ContactPoint::create(CreateContactPoint {
        contact_point_id: id,
        party_ref: party,
        kind: input.kind.into(),
        value: input.value,
        preferred: input.preferred,
        valid_from_unix_nanos: input.valid_from_unix_nanos,
        valid_until_unix_nanos: input.valid_until_unix_nanos,
        occurred_at_unix_nanos: now_nanos(),
    }) {
        Ok(x) => x,
        Err(e) => return Err(rejected(e.code, ErrorCategory::InvalidArgument)),
    };
    let saved = encode(&point).map_err(storage_failure)?;
    let result=bind(db,
        "INSERT INTO crm_contact_point_records(tenant,id,party_id,state,version,idem_key,idem_digest,request_id,actor) VALUES(?,?,?,?,?,?,?,?,?) ON CONFLICT(tenant,id) DO NOTHING",
        &[JsValue::from_str(tenant),JsValue::from_str(point.contact_point_id().as_str()),
          JsValue::from_str(point.party_ref().as_str()),JsValue::from_str(&saved),
          JsValue::from_f64(point.version() as f64),JsValue::from_str(idem),
          JsValue::from_str(&hash),JsValue::from_str(trace),
           JsValue::from_str(&actor.identity)]).map_err(storage_failure)?.run().await;
    match d1_write::confirm(
        result,
        previous(db, tenant, idem),
        "CONTACT_POINTS_CREATE_CONFLICT",
        UNCONFIRMED,
    )
    .await?
    {
        Confirmation::Applied => Ok(MutationOutcome {
            contact_point: point,
            replayed: false,
            original_request_id: Some(trace.to_owned()),
        }),
        Confirmation::Replayed(receipt) => replay(receipt, &hash, &input.id),
    }
}
pub(super) async fn update_typed(
    input: UpdateInput,
    idem: &str,
    db: &D1Database,
    actor: &access::Actor,
    id: &str,
    trace: &str,
) -> WriteResult<MutationOutcome> {
    let tenant = &actor.tenant;
    if !(8..=120).contains(&idem.len()) {
        return Err(rejected(
            "IDEMPOTENCY_KEY_REQUIRED",
            ErrorCategory::InvalidArgument,
        ));
    }
    let hash = digest("update", id, &input).map_err(storage_failure)?;
    // Read the receipt after the snapshot so a concurrent replay wins over stale state.
    let current = load(db, tenant, id).await;
    if let Some(receipt) = previous(db, tenant, idem).await.map_err(storage_failure)? {
        return replay(receipt, &hash, id);
    }
    let Some(mut point) = current.map_err(storage_failure)? else {
        return Err(rejected("NOT_FOUND", ErrorCategory::NotFound));
    };
    let from = input
        .valid_from_unix_nanos
        .resolve(point.valid_from_unix_nanos());
    let until = input
        .valid_until_unix_nanos
        .resolve(point.valid_until_unix_nanos());
    if let Err(e) = point.apply_update(UpdateContactPoint {
        expected_version: input.expected_version,
        value: input.value,
        status: input.status.into(),
        preferred: input.preferred,
        valid_from_unix_nanos: from,
        valid_until_unix_nanos: until,
        occurred_at_unix_nanos: now_nanos(),
    }) {
        return Err(rejected(
            e.code.clone(),
            if e.code == "CONTACT_POINTS_VERSION_CONFLICT" {
                ErrorCategory::Conflict
            } else {
                ErrorCategory::InvalidArgument
            },
        ));
    }
    let saved = encode(&point).map_err(storage_failure)?;
    let result=bind(db,
        "UPDATE crm_contact_point_records SET state=?,version=?,idem_key=?,idem_digest=?,request_id=?,actor=?,last_action='contact_points.update' WHERE tenant=? AND id=? AND version=?",
        &[JsValue::from_str(&saved),JsValue::from_f64(point.version() as f64),
          JsValue::from_str(idem),JsValue::from_str(&hash),JsValue::from_str(trace),
          JsValue::from_str(&actor.identity),JsValue::from_str(tenant),JsValue::from_str(id),
          JsValue::from_f64(input.expected_version as f64)]).map_err(storage_failure)?.run().await;
    match d1_write::confirm(
        result,
        previous(db, tenant, idem),
        "CONTACT_POINTS_VERSION_CONFLICT",
        UNCONFIRMED,
    )
    .await?
    {
        Confirmation::Applied => Ok(MutationOutcome {
            contact_point: point,
            replayed: false,
            original_request_id: Some(trace.to_owned()),
        }),
        Confirmation::Replayed(receipt) => replay(receipt, &hash, id),
    }
}
// Human verification proves matching Access email. The enrolled dev service
// admin may also mark a test contact, with DISTINCT test-agent evidence.
// Both paths use the original ContactPoint owner/CAS and D1 audit trigger.
fn verified_replay(
    receipt: Receipt,
    digest: &str,
    id: &str,
    email: Option<&str>,
) -> WriteResult<MutationOutcome> {
    let saved = replay(receipt, digest, id)?;
    if saved.contact_point.kind() != ContactPointKind::Email
        || email.is_some_and(|email| saved.contact_point.normalized_value() != email)
    {
        return Err(rejected(
            "CONTACT_POINTS_EMAIL_OWNERSHIP_REQUIRED",
            ErrorCategory::Authorization,
        ));
    }
    Ok(saved)
}
pub(super) async fn verify_own_email_typed(
    idem: &str,
    expected_version: i64,
    db: &D1Database,
    actor: &access::Actor,
    id: &str,
    trace: &str,
) -> WriteResult<MutationOutcome> {
    let human_email = signed_human_email(&actor.identity);
    let test_agent = actor.is_dev_test_agent();
    if human_email.is_none() && !test_agent {
        return Err(rejected(
            "CONTACT_POINTS_SIGNED_HUMAN_EMAIL_REQUIRED",
            ErrorCategory::Authorization,
        ));
    }
    if !(8..=120).contains(&idem.len()) {
        return Err(rejected(
            "IDEMPOTENCY_KEY_REQUIRED",
            ErrorCategory::InvalidArgument,
        ));
    }
    let hash = digest(
        "verify-own-email",
        id,
        &json!([expected_version, &actor.identity]),
    )
    .map_err(storage_failure)?;
    // Read the receipt after the snapshot so a concurrent replay wins over stale state.
    let current = load(db, &actor.tenant, id).await;
    if let Some(receipt) = previous(db, &actor.tenant, idem)
        .await
        .map_err(storage_failure)?
    {
        return verified_replay(receipt, &hash, id, human_email);
    }
    let Some(mut point) = current.map_err(storage_failure)? else {
        return Err(rejected("NOT_FOUND", ErrorCategory::NotFound));
    };
    if point.kind() != ContactPointKind::Email
        || human_email.is_some_and(|email| point.normalized_value() != email)
    {
        return Err(rejected(
            "CONTACT_POINTS_EMAIL_OWNERSHIP_REQUIRED",
            ErrorCategory::Authorization,
        ));
    }
    // Dev agent verification is never represented as human email ownership.
    let evidence_source = if test_agent {
        "crm-dev-agent-email"
    } else {
        "access-email"
    };
    let proof = format!(
        "{:x}",
        Sha256::digest(
            format!(
                "{}:{}:{}:{}",
                point.normalized_value(),
                id,
                trace,
                actor.identity
            )
            .as_bytes()
        )
    );
    if let Err(e) = point.verify(VerifyContactPoint {
        expected_version,
        evidence_ref: format!("{evidence_source}:{proof}"),
        occurred_at_unix_nanos: now_nanos(),
    }) {
        return Err(rejected(
            e.code.clone(),
            if e.code == "CONTACT_POINTS_VERSION_CONFLICT"
                || e.code == "CONTACT_POINTS_ALREADY_VERIFIED"
            {
                ErrorCategory::Conflict
            } else {
                ErrorCategory::InvalidArgument
            },
        ));
    }
    let saved = encode(&point).map_err(storage_failure)?;
    let result = bind(db,
        "UPDATE crm_contact_point_records SET state=?,version=?,idem_key=?,idem_digest=?,request_id=?,actor=?,last_action='contact_points.verify' WHERE tenant=? AND id=? AND version=?",
        &[JsValue::from_str(&saved),JsValue::from_f64(point.version() as f64),
          JsValue::from_str(idem),JsValue::from_str(&hash),JsValue::from_str(trace),
          JsValue::from_str(&actor.identity),JsValue::from_str(&actor.tenant),
          JsValue::from_str(id),JsValue::from_f64(expected_version as f64)]).map_err(storage_failure)?.run().await;
    match d1_write::confirm(
        result,
        previous(db, &actor.tenant, idem),
        "CONTACT_POINTS_VERSION_CONFLICT",
        UNCONFIRMED,
    )
    .await?
    {
        Confirmation::Applied => Ok(MutationOutcome {
            contact_point: point,
            replayed: false,
            original_request_id: Some(trace.to_owned()),
        }),
        Confirmation::Replayed(receipt) => verified_replay(receipt, &hash, id, human_email),
    }
}

// One typed list input for the original ContactPoint owner. Keep all
// predicates in D1 BEFORE LIMIT to prevent cursor omissions.
#[derive(Clone, Copy)]
pub(super) struct ContactListFilters<'a> {
    pub(super) party: Option<&'a str>,
    pub(super) kind: Option<KindInput>,
    pub(super) status: Option<StatusInput>,
}
pub(super) async fn list_page_typed(
    db: &D1Database,
    tenant: &str,
    after: &str,
    size: usize,
    filters: ContactListFilters<'_>,
) -> Result<(Vec<ContactPoint>, Option<String>)> {
    let mut query =
        String::from("SELECT id,state FROM crm_contact_point_records WHERE tenant=? AND id>?");
    let mut params = vec![JsValue::from_str(tenant), JsValue::from_str(after)];
    if let Some(id) = filters.party {
        // Already a native indexed column, not parsed from JSON.
        query.push_str(" AND party_id=?");
        params.push(JsValue::from_str(id));
    }
    if let Some(kind) = filters.kind {
        let value = match kind {
            KindInput::Email => "email",
            KindInput::Phone => "phone",
            KindInput::Postal => "postal",
            KindInput::Web => "web",
            KindInput::Messaging => "messaging",
        };
        query.push_str(" AND json_extract(state,'$.kind')=?");
        params.push(JsValue::from_str(value));
    }
    if let Some(status) = filters.status {
        let value = match status {
            StatusInput::Active => "active",
            StatusInput::Inactive => "inactive",
        };
        query.push_str(" AND json_extract(state,'$.status')=?");
        params.push(JsValue::from_str(value));
    }
    query.push_str(" ORDER BY id LIMIT ?");
    params.push(JsValue::from_f64((size + 1) as f64));
    let rows = bind(db, &query, &params)?.all().await?.results::<Row>()?;
    let more = rows.len() > size;
    let next = if more {
        rows.get(size - 1).map(|row| row.id.clone())
    } else {
        None
    };
    let mut contacts = Vec::with_capacity(rows.len().min(size));
    for row in rows.iter().take(size) {
        contacts.push(decode(&row.state)?);
    }
    Ok((contacts, next))
}
