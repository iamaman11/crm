//! Native original crm-consents authority, NOT a self-attested consent API.
//! Only authenticated *operator suppression* (Deny) is currently accepted.
//! No user-controlled evidence_ref, grant or source is ever trusted. Operator
//! action evidence proves only who recorded the block, NOT subject consent.
//! Future Grant ingress requires a separately verified authoritative source.
use crm_consents::{
    ConsentAuthorization, ConsentAuthorizationId, ContactPointReference,
    PartyReference, PurposeCode, JurisdictionCode, LegalBasisCode, SourceCode,
    EvidenceReference, ConsentEffect, ConsentAuthorizationStatus,
    CommunicationChannel, CommunicationAuthorizationReason,
    CreateConsentAuthorization, EvaluateCommunicationAuthorization,
    evaluate_communication_authorization,
    encode_consent_authorization_state, decode_consent_authorization_state,
};
use serde::{Deserialize,Serialize};
use serde_json::{Value,json};
use sha2::{Digest,Sha256};
use wasm_bindgen::JsValue;
use worker::{D1Database,Method,Request,Response,Result};
use super::{bind,error,now_nanos,response};

#[derive(Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
enum ChannelInput { Email, Phone, Sms, Postal, Messaging, Push }
impl From<ChannelInput> for CommunicationChannel {
    fn from(x:ChannelInput)->Self {
        match x {ChannelInput::Email=>Self::Email,ChannelInput::Phone=>Self::Phone,
            ChannelInput::Sms=>Self::Sms,ChannelInput::Postal=>Self::Postal,
            ChannelInput::Messaging=>Self::Messaging,ChannelInput::Push=>Self::Push}
    }
}
#[derive(Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct ScopeInput {
    party_id:String,contact_point_id:Option<String>,
    purpose:String,channel:ChannelInput,
}
#[derive(Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct SuppressionInput {
    id:String,party_id:String,contact_point_id:Option<String>,
    purpose:String,channel:ChannelInput,jurisdiction:String,
}
#[derive(Deserialize)]
struct Row {state:String}
#[derive(Deserialize)]
struct Receipt {digest:String,state:String}
fn hash<T:Serialize>(op:&str,id:&str,payload:&T)->Result<String>{
    let bytes=serde_json::to_vec(&json!([op,id,payload]))
        .map_err(|e|worker::Error::RustError(e.to_string()))?;
    Ok(format!("{:x}",Sha256::digest(bytes)))
}
fn state(item:&ConsentAuthorization)->Result<String>{
    String::from_utf8(encode_consent_authorization_state(item)
        .map_err(|e|worker::Error::RustError(e.code))?)
        .map_err(|e|worker::Error::RustError(e.to_string()))
}
fn decode(value:&str)->Result<ConsentAuthorization>{
    decode_consent_authorization_state(value.as_bytes())
        .map_err(|e|worker::Error::RustError(e.code))
}
fn channel_name(x:CommunicationChannel)->&'static str {
    match x {CommunicationChannel::Email=>"email",CommunicationChannel::Phone=>"phone",
        CommunicationChannel::Sms=>"sms",CommunicationChannel::Postal=>"postal",
        CommunicationChannel::Messaging=>"messaging",CommunicationChannel::Push=>"push"}
}
fn view(v:&ConsentAuthorization)->Value {
    json!({
        "id":v.authorization_id().as_str(),
        "party_id":v.party_ref().as_str(),
        "contact_point_id":v.contact_point_ref().map(|x|x.as_str()),
        "purpose":v.purpose().as_str(),
        "channel":channel_name(v.channel()),
        "effect":match v.effect(){ConsentEffect::Grant=>"grant",ConsentEffect::Deny=>"deny"},
        "status":match v.status(){ConsentAuthorizationStatus::Active=>"active",
            ConsentAuthorizationStatus::Withdrawn=>"withdrawn"},
        "jurisdiction":v.jurisdiction().as_str(),
        "legal_basis":v.legal_basis().as_str(),
        "source":v.source().as_str(),
        "evidence_ref":v.evidence_ref().as_str(),
        "version":v.version(),
    })
}
fn key(req:&Request)->Result<Option<String>>{
    Ok(req.headers().get("idempotency-key")?
        .filter(|k|(8..=120).contains(&k.len())))
}
async fn receipt(db:&D1Database,tenant:&str,key:&str)->Result<Option<Receipt>>{
    bind(db,"SELECT digest,state FROM crm_consent_receipts WHERE tenant=? AND key=?",
        &[JsValue::from_str(tenant),JsValue::from_str(key)])?
        .first::<Receipt>(None).await
}
async fn party_exists(db:&D1Database,tenant:&str,id:&str)->Result<bool>{
    Ok(bind(db,"SELECT id FROM crm_party_records WHERE tenant=? AND id=?",
        &[JsValue::from_str(tenant),JsValue::from_str(id)])?
        .first::<Value>(None).await?.is_some())
}
async fn contact_belongs(db:&D1Database,tenant:&str,party:&str,contact:&str)->Result<bool>{
    Ok(bind(db,"SELECT id FROM crm_contact_point_records WHERE tenant=? AND party_id=? AND id=?",
        &[JsValue::from_str(tenant),JsValue::from_str(party),
          JsValue::from_str(contact)])?.first::<Value>(None).await?.is_some())
}
async fn all(db:&D1Database,tenant:&str)->Result<Vec<ConsentAuthorization>>{
    // Evaluator never silently drops records: an incomplete view must not grant.
    let rows=bind(db,"SELECT state FROM crm_consent_records WHERE tenant=? ORDER BY id LIMIT 501",
        &[JsValue::from_str(tenant)])?.all().await?.results::<Row>()?;
    if rows.len()>500 {
        return Err(worker::Error::RustError("CONSENTS_EVALUATION_CAP_REACHED".into()));
    }
    rows.iter().map(|row|decode(&row.state)).collect()
}
async fn suppress(mut req:Request,db:&D1Database,tenant:&str,actor:&str,role:&str,trace:&str)
    ->Result<Response>{
    if role!="admin" {return error("FORBIDDEN",403);}
    let Some(k)=key(&req)? else{return error("IDEMPOTENCY_KEY_REQUIRED",422)};
    let input=match serde_json::from_str::<SuppressionInput>(&req.text().await?){
        Ok(x)=>x,Err(_)=>return error("INVALID_INPUT",422),
    };
    let digest=hash("operator-suppress",&input.id,&input)?;
    if let Some(p)=receipt(db,tenant,&k).await?{
        if p.digest!=digest{return error("IDEMPOTENCY_CONFLICT",409);}
        return response(json!({"item":view(&decode(&p.state)?),"replayed":true}),200);
    }
    let id=match ConsentAuthorizationId::try_new(&input.id){
        Ok(x)=>x,Err(e)=>return error(&e.code,422),
    };
    let party=match PartyReference::try_new(&input.party_id){
        Ok(x)=>x,Err(e)=>return error(&e.code,422),
    };
    if !party_exists(db,tenant,party.as_str()).await?{
        return error("CONSENTS_PARTY_NOT_FOUND",422);
    }
    let contact=match input.contact_point_id {
        Some(id)=>{
            if !contact_belongs(db,tenant,party.as_str(),&id).await? {
                return error("CONSENTS_CONTACT_POINT_PARTY_MISMATCH",422);
            }
            match ContactPointReference::try_new(id){
                Ok(x)=>Some(x),Err(e)=>return error(&e.code,422),
            }
        },
        None=>None,
    };
    let purpose=match PurposeCode::try_new(input.purpose){
        Ok(x)=>x,Err(e)=>return error(&e.code,422),
    };
    let jurisdiction=match JurisdictionCode::try_new(input.jurisdiction){
        Ok(x)=>x,Err(e)=>return error(&e.code,422),
    };
    let legal_basis=LegalBasisCode::try_new("operational.suppression")
        .map_err(|e|worker::Error::RustError(e.code))?;
    let source=SourceCode::try_new("crm.access.operator")
        .map_err(|e|worker::Error::RustError(e.code))?;
    let evidence_ref=EvidenceReference::try_new(format!("operator-action:{trace}"))
        .map_err(|e|worker::Error::RustError(e.code))?;
    let now=now_nanos();
    let item=match ConsentAuthorization::create(CreateConsentAuthorization{
        authorization_id:id,party_ref:party.clone(),contact_point_ref:contact,
        purpose,channel:input.channel.into(),effect:ConsentEffect::Deny,
        legal_basis,jurisdiction,source,evidence_ref,
        effective_from_unix_nanos:now,expires_at_unix_nanos:None,
        occurred_at_unix_nanos:now,
    }){
        Ok(x)=>x,Err(e)=>return error(&e.code,422),
    };
    let saved=state(&item)?;
    let r=bind(db,"INSERT INTO crm_consent_records(tenant,id,party_id,state,version,idem_key,idem_digest,request_id,operator_identity) VALUES(?,?,?,?,?,?,?,?,?)",
        &[JsValue::from_str(tenant),JsValue::from_str(item.authorization_id().as_str()),
          JsValue::from_str(party.as_str()),JsValue::from_str(&saved),
          JsValue::from_f64(item.version() as f64),JsValue::from_str(&k),
          JsValue::from_str(&digest),JsValue::from_str(trace),
          JsValue::from_str(actor)])?.run().await;
    if r.is_err(){return error("CONSENTS_SUPPRESSION_CREATE_CONFLICT",409);}
    response(json!({"item":view(&item),"operator_action":true,
        "subject_consent_evidence":false,"replayed":false}),201)
}
async fn evaluate(mut req:Request,db:&D1Database,tenant:&str)->Result<Response>{
    let input=match serde_json::from_str::<ScopeInput>(&req.text().await?){
        Ok(x)=>x,Err(_)=>return error("INVALID_INPUT",422),
    };
    let party=match PartyReference::try_new(&input.party_id){
        Ok(x)=>x,Err(e)=>return error(&e.code,422),
    };
    if !party_exists(db,tenant,party.as_str()).await?{
        return error("CONSENTS_PARTY_NOT_FOUND",422);
    }
    let contact=match input.contact_point_id{
        Some(id)=>{
            if !contact_belongs(db,tenant,party.as_str(),&id).await?{
                return error("CONSENTS_CONTACT_POINT_PARTY_MISMATCH",422);
            }
            match ContactPointReference::try_new(id){
                Ok(x)=>Some(x),Err(e)=>return error(&e.code,422),
            }
        },
        None=>None,
    };
    let purpose=match PurposeCode::try_new(input.purpose){
        Ok(x)=>x,Err(e)=>return error(&e.code,422),
    };
    let scope=EvaluateCommunicationAuthorization{
        party_ref:party,contact_point_ref:contact,purpose,
        channel:input.channel.into(),evaluation_time_unix_nanos:now_nanos(),
    };
    let rows=all(db,tenant).await?;
    let decision=evaluate_communication_authorization(&scope,rows.iter())
        .map_err(|e|worker::Error::RustError(e.code))?;
    response(json!({
        "allowed":decision.allowed,
        "reason":match decision.reason {
            CommunicationAuthorizationReason::ActiveGrant=>"active_grant",
            CommunicationAuthorizationReason::ActiveDeny=>"active_deny",
            CommunicationAuthorizationReason::Withdrawn=>"withdrawn",
            CommunicationAuthorizationReason::NoApplicableGrant=>"no_applicable_grant",
        },
        "determining_authorization_ids":decision.determining_authorization_ids
            .iter().map(|id|id.as_str()).collect::<Vec<_>>(),
        "grant_ingress":"NOT_CONNECTED",
        "owner":"original-crm-consents",
    }),200)
}
async fn list(db:&D1Database,tenant:&str)->Result<Response>{
    let items=all(db,tenant).await?.iter().map(view).collect::<Vec<_>>();
    response(json!({"items":items,"grant_ingress":"NOT_CONNECTED"}),200)
}
pub(super) async fn route(req:Request,db:D1Database,tenant:&str,actor:&str,role:&str,trace:&str)
    ->Result<Response>{
    let path=req.path();let method=req.method();
    match (path.as_str(),method){
        ("/api/consents",Method::Get)=>list(&db,tenant).await,
        ("/api/consents/evaluate",Method::Post)=>evaluate(req,&db,tenant).await,
        ("/api/consents/suppress",Method::Post)=>suppress(req,&db,tenant,actor,role,trace).await,
        // No HTTP grant/withdrawal until verifiable subject evidence is wired.
        _=>error("NOT_FOUND",404),
    }
}
