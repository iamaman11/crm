//! Native HTTP + D1 boundary for ORIGINAL crm-activities Task aggregate.
//! Domain validity/completion rules remain exclusively in crm-activities.
//! Owner comes from a cryptographically verified Cloudflare Access principal.
use crm_activities::{
    Task, TaskOwner, TaskPriority, TaskStatus, TaskMutation,
    CreateTask, CompleteTask, encode_task_state, decode_task_state,
};
use crm_module_sdk::{ActorId, RecordId, ResourceRef};
use serde::{Deserialize,Serialize};
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use wasm_bindgen::JsValue;
use worker::{D1Database,Method,Request,Response,Result};
use super::{bind,error,now_nanos,response};

#[derive(Clone,Copy,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
enum Priority { Low, Normal, High, Urgent }
impl From<Priority> for TaskPriority {
    fn from(v:Priority)->Self {
        match v {Priority::Low=>Self::Low,Priority::Normal=>Self::Normal,
            Priority::High=>Self::High,Priority::Urgent=>Self::Urgent}
    }
}
#[derive(Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateInput {
    id:String,subject:String,description:Option<String>,
    account_id:String,priority:Priority,
    due_at_unix_nanos:Option<i64>,reminder_at_unix_nanos:Option<i64>,
}
#[derive(Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct CompleteInput { expected_version:i64 }
#[derive(Deserialize)]
struct Row { state:String }
#[derive(Deserialize)]
struct Receipt {digest:String,state:String}
fn hash<T:Serialize>(operation:&str,id:&str,payload:&T)->Result<String>{
    let bytes=serde_json::to_vec(&json!([operation,id,payload]))
        .map_err(|e|worker::Error::RustError(e.to_string()))?;
    Ok(format!("{:x}",Sha256::digest(bytes)))
}
fn state(item:&Task)->Result<String>{
    String::from_utf8(encode_task_state(item)
        .map_err(|e|worker::Error::RustError(e.code))?)
        .map_err(|e|worker::Error::RustError(e.to_string()))
}
fn decode(input:&str)->Result<Task>{
    decode_task_state(input.as_bytes()).map_err(|e|worker::Error::RustError(e.code))
}
fn view(task:&Task)->Value{
    json!({
        "id":task.task_id().as_str(),"subject":task.subject(),
        "description":task.description(),
        "owner":match task.owner(){
            TaskOwner::Actor(actor)=>json!({"actor":actor.as_str()}),
            TaskOwner::Team(team)=>json!({"team":team.as_str()}),
        },
        "related_resources":task.related_resources(),
        "priority":match task.priority(){
            TaskPriority::Low=>"low",TaskPriority::Normal=>"normal",
            TaskPriority::High=>"high",TaskPriority::Urgent=>"urgent",
        },
        "status":match task.status(){
            TaskStatus::Open=>"open",TaskStatus::Completed=>"completed",
        },
        "due_at_unix_nanos":task.due_at_unix_nanos(),
        "reminder_at_unix_nanos":task.reminder_at_unix_nanos(),
        "completed_at_unix_nanos":task.completed_at_unix_nanos(),
        "version":task.version(),
    })
}
fn idempotency_key(req:&Request)->Result<Option<String>>{
    Ok(req.headers().get("idempotency-key")?
        .filter(|value|(8..=120).contains(&value.len())))
}
async fn prior(db:&D1Database,tenant:&str,key:&str)->Result<Option<Receipt>>{
    bind(db,"SELECT digest,state FROM crm_activity_task_receipts WHERE tenant = ? AND key = ?",
        &[JsValue::from_str(tenant),JsValue::from_str(key)])?
        .first::<Receipt>(None).await
}
async fn stored(db:&D1Database,tenant:&str,id:&str)->Result<Option<Task>>{
    bind(db,"SELECT state FROM crm_activity_task_records WHERE tenant = ? AND id = ?",
        &[JsValue::from_str(tenant),JsValue::from_str(id)])?
        .first::<Row>(None).await?.map(|r|decode(&r.state)).transpose()
}
async fn account_exists(db:&D1Database,tenant:&str,id:&str)->Result<bool>{
    Ok(bind(db,"SELECT id FROM crm_account_records WHERE tenant = ? AND id = ?",
        &[JsValue::from_str(tenant),JsValue::from_str(id)])?
        .first::<Value>(None).await?.is_some())
}
async fn create(mut req:Request,db:&D1Database,tenant:&str,actor:&str,trace:&str)->Result<Response>{
    let Some(key)=idempotency_key(&req)? else{return error("IDEMPOTENCY_KEY_REQUIRED",422)};
    let input=match serde_json::from_str::<CreateInput>(&req.text().await?){
        Ok(x)=>x,Err(_)=>return error("INVALID_INPUT",422),
    };
    let digest=hash("create",&input.id,&input)?;
    if let Some(p)=prior(db,tenant,&key).await?{
        if p.digest!=digest{return error("IDEMPOTENCY_CONFLICT",409)}
        return response(json!({"item":view(&decode(&p.state)?),"replayed":true}),200);
    }
    let task_id=match RecordId::try_new(&input.id){
        Ok(x)=>x,Err(_)=>return error("ACTIVITIES_TASK_ID_INVALID",422),
    };
    let account_id=match RecordId::try_new(&input.account_id){
        Ok(x)=>x,Err(_)=>return error("ACTIVITIES_ACCOUNT_ID_INVALID",422),
    };
    if !account_exists(db,tenant,account_id.as_str()).await?{
        return error("ACTIVITIES_ACCOUNT_NOT_FOUND",422);
    }
    let owner=match ActorId::try_new(actor){
        Ok(x)=>TaskOwner::Actor(x),Err(_)=>return error("ACTIVITIES_OWNER_INVALID",422),
    };
    let task=match Task::create(CreateTask{
        task_id,subject:input.subject,description:input.description,owner,
        related_resources:vec![ResourceRef{
            resource_type:"crm.customer-accounts".into(),
            resource_id:account_id.as_str().to_string(),version:None,
        }],
        priority:input.priority.into(),due_at_unix_nanos:input.due_at_unix_nanos,
        reminder_at_unix_nanos:input.reminder_at_unix_nanos,
        occurred_at_unix_nanos:now_nanos(),
    }){Ok(x)=>x,Err(e)=>return error(&e.code,422)};
    let saved=state(&task)?;
    let r=bind(db,"INSERT INTO crm_activity_task_records(tenant,id,account_id,state,version,idem_key,idem_digest,request_id) VALUES(?,?,?,?,?,?,?,?)",
        &[JsValue::from_str(tenant),JsValue::from_str(task.task_id().as_str()),
          JsValue::from_str(account_id.as_str()),JsValue::from_str(&saved),
          JsValue::from_f64(task.version() as f64),JsValue::from_str(&key),
          JsValue::from_str(&digest),JsValue::from_str(trace)])?.run().await;
    if r.is_err(){return error("ACTIVITIES_TASK_CREATE_CONFLICT",409);}
    response(json!({"item":view(&task),"replayed":false}),201)
}
async fn complete(mut req:Request,db:&D1Database,tenant:&str,id:&str,trace:&str)->Result<Response>{
    let Some(key)=idempotency_key(&req)? else{return error("IDEMPOTENCY_KEY_REQUIRED",422)};
    let input=match serde_json::from_str::<CompleteInput>(&req.text().await?){
        Ok(x)=>x,Err(_)=>return error("INVALID_INPUT",422),
    };
    let digest=hash("complete",id,&input)?;
    if let Some(p)=prior(db,tenant,&key).await?{
        if p.digest!=digest{return error("IDEMPOTENCY_CONFLICT",409)}
        return response(json!({"item":view(&decode(&p.state)?),"replayed":true}),200);
    }
    let Some(mut task)=stored(db,tenant,id).await? else{return error("NOT_FOUND",404)};
    let changed=match task.complete(CompleteTask{
        expected_version:input.expected_version,occurred_at_unix_nanos:now_nanos(),
    }){
        Ok(x)=>x,
        Err(e)=>return error(&e.code,if e.code=="ACTIVITIES_TASK_VERSION_CONFLICT"{409}else{422}),
    };
    // The original aggregate decides that completion is idempotent. Do not
    // fabricate a new version or audit record for an unchanged aggregate.
    if changed==TaskMutation::Unchanged{
        return response(json!({"item":view(&task),"changed":false}),200);
    }
    let saved=state(&task)?;
    let r=bind(db,"UPDATE crm_activity_task_records SET state=?,version=?,idem_key=?,idem_digest=?,request_id=? WHERE tenant=? AND id=? AND version=?",
        &[JsValue::from_str(&saved),JsValue::from_f64(task.version() as f64),
          JsValue::from_str(&key),JsValue::from_str(&digest),JsValue::from_str(trace),
          JsValue::from_str(tenant),JsValue::from_str(id),
          JsValue::from_f64(input.expected_version as f64)])?.run().await;
    let Ok(result)=r else{return error("ACTIVITIES_TASK_COMPLETE_CONFLICT",409)};
    if result.meta()?.and_then(|m|m.changes).unwrap_or(0)==0{
        return error("ACTIVITIES_TASK_VERSION_CONFLICT",409);
    }
    response(json!({"item":view(&task),"changed":true,"replayed":false}),200)
}
async fn list(db:&D1Database,tenant:&str)->Result<Response>{
    let rows=bind(db,"SELECT state FROM crm_activity_task_records WHERE tenant = ? ORDER BY id LIMIT 300",
        &[JsValue::from_str(tenant)])?.all().await?.results::<Row>()?;
    let items=rows.iter().map(|r|decode(&r.state).map(|task|view(&task)))
        .collect::<Result<Vec<_>>>()?;
    response(json!({"items":items}),200)
}
pub(super) async fn route(req:Request,db:D1Database,tenant:&str,actor:&str,trace:&str)->Result<Response>{
    let path=req.path();let method=req.method();
    if path=="/api/activity-tasks"{
        return match method {
            Method::Get=>list(&db,tenant).await,
            Method::Post=>create(req,&db,tenant,actor,trace).await,
            _=>error("NOT_FOUND",404),
        };
    }
    let Some(id)=path.strip_prefix("/api/activity-tasks/") else{return error("NOT_FOUND",404)};
    if id.is_empty()||id.contains('/'){return error("NOT_FOUND",404);}
    if let Some(id)=id.strip_suffix("/complete"){
        if id.is_empty()||id.contains('/')||method!=Method::Post{return error("NOT_FOUND",404);}
        return complete(req,&db,tenant,id,trace).await;
    }
    match method {
        Method::Get=>match stored(&db,tenant,id).await?{
            Some(item)=>response(json!({"item":view(&item)}),200),
            None=>error("NOT_FOUND",404),
        },
        _=>error("NOT_FOUND",404),
    }
}
