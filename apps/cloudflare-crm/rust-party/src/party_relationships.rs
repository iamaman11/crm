//! Native one-Worker D1 boundary for ORIGINAL crm.party-relationships.
//! Both Party references must exist within the authenticated tenant.
//! Business semantics, temporal validity and optimistic versions belong to
//! crm-party-relationships, never to this HTTP/SQL boundary.
use super::{bind, now_nanos};
use crm_party_relationships::{
    CreatePartyRelationship, PartyReference, PartyRelationship, PartyRelationshipId,
    PartyRelationshipStatus, RelationshipDirectionality, RelationshipType, UpdatePartyRelationship,
    decode_party_relationship_state, encode_party_relationship_state,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use wasm_bindgen::JsValue;
use worker::{D1Database, Result};

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum DirectionalityInput {
    Directional,
    Reciprocal,
}
impl From<DirectionalityInput> for RelationshipDirectionality {
    fn from(value: DirectionalityInput) -> Self {
        match value {
            DirectionalityInput::Directional => Self::Directional,
            DirectionalityInput::Reciprocal => Self::Reciprocal,
        }
    }
}
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum StatusInput {
    Active,
    Inactive,
}
impl From<StatusInput> for PartyRelationshipStatus {
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
    pub(super) from_party_id: String,
    pub(super) to_party_id: String,
    pub(super) code: String,
    pub(super) directionality: DirectionalityInput,
    pub(super) from_role: String,
    pub(super) to_role: String,
    pub(super) valid_from_unix_nanos: Option<i64>,
    pub(super) valid_until_unix_nanos: Option<i64>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateInput {
    expected_version: i64,
    status: StatusInput,
    valid_from_unix_nanos: Option<i64>,
    valid_until_unix_nanos: Option<i64>,
}
// The *request* is the idempotency identity. A missing Connect timestamp
// patch is Keep; never hash values loaded from mutable D1 before receipt lookup.
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum TimestampChange {
    Keep,
    Clear,
    Set(i64),
}
impl TimestampChange {
    fn resolve(self, previous: Option<i64>) -> Option<i64> {
        match self {
            Self::Keep => previous,
            Self::Clear => None,
            Self::Set(value) => Some(value),
        }
    }
}
#[derive(Serialize)]
pub(super) struct UpdateCommand {
    pub(super) expected_version: i64,
    pub(super) status: StatusInput,
    pub(super) valid_from: TimestampChange,
    pub(super) valid_until: TimestampChange,
}
pub(super) struct MutationOutcome {
    pub(super) relationship: PartyRelationship,
    pub(super) replayed: bool,
    pub(super) original_request_id: Option<String>,
}
pub(super) use super::d1_write::OwnerFailure as MutationFailure;
use connectrpc::ErrorCode;
#[derive(Deserialize)]
struct PagedRow {
    id: String,
    state: String,
}
#[derive(Deserialize)]
struct Row {
    state: String,
}
use super::d1_write::Receipt;

fn digest<T: Serialize>(op: &str, id: &str, input: &T) -> Result<String> {
    let bytes = serde_json::to_vec(&json!([op, id, input]))
        .map_err(|e| worker::Error::RustError(e.to_string()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
fn encoded(item: &PartyRelationship) -> Result<String> {
    String::from_utf8(
        encode_party_relationship_state(item).map_err(|e| worker::Error::RustError(e.code))?,
    )
    .map_err(|e| worker::Error::RustError(e.to_string()))
}
pub(super) fn decoded(state: &str) -> Result<PartyRelationship> {
    decode_party_relationship_state(state.as_bytes()).map_err(|e| worker::Error::RustError(e.code))
}
async fn prior(db: &D1Database, tenant: &str, k: &str) -> Result<Option<Receipt>> {
    bind(
        db,
        "SELECT digest,state,original_request_id FROM crm_party_relationship_receipts WHERE tenant = ? AND key = ?",
        &[JsValue::from_str(tenant), JsValue::from_str(k)],
    )?
    .first::<Receipt>(None)
    .await
}
pub(super) async fn load(
    db: &D1Database,
    tenant: &str,
    id: &str,
) -> Result<Option<PartyRelationship>> {
    bind(
        db,
        "SELECT state FROM crm_party_relationship_records WHERE tenant = ? AND id = ?",
        &[JsValue::from_str(tenant), JsValue::from_str(id)],
    )?
    .first::<Row>(None)
    .await?
    .map(|row| decoded(&row.state))
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
// ONE persistence implementation, shared by native Connect and the blocked
// historical HTTP surface; original crm-party-relationships owns all rules.
pub(super) async fn create_typed(
    input: CreateInput,
    k: &str,
    db: &D1Database,
    tenant: &str,
    actor: &str,
    trace: &str,
) -> Result<std::result::Result<MutationOutcome, MutationFailure>> {
    if !(8..=120).contains(&k.len()) {
        return Ok(Err(MutationFailure::domain(
            "IDEMPOTENCY_KEY_REQUIRED",
            ErrorCode::InvalidArgument,
        )));
    }
    let hash = digest("create", &input.id, &input)?;
    let replay = |p: Receipt| {
        if p.digest != hash {
            return Ok(Err(MutationFailure::domain(
                "IDEMPOTENCY_CONFLICT",
                ErrorCode::Aborted,
            )));
        }
        let relationship = decoded(&p.state)?;
        if relationship.party_relationship_id().as_str() != input.id {
            return Ok(Err(MutationFailure::domain(
                "IDEMPOTENCY_CONFLICT",
                ErrorCode::Aborted,
            )));
        }
        Ok(Ok(MutationOutcome {
            relationship,
            replayed: true,
            original_request_id: p.original_request_id,
        }))
    };
    if let Some(p) = prior(db, tenant, k).await? {
        return replay(p);
    }
    let id = match PartyRelationshipId::try_new(&input.id) {
        Ok(x) => x,
        Err(e) => {
            return Ok(Err(MutationFailure::domain(
                e.code,
                ErrorCode::InvalidArgument,
            )));
        }
    };
    let from = match PartyReference::try_new(&input.from_party_id) {
        Ok(x) => x,
        Err(e) => {
            return Ok(Err(MutationFailure::domain(
                e.code,
                ErrorCode::InvalidArgument,
            )));
        }
    };
    let to = match PartyReference::try_new(&input.to_party_id) {
        Ok(x) => x,
        Err(e) => {
            return Ok(Err(MutationFailure::domain(
                e.code,
                ErrorCode::InvalidArgument,
            )));
        }
    };
    if !party_exists(db, tenant, from.as_str()).await?
        || !party_exists(db, tenant, to.as_str()).await?
    {
        return Ok(Err(MutationFailure::domain(
            "PARTY_RELATIONSHIPS_PARTY_NOT_FOUND",
            ErrorCode::InvalidArgument,
        )));
    }
    let typ = match RelationshipType::try_new(
        input.code,
        input.directionality.into(),
        input.from_role,
        input.to_role,
    ) {
        Ok(x) => x,
        Err(e) => {
            return Ok(Err(MutationFailure::domain(
                e.code,
                ErrorCode::InvalidArgument,
            )));
        }
    };
    let relationship = match PartyRelationship::create(CreatePartyRelationship {
        party_relationship_id: id,
        from_party_ref: from,
        to_party_ref: to,
        relationship_type: typ,
        valid_from_unix_nanos: input.valid_from_unix_nanos,
        valid_until_unix_nanos: input.valid_until_unix_nanos,
        occurred_at_unix_nanos: now_nanos(),
    }) {
        Ok(x) => x,
        Err(e) => {
            return Ok(Err(MutationFailure::domain(
                e.code,
                ErrorCode::InvalidArgument,
            )));
        }
    };
    let stored = encoded(&relationship)?;
    let r=bind(db,"INSERT INTO crm_party_relationship_records(tenant,id,from_party,to_party,state,version,idem_key,idem_digest,request_id,actor) VALUES(?,?,?,?,?,?,?,?,?,?) ON CONFLICT(tenant,id) DO NOTHING",
        &[JsValue::from_str(tenant),
          JsValue::from_str(relationship.party_relationship_id().as_str()),
          JsValue::from_str(relationship.from_party_ref().as_str()),
          JsValue::from_str(relationship.to_party_ref().as_str()),
          JsValue::from_str(&stored),JsValue::from_f64(relationship.version() as f64),
          JsValue::from_str(k),JsValue::from_str(&hash),JsValue::from_str(trace),JsValue::from_str(actor)])?
        .run().await;
    match super::d1_write::confirm(
        r,
        prior(db, tenant, k),
        "PARTY_RELATIONSHIPS_CREATE_CONFLICT",
        "PARTY_RELATIONSHIPS_WRITE_UNCONFIRMED",
    )
    .await
    {
        Ok(super::d1_write::Confirmation::Applied) => {}
        Ok(super::d1_write::Confirmation::Replayed(saved)) => return replay(saved),
        Err(failure) => return Ok(Err(MutationFailure::Write(failure))),
    }
    Ok(Ok(MutationOutcome {
        relationship,
        replayed: false,
        original_request_id: Some(trace.to_owned()),
    }))
}
pub(super) async fn update_typed(
    input: UpdateCommand,
    k: &str,
    db: &D1Database,
    tenant: &str,
    id: &str,
    actor: &str,
    trace: &str,
) -> Result<std::result::Result<MutationOutcome, MutationFailure>> {
    if !(8..=120).contains(&k.len()) {
        return Ok(Err(MutationFailure::domain(
            "IDEMPOTENCY_KEY_REQUIRED",
            ErrorCode::InvalidArgument,
        )));
    }
    let hash = digest("update", id, &input)?;
    let replay = |p: Receipt| {
        if p.digest != hash {
            return Ok(Err(MutationFailure::domain(
                "IDEMPOTENCY_CONFLICT",
                ErrorCode::Aborted,
            )));
        }
        let relationship = decoded(&p.state)?;
        if relationship.party_relationship_id().as_str() != id {
            return Ok(Err(MutationFailure::domain(
                "IDEMPOTENCY_CONFLICT",
                ErrorCode::Aborted,
            )));
        }
        Ok(Ok(MutationOutcome {
            relationship,
            replayed: true,
            original_request_id: p.original_request_id,
        }))
    };
    let current = load(db, tenant, id).await;
    if let Some(p) = prior(db, tenant, k).await? {
        return replay(p);
    }
    let Some(mut relationship) = current? else {
        return Ok(Err(MutationFailure::domain(
            "NOT_FOUND",
            ErrorCode::NotFound,
        )));
    };
    let from = input
        .valid_from
        .resolve(relationship.valid_from_unix_nanos());
    let until = input
        .valid_until
        .resolve(relationship.valid_until_unix_nanos());
    if let Err(e) = relationship.apply_update(UpdatePartyRelationship {
        expected_version: input.expected_version,
        status: input.status.into(),
        valid_from_unix_nanos: from,
        valid_until_unix_nanos: until,
        occurred_at_unix_nanos: now_nanos(),
    }) {
        return Ok(Err(MutationFailure::domain(
            e.code.clone(),
            if e.code == "PARTY_RELATIONSHIPS_VERSION_CONFLICT" {
                ErrorCode::Aborted
            } else {
                ErrorCode::InvalidArgument
            },
        )));
    }
    let stored = encoded(&relationship)?;
    let r=bind(db,"UPDATE crm_party_relationship_records SET state = ?,version = ?,idem_key = ?,idem_digest = ?,request_id = ?,actor = ? WHERE tenant = ? AND id = ? AND version = ?",
        &[JsValue::from_str(&stored),JsValue::from_f64(relationship.version() as f64),
          JsValue::from_str(k),JsValue::from_str(&hash),JsValue::from_str(trace),
          JsValue::from_str(actor),JsValue::from_str(tenant),JsValue::from_str(id),
          JsValue::from_f64(input.expected_version as f64)])?.run().await;
    match super::d1_write::confirm(
        r,
        prior(db, tenant, k),
        "PARTY_RELATIONSHIPS_VERSION_CONFLICT",
        "PARTY_RELATIONSHIPS_WRITE_UNCONFIRMED",
    )
    .await
    {
        Ok(super::d1_write::Confirmation::Applied) => {}
        Ok(super::d1_write::Confirmation::Replayed(saved)) => return replay(saved),
        Err(failure) => return Ok(Err(MutationFailure::Write(failure))),
    }
    Ok(Ok(MutationOutcome {
        relationship,
        replayed: false,
        original_request_id: Some(trace.to_owned()),
    }))
}
pub(super) async fn list_page_typed(
    db: &D1Database,
    tenant: &str,
    after: &str,
    size: usize,
    party: Option<&str>,
    status: Option<StatusInput>,
) -> Result<(Vec<PartyRelationship>, Option<String>)> {
    // Tenant, either-party and status predicates run before LIMIT/page token.
    // The exact status value is the original owner's canonical persisted enum.
    let mut query =
        String::from("SELECT id,state FROM crm_party_relationship_records WHERE tenant=? AND id>?");
    let mut params = vec![JsValue::from_str(tenant), JsValue::from_str(after)];
    if let Some(id) = party {
        query.push_str(" AND (from_party=? OR to_party=?)");
        params.push(JsValue::from_str(id));
        params.push(JsValue::from_str(id));
    }
    if let Some(status) = status {
        query.push_str(" AND json_extract(state,'$.status')=?");
        params.push(JsValue::from_str(match status {
            StatusInput::Active => "active",
            StatusInput::Inactive => "inactive",
        }));
    }
    query.push_str(" ORDER BY id LIMIT ?");
    params.push(JsValue::from_f64((size + 1) as f64));
    let rows = bind(db, &query, &params)?
        .all()
        .await?
        .results::<PagedRow>()?;
    let more = rows.len() > size;
    let next = if more {
        rows.get(size - 1).map(|row| row.id.clone())
    } else {
        None
    };
    let mut items = Vec::with_capacity(rows.len().min(size));
    for row in rows.iter().take(size) {
        items.push(decoded(&row.state)?);
    }
    Ok((items, next))
}
