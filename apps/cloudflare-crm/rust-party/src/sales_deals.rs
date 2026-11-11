//! Cloudflare native HTTP/D1 boundary for the ORIGINAL crm-sales Deal aggregate.
//! Stage progression, closing rules, money and version invariants belong to
//! crm-sales and crm-core-contracts, never the test pilot's crm_deals model.
use crm_sales::{
    Deal, DealOwner, DealStage, DealStatus, CreateDeal, UpdateDeal, AdvanceDealStage,
    StageTransitionPolicy, PipelineId, StageId, ReasonCode,
    encode_deal_state, decode_deal_state,
};
use crm_module_sdk::{ActorId, RecordId, ResourceRef, ErrorCategory};
use crm_core_contracts::{BasisPoints, CurrencyCode, Money, Patch};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest,Sha256};
use wasm_bindgen::JsValue;
use worker::{D1Database, Method, Request, Response, Result};
use super::{bind, error, now_nanos, response};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CreateInput {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) account_id: String,
    pub(super) pipeline_id: String,
    pub(super) stage_id: String,
    pub(super) ordinal: u16,
    pub(super) probability_bps: u16,
    // Minor units are exact integers. Floating point money is never accepted.
    pub(super) amount_minor_units: Option<i64>,
    pub(super) currency: Option<String>,
}
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all="snake_case")]
pub(super) enum StatusInput { Open, Won, Lost }
impl From<StatusInput> for DealStatus {
    fn from(value: StatusInput)->Self{
        match value {StatusInput::Open=>Self::Open,
            StatusInput::Won=>Self::Won,StatusInput::Lost=>Self::Lost}
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StageInput {
    pub(super) expected_version:i64,
    pub(super) pipeline_id:String,
    pub(super) stage_id:String,
    pub(super) ordinal:u16,
    pub(super) status:StatusInput,
    pub(super) close_reason_code:Option<String>,
}
// HTTP only edits the original Deal's business fields, never trusted ownership.
#[derive(Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct UpdateInput {
    pub(super) expected_version:i64,
    pub(super) name:Option<String>,
    pub(super) clear_name:Option<bool>,
    pub(super) amount_minor_units:Option<i64>,
    pub(super) currency:Option<String>,
    pub(super) clear_amount:Option<bool>,
    pub(super) probability_bps:Option<u16>,
}
#[derive(Deserialize)]
struct Row { state: String }
#[derive(Deserialize)]
struct PagedRow { id:String,state:String }
#[derive(Deserialize)]
struct Receipt { digest:String,state:String }

fn hash<T:Serialize>(operation:&str,id:&str,input:&T)->Result<String> {
    let bytes=serde_json::to_vec(&json!([operation,id,input]))
        .map_err(|e|worker::Error::RustError(e.to_string()))?;
    Ok(format!("{:x}",Sha256::digest(bytes)))
}
fn encode(deal:&Deal)->Result<String>{
    String::from_utf8(encode_deal_state(deal)
        .map_err(|e|worker::Error::RustError(e.code))?)
        .map_err(|e|worker::Error::RustError(e.to_string()))
}
pub(super) fn decode(data:&str)->Result<Deal>{
    decode_deal_state(data.as_bytes()).map_err(|e|worker::Error::RustError(e.code))
}
pub(super) fn view(deal:&Deal)->Value{
    let stage=deal.stage();
    json!({
        "id":deal.deal_id().as_str(),
        "name":deal.name(),
        "owner":match deal.owner(){
            DealOwner::Actor(x)=>json!({"actor":x.as_str()}),
            DealOwner::Team(x)=>json!({"team":x.as_str()}),
        },
        "account_id":deal.account().map(|x|x.resource_id.as_str()),
        "primary_contact_id":deal.primary_contact().map(|x|x.resource_id.as_str()),
        "stage":{"pipeline_id":stage.pipeline_id().as_str(),
                 "stage_id":stage.stage_id().as_str(),"ordinal":stage.ordinal()},
        "status":match deal.status(){
            DealStatus::Open=>"open",DealStatus::Won=>"won",DealStatus::Lost=>"lost",
        },
        "amount":deal.amount().map(|amount|json!({
            "minor_units":amount.minor_units().to_string(),
            "currency":amount.currency().as_str(),
        })),
        "probability_bps":deal.probability().get(),
        "close_outcome":deal.close_outcome().map(|outcome|json!({
            "status":match outcome.status(){
                DealStatus::Open=>"open",DealStatus::Won=>"won",DealStatus::Lost=>"lost",
            },
            "reason_code":outcome.reason_code().as_str(),
            "closed_at_unix_nanos":outcome.closed_at_unix_nanos(),
        })),
        "version":deal.version(),
    })
}
fn key(req:&Request)->Result<Option<String>>{
    Ok(req.headers().get("idempotency-key")?
        .filter(|value|(8..=120).contains(&value.len())))
}
async fn prior(db:&D1Database,tenant:&str,k:&str)->Result<Option<Receipt>>{
    bind(db,"SELECT digest,state FROM crm_sales_deal_receipts WHERE tenant=? AND key=?",
        &[JsValue::from_str(tenant),JsValue::from_str(k)])?.first::<Receipt>(None).await
}
pub(super) async fn load(db:&D1Database,tenant:&str,id:&str)->Result<Option<Deal>>{
    bind(db,"SELECT state FROM crm_sales_deal_records WHERE tenant=? AND id=?",
        &[JsValue::from_str(tenant),JsValue::from_str(id)])?
        .first::<Row>(None).await?.map(|row|decode(&row.state)).transpose()
}
async fn account_exists(db:&D1Database,tenant:&str,id:&str)->Result<bool>{
    Ok(bind(db,"SELECT id FROM crm_account_records WHERE tenant=? AND id=?",
        &[JsValue::from_str(tenant),JsValue::from_str(id)])?
        .first::<Value>(None).await?.is_some())
}
fn stage(pipeline_id:String,stage_id:String,ordinal:u16)
    ->std::result::Result<DealStage,String> {
    let pipeline=PipelineId::try_new(pipeline_id).map_err(|e|e.code)?;
    let id=StageId::try_new(stage_id).map_err(|e|e.code)?;
    DealStage::try_new(pipeline,id,ordinal).map_err(|e|e.code)
}
// One D1 + ORIGINAL crm-sales mutation shared by both native HTTP and Connect.
// The caller handles transport-level decoding and error formatting only.
pub(super) struct SalesSaved {
    pub(super) deal: Deal,
    pub(super) replayed: bool,
}
pub(super) struct SalesFailure {
    pub(super) code: String,
    pub(super) status: u16,
}
fn rejected(code: impl Into<String>, status: u16)
    -> std::result::Result<SalesSaved, SalesFailure> {
    Err(SalesFailure {code: code.into(), status})
}
pub(super) async fn create_typed(input:CreateInput,k:&str,db:&D1Database,
    tenant:&str,actor:&str,trace:&str)
    -> Result<std::result::Result<SalesSaved, SalesFailure>> {
    if !(8..=120).contains(&k.len()) {
        return Ok(rejected("IDEMPOTENCY_KEY_REQUIRED",422));
    }
    let digest=hash("create",&input.id,&input)?;
    if let Some(p)=prior(db,tenant,k).await? {
        if p.digest!=digest{return Ok(rejected("IDEMPOTENCY_CONFLICT",409));}
        return Ok(Ok(SalesSaved{deal:decode(&p.state)?,replayed:true}));
    }
    let id=match RecordId::try_new(&input.id){
        Ok(x)=>x,Err(_)=>return Ok(rejected("SALES_DEAL_ID_INVALID",422)),
    };
    let account_id=match RecordId::try_new(&input.account_id){
        Ok(x)=>x,Err(_)=>return Ok(rejected("SALES_ACCOUNT_ID_INVALID",422)),
    };
    if !account_exists(db,tenant,account_id.as_str()).await? {
        return Ok(rejected("SALES_ACCOUNT_NOT_FOUND",422));
    }
    let owner=match ActorId::try_new(actor){
        Ok(x)=>DealOwner::Actor(x),Err(_)=>return Ok(rejected("SALES_OWNER_INVALID",422)),
    };
    let stage=match stage(input.pipeline_id,input.stage_id,input.ordinal){
        Ok(x)=>x,Err(code)=>return Ok(rejected(code,422)),
    };
    let probability=match BasisPoints::try_new(input.probability_bps){
        Ok(x)=>x,Err(e)=>return Ok(rejected(e.code,422)),
    };
    let amount=match (input.amount_minor_units,input.currency) {
        (None,None)=>None,
        (Some(units),Some(currency))=>{
            let currency=match CurrencyCode::try_new(currency){
                Ok(x)=>x,Err(e)=>return Ok(rejected(e.code,422)),
            };
            Some(Money::new(i128::from(units),currency))
        },
        _=>return Ok(rejected("SALES_AMOUNT_CURRENCY_PAIR_REQUIRED",422)),
    };
    let deal=match Deal::create(CreateDeal{
        deal_id:id,name:input.name,owner,
        account:Some(ResourceRef{resource_type:"crm.customer-accounts".into(),
            resource_id:account_id.as_str().into(),version:None}),
        primary_contact:None,stage,amount,
        expected_close_date:None,probability,
        occurred_at_unix_nanos:now_nanos(),
    }){
        Ok(x)=>x,Err(e)=>return Ok(rejected(e.code,422)),
    };
    let saved=encode(&deal)?;
    let r=bind(db,"INSERT INTO crm_sales_deal_records(tenant,id,account_id,state,version,idem_key,idem_digest,request_id) VALUES(?,?,?,?,?,?,?,?)",
        &[JsValue::from_str(tenant),JsValue::from_str(deal.deal_id().as_str()),
          JsValue::from_str(account_id.as_str()),JsValue::from_str(&saved),
          JsValue::from_f64(deal.version() as f64),JsValue::from_str(k),
          JsValue::from_str(&digest),JsValue::from_str(trace)])?.run().await;
    if r.is_err(){return Ok(rejected("SALES_DEAL_CREATE_CONFLICT",409));}
    Ok(Ok(SalesSaved{deal,replayed:false}))
}
async fn create(mut req:Request,db:&D1Database,tenant:&str,actor:&str,trace:&str)
    ->Result<Response>{
    let Some(k)=key(&req)? else{return error("IDEMPOTENCY_KEY_REQUIRED",422)};
    // Preserve the strict JSON REST boundary during the Connect migration.
    let input=match serde_json::from_str::<CreateInput>(&req.text().await?){
        Ok(x)=>x,Err(_)=>return error("INVALID_INPUT",422),
    };
    match create_typed(input,&k,db,tenant,actor,trace).await? {
        Ok(saved)=>response(json!({"item":view(&saved.deal),
            "replayed":saved.replayed}),if saved.replayed{200}else{201}),
        Err(failure)=>error(&failure.code,failure.status),
    }
}
/// The ORIGINAL owner applies Sales updates, CAS, receipt replay and audit.
pub(super) async fn update_typed(
    input:UpdateInput,key:&str,db:&D1Database,tenant:&str,id:&str,trace:&str,
)->Result<std::result::Result<SalesSaved,SalesFailure>>{
    if !(8..=120).contains(&key.len()){
        return Ok(rejected("IDEMPOTENCY_KEY_REQUIRED",422));
    }
    let digest=hash("update",id,&input)?;
    if let Some(p)=prior(db,tenant,key).await?{
        if p.digest!=digest{return Ok(rejected("IDEMPOTENCY_CONFLICT",409));}
        return Ok(Ok(SalesSaved{deal:decode(&p.state)?,replayed:true}));
    }
    let Some(mut deal)=load(db,tenant,id).await? else{return Ok(rejected("NOT_FOUND",404));};
    if input.clear_name==Some(true)&&input.name.is_some(){
        return Ok(rejected("SALES_NAME_UPDATE_CONFLICT",422));
    }
    let name=if input.clear_name==Some(true){Patch::Clear}
        else{input.name.map_or(Patch::Keep,Patch::Set)};
    if input.clear_amount==Some(true)&&
        (input.amount_minor_units.is_some()||input.currency.is_some()){
        return Ok(rejected("SALES_AMOUNT_UPDATE_CONFLICT",422));
    }
    let amount=if input.clear_amount==Some(true){Patch::Clear}else{
        match (input.amount_minor_units,input.currency){
            (None,None)=>Patch::Keep,
            (Some(units),Some(currency))=>{
                let currency=match CurrencyCode::try_new(currency){
                    Ok(x)=>x,Err(e)=>return Ok(rejected(e.code,422)),
                };
                Patch::Set(Money::new(i128::from(units),currency))
            },
            _=>return Ok(rejected("SALES_AMOUNT_CURRENCY_PAIR_REQUIRED",422)),
        }
    };
    let probability=match input.probability_bps{
        Some(v)=>match BasisPoints::try_new(v){
            Ok(x)=>Patch::Set(x),Err(e)=>return Ok(rejected(e.code,422)),
        },
        None=>Patch::Keep,
    };
    if matches!(name,Patch::Keep)&&matches!(amount,Patch::Keep)&&
        matches!(probability,Patch::Keep){
        return Ok(rejected("SALES_EMPTY_UPDATE",422));
    }
    if let Err(e)=deal.apply_update(UpdateDeal{
        expected_version:input.expected_version,
        name,owner:Patch::Keep,account:Patch::Keep,
        primary_contact:Patch::Keep,amount,
        expected_close_date:Patch::Keep,probability,
        occurred_at_unix_nanos:now_nanos(),
    }){
        return Ok(rejected(e.code,
            if e.category==ErrorCategory::Conflict{409}else{422}));
    }
    let saved=encode(&deal)?;
    let result=bind(db,"UPDATE crm_sales_deal_records SET state=?,version=?,idem_key=?,idem_digest=?,request_id=?,last_action='sales_deals.update' WHERE tenant=? AND id=? AND version=?",
        &[JsValue::from_str(&saved),JsValue::from_f64(deal.version() as f64),
          JsValue::from_str(key),JsValue::from_str(&digest),JsValue::from_str(trace),
          JsValue::from_str(tenant),JsValue::from_str(id),
          JsValue::from_f64(input.expected_version as f64)])?.run().await;
    let Ok(result)=result else{return Ok(rejected("SALES_DEAL_UPDATE_CONFLICT",409));};
    if result.meta()?.and_then(|m|m.changes).unwrap_or(0)==0{
        return Ok(rejected("SALES_DEAL_VERSION_CONFLICT",409));
    }
    Ok(Ok(SalesSaved{deal,replayed:false}))
}
async fn update(mut req:Request,db:&D1Database,tenant:&str,id:&str,trace:&str)
    ->Result<Response>{
    let Some(k)=key(&req)? else{return error("IDEMPOTENCY_KEY_REQUIRED",422)};
    let input=match serde_json::from_str::<UpdateInput>(&req.text().await?){
        Ok(x)=>x,Err(_)=>return error("INVALID_INPUT",422),
    };
    match update_typed(input,&k,db,tenant,id,trace).await?{
        Ok(saved)=>response(json!({"item":view(&saved.deal),
            "replayed":saved.replayed}),200),
        Err(f)=>error(&f.code,f.status),
    }
}

pub(super) struct LinkOutcome {
    pub(super) delivery_id:String,
    pub(super) applied:bool,
    pub(super) task_id:Option<String>,
}
pub(super) struct AdvanceSaved {
    pub(super) saved:SalesSaved,
    pub(super) link:Option<LinkOutcome>,
}
fn advance_rejected(code:impl Into<String>,status:u16)
    ->std::result::Result<AdvanceSaved,SalesFailure>{
    Err(SalesFailure{code:code.into(),status})
}
/// Atomically persists ONE domain stage advancement, its idempotency receipt
/// and an optional original Activities follow-up in the SAME D1 batch.
pub(super) async fn advance_typed(
    input:StageInput,key:&str,db:&D1Database,tenant:&str,id:&str,actor:&str,
    trace:&str,
)->Result<std::result::Result<AdvanceSaved,SalesFailure>>{
    if !(8..=120).contains(&key.len()){
        return Ok(advance_rejected("IDEMPOTENCY_KEY_REQUIRED",422));
    }
    let digest=hash("advance",id,&input)?;
    if let Some(p)=prior(db,tenant,key).await?{
        if p.digest!=digest{return Ok(advance_rejected("IDEMPOTENCY_CONFLICT",409));}
        return Ok(Ok(AdvanceSaved{
            saved:SalesSaved{deal:decode(&p.state)?,replayed:true},link:None
        }));
    }
    let Some(mut deal)=load(db,tenant,id).await? else{
        return Ok(advance_rejected("NOT_FOUND",404));
    };
    let before=deal.clone();
    let target=match stage(input.pipeline_id,input.stage_id,input.ordinal){
        Ok(x)=>x,Err(code)=>return Ok(advance_rejected(code,422)),
    };
    let reason=match input.close_reason_code{
        Some(code)=>match ReasonCode::try_new(code){
            Ok(v)=>Some(v),Err(e)=>return Ok(advance_rejected(e.code,422)),
        },
        None=>None,
    };
    if let Err(e)=deal.advance_stage(AdvanceDealStage{
        expected_version:input.expected_version,
        target_stage:target,target_status:input.status.into(),
        close_reason_code:reason,occurred_at_unix_nanos:now_nanos(),
        policy:StageTransitionPolicy::default(),
    }){
        return Ok(advance_rejected(e.code,
            if e.category==ErrorCategory::Conflict{409}else{422}));
    }
    let saved=encode(&deal)?;
    let prepared=match super::sales_activities_link::prepare(
        &before,&deal,tenant,actor,trace,now_nanos()).await{
        Ok(x)=>x,Err(e)=>return Ok(advance_rejected(e.code,
            if e.category==ErrorCategory::Conflict{409}else{422})),
    };
    let update=bind(db,"UPDATE crm_sales_deal_records SET state=?,version=?,idem_key=?,idem_digest=?,request_id=?,last_action='sales_deals.advance' WHERE tenant=? AND id=? AND version=?",
        &[JsValue::from_str(&saved),JsValue::from_f64(deal.version() as f64),
          JsValue::from_str(key),JsValue::from_str(&digest),JsValue::from_str(trace),
          JsValue::from_str(tenant),JsValue::from_str(id),
          JsValue::from_f64(input.expected_version as f64)])?;
    let mut writes=vec![update];
    if let Some(task)=&prepared.task{
        let account_id=match deal.account(){
            Some(x)=>&x.resource_id,
            None=>return Ok(advance_rejected("SALES_LINK_ACCOUNT_REQUIRED",422)),
        };
        let task_state=String::from_utf8(crm_activities::encode_task_state(task)
            .map_err(|e|worker::Error::RustError(e.code))?)
            .map_err(|e|worker::Error::RustError(e.to_string()))?;
        let task_stmt=bind(db,"INSERT INTO crm_activity_task_records(tenant,id,account_id,state,version,idem_key,idem_digest,request_id) VALUES(?,?,?,?,?,?,?,?)",
            &[JsValue::from_str(tenant),JsValue::from_str(task.task_id().as_str()),
              JsValue::from_str(account_id),JsValue::from_str(&task_state),
              JsValue::from_f64(task.version() as f64),
              JsValue::from_str(&prepared.delivery_id),
              JsValue::from_str(&prepared.event_id),JsValue::from_str(trace)])?;
        writes.push(task_stmt);
    }
    let link_stmt=bind(db,"INSERT INTO crm_sales_link_delivery_records(tenant,delivery_id,event_id,deal_id,deal_version,task_id,disposition,state_json,request_id) VALUES(?,?,?,?,?,?,?,?,?)",
        &[JsValue::from_str(tenant),
          JsValue::from_str(&prepared.delivery_id),JsValue::from_str(&prepared.event_id),
          JsValue::from_str(id),JsValue::from_f64(deal.version() as f64),
          prepared.task.as_ref().map_or(JsValue::NULL,|task|
              JsValue::from_str(task.task_id().as_str())),
          JsValue::from_str(if prepared.task.is_some(){"applied"}else{"ignored"}),
          JsValue::from_str(&prepared.state_json),JsValue::from_str(trace)])?;
    writes.push(link_stmt);
    if db.batch(writes).await.is_err(){
        return Ok(advance_rejected("SALES_LINK_ATOMIC_COMMIT_CONFLICT",409));
    }
    let link=Some(LinkOutcome{
        delivery_id:prepared.delivery_id,
        applied:prepared.task.is_some(),
        task_id:prepared.task.as_ref().map(|x|x.task_id().as_str().to_owned()),
    });
    Ok(Ok(AdvanceSaved{
        saved:SalesSaved{deal,replayed:false},link,
    }))
}
async fn advance(mut req:Request,db:&D1Database,tenant:&str,id:&str,actor:&str,
    trace:&str)->Result<Response>{
    let Some(k)=key(&req)? else{return error("IDEMPOTENCY_KEY_REQUIRED",422)};
    let input=match serde_json::from_str::<StageInput>(&req.text().await?){
        Ok(x)=>x,Err(_)=>return error("INVALID_INPUT",422),
    };
    match advance_typed(input,&k,db,tenant,id,actor,trace).await?{
        Ok(outcome)=>response(json!({
            "item":view(&outcome.saved.deal),"replayed":outcome.saved.replayed,
            "link":outcome.link.as_ref().map(|link|json!({
                "delivery_id":link.delivery_id,
                "disposition":if link.applied{"applied"}else{"ignored"},
                "task_id":link.task_id,
            }))
        }),200),
        Err(f)=>error(&f.code,f.status),
    }
}

/// Filter BEFORE LIMIT so filtered pagination never drops matching Deals.
/// State is owned by crm-sales' exact persisted DealStateV1 JSON codec; no
/// alternate status or pipeline projection table is added.
pub(super) async fn list_page_typed(
    db:&D1Database,tenant:&str,after:&str,size:usize,
    pipeline:Option<&str>,status:Option<DealStatus>,
)->Result<(Vec<Deal>,Option<String>)>{
    let mut args=vec![JsValue::from_str(tenant),JsValue::from_str(after)];
    let mut sql=String::from(
        "SELECT id,state FROM crm_sales_deal_records WHERE tenant=? AND id>?");
    if let Some(pipeline)=pipeline{
        sql.push_str(" AND json_extract(state,'$.stage.pipeline_id')=?");
        args.push(JsValue::from_str(pipeline));
    }
    if let Some(status)=status{
        sql.push_str(" AND json_extract(state,'$.status')=?");
        args.push(JsValue::from_str(match status{
            DealStatus::Open=>"open",DealStatus::Won=>"won",DealStatus::Lost=>"lost",
        }));
    }
    sql.push_str(" ORDER BY id LIMIT ?");
    args.push(JsValue::from_f64((size+1) as f64));
    let mut rows=bind(db,&sql,&args)?.all().await?.results::<PagedRow>()?;
    let more=rows.len()>size;
    rows.truncate(size);
    let next=if more{rows.last().map(|x|x.id.clone())}else{None};
    let deals=rows.iter().map(|x|decode(&x.state)).collect::<Result<Vec<_>>>()?;
    Ok((deals,next))
}
async fn list(db:&D1Database,tenant:&str)->Result<Response>{
    let rows=bind(db,"SELECT state FROM crm_sales_deal_records WHERE tenant=? ORDER BY id LIMIT 300",
        &[JsValue::from_str(tenant)])?.all().await?.results::<Row>()?;
    let items=rows.iter().map(|r|decode(&r.state).map(|deal|view(&deal)))
        .collect::<Result<Vec<_>>>()?;
    response(json!({"items":items}),200)
}
pub(super) async fn route(req:Request,db:D1Database,tenant:&str,actor:&str,trace:&str)->Result<Response>{
    let path=req.path();let method=req.method();
    if path=="/api/sales-deals" {
        return match method {
            Method::Get=>list(&db,tenant).await,
            Method::Post=>create(req,&db,tenant,actor,trace).await,
            _=>error("NOT_FOUND",404),
        };
    }
    let Some(id)=path.strip_prefix("/api/sales-deals/") else{return error("NOT_FOUND",404)};
    if let Some(id)=id.strip_suffix("/advance") {
        if id.is_empty()||id.contains('/')||method!=Method::Post{return error("NOT_FOUND",404);}
        return advance(req,&db,tenant,id,actor,trace).await;
    }
    if id.is_empty()||id.contains('/'){return error("NOT_FOUND",404);}
    match method {
        Method::Get=>match load(&db,tenant,id).await?{
            Some(item)=>response(json!({"item":view(&item)}),200),
            None=>error("NOT_FOUND",404),
        },
        Method::Patch=>update(req,&db,tenant,id,trace).await,
        _=>error("NOT_FOUND",404),
    }
}
