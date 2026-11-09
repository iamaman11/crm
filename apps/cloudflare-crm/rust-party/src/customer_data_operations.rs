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
use crm_parties::{Party, PartyKind, decode_party_state};
use serde::Deserialize;
use serde_json::json;
use wasm_bindgen::JsValue;
use worker::{D1Database, Method, Request, Response, Result};
use super::{bind, error, response};

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

pub(super) async fn route(req: Request, db: D1Database,
    tenant: &str, role: Option<&str>) -> Result<Response> {
    // Export discloses an entire bounded set of personal/business Party names,
    // so unlike ordinary Party reads it is expressly admin-only.
    if role != Some("admin") {return error("FORBIDDEN",403);}
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
