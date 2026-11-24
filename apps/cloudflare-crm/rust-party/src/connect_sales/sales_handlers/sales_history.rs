//! Keyset scans of original Deal lifecycle events with a tenant-local read fence.
use super::*;
use crm_sales::{DealHistory, HistoryAccumulator, HistoryEvent, QuotaCadence, QuotaDate, QuotaPeriod};
use sha2::{Digest, Sha256};
use wasm_bindgen::JsValue;

const PAGE_ROWS: usize = 256;
type ReadResult<T> = std::result::Result<T, ConnectError>;

fn failure() -> ConnectError {
    ConnectError::internal("SALES_HISTORY_READ_FAILED")
}

fn capacity() -> ConnectError {
    ConnectError::resource_exhausted("SALES_HISTORY_CAPACITY")
}

fn changed() -> ConnectError {
    ConnectError::aborted("SALES_HISTORY_SNAPSHOT_CHANGED")
}

fn invalid() -> ConnectError {
    ConnectError::invalid_argument("SALES_HISTORY_SNAPSHOT_INVALID")
}

fn integer(raw: Option<String>) -> ReadResult<i64> {
    raw.ok_or_else(failure)?.parse().map_err(|_| failure())
}

#[derive(serde::Deserialize)]
struct Anchor {
    as_of: String,
    revision: String,
    pipeline_exists: i64,
}

#[derive(serde::Deserialize)]
struct HistoricalRow {
    revision: String,
    deal_id: Option<String>,
    deal_pipeline: Option<String>,
    record_version: Option<String>,
    record_created_at: Option<String>,
    event_version: Option<String>,
    kind: Option<crm_sales::HistoryKind>,
    pipeline_id: Option<String>,
    stage_id: Option<String>,
    previous_stage_id: Option<String>,
    status: Option<DomainDealStatus>,
    previous_status: Option<DomainDealStatus>,
    reason_code: Option<String>,
    event_at: Option<String>,
    catalog_version: Option<String>,
}

fn scope_digest(
    tenant: &str,
    pipeline: Option<&str>,
    period: &QuotaPeriod,
    compare: bool,
    as_of: i64,
    revision: i64,
) -> ReadResult<String> {
    let bytes = serde_json::to_vec(&(tenant, pipeline, period, compare, as_of, revision))
        .map_err(|_| failure())?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn read_period(input: &sale_wire::GetSalesHistoryRequest) -> ReadResult<QuotaPeriod> {
    let period = input
        .period
        .as_option()
        .ok_or_else(|| ConnectError::invalid_argument("SALES_HISTORY_PERIOD_REQUIRED"))?;
    let date = |value: &core_v2::CalendarDate| -> ReadResult<QuotaDate> {
        let invalid = || ConnectError::invalid_argument("SALES_HISTORY_DATE_INVALID");
        Ok(QuotaDate {
            year: value.year,
            month: u8::try_from(value.month).map_err(|_| invalid())?,
            day: u8::try_from(value.day).map_err(|_| invalid())?,
        })
    };
    let start = date(
        period
            .start_date
            .as_option()
            .ok_or_else(|| ConnectError::invalid_argument("SALES_HISTORY_DATE_REQUIRED"))?,
    )?;
    let end = date(
        period
            .end_date
            .as_option()
            .ok_or_else(|| ConnectError::invalid_argument("SALES_HISTORY_DATE_REQUIRED"))?,
    )?;
    QuotaPeriod::new(start, end, input.time_zone.clone(), QuotaCadence::Custom)
        .map_err(|error| ConnectError::invalid_argument(error.code))
}

struct Scan {
    current: HistoryAccumulator,
    previous: Option<HistoryAccumulator>,
    detail: bool,
}

impl Scan {
    fn add(&mut self, history: &DealHistory) -> bool {
        self.current.add(history);
        if let Some(previous) = &mut self.previous {
            previous.add(history);
        }
        self.detail && self.current.has_more()
    }
}

// A page always returns the revision, even when no candidate remains. The
// shared fence changes on Deal/catalog mutations and event insert/update/delete.
// Thus no candidate can appear, disappear or change between accepted pages.
async fn scan_history(
    db: &D1Database,
    tenant: &str,
    pipeline: Option<&str>,
    revision: i64,
    after: &str,
    scan: &mut Scan,
) -> ReadResult<()> {
    let revision = revision.to_string();
    let pipeline_filter = if pipeline.is_some() {
        " AND json_extract(d.state,'$.stage.pipeline_id')=?6"
    } else {
        ""
    };
    let sql = format!(
        r#"WITH fence AS MATERIALIZED (
            SELECT CAST(COALESCE((SELECT revision FROM crm_sales_forecast_revisions WHERE tenant=?1),0) AS TEXT) revision
        ), page AS MATERIALIZED (
            SELECT d.id deal_id,json_extract(d.state,'$.stage.pipeline_id') deal_pipeline,
                CAST(d.version AS TEXT) record_version,CAST(json_extract(d.state,'$.created_at_unix_nanos') AS TEXT) record_created_at,
                CAST(e.deal_version AS TEXT) event_version,e.action kind,e.pipeline_id,e.stage_id,e.previous_stage_id,
                e.status,e.previous_status,e.reason_code,CAST(e.occurred_at_unix_nanos AS TEXT) event_at,
                CAST(e.catalog_version AS TEXT) catalog_version,COALESCE(e.deal_version,0) event_order
            FROM crm_sales_deal_records d LEFT JOIN crm_sales_deal_events e ON e.tenant=d.tenant AND e.deal_id=d.id
            WHERE d.tenant=?1 AND d.id>=?3 AND (d.id>?3 OR COALESCE(e.deal_version,0)>CAST(?4 AS INTEGER))
                AND (SELECT revision FROM fence)=?2{pipeline_filter}
            ORDER BY d.id,e.deal_version LIMIT ?5
        )
        SELECT f.revision,p.* FROM fence f LEFT JOIN page p ON 1
        ORDER BY p.deal_id,p.event_order"#
    );
    let mut cursor = after.to_owned();
    let mut version = i64::MAX;
    let mut pending: Option<DealHistory> = None;
    let mut scanned = 0usize;
    loop {
        let mut args = vec![
            JsValue::from_str(tenant),
            JsValue::from_str(&revision),
            JsValue::from_str(&cursor),
            JsValue::from_str(&version.to_string()),
            JsValue::from_f64(PAGE_ROWS as f64),
        ];
        if let Some(pipeline) = pipeline {
            args.push(JsValue::from_str(pipeline));
        }
        let rows = crate::bind(db, &sql, &args)
            .map_err(|_| failure())?
            .all()
            .await
            .map_err(|_| failure())?
            .results::<HistoricalRow>()
            .map_err(|_| failure())?;
        if rows.is_empty() || rows.iter().any(|row| row.revision != revision) {
            return Err(changed());
        }
        let mut count = 0usize;
        for row in rows {
            let Some(id) = row.deal_id else {
                continue;
            };
            let event_version = row.event_version.map(|v| integer(Some(v))).transpose()?;
            let order = event_version.unwrap_or(0);
            if id < cursor || (id == cursor && order <= version) {
                return Err(failure());
            }
            cursor = id.clone();
            version = order;
            count += 1;
            scanned += 1;
            if scanned > crm_sales::MAX_HISTORY_SCAN_ROWS {
                return Err(capacity());
            }
            if pending.as_ref().is_some_and(|history| history.id != id)
                && let Some(history) = pending.take()
                && scan.add(&history)
            {
                return Ok(());
            }
            let history = pending.get_or_insert(DealHistory {
                id,
                pipeline_id: row.deal_pipeline.ok_or_else(failure)?,
                version: integer(row.record_version)?,
                created_at: integer(row.record_created_at)?,
                events: Vec::new(),
            });
            if let Some(event_version) = event_version {
                if history.events.len() >= crm_sales::MAX_HISTORY_DEAL_EVENTS {
                    return Err(capacity());
                }
                history.events.push(HistoryEvent {
                    version: event_version,
                    kind: row.kind.ok_or_else(failure)?,
                    pipeline_id: row.pipeline_id.ok_or_else(failure)?,
                    stage_id: row.stage_id.ok_or_else(failure)?,
                    previous_stage_id: row.previous_stage_id,
                    status: row.status.ok_or_else(failure)?,
                    previous_status: row.previous_status,
                    reason_code: row.reason_code,
                    at: integer(row.event_at)?,
                    catalog_version: row.catalog_version.map(|v| integer(Some(v))).transpose()?,
                });
            }
        }
        if count < PAGE_ROWS {
            if let Some(history) = pending.take() {
                scan.add(&history);
            }
            return Ok(());
        }
    }
}

impl SalesV2Service {
    pub(super) async fn read_history(
        &self,
        input: sale_wire::GetSalesHistoryRequest,
    ) -> ReadResult<sale_wire::GetSalesHistoryResponse> {
        let period = read_period(&input)?;
        let previous = input
            .compare_previous
            .then(|| crm_sales::previous_history_period(&period))
            .transpose()
            .map_err(|error| ConnectError::invalid_argument(error.code))?;
        let page_size = if input.page_size == 0 {
            100
        } else {
            Self::performance_page_size(input.page_size)?
        };
        let detail = input.detail.as_option();
        if let Some(detail) = detail {
            if input.snapshot.as_option().is_none()
                || (detail.previous && previous.is_none())
            {
                return Err(invalid());
            }
            if !detail.after_deal_id.is_empty() {
                Self::id(&detail.after_deal_id)?;
            }
        }
        let pipeline = input.pipeline_id.as_deref();
        if let Some(id) = pipeline {
            Self::pipeline_id(id)?;
        }
        let db = self.d1()?;
        let anchor = crate::bind(
            &db,
            "SELECT CAST(CAST(strftime('%s','now') AS INTEGER)*1000000000+CAST(substr(strftime('%f','now'),4,3) AS INTEGER)*1000000 AS TEXT) as_of,CAST(COALESCE((SELECT revision FROM crm_sales_forecast_revisions WHERE tenant=?1),0) AS TEXT) revision,CASE WHEN ?2 IS NULL OR EXISTS(SELECT 1 FROM crm_sales_pipeline_records WHERE tenant=?1 AND id=?2) THEN 1 ELSE 0 END pipeline_exists",
            &[JsValue::from_str(&self.tenant), pipeline.map_or(JsValue::NULL, JsValue::from_str)],
        )
        .map_err(|_| failure())?
        .first::<Anchor>(None)
        .await
        .map_err(|_| failure())?
        .ok_or_else(failure)?;
        if anchor.pipeline_exists != 1 {
            return Err(ConnectError::not_found("SALES_PIPELINE_NOT_FOUND"));
        }
        let now = integer(Some(anchor.as_of))?;
        let revision = integer(Some(anchor.revision))?;
        let as_of = if let Some(snapshot) = input.snapshot.as_option() {
            if snapshot.as_of_unix_nanos <= 0 || snapshot.as_of_unix_nanos > now
                || snapshot.revision < 0
            {
                return Err(invalid());
            }
            let digest = scope_digest(
                &self.tenant, pipeline, &period, input.compare_previous,
                snapshot.as_of_unix_nanos, snapshot.revision,
            )?;
            if snapshot.scope_sha256_hex != digest {
                return Err(invalid());
            }
            if snapshot.revision != revision {
                return Err(changed());
            }
            snapshot.as_of_unix_nanos
        } else {
            now
        };
        let snapshot = sale_wire::SalesHistorySnapshot {
            as_of_unix_nanos: as_of,
            revision,
            scope_sha256_hex: scope_digest(
                &self.tenant, pipeline, &period, input.compare_previous, as_of, revision,
            )?,
            ..Default::default()
        };
        let selected = if detail.is_some_and(|d| d.previous) {
            previous.as_ref().ok_or_else(invalid)?
        } else {
            &period
        };
        let accumulator = |period: &QuotaPeriod| {
            HistoryAccumulator::new(period, as_of, page_size)
                .map_err(|error| ConnectError::invalid_argument(error.code))
        };
        let mut scan = Scan {
            current: accumulator(selected)?,
            previous: if detail.is_none() {
                previous.as_ref().map(accumulator).transpose()?
            } else {
                None
            },
            detail: detail.is_some(),
        };
        scan_history(
            &db, &self.tenant, pipeline, revision,
            detail.map_or("", |d| d.after_deal_id.as_str()), &mut scan,
        )
        .await?;
        let current = scan.current.finish();
        if let Some(detail) = detail {
            return Ok(sale_wire::GetSalesHistoryResponse {
                as_of_unix_nanos: as_of,
                snapshot: Some(snapshot).into(),
                detail: Some(sale_wire::SalesHistoryDetailPage {
                    previous: detail.previous,
                    deals: current.deals.into_iter().map(history_deal).collect(),
                    next_deal_id: current.next_deal_id,
                    ..Default::default()
                }).into(),
                ..Default::default()
            });
        }
        let previous = scan.previous.map(HistoryAccumulator::finish);
        let (comparable, delta) = crm_sales::history_comparison(&current, previous.as_ref());
        Ok(sale_wire::GetSalesHistoryResponse {
            as_of_unix_nanos: as_of,
            snapshot: Some(snapshot).into(),
            current: Some(history_window(current)).into(),
            previous: previous.map(history_window).into(),
            comparison_comparable: comparable,
            win_rate_delta_basis_points: delta,
            ..Default::default()
        })
    }
}
