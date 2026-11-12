//! Bounded, synchronous Party CSV download using the ORIGINAL
//! crm-customer-data-operations scope/profile/specification/CSV functions.
//! One signed Access tenant, one original Party D1 owner; NO shadow export
//! business rules, persisted export job, file store or background service.
use crm_customer_data_operations::{
    PartyExportField, PartyExportKindFilter, PartyExportProfile,
    PartyExportScope, PartyExportSpecification, canonical_party_export_csv_record,
    party_export_hex, party_export_sha256,
    PARTY_EXPORT_CSV_MEDIA_TYPE,
};
use crm_parties::{Party, PartyKind, PartyId, CreateParty, decode_party_state};
use crm_customer_data_operations::{
    parse_import_source, ImportParserProfile, party_export_hash_parts,
    ImportJobId, PartyImportMapping, ImportRow, CreateImportRow,
    PreparedPartyRow, PartyImportKind,
};
use std::collections::BTreeSet;
use serde::Deserialize;
use wasm_bindgen::JsValue;
use worker::{D1Database, Method, Request, Response, Result};
use super::{bind, error, parties};

pub(super) const LIMIT: usize = 100;
const RETENTION: &str = "crm.dev.transient_csv_download.v1";

#[derive(Deserialize)]
struct Row { state: String }

pub(super) struct Export {
    pub(super) count: usize,
    bytes: Vec<u8>,
    pub(super) sha256: String,
    pub(super) specification: PartyExportSpecification,
}

fn filter(req: &Request) -> Result<Option<PartyExportKindFilter>> {
    let mut value = None;
    for (key, candidate) in req.url()?.query_pairs() {
        if key != "kind" || value.is_some() {
            return Err(worker::Error::RustError("EXPORT_QUERY_INVALID".into()));
        }
        value = Some(candidate.to_string());
    }
    match value.as_deref().unwrap_or("all") {
        "all" => Ok(None),
        "person" => Ok(Some(PartyExportKindFilter::Person)),
        "organization" => Ok(Some(PartyExportKindFilter::Organization)),
        _ => Err(worker::Error::RustError("EXPORT_KIND_INVALID".into())),
    }
}

fn matches_kind(p: &Party, kind: Option<PartyExportKindFilter>) -> bool {
    matches!((kind, p.kind()),
        (None, _) |
        (Some(PartyExportKindFilter::Person), PartyKind::Person) |
        (Some(PartyExportKindFilter::Organization), PartyKind::Organization))
}

pub(super) async fn assemble(db: &D1Database, tenant: &str,
    kind: Option<PartyExportKindFilter>) -> Result<Option<Export>> {
    let scope = PartyExportScope::try_new(kind, LIMIT as u32)
        .map_err(|e| worker::Error::RustError(e.code))?;
    let profile = PartyExportProfile::v1(vec![
        PartyExportField::PartyId,
        PartyExportField::Kind,
        PartyExportField::DisplayName,
        PartyExportField::ResourceVersion,
    ], RETENTION).map_err(|e| worker::Error::RustError(e.code))?;
    let specification = PartyExportSpecification::try_new(scope,profile)
        .map_err(|e| worker::Error::RustError(e.code))?;

    // LIMIT+1 before filter: never conceal an incomplete tenant-scoped export.
    let rows = bind(db,
        "SELECT state FROM crm_party_records WHERE tenant = ? ORDER BY id LIMIT 101",
        &[JsValue::from_str(tenant)])?
        .all().await?.results::<Row>()?;
    if rows.len() > LIMIT { return Ok(None); }
    let mut bytes = canonical_party_export_csv_record(
        &specification.profile().fields().iter()
            .map(|field| field.canonical_name().to_string()).collect::<Vec<_>>())
        .map_err(|e| worker::Error::RustError(e.code))?;
    let mut count=0;
    for row in rows {
        let party = decode_party_state(row.state.as_bytes())
            .map_err(|e| worker::Error::RustError(e.code))?;
        if !matches_kind(&party, specification.scope().kind_filter()) {continue;}
        let cells = specification.profile().fields().iter().map(|field| match field {
            PartyExportField::PartyId => party.party_id().as_str().to_owned(),
            PartyExportField::Kind => match party.kind() {
                PartyKind::Person => "person".to_owned(),
                PartyKind::Organization => "organization".to_owned(),
            },
            PartyExportField::DisplayName => party.display_name().to_owned(),
            PartyExportField::ResourceVersion => party.version().to_string(),
        }).collect::<Vec<_>>();
        bytes.extend(canonical_party_export_csv_record(&cells)
            .map_err(|e| worker::Error::RustError(e.code))?);
        count+=1;
    }
    let sha256 = party_export_hex(&party_export_sha256(&bytes));
    Ok(Some(Export {count,bytes,sha256,specification}))
}

// Original parser, PartyImportMapping and ImportRow own CSV interpretation,
// provenance and stable target identity. External IDs NEVER become CRM Party IDs.
const IMPORT_MAX_ROWS:usize=20;
const IMPORT_MAX_BYTES:usize=16*1024;
const IMPORT_FIELDS:[&str;4]=["party_id","kind","display_name","resource_version"];
type ImportCommand=(String,PartyKind,String,String); // target ID,kind,name,receipt key
struct ImportPlan {
    commands:Vec<ImportCommand>,
    hash:String,
}
fn parse_import(bytes:&[u8], tenant:&str)->std::result::Result<ImportPlan,&'static str>{
    if bytes.is_empty()||bytes.len()>IMPORT_MAX_BYTES{return Err("IMPORT_FILE_SIZE_INVALID");}
    let profile=ImportParserProfile::csv_v1(b',',b'"')
        .map_err(|_|"IMPORT_PARSER_PROFILE_INVALID")?;
    let parsed=parse_import_source(bytes,&profile)
        .map_err(|_|"IMPORT_CSV_INVALID")?;
    if parsed.headers().iter().map(String::as_str).collect::<Vec<_>>()!=IMPORT_FIELDS{
        return Err("IMPORT_HEADER_INVALID");
    }
    if parsed.rows().is_empty()||parsed.rows().len()>IMPORT_MAX_ROWS{return Err("IMPORT_ROW_LIMIT_REACHED");}
    let mapping=PartyImportMapping::try_new(
        None,"kind","display_name",Some("party_id".to_owned()),None)
        .map_err(|_|"IMPORT_MAPPING_INVALID")?;
    let mapping_version=mapping.version_id()
        .map_err(|_|"IMPORT_MAPPING_INVALID")?.as_str().to_owned();
    let hash=party_export_hex(&party_export_sha256(bytes));
    let identity=party_export_hash_parts(b"crm.cloudflare.csv-import-create-only/v1",
        &[tenant.as_bytes(),bytes,mapping_version.as_bytes()]);
    let job_id=ImportJobId::try_new(
        format!("cdo-import-{}",party_export_hex(&identity)))
        .map_err(|_|"IMPORT_JOB_ID_INVALID")?;
    let mut seen=BTreeSet::new();
    let mut commands=Vec::new();
    for source in parsed.rows(){
        let source_id=source.columns.get("party_id").ok_or("IMPORT_ROW_INVALID")?;
        let kind=source.columns.get("kind").ok_or("IMPORT_ROW_INVALID")?;
        let name=source.columns.get("display_name").ok_or("IMPORT_ROW_INVALID")?;
        let version=source.columns.get("resource_version").ok_or("IMPORT_ROW_INVALID")?;
        // Source version is provenance, not permission to mutate the target.
        if version.parse::<u64>().ok().filter(|v|*v>0).is_none(){
            return Err("IMPORT_SOURCE_VERSION_INVALID");
        }
        if source_id.is_empty()||!seen.insert(source_id.to_owned()){
            return Err("IMPORT_DUPLICATE_OR_EMPTY_SOURCE_ID");
        }
        let import_kind=match kind.as_str(){
            "person"=>PartyImportKind::Person,
            "organization"=>PartyImportKind::Organization,
            _=>return Err("IMPORT_KIND_INVALID"),
        };
        let row=ImportRow::create(CreateImportRow{
            job_id:job_id.clone(),row_position:source.row_position,
            external_row_key:None,source_external_id:Some(source_id.clone()),
            occurred_at_unix_nanos:1,
        }).map_err(|_|"IMPORT_ROW_ID_INVALID")?;
        let target=row.derived_target_party_id().map_err(|_|"IMPORT_TARGET_ID_INVALID")?;
        let prepared=PreparedPartyRow::try_new(target,import_kind,name.clone())
            .map_err(|_|"IMPORT_PARTY_INVALID")?;
        let kind=match prepared.kind(){
            PartyImportKind::Person=>PartyKind::Person,
            PartyImportKind::Organization=>PartyKind::Organization,
        };
        let checked=Party::create(CreateParty{
            party_id:PartyId::try_new(prepared.party_id().as_str().to_owned())
                .map_err(|_|"IMPORT_TARGET_ID_INVALID")?,
            kind,display_name:prepared.display_name().to_owned(),
            occurred_at_unix_nanos:1,
        }).map_err(|_|"IMPORT_PARTY_INVALID")?;
        if checked.display_name()!=name{return Err("IMPORT_NAME_NONCANONICAL");}
        commands.push((checked.party_id().as_str().to_owned(),kind,
            checked.display_name().to_owned(),row.target_idempotency_key()));
    }
    Ok(ImportPlan{commands,hash})
}

pub(super) struct ImportPreview {
    pub(super) count:usize,
    pub(super) hash:String,
    pub(super) already:usize,
    pub(super) target_ids:Vec<String>,
}
pub(super) struct ImportApplied {
    pub(super) saved:parties::ImportSaved,
    pub(super) hash:String,
}
pub(super) async fn preview_import(bytes:&[u8],db:&D1Database,tenant:&str)
    ->Result<std::result::Result<ImportPreview,parties::MutationFailure>>{
    let plan=match parse_import(bytes,tenant){
        Ok(plan)=>plan,Err(code)=>return Ok(Err(parties::MutationFailure{code:code.into(),status:422})),
    };
    let mut already=0;
    for (id,kind,name,key) in &plan.commands{
        if parties::import_receipt_matches(db,tenant,key,id,*kind,name).await?{
            already+=1;
        }else if parties::stored(db,tenant,id).await?.is_some(){
            return Ok(Err(parties::MutationFailure{code:"IMPORT_TARGET_PARTY_CONFLICT".into(),status:409}));
        }
    }
    Ok(Ok(ImportPreview{count:plan.commands.len(),hash:plan.hash,already,
        target_ids:plan.commands.into_iter().map(|r|r.0).collect()}))
}
pub(super) async fn apply_import(bytes:&[u8],expected:&str,key:&str,
    db:&D1Database,tenant:&str,trace:&str)
    ->Result<std::result::Result<ImportApplied,parties::MutationFailure>>{
    let plan=match parse_import(bytes,tenant){
        Ok(plan)=>plan,Err(code)=>return Ok(Err(parties::MutationFailure{code:code.into(),status:422})),
    };
    if expected!=plan.hash{
        return Ok(Err(parties::MutationFailure{code:"IMPORT_SOURCE_CHANGED".into(),status:409}));
    }
    Ok(parties::import_batch(db,tenant,plan.commands,key,&plan.hash,trace).await?
        .map(|saved|ImportApplied{saved,hash:plan.hash}))
}

// The only non-RPC business endpoint: bounded CSV attachment bytes.
pub(super) async fn route(req:Request,db:D1Database,tenant:&str,role:Option<&str>)->Result<Response>{
    if role!=Some("admin"){return error("FORBIDDEN",403);}
    if req.method()!=Method::Get||req.path()!="/api/customer-data-operations/parties.csv"{
        return error("NOT_FOUND",404);
    }
    let kind=match filter(&req){Ok(kind)=>kind,Err(_)=>return error("EXPORT_QUERY_INVALID",422)};
    let Some(export)=assemble(&db,tenant,kind).await? else{
        return error("EXPORT_SCOPE_LIMIT_REACHED",409);
    };
    let mut reply=Response::from_bytes(export.bytes)?;
    reply.headers_mut().set("content-type",PARTY_EXPORT_CSV_MEDIA_TYPE)?;
    reply.headers_mut().set("content-disposition","attachment; filename=\"crm-parties.csv\"")?;
    reply.headers_mut().set("cache-control","private, no-store")?;
    reply.headers_mut().set("x-crm-export-count",&export.count.to_string())?;
    reply.headers_mut().set("x-crm-export-sha256",&export.sha256)?;
    reply.headers_mut().set("x-crm-export-specification",export.specification.version_id().as_str())?;
    Ok(reply)
}
