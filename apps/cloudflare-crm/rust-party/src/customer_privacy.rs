//! D1 operations for the original PrivacyCase domain. No HTTP/JSON business API.
//! Verification and privacy execution remain outside this intake capability.
use crm_customer_privacy::{
    PrivacyCase, PrivacyCaseKind, PrivacyDomainError,
    encode_privacy_case_state, decode_privacy_case_state,
};
use crm_module_sdk::{RecordId,TenantId,SchemaVersion};
use serde::Deserialize;
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use wasm_bindgen::JsValue;
use worker::{D1Database,Result};
use super::{bind,now_nanos};
#[derive(Deserialize)]
struct StateRow { state:String }
#[derive(Deserialize)]
struct Receipt { digest:String,state:String }
#[derive(Deserialize)]
struct PageRow { id:String,state:String }
pub(super) struct Saved {
    pub(super) case:PrivacyCase,
    pub(super) replayed:bool,
}
pub(super) struct Failure {
    pub(super) code:String,
    pub(super) status:u16,
}
pub(super) enum Transition { Submit, Cancel }
fn fail(code:impl Into<String>,status:u16)->Failure{Failure{code:code.into(),status}}
fn encode(c:&PrivacyCase)->Result<String>{
    String::from_utf8(encode_privacy_case_state(c)
        .map_err(|e|worker::Error::RustError(e.code))?)
        .map_err(|e|worker::Error::RustError(e.to_string()))
}
fn decode(s:&str)->Result<PrivacyCase>{
    decode_privacy_case_state(s.as_bytes()).map_err(|e|worker::Error::RustError(e.code))
}
fn digest(op:&str,id:&str,payload:&Value)->String{
    format!("{:x}",Sha256::digest(serde_json::to_vec(&json!([op,id,payload]))
        .expect("static digest serialization")))
}

async fn receipt(db:&D1Database,tenant:&str,k:&str)->Result<Option<Receipt>>{
    bind(db,"SELECT digest,state FROM crm_privacy_case_receipts WHERE tenant=? AND key=?",
        &[JsValue::from_str(tenant),JsValue::from_str(k)])?
        .first::<Receipt>(None).await
}
pub(super) async fn stored(db:&D1Database,tenant:&str,id:&str)->Result<Option<PrivacyCase>>{
    let row=bind(db,"SELECT state FROM crm_privacy_case_records WHERE tenant=? AND id=?",
        &[JsValue::from_str(tenant),JsValue::from_str(id)])?
        .first::<StateRow>(None).await?;
    row.map(|x|decode(&x.state)).transpose()
}

async fn replay(db:&D1Database,tenant:&str,key:&str,hash:&str)
    ->Result<Option<std::result::Result<Saved,Failure>>> {
    Ok(receipt(db,tenant,key).await?.map(|r| -> Result<std::result::Result<Saved,Failure>> {
        if r.digest!=hash {return Ok(Err(fail("IDEMPOTENCY_CONFLICT",409)));}
        Ok(Ok(Saved{case:decode(&r.state)?,replayed:true}))
    }).transpose()?)
}
pub(super) async fn create_typed(id:RecordId,kind:PrivacyCaseKind,key:&str,
    db:&D1Database,tenant:&str,actor:&str,trace:&str)
    ->Result<std::result::Result<Saved,Failure>> {
    if !(8..=120).contains(&key.len()){return Ok(Err(fail("IDEMPOTENCY_KEY_REQUIRED",422)));}
    let hash=digest("create",id.as_str(),&json!([kind]));
    if let Some(saved)=replay(db,tenant,key,&hash).await?{return Ok(saved);}
    let tenant_id=TenantId::try_new(tenant).map_err(|e|worker::Error::RustError(e.to_string()))?;
    let policy=SchemaVersion::try_new("1.0.0").map_err(|e|worker::Error::RustError(e.to_string()))?;
    let case=match PrivacyCase::new(id,tenant_id,kind,policy,now_nanos(),None){
        Ok(case)=>case,Err(e)=>return Ok(Err(fail(e.code(),422))),
    };
    let state=encode(&case)?;
    let inserted=bind(db,
        "INSERT INTO crm_privacy_case_records(tenant,id,state,version,idem_key,idem_digest,request_id,created_by) VALUES(?,?,?,?,?,?,?,?)",
        &[JsValue::from_str(tenant),JsValue::from_str(case.case_id().as_str()),
          JsValue::from_str(&state),JsValue::from_str(&case.version().to_string()),
          JsValue::from_str(key),JsValue::from_str(&hash),JsValue::from_str(trace),
          JsValue::from_str(actor)])?.run().await;
    if inserted.is_err(){
        if let Some(saved)=replay(db,tenant,key,&hash).await?{return Ok(saved);}
        return Ok(Err(fail("PRIVACY_CASE_CREATE_CONFLICT",409)));
    }
    Ok(Ok(Saved{case,replayed:false}))
}
pub(super) async fn transition_typed(id:&str,action:Transition,expected_version:u64,
    key:&str,db:&D1Database,tenant:&str,trace:&str)
    ->Result<std::result::Result<Saved,Failure>> {
    if !(8..=120).contains(&key.len()){return Ok(Err(fail("IDEMPOTENCY_KEY_REQUIRED",422)));}
    let operation=match action{Transition::Submit=>"submit",Transition::Cancel=>"cancel"};
    let hash=digest(operation,id,&json!([expected_version]));
    if let Some(saved)=replay(db,tenant,key,&hash).await?{return Ok(saved);}
    let Some(mut case)=stored(db,tenant,id).await? else{return Ok(Err(fail("NOT_FOUND",404)))};
    let result=match action{
        Transition::Submit=>case.submit(expected_version,now_nanos()),
        Transition::Cancel=>case.cancel(expected_version,now_nanos()),
    };
    if let Err(e)=result{
        return Ok(Err(fail(e.code(),match e{
            PrivacyDomainError::InvalidArgument{..}=>422,
            PrivacyDomainError::InvalidTransition{..}=>412,
            PrivacyDomainError::VersionConflict{..}=>409,
        })));
    }
    let state=encode(&case)?;
    let result=bind(db,
        "UPDATE crm_privacy_case_records SET state=?,version=?,idem_key=?,idem_digest=?,request_id=? WHERE tenant=? AND id=? AND version=?",
        &[JsValue::from_str(&state),JsValue::from_str(&case.version().to_string()),
          JsValue::from_str(key),JsValue::from_str(&hash),JsValue::from_str(trace),
          JsValue::from_str(tenant),JsValue::from_str(id),
          JsValue::from_str(&expected_version.to_string())])?.run().await;
    let changed=match result{
        Ok(result)=>result.meta()?.and_then(|m|m.changes).unwrap_or(0)>0,
        Err(_)=>false,
    };
    if !changed{
        if let Some(saved)=replay(db,tenant,key,&hash).await?{return Ok(saved);}
        return Ok(Err(fail("PRIVACY_CASE_VERSION_CONFLICT",409)));
    }
    Ok(Ok(Saved{case,replayed:false}))
}
pub(super) async fn list_page_typed(db:&D1Database,tenant:&str,after:&str,size:usize)
    ->Result<(Vec<PrivacyCase>,Option<String>)>{
    let mut rows=bind(db,
        "SELECT id,state FROM crm_privacy_case_records WHERE tenant=? AND id>? ORDER BY id LIMIT ?",
        &[JsValue::from_str(tenant),JsValue::from_str(after),
          JsValue::from_f64((size+1) as f64)])?.all().await?.results::<PageRow>()?;
    let next=if rows.len()>size{rows.truncate(size);rows.last().map(|r|r.id.clone())}else{None};
    let cases=rows.into_iter().map(|row|decode(&row.state)).collect::<Result<Vec<_>>>()?;
    Ok((cases,next))
}
