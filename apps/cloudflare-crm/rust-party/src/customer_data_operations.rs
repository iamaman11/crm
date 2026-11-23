//! Bounded, synchronous Party CSV download using the ORIGINAL
//! crm-customer-data-operations scope/profile/specification/CSV functions.
//! One signed Access tenant, one original Party D1 owner; NO shadow export
//! business rules, persisted export job, file store or background service.
use super::{access, bind, parties};
use crm_customer_data_operations::{
    CreateImportRow, ImportJobId, ImportParserProfile, ImportRow, PartyImportKind,
    PartyImportMapping, PreparedPartyRow, RowDiagnostic, parse_import_source,
    party_export_hash_parts,
};
use crm_customer_data_operations::{
    PartyExportField, PartyExportKindFilter, PartyExportProfile,
    PartyExportScope, PartyExportSpecification, canonical_party_export_csv_record,
    party_export_hex, party_export_sha256,
};
use crm_parties::{CreateParty, Party, PartyId, PartyKind, decode_party_state};
use serde::Deserialize;
use std::collections::BTreeSet;
use wasm_bindgen::JsValue;
use worker::{D1Database, Result};

pub(super) const LIMIT: usize = 100;
const RETENTION: &str = "crm.dev.transient_csv_download.v1";

#[derive(Deserialize)]
struct Row {
    state: String,
}

pub(super) struct Export {
    pub(super) count: usize,
    pub(super) bytes: Vec<u8>,
    pub(super) sha256: String,
    pub(super) specification: PartyExportSpecification,
}

fn matches_kind(p: &Party, kind: Option<PartyExportKindFilter>) -> bool {
    matches!(
        (kind, p.kind()),
        (None, _)
            | (Some(PartyExportKindFilter::Person), PartyKind::Person)
            | (
                Some(PartyExportKindFilter::Organization),
                PartyKind::Organization
            )
    )
}

pub(super) async fn assemble(
    db: &D1Database,
    tenant: &str,
    kind: Option<PartyExportKindFilter>,
) -> Result<Option<Export>> {
    let scope = PartyExportScope::try_new(kind, LIMIT as u32)
        .map_err(|e| worker::Error::RustError(e.code))?;
    let profile = PartyExportProfile::v1(
        vec![
            PartyExportField::PartyId,
            PartyExportField::Kind,
            PartyExportField::DisplayName,
            PartyExportField::ResourceVersion,
        ],
        RETENTION,
    )
    .map_err(|e| worker::Error::RustError(e.code))?;
    let specification = PartyExportSpecification::try_new(scope, profile)
        .map_err(|e| worker::Error::RustError(e.code))?;

    // LIMIT+1 before filter: never conceal an incomplete tenant-scoped export.
    let rows = bind(
        db,
        "SELECT state FROM crm_party_records WHERE tenant = ? ORDER BY id LIMIT 101",
        &[JsValue::from_str(tenant)],
    )?
    .all()
    .await?
    .results::<Row>()?;
    if rows.len() > LIMIT {
        return Ok(None);
    }
    let mut bytes = canonical_party_export_csv_record(
        &specification
            .profile()
            .fields()
            .iter()
            .map(|field| field.canonical_name().to_string())
            .collect::<Vec<_>>(),
    )
    .map_err(|e| worker::Error::RustError(e.code))?;
    let mut count = 0;
    for row in rows {
        let party = decode_party_state(row.state.as_bytes())
            .map_err(|e| worker::Error::RustError(e.code))?;
        if !matches_kind(&party, specification.scope().kind_filter()) {
            continue;
        }
        let cells = specification
            .profile()
            .fields()
            .iter()
            .map(|field| match field {
                PartyExportField::PartyId => party.party_id().as_str().to_owned(),
                PartyExportField::Kind => match party.kind() {
                    PartyKind::Person => "person".to_owned(),
                    PartyKind::Organization => "organization".to_owned(),
                },
                PartyExportField::DisplayName => party.display_name().to_owned(),
                PartyExportField::ResourceVersion => party.version().to_string(),
            })
            .collect::<Vec<_>>();
        bytes.extend(
            canonical_party_export_csv_record(&cells)
                .map_err(|e| worker::Error::RustError(e.code))?,
        );
        count += 1;
    }
    let sha256 = party_export_hex(&party_export_sha256(&bytes));
    Ok(Some(Export {
        count,
        bytes,
        sha256,
        specification,
    }))
}

// Original parser, PartyImportMapping and ImportRow own CSV interpretation,
// provenance and stable target identity. External IDs NEVER become CRM Party IDs.
const IMPORT_MAX_ROWS: usize = 20;
const IMPORT_MAX_BYTES: usize = 16 * 1024;
// A normal list needs only a type and a name. The four-column export-shaped
// import remains supported with unchanged provenance and version validation.
const IMPORT_SIMPLE_FIELDS: [&str; 2] = ["kind", "display_name"];
const IMPORT_FIELDS: [&str; 4] = ["party_id", "kind", "display_name", "resource_version"];
type ImportCommand = (String, PartyKind, String, String); // target ID,kind,name,receipt key

// Reuse the ORIGINAL validated RowDiagnostic (field + stable reason) and the
// original parser's 1-based data-record position, not a second CSV grammar.
pub(super) struct ImportRowIssue {
    pub(super) row_position: u32,
    pub(super) diagnostic: RowDiagnostic,
}
fn row_issue(issues: &mut Vec<ImportRowIssue>, position: u32, field: &str, code: &str) {
    // All source column names and error codes here are static, bounded identifiers.
    let diagnostic =
        RowDiagnostic::try_new(code, field).expect("static CSV row diagnostic field and code");
    issues.push(ImportRowIssue {
        row_position: position,
        diagnostic,
    });
}
struct ImportPlan {
    commands: Vec<ImportCommand>,
    positions: Vec<u32>,
    row_issues: Vec<ImportRowIssue>,
    count: usize,
    hash: String,
}
fn parse_import(bytes: &[u8], tenant: &str) -> std::result::Result<ImportPlan, &'static str> {
    if bytes.is_empty() || bytes.len() > IMPORT_MAX_BYTES {
        return Err("IMPORT_FILE_SIZE_INVALID");
    }
    let profile =
        ImportParserProfile::csv_v1(b',', b'"').map_err(|_| "IMPORT_PARSER_PROFILE_INVALID")?;
    let parsed = parse_import_source(bytes, &profile).map_err(|_| "IMPORT_CSV_INVALID")?;
    let headers = parsed
        .headers()
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let simple = if headers.as_slice() == &IMPORT_SIMPLE_FIELDS[..] {
        true
    } else if headers.as_slice() == &IMPORT_FIELDS[..] {
        false
    } else {
        return Err("IMPORT_HEADER_INVALID");
    };
    if parsed.rows().is_empty() || parsed.rows().len() > IMPORT_MAX_ROWS {
        return Err("IMPORT_ROW_LIMIT_REACHED");
    }
    let mapping = PartyImportMapping::try_new(
        None,
        "kind",
        "display_name",
        (!simple).then(|| "party_id".to_owned()),
        None,
    )
    .map_err(|_| "IMPORT_MAPPING_INVALID")?;
    let mapping_version = mapping
        .version_id()
        .map_err(|_| "IMPORT_MAPPING_INVALID")?
        .as_str()
        .to_owned();
    let hash = party_export_hex(&party_export_sha256(bytes));
    let identity = party_export_hash_parts(
        b"crm.cloudflare.csv-import-create-only/v1",
        &[tenant.as_bytes(), bytes, mapping_version.as_bytes()],
    );
    let job_id = ImportJobId::try_new(format!("cdo-import-{}", party_export_hex(&identity)))
        .map_err(|_| "IMPORT_JOB_ID_INVALID")?;
    let mut seen = BTreeSet::new();
    let mut commands = Vec::new();
    let mut positions = Vec::new();
    let mut row_issues = Vec::new();
    for source in parsed.rows() {
        let start_issues = row_issues.len();
        let get = |column: &str| source.columns.get(column).map(String::as_str).unwrap_or("");
        let source_id = get("party_id");
        let kind = get("kind");
        let name = get("display_name");
        let version = get("resource_version");
        if !simple
            && version
                .parse::<u64>()
                .ok()
                .filter(|value| *value > 0)
                .is_none()
        {
            row_issue(
                &mut row_issues,
                source.row_position,
                "resource_version",
                "IMPORT_SOURCE_VERSION_INVALID",
            );
        }
        if !simple && (source_id.is_empty() || !seen.insert(source_id.to_owned())) {
            row_issue(
                &mut row_issues,
                source.row_position,
                "party_id",
                "IMPORT_DUPLICATE_OR_EMPTY_SOURCE_ID",
            );
        }
        let import_kind = match kind {
            "person" => Some(PartyImportKind::Person),
            "organization" => Some(PartyImportKind::Organization),
            _ => {
                row_issue(
                    &mut row_issues,
                    source.row_position,
                    "kind",
                    "IMPORT_KIND_INVALID",
                );
                None
            }
        };
        // The original Party domain validates names, but can only be used after
        // all earlier required fields have passed their own checks.
        if row_issues.len() != start_issues {
            if name.is_empty() {
                row_issue(
                    &mut row_issues,
                    source.row_position,
                    "display_name",
                    "IMPORT_PARTY_INVALID",
                );
            }
            continue;
        }
        let Some(import_kind) = import_kind else {
            continue;
        };
        let row = match ImportRow::create(CreateImportRow {
            job_id: job_id.clone(),
            row_position: source.row_position,
            external_row_key: None,
            // For simple CSV, the ORIGINAL ImportRow uses a deterministic
            // job-scoped row_position; never invent an external source ID.
            source_external_id: (!simple).then(|| source_id.to_owned()),
            occurred_at_unix_nanos: 1,
        }) {
            Ok(row) => row,
            Err(_) => {
                row_issue(
                    &mut row_issues,
                    source.row_position,
                    "party_id",
                    "IMPORT_ROW_ID_INVALID",
                );
                continue;
            }
        };
        let target = match row.derived_target_party_id() {
            Ok(target) => target,
            Err(_) => {
                row_issue(
                    &mut row_issues,
                    source.row_position,
                    "party_id",
                    "IMPORT_TARGET_ID_INVALID",
                );
                continue;
            }
        };
        let prepared = match PreparedPartyRow::try_new(target, import_kind, name.to_owned()) {
            Ok(prepared) => prepared,
            Err(_) => {
                row_issue(
                    &mut row_issues,
                    source.row_position,
                    "display_name",
                    "IMPORT_PARTY_INVALID",
                );
                continue;
            }
        };
        let kind = match prepared.kind() {
            PartyImportKind::Person => PartyKind::Person,
            PartyImportKind::Organization => PartyKind::Organization,
        };
        let party_id = match PartyId::try_new(prepared.party_id().as_str().to_owned()) {
            Ok(id) => id,
            Err(_) => {
                row_issue(
                    &mut row_issues,
                    source.row_position,
                    "party_id",
                    "IMPORT_TARGET_ID_INVALID",
                );
                continue;
            }
        };
        let checked = match Party::create(CreateParty {
            party_id,
            kind,
            display_name: prepared.display_name().to_owned(),
            occurred_at_unix_nanos: 1,
        }) {
            Ok(checked) => checked,
            Err(_) => {
                row_issue(
                    &mut row_issues,
                    source.row_position,
                    "display_name",
                    "IMPORT_PARTY_INVALID",
                );
                continue;
            }
        };
        if checked.display_name() != name {
            row_issue(
                &mut row_issues,
                source.row_position,
                "display_name",
                "IMPORT_NAME_NONCANONICAL",
            );
            continue;
        }
        commands.push((
            checked.party_id().as_str().to_owned(),
            kind,
            checked.display_name().to_owned(),
            row.target_idempotency_key(),
        ));
        positions.push(source.row_position);
    }
    Ok(ImportPlan {
        commands,
        positions,
        row_issues,
        count: parsed.rows().len(),
        hash,
    })
}
pub(super) struct ImportPreview {
    pub(super) count: usize,
    pub(super) hash: String,
    pub(super) already: usize,
    pub(super) target_ids: Vec<String>,
    pub(super) row_issues: Vec<ImportRowIssue>,
}
pub(super) struct ImportApplied {
    pub(super) saved: parties::ImportSaved,
    pub(super) hash: String,
}
pub(super) async fn preview_import(
    bytes: &[u8],
    db: &D1Database,
    tenant: &str,
) -> Result<std::result::Result<ImportPreview, parties::MutationFailure>> {
    let plan = match parse_import(bytes, tenant) {
        Ok(plan) => plan,
        Err(code) => {
            return Ok(Err(parties::MutationFailure::domain(
                code,
                connectrpc::ErrorCode::InvalidArgument,
            )));
        }
    };
    // A preview with ANY invalid row is informational, NEVER a write permit.
    if !plan.row_issues.is_empty() {
        return Ok(Ok(ImportPreview {
            count: plan.count,
            hash: plan.hash,
            already: 0,
            target_ids: Vec::new(),
            row_issues: plan.row_issues,
        }));
    }
    let mut already = 0;
    let mut issues = Vec::new();
    for ((id, kind, name, key), position) in plan.commands.iter().zip(plan.positions.iter()) {
        if parties::import_receipt_matches(db, tenant, key, id, *kind, name).await? {
            already += 1;
        } else if parties::stored(db, tenant, id).await?.is_some() {
            row_issue(
                &mut issues,
                *position,
                "party_id",
                "IMPORT_TARGET_PARTY_CONFLICT",
            );
        }
    }
    if !issues.is_empty() {
        return Ok(Ok(ImportPreview {
            count: plan.count,
            hash: plan.hash,
            already: 0,
            target_ids: Vec::new(),
            row_issues: issues,
        }));
    }
    Ok(Ok(ImportPreview {
        count: plan.count,
        hash: plan.hash,
        already,
        target_ids: plan.commands.into_iter().map(|r| r.0).collect(),
        row_issues: Vec::new(),
    }))
}
pub(super) async fn import_receipt(
    db: &D1Database,
    tenant: &str,
    key: &str,
) -> Result<Option<parties::ImportReceipt>> {
    parties::import_operation(db, tenant, key).await
}
pub(super) async fn apply_import(
    bytes: &[u8],
    expected: &str,
    key: &str,
    db: &D1Database,
    actor: &access::Actor,
    trace: &str,
) -> Result<std::result::Result<ImportApplied, parties::MutationFailure>> {
    let plan = match parse_import(bytes, &actor.tenant) {
        Ok(plan) => plan,
        Err(code) => {
            return Ok(Err(parties::MutationFailure::domain(
                code,
                connectrpc::ErrorCode::InvalidArgument,
            )));
        }
    };
    if expected != plan.hash {
        return Ok(Err(parties::MutationFailure::domain(
            "IMPORT_SOURCE_CHANGED",
            connectrpc::ErrorCode::Aborted,
        )));
    }
    // Apply independently revalidates the same original rows: a partial
    // preview can NEVER create a partial batch or reserve its receipt key.
    if let Some(issue) = plan.row_issues.first() {
        return Ok(Err(parties::MutationFailure::domain(
            issue.diagnostic.code().to_owned(),
            connectrpc::ErrorCode::InvalidArgument,
        )));
    }
    Ok(
        parties::import_batch(db, actor, plan.commands, key, &plan.hash, trace)
            .await?
            .map(|saved| ImportApplied {
                saved,
                hash: plan.hash,
            }),
    )
}
