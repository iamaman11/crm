//! D1 persistence of the original Sales performance report. The current
//! portfolio uses indexed keyset pages fenced by a tenant-local revision.
//! Each page and its fence are read in the same D1 statement.
use super::{bind, d1_write, diagnostics::ReceiptUncertainty, sales_deals};
use connectrpc::{ConnectError, ErrorCode};
use crm_sales::{
    CloseCreditContext, CommercialDimension, ForecastAccumulator, ForecastSource,
    PerformanceReport, SalesAccountAssignment, SalesQuota, SalesTerritory,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use wasm_bindgen::JsValue;
use worker::{D1Database, Result};

pub(super) type Failure = d1_write::OwnerFailure;

fn invalid() -> worker::Error {
    worker::Error::RustError("SALES_PERFORMANCE_SOURCE_INVALID".into())
}

pub(super) fn decode(raw: &str) -> Result<PerformanceReport> {
    PerformanceReport::decode(raw).map_err(|e| worker::Error::RustError(e.code))
}

// The synchronous envelope is explicit, not a silent LIMIT on totals.
// 32 full pages + end probe, plus existing auth/actuals/commit reads.
// A future durable bulk job can reuse the accumulator and versioned sources.
const CAPTURE_PAGE: usize = 256;
const MAX_CANDIDATES: usize = 8192;
const MAX_ROW_BYTES: usize = 16 * 1024;
const MAX_SOURCE_BYTES: usize = 8192;
const MAX_CAPTURE_BYTES: usize = 8 * 1024 * 1024;

pub(super) enum CaptureFailure {
    Storage,
    Changed,
    Capacity,
}

impl From<worker::Error> for CaptureFailure {
    fn from(_: worker::Error) -> Self {
        Self::Storage
    }
}

impl CaptureFailure {
    pub(super) fn connect(self) -> ConnectError {
        match self {
            Self::Storage => ConnectError::internal("SALES_FORECAST_SNAPSHOT_FAILED"),
            Self::Changed => ConnectError::aborted("SALES_FORECAST_SNAPSHOT_CHANGED"),
            Self::Capacity => ConnectError::resource_exhausted("SALES_FORECAST_CAPTURE_CAPACITY"),
        }
    }
}

pub(super) struct Snapshot {
    pub captured_at: i64,
    pub event_sequence: i64,
    pub actual_candidate_count: u64,
    pub forecast: ForecastAccumulator,
    pub sources: Vec<ForecastSource>,
}

#[derive(Deserialize)]
struct CaptureAnchor {
    captured_at: String,
    event_sequence: String,
    actual_candidate_count: String,
    revision: String,
}

#[derive(Deserialize)]
struct SnapshotRow {
    revision: String,
    id: Option<String>,
    oversized: i64,
    deal: Option<String>,
    teams: String,
    assignment: Option<String>,
    territory: Option<String>,
}

// Use exactly the same event-time, commit-order, tenant and pipeline predicate
// as the quota actuals query; the count includes unattributed/legacy events.
const MEMBERSHIP: &str = "SELECT COUNT(*) FROM crm_sales_deal_events e JOIN crm_sales_commercial_event_order o ON o.tenant=e.tenant AND o.deal_id=e.deal_id AND o.deal_version=e.deal_version WHERE e.tenant=?1 AND e.action IN ('closed','reopened') AND e.occurred_at_unix_nanos>=CAST(?2 AS INTEGER) AND e.occurred_at_unix_nanos<CAST(?3 AS INTEGER) AND (?4 IS NULL OR e.pipeline_id=?4)";

fn membership_parameters(tenant: &str, quota: &SalesQuota) -> Result<Vec<JsValue>> {
    let (start, end) = quota.period.bounds().map_err(|_| invalid())?;
    Ok(vec![
        JsValue::from_str(tenant),
        JsValue::from_str(&start.to_string()),
        JsValue::from_str(&end.to_string()),
        quota
            .pipeline_id
            .as_deref()
            .map_or(JsValue::NULL, JsValue::from_str),
    ])
}

#[derive(Deserialize)]
struct CountRow {
    count: String,
}

pub(super) async fn membership_count(
    db: &D1Database,
    tenant: &str,
    quota: &SalesQuota,
    captured_at: &str,
    sequence: &str,
) -> Result<u64> {
    let mut args = membership_parameters(tenant, quota)?;
    args.extend([JsValue::from_str(captured_at), JsValue::from_str(sequence)]);
    let sql = format!(
        "SELECT CAST(({MEMBERSHIP} AND e.occurred_at_unix_nanos<=CAST(?5 AS INTEGER) AND o.sequence<=CAST(?6 AS INTEGER)) AS TEXT) count"
    );
    bind(db, &sql, &args)?
        .first::<CountRow>(None)
        .await?
        .ok_or_else(invalid)?
        .count
        .parse()
        .map_err(|_| invalid())
}

pub(super) async fn snapshot(
    db: &D1Database,
    tenant: &str,
    quota: &SalesQuota,
    keep: Option<usize>,
) -> std::result::Result<Snapshot, CaptureFailure> {
    let anchor_sql = format!(
        "WITH clock AS MATERIALIZED (SELECT CAST(strftime('%s','now') AS INTEGER)*1000000000 + CAST(substr(strftime('%f','now'),4,3) AS INTEGER)*1000000 captured_at, (SELECT COALESCE(MAX(sequence),0) FROM crm_sales_commercial_event_order WHERE tenant=?1) sequence, COALESCE((SELECT revision FROM crm_sales_forecast_revisions WHERE tenant=?1),0) revision) SELECT CAST(c.captured_at AS TEXT) captured_at,CAST(c.sequence AS TEXT) event_sequence,CAST(c.revision AS TEXT) revision,CAST(({MEMBERSHIP} AND e.occurred_at_unix_nanos<=c.captured_at AND o.sequence<=c.sequence) AS TEXT) actual_candidate_count FROM clock c"
    );
    let anchor = bind(db, &anchor_sql, &membership_parameters(tenant, quota)?)?
        .first::<CaptureAnchor>(None)
        .await?
        .ok_or_else(invalid)?;
    let mut snapshot = Snapshot {
        captured_at: anchor.captured_at.parse().map_err(|_| invalid())?,
        event_sequence: anchor.event_sequence.parse().map_err(|_| invalid())?,
        actual_candidate_count: anchor
            .actual_candidate_count
            .parse()
            .map_err(|_| invalid())?,
        forecast: ForecastAccumulator::default(),
        sources: Vec::new(),
    };
    let teams = sales_deals::credit_teams_projection(
        "d.tenant",
        "json_extract(d.state,'$.owner.kind')",
        "COALESCE(json_extract(d.state,'$.owner.actor_id'),json_extract(d.state,'$.owner.team_id'))",
        true,
    );
    // Static predicate variants allow the partial indexes to be used without
    // an OR on pipeline. All user values remain bound.
    let pipeline_filter = if quota.pipeline_id.is_some() {
        " AND json_extract(d.state,'$.stage.pipeline_id')=?5"
    } else {
        ""
    };
    let oversized = "COALESCE(length(CAST(d.state AS BLOB)),0)+COALESCE(length(CAST(a.state AS BLOB)),0)+COALESCE(length(CAST(t.state AS BLOB)),0)";
    let sql = format!(
        "WITH fence AS MATERIALIZED (SELECT COALESCE((SELECT revision FROM crm_sales_forecast_revisions WHERE tenant=?1),0) revision), page AS MATERIALIZED (SELECT d.tenant,d.id,d.state,d.account_id FROM crm_sales_deal_records d WHERE d.tenant=?1 AND d.id>?2 AND (json_extract(d.state,'$.status')='open') IS TRUE{pipeline_filter} AND (SELECT revision FROM fence)=CAST(?3 AS INTEGER) ORDER BY d.id LIMIT ?4) SELECT CAST(f.revision AS TEXT) revision,d.id,CASE WHEN ({oversized})>{MAX_ROW_BYTES} THEN 1 ELSE 0 END oversized,CASE WHEN ({oversized})<={MAX_ROW_BYTES} THEN d.state END deal,CASE WHEN ({oversized})<={MAX_ROW_BYTES} THEN ({teams}) ELSE '[]' END teams,CASE WHEN ({oversized})<={MAX_ROW_BYTES} THEN a.state END assignment,CASE WHEN ({oversized})<={MAX_ROW_BYTES} THEN t.state END territory FROM fence f LEFT JOIN page d ON 1=1 LEFT JOIN crm_sales_account_assignment_records a ON a.tenant=d.tenant AND a.id=d.account_id LEFT JOIN crm_sales_territory_records t ON t.tenant=d.tenant AND t.id=json_extract(a.state,'$.territory_id') AND json_extract(a.state,'$.archived')=0 ORDER BY d.id"
    );
    let mut after = String::new();
    let mut candidates = 0usize;
    let mut source_bytes = 0usize;
    loop {
        let mut args = vec![
            JsValue::from_str(tenant),
            JsValue::from_str(&after),
            JsValue::from_str(&anchor.revision),
            JsValue::from_f64(CAPTURE_PAGE as f64),
        ];
        if let Some(pipeline) = &quota.pipeline_id {
            args.push(JsValue::from_str(pipeline));
        }
        let rows = bind(db, &sql, &args)?
            .all()
            .await?
            .results::<SnapshotRow>()?;
        if rows.is_empty() || rows.iter().any(|row| row.revision != anchor.revision) {
            return Err(CaptureFailure::Changed);
        }
        let mut page_count = 0usize;
        for row in rows {
            let Some(id) = row.id else {
                continue;
            };
            if id <= after {
                return Err(invalid().into());
            }
            after = id;
            page_count += 1;
            candidates += 1;
            if candidates > MAX_CANDIDATES || row.oversized != 0 {
                return Err(CaptureFailure::Capacity);
            }
            let state = row.deal.ok_or_else(invalid)?;
            let deal = sales_deals::decode(&state)?;
            if deal.deal_id().as_str() != after {
                return Err(invalid().into());
            }
            let assignment = row
                .assignment
                .as_deref()
                .map(SalesAccountAssignment::decode)
                .transpose()
                .map_err(|_| invalid())?;
            let assigned = assignment.as_ref().filter(|value| !value.archived);
            let territory = row
                .territory
                .as_deref()
                .map(SalesTerritory::decode)
                .transpose()
                .map_err(|_| invalid())?;
            let territory = match (
                assigned.and_then(|value| value.territory_id.as_deref()),
                territory,
            ) {
                (None, None) => None,
                (Some(id), Some(value)) if !value.archived && value.territory_id == id => {
                    Some(CommercialDimension {
                        id: value.territory_id,
                        name: value.name,
                        version: value.version,
                    })
                }
                _ => return Err(invalid().into()),
            };
            let context = CloseCreditContext {
                teams: serde_json::from_str(&row.teams).map_err(|_| invalid())?,
                territory,
                assignment_version: assigned.map(|value| value.version),
                catalog_version: None,
            };
            if let Some(source) =
                ForecastSource::capture(quota, &deal, context).map_err(|_| invalid())?
            {
                snapshot.forecast.observe(&source).map_err(|_| invalid())?;
                let bytes = serde_json::to_vec(&source).map_err(|_| invalid())?.len();
                source_bytes = source_bytes
                    .checked_add(bytes + 2)
                    .ok_or(CaptureFailure::Capacity)?;
                if bytes > MAX_SOURCE_BYTES || source_bytes > MAX_CAPTURE_BYTES {
                    return Err(CaptureFailure::Capacity);
                }
                if keep.is_none_or(|limit| snapshot.sources.len() < limit) {
                    snapshot.sources.push(source);
                }
            }
        }
        if page_count < CAPTURE_PAGE {
            break;
        }
    }
    Ok(snapshot)
}

#[derive(Deserialize)]
struct StateRow {
    state: String,
}

pub(super) async fn get(
    db: &D1Database,
    tenant: &str,
    id: &str,
) -> Result<Option<PerformanceReport>> {
    bind(
        db,
        "SELECT state FROM crm_sales_performance_reports WHERE tenant=? AND id=?",
        &[JsValue::from_str(tenant), JsValue::from_str(id)],
    )?
    .first::<StateRow>(None)
    .await?
    .map(|row| decode(&row.state))
    .transpose()
}

pub(super) struct SaveContext<'a> {
    pub db: &'a D1Database,
    pub tenant: &'a str,
    pub actor: &'a str,
    pub request_id: &'a str,
    pub key: &'a str,
    pub digest: &'a str,
}

pub(super) struct Saved {
    pub report: PerformanceReport,
    pub original_request_id: String,
    pub replayed: bool,
}

pub(super) fn digest(report_id: &str, name: &str, quota_id: &str, quota_version: i64) -> String {
    format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&(report_id, name, quota_id, quota_version))
                .expect("typed report intent")
        )
    )
}

pub(super) async fn replay(
    ctx: &SaveContext<'_>,
) -> Result<Option<std::result::Result<Saved, Failure>>> {
    let prior = bind(ctx.db, "SELECT idem_digest AS digest,state,request_id AS original_request_id FROM crm_sales_performance_reports WHERE tenant=? AND idem_key=?",
        &[JsValue::from_str(ctx.tenant), JsValue::from_str(ctx.key)])?
        .first::<d1_write::Receipt>(None).await?;
    let Some(prior) = prior else {
        return Ok(None);
    };
    if prior.digest != ctx.digest {
        return Ok(Some(Err(Failure::domain(
            "IDEMPOTENCY_CONFLICT",
            ErrorCode::Aborted,
        ))));
    }
    Ok(Some(Ok(Saved {
        report: decode(&prior.state)?,
        original_request_id: prior.original_request_id.ok_or_else(invalid)?,
        replayed: true,
    })))
}

pub(super) async fn save(
    ctx: &SaveContext<'_>,
    report: PerformanceReport,
    sources: &[ForecastSource],
) -> Result<std::result::Result<Saved, Failure>> {
    report.validate().map_err(|_| invalid())?;
    if report.forecast.source_count() != Some(sources.len() as u64)
        || report.captured_by != ctx.actor
    {
        return Err(invalid());
    }
    let state = serde_json::to_string(&report).map_err(|_| invalid())?;
    // Recheck source membership inside the write. Appending newer events is
    // permitted; deletion of a captured event makes this capture stale.
    let mut args = membership_parameters(ctx.tenant, &report.quota)?;
    args.extend([
        JsValue::from_str(&report.captured_at),
        JsValue::from_str(&report.event_sequence),
        JsValue::from_str(&report.actual_candidate_count.to_string()),
        JsValue::from_str(&report.report_id),
        JsValue::from_str(&report.quota.quota_id),
        JsValue::from_str(&report.quota.version.to_string()),
        JsValue::from_str(&state),
        JsValue::from_str(ctx.key),
        JsValue::from_str(ctx.digest),
        JsValue::from_str(ctx.request_id),
        JsValue::from_str(ctx.actor),
    ]);
    let insert = format!(
        "INSERT INTO crm_sales_performance_reports(tenant,id,quota_id,quota_version,state,idem_key,idem_digest,request_id,actor) SELECT ?1,?8,?9,CAST(?10 AS INTEGER),?11,?12,?13,?14,?15 WHERE ({MEMBERSHIP} AND e.occurred_at_unix_nanos<=CAST(?5 AS INTEGER) AND o.sequence<=CAST(?6 AS INTEGER))=CAST(?7 AS INTEGER) ON CONFLICT(tenant,id) DO NOTHING"
    );
    let mut writes = vec![bind(ctx.db, &insert, &args)?];
    // Chunk JSON payloads, not individual binds. A source row is a frozen
    // minimal projection, not a copy of the complete Deal or commercial ledger.
    for chunk in sources.chunks(100) {
        let json = serde_json::to_string(chunk).map_err(|_| invalid())?;
        writes.push(bind(ctx.db,
            "INSERT INTO crm_sales_performance_forecast_sources(tenant,report_id,deal_id,state) SELECT ?1,?2,json_extract(j.value,'$.deal_id'),j.value FROM json_each(?3) j WHERE EXISTS (SELECT 1 FROM crm_sales_performance_reports r WHERE r.tenant=?1 AND r.id=?2 AND r.request_id=?4)",
            &[JsValue::from_str(ctx.tenant), JsValue::from_str(&report.report_id), JsValue::from_str(&json), JsValue::from_str(ctx.request_id)])?);
    }
    let count = writes.len();
    let result = ctx.db.batch(writes).await;
    let confirmed = result
        .as_ref()
        .ok()
        .filter(|rows| rows.len() == count && rows.iter().all(worker::d1::D1Result::success))
        .and_then(|rows| rows.first())
        .and_then(|row| row.meta().ok().flatten()?.changes);
    if confirmed == Some(1) {
        return Ok(Ok(Saved {
            report,
            original_request_id: ctx.request_id.to_owned(),
            replayed: false,
        }));
    }
    let failure = |reason| {
        Failure::Write(super::diagnostics::MutationFailure::CommitUnconfirmed {
            code: "SALES_PERFORMANCE_REPORT_WRITE_UNCONFIRMED",
            reason,
        })
    };
    match replay(ctx).await {
        Ok(Some(saved)) => Ok(saved),
        Ok(None) if confirmed == Some(0) => Ok(Err(Failure::domain(
            "SALES_PERFORMANCE_REPORT_CONFLICT",
            ErrorCode::Aborted,
        ))),
        Ok(None) => Ok(Err(failure(ReceiptUncertainty::NotFound))),
        Err(_) => Ok(Err(failure(ReceiptUncertainty::ReadFailed))),
    }
}

pub(super) async fn list(
    db: &D1Database,
    tenant: &str,
    quota_id: Option<&str>,
    after: &str,
    size: usize,
) -> Result<(Vec<PerformanceReport>, String)> {
    let rows = bind(db,
        "SELECT state FROM crm_sales_performance_reports WHERE tenant=?1 AND id>?2 AND (?3 IS NULL OR quota_id=?3) ORDER BY id LIMIT ?4",
        &[JsValue::from_str(tenant), JsValue::from_str(after), quota_id.map_or(JsValue::NULL, JsValue::from_str), JsValue::from_f64((size+1) as f64)])?
        .all().await?.results::<StateRow>()?;
    let more = rows.len() > size;
    let reports = rows
        .into_iter()
        .take(size)
        .map(|row| decode(&row.state))
        .collect::<Result<Vec<_>>>()?;
    let next = if more {
        reports.last().ok_or_else(invalid)?.report_id.clone()
    } else {
        String::new()
    };
    Ok((reports, next))
}

pub(super) async fn forecast_page(
    db: &D1Database,
    tenant: &str,
    report: &PerformanceReport,
    after: &str,
    size: usize,
) -> Result<(Vec<ForecastSource>, String)> {
    let args = [
        JsValue::from_str(tenant),
        JsValue::from_str(&report.report_id),
    ];
    let count = bind(db, "SELECT CAST(COUNT(*) AS TEXT) count FROM crm_sales_performance_forecast_sources WHERE tenant=? AND report_id=?", &args)?
        .first::<CountRow>(None).await?.ok_or_else(invalid)?.count.parse::<u64>().map_err(|_| invalid())?;
    if Some(count) != report.forecast.source_count() {
        return Err(invalid());
    }
    let rows = bind(db,
        "SELECT state FROM crm_sales_performance_forecast_sources WHERE tenant=? AND report_id=? AND deal_id>? ORDER BY deal_id LIMIT ?",
        &[args[0].clone(), args[1].clone(), JsValue::from_str(after), JsValue::from_f64((size+1) as f64)])?
        .all().await?.results::<StateRow>()?;
    let more = rows.len() > size;
    let sources = rows
        .into_iter()
        .take(size)
        .map(|row| {
            let source =
                serde_json::from_str::<ForecastSource>(&row.state).map_err(|_| invalid())?;
            source.validate().map_err(|_| invalid())?;
            Ok(source)
        })
        .collect::<Result<Vec<_>>>()?;
    let next = if more {
        sources.last().ok_or_else(invalid)?.deal_id.clone()
    } else {
        String::new()
    };
    Ok((sources, next))
}
