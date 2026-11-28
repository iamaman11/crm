//! Bounded, synchronous Party CSV download using the ORIGINAL
//! crm-customer-data-operations scope/profile/specification/CSV functions.
//! One signed Access tenant, one original Party D1 owner; NO shadow export
//! business rules, persisted export job, file store or background service.
pub(super) use super::parties::SourceImportProof as ImportProof;
use super::{access, bind, parties};
use crm_customer_data_operations::{
    CreateImportRow, ExternalPartyIdentifierDigest, ImportJobId, ImportParserProfile, ImportRow,
    PartyImportKind, PartyImportMapping, PreparedPartyRow, RowDiagnostic, SourceSystemId,
    parse_import_source, party_export_hash_parts,
};
use crm_customer_data_operations::{
    PartyExportField, PartyExportKindFilter, PartyExportProfile, PartyExportScope,
    PartyExportSpecification, canonical_party_export_csv_record, party_export_hex,
    party_export_sha256,
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
#[derive(Debug, Clone)]
struct ImportCommand {
    id: String,
    kind: PartyKind,
    name: String,
    key: String,
    position: u32,
    external_id_sha256: Option<String>,
}

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
    source_id: Option<String>,
    commands: Vec<ImportCommand>,
    row_issues: Vec<ImportRowIssue>,
    count: usize,
    hash: String,
}
fn parse_import(
    bytes: &[u8],
    tenant: &str,
    selected_source_id: Option<&str>,
) -> std::result::Result<ImportPlan, &'static str> {
    let source_id = selected_source_id
        .map(|value| {
            SourceSystemId::try_new(value.to_owned()).map(|typed| typed.as_str().to_owned())
        })
        .transpose()
        .map_err(|_| "IMPORT_SOURCE_ID_INVALID")?;
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
    if source_id.is_some() && simple {
        return Err("IMPORT_SOURCE_ID_COLUMN_REQUIRED");
    }
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
    let identity = if let Some(source_id) = source_id.as_deref() {
        party_export_hash_parts(
            b"crm.cloudflare.csv-import-source-job/v1",
            &[
                tenant.as_bytes(),
                source_id.as_bytes(),
                mapping_version.as_bytes(),
            ],
        )
    } else {
        party_export_hash_parts(
            b"crm.cloudflare.csv-import-create-only/v1",
            &[tenant.as_bytes(), bytes, mapping_version.as_bytes()],
        )
    };
    let job_id = ImportJobId::try_new(format!("cdo-import-{}", party_export_hex(&identity)))
        .map_err(|_| "IMPORT_JOB_ID_INVALID")?;
    let mut seen = BTreeSet::new();
    let mut commands = Vec::new();
    let mut row_issues = Vec::new();
    for source in parsed.rows() {
        let start_issues = row_issues.len();
        let get = |column: &str| source.columns.get(column).map(String::as_str).unwrap_or("");
        let external_id = get("party_id");
        let kind = get("kind");
        let name = get("display_name");
        let version = get("resource_version");
        let external_id_sha256 = if !simple && !external_id.is_empty() {
            ExternalPartyIdentifierDigest::for_identifier(external_id.to_owned())
                .ok()
                .map(|digest| digest.as_str().to_owned())
        } else {
            None
        };
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
        let duplicate_external_id = if source_id.is_some() {
            match external_id_sha256.as_ref() {
                Some(digest) => !seen.insert(digest.clone()),
                None => true,
            }
        } else {
            !seen.insert(external_id.to_owned())
        };
        if !simple && (external_id.is_empty() || duplicate_external_id) {
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
            // In source mode the original ImportRow hashes the external key so
            // row identity survives file reordering and changed file bytes.
            // Legacy imports keep their original position-derived row identity.
            external_row_key: source_id.as_ref().map(|_| external_id.to_owned()),
            source_external_id: (!simple).then(|| external_id.to_owned()),
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
        commands.push(ImportCommand {
            id: checked.party_id().as_str().to_owned(),
            kind,
            name: checked.display_name().to_owned(),
            key: row.target_idempotency_key(),
            position: source.row_position,
            external_id_sha256: row
                .source_external_id_sha256()
                .map(|digest| digest.as_str().to_owned()),
        });
    }
    Ok(ImportPlan {
        source_id,
        commands,
        row_issues,
        count: parsed.rows().len(),
        hash,
    })
}
pub(super) struct ImportPreview {
    pub(super) count: usize,
    pub(super) hash: String,
    pub(super) already: usize,
    pub(super) new_count: usize,
    pub(super) matched_count: usize,
    pub(super) preview_fingerprint: String,
    pub(super) target_ids: Vec<String>,
    pub(super) row_issues: Vec<ImportRowIssue>,
}

struct SourceImportPreview {
    rows: Vec<parties::SourceImportRow>,
    target_ids: Vec<String>,
    new_count: usize,
    matched_count: usize,
    row_issues: Vec<ImportRowIssue>,
    fingerprint: String,
}

async fn source_preview(
    plan: &ImportPlan,
    db: &D1Database,
    tenant: &str,
) -> Result<std::result::Result<SourceImportPreview, parties::MutationFailure>> {
    let Some(source_id) = plan.source_id.as_deref() else {
        return Ok(Err(parties::MutationFailure::domain(
            "IMPORT_SOURCE_SELECTION_REQUIRED",
            connectrpc::ErrorCode::InvalidArgument,
        )));
    };
    if !parties::import_source_exists(db, tenant, source_id).await? {
        return Ok(Err(parties::MutationFailure::domain(
            "IMPORT_SOURCE_NOT_FOUND",
            connectrpc::ErrorCode::NotFound,
        )));
    }

    let mut rows = Vec::with_capacity(plan.commands.len());
    let mut row_issues = Vec::new();
    let mut new_count = 0;
    let mut matched_count = 0;
    let mut fingerprint_parts = vec![
        tenant.as_bytes().to_vec(),
        source_id.as_bytes().to_vec(),
        plan.hash.as_bytes().to_vec(),
    ];

    for command in &plan.commands {
        let Some(external_id_sha256) = command.external_id_sha256.as_deref() else {
            row_issue(
                &mut row_issues,
                command.position,
                "party_id",
                "IMPORT_EXTERNAL_ID_INVALID",
            );
            continue;
        };
        let mapped = parties::source_identity(db, tenant, source_id, external_id_sha256).await?;
        let mut party_id = command.id.clone();
        let mut expected_version = None;
        let mut status = "new";
        let mut current_state = "absent".to_owned();

        if let Some(mapped_party_id) = mapped {
            party_id = mapped_party_id;
            match parties::stored_versioned(db, tenant, &party_id).await? {
                Some((party, version)) => {
                    expected_version = Some(version);
                    current_state = format!(
                        "{}|{}|{}",
                        match party.kind() {
                            PartyKind::Person => "person",
                            PartyKind::Organization => "organization",
                        },
                        party.display_name(),
                        version
                    );
                    let mut matches = true;
                    if party.kind() != command.kind {
                        row_issue(
                            &mut row_issues,
                            command.position,
                            "kind",
                            "IMPORT_SOURCE_IDENTITY_CONFLICT",
                        );
                        matches = false;
                    }
                    if party.display_name() != command.name {
                        row_issue(
                            &mut row_issues,
                            command.position,
                            "display_name",
                            "IMPORT_SOURCE_IDENTITY_CONFLICT",
                        );
                        matches = false;
                    }
                    if matches {
                        status = "matched";
                        matched_count += 1;
                    } else {
                        status = "conflict";
                    }
                }
                None => {
                    status = "missing-target";
                    current_state = "missing".to_owned();
                    row_issue(
                        &mut row_issues,
                        command.position,
                        "party_id",
                        "IMPORT_SOURCE_IDENTITY_TARGET_MISSING",
                    );
                }
            }
        } else if let Some((party, version)) =
            parties::stored_versioned(db, tenant, &party_id).await?
        {
            status = "target-conflict";
            expected_version = Some(version);
            current_state = format!(
                "{}|{}|{}",
                match party.kind() {
                    PartyKind::Person => "person",
                    PartyKind::Organization => "organization",
                },
                party.display_name(),
                version
            );
            row_issue(
                &mut row_issues,
                command.position,
                "party_id",
                "IMPORT_SOURCE_TARGET_PARTY_CONFLICT",
            );
        } else {
            new_count += 1;
        }

        for part in [
            external_id_sha256.to_owned(),
            status.to_owned(),
            party_id.clone(),
            current_state,
            command.name.clone(),
            match command.kind {
                PartyKind::Person => "person".to_owned(),
                PartyKind::Organization => "organization".to_owned(),
            },
            command.position.to_string(),
        ] {
            fingerprint_parts.push(part.into_bytes());
        }
        rows.push(parties::SourceImportRow {
            party_id,
            kind: command.kind,
            display_name: command.name.clone(),
            row_key: command.key.clone(),
            external_id_sha256: external_id_sha256.to_owned(),
            expected_version,
        });
    }

    let fingerprint_refs = fingerprint_parts
        .iter()
        .map(Vec::as_slice)
        .collect::<Vec<_>>();
    let fingerprint = party_export_hex(&party_export_hash_parts(
        b"crm.cloudflare.csv-import-source-preview/v1",
        &fingerprint_refs,
    ));
    let target_ids = if row_issues.is_empty() {
        rows.iter().map(|row| row.party_id.clone()).collect()
    } else {
        Vec::new()
    };
    Ok(Ok(SourceImportPreview {
        rows,
        target_ids,
        new_count,
        matched_count,
        row_issues,
        fingerprint,
    }))
}

pub(super) async fn list_import_sources(
    db: &D1Database,
    tenant: &str,
) -> Result<Vec<parties::ImportSource>> {
    parties::list_import_sources(db, tenant).await
}

pub(super) async fn register_import_source(
    db: &D1Database,
    actor: &access::Actor,
    requested_name: &str,
    operation_key: &str,
    trace: &str,
) -> Result<
    std::result::Result<(parties::ImportSource, parties::ImportSaved), parties::MutationFailure>,
> {
    let display_name = requested_name.trim().to_owned();
    if display_name.is_empty()
        || display_name.len() > 240
        || display_name.chars().any(char::is_control)
    {
        return Ok(Err(parties::MutationFailure::domain(
            "IMPORT_SOURCE_NAME_INVALID",
            connectrpc::ErrorCode::InvalidArgument,
        )));
    }
    let normalized_name = crm_core_contracts::nfc_text_key(&display_name.to_lowercase());
    let identity = party_export_hash_parts(
        b"crm.cloudflare.csv-import-source/v1",
        &[actor.tenant.as_bytes(), normalized_name.as_bytes()],
    );
    let source_id = SourceSystemId::try_new(format!("csv-source-{}", party_export_hex(&identity)))
        .map_err(|error| worker::Error::RustError(error.code))?;
    parties::register_import_source(
        db,
        actor,
        source_id.as_str(),
        &display_name,
        &normalized_name,
        operation_key,
        trace,
    )
    .await
}

pub(super) async fn preview_import(
    bytes: &[u8],
    source_id: Option<&str>,
    db: &D1Database,
    tenant: &str,
) -> Result<std::result::Result<ImportPreview, parties::MutationFailure>> {
    let plan = match parse_import(bytes, tenant, source_id) {
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
            new_count: 0,
            matched_count: 0,
            preview_fingerprint: String::new(),
            target_ids: Vec::new(),
            row_issues: plan.row_issues,
        }));
    }
    if plan.source_id.is_some() {
        let source = match source_preview(&plan, db, tenant).await? {
            Ok(source) => source,
            Err(failure) => return Ok(Err(failure)),
        };
        return Ok(Ok(ImportPreview {
            count: plan.count,
            hash: plan.hash,
            already: source.matched_count,
            new_count: source.new_count,
            matched_count: source.matched_count,
            preview_fingerprint: source.fingerprint,
            target_ids: source.target_ids,
            row_issues: source.row_issues,
        }));
    }

    let mut already = 0;
    let mut issues = Vec::new();
    for command in &plan.commands {
        if parties::import_receipt_matches(
            db,
            tenant,
            &command.key,
            &command.id,
            command.kind,
            &command.name,
        )
        .await?
        {
            already += 1;
        } else if parties::stored(db, tenant, &command.id).await?.is_some() {
            row_issue(
                &mut issues,
                command.position,
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
            new_count: 0,
            matched_count: 0,
            preview_fingerprint: String::new(),
            target_ids: Vec::new(),
            row_issues: issues,
        }));
    }
    Ok(Ok(ImportPreview {
        count: plan.count,
        hash: plan.hash,
        already,
        new_count: plan.commands.len().saturating_sub(already),
        matched_count: already,
        preview_fingerprint: String::new(),
        target_ids: plan.commands.iter().map(|row| row.id.clone()).collect(),
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

pub(super) struct ImportApplied {
    pub(super) saved: parties::ImportSaved,
    pub(super) hash: String,
}

pub(super) async fn apply_import(
    bytes: &[u8],
    source_id: Option<&str>,
    proof: ImportProof<'_>,
    key: &str,
    db: &D1Database,
    actor: &access::Actor,
    trace: &str,
) -> Result<std::result::Result<ImportApplied, parties::MutationFailure>> {
    let plan = match parse_import(bytes, &actor.tenant, source_id) {
        Ok(plan) => plan,
        Err(code) => {
            return Ok(Err(parties::MutationFailure::domain(
                code,
                connectrpc::ErrorCode::InvalidArgument,
            )));
        }
    };
    if proof.source_hash != plan.hash {
        return Ok(Err(parties::MutationFailure::domain(
            "IMPORT_SOURCE_CHANGED",
            connectrpc::ErrorCode::Aborted,
        )));
    }
    if let Some(issue) = plan.row_issues.first() {
        return Ok(Err(parties::MutationFailure::domain(
            issue.diagnostic.code().to_owned(),
            connectrpc::ErrorCode::InvalidArgument,
        )));
    }
    if let Some(source_id) = plan.source_id.as_deref() {
        if proof.preview_fingerprint.is_empty() {
            return Ok(Err(parties::MutationFailure::domain(
                "IMPORT_PREVIEW_FINGERPRINT_REQUIRED",
                connectrpc::ErrorCode::InvalidArgument,
            )));
        }
        // A retry after a lost response must recover the committed receipt before
        // recomputing the now-changed (new -> matched) identity preview.
        if let Some(old) = parties::import_operation(db, &actor.tenant, key).await? {
            if old.source_sha256 != plan.hash
                || old.row_count != plan.count
                || old.source_id.as_deref() != Some(source_id)
                || old.preview_fingerprint.as_deref() != Some(proof.preview_fingerprint)
            {
                return Ok(Err(parties::MutationFailure::domain(
                    "IDEMPOTENCY_CONFLICT",
                    connectrpc::ErrorCode::Aborted,
                )));
            }
            return Ok(Ok(ImportApplied {
                saved: parties::ImportSaved {
                    created: 0,
                    replayed: old.row_count,
                    operation_replayed: true,
                    original_request_id: old.request_id,
                },
                hash: plan.hash,
            }));
        }
        let source = match source_preview(&plan, db, &actor.tenant).await? {
            Ok(source) => source,
            Err(failure) => return Ok(Err(failure)),
        };
        if let Some(issue) = source.row_issues.first() {
            return Ok(Err(parties::MutationFailure::domain(
                issue.diagnostic.code().to_owned(),
                connectrpc::ErrorCode::Aborted,
            )));
        }
        if source.fingerprint != proof.preview_fingerprint {
            return Ok(Err(parties::MutationFailure::domain(
                "IMPORT_PREVIEW_STALE",
                connectrpc::ErrorCode::Aborted,
            )));
        }
        return Ok(parties::import_source_batch(
            db,
            actor,
            source_id,
            source.rows,
            key,
            ImportProof {
                source_hash: &plan.hash,
                preview_fingerprint: &source.fingerprint,
            },
            trace,
        )
        .await?
        .map(|saved| ImportApplied {
            saved,
            hash: plan.hash,
        }));
    }
    if !proof.preview_fingerprint.is_empty() {
        return Ok(Err(parties::MutationFailure::domain(
            "IMPORT_SOURCE_SELECTION_INVALID",
            connectrpc::ErrorCode::InvalidArgument,
        )));
    }
    let rows = plan
        .commands
        .iter()
        .map(|row| (row.id.clone(), row.kind, row.name.clone(), row.key.clone()))
        .collect();
    Ok(
        parties::import_batch(db, actor, rows, key, &plan.hash, trace)
            .await?
            .map(|saved| ImportApplied {
                saved,
                hash: plan.hash,
            }),
    )
}
