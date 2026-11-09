//! Native one-Worker D1 boundary for ORIGINAL crm.party-relationships.
//! Both Party references must exist within the authenticated tenant.
//! Business semantics, temporal validity and optimistic versions belong to
//! crm-party-relationships, never to this HTTP/SQL boundary.
use crm_party_relationships::{
    PartyRelationship, PartyRelationshipId, PartyRelationshipStatus,
    PartyReference, RelationshipType, RelationshipDirectionality,
    CreatePartyRelationship, UpdatePartyRelationship,
    decode_party_relationship_state, encode_party_relationship_state,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use wasm_bindgen::JsValue;
use worker::{D1Database, Method, Request, Response, Result};
use super::{bind, error, now_nanos, response};

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all="snake_case")]
enum DirectionalityInput { Directional, Reciprocal }
impl From<DirectionalityInput> for RelationshipDirectionality {
    fn from(value: DirectionalityInput)->Self {
        match value {
            DirectionalityInput::Directional=>Self::Directional,
            DirectionalityInput::Reciprocal=>Self::Reciprocal,
        }
    }
}
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all="snake_case")]
enum StatusInput { Active, Inactive }
impl From<StatusInput> for PartyRelationshipStatus {
    fn from(value: StatusInput)->Self {
        match value { StatusInput::Active=>Self::Active, StatusInput::Inactive=>Self::Inactive }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateInput {
    id:String,
    from_party_id:String,
    to_party_id:String,
    code:String,
    directionality:DirectionalityInput,
    from_role:String,
    to_role:String,
    valid_from_unix_nanos:Option<i64>,
    valid_until_unix_nanos:Option<i64>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateInput {
    expected_version:i64,
    status:StatusInput,
    valid_from_unix_nanos:Option<i64>,
    valid_until_unix_nanos:Option<i64>,
}
#[derive(Deserialize)]
struct Row { state:String }
#[derive(Deserialize)]
struct Receipt { digest:String, state:String }

fn digest<T:Serialize>(op:&str,id:&str,input:&T)->Result<String>{
    let bytes=serde_json::to_vec(&json!([op,id,input]))
        .map_err(|e|worker::Error::RustError(e.to_string()))?;
    Ok(format!("{:x}",Sha256::digest(bytes)))
}
fn encoded(item:&PartyRelationship)->Result<String>{
    String::from_utf8(encode_party_relationship_state(item)
        .map_err(|e|worker::Error::RustError(e.code))?)
        .map_err(|e|worker::Error::RustError(e.to_string()))
}
pub(super) fn decoded(state:&str)->Result<PartyRelationship>{
    decode_party_relationship_state(state.as_bytes())
        .map_err(|e|worker::Error::RustError(e.code))
}
pub(super) fn view(item:&PartyRelationship)->Value {
    let typ=item.relationship_type();
    json!({
        "id":item.party_relationship_id().as_str(),
        "from_party_id":item.from_party_ref().as_str(),
        "to_party_id":item.to_party_ref().as_str(),
        "type":{
            "code":typ.code(),"from_role":typ.from_role(),"to_role":typ.to_role(),
            "directionality":match typ.directionality() {
                RelationshipDirectionality::Directional=>"directional",
                RelationshipDirectionality::Reciprocal=>"reciprocal",
            },
        },
        "status":match item.status() {
            PartyRelationshipStatus::Active=>"active",
            PartyRelationshipStatus::Inactive=>"inactive",
        },
        "valid_from_unix_nanos":item.valid_from_unix_nanos(),
        "valid_until_unix_nanos":item.valid_until_unix_nanos(),
        "version":item.version(),
    })
}
fn key(req:&Request)->Result<Option<String>> {
    Ok(req.headers().get("idempotency-key")?
        .filter(|key|(8..=120).contains(&key.len())))
}
async fn prior(db:&D1Database,tenant:&str,k:&str)->Result<Option<Receipt>> {
    bind(db,"SELECT digest,state FROM crm_party_relationship_receipts WHERE tenant = ? AND key = ?",
        &[JsValue::from_str(tenant),JsValue::from_str(k)])?
        .first::<Receipt>(None).await
}
async fn load(db:&D1Database,tenant:&str,id:&str)->Result<Option<PartyRelationship>>{
    bind(db,"SELECT state FROM crm_party_relationship_records WHERE tenant = ? AND id = ?",
        &[JsValue::from_str(tenant),JsValue::from_str(id)])?
        .first::<Row>(None).await?
        .map(|row|decoded(&row.state)).transpose()
}
async fn party_exists(db:&D1Database,tenant:&str,id:&str)->Result<bool>{
    Ok(bind(db,"SELECT id FROM crm_party_records WHERE tenant = ? AND id = ?",
        &[JsValue::from_str(tenant),JsValue::from_str(id)])?
        .first::<Value>(None).await?.is_some())
}
async fn create(mut req:Request,db:&D1Database,tenant:&str,trace:&str)->Result<Response>{
    let Some(k)=key(&req)? else{return error("IDEMPOTENCY_KEY_REQUIRED",422)};
    let input=match serde_json::from_str::<CreateInput>(&req.text().await?){
        Ok(x)=>x,Err(_)=>return error("INVALID_INPUT",422),
    };
    let hash=digest("create",&input.id,&input)?;
    if let Some(p)=prior(db,tenant,&k).await?{
        if p.digest!=hash{return error("IDEMPOTENCY_CONFLICT",409);}
        return response(json!({"item":view(&decoded(&p.state)?),"replayed":true}),200);
    }
    let id=match PartyRelationshipId::try_new(&input.id){
        Ok(x)=>x,Err(e)=>return error(&e.code,422),
    };
    let from=match PartyReference::try_new(&input.from_party_id){
        Ok(x)=>x,Err(e)=>return error(&e.code,422),
    };
    let to=match PartyReference::try_new(&input.to_party_id){
        Ok(x)=>x,Err(e)=>return error(&e.code,422),
    };
    if !party_exists(db,tenant,from.as_str()).await? ||
        !party_exists(db,tenant,to.as_str()).await? {
        return error("PARTY_RELATIONSHIPS_PARTY_NOT_FOUND",422);
    }
    let typ=match RelationshipType::try_new(input.code,
        input.directionality.into(),input.from_role,input.to_role){
        Ok(x)=>x,Err(e)=>return error(&e.code,422),
    };
    let relation=match PartyRelationship::create(CreatePartyRelationship{
        party_relationship_id:id,from_party_ref:from,to_party_ref:to,
        relationship_type:typ,valid_from_unix_nanos:input.valid_from_unix_nanos,
        valid_until_unix_nanos:input.valid_until_unix_nanos,
        occurred_at_unix_nanos:now_nanos(),
    }) {Ok(x)=>x,Err(e)=>return error(&e.code,422)};
    let stored=encoded(&relation)?;
    let r=bind(db,"INSERT INTO crm_party_relationship_records(tenant,id,from_party,to_party,state,version,idem_key,idem_digest,request_id) VALUES(?,?,?,?,?,?,?,?,?)",
        &[JsValue::from_str(tenant),
          JsValue::from_str(relation.party_relationship_id().as_str()),
          JsValue::from_str(relation.from_party_ref().as_str()),
          JsValue::from_str(relation.to_party_ref().as_str()),
          JsValue::from_str(&stored),JsValue::from_f64(relation.version() as f64),
          JsValue::from_str(&k),JsValue::from_str(&hash),JsValue::from_str(trace)])?
        .run().await;
    if r.is_err(){return error("PARTY_RELATIONSHIPS_CREATE_CONFLICT",409);}
    response(json!({"item":view(&relation),"replayed":false}),201)
}
async fn update(mut req:Request,db:&D1Database,tenant:&str,id:&str,trace:&str)->Result<Response>{
    let Some(k)=key(&req)? else{return error("IDEMPOTENCY_KEY_REQUIRED",422)};
    let input=match serde_json::from_str::<UpdateInput>(&req.text().await?){
        Ok(x)=>x,Err(_)=>return error("INVALID_INPUT",422),
    };
    let hash=digest("update",id,&input)?;
    if let Some(p)=prior(db,tenant,&k).await?{
        if p.digest!=hash{return error("IDEMPOTENCY_CONFLICT",409);}
        return response(json!({"item":view(&decoded(&p.state)?),"replayed":true}),200);
    }
    let Some(mut relation)=load(db,tenant,id).await? else{return error("NOT_FOUND",404)};
    if let Err(e)=relation.apply_update(UpdatePartyRelationship{
        expected_version:input.expected_version,status:input.status.into(),
        valid_from_unix_nanos:input.valid_from_unix_nanos,
        valid_until_unix_nanos:input.valid_until_unix_nanos,
        occurred_at_unix_nanos:now_nanos(),
    }) {return error(&e.code,if e.code=="PARTY_RELATIONSHIPS_VERSION_CONFLICT"{409}else{422});}
    let stored=encoded(&relation)?;
    let r=bind(db,"UPDATE crm_party_relationship_records SET state = ?,version = ?,idem_key = ?,idem_digest = ?,request_id = ? WHERE tenant = ? AND id = ? AND version = ?",
        &[JsValue::from_str(&stored),JsValue::from_f64(relation.version() as f64),
          JsValue::from_str(&k),JsValue::from_str(&hash),JsValue::from_str(trace),
          JsValue::from_str(tenant),JsValue::from_str(id),
          JsValue::from_f64(input.expected_version as f64)])?.run().await;
    let Ok(result)=r else{return error("PARTY_RELATIONSHIPS_UPDATE_CONFLICT",409)};
    if result.meta()?.and_then(|m|m.changes).unwrap_or(0)==0{
        return error("PARTY_RELATIONSHIPS_VERSION_CONFLICT",409);
    }
    response(json!({"item":view(&relation),"replayed":false}),200)
}
async fn list(db:&D1Database,tenant:&str)->Result<Response>{
    let rows=bind(db,"SELECT state FROM crm_party_relationship_records WHERE tenant = ? ORDER BY id LIMIT 300",
        &[JsValue::from_str(tenant)])?.all().await?.results::<Row>()?;
    let items=rows.iter().map(|row|decoded(&row.state).map(|v|view(&v)))
        .collect::<Result<Vec<_>>>()?;
    response(json!({"items":items}),200)
}
pub(super) async fn route(req:Request,db:D1Database,tenant:&str,trace:&str)->Result<Response>{
    let path=req.path();let method=req.method();
    if path=="/api/party-relationships" {
        return match method {
            Method::Get=>list(&db,tenant).await,
            Method::Post=>create(req,&db,tenant,trace).await,
            _=>error("NOT_FOUND",404),
        };
    }
    let Some(id)=path.strip_prefix("/api/party-relationships/") else{return error("NOT_FOUND",404)};
    if id.is_empty()||id.contains('/'){return error("NOT_FOUND",404);}
    match method {
        Method::Get=>match load(&db,tenant,id).await?{
            Some(v)=>response(json!({"item":view(&v)}),200),
            None=>error("NOT_FOUND",404),
        },
        Method::Patch=>update(req,&db,tenant,id,trace).await,
        _=>error("NOT_FOUND",404),
    }
}
