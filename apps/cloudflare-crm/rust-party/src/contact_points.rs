//! Native Worker HTTP/D1 boundary for ORIGINAL crm.contact-points.
//! Contact Point is a Party-owned endpoint, not a customer or a consent.
//! Domain invariants and strict persisted codecs remain in crm-contact-points.
use crm_contact_points::{
    ContactPoint, ContactPointId, ContactPointKind, ContactPointStatus,
    CreateContactPoint, PartyReference, UpdateContactPoint,
    decode_contact_point_state, encode_contact_point_state,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use wasm_bindgen::JsValue;
use worker::{D1Database, Method, Request, Response, Result};
use super::{bind, error, now_nanos, response};

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum KindInput { Email, Phone, Postal, Web, Messaging }
impl From<KindInput> for ContactPointKind {
    fn from(value: KindInput) -> Self {
        match value {
            KindInput::Email => Self::Email, KindInput::Phone => Self::Phone,
            KindInput::Postal => Self::Postal, KindInput::Web => Self::Web,
            KindInput::Messaging => Self::Messaging,
        }
    }
}
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StatusInput { Active, Inactive }
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
struct CreateInput {
    id: String,
    party_id: String,
    kind: KindInput,
    value: String,
    preferred: bool,
    valid_from_unix_nanos: Option<i64>,
    valid_until_unix_nanos: Option<i64>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateInput {
    expected_version: i64,
    value: String,
    status: StatusInput,
    preferred: bool,
    valid_from_unix_nanos: Option<i64>,
    valid_until_unix_nanos: Option<i64>,
}
#[derive(Deserialize)]
struct Row { state: String }
#[derive(Deserialize)]
struct Receipt { digest: String, state: String }

fn digest<T: Serialize>(operation: &str, id: &str, input: &T) -> Result<String> {
    let bytes = serde_json::to_vec(&json!([operation, id, input]))
        .map_err(|e| worker::Error::RustError(e.to_string()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
fn encode(point: &ContactPoint) -> Result<String> {
    String::from_utf8(encode_contact_point_state(point)
        .map_err(|e| worker::Error::RustError(e.code))?)
        .map_err(|e| worker::Error::RustError(e.to_string()))
}
pub(super) fn decode(value: &str) -> Result<ContactPoint> {
    decode_contact_point_state(value.as_bytes())
        .map_err(|e| worker::Error::RustError(e.code))
}
pub(super) fn view(point: &ContactPoint) -> Value {
    json!({
        "id": point.contact_point_id().as_str(),
        "party_id": point.party_ref().as_str(),
        "kind": match point.kind() {
            ContactPointKind::Email => "email",
            ContactPointKind::Phone => "phone",
            ContactPointKind::Postal => "postal",
            ContactPointKind::Web => "web",
            ContactPointKind::Messaging => "messaging",
        },
        "value": point.display_value(),
        "normalized_value": point.normalized_value(),
        "preferred": point.preferred(),
        "status": match point.status() {
            ContactPointStatus::Active => "active",
            ContactPointStatus::Inactive => "inactive",
        },
        "verified": point.verification().is_verified(),
        "valid_from_unix_nanos": point.valid_from_unix_nanos(),
        "valid_until_unix_nanos": point.valid_until_unix_nanos(),
        "version": point.version(),
    })
}
fn key(req: &Request) -> Result<Option<String>> {
    Ok(req.headers().get("idempotency-key")?
        .filter(|k| (8..=120).contains(&k.len())))
}
async fn previous(db: &D1Database, tenant: &str, idem: &str) -> Result<Option<Receipt>> {
    bind(db, "SELECT digest, state FROM crm_contact_point_receipts WHERE tenant = ? AND key = ?",
        &[JsValue::from_str(tenant), JsValue::from_str(idem)])?
        .first::<Receipt>(None).await
}
async fn load(db: &D1Database, tenant: &str, id: &str) -> Result<Option<ContactPoint>> {
    bind(db, "SELECT state FROM crm_contact_point_records WHERE tenant = ? AND id = ?",
        &[JsValue::from_str(tenant), JsValue::from_str(id)])?
        .first::<Row>(None).await?
        .map(|row| decode(&row.state)).transpose()
}
async fn party_exists(db: &D1Database, tenant: &str, id: &str) -> Result<bool> {
    Ok(bind(db, "SELECT id FROM crm_party_records WHERE tenant = ? AND id = ?",
        &[JsValue::from_str(tenant), JsValue::from_str(id)])?
        .first::<Value>(None).await?.is_some())
}
async fn create(mut req: Request, db: &D1Database, tenant: &str, trace: &str) -> Result<Response> {
    let Some(idem) = key(&req)? else { return error("IDEMPOTENCY_KEY_REQUIRED",422); };
    let input = match serde_json::from_str::<CreateInput>(&req.text().await?) {
        Ok(x) => x, Err(_) => return error("INVALID_INPUT",422),
    };
    let hash = digest("create", &input.id, &input)?;
    if let Some(receipt) = previous(db, tenant, &idem).await? {
        if receipt.digest != hash { return error("IDEMPOTENCY_CONFLICT",409); }
        return response(json!({"item":view(&decode(&receipt.state)?),"replayed":true}),200);
    }
    let id = match ContactPointId::try_new(&input.id) {
        Ok(x) => x, Err(e) => return error(&e.code,422),
    };
    let party = match PartyReference::try_new(&input.party_id) {
        Ok(x) => x, Err(e) => return error(&e.code,422),
    };
    if !party_exists(db, tenant, party.as_str()).await? {
        return error("CONTACT_POINTS_PARTY_NOT_FOUND",422);
    }
    let point = match ContactPoint::create(CreateContactPoint {
        contact_point_id: id, party_ref: party,
        kind: input.kind.into(), value: input.value, preferred: input.preferred,
        valid_from_unix_nanos: input.valid_from_unix_nanos,
        valid_until_unix_nanos: input.valid_until_unix_nanos,
        occurred_at_unix_nanos: now_nanos(),
    }) { Ok(x) => x, Err(e) => return error(&e.code,422) };
    let saved = encode(&point)?;
    let r = bind(db, "INSERT INTO crm_contact_point_records(tenant,id,party_id,state,version,idem_key,idem_digest,request_id) VALUES(?,?,?,?,?,?,?,?)",
        &[JsValue::from_str(tenant),JsValue::from_str(point.contact_point_id().as_str()),
          JsValue::from_str(point.party_ref().as_str()),JsValue::from_str(&saved),
          JsValue::from_f64(point.version() as f64),
          JsValue::from_str(&idem),JsValue::from_str(&hash),
          JsValue::from_str(trace)])?.run().await;
    if r.is_err() { return error("CONTACT_POINTS_CREATE_CONFLICT",409); }
    response(json!({"item":view(&point),"replayed":false}),201)
}
async fn update(mut req: Request, db: &D1Database, tenant: &str, id: &str, trace: &str) -> Result<Response> {
    let Some(idem) = key(&req)? else { return error("IDEMPOTENCY_KEY_REQUIRED",422); };
    let input = match serde_json::from_str::<UpdateInput>(&req.text().await?) {
        Ok(x) => x, Err(_) => return error("INVALID_INPUT",422),
    };
    let hash = digest("update", id, &input)?;
    if let Some(receipt) = previous(db, tenant, &idem).await? {
        if receipt.digest != hash { return error("IDEMPOTENCY_CONFLICT",409); }
        return response(json!({"item":view(&decode(&receipt.state)?),"replayed":true}),200);
    }
    let Some(mut point) = load(db, tenant, id).await? else {
        return error("NOT_FOUND",404);
    };
    if let Err(e) = point.apply_update(UpdateContactPoint {
        expected_version: input.expected_version, value: input.value,
        status: input.status.into(), preferred: input.preferred,
        valid_from_unix_nanos: input.valid_from_unix_nanos,
        valid_until_unix_nanos: input.valid_until_unix_nanos,
        occurred_at_unix_nanos: now_nanos(),
    }) {
        return error(&e.code,if e.code=="CONTACT_POINTS_VERSION_CONFLICT"{409}else{422});
    }
    let saved = encode(&point)?;
    let r=bind(db,"UPDATE crm_contact_point_records SET state = ?,version = ?,idem_key = ?,idem_digest = ?,request_id = ? WHERE tenant = ? AND id = ? AND version = ?",
        &[JsValue::from_str(&saved),JsValue::from_f64(point.version() as f64),
          JsValue::from_str(&idem),JsValue::from_str(&hash),JsValue::from_str(trace),
          JsValue::from_str(tenant),JsValue::from_str(id),
          JsValue::from_f64(input.expected_version as f64)])?.run().await;
    let Ok(result)=r else{return error("CONTACT_POINTS_UPDATE_CONFLICT",409)};
    if result.meta()?.and_then(|m|m.changes).unwrap_or(0)==0 {
        return error("CONTACT_POINTS_VERSION_CONFLICT",409);
    }
    response(json!({"item":view(&point),"replayed":false}),200)
}
async fn list(db: &D1Database, tenant: &str) -> Result<Response> {
    let rows=bind(db,"SELECT state FROM crm_contact_point_records WHERE tenant = ? ORDER BY id LIMIT 300",
        &[JsValue::from_str(tenant)])?.all().await?.results::<Row>()?;
    let items=rows.iter().map(|row|decode(&row.state).map(|point|view(&point)))
        .collect::<Result<Vec<_>>>()?;
    response(json!({"items":items}),200)
}
pub(super) async fn route(req: Request, db: D1Database, tenant: &str, trace: &str) -> Result<Response> {
    let path=req.path();
    let method=req.method();
    if path=="/api/contact-points" {
        return match method {
            Method::Get => list(&db,tenant).await,
            Method::Post => create(req,&db,tenant,trace).await,
            _ => error("NOT_FOUND",404),
        };
    }
    let Some(id)=path.strip_prefix("/api/contact-points/") else {
        return error("NOT_FOUND",404);
    };
    if id.is_empty() || id.contains('/') { return error("NOT_FOUND",404); }
    match method {
        Method::Get => match load(&db,tenant,id).await? {
            Some(point) => response(json!({"item":view(&point)}),200),
            None => error("NOT_FOUND",404),
        },
        Method::Patch => update(req,&db,tenant,id,trace).await,
        // Verification is intentionally not exposed until a trusted evidence
        // producer is wired; client-submitted arbitrary evidence is NOT proof.
        _ => error("NOT_FOUND",404),
    }
}
