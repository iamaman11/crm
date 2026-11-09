//! Original customer-enrichment immutable provider/mapping definitions.
//! Configuration only: no provider network, credentials, responses or Party updates.
use crm_customer_enrichment::{
    ProviderProfileDraft, ProviderProfileVersion, MappingDraft, MappingVersion,
    TargetField, RawPayloadPolicy, MappingNormalization,
    encode_provider_profile_version_state, decode_provider_profile_version_state,
    encode_mapping_version_state, decode_mapping_version_state,
};
use serde::{Deserialize,Serialize};
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use wasm_bindgen::JsValue;
use worker::{D1Database,Method,Request,Response,Result};
use super::{bind,error,response};

const PROFILES:&str="crm_enrichment_profiles";
const MAPPINGS:&str="crm_enrichment_mappings";

#[derive(Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileInput {
    provider_key:String,
    adapter_kind:String,
    adapter_contract_version:String,
    purpose_codes:Vec<String>,
    license_id:String,
    permitted_use_class:String,
    residency_region:String,
    retention_days:u32,
    raw_payload_policy:RawPayloadPolicy,
    credential_handle_aliases:Vec<String>,
    effective_at_unix_ms:u64,
    expires_at_unix_ms:Option<u64>,
}
#[derive(Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct MappingInput {
    mapping_key:String,
    provider_profile_version_id:String,
    provider_response_field_path:String,
    maximum_suggestions_per_response:u32,
    confidence_required:bool,
}
#[derive(Deserialize)]
struct StateRow {state:String}
#[derive(Deserialize)]
struct Receipt {kind:String,digest:String,state:String}

fn profile_view(p:&ProviderProfileVersion)->Value{
    let mut value=serde_json::to_value(p).expect("original profile can serialize");
    value["id"]=json!(p.version_id().as_str());
    value["execution"]=json!("NOT_CONNECTED");
    value
}
fn mapping_view(m:&MappingVersion)->Value{
    let mut value=serde_json::to_value(m).expect("original mapping can serialize");
    value["id"]=json!(m.version_id().as_str());
    value["execution"]=json!("NOT_CONNECTED");
    value
}
fn encode_profile(p:&ProviderProfileVersion)->Result<String>{
    String::from_utf8(encode_provider_profile_version_state(p)
        .map_err(|e|worker::Error::RustError(e.code))?)
        .map_err(|e|worker::Error::RustError(e.to_string()))
}
fn decode_profile(s:&str)->Result<ProviderProfileVersion>{
    decode_provider_profile_version_state(s.as_bytes())
        .map_err(|e|worker::Error::RustError(e.code))
}
fn encode_mapping(m:&MappingVersion)->Result<String>{
    String::from_utf8(encode_mapping_version_state(m)
        .map_err(|e|worker::Error::RustError(e.code))?)
        .map_err(|e|worker::Error::RustError(e.to_string()))
}
fn decode_mapping(s:&str)->Result<MappingVersion>{
    decode_mapping_version_state(s.as_bytes())
        .map_err(|e|worker::Error::RustError(e.code))
}
fn digest<T:Serialize>(kind:&str,input:&T)->String{
    format!("{:x}",Sha256::digest(
        serde_json::to_vec(&json!([kind,input])).expect("typed request serialization")
    ))
}
fn key(req:&Request)->Result<Option<String>>{
    Ok(req.headers().get("idempotency-key")?
        .filter(|k|(8..=120).contains(&k.len())))
}
async fn receipt(db:&D1Database,tenant:&str,key:&str)->Result<Option<Receipt>>{
    bind(db,"SELECT kind,digest,state FROM crm_enrichment_receipts WHERE tenant=? AND key=?",
        &[JsValue::from_str(tenant),JsValue::from_str(key)])?
        .first::<Receipt>(None).await
}
async fn state(db:&D1Database,table:&str,tenant:&str,id:&str)->Result<Option<String>>{
    // The table comes only from the fixed module constants, never the request.
    let sql=format!("SELECT state FROM {table} WHERE tenant=? AND id=?");
    Ok(bind(db,&sql,&[JsValue::from_str(tenant),JsValue::from_str(id)])?
        .first::<StateRow>(None).await?.map(|r|r.state))
}
async fn list(db:&D1Database,table:&str,tenant:&str,kind:&str)->Result<Response>{
    let sql=format!("SELECT state FROM {table} WHERE tenant=? ORDER BY id LIMIT 101");
    let rows=bind(db,&sql,&[JsValue::from_str(tenant)])?
        .all().await?.results::<StateRow>()?;
    if rows.len()>100{return error("ENRICHMENT_DEFINITION_LIST_LIMIT_REACHED",503);}
    let items=rows.iter().map(|row|match kind{
        "profile"=>decode_profile(&row.state).map(|p|profile_view(&p)),
        _=>decode_mapping(&row.state).map(|m|mapping_view(&m)),
    }).collect::<Result<Vec<_>>>()?;
    response(json!({"items":items,"execution":"NOT_CONNECTED",
        "scope":"CONFIGURATION_ONLY"}),200)
}
async fn publish_profile(mut req:Request,db:&D1Database,tenant:&str,
    actor:&str,trace:&str)->Result<Response>{
    let Some(k)=key(&req)? else{return error("IDEMPOTENCY_KEY_REQUIRED",422)};
    let input=match req.json::<ProfileInput>().await{
        Ok(input)=>input,Err(_)=>return error("INVALID_INPUT",422),
    };
    let hash=digest("profile",&input);
    if let Some(r)=receipt(db,tenant,&k).await?{
        if r.kind!="profile"||r.digest!=hash{return error("IDEMPOTENCY_CONFLICT",409);}
        let profile=decode_profile(&r.state)?;
        return response(json!({"item":profile_view(&profile),"replayed":true}),200);
    }
    let draft=ProviderProfileDraft{
        provider_key:input.provider_key,
        adapter_kind:input.adapter_kind,
        adapter_contract_version:input.adapter_contract_version,
        supported_target_fields:vec![TargetField::PartyDisplayName],
        purpose_codes:input.purpose_codes,
        license_id:input.license_id,
        permitted_use_class:input.permitted_use_class,
        residency_region:input.residency_region,
        retention_days:input.retention_days,
        raw_payload_policy:input.raw_payload_policy,
        credential_handle_aliases:input.credential_handle_aliases,
        effective_at_unix_ms:input.effective_at_unix_ms,
        expires_at_unix_ms:input.expires_at_unix_ms,
    };
    let profile=match ProviderProfileVersion::publish(draft){
        Ok(x)=>x,Err(e)=>return error(&e.code,422),
    };
    let saved=encode_profile(&profile)?;
    let inserted=bind(db,"INSERT INTO crm_enrichment_profiles (tenant,id,state,idem_key,idem_digest,request_id,created_by) VALUES(?,?,?,?,?,?,?)",
        &[JsValue::from_str(tenant),
          JsValue::from_str(profile.version_id().as_str()),
          JsValue::from_str(&saved),JsValue::from_str(&k),
          JsValue::from_str(&hash),JsValue::from_str(trace),
          JsValue::from_str(actor)])?.run().await;
    if inserted.is_err(){
        // A race on the same key may have committed while we were inserting.
        if let Some(r)=receipt(db,tenant,&k).await?{
            if r.kind=="profile"&&r.digest==hash{
                return response(json!({"item":profile_view(&decode_profile(&r.state)?),
                    "replayed":true}),200);
            }
        }
        return error("ENRICHMENT_PROFILE_CREATE_CONFLICT",409);
    }
    response(json!({"item":profile_view(&profile),"replayed":false}),201)
}
async fn publish_mapping(mut req:Request,db:&D1Database,tenant:&str,
    actor:&str,trace:&str)->Result<Response>{
    let Some(k)=key(&req)? else{return error("IDEMPOTENCY_KEY_REQUIRED",422)};
    let input=match req.json::<MappingInput>().await{
        Ok(input)=>input,Err(_)=>return error("INVALID_INPUT",422),
    };
    let hash=digest("mapping",&input);
    if let Some(r)=receipt(db,tenant,&k).await?{
        if r.kind!="mapping"||r.digest!=hash{return error("IDEMPOTENCY_CONFLICT",409);}
        return response(json!({"item":mapping_view(&decode_mapping(&r.state)?),
            "replayed":true}),200);
    }
    let Some(source)=state(db,PROFILES,tenant,&input.provider_profile_version_id).await?
        else{return error("ENRICHMENT_PROVIDER_PROFILE_NOT_FOUND",404)};
    let profile=decode_profile(&source)?;
    // The only allowed target for this first usable native slice. The owner
    // domain validates the target-normalization combination and immutable ID.
    if !profile.supported_target_fields().contains(&TargetField::PartyDisplayName){
        return error("ENRICHMENT_UNSUPPORTED_TARGET",422);
    }
    let mapping=match MappingVersion::publish(MappingDraft{
        mapping_key:input.mapping_key,
        provider_profile_version_id:profile.version_id().clone(),
        provider_response_field_path:input.provider_response_field_path,
        target_field:TargetField::PartyDisplayName,
        normalization:MappingNormalization::CanonicalPartyDisplayNameV1,
        maximum_suggestions_per_response:input.maximum_suggestions_per_response,
        confidence_required:input.confidence_required,
    }){
        Ok(x)=>x,Err(e)=>return error(&e.code,422),
    };
    let saved=encode_mapping(&mapping)?;
    let inserted=bind(db,"INSERT INTO crm_enrichment_mappings (tenant,id,provider_profile_id,state,idem_key,idem_digest,request_id,created_by) VALUES(?,?,?,?,?,?,?,?)",
        &[JsValue::from_str(tenant),
          JsValue::from_str(mapping.version_id().as_str()),
          JsValue::from_str(&input.provider_profile_version_id),
          JsValue::from_str(&saved),JsValue::from_str(&k),
          JsValue::from_str(&hash),JsValue::from_str(trace),
          JsValue::from_str(actor)])?.run().await;
    if inserted.is_err(){
        if let Some(r)=receipt(db,tenant,&k).await?{
            if r.kind=="mapping"&&r.digest==hash{
                return response(json!({"item":mapping_view(&decode_mapping(&r.state)?),
                    "replayed":true}),200);
            }
        }
        return error("ENRICHMENT_MAPPING_CREATE_CONFLICT",409);
    }
    response(json!({"item":mapping_view(&mapping),"replayed":false}),201)
}
pub(super) async fn route(req:Request,db:D1Database,tenant:&str,
    actor:&str,role:&str,trace:&str)->Result<Response>{
    let path=req.path();let method=req.method();
    if path=="/api/customer-enrichment/profiles"{
        return match method{
            Method::Get=>list(&db,PROFILES,tenant,"profile").await,
            Method::Post if role=="admin"=>publish_profile(req,&db,tenant,actor,trace).await,
            Method::Post=>error("FORBIDDEN",403),
            _=>error("NOT_FOUND",404),
        };
    }
    if path=="/api/customer-enrichment/mappings"{
        return match method{
            Method::Get=>list(&db,MAPPINGS,tenant,"mapping").await,
            Method::Post if role=="admin"=>publish_mapping(req,&db,tenant,actor,trace).await,
            Method::Post=>error("FORBIDDEN",403),
            _=>error("NOT_FOUND",404),
        };
    }
    for (prefix,table) in [("/api/customer-enrichment/profiles/",PROFILES),
        ("/api/customer-enrichment/mappings/",MAPPINGS)]{
        if let Some(id)=path.strip_prefix(prefix){
            if id.is_empty()||id.contains('/')||method!=Method::Get{
                return error("NOT_FOUND",404);
            }
            return match state(&db,table,tenant,id).await?{
                Some(s) if table==PROFILES=>response(json!({
                    "item":profile_view(&decode_profile(&s)?)}),200),
                Some(s)=>response(json!({"item":mapping_view(&decode_mapping(&s)?)}),200),
                None=>error("NOT_FOUND",404),
            };
        }
    }
    error("NOT_FOUND",404)
}
