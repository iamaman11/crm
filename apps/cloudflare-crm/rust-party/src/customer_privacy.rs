//! Safe original crm-customer-privacy intake adapter. Only the original
//! PrivacyCase domain owns state transitions and persisted canonical encoding.
//! Subject identity verification and all privacy execution are NOT connected.
use crm_customer_privacy::{
    PrivacyCase,PrivacyCaseKind,PrivacyDomainError,
    encode_privacy_case_state,decode_privacy_case_state,
};
use crm_module_sdk::{RecordId,TenantId,SchemaVersion};
use serde::Deserialize;
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use wasm_bindgen::JsValue;
use worker::{D1Database,Method,Request,Response,Result};
use super::{bind,error,now_nanos,response};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateInput{id:String,kind:String}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TransitionInput{expected_version:u64}
#[derive(Deserialize)]
struct StateRow{state:String}
#[derive(Deserialize)]
struct Receipt{digest:String,state:String}

fn view(c:&PrivacyCase)->Value{
    let kind=match c.kind(){
        PrivacyCaseKind::Access=>"access",
        PrivacyCaseKind::PortabilityExport=>"portability_export",
        PrivacyCaseKind::RestrictProcessing=>"restrict_processing",
        PrivacyCaseKind::Erasure=>"erasure",
    };
    json!({"id":c.case_id().as_str(),"kind":kind,"status":c.status().label(),
        "version":c.version(),"subject_verified":c.subject_binding().is_some(),
        "execution":"NOT_CONNECTED"})
}
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
fn key(req:&Request)->Result<Option<String>>{
    Ok(req.headers().get("idempotency-key")?
        .filter(|x|(8..=120).contains(&x.len())))
}
async fn receipt(db:&D1Database,tenant:&str,k:&str)->Result<Option<Receipt>>{
    bind(db,"SELECT digest,state FROM crm_privacy_case_receipts WHERE tenant=? AND key=?",
        &[JsValue::from_str(tenant),JsValue::from_str(k)])?
        .first::<Receipt>(None).await
}
async fn stored(db:&D1Database,tenant:&str,id:&str)->Result<Option<PrivacyCase>>{
    let row=bind(db,"SELECT state FROM crm_privacy_case_records WHERE tenant=? AND id=?",
        &[JsValue::from_str(tenant),JsValue::from_str(id)])?
        .first::<StateRow>(None).await?;
    row.map(|x|decode(&x.state)).transpose()
}
async fn list(db:&D1Database,tenant:&str)->Result<Response>{
    let rows=bind(db,"SELECT state FROM crm_privacy_case_records WHERE tenant=? ORDER BY id LIMIT 101",
        &[JsValue::from_str(tenant)])?.all().await?.results::<StateRow>()?;
    if rows.len()>100{return error("PRIVACY_CASE_LIST_LIMIT_REACHED",503);}
    let items=rows.iter().map(|row|decode(&row.state).map(|c|view(&c)))
        .collect::<Result<Vec<_>>>()?;
    response(json!({"items":items,"subject_verification":"NOT_CONNECTED",
        "privacy_execution":"NOT_CONNECTED"}),200)
}
async fn create(mut req:Request,db:&D1Database,tenant:&str,actor:&str,trace:&str)
    ->Result<Response>{
    let Some(k)=key(&req)? else{return error("IDEMPOTENCY_KEY_REQUIRED",422)};
    let input=match req.json::<CreateInput>().await{
        Ok(x)=>x,Err(_)=>return error("INVALID_INPUT",422)
    };
    let kind=match input.kind.as_str(){
        "access"=>PrivacyCaseKind::Access,
        "portability_export"=>PrivacyCaseKind::PortabilityExport,
        "restrict_processing"=>PrivacyCaseKind::RestrictProcessing,
        "erasure"=>PrivacyCaseKind::Erasure,
        _=>return error("PRIVACY_CASE_KIND_INVALID",422),
    };
    let hash=digest("create",&input.id,&json!([input.kind]));
    if let Some(saved)=receipt(db,tenant,&k).await?{
        if saved.digest!=hash{return error("IDEMPOTENCY_CONFLICT",409);}
        return response(json!({"item":view(&decode(&saved.state)?),"replayed":true}),200);
    }
    let id=match RecordId::try_new(input.id){
        Ok(id)=>id,Err(_)=>return error("PRIVACY_CASE_ID_INVALID",422),
    };
    let tenant_id=TenantId::try_new(tenant)
        .map_err(|e|worker::Error::RustError(e.to_string()))?;
    let policy=SchemaVersion::try_new("1.0.0")
        .map_err(|e|worker::Error::RustError(e.to_string()))?;
    let case=PrivacyCase::new(id,tenant_id,kind,policy,now_nanos(),None)
        .map_err(|e|worker::Error::RustError(e.code().into()))?;
    let state=encode(&case)?;
    if bind(db,"INSERT INTO crm_privacy_case_records(tenant,id,state,version,idem_key,idem_digest,request_id,created_by) VALUES(?,?,?,?,?,?,?,?)",
        &[JsValue::from_str(tenant),JsValue::from_str(case.case_id().as_str()),
          JsValue::from_str(&state),JsValue::from_f64(case.version() as f64),
          JsValue::from_str(&k),JsValue::from_str(&hash),JsValue::from_str(trace),
          JsValue::from_str(actor)])?.run().await.is_err(){
        return error("PRIVACY_CASE_CREATE_CONFLICT",409);
    }
    response(json!({"item":view(&case),"replayed":false}),201)
}
async fn transition(mut req:Request,db:&D1Database,tenant:&str,
    id:&str,action:&str,trace:&str)->Result<Response>{
    let Some(k)=key(&req)? else{return error("IDEMPOTENCY_KEY_REQUIRED",422)};
    let input=match req.json::<TransitionInput>().await{
        Ok(x)=>x,Err(_)=>return error("INVALID_INPUT",422)
    };
    let hash=digest(action,id,&json!([input.expected_version]));
    if let Some(saved)=receipt(db,tenant,&k).await?{
        if saved.digest!=hash{return error("IDEMPOTENCY_CONFLICT",409);}
        return response(json!({"item":view(&decode(&saved.state)?),"replayed":true}),200);
    }
    let Some(mut case)=stored(db,tenant,id).await? else{return error("NOT_FOUND",404)};
    let result=match action{
        "submit"=>case.submit(input.expected_version,now_nanos()),
        "cancel"=>case.cancel(input.expected_version,now_nanos()),
        _=>return error("NOT_FOUND",404),
    };
    if let Err(e)=result{
        return error(e.code(),match e{
            PrivacyDomainError::InvalidArgument{..}=>422,_=>409
        });
    }
    let state=encode(&case)?;
    let result=bind(db,"UPDATE crm_privacy_case_records SET state=?,version=?,idem_key=?,idem_digest=?,request_id=? WHERE tenant=? AND id=? AND version=?",
        &[JsValue::from_str(&state),JsValue::from_f64(case.version() as f64),
          JsValue::from_str(&k),JsValue::from_str(&hash),JsValue::from_str(trace),
          JsValue::from_str(tenant),JsValue::from_str(id),
          JsValue::from_f64(input.expected_version as f64)])?.run().await;
    let Ok(updated)=result else{return error("PRIVACY_CASE_TRANSITION_CONFLICT",409)};
    if updated.meta()?.and_then(|m|m.changes).unwrap_or(0)!=1{
        return error("PRIVACY_CASE_VERSION_CONFLICT",409);
    }
    response(json!({"item":view(&case),"replayed":false}),200)
}
pub(super) async fn route(req:Request,db:D1Database,tenant:&str,
    actor:&str,role:&str,trace:&str)->Result<Response>{
    let path=req.path();let method=req.method();
    if path=="/api/customer-privacy/cases"{
        return match method{
            Method::Get=>list(&db,tenant).await,
            Method::Post if role=="admin"=>create(req,&db,tenant,actor,trace).await,
            Method::Post=>error("FORBIDDEN",403),
            _=>error("NOT_FOUND",404)
        };
    }
    let Some(tail)=path.strip_prefix("/api/customer-privacy/cases/") else{
        return error("NOT_FOUND",404)
    };
    let (id,action)=if let Some(id)=tail.strip_suffix("/submit"){(id,"submit")}
        else if let Some(id)=tail.strip_suffix("/cancel"){(id,"cancel")}
        else{(tail,"")};
    if id.is_empty()||id.contains('/'){return error("NOT_FOUND",404);}
    match (action,method){
        ("",Method::Get)=>match stored(&db,tenant,id).await?{
            Some(case)=>response(json!({"item":view(&case)}),200),
            None=>error("NOT_FOUND",404)
        },
        ("submit"|"cancel",Method::Post) if role=="admin"=>
            transition(req,&db,tenant,id,action,trace).await,
        ("submit"|"cancel",Method::Post)=>error("FORBIDDEN",403),
        _=>error("NOT_FOUND",404)
    }
}
