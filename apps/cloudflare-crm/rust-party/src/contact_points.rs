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
pub(super) enum KindInput { Email, Phone, Postal, Web, Messaging }
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
pub(super) enum StatusInput { Active, Inactive }
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
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct UpdateInput {
    pub(super) expected_version: i64,
    pub(super) value: String,
    pub(super) status: StatusInput,
    pub(super) preferred: bool,
    pub(super) valid_from_unix_nanos: Option<i64>,
    pub(super) valid_until_unix_nanos: Option<i64>,
}
#[derive(Deserialize)]
struct Row { id: String, state: String }
#[derive(Deserialize)]
struct Stored { state: String }
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
pub(super) async fn load(db: &D1Database, tenant: &str, id: &str) -> Result<Option<ContactPoint>> {
    bind(db, "SELECT state FROM crm_contact_point_records WHERE tenant = ? AND id = ?",
        &[JsValue::from_str(tenant), JsValue::from_str(id)])?
        .first::<Stored>(None).await?
        .map(|row| decode(&row.state)).transpose()
}
async fn party_exists(db: &D1Database, tenant: &str, id: &str) -> Result<bool> {
    Ok(bind(db, "SELECT id FROM crm_party_records WHERE tenant = ? AND id = ?",
        &[JsValue::from_str(tenant), JsValue::from_str(id)])?
        .first::<Value>(None).await?.is_some())
}
// Native D1 commands owned ONLY by the ORIGINAL ContactPoint aggregate.
// Connect and the blocked old HTTP wrappers call these same functions.
pub(super) struct MutationOutcome {
    pub(super) contact_point: ContactPoint,
    pub(super) replayed: bool,
}
pub(super) struct MutationFailure{
    pub(super) code: String,
    pub(super) status: u16,
}
fn failure(code:impl Into<String>,status:u16)->MutationFailure{
    MutationFailure{code:code.into(),status}
}
pub(super) async fn create_typed(input:CreateInput,idem:&str,db:&D1Database,
    tenant:&str,trace:&str,
)->Result<std::result::Result<MutationOutcome,MutationFailure>>{
    if !(8..=120).contains(&idem.len()){
        return Ok(Err(failure("IDEMPOTENCY_KEY_REQUIRED",422)));
    }
    let hash=digest("create",&input.id,&input)?;
    if let Some(receipt)=previous(db,tenant,idem).await?{
        if receipt.digest!=hash{return Ok(Err(failure("IDEMPOTENCY_CONFLICT",409)));}
        let contact_point=decode(&receipt.state)?;
        if contact_point.contact_point_id().as_str()!=input.id{
            return Ok(Err(failure("IDEMPOTENCY_CONFLICT",409)));
        }
        return Ok(Ok(MutationOutcome{contact_point,replayed:true}));
    }
    let id=match ContactPointId::try_new(&input.id){
        Ok(x)=>x,Err(e)=>return Ok(Err(failure(e.code,422))),
    };
    let party=match PartyReference::try_new(&input.party_id){
        Ok(x)=>x,Err(e)=>return Ok(Err(failure(e.code,422))),
    };
    if !party_exists(db,tenant,party.as_str()).await?{
        return Ok(Err(failure("CONTACT_POINTS_PARTY_NOT_FOUND",422)));
    }
    let point=match ContactPoint::create(CreateContactPoint{
        contact_point_id:id,party_ref:party,kind:input.kind.into(),
        value:input.value,preferred:input.preferred,
        valid_from_unix_nanos:input.valid_from_unix_nanos,
        valid_until_unix_nanos:input.valid_until_unix_nanos,
        occurred_at_unix_nanos:now_nanos(),
    }){
        Ok(x)=>x,Err(e)=>return Ok(Err(failure(e.code,422))),
    };
    let saved=encode(&point)?;
    let result=bind(db,
        "INSERT INTO crm_contact_point_records(tenant,id,party_id,state,version,idem_key,idem_digest,request_id) VALUES(?,?,?,?,?,?,?,?)",
        &[JsValue::from_str(tenant),JsValue::from_str(point.contact_point_id().as_str()),
          JsValue::from_str(point.party_ref().as_str()),JsValue::from_str(&saved),
          JsValue::from_f64(point.version() as f64),JsValue::from_str(idem),
          JsValue::from_str(&hash),JsValue::from_str(trace)])?.run().await;
    if result.is_err(){return Ok(Err(failure("CONTACT_POINTS_CREATE_CONFLICT",409)));}
    Ok(Ok(MutationOutcome{contact_point:point,replayed:false}))
}
pub(super) async fn update_typed(input:UpdateInput,idem:&str,db:&D1Database,
    tenant:&str,id:&str,trace:&str,
)->Result<std::result::Result<MutationOutcome,MutationFailure>>{
    if !(8..=120).contains(&idem.len()){
        return Ok(Err(failure("IDEMPOTENCY_KEY_REQUIRED",422)));
    }
    let hash=digest("update",id,&input)?;
    if let Some(receipt)=previous(db,tenant,idem).await?{
        if receipt.digest!=hash{return Ok(Err(failure("IDEMPOTENCY_CONFLICT",409)));}
        let contact_point=decode(&receipt.state)?;
        if contact_point.contact_point_id().as_str()!=id{
            return Ok(Err(failure("IDEMPOTENCY_CONFLICT",409)));
        }
        return Ok(Ok(MutationOutcome{contact_point,replayed:true}));
    }
    let Some(mut point)=load(db,tenant,id).await? else {
        return Ok(Err(failure("NOT_FOUND",404)));
    };
    if let Err(e)=point.apply_update(UpdateContactPoint{
        expected_version:input.expected_version,value:input.value,
        status:input.status.into(),preferred:input.preferred,
        valid_from_unix_nanos:input.valid_from_unix_nanos,
        valid_until_unix_nanos:input.valid_until_unix_nanos,
        occurred_at_unix_nanos:now_nanos(),
    }){
        return Ok(Err(failure(e.code.clone(),
            if e.code=="CONTACT_POINTS_VERSION_CONFLICT"{409}else{422})));
    }
    let saved=encode(&point)?;
    let result=bind(db,
        "UPDATE crm_contact_point_records SET state=?,version=?,idem_key=?,idem_digest=?,request_id=? WHERE tenant=? AND id=? AND version=?",
        &[JsValue::from_str(&saved),JsValue::from_f64(point.version() as f64),
          JsValue::from_str(idem),JsValue::from_str(&hash),JsValue::from_str(trace),
          JsValue::from_str(tenant),JsValue::from_str(id),
          JsValue::from_f64(input.expected_version as f64)])?.run().await;
    let Ok(changed)=result else{return Ok(Err(failure("CONTACT_POINTS_UPDATE_CONFLICT",409)))};
    if changed.meta()?.and_then(|m|m.changes).unwrap_or(0)==0 {
        return Ok(Err(failure("CONTACT_POINTS_VERSION_CONFLICT",409)));
    }
    Ok(Ok(MutationOutcome{contact_point:point,replayed:false}))
}
pub(super) async fn list_page_typed(db:&D1Database,tenant:&str,after:&str,
    size:usize,party:Option<&str>,
)->Result<(Vec<ContactPoint>,Option<String>)>{
    let rows=bind(db,
        "SELECT id,state FROM crm_contact_point_records WHERE tenant=? AND id>? ORDER BY id LIMIT ?",
        &[JsValue::from_str(tenant),JsValue::from_str(after),
          JsValue::from_f64((size+1) as f64)])?.all().await?.results::<Row>()?;
    let more=rows.len()>size;
    let next=if more{rows.get(size-1).map(|row|row.id.clone())}else{None};
    let mut contacts=Vec::new();
    for row in rows.iter().take(size){
        let point=decode(&row.state)?;
        if party.is_none_or(|id|id==point.party_ref().as_str()){contacts.push(point);}
    }
    Ok((contacts,next))
}
async fn create(mut req:Request,db:&D1Database,tenant:&str,trace:&str)->Result<Response>{
    let Some(idem)=key(&req)? else{return error("IDEMPOTENCY_KEY_REQUIRED",422)};
    let input=match serde_json::from_str::<CreateInput>(&req.text().await?){
        Ok(x)=>x,Err(_)=>return error("INVALID_INPUT",422),
    };
    match create_typed(input,&idem,db,tenant,trace).await?{
        Ok(out)=>response(json!({"item":view(&out.contact_point),"replayed":out.replayed}),
            if out.replayed{200}else{201}),
        Err(f)=>error(&f.code,f.status),
    }
}
async fn update(mut req:Request,db:&D1Database,tenant:&str,id:&str,trace:&str)->Result<Response>{
    let Some(idem)=key(&req)? else{return error("IDEMPOTENCY_KEY_REQUIRED",422)};
    let input=match serde_json::from_str::<UpdateInput>(&req.text().await?){
        Ok(x)=>x,Err(_)=>return error("INVALID_INPUT",422),
    };
    match update_typed(input,&idem,db,tenant,id,trace).await?{
        Ok(out)=>response(json!({"item":view(&out.contact_point),"replayed":out.replayed}),200),
        Err(f)=>error(&f.code,f.status),
    }
}
async fn list(db:&D1Database,tenant:&str)->Result<Response>{
    let (contacts,_)=list_page_typed(db,tenant,"",100,None).await?;
    response(json!({"items":contacts.iter().map(view).collect::<Vec<_>>()}),200)
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
