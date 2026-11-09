//! Bounded, synchronous Party CSV download using the ORIGINAL
//! crm-customer-data-operations scope/profile/specification/CSV functions.
//! One signed Access tenant, one original Party D1 owner; NO shadow export
//! business rules, persisted export job, file store or background service.
use crm_customer_data_operations::{
    PartyExportField, PartyExportKindFilter, PartyExportProfile,
    PartyExportScope, PartyExportSpecification, canonical_party_export_csv_record,
    party_export_hex, party_export_sha256, PARTY_EXPORT_CANONICALIZATION_V1,
    PARTY_EXPORT_CSV_MEDIA_TYPE,
};
use crm_parties::{Party, PartyKind, PartyId, CreateParty, decode_party_state};
use crm_customer_data_operations::{parse_import_source, ImportParserProfile, party_export_hash_parts};
use std::collections::BTreeSet;
use serde::Deserialize;
use serde_json::json;
use wasm_bindgen::JsValue;
use worker::{D1Database, Method, Request, Response, Result};
use super::{bind, error, response, parties};

const LIMIT: usize = 100;
const RETENTION: &str = "crm.dev.transient_csv_download.v1";

#[derive(Deserialize)]
struct Row { state: String }

struct Export {
    count: usize,
    bytes: Vec<u8>,
    sha256: String,
    specification: PartyExportSpecification,
    filter_label: &'static str,
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

async fn assemble(db: &D1Database, tenant: &str,
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
    let filter_label = match kind {
        None => "all",
        Some(PartyExportKindFilter::Person) => "person",
        Some(PartyExportKindFilter::Organization) => "organization",
    };
    Ok(Some(Export {count,bytes,sha256,specification,filter_label}))
}

// Create-only CSV v1: the original source parser owns strict CSV semantics.
// The original Party aggregate owns all target validation. No client-supplied
// "validated" row list is accepted and no import job is falsely persisted.
const IMPORT_MAX_ROWS: usize=20;
const IMPORT_MAX_BYTES: usize=16*1024;
const IMPORT_FIELDS: [&str;4]=["party_id","kind","display_name","resource_version"];
fn parse_import(bytes: &[u8]) -> std::result::Result<Vec<(String, PartyKind, String)>, &'static str> {
    if bytes.len()>IMPORT_MAX_BYTES {return Err("IMPORT_FILE_TOO_LARGE");}
    let profile=ImportParserProfile::csv_v1(b',',b'"')
        .map_err(|_|"IMPORT_PARSER_PROFILE_INVALID")?;
    let parsed=parse_import_source(bytes,&profile)
        .map_err(|_|"IMPORT_CSV_INVALID")?;
    if parsed.headers().iter().map(String::as_str).collect::<Vec<_>>()!=IMPORT_FIELDS {return Err("IMPORT_HEADER_INVALID");}
    if parsed.rows().len()>IMPORT_MAX_ROWS {return Err("IMPORT_ROW_LIMIT_REACHED");}
    let mut seen=BTreeSet::new();
    let mut rows=Vec::new();
    for row in parsed.rows() {
        let id=row.columns.get("party_id").ok_or("IMPORT_ROW_INVALID")?;
        let kind=row.columns.get("kind").ok_or("IMPORT_ROW_INVALID")?;
        let name=row.columns.get("display_name").ok_or("IMPORT_ROW_INVALID")?;
        let version=row.columns.get("resource_version").ok_or("IMPORT_ROW_INVALID")?;
        if version!="1" {return Err("IMPORT_REQUIRES_NEW_PARTY_VERSION_1");}
        if !seen.insert(id.clone()) {return Err("IMPORT_DUPLICATE_ID");}
        let party_kind=match kind.as_str(){
            "person"=>PartyKind::Person,
            "organization"=>PartyKind::Organization,
            _=>return Err("IMPORT_KIND_INVALID")
        };
        let party_id=PartyId::try_new(id.clone()).map_err(|_|"IMPORT_PARTY_ID_INVALID")?;
        let checked=crm_parties::Party::create(CreateParty {
            party_id,kind:party_kind,display_name:name.clone(),occurred_at_unix_nanos:1,
        }).map_err(|_|"IMPORT_PARTY_INVALID")?;
        if checked.display_name()!=name {return Err("IMPORT_NAME_NONCANONICAL");}
        rows.push((id.clone(),party_kind,name.clone()));
    }
    Ok(rows)
}
async fn import(mut req: Request, db: D1Database, tenant:&str, trace:&str) -> Result<Response> {
    if req.method()!=Method::Post {return error("NOT_FOUND",404);}
    let preview=req.path()=="/api/customer-data-operations/import-preview";
    // Must capture headers BEFORE consuming Request.
    let expected=if preview {None} else {req.headers().get("x-crm-import-sha256")?};
    let bytes=req.bytes().await?;
    let rows=match parse_import(&bytes){
        Ok(rows)=>rows,Err(code)=>return error(code,422)
    };
    let hash=party_export_hex(&party_export_sha256(&bytes));
    if preview {
        for (id,_,_) in &rows {
            if parties::stored(&db,tenant,id).await?.is_some() {
                return error("IMPORT_PARTY_ALREADY_EXISTS",409);
            }
        }
        return response(json!({
            "count":rows.len(),"file_sha256":hash,
            "fields":IMPORT_FIELDS,"max_rows":IMPORT_MAX_ROWS,
            "mode":"CREATE_ONLY","atomic":true,
            "import_jobs":"NOT_CONNECTED","read_only":true
        }),200);
    }
    if expected.as_deref()!=Some(hash.as_str()) {
        return error("IMPORT_SOURCE_CHANGED",409);
    }
    // Stable per-tenant and per-file receipt prefix, never derived from
    // external IDs or client-provided tenancy. Replaying an applied CSV is safe.
    let source_key=party_export_hex(&party_export_hash_parts(
        b"crm-cdo-import-create-only-v1", &[tenant.as_bytes(),hash.as_bytes()]));
    parties::import_batch(&db,tenant,rows,&source_key[..32],trace).await
}

pub(super) async fn route(req: Request, db: D1Database,
    tenant: &str, role: Option<&str>, trace:&str) -> Result<Response> {
    // Export discloses an entire bounded set of personal/business Party names,
    // so unlike ordinary Party reads it is expressly admin-only.
    if role != Some("admin") {return error("FORBIDDEN",403);}
    if req.path()=="/api/customer-data-operations/import-preview" ||
        req.path()=="/api/customer-data-operations/import-apply" {
        return import(req,db,tenant,trace).await;
    }
    if req.method()!=Method::Get {return error("NOT_FOUND",404);}
    let path=req.path();
    if path!="/api/customer-data-operations/preview"
        && path!="/api/customer-data-operations/parties.csv" {
        return error("NOT_FOUND",404);
    }
    let kind = match filter(&req) {
        Ok(kind) => kind,
        Err(_) => return error("EXPORT_QUERY_INVALID",422),
    };
    let Some(export) = assemble(&db,tenant,kind).await? else {
        return error("EXPORT_SCOPE_LIMIT_REACHED",409);
    };
    if path.ends_with("/preview") {
        return response(json!({
            "count": export.count,
            "kind": export.filter_label,
            "fields": export.specification.profile().fields().iter()
                .map(|f| f.canonical_name()).collect::<Vec<_>>(),
            "specification_version": export.specification.version_id().as_str(),
            "csv_sha256": export.sha256,
            "canonicalization": PARTY_EXPORT_CANONICALIZATION_V1,
            "max_tenant_parties": LIMIT,
            "read_only": true,
            "persisted_export_job": "NOT_CONNECTED",
            "import": "NOT_CONNECTED"
        }),200);
    }
    let mut reply = Response::from_bytes(export.bytes)?;
    reply.headers_mut().set("content-type", PARTY_EXPORT_CSV_MEDIA_TYPE)?;
    reply.headers_mut().set("content-disposition",
        "attachment; filename=\"crm-parties.csv\"")?;
    reply.headers_mut().set("cache-control", "private, no-store")?;
    reply.headers_mut().set("x-crm-export-count", &export.count.to_string())?;
    reply.headers_mut().set("x-crm-export-sha256", &export.sha256)?;
    reply.headers_mut().set("x-crm-export-specification",
        export.specification.version_id().as_str())?;
    Ok(reply)
}
