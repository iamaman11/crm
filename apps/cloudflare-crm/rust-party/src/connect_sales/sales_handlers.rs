//! Generated Connect trait for the existing SalesV2Service Rust domain owner.
//! Access, tenant, and the single native router are defined by the parent.
use super::*;
use crate::diagnostics;

#[derive(serde::Deserialize)]
struct GroupRow {
    #[serde(default)]
    bucket: usize,
    action: String,
    outcome: Option<String>,
    currency: Option<String>,
    dimension_id: Option<String>,
    fact_known: i64,
    amount_present: i64,
    count: String,
    invalid_amount: i64,
    limb0: String,
    limb1: String,
    limb2: String,
    limb3: String,
    limb4: String,
}

fn commercial_group_sql(candidates: &str, filter: &str, by_period: bool) -> String {
    let limbs = (0..5)
        .map(|i| {
            format!(
                "CAST(SUM(CAST(substr('000000000000000000000000000000000000000000000'||COALESCE(minor_units,'0'),-{},9) AS INTEGER)) AS TEXT) limb{i}",
                (i + 1) * 9
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let bucket = if by_period { "bucket," } else { "" };
    format!(
        "{candidates} SELECT {bucket}action,outcome,currency,dimension_id,commercial_close_fact IS NOT NULL fact_known,\
             minor_units IS NOT NULL amount_present,CAST(COUNT(*) AS TEXT) count,\
             MAX(CASE WHEN minor_units IS NULL THEN 0 WHEN typeof(minor_units)<>'text' \
             OR length(minor_units) NOT BETWEEN 1 AND 39 OR minor_units GLOB '*[^0-9]*' \
             OR (length(minor_units)>1 AND substr(minor_units,1,1)='0') \
             OR (length(minor_units)=39 AND minor_units>'170141183460469231731687303715884105727') \
             THEN 1 ELSE 0 END) invalid_amount,{limbs} FROM candidates{filter} \
         GROUP BY {bucket}action,outcome,currency,dimension_id,fact_known,amount_present"
    )
}

impl GroupRow {
    fn fact(&self, in_scope: bool) -> std::result::Result<crm_sales::QuotaFactGroup<'_>, ConnectError> {
        let invalid = || ConnectError::internal("SALES_COMMERCIAL_EVENT_INVALID");
        if self.invalid_amount != 0 || (self.amount_present == 1 && self.currency.is_none()) {
            return Err(invalid());
        }
        let outcome = match (self.fact_known, self.outcome.as_deref()) {
            (0, None) => None,
            (1, Some("won")) => Some(crm_sales::DealStatus::Won),
            (1, Some("lost")) => Some(crm_sales::DealStatus::Lost),
            _ => return Err(invalid()),
        };
        let booked = if in_scope
            && self.amount_present == 1
            && outcome == Some(crm_sales::DealStatus::Won)
        {
            Some(
                crm_sales::quota_sum_limbs([
                    &self.limb4,
                    &self.limb3,
                    &self.limb2,
                    &self.limb1,
                    &self.limb0,
                ])
                .map_err(|_| ConnectError::internal("SALES_QUOTA_TOTAL_OVERFLOW"))?,
            )
        } else {
            None
        };
        Ok(crm_sales::QuotaFactGroup {
            outcome,
            dimension: self.dimension_id.as_deref(),
            currency: self.currency.as_deref(),
            reversal: self.action == "reopened",
            booked_minor_units: booked,
            count: self.count.parse().map_err(|_| invalid())?,
        })
    }
}

impl SalesV2Service {
    async fn scorecard_detail(
        &self,
        card: &crm_sales::Scorecard,
        input: &sale_wire::GetSalesScorecardRequest,
        detail: &sale_wire::SalesScorecardDetailRequest,
        dimension: &str,
    ) -> std::result::Result<sale_wire::GetSalesScorecardResponse, ConnectError> {
        use wasm_bindgen::JsValue;
        let invalid = || ConnectError::invalid_argument("SALES_SCORECARD_DETAIL_INVALID");
        let failure = || ConnectError::internal("SALES_SCORECARD_DETAIL_READ_FAILED");
        let now = crate::now_nanos();
        if detail.as_of_unix_nanos <= 0
            || detail.as_of_unix_nanos > now
            || detail.event_sequence < 0
            || (detail.organization && detail.dimension_id.is_some())
            || (!detail.organization && dimension == "NULL")
        {
            return Err(invalid());
        }
        if detail
            .dimension_id
            .as_deref()
            .is_some_and(|id| id.is_empty() || id.len() > 254)
        {
            return Err(invalid());
        }
        let bucket = card
            .buckets()
            .get(detail.bucket_index as usize)
            .ok_or_else(invalid)?;
        let size = Self::performance_page_size(detail.page_size)?;
        let (start, end) = bucket.period.bounds().map_err(|_| invalid())?;
        let db = self.d1()?;
        let pipeline = input
            .pipeline_id
            .as_deref()
            .map_or(JsValue::NULL, JsValue::from_str);
        let mut values = vec![
            JsValue::from_str(&self.tenant),
            JsValue::from_str(&start.to_string()),
            JsValue::from_str(&end.to_string()),
            JsValue::from_str(&detail.as_of_unix_nanos.to_string()),
            JsValue::from_str(&detail.event_sequence.to_string()),
            pipeline,
            JsValue::from_str(&input.currency_code),
        ];
        let mut candidates = String::from(
            " FROM crm_sales_deal_events e JOIN crm_sales_commercial_event_order o ON o.tenant=e.tenant AND o.deal_id=e.deal_id AND o.deal_version=e.deal_version WHERE e.tenant=?1 AND e.action IN ('closed','reopened') AND e.occurred_at_unix_nanos>=CAST(?2 AS INTEGER) AND e.occurred_at_unix_nanos<CAST(?3 AS INTEGER) AND e.occurred_at_unix_nanos<=CAST(?4 AS INTEGER) AND o.sequence<=CAST(?5 AS INTEGER) AND (?6 IS NULL OR e.pipeline_id=?6) AND (json_extract(e.commercial_close_fact,'$.amount.currency_code') IS NULL OR json_extract(e.commercial_close_fact,'$.amount.currency_code')=?7)"
        );
        if !detail.organization {
            candidates.push_str(&format!(
                " AND e.commercial_close_fact IS NOT NULL AND {dimension} IS ?8"
            ));
            values.push(
                detail
                    .dimension_id
                    .as_deref()
                    .map_or(JsValue::NULL, JsValue::from_str),
            );
        }
        let count = crate::bind(
            &db,
            &format!("SELECT CAST(COUNT(*) AS TEXT) count,(SELECT CAST(COALESCE(MAX(sequence),0) AS TEXT) FROM crm_sales_commercial_event_order WHERE tenant=?1) watermark{candidates}"),
            &values,
        )
        .map_err(|_| failure())?;
        let mut page_sql = format!(
            "SELECT e.deal_id,e.deal_version,e.action,CAST(e.occurred_at_unix_nanos AS TEXT) event_at,e.commercial_close_fact{candidates}"
        );
        if let Some(after) = detail.after.as_option() {
            Self::id(&after.deal_id)?;
            if after.deal_version <= 0 {
                return Err(invalid());
            }
            page_sql.push_str(" AND (e.deal_id,e.deal_version)>(?,CAST(? AS INTEGER))");
            values.push(JsValue::from_str(&after.deal_id));
            values.push(JsValue::from_str(&after.deal_version.to_string()));
        }
        page_sql.push_str(" ORDER BY e.deal_id,e.deal_version LIMIT ?");
        values.push(JsValue::from_f64((size + 1) as f64));
        let page = crate::bind(&db, &page_sql, &values).map_err(|_| failure())?;
        let results = db.batch(vec![count, page]).await.map_err(|_| failure())?;
        if results.len() != 2 || !results.iter().all(worker::d1::D1Result::success) {
            return Err(failure());
        }
        #[derive(serde::Deserialize)]
        struct CountRow {
            count: String,
            watermark: String,
        }
        #[derive(serde::Deserialize)]
        struct FactRow {
            deal_id: String,
            deal_version: i64,
            action: String,
            event_at: String,
            commercial_close_fact: Option<String>,
        }
        let counts = results[0].results::<CountRow>().map_err(|_| failure())?;
        let row = counts.first().ok_or_else(failure)?;
        let count = row.count.parse::<u64>().map_err(|_| failure())?;
        let watermark = row.watermark.parse::<i64>().map_err(|_| failure())?;
        if count != detail.expected_event_count || detail.event_sequence > watermark {
            return Err(ConnectError::aborted("SALES_SCORECARD_HISTORY_CHANGED"));
        }
        let rows = results[1].results::<FactRow>().map_err(|_| failure())?;
        let more = rows.len() > size;
        let facts = rows
            .into_iter()
            .take(size)
            .map(|row| {
                let reversal = row.action == "reopened";
                let fact = row
                    .commercial_close_fact
                    .as_deref()
                    .map(serde_json::from_str::<crm_sales::CommercialCloseFact>)
                    .transpose()
                    .map_err(|_| failure())?;
                let amount = fact
                    .as_ref()
                    .map(|f| f.booked_minor_units(reversal))
                    .transpose()
                    .map_err(|_| failure())?
                    .flatten();
                Ok(sale_wire::SalesQuotaFact {
                    deal_id: row.deal_id,
                    deal_version: row.deal_version,
                    event_at_unix_nanos: row.event_at.parse().map_err(|_| failure())?,
                    reversal,
                    close_fact: fact
                        .as_ref()
                        .map(Self::commercial_fact_wire)
                        .transpose()?
                        .into(),
                    signed_minor_units: amount.map(|n| n.to_string()),
                    ..Default::default()
                })
            })
            .collect::<std::result::Result<Vec<_>, ConnectError>>()?;
        let next = if more {
            facts.last().map(|fact| sale_wire::SalesQuotaFactCursor {
                deal_id: fact.deal_id.clone(),
                deal_version: fact.deal_version,
                ..Default::default()
            })
        } else {
            None
        };
        Ok(sale_wire::GetSalesScorecardResponse {
            as_of_unix_nanos: detail.as_of_unix_nanos,
            event_sequence: detail.event_sequence,
            currency_code: input.currency_code.clone(),
            axis: input.axis,
            detail: Some(sale_wire::SalesScorecardDetail {
                facts,
                next: next.into(),
                source_event_count: count,
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
    }

    async fn capture_performance(
        &self,
        quota_id: &str,
        quota_version: i64,
        report_id: String,
        name: String,
        keep: Option<usize>,
    ) -> std::result::Result<
        (crm_sales::PerformanceReport, Vec<crm_sales::ForecastSource>),
        ConnectError,
    > {
        if quota_version <= 0 {
            return Err(ConnectError::invalid_argument(
                "SALES_QUOTA_VERSION_INVALID",
            ));
        }
        crm_sales::PerformanceReport::validate_name(&name)
            .map_err(|e| ConnectError::invalid_argument(e.code))?;
        let db = self.d1()?;
        let record = sales_territories::quota_revision(&db, &self.tenant, quota_id, quota_version)
            .await
            .map_err(|_| ConnectError::internal("SALES_QUOTA_READ_FAILED"))?
            .ok_or_else(|| ConnectError::not_found("SALES_QUOTA_NOT_FOUND"))?;
        let sales_territories::Record::Quota(quota) = record else {
            return Err(ConnectError::internal("SALES_QUOTA_STATE_INVALID"));
        };
        if !quota.published {
            return Err(ConnectError::invalid_argument(
                "SALES_PERFORMANCE_PUBLISHED_QUOTA_REQUIRED",
            ));
        }
        let snapshot = sales_performance::snapshot(&db, &self.tenant, &quota, keep)
            .await
            .map_err(sales_performance::CaptureFailure::connect)?;
        let (_, actuals) = self
            .quota_performance(sale_wire::GetSalesQuotaPerformanceRequest {
                quota_id: quota_id.to_owned(),
                quota_version: Some(quota_version),
                as_of_unix_nanos: Some(snapshot.captured_at),
                event_sequence: Some(snapshot.event_sequence),
                page_size: 1,
                ..Default::default()
            })
            .await?;
        let captured_at = snapshot.captured_at.to_string();
        let event_sequence = snapshot.event_sequence.to_string();
        let count = sales_performance::membership_count(
            &db,
            &self.tenant,
            &quota,
            &captured_at,
            &event_sequence,
        )
        .await
        .map_err(|_| ConnectError::internal("SALES_PERFORMANCE_MEMBERSHIP_READ_FAILED"))?;
        if count != snapshot.actual_candidate_count {
            return Err(ConnectError::aborted("SALES_PERFORMANCE_HISTORY_CHANGED"));
        }
        let forecast = snapshot
            .forecast
            .finish(&actuals)
            .map_err(|_| ConnectError::internal("SALES_PERFORMANCE_TOTAL_OVERFLOW"))?;
        Ok((
            crm_sales::PerformanceReport {
                report_id,
                name,
                quota,
                captured_at,
                captured_by: self.actor.clone(),
                event_sequence,
                formula_version: 1,
                actuals,
                forecast,
                actual_candidate_count: count,
            },
            snapshot.sources,
        ))
    }

    fn report_wire(
        value: crm_sales::PerformanceReport,
    ) -> std::result::Result<sale_wire::SalesPerformanceReport, ConnectError> {
        let invalid = || ConnectError::internal("SALES_PERFORMANCE_REPORT_INVALID");
        let a = value.actuals;
        let f = value.forecast;
        Ok(sale_wire::SalesPerformanceReport {
            report_id: value.report_id,
            name: value.name,
            quota: Some(Self::quota_wire(sales_territories::Record::Quota(
                value.quota,
            ))?)
            .into(),
            captured_at_unix_nanos: value.captured_at.parse().map_err(|_| invalid())?,
            event_sequence: value.event_sequence.parse().map_err(|_| invalid())?,
            captured_by: value.captured_by,
            formula_version: value.formula_version,
            actual_candidate_count: value.actual_candidate_count,
            actuals: Some(sale_wire::SalesPerformanceActuals {
                gross_minor_units: a.gross,
                reversal_minor_units: a.reversals,
                net_minor_units: a.net,
                remaining_minor_units: a.remaining,
                attainment_basis_points: a.attainment,
                won_count: a.won_count,
                lost_count: a.lost_count,
                reversal_count: a.reversal_count,
                missing_amount_count: a.missing_amount_count,
                unknown_history_count: a.unknown_history_count,
                unallocated_count: a.unallocated_count,
                ..Default::default()
            })
            .into(),
            forecast: Some(sale_wire::SalesForecastCoverage {
                open_minor_units: f.open,
                weighted_minor_units_times_basis_points: f.weighted,
                open_coverage_basis_points: f.open_coverage,
                weighted_coverage_basis_points: f.weighted_coverage,
                included_count: f.included_count,
                unallocated_count: f.unallocated_count,
                undated_count: f.undated_count,
                omitted_count: f.omitted_count,
                missing_amount_count: f.missing_amount_count,
                other_currency_count: f.other_currency_count,
                buckets: f
                    .buckets
                    .into_iter()
                    .map(|b| sale_wire::SalesForecastBucket {
                        category: Self::forecast_wire(b.category).into(),
                        deal_count: b.count,
                        open_minor_units: b.open,
                        weighted_minor_units_times_basis_points: b.weighted,
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
    }

    fn forecast_source_wire(
        value: crm_sales::ForecastSource,
    ) -> std::result::Result<sale_wire::SalesForecastSource, ConnectError> {
        use crm_sales::ForecastDisposition as Domain;
        use sale_wire::SalesForecastDisposition as Wire;
        let dimension = |v: crm_sales::CommercialDimension| sale_wire::CommercialDimension {
            id: v.id,
            name: v.name,
            version: v.version,
            ..Default::default()
        };
        let amount = value
            .amount
            .map(|a| {
                Ok::<_, ConnectError>(core_v2::ExactMoney {
                    minor_units: a
                        .minor_units
                        .parse()
                        .map_err(|_| ConnectError::internal("SALES_FORECAST_AMOUNT_INVALID"))?,
                    currency_code: a.currency_code,
                    ..Default::default()
                })
            })
            .transpose()?;
        Ok(sale_wire::SalesForecastSource {
            deal_id: value.deal_id,
            deal_version: value.deal_version,
            pipeline_id: value.pipeline_id,
            expected_close_date: value
                .expected_close_date
                .map(|d| core_v2::CalendarDate {
                    year: d.year,
                    month: u32::from(d.month),
                    day: u32::from(d.day),
                    ..Default::default()
                })
                .into(),
            amount: amount.into(),
            probability_basis_points: u32::from(value.probability_basis_points),
            probability_source: match value.probability_source {
                crm_sales::ProbabilitySource::Manual => {
                    sale_wire::ProbabilitySource::PROBABILITY_SOURCE_MANUAL
                }
                crm_sales::ProbabilitySource::StageDefault => {
                    sale_wire::ProbabilitySource::PROBABILITY_SOURCE_STAGE_DEFAULT
                }
            }
            .into(),
            category: Self::forecast_wire(value.category).into(),
            credited_actor: value.credited_actor,
            credited_team: value.credited_team.map(dimension).into(),
            credited_territory: value.credited_territory.map(dimension).into(),
            disposition: match value.disposition {
                Domain::Included => Wire::SALES_FORECAST_DISPOSITION_INCLUDED,
                Domain::Unallocated => Wire::SALES_FORECAST_DISPOSITION_UNALLOCATED,
                Domain::Undated => Wire::SALES_FORECAST_DISPOSITION_UNDATED,
                Domain::Omitted => Wire::SALES_FORECAST_DISPOSITION_OMITTED,
                Domain::MissingAmount => Wire::SALES_FORECAST_DISPOSITION_MISSING_AMOUNT,
                Domain::OtherCurrency => Wire::SALES_FORECAST_DISPOSITION_OTHER_CURRENCY,
            }
            .into(),
            ..Default::default()
        })
    }

    fn performance_page_size(raw: u32) -> std::result::Result<usize, ConnectError> {
        match raw {
            0 => Ok(25),
            1..=100 => Ok(raw as usize),
            _ => Err(ConnectError::invalid_argument("page_size exceeds 100")),
        }
    }

    async fn quota_performance(
        &self,
        input: sale_wire::GetSalesQuotaPerformanceRequest,
    ) -> std::result::Result<
        (
            sale_wire::GetSalesQuotaPerformanceResponse,
            crm_sales::PerformanceActuals,
        ),
        ConnectError,
    > {
        let id = Self::id(&input.quota_id)?;
        let now = crate::now_nanos();
        let as_of = input.as_of_unix_nanos.unwrap_or(now);
        if as_of < 0 || as_of > now || input.quota_version.is_some_and(|version| version <= 0) {
            return Err(ConnectError::invalid_argument("SALES_QUOTA_AS_OF_INVALID"));
        }
        let size = if input.page_size == 0 {
            25
        } else {
            input.page_size
        };
        if size > 100 {
            return Err(ConnectError::invalid_argument("page_size exceeds 100"));
        }
        if let Some(after) = input.after.as_option() {
            Self::id(&after.deal_id)?;
            if after.deal_version <= 0
                || input.as_of_unix_nanos.is_none()
                || input.quota_version.is_none()
                || input.event_sequence.is_none()
            {
                return Err(ConnectError::invalid_argument("SALES_QUOTA_CURSOR_INVALID"));
            }
        }
        let db = self.d1()?;
        let record = if let Some(version) = input.quota_version {
            sales_territories::quota_revision(&db, &self.tenant, &id, version).await
        } else {
            sales_territories::get(&db, &self.tenant, sales_territories::Kind::Quota, &id).await
        }
        .map_err(|_| ConnectError::internal("SALES_QUOTA_READ_FAILED"))?
        .ok_or_else(|| ConnectError::not_found("SALES_QUOTA_NOT_FOUND"))?;
        let sales_territories::Record::Quota(quota) = &record else {
            return Err(ConnectError::internal("SALES_QUOTA_STATE_INVALID"));
        };
        let (start, end) = quota
            .period
            .bounds()
            .map_err(|_| ConnectError::internal("SALES_QUOTA_PERIOD_INVALID"))?;
        // The append-only committed event order freezes membership independently
        // of business event time, including events committed late with an older
        // timestamp. Quota revisions and facts are immutable business records.
        use wasm_bindgen::JsValue;
        #[derive(serde::Deserialize)]
        struct Watermark {
            sequence: String,
        }
        let failure = || ConnectError::internal("SALES_QUOTA_FACTS_READ_FAILED");
        let maximum = crate::bind(
            &db,
            "SELECT CAST(COALESCE(MAX(sequence),0) AS TEXT) sequence FROM crm_sales_commercial_event_order WHERE tenant=?",
            &[JsValue::from_str(&self.tenant)],
        )
        .map_err(|_| failure())?
        .first::<Watermark>(None)
        .await
        .map_err(|_| failure())?
        .ok_or_else(failure)?
        .sequence
        .parse::<i64>()
        .map_err(|_| failure())?;
        let event_sequence = input.event_sequence.unwrap_or(maximum);
        if event_sequence < 0
            || event_sequence > maximum
            || (input.event_sequence.is_some()
                && (input.as_of_unix_nanos.is_none() || input.quota_version.is_none()))
        {
            return Err(ConnectError::invalid_argument(
                "SALES_QUOTA_SNAPSHOT_INVALID",
            ));
        }
        // SQL projects stored attributes, filters indexed candidate events and
        // sums unsigned base-10^9 limbs. Each limb sum fits SQLite INTEGER for
        // the physical D1 database capacity; all results cross JS as TEXT.
        // The original Sales owner alone assigns scope, outcomes and signs.
        let dimension = match &quota.scope {
            crm_sales::SalesQuotaScope::Tenant => "NULL",
            crm_sales::SalesQuotaScope::Owner(crm_sales::SalesOwnerScope::Actor(_)) => {
                "json_extract(e.commercial_close_fact,'$.credited_actor')"
            }
            crm_sales::SalesQuotaScope::Owner(crm_sales::SalesOwnerScope::Team(_)) => {
                "json_extract(e.commercial_close_fact,'$.credited_team.id')"
            }
            crm_sales::SalesQuotaScope::Territory(_) => {
                "json_extract(e.commercial_close_fact,'$.credited_territory.id')"
            }
        };
        let mut candidates = format!(
            "WITH candidates AS (SELECT e.deal_id,e.deal_version,e.action,e.occurred_at_unix_nanos,e.commercial_close_fact,\
             json_extract(e.commercial_close_fact,'$.outcome') outcome,\
             json_extract(e.commercial_close_fact,'$.amount.currency_code') currency,\
             json_extract(e.commercial_close_fact,'$.amount.minor_units') minor_units,{dimension} dimension_id \
             FROM crm_sales_deal_events e JOIN crm_sales_commercial_event_order o \
             ON o.tenant=e.tenant AND o.deal_id=e.deal_id AND o.deal_version=e.deal_version \
             WHERE e.tenant=? AND e.action IN ('closed','reopened') \
             AND e.occurred_at_unix_nanos>=CAST(? AS INTEGER) AND e.occurred_at_unix_nanos<CAST(? AS INTEGER) \
             AND e.occurred_at_unix_nanos<=CAST(? AS INTEGER) AND o.sequence<=CAST(? AS INTEGER)"
        );
        let mut values = vec![
            JsValue::from_str(&self.tenant),
            JsValue::from_str(&start.to_string()),
            JsValue::from_str(&end.to_string()),
            JsValue::from_str(&as_of.to_string()),
            JsValue::from_str(&event_sequence.to_string()),
        ];
        if let Some(pipeline) = &quota.pipeline_id {
            candidates.push_str(" AND e.pipeline_id=?");
            values.push(JsValue::from_str(pipeline));
        }
        candidates.push(')');
        values.push(JsValue::from_str(&quota.currency_code));
        let mut aggregate_filter = String::from(" WHERE (currency IS NULL OR currency=?)");
        let mut page_filter = String::from(
            " WHERE commercial_close_fact IS NOT NULL AND (currency IS NULL OR currency=?)",
        );
        if let Some(id) = quota.scope.dimension_id() {
            aggregate_filter.push_str(" AND (dimension_id IS NULL OR dimension_id=?)");
            page_filter.push_str(" AND dimension_id=?");
            values.push(JsValue::from_str(id));
        }
        let aggregate_sql = commercial_group_sql(&candidates, &aggregate_filter, false);
        let groups = crate::bind(&db, &aggregate_sql, &values)
            .map_err(|_| failure())?
            .all()
            .await
            .map_err(|_| failure())?
            .results::<GroupRow>()
            .map_err(|_| failure())?;
        let mut actual = crm_sales::QuotaActuals::default();
        let invalid = || ConnectError::internal("SALES_COMMERCIAL_EVENT_INVALID");
        for row in groups {
            let in_scope = quota.scope.dimension_id().is_none()
                || quota.scope.dimension_id() == row.dimension_id.as_deref();
            actual
                .observe_group(quota, row.fact(in_scope)?)
                .map_err(|_| ConnectError::internal("SALES_QUOTA_TOTAL_INVALID"))?;
        }

        // Independent keyset read: only the requested facts enter WASM.
        // The same frozen membership and filters apply to the full aggregate.
        #[derive(serde::Deserialize)]
        struct FactRow {
            deal_id: String,
            deal_version: i64,
            action: String,
            event_at: String,
            commercial_close_fact: String,
        }
        let mut page_sql = format!(
            "{candidates} SELECT deal_id,deal_version,action,CAST(occurred_at_unix_nanos AS TEXT) event_at,commercial_close_fact FROM candidates{page_filter}"
        );
        if let Some(after) = input.after.as_option() {
            page_sql.push_str(" AND (deal_id,deal_version)>(?,CAST(? AS INTEGER))");
            values.push(JsValue::from_str(&after.deal_id));
            values.push(JsValue::from_str(&after.deal_version.to_string()));
        }
        page_sql.push_str(" ORDER BY deal_id,deal_version LIMIT ?");
        values.push(JsValue::from_f64(f64::from(size + 1)));
        let rows = crate::bind(&db, &page_sql, &values)
            .map_err(|_| failure())?
            .all()
            .await
            .map_err(|_| failure())?
            .results::<FactRow>()
            .map_err(|_| failure())?;
        let mut facts = Vec::with_capacity(rows.len());
        let mut page_actual = crm_sales::QuotaActuals::default();
        for row in rows {
            let fact =
                serde_json::from_str::<crm_sales::CommercialCloseFact>(&row.commercial_close_fact)
                    .map_err(|_| invalid())?;
            let reversal = row.action == "reopened";
            let amount = page_actual
                .observe(quota, Some(&fact), reversal)
                .map_err(|_| invalid())?
                .ok_or_else(invalid)?;
            facts.push(sale_wire::SalesQuotaFact {
                deal_id: row.deal_id,
                deal_version: row.deal_version,
                event_at_unix_nanos: row
                    .event_at
                    .parse()
                    .map_err(|_| ConnectError::internal("SALES_QUOTA_EVENT_TIME_INVALID"))?,
                reversal,
                close_fact: Some(Self::commercial_fact_wire(&fact)?).into(),
                signed_minor_units: amount.map(|value| value.to_string()),
                ..Default::default()
            });
        }
        let next = if facts.len() > size as usize {
            facts.truncate(size as usize);
            facts.last().map(|fact| sale_wire::SalesQuotaFactCursor {
                deal_id: fact.deal_id.clone(),
                deal_version: fact.deal_version,
                ..Default::default()
            })
        } else {
            None
        };
        let overflow = || ConnectError::internal("SALES_QUOTA_TOTAL_OVERFLOW");
        let totals = crm_sales::PerformanceActuals::new(quota, &actual).map_err(|_| overflow())?;
        Ok((
            sale_wire::GetSalesQuotaPerformanceResponse {
                quota: Some(Self::quota_wire(record)?).into(),
                as_of_unix_nanos: as_of,
                event_sequence,
                gross_minor_units: totals.gross.clone(),
                reversal_minor_units: totals.reversals.clone(),
                net_minor_units: totals.net.clone(),
                remaining_minor_units: totals.remaining.clone(),
                attainment_basis_points: totals.attainment.clone(),
                won_count: actual.won_count,
                lost_count: actual.lost_count,
                reversal_count: actual.reversal_count,
                missing_amount_count: actual.missing_amount_count,
                unknown_history_count: actual.unknown_history_count,
                unallocated_count: actual.unallocated_count,
                facts,
                next: next.into(),
                ..Default::default()
            },
            totals,
        ))
    }
}

impl sale_wire::DealService for SalesV2Service {
    #[worker::send]
    async fn create_sales_quota(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::CreateSalesQuotaRequest>,
    ) -> ServiceResult<sale_wire::CreateSalesQuotaResponse> {
        let input = req.to_owned_message();
        let id = Self::id(&input.quota_id)?;
        let (key, expected) = wire_precondition(input.precondition.as_option(), true)?;
        let cfg = input
            .configuration
            .as_option()
            .ok_or_else(|| ConnectError::invalid_argument("SALES_QUOTA_CONFIGURATION_REQUIRED"))?;
        let record = Self::quota_draft(id, cfg)?;
        let db = self.d1()?;
        let saved = sales_territories::mutate(
            &sales_territories::Context {
                db: &db,
                tenant: &self.tenant,
                actor: &self.actor,
                request_id: &self.request_id,
            },
            record,
            &key,
            expected,
        )
        .await
        .map_err(|_| ConnectError::unavailable("SALES_QUOTA_WRITE_FAILED"))?
        .map_err(|failure| failure.connect("CreateSalesQuota", &self.request_id))?;
        let receipt = self.coverage_receipt(&saved);
        RpcResponse::ok(sale_wire::CreateSalesQuotaResponse {
            quota: Some(Self::quota_wire(saved.record)?).into(),
            receipt: Some(receipt).into(),
            ..Default::default()
        })
    }

    #[worker::send]
    async fn update_sales_quota(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::UpdateSalesQuotaRequest>,
    ) -> ServiceResult<sale_wire::UpdateSalesQuotaResponse> {
        let input = req.to_owned_message();
        let id = Self::id(&input.quota_id)?;
        let (key, expected) = wire_precondition(input.precondition.as_option(), false)?;
        let cfg = input
            .configuration
            .as_option()
            .ok_or_else(|| ConnectError::invalid_argument("SALES_QUOTA_CONFIGURATION_REQUIRED"))?;
        let record = Self::quota_draft(id, cfg)?;
        let db = self.d1()?;
        let saved = sales_territories::mutate(
            &sales_territories::Context {
                db: &db,
                tenant: &self.tenant,
                actor: &self.actor,
                request_id: &self.request_id,
            },
            record,
            &key,
            expected,
        )
        .await
        .map_err(|_| ConnectError::unavailable("SALES_QUOTA_WRITE_FAILED"))?
        .map_err(|failure| failure.connect("UpdateSalesQuota", &self.request_id))?;
        let receipt = self.coverage_receipt(&saved);
        RpcResponse::ok(sale_wire::UpdateSalesQuotaResponse {
            quota: Some(Self::quota_wire(saved.record)?).into(),
            receipt: Some(receipt).into(),
            ..Default::default()
        })
    }

    #[worker::send]
    async fn get_sales_quota(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::GetSalesQuotaRequest>,
    ) -> ServiceResult<sale_wire::GetSalesQuotaResponse> {
        let input = req.to_owned_message();
        let id = Self::id(&input.quota_id)?;
        let db = self.d1()?;
        let kind = sales_territories::Kind::Quota;
        let record = if let Some(version) = input.version {
            if version <= 0 {
                return Err(ConnectError::invalid_argument(
                    "SALES_QUOTA_VERSION_INVALID",
                ));
            }
            sales_territories::quota_revision(&db, &self.tenant, &id, version).await
        } else {
            sales_territories::get(&db, &self.tenant, kind, &id).await
        }
        .map_err(|_| ConnectError::internal("SALES_QUOTA_READ_FAILED"))?
        .ok_or_else(|| ConnectError::not_found("SALES_QUOTA_NOT_FOUND"))?;
        let confirmed_mutation = match receipt_key(input.receipt_idempotency_key.as_deref())? {
            None => None,
            Some(key) => {
                let row = crud_receipt(&db, &self.tenant, key, CrudReceiptOwner::Quota)
                    .await
                    .map_err(|_| ConnectError::unavailable("SALES_QUOTA_RECEIPT_READ_FAILED"))?;
                crud_commit_evidence(row, &id, &self.request_id, |state| {
                    let record = kind.decode(state)?;
                    Ok((record.id().to_owned(), record.version()))
                })
                .map_err(|_| ConnectError::unavailable("SALES_QUOTA_RECEIPT_DECODE_FAILED"))?
            }
        };
        RpcResponse::ok(sale_wire::GetSalesQuotaResponse {
            quota: Some(Self::quota_wire(record)?).into(),
            confirmed_mutation: confirmed_mutation.into(),
            ..Default::default()
        })
    }

    #[worker::send]
    async fn list_sales_quotas(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::ListSalesQuotasRequest>,
    ) -> ServiceResult<sale_wire::ListSalesQuotasResponse> {
        let size = if req.page.page_size == 0 {
            25
        } else {
            req.page.page_size
        };
        if size > 100 {
            return Err(ConnectError::invalid_argument("page_size exceeds 100"));
        }
        if !req.page.page_token.is_empty() {
            Self::id(req.page.page_token)?;
        }
        let (items, next) = sales_territories::list(
            &self.d1()?,
            &self.tenant,
            sales_territories::Kind::Quota,
            sales_territories::Filter {
                after: req.page.page_token,
                limit: size as usize,
                include_archived: req.include_archived,
                territory_id: None,
                primary_owner: None,
            },
        )
        .await
        .map_err(|_| ConnectError::internal("SALES_QUOTA_LIST_FAILED"))?;
        RpcResponse::ok(sale_wire::ListSalesQuotasResponse {
            quotas: items
                .into_iter()
                .map(Self::quota_wire)
                .collect::<std::result::Result<Vec<_>, _>>()?,
            page_info: Some(core_v2::PageInfo {
                next_page_token: next.unwrap_or_default(),
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
    }

    #[worker::send]
    async fn get_sales_scorecard(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::GetSalesScorecardRequest>,
    ) -> ServiceResult<sale_wire::GetSalesScorecardResponse> {
        use crm_sales::{QuotaCadence, QuotaDate, Scorecard, ScorecardAxis};
        use sale_wire::SalesScorecardAxis as Axis;
        use wasm_bindgen::JsValue;
        let input = req.to_owned_message();
        let (axis, dimension) = match input.axis {
            v if v == Axis::SALES_SCORECARD_AXIS_ORGANIZATION => {
                (ScorecardAxis::Organization, "NULL")
            }
            v if v == Axis::SALES_SCORECARD_AXIS_ACTOR => (
                ScorecardAxis::Actor,
                "json_extract(e.commercial_close_fact,'$.credited_actor')",
            ),
            v if v == Axis::SALES_SCORECARD_AXIS_TEAM => (
                ScorecardAxis::Team,
                "json_extract(e.commercial_close_fact,'$.credited_team.id')",
            ),
            v if v == Axis::SALES_SCORECARD_AXIS_TERRITORY => (
                ScorecardAxis::Territory,
                "json_extract(e.commercial_close_fact,'$.credited_territory.id')",
            ),
            _ => return Err(ConnectError::invalid_argument("SALES_SCORECARD_AXIS_INVALID")),
        };
        let (cadence, cadence_key) = match input.cadence {
            v if v == sale_wire::SalesQuotaCadence::SALES_QUOTA_CADENCE_MONTHLY => {
                (QuotaCadence::Monthly, "monthly")
            }
            v if v == sale_wire::SalesQuotaCadence::SALES_QUOTA_CADENCE_QUARTERLY => {
                (QuotaCadence::Quarterly, "quarterly")
            }
            _ => return Err(ConnectError::invalid_argument("SALES_SCORECARD_CADENCE_INVALID")),
        };
        let date = input
            .first_date
            .as_option()
            .ok_or_else(|| ConnectError::invalid_argument("SALES_SCORECARD_DATE_REQUIRED"))?;
        let invalid_date = || ConnectError::invalid_argument("SALES_SCORECARD_DATE_INVALID");
        let mut card = Scorecard::new(
            axis,
            input.currency_code.clone(),
            input.pipeline_id.clone(),
            QuotaDate {
                year: date.year,
                month: u8::try_from(date.month).map_err(|_| invalid_date())?,
                day: u8::try_from(date.day).map_err(|_| invalid_date())?,
            },
            input.time_zone.clone(),
            cadence,
            input.period_count as usize,
        )
        .map_err(|e| ConnectError::invalid_argument(e.code))?;
        if let Some(detail) = input.detail.as_option() {
            return RpcResponse::ok(
                self.scorecard_detail(&card, &input, detail, dimension).await?,
            );
        }
        let failure = || ConnectError::internal("SALES_SCORECARD_READ_FAILED");
        let capacity = || ConnectError::resource_exhausted("SALES_SCORECARD_CAPACITY");
        let domain_failure = |e: crm_module_sdk::SdkError| {
            if e.code == "SALES_SCORECARD_CAPACITY" {
                capacity()
            } else {
                ConnectError::internal(e.code)
            }
        };
        let db = self.d1()?;
        let as_of = crate::now_nanos();
        let ranges = card
            .buckets()
            .iter()
            .map(|b| [&b.period.start_unix_nanos, &b.period.end_exclusive_unix_nanos])
            .collect::<Vec<_>>();
        let ranges = serde_json::to_string(&ranges).map_err(|_| failure())?;
        let pipeline = input
            .pipeline_id
            .as_deref()
            .map_or(JsValue::NULL, JsValue::from_str);
        // D1 executes this read batch transactionally. Calendar instants come
        // from the original Sales owner, never from SQL date calculations.
        let candidates = format!(
            "WITH periods AS (SELECT CAST(key AS INTEGER) bucket,CAST(json_extract(value,'$[0]') AS INTEGER) lo,CAST(json_extract(value,'$[1]') AS INTEGER) hi FROM json_each(?1)), candidates AS (SELECT p.bucket,e.action,e.commercial_close_fact,\
             json_extract(e.commercial_close_fact,'$.outcome') outcome,\
             json_extract(e.commercial_close_fact,'$.amount.currency_code') currency,\
             json_extract(e.commercial_close_fact,'$.amount.minor_units') minor_units,{dimension} dimension_id \
             FROM periods p JOIN crm_sales_deal_events e ON e.occurred_at_unix_nanos>=p.lo AND e.occurred_at_unix_nanos<p.hi \
             JOIN crm_sales_commercial_event_order o ON o.tenant=e.tenant AND o.deal_id=e.deal_id AND o.deal_version=e.deal_version \
             WHERE e.tenant=?2 AND e.action IN ('closed','reopened') AND e.occurred_at_unix_nanos<=CAST(?3 AS INTEGER) AND (?4 IS NULL OR e.pipeline_id=?4))"
        );
        let aggregate = format!(
            "{} LIMIT 8193",
            commercial_group_sql(&candidates, " WHERE (currency IS NULL OR currency=?5)", true)
        );
        let quota_sql = "SELECT state FROM crm_sales_quota_records WHERE tenant=?1 AND json_extract(state,'$.published')=1 AND json_extract(state,'$.currency_code')=?2 AND json_extract(state,'$.pipeline_id') IS ?3 AND json_extract(state,'$.period.time_zone')=?4 AND CAST(json_extract(state,'$.period.start_unix_nanos') AS INTEGER)>=CAST(?5 AS INTEGER) AND CAST(json_extract(state,'$.period.start_unix_nanos') AS INTEGER)<CAST(?6 AS INTEGER) AND json_extract(state,'$.period.cadence')=?7 LIMIT 2049";
        let first = card.buckets().first().ok_or_else(failure)?;
        let last = card.buckets().last().ok_or_else(failure)?;
        let statements = vec![
            crate::bind(
                &db,
                "SELECT CAST(COALESCE(MAX(sequence),0) AS TEXT) sequence FROM crm_sales_commercial_event_order WHERE tenant=?",
                &[JsValue::from_str(&self.tenant)],
            )
            .map_err(|_| failure())?,
            crate::bind(
                &db,
                &aggregate,
                &[
                    JsValue::from_str(&ranges),
                    JsValue::from_str(&self.tenant),
                    JsValue::from_str(&as_of.to_string()),
                    pipeline.clone(),
                    JsValue::from_str(&input.currency_code),
                ],
            )
            .map_err(|_| failure())?,
            crate::bind(
                &db,
                quota_sql,
                &[
                    JsValue::from_str(&self.tenant),
                    JsValue::from_str(&input.currency_code),
                    pipeline,
                    JsValue::from_str(&input.time_zone),
                    JsValue::from_str(&first.period.start_unix_nanos),
                    JsValue::from_str(&last.period.end_exclusive_unix_nanos),
                    JsValue::from_str(cadence_key),
                ],
            )
            .map_err(|_| failure())?,
        ];
        let results = db.batch(statements).await.map_err(|_| failure())?;
        if results.len() != 3 || !results.iter().all(worker::d1::D1Result::success) {
            return Err(failure());
        }
        #[derive(serde::Deserialize)]
        struct Watermark {
            sequence: String,
        }
        #[derive(serde::Deserialize)]
        struct QuotaRow {
            state: String,
        }
        let event_sequence = results[0]
            .results::<Watermark>()
            .map_err(|_| failure())?
            .first()
            .ok_or_else(failure)?
            .sequence
            .parse::<i64>()
            .map_err(|_| failure())?;
        let groups = results[1].results::<GroupRow>().map_err(|_| failure())?;
        let quotas = results[2].results::<QuotaRow>().map_err(|_| failure())?;
        if groups.len() > 8192 || quotas.len() > crm_sales::MAX_SCORECARD_CELLS {
            return Err(capacity());
        }
        for row in groups {
            card.observe_group(row.bucket, row.fact(true)?)
                .map_err(domain_failure)?;
        }
        for row in quotas {
            let quota = crm_sales::SalesQuota::decode(&row.state).map_err(domain_failure)?;
            card.attach_quota(quota).map_err(domain_failure)?;
        }
        // Current catalogs only add zero-activity rows. They never determine
        // the credited owner of a historical event.
        let limit = crm_sales::MAX_SCORECARD_CELLS / card.buckets().len();
        let active_ids = match axis {
            ScorecardAxis::Organization => Vec::new(),
            ScorecardAxis::Actor => {
                let (members, next) = access_members::list(&db, &self.tenant, "", limit, false)
                    .await
                    .map_err(|_| failure())?;
                if next.is_some() {
                    return Err(capacity());
                }
                members
                    .into_iter()
                    .filter(|m| !m.email.starts_with("service:"))
                    .map(|m| m.email)
                    .collect()
            }
            ScorecardAxis::Team => {
                let (teams, next) = access_teams::list(&db, &self.tenant, "", limit, false)
                    .await
                    .map_err(|_| failure())?;
                if next.is_some() {
                    return Err(capacity());
                }
                teams.into_iter().map(|t| t.team_id).collect()
            }
            ScorecardAxis::Territory => {
                let (territories, next) = sales_territories::list(
                    &db,
                    &self.tenant,
                    sales_territories::Kind::Territory,
                    sales_territories::Filter {
                        after: "",
                        limit,
                        include_archived: false,
                        territory_id: None,
                        primary_owner: None,
                    },
                )
                .await
                .map_err(|_| failure())?;
                if next.is_some() {
                    return Err(capacity());
                }
                territories.into_iter().map(|t| t.id().to_owned()).collect()
            }
        };
        card.complete_dimensions(active_ids).map_err(domain_failure)?;
        let cell_wire = |cell: crm_sales::ScorecardCell|
         -> std::result::Result<sale_wire::SalesScorecardCell, ConnectError> {
            let progress = cell.target_progress().map_err(domain_failure)?;
            let win_rate = cell.win_rate_basis_points();
            let a = cell.actual;
            let net = a.net().map_err(domain_failure)?;
            Ok(sale_wire::SalesScorecardCell {
                gross_minor_units: a.gross.to_string(),
                reversal_minor_units: a.reversals.to_string(),
                net_minor_units: net.to_string(),
                won_count: a.won_count,
                lost_count: a.lost_count,
                reversal_count: a.reversal_count,
                missing_amount_count: a.missing_amount_count,
                unknown_history_count: a.unknown_history_count,
                unallocated_count: a.unallocated_count,
                win_rate_basis_points: win_rate,
                source_event_count: cell.source_event_count,
                quota: cell
                    .quota
                    .map(|q| Self::quota_wire(sales_territories::Record::Quota(q)))
                    .transpose()?
                    .into(),
                remaining_minor_units: progress.map(|(remaining, _)| remaining.to_string()),
                attainment_basis_points: progress.map(|(_, attainment)| attainment.to_string()),
                ..Default::default()
            })
        };
        let date_wire = |d: QuotaDate| core_v2::CalendarDate {
            year: d.year,
            month: u32::from(d.month),
            day: u32::from(d.day),
            ..Default::default()
        };
        let buckets = card
            .into_buckets()
            .into_iter()
            .map(|b| {
                let (start, end) = b.period.bounds().map_err(domain_failure)?;
                Ok(sale_wire::SalesScorecardBucket {
                    period: Some(sale_wire::SalesQuotaPeriod {
                        start_date: Some(date_wire(b.period.start_date)).into(),
                        end_date: Some(date_wire(b.period.end_date)).into(),
                        time_zone: b.period.time_zone,
                        cadence: input.cadence,
                        ..Default::default()
                    })
                    .into(),
                    start_unix_nanos: start,
                    end_exclusive_unix_nanos: end,
                    organization: Some(cell_wire(b.organization)?).into(),
                    dimensions: b
                        .dimensions
                        .into_iter()
                        .map(|(dimension_id, cell)| {
                            Ok(sale_wire::SalesScorecardDimension {
                                dimension_id,
                                cell: Some(cell_wire(cell)?).into(),
                                ..Default::default()
                            })
                        })
                        .collect::<std::result::Result<Vec<_>, ConnectError>>()?,
                    ..Default::default()
                })
            })
            .collect::<std::result::Result<Vec<_>, ConnectError>>()?;
        RpcResponse::ok(sale_wire::GetSalesScorecardResponse {
            as_of_unix_nanos: as_of,
            event_sequence,
            currency_code: input.currency_code,
            axis: input.axis,
            buckets,
            ..Default::default()
        })
    }

    #[worker::send]
    async fn get_sales_quota_performance(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::GetSalesQuotaPerformanceRequest>,
    ) -> ServiceResult<sale_wire::GetSalesQuotaPerformanceResponse> {
        let (result, _) = self.quota_performance(req.to_owned_message()).await?;
        RpcResponse::ok(result)
    }

    #[worker::send]
    async fn preview_sales_performance_report(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::PreviewSalesPerformanceReportRequest>,
    ) -> ServiceResult<sale_wire::PreviewSalesPerformanceReportResponse> {
        let input = req.to_owned_message();
        let id = Self::id(&input.quota_id)?;
        let size = Self::performance_page_size(input.page_size)?;
        let (report, sources) = self
            .capture_performance(
                &id,
                input.quota_version,
                String::new(),
                "Live forecast".into(),
                Some(size + 1),
            )
            .await?;
        let more = sources.len() > size;
        RpcResponse::ok(sale_wire::PreviewSalesPerformanceReportResponse {
            report: Some(Self::report_wire(report)?).into(),
            more_forecast_sources: more,
            forecast_sources: sources
                .into_iter()
                .take(size)
                .map(Self::forecast_source_wire)
                .collect::<std::result::Result<Vec<_>, _>>()?,
            ..Default::default()
        })
    }

    #[worker::send]
    async fn save_sales_performance_report(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::SaveSalesPerformanceReportRequest>,
    ) -> ServiceResult<sale_wire::SaveSalesPerformanceReportResponse> {
        let input = req.to_owned_message();
        let id = Self::id(&input.report_id)?;
        let quota_id = Self::id(&input.quota_id)?;
        let (key, _) = wire_precondition(input.precondition.as_option(), true)?;
        let digest = sales_performance::digest(&id, &input.name, &quota_id, input.quota_version);
        let db = self.d1()?;
        let ctx = sales_performance::SaveContext {
            db: &db,
            tenant: &self.tenant,
            actor: &self.actor,
            request_id: &self.request_id,
            key: &key,
            digest: &digest,
        };
        let preparation = || {
            diagnostics::mutation_preparation_failure(
                "SaveSalesPerformanceReport",
                &self.request_id,
            )
        };
        let prior = sales_performance::replay(&ctx)
            .await
            .map_err(|_| preparation())?;
        let saved = if let Some(saved) = prior {
            saved
        } else {
            let (report, sources) = self
                .capture_performance(&quota_id, input.quota_version, id, input.name, None)
                .await?;
            sales_performance::save(&ctx, report, &sources)
                .await
                .map_err(|_| preparation())?
        }
        .map_err(|failure| failure.connect("SaveSalesPerformanceReport", &self.request_id))?;
        let receipt = diagnostics::receipt(
            "SaveSalesPerformanceReport",
            &self.request_id,
            Some(&saved.original_request_id),
            1,
            saved.replayed,
            None,
        );
        RpcResponse::ok(sale_wire::SaveSalesPerformanceReportResponse {
            report: Some(Self::report_wire(saved.report)?).into(),
            receipt: Some(receipt).into(),
            ..Default::default()
        })
    }

    #[worker::send]
    async fn get_sales_performance_report(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::GetSalesPerformanceReportRequest>,
    ) -> ServiceResult<sale_wire::GetSalesPerformanceReportResponse> {
        let input = req.to_owned_message();
        let id = Self::id(&input.report_id)?;
        let size = Self::performance_page_size(input.page_size)?;
        if !input.forecast_after.is_empty() {
            Self::id(&input.forecast_after)?;
        }
        if let Some(after) = input.actual_after.as_option() {
            Self::id(&after.deal_id)?;
            if after.deal_version <= 0 {
                return Err(ConnectError::invalid_argument("SALES_QUOTA_CURSOR_INVALID"));
            }
        }
        let db = self.d1()?;
        let report = sales_performance::get(&db, &self.tenant, &id)
            .await
            .map_err(|_| ConnectError::internal("SALES_PERFORMANCE_REPORT_READ_FAILED"))?
            .ok_or_else(|| ConnectError::not_found("SALES_PERFORMANCE_REPORT_NOT_FOUND"))?;
        let comparison = if let Some(baseline_id) = input.baseline_report_id.as_deref() {
            let baseline_id = Self::id(baseline_id)?;
            let baseline = sales_performance::get(&db, &self.tenant, &baseline_id)
                .await
                .map_err(|_| ConnectError::internal("SALES_PERFORMANCE_REPORT_READ_FAILED"))?
                .ok_or_else(|| ConnectError::not_found("SALES_PERFORMANCE_REPORT_NOT_FOUND"))?;
            let (status, delta) = report
                .compare(&baseline)
                .map_err(|e| ConnectError::internal(e.code))?;
            use crm_sales::ComparisonStatus as Domain;
            use sale_wire::SalesPerformanceComparisonStatus as Wire;
            Some(sale_wire::SalesPerformanceComparison {
                status: match status {
                    Domain::Comparable => Wire::SALES_PERFORMANCE_COMPARISON_STATUS_COMPARABLE,
                    Domain::DifferentSeries => {
                        Wire::SALES_PERFORMANCE_COMPARISON_STATUS_DIFFERENT_SERIES
                    }
                    Domain::DifferentFormula => {
                        Wire::SALES_PERFORMANCE_COMPARISON_STATUS_DIFFERENT_FORMULA
                    }
                }
                .into(),
                baseline: Some(Self::report_wire(baseline)?).into(),
                delta: delta
                    .map(|d| sale_wire::SalesPerformanceDelta {
                        target_minor_units: d.target,
                        gross_minor_units: d.gross,
                        reversal_minor_units: d.reversals,
                        net_minor_units: d.net,
                        remaining_minor_units: d.remaining,
                        attainment_basis_points: d.attainment,
                        open_minor_units: d.open,
                        weighted_minor_units_times_basis_points: d.weighted,
                        open_coverage_basis_points: d.open_coverage,
                        weighted_coverage_basis_points: d.weighted_coverage,
                        won_count: d.won_count,
                        lost_count: d.lost_count,
                        reversal_count: d.reversal_count,
                        missing_amount_count: d.missing_amount_count,
                        unknown_history_count: d.unknown_history_count,
                        unallocated_count: d.unallocated_count,
                        ..Default::default()
                    })
                    .into(),
                ..Default::default()
            })
        } else {
            None
        };
        let confirmed_mutation = match receipt_key(input.receipt_idempotency_key.as_deref())? {
            None => None,
            Some(key) => {
                let row = crud_receipt(&db, &self.tenant, key, CrudReceiptOwner::PerformanceReport)
                    .await
                    .map_err(|_| {
                        ConnectError::unavailable("SALES_PERFORMANCE_RECEIPT_READ_FAILED")
                    })?;
                crud_commit_evidence(row, &id, &self.request_id, |state| {
                    Ok((sales_performance::decode(state)?.report_id, 1))
                })
                .map_err(|_| ConnectError::unavailable("SALES_PERFORMANCE_RECEIPT_DECODE_FAILED"))?
            }
        };
        let count = sales_performance::membership_count(
            &db,
            &self.tenant,
            &report.quota,
            &report.captured_at,
            &report.event_sequence,
        )
        .await
        .map_err(|_| ConnectError::internal("SALES_PERFORMANCE_MEMBERSHIP_READ_FAILED"))?;
        let mut actual_history_available = false;
        let mut actual_facts = Vec::new();
        let mut actual_next = Default::default();
        // The stored totals are never replaced by a fresh calculation.
        // Missing history (e.g. governed retention) disables drilldown.
        if count == report.actual_candidate_count {
            let cutoff = report
                .captured_at
                .parse()
                .map_err(|_| ConnectError::internal("SALES_PERFORMANCE_REPORT_INVALID"))?;
            let sequence = report
                .event_sequence
                .parse()
                .map_err(|_| ConnectError::internal("SALES_PERFORMANCE_REPORT_INVALID"))?;
            if let Ok((live, totals)) = self
                .quota_performance(sale_wire::GetSalesQuotaPerformanceRequest {
                    quota_id: report.quota.quota_id.clone(),
                    quota_version: Some(report.quota.version),
                    as_of_unix_nanos: Some(cutoff),
                    event_sequence: Some(sequence),
                    after: input.actual_after,
                    page_size: size as u32,
                    ..Default::default()
                })
                .await
            {
                let after_count = sales_performance::membership_count(
                    &db,
                    &self.tenant,
                    &report.quota,
                    &report.captured_at,
                    &report.event_sequence,
                )
                .await
                .map_err(|_| ConnectError::internal("SALES_PERFORMANCE_MEMBERSHIP_READ_FAILED"))?;
                if totals == report.actuals && after_count == count {
                    actual_history_available = true;
                    actual_facts = live.facts;
                    actual_next = live.next;
                }
            }
        }
        let (sources, next) = sales_performance::forecast_page(
            &db,
            &self.tenant,
            &report,
            &input.forecast_after,
            size,
        )
        .await
        .map_err(|_| ConnectError::internal("SALES_PERFORMANCE_FORECAST_READ_FAILED"))?;
        RpcResponse::ok(sale_wire::GetSalesPerformanceReportResponse {
            report: Some(Self::report_wire(report)?).into(),
            confirmed_mutation: confirmed_mutation.into(),
            actual_history_available,
            actual_facts,
            actual_next,
            forecast_sources: sources
                .into_iter()
                .map(Self::forecast_source_wire)
                .collect::<std::result::Result<Vec<_>, _>>()?,
            forecast_next: next,
            comparison: comparison.into(),
            ..Default::default()
        })
    }

    #[worker::send]
    async fn list_sales_performance_reports(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::ListSalesPerformanceReportsRequest>,
    ) -> ServiceResult<sale_wire::ListSalesPerformanceReportsResponse> {
        let input = req.to_owned_message();
        let size = Self::performance_page_size(input.page.page_size)?;
        if !input.page.page_token.is_empty() {
            Self::id(&input.page.page_token)?;
        }
        if let Some(id) = &input.quota_id {
            Self::id(id)?;
        }
        let (reports, next) = sales_performance::list(
            &self.d1()?,
            &self.tenant,
            input.quota_id.as_deref(),
            &input.page.page_token,
            size,
        )
        .await
        .map_err(|_| ConnectError::internal("SALES_PERFORMANCE_REPORT_LIST_FAILED"))?;
        RpcResponse::ok(sale_wire::ListSalesPerformanceReportsResponse {
            reports: reports
                .into_iter()
                .map(Self::report_wire)
                .collect::<std::result::Result<Vec<_>, _>>()?,
            page_info: Some(core_v2::PageInfo {
                next_page_token: next,
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
    }

    #[worker::send]
    async fn create_sales_routing_rule(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::CreateSalesRoutingRuleRequest>,
    ) -> ServiceResult<sale_wire::CreateSalesRoutingRuleResponse> {
        let input = req.to_owned_message();
        let id = Self::id(&input.rule_id)?;
        let (key, expected) = wire_precondition(input.precondition.as_option(), true)?;
        let cfg = input.configuration.as_option().ok_or_else(|| {
            ConnectError::invalid_argument("SALES_ROUTING_CONFIGURATION_REQUIRED")
        })?;
        let record = Self::routing_draft(id, cfg)?;
        let db = self.d1()?;
        let saved = sales_territories::mutate(
            &sales_territories::Context {
                db: &db,
                tenant: &self.tenant,
                actor: &self.actor,
                request_id: &self.request_id,
            },
            record,
            &key,
            expected,
        )
        .await
        .map_err(|_| ConnectError::unavailable("SALES_ROUTING_WRITE_FAILED"))?
        .map_err(|failure| failure.connect("CreateSalesRoutingRule", &self.request_id))?;
        let receipt = self.coverage_receipt(&saved);
        RpcResponse::ok(sale_wire::CreateSalesRoutingRuleResponse {
            rule: Some(Self::routing_rule_wire(saved.record)?).into(),
            receipt: Some(receipt).into(),
            ..Default::default()
        })
    }

    #[worker::send]
    async fn update_sales_routing_rule(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::UpdateSalesRoutingRuleRequest>,
    ) -> ServiceResult<sale_wire::UpdateSalesRoutingRuleResponse> {
        let input = req.to_owned_message();
        let id = Self::id(&input.rule_id)?;
        let (key, expected) = wire_precondition(input.precondition.as_option(), false)?;
        let cfg = input.configuration.as_option().ok_or_else(|| {
            ConnectError::invalid_argument("SALES_ROUTING_CONFIGURATION_REQUIRED")
        })?;
        let record = Self::routing_draft(id, cfg)?;
        let db = self.d1()?;
        let saved = sales_territories::mutate(
            &sales_territories::Context {
                db: &db,
                tenant: &self.tenant,
                actor: &self.actor,
                request_id: &self.request_id,
            },
            record,
            &key,
            expected,
        )
        .await
        .map_err(|_| ConnectError::unavailable("SALES_ROUTING_WRITE_FAILED"))?
        .map_err(|failure| failure.connect("UpdateSalesRoutingRule", &self.request_id))?;
        let receipt = self.coverage_receipt(&saved);
        RpcResponse::ok(sale_wire::UpdateSalesRoutingRuleResponse {
            rule: Some(Self::routing_rule_wire(saved.record)?).into(),
            receipt: Some(receipt).into(),
            ..Default::default()
        })
    }

    #[worker::send]
    async fn get_sales_routing_rule(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::GetSalesRoutingRuleRequest>,
    ) -> ServiceResult<sale_wire::GetSalesRoutingRuleResponse> {
        let input = req.to_owned_message();
        let id = Self::id(&input.rule_id)?;
        let db = self.d1()?;
        let kind = sales_territories::Kind::RoutingRule;
        let record = sales_territories::get(&db, &self.tenant, kind, &id)
            .await
            .map_err(|_| ConnectError::internal("SALES_ROUTING_READ_FAILED"))?
            .ok_or_else(|| ConnectError::not_found("SALES_ROUTING_RULE_NOT_FOUND"))?;
        let confirmed_mutation = match receipt_key(input.receipt_idempotency_key.as_deref())? {
            None => None,
            Some(key) => {
                let row = crud_receipt(&db, &self.tenant, key, CrudReceiptOwner::RoutingRule)
                    .await
                    .map_err(|_| ConnectError::unavailable("SALES_ROUTING_RECEIPT_READ_FAILED"))?;
                crud_commit_evidence(row, &id, &self.request_id, |state| {
                    let record = kind.decode(state)?;
                    Ok((record.id().to_owned(), record.version()))
                })
                .map_err(|_| ConnectError::unavailable("SALES_ROUTING_RECEIPT_DECODE_FAILED"))?
            }
        };
        RpcResponse::ok(sale_wire::GetSalesRoutingRuleResponse {
            rule: Some(Self::routing_rule_wire(record)?).into(),
            confirmed_mutation: confirmed_mutation.into(),
            ..Default::default()
        })
    }

    #[worker::send]
    async fn list_sales_routing_rules(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::ListSalesRoutingRulesRequest>,
    ) -> ServiceResult<sale_wire::ListSalesRoutingRulesResponse> {
        let size = if req.page.page_size == 0 {
            25
        } else {
            req.page.page_size
        };
        if size > 100 {
            return Err(ConnectError::invalid_argument("page_size exceeds 100"));
        }
        if !req.page.page_token.is_empty() {
            Self::id(req.page.page_token)?;
        }
        let (items, next) = sales_territories::list(
            &self.d1()?,
            &self.tenant,
            sales_territories::Kind::RoutingRule,
            sales_territories::Filter {
                after: req.page.page_token,
                limit: size as usize,
                include_archived: req.include_archived,
                territory_id: None,
                primary_owner: None,
            },
        )
        .await
        .map_err(|_| ConnectError::internal("SALES_ROUTING_LIST_FAILED"))?;
        RpcResponse::ok(sale_wire::ListSalesRoutingRulesResponse {
            rules: items
                .into_iter()
                .map(Self::routing_rule_wire)
                .collect::<std::result::Result<Vec<_>, _>>()?,
            page_info: Some(core_v2::PageInfo {
                next_page_token: next.unwrap_or_default(),
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
    }

    #[worker::send]
    async fn preview_sales_routing(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::PreviewSalesRoutingRequest>,
    ) -> ServiceResult<sale_wire::PreviewSalesRoutingResponse> {
        let input = req.to_owned_message();
        let stage = input
            .stage
            .as_option()
            .ok_or_else(|| ConnectError::invalid_argument("stage required"))?;
        let (pipeline_id, stage_id) = Self::stage(stage);
        let facts = crm_sales::SalesRoutingFacts {
            account_id: Self::id(&input.account_id)?,
            pipeline_id,
            stage_id,
            territory_id: input.territory_id.clone(),
            currency_code: input.amount.as_option().map(|m| m.currency_code.clone()),
            amount_minor_units: input.amount.as_option().map(|m| m.minor_units),
        };
        let result = sales_routing::resolve(&self.d1()?, &self.tenant, &facts)
            .await
            .map_err(|_| ConnectError::internal("SALES_ROUTING_PREVIEW_FAILED"))?
            .map_err(|e| {
                if e.category == crm_module_sdk::ErrorCategory::Conflict {
                    ConnectError::new(connectrpc::ErrorCode::Aborted, e.code)
                } else {
                    ConnectError::invalid_argument(e.code)
                }
            })?;
        RpcResponse::ok(sale_wire::PreviewSalesRoutingResponse {
            decision: Some(Self::routing_decision_wire(&result.decision)).into(),
            expected_decision: result.fingerprint,
            ..Default::default()
        })
    }

    #[worker::send]
    async fn create_territory(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::CreateTerritoryRequest>,
    ) -> ServiceResult<sale_wire::CreateTerritoryResponse> {
        let input = req.to_owned_message();
        let id = Self::id(&input.territory_id)?;
        let (key, expected) = wire_precondition(input.precondition.as_option(), true)?;
        let cfg = input.configuration.as_option().ok_or_else(|| {
            ConnectError::invalid_argument("SALES_COVERAGE_CONFIGURATION_REQUIRED")
        })?;
        let record = Self::territory_draft(id, cfg)?;
        let db = self.d1()?;
        let saved = sales_territories::mutate(
            &sales_territories::Context {
                db: &db,
                tenant: &self.tenant,
                actor: &self.actor,
                request_id: &self.request_id,
            },
            record,
            &key,
            expected,
        )
        .await
        .map_err(|_| ConnectError::unavailable("SALES_COVERAGE_WRITE_FAILED"))?
        .map_err(|failure| failure.connect("CreateTerritory", &self.request_id))?;
        let receipt = self.coverage_receipt(&saved);
        RpcResponse::ok(sale_wire::CreateTerritoryResponse {
            territory: Some(Self::territory_wire(saved.record)?).into(),
            receipt: Some(receipt).into(),
            ..Default::default()
        })
    }

    #[worker::send]
    async fn update_territory(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::UpdateTerritoryRequest>,
    ) -> ServiceResult<sale_wire::UpdateTerritoryResponse> {
        let input = req.to_owned_message();
        let id = Self::id(&input.territory_id)?;
        let (key, expected) = wire_precondition(input.precondition.as_option(), false)?;
        let cfg = input.configuration.as_option().ok_or_else(|| {
            ConnectError::invalid_argument("SALES_COVERAGE_CONFIGURATION_REQUIRED")
        })?;
        let record = Self::territory_draft(id, cfg)?;
        let db = self.d1()?;
        let saved = sales_territories::mutate(
            &sales_territories::Context {
                db: &db,
                tenant: &self.tenant,
                actor: &self.actor,
                request_id: &self.request_id,
            },
            record,
            &key,
            expected,
        )
        .await
        .map_err(|_| ConnectError::unavailable("SALES_COVERAGE_WRITE_FAILED"))?
        .map_err(|failure| failure.connect("UpdateTerritory", &self.request_id))?;
        let receipt = self.coverage_receipt(&saved);
        RpcResponse::ok(sale_wire::UpdateTerritoryResponse {
            territory: Some(Self::territory_wire(saved.record)?).into(),
            receipt: Some(receipt).into(),
            ..Default::default()
        })
    }

    #[worker::send]
    async fn get_territory(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::GetTerritoryRequest>,
    ) -> ServiceResult<sale_wire::GetTerritoryResponse> {
        let input = req.to_owned_message();
        let id = Self::id(&input.territory_id)?;
        let db = self.d1()?;
        let kind = sales_territories::Kind::Territory;
        let record = sales_territories::get(&db, &self.tenant, kind, &id)
            .await
            .map_err(|_| ConnectError::internal("Sales coverage read failed"))?;
        let record = record.ok_or_else(|| ConnectError::not_found("SALES_TERRITORY_NOT_FOUND"))?;
        let confirmed_mutation = match receipt_key(input.receipt_idempotency_key.as_deref())? {
            None => None,
            Some(key) => {
                let row = crud_receipt(&db, &self.tenant, key, CrudReceiptOwner::Territory)
                    .await
                    .map_err(|_| ConnectError::unavailable("SALES_COVERAGE_RECEIPT_READ_FAILED"))?;
                crud_commit_evidence(row, &id, &self.request_id, |state| {
                    let record = kind.decode(state)?;
                    Ok((record.id().to_owned(), record.version()))
                })
                .map_err(|_| ConnectError::unavailable("SALES_COVERAGE_RECEIPT_DECODE_FAILED"))?
            }
        };
        RpcResponse::ok(sale_wire::GetTerritoryResponse {
            territory: Some(Self::territory_wire(record)?).into(),
            confirmed_mutation: confirmed_mutation.into(),
            ..Default::default()
        })
    }

    #[worker::send]
    async fn list_territories(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::ListTerritoriesRequest>,
    ) -> ServiceResult<sale_wire::ListTerritoriesResponse> {
        let size = if req.page.page_size == 0 {
            25
        } else {
            req.page.page_size
        };
        if size > 100 {
            return Err(ConnectError::invalid_argument("page_size exceeds 100"));
        }
        if !req.page.page_token.is_empty() {
            Self::id(req.page.page_token)?;
        }
        let (items, next) = sales_territories::list(
            &self.d1()?,
            &self.tenant,
            sales_territories::Kind::Territory,
            sales_territories::Filter {
                after: req.page.page_token,
                limit: size as usize,
                include_archived: req.include_archived,
                territory_id: None,
                primary_owner: None,
            },
        )
        .await
        .map_err(|_| ConnectError::internal("Sales coverage list failed"))?;
        RpcResponse::ok(sale_wire::ListTerritoriesResponse {
            territories: items
                .into_iter()
                .map(Self::territory_wire)
                .collect::<std::result::Result<Vec<_>, _>>()?,
            page_info: Some(core_v2::PageInfo {
                next_page_token: next.unwrap_or_default(),
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
    }

    #[worker::send]
    async fn create_sales_account_assignment(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::CreateSalesAccountAssignmentRequest>,
    ) -> ServiceResult<sale_wire::CreateSalesAccountAssignmentResponse> {
        let input = req.to_owned_message();
        let id = Self::id(&input.account_id)?;
        let (key, expected) = wire_precondition(input.precondition.as_option(), true)?;
        let cfg = input.configuration.as_option().ok_or_else(|| {
            ConnectError::invalid_argument("SALES_COVERAGE_CONFIGURATION_REQUIRED")
        })?;
        let record = Self::assignment_draft(id, cfg)?;
        let db = self.d1()?;
        let saved = sales_territories::mutate(
            &sales_territories::Context {
                db: &db,
                tenant: &self.tenant,
                actor: &self.actor,
                request_id: &self.request_id,
            },
            record,
            &key,
            expected,
        )
        .await
        .map_err(|_| ConnectError::unavailable("SALES_COVERAGE_WRITE_FAILED"))?
        .map_err(|failure| failure.connect("CreateSalesAccountAssignment", &self.request_id))?;
        let receipt = self.coverage_receipt(&saved);
        RpcResponse::ok(sale_wire::CreateSalesAccountAssignmentResponse {
            assignment: Some(Self::assignment_wire(saved.record)?).into(),
            receipt: Some(receipt).into(),
            ..Default::default()
        })
    }

    #[worker::send]
    async fn update_sales_account_assignment(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::UpdateSalesAccountAssignmentRequest>,
    ) -> ServiceResult<sale_wire::UpdateSalesAccountAssignmentResponse> {
        let input = req.to_owned_message();
        let id = Self::id(&input.account_id)?;
        let (key, expected) = wire_precondition(input.precondition.as_option(), false)?;
        let cfg = input.configuration.as_option().ok_or_else(|| {
            ConnectError::invalid_argument("SALES_COVERAGE_CONFIGURATION_REQUIRED")
        })?;
        let record = Self::assignment_draft(id, cfg)?;
        let db = self.d1()?;
        let saved = sales_territories::mutate(
            &sales_territories::Context {
                db: &db,
                tenant: &self.tenant,
                actor: &self.actor,
                request_id: &self.request_id,
            },
            record,
            &key,
            expected,
        )
        .await
        .map_err(|_| ConnectError::unavailable("SALES_COVERAGE_WRITE_FAILED"))?
        .map_err(|failure| failure.connect("UpdateSalesAccountAssignment", &self.request_id))?;
        let receipt = self.coverage_receipt(&saved);
        RpcResponse::ok(sale_wire::UpdateSalesAccountAssignmentResponse {
            assignment: Some(Self::assignment_wire(saved.record)?).into(),
            receipt: Some(receipt).into(),
            ..Default::default()
        })
    }

    #[worker::send]
    async fn get_sales_account_assignment(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::GetSalesAccountAssignmentRequest>,
    ) -> ServiceResult<sale_wire::GetSalesAccountAssignmentResponse> {
        let input = req.to_owned_message();
        let id = Self::id(&input.account_id)?;
        let db = self.d1()?;
        let kind = sales_territories::Kind::Assignment;
        if accounts::load(&db, &self.tenant, &id)
            .await
            .map_err(|_| ConnectError::internal("Account read failed"))?
            .is_none()
        {
            return Err(ConnectError::not_found("CUSTOMER_ACCOUNTS_NOT_FOUND"));
        }
        let record = sales_territories::get(&db, &self.tenant, kind, &id)
            .await
            .map_err(|_| ConnectError::internal("Sales coverage read failed"))?;
        let confirmed_mutation = match receipt_key(input.receipt_idempotency_key.as_deref())? {
            None => None,
            Some(key) => {
                let row = crud_receipt(&db, &self.tenant, key, CrudReceiptOwner::AccountAssignment)
                    .await
                    .map_err(|_| ConnectError::unavailable("SALES_COVERAGE_RECEIPT_READ_FAILED"))?;
                crud_commit_evidence(row, &id, &self.request_id, |state| {
                    let record = kind.decode(state)?;
                    Ok((record.id().to_owned(), record.version()))
                })
                .map_err(|_| ConnectError::unavailable("SALES_COVERAGE_RECEIPT_DECODE_FAILED"))?
            }
        };
        RpcResponse::ok(sale_wire::GetSalesAccountAssignmentResponse {
            assignment: record.map(Self::assignment_wire).transpose()?.into(),
            confirmed_mutation: confirmed_mutation.into(),
            ..Default::default()
        })
    }

    #[worker::send]
    async fn list_sales_account_assignments(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::ListSalesAccountAssignmentsRequest>,
    ) -> ServiceResult<sale_wire::ListSalesAccountAssignmentsResponse> {
        let size = if req.page.page_size == 0 {
            25
        } else {
            req.page.page_size
        };
        if size > 100 {
            return Err(ConnectError::invalid_argument("page_size exceeds 100"));
        }
        if !req.page.page_token.is_empty() {
            Self::id(req.page.page_token)?;
        }
        let after = req.page.page_token.to_owned();
        let include_archived = req.include_archived;
        let input = req.to_owned_message();
        let owner = Self::owner_scope(input.primary_owner_filter.as_option())?;
        if let Some(id) = &input.territory_id {
            Self::id(id)?;
        }
        let (items, next) = sales_territories::list(
            &self.d1()?,
            &self.tenant,
            sales_territories::Kind::Assignment,
            sales_territories::Filter {
                after: &after,
                limit: size as usize,
                include_archived,
                territory_id: input.territory_id.as_deref(),
                primary_owner: owner.as_ref(),
            },
        )
        .await
        .map_err(|_| ConnectError::internal("Sales coverage list failed"))?;
        RpcResponse::ok(sale_wire::ListSalesAccountAssignmentsResponse {
            assignments: items
                .into_iter()
                .map(Self::assignment_wire)
                .collect::<std::result::Result<Vec<_>, _>>()?,
            page_info: Some(core_v2::PageInfo {
                next_page_token: next.unwrap_or_default(),
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
    }

    #[worker::send]
    async fn create_sales_view(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::CreateSalesViewRequest>,
    ) -> ServiceResult<sale_wire::CreateSalesViewResponse> {
        let input = req.to_owned_message();
        Self::id(&input.view_id)?;
        let status = Self::view_status(input.status)?;
        if let Some(id) = &input.account_id {
            AccountV2Service::valid_id(id)?;
        }
        if let Some(id) = &input.pipeline_id {
            Self::pipeline_id(id)?;
        }
        let (key, _) = wire_precondition(input.precondition.as_option(), true)?;
        let saved = sales_views::create(
            sales_views::CreateInput {
                view_id: input.view_id,
                name: input.name,
                pipeline_id: input.pipeline_id,
                status,
                account_id: input.account_id,
                owner_filter: Self::owner_scope(input.owner_filter.as_option())?,
            },
            &key,
            &self.d1()?,
            &self.tenant,
            &self.actor,
            &self.request_id,
        )
        .await
        .map_err(|_| ConnectError::internal("Sales view D1 create failed"))?
        .map_err(|failure| failure.connect("CreateSalesView", &self.request_id))?;
        RpcResponse::ok(sale_wire::CreateSalesViewResponse {
            view: Some(Self::view(&saved.value)).into(),
            receipt: Some(self.view_receipt(&saved)).into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn update_sales_view(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::UpdateSalesViewRequest>,
    ) -> ServiceResult<sale_wire::UpdateSalesViewResponse> {
        let input = req.to_owned_message();
        Self::id(&input.view_id)?;
        let status = Self::view_status(input.status)?;
        if let Some(id) = &input.account_id {
            AccountV2Service::valid_id(id)?;
        }
        if let Some(id) = &input.pipeline_id {
            Self::pipeline_id(id)?;
        }
        let (key, version) = wire_precondition(input.precondition.as_option(), false)?;
        let saved = sales_views::update(
            sales_views::UpdateInput {
                view_id: input.view_id,
                expected_version: version.unwrap_or_default(),
                name: input.name,
                pipeline_id: input.pipeline_id,
                status,
                account_id: input.account_id,
                owner_filter: Self::owner_scope(input.owner_filter.as_option())?,
                archived: input.archived,
            },
            &key,
            &self.d1()?,
            &self.tenant,
            &self.actor,
            &self.request_id,
        )
        .await
        .map_err(|_| ConnectError::internal("Sales view D1 update failed"))?
        .map_err(|failure| failure.connect("UpdateSalesView", &self.request_id))?;
        RpcResponse::ok(sale_wire::UpdateSalesViewResponse {
            view: Some(Self::view(&saved.value)).into(),
            receipt: Some(self.view_receipt(&saved)).into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn get_sales_view(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::GetSalesViewRequest>,
    ) -> ServiceResult<sale_wire::GetSalesViewResponse> {
        let input = req.to_owned_message();
        Self::id(&input.view_id)?;
        let db = self.d1()?;
        let value = sales_views::get(&db, &self.tenant, &input.view_id)
            .await
            .map_err(|_| {
                diagnostics::read_failure(
                    &self.request_id,
                    "sales_view_read",
                    diagnostics::ReadFailureReason::InternalReadError,
                );
                ConnectError::internal("Sales view D1 read failed")
            })?
            .ok_or_else(|| ConnectError::not_found("SALES_VIEW_NOT_FOUND"))?;
        let confirmed_mutation = match receipt_key(input.receipt_idempotency_key.as_deref())? {
            None => None,
            Some(key) => {
                let row = crud_receipt(&db, &self.tenant, key, CrudReceiptOwner::SalesView)
                    .await
                    .map_err(|_| {
                        diagnostics::read_failure(
                            &self.request_id,
                            "sales_view_receipt_lookup",
                            diagnostics::ReadFailureReason::InternalReadError,
                        );
                        ConnectError::unavailable("SALES_VIEW_RECEIPT_READ_FAILED")
                    })?;
                crud_commit_evidence(row, &input.view_id, &self.request_id, |state| {
                    let view = crm_sales::SalesDealView::decode(state.as_bytes())
                        .map_err(|error| worker::Error::RustError(error.code))?;
                    Ok((view.view_id, view.version))
                })
                .map_err(|_| {
                    diagnostics::read_failure(
                        &self.request_id,
                        "sales_view_receipt_decode",
                        diagnostics::ReadFailureReason::SalesStateDecodeFailed,
                    );
                    ConnectError::unavailable("SALES_VIEW_RECEIPT_READ_FAILED")
                })?
            }
        };
        RpcResponse::ok(sale_wire::GetSalesViewResponse {
            view: Some(Self::view(&value)).into(),
            confirmed_mutation: confirmed_mutation.into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn list_sales_views(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::ListSalesViewsRequest>,
    ) -> ServiceResult<sale_wire::ListSalesViewsResponse> {
        let size = if req.page.page_size == 0 {
            25
        } else {
            req.page.page_size
        };
        if size > 100 {
            return Err(ConnectError::invalid_argument("page_size exceeds 100"));
        }
        let cursor = &req.page.page_token;
        if !cursor.is_empty() {
            Self::id(cursor)?;
        }
        let (views, next) = sales_views::list(
            &self.d1()?,
            &self.tenant,
            cursor,
            size as usize,
            req.include_archived,
        )
        .await
        .map_err(|_| ConnectError::internal("Sales view D1 list failed"))?;
        RpcResponse::ok(sale_wire::ListSalesViewsResponse {
            views: views.iter().map(Self::view).collect(),
            page_info: Some(core_v2::PageInfo {
                next_page_token: next.unwrap_or_default(),
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn create_pipeline(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::CreatePipelineRequest>,
    ) -> ServiceResult<sale_wire::CreatePipelineResponse> {
        let input = req.to_owned_message();
        Self::pipeline_id(&input.pipeline_id)?;
        let (key, _) = wire_precondition(input.precondition.as_option(), true)?;
        let saved = sales_pipelines::create(
            sales_pipelines::CreateInput {
                pipeline_id: input.pipeline_id,
                name: input.name,
                stages: Self::stage_inputs(&input.stages)?,
            },
            &key,
            &self.d1()?,
            &self.tenant,
            &self.actor,
            &self.request_id,
        )
        .await
        .map_err(|_| ConnectError::internal("Pipeline D1 create failed"))?
        .map_err(|failure| failure.connect("CreatePipeline", &self.request_id))?;
        RpcResponse::ok(sale_wire::CreatePipelineResponse {
            pipeline: Some(Self::pipeline(&saved.value)).into(),
            receipt: Some(self.pipeline_receipt(&saved)).into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn update_pipeline(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::UpdatePipelineRequest>,
    ) -> ServiceResult<sale_wire::UpdatePipelineResponse> {
        let input = req.to_owned_message();
        Self::pipeline_id(&input.pipeline_id)?;
        let (key, version) = wire_precondition(input.precondition.as_option(), false)?;
        let saved = sales_pipelines::update(
            sales_pipelines::UpdateInput {
                pipeline_id: input.pipeline_id,
                expected_version: version.unwrap_or_default(),
                name: input.name,
                stages: Self::stage_inputs(&input.stages)?,
                archived: input.archived,
            },
            &key,
            &self.d1()?,
            &self.tenant,
            &self.actor,
            &self.request_id,
        )
        .await
        .map_err(|_| ConnectError::internal("Pipeline D1 update failed"))?
        .map_err(|failure| failure.connect("UpdatePipeline", &self.request_id))?;
        RpcResponse::ok(sale_wire::UpdatePipelineResponse {
            pipeline: Some(Self::pipeline(&saved.value)).into(),
            receipt: Some(self.pipeline_receipt(&saved)).into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn get_pipeline(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::GetPipelineRequest>,
    ) -> ServiceResult<sale_wire::GetPipelineResponse> {
        let input = req.to_owned_message();
        Self::pipeline_id(&input.pipeline_id)?;
        let db = self.d1()?;
        let pipeline = sales_pipelines::get(&db, &self.tenant, &input.pipeline_id)
            .await
            .map_err(|_| {
                diagnostics::read_failure(
                    &self.request_id,
                    "sales_pipeline_read",
                    diagnostics::ReadFailureReason::InternalReadError,
                );
                ConnectError::internal("Pipeline D1 get failed")
            })?
            .ok_or_else(|| ConnectError::not_found("SALES_PIPELINE_NOT_FOUND"))?;
        let confirmed_mutation = match receipt_key(input.receipt_idempotency_key.as_deref())? {
            None => None,
            Some(key) => {
                let row = crud_receipt(&db, &self.tenant, key, CrudReceiptOwner::Pipeline)
                    .await
                    .map_err(|_| {
                        diagnostics::read_failure(
                            &self.request_id,
                            "sales_pipeline_receipt_lookup",
                            diagnostics::ReadFailureReason::InternalReadError,
                        );
                        ConnectError::unavailable("SALES_PIPELINE_RECEIPT_READ_FAILED")
                    })?;
                crud_commit_evidence(row, &input.pipeline_id, &self.request_id, |state| {
                    let pipeline = crm_sales::SalesPipeline::decode(state.as_bytes())
                        .map_err(|error| worker::Error::RustError(error.code))?;
                    Ok((pipeline.pipeline_id, pipeline.version))
                })
                .map_err(|_| {
                    diagnostics::read_failure(
                        &self.request_id,
                        "sales_pipeline_receipt_decode",
                        diagnostics::ReadFailureReason::SalesStateDecodeFailed,
                    );
                    ConnectError::unavailable("SALES_PIPELINE_RECEIPT_READ_FAILED")
                })?
            }
        };
        RpcResponse::ok(sale_wire::GetPipelineResponse {
            pipeline: Some(Self::pipeline(&pipeline)).into(),
            confirmed_mutation: confirmed_mutation.into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn list_pipelines(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::ListPipelinesRequest>,
    ) -> ServiceResult<sale_wire::ListPipelinesResponse> {
        let size = if req.page.page_size == 0 {
            25
        } else {
            req.page.page_size
        };
        if size > 100 {
            return Err(ConnectError::invalid_argument("page_size exceeds 100"));
        }
        let cursor = &req.page.page_token;
        if !cursor.is_empty() {
            Self::pipeline_id(cursor)?;
        }
        let (pipelines, next) = sales_pipelines::list(
            &self.d1()?,
            &self.tenant,
            cursor,
            size as usize,
            req.include_archived,
        )
        .await
        .map_err(|_| ConnectError::internal("Pipeline D1 list failed"))?;
        RpcResponse::ok(sale_wire::ListPipelinesResponse {
            pipelines: pipelines.iter().map(Self::pipeline).collect(),
            currencies: if cursor.is_empty() {
                crm_core_contracts::CurrencyCode::catalog()
                    .into_iter()
                    .map(|currency| core_v2::CurrencyDefinition {
                        currency_code: currency.as_str().to_owned(),
                        minor_unit_exponent: Some(u32::from(currency.exponent())),
                        ..Default::default()
                    })
                    .collect()
            } else {
                Vec::new()
            },
            page_info: Some(core_v2::PageInfo {
                next_page_token: next.unwrap_or_default(),
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn list_stage_deals(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::ListStageDealsRequest>,
    ) -> ServiceResult<sale_wire::ListStageDealsResponse> {
        Self::pipeline_id(req.pipeline_id)?;
        crm_sales::StageId::try_new(req.stage_id.to_owned())
            .map_err(|_| ConnectError::invalid_argument("SALES_STAGE_ID_INVALID"))?;
        let size = if req.page.page_size == 0 {
            25
        } else {
            req.page.page_size
        };
        if size > 100 {
            return Err(ConnectError::invalid_argument("page_size exceeds 100"));
        }
        let cursor = &req.page.page_token;
        if !cursor.is_empty() {
            Self::id(cursor)?;
        }
        let status = match req.status {
            None => None,
            Some(raw) => Some(match raw {
                x if x == sale_wire::DealStatus::DEAL_STATUS_OPEN => DomainDealStatus::Open,
                x if x == sale_wire::DealStatus::DEAL_STATUS_WON => DomainDealStatus::Won,
                x if x == sale_wire::DealStatus::DEAL_STATUS_LOST => DomainDealStatus::Lost,
                _ => return Err(ConnectError::invalid_argument("invalid DealStatus filter")),
            }),
        };
        if let Some(id) = req.account_id {
            AccountV2Service::valid_id(id)?;
        }
        let db = self.d1()?;
        let pipeline = sales_pipelines::get(&db, &self.tenant, req.pipeline_id)
            .await
            .map_err(|_| ConnectError::internal("Pipeline D1 read failed"))?
            .ok_or_else(|| ConnectError::not_found("SALES_PIPELINE_NOT_FOUND"))?;
        if pipeline.stage(req.stage_id).is_none() {
            return Err(ConnectError::not_found("SALES_PIPELINE_STAGE_NOT_FOUND"));
        }
        let input = req.to_owned_message();
        let owner_filter = Self::owner_scope(input.owner_filter.as_option())?;
        let (list, next, total_count, currency_totals) = sales_deals::list_stage_page(
            &db,
            &self.tenant,
            sales_deals::StageListQuery {
                pipeline_id: req.pipeline_id,
                stage_id: req.stage_id,
                after: cursor,
                limit: size as usize,
                status,
                account: req.account_id,
                owner_filter: owner_filter.as_ref(),
            },
        )
        .await
        .map_err(|error| {
            diagnostics::read_failure(&self.request_id, "sales_stage_list", error);
            ConnectError::internal("Sales D1 stage list failed")
        })?;
        let deals = list
            .iter()
            .map(Self::convert)
            .collect::<std::result::Result<Vec<_>, ConnectError>>()?;
        RpcResponse::ok(sale_wire::ListStageDealsResponse {
            deals,
            page_info: Some(core_v2::PageInfo {
                next_page_token: next.unwrap_or_default(),
                ..Default::default()
            })
            .into(),
            total_count,
            currency_totals: currency_totals
                .into_iter()
                .map(|total| sale_wire::StageCurrencyTotal {
                    currency_code: total.currency_code,
                    total_minor_units: total.total_minor_units.to_string(),
                    amount_deal_count: total.amount_deal_count,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn get_pipeline_metrics(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::GetPipelineMetricsRequest>,
    ) -> ServiceResult<sale_wire::GetPipelineMetricsResponse> {
        Self::pipeline_id(req.pipeline_id)?;
        let status = match req.status {
            None => None,
            Some(raw) => Some(match raw {
                x if x == sale_wire::DealStatus::DEAL_STATUS_OPEN => DomainDealStatus::Open,
                x if x == sale_wire::DealStatus::DEAL_STATUS_WON => DomainDealStatus::Won,
                x if x == sale_wire::DealStatus::DEAL_STATUS_LOST => DomainDealStatus::Lost,
                _ => return Err(ConnectError::invalid_argument("invalid DealStatus filter")),
            }),
        };
        if let Some(id) = req.account_id {
            AccountV2Service::valid_id(id)?;
        }
        let db = self.d1()?;
        if sales_pipelines::get(&db, &self.tenant, req.pipeline_id)
            .await
            .map_err(|_| ConnectError::internal("Pipeline D1 read failed"))?
            .is_none()
        {
            return Err(ConnectError::not_found("SALES_PIPELINE_NOT_FOUND"));
        }
        let input = req.to_owned_message();
        let period = input
            .analysis_period
            .as_option()
            .map(|period| {
                let date = |value: &core_v2::CalendarDate| sales_deals::DateInput {
                    year: value.year,
                    month: value.month,
                    day: value.day,
                };
                let start = period.start_date.as_option().ok_or_else(|| {
                    ConnectError::invalid_argument("SALES_ANALYSIS_PERIOD_START_REQUIRED")
                })?;
                let end = period.end_date.as_option().ok_or_else(|| {
                    ConnectError::invalid_argument("SALES_ANALYSIS_PERIOD_END_REQUIRED")
                })?;
                sales_deals::AnalysisPeriod::new(date(start), date(end))
                    .map_err(ConnectError::invalid_argument)
            })
            .transpose()?;
        let owner_filter = Self::owner_scope(input.owner_filter.as_option())?;
        let metrics = sales_deals::pipeline_metrics(
            &db,
            &self.tenant,
            req.pipeline_id,
            status,
            req.account_id,
            period,
            owner_filter.as_ref(),
        )
        .await
        .map_err(|error| {
            diagnostics::read_failure(&self.request_id, "sales_metrics", error);
            ConnectError::internal("Sales D1 metrics read failed")
        })?;
        let as_status = |value: DomainDealStatus| match value {
            DomainDealStatus::Open => sale_wire::DealStatus::DEAL_STATUS_OPEN,
            DomainDealStatus::Won => sale_wire::DealStatus::DEAL_STATUS_WON,
            DomainDealStatus::Lost => sale_wire::DealStatus::DEAL_STATUS_LOST,
        };
        let forecast_category_metric =
            |row: sales_deals::ForecastCategoryMetric| sale_wire::ForecastCategoryMetric {
                category: Self::forecast_wire(row.category).into(),
                deal_count: row.deal_count,
                stage_default_count: row.stage_default_count,
                manual_count: row.manual_count,
                ..Default::default()
            };
        let forecast_currency_metric =
            |row: sales_deals::ForecastCurrencyMetric| sale_wire::ForecastCurrencyMetric {
                category: Self::forecast_wire(row.category).into(),
                currency_code: row.currency_code,
                total_minor_units: row.total_minor_units.to_string(),
                weighted_minor_units_times_basis_points: row
                    .weighted_minor_units_times_basis_points
                    .to_string(),
                amount_deal_count: row.amount_deal_count,
                ..Default::default()
            };
        RpcResponse::ok(sale_wire::GetPipelineMetricsResponse {
            pipeline_id: req.pipeline_id.to_owned(),
            total_deal_count: metrics.total_deal_count,
            statuses: metrics
                .statuses
                .into_iter()
                .map(|row| sale_wire::PipelineStatusMetric {
                    stage_id: row.stage_id,
                    status: as_status(row.status).into(),
                    deal_count: row.deal_count,
                    ..Default::default()
                })
                .collect(),
            currencies: metrics
                .currencies
                .into_iter()
                .map(|row| sale_wire::PipelineCurrencyMetric {
                    stage_id: row.stage_id,
                    status: as_status(row.status).into(),
                    currency_code: row.currency_code,
                    total_minor_units: row.total_minor_units.to_string(),
                    weighted_minor_units_times_basis_points: row
                        .weighted_minor_units_times_basis_points
                        .to_string(),
                    amount_deal_count: row.amount_deal_count,
                    ..Default::default()
                })
                .collect(),
            operational: metrics
                .operational
                .into_iter()
                .map(|row| sale_wire::PipelineOperationalMetric {
                    stage_id: row.stage_id,
                    status: as_status(row.status).into(),
                    overdue_expected_close_count: row.overdue_expected_close_count,
                    due_next_30_days_count: row.due_next_30_days_count,
                    due_later_count: row.due_later_count,
                    no_expected_close_count: row.no_expected_close_count,
                    known_stage_age_count: row.known_stage_age_count,
                    total_stage_age_seconds: row.total_stage_age_seconds.to_string(),
                    overdue_linked_task_count: row.overdue_linked_task_count,
                    ..Default::default()
                })
                .collect(),
            forecast_categories: metrics
                .forecast_categories
                .into_iter()
                .map(forecast_category_metric)
                .collect(),
            forecast_currencies: metrics
                .forecast_currencies
                .into_iter()
                .map(forecast_currency_metric)
                .collect(),
            undated_forecast_categories: metrics
                .undated_forecast_categories
                .into_iter()
                .map(forecast_category_metric)
                .collect(),
            undated_forecast_currencies: metrics
                .undated_forecast_currencies
                .into_iter()
                .map(forecast_currency_metric)
                .collect(),
            closed_outcomes: metrics
                .closed_outcomes
                .map(|row| sale_wire::SalesClosedOutcomeMetric {
                    won_count: row.won_count,
                    lost_count: row.lost_count,
                    unknown_close_count: row.unknown_close_count,
                    denominator: row.denominator(),
                    win_rate_basis_points: row.win_rate_basis_points(),
                    ..Default::default()
                })
                .into(),
            analysis_period: input.analysis_period,
            as_of_unix_seconds: metrics.as_of_unix_seconds,
            ..Default::default()
        })
    }
    #[worker::send]
    async fn create_deal(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::CreateDealRequest>,
    ) -> ServiceResult<sale_wire::CreateDealResponse> {
        let input = req.to_owned_message();
        let id = Self::id(&input.deal_id)?;
        let (key, _) = wire_precondition(input.precondition.as_option(), true)?;
        let stage = input
            .stage
            .as_option()
            .ok_or_else(|| ConnectError::invalid_argument("stage required"))?;
        let (pipeline_id, stage_id) = Self::stage(stage);
        let probability_bps = input
            .probability_basis_points
            .map(|value| {
                u16::try_from(value)
                    .map_err(|_| ConnectError::invalid_argument("invalid probability basis points"))
            })
            .transpose()?;
        let forecast_category =
            Self::forecast_category(input.forecast_category)?.unwrap_or_default();
        let (amount_minor_units, currency) = match input.amount.as_option() {
            Some(money) => (Some(money.minor_units), Some(money.currency_code.clone())),
            None => (None, None),
        };
        let db = self.d1()?;
        let saved = sales_deals::create_typed(
            sales_deals::CreateInput {
                id,
                name: input.name,
                account_id: input.account_id,
                owner: Self::owner_scope(input.owner.as_option())?,
                automatic_routing: input.automatic_routing.as_option().map(|routing| {
                    sales_routing::AutomaticRouting {
                        territory_id: routing.territory_id.clone(),
                        expected_decision: routing.expected_decision.clone(),
                    }
                }),
                pipeline_id,
                stage_id,
                probability_bps,
                forecast_category,
                amount_minor_units,
                currency,
                primary_contact_id: input.primary_contact_id,
                expected_close_date: input.expected_close_date.as_option().map(|d| {
                    sales_deals::DateInput {
                        year: d.year,
                        month: d.month,
                        day: d.day,
                    }
                }),
            },
            &key,
            &db,
            &self.tenant,
            &self.actor,
            &self.request_id,
        )
        .await
        .map_err(|_| diagnostics::mutation_preparation_failure("CreateDeal", &self.request_id))?
        .map_err(|failure| diagnostics::mutation_error("CreateDeal", &self.request_id, failure))?;
        RpcResponse::ok(sale_wire::CreateDealResponse {
            deal: Some(Self::convert(&saved.deal)?).into(),
            receipt: Some(diagnostics::receipt(
                "CreateDeal",
                &self.request_id,
                saved.original_request_id.as_deref(),
                saved.deal.version(),
                saved.replayed,
                saved.d1_statement_changes,
            ))
            .into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn update_deal(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::UpdateDealRequest>,
    ) -> ServiceResult<sale_wire::UpdateDealResponse> {
        let input = req.to_owned_message();
        let id = Self::id(&input.deal_id)?;
        let (key, version) = wire_precondition(input.precondition.as_option(), false)?;
        let (name, clear_name) = match input.name.as_option() {
            None => (None, None),
            Some(patch) => match patch.operation.as_ref() {
                Some(core_v2::string_patch::Operation::Set(name)) => (Some(name.clone()), None),
                Some(core_v2::string_patch::Operation::Clear(true)) => (None, Some(true)),
                _ => return Err(ConnectError::invalid_argument("invalid StringPatch")),
            },
        };
        let (amount_minor_units, currency, clear_amount) = match input.amount.as_option() {
            None => (None, None, None),
            Some(patch) => match patch.operation.as_ref() {
                Some(core_v2::money_patch::Operation::Set(money)) => (
                    Some(money.minor_units),
                    Some(money.currency_code.clone()),
                    None,
                ),
                Some(core_v2::money_patch::Operation::Clear(true)) => (None, None, Some(true)),
                _ => return Err(ConnectError::invalid_argument("invalid MoneyPatch")),
            },
        };
        let probability_bps = input
            .probability_basis_points
            .map(|v| {
                u16::try_from(v)
                    .map_err(|_| ConnectError::invalid_argument("invalid probability basis points"))
            })
            .transpose()?;
        let primary_contact = match input.primary_contact_id.as_option() {
            None => sales_deals::FieldChange::Keep,
            Some(p) => match p.operation.as_ref() {
                Some(core_v2::string_patch::Operation::Set(value)) => {
                    sales_deals::FieldChange::Set(value.clone())
                }
                Some(core_v2::string_patch::Operation::Clear(true)) => {
                    sales_deals::FieldChange::Clear
                }
                _ => {
                    return Err(ConnectError::invalid_argument(
                        "invalid primary contact patch",
                    ));
                }
            },
        };
        let expected_close_date = match input.expected_close_date.as_option() {
            None => sales_deals::FieldChange::Keep,
            Some(p) => match p.operation.as_ref() {
                Some(core_v2::calendar_date_patch::Operation::Set(d)) => {
                    sales_deals::FieldChange::Set(sales_deals::DateInput {
                        year: d.year,
                        month: d.month,
                        day: d.day,
                    })
                }
                Some(core_v2::calendar_date_patch::Operation::Clear(true)) => {
                    sales_deals::FieldChange::Clear
                }
                _ => {
                    return Err(ConnectError::invalid_argument(
                        "invalid expected date patch",
                    ));
                }
            },
        };
        let saved = sales_deals::update_typed(
            sales_deals::UpdateInput {
                expected_version: version.unwrap_or_default(),
                name,
                clear_name,
                amount_minor_units,
                currency,
                clear_amount,
                probability_bps,
                use_stage_probability: input.use_stage_probability,
                forecast_category: Self::forecast_category(input.forecast_category)?,
                primary_contact,
                expected_close_date,
                owner: Self::owner_scope(input.owner.as_option())?,
            },
            &key,
            &self.d1()?,
            &self.tenant,
            &id,
            &self.actor,
            &self.request_id,
        )
        .await
        .map_err(|_| diagnostics::mutation_preparation_failure("UpdateDeal", &self.request_id))?
        .map_err(|failure| diagnostics::mutation_error("UpdateDeal", &self.request_id, failure))?;
        RpcResponse::ok(sale_wire::UpdateDealResponse {
            deal: Some(Self::convert(&saved.deal)?).into(),
            receipt: Some(diagnostics::receipt(
                "UpdateDeal",
                &self.request_id,
                saved.original_request_id.as_deref(),
                saved.deal.version(),
                saved.replayed,
                saved.d1_statement_changes,
            ))
            .into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn advance_stage(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::AdvanceStageRequest>,
    ) -> ServiceResult<sale_wire::AdvanceStageResponse> {
        let input = req.to_owned_message();
        let id = Self::id(&input.deal_id)?;
        let (key, version) = wire_precondition(input.precondition.as_option(), false)?;
        let stage = input
            .target_stage
            .as_option()
            .ok_or_else(|| ConnectError::invalid_argument("target_stage required"))?;
        let (pipeline_id, stage_id) = Self::stage(stage);
        let status = Self::deal_status(input.target_status)?;
        if !matches!(status, sales_deals::StatusInput::Open) {
            return Err(ConnectError::invalid_argument(
                "SALES_CLOSE_SEPARATE_ACTION_REQUIRED",
            ));
        }
        let advanced = sales_deals::advance_typed(
            sales_deals::StageInput {
                expected_version: version.unwrap_or_default(),
                pipeline_id,
                stage_id,
                status,
                close_reason_code: input.close_reason_code,
            },
            &key,
            &self.d1()?,
            &self.tenant,
            &id,
            &self.actor,
            &self.request_id,
        )
        .await
        .map_err(|_| diagnostics::mutation_preparation_failure("AdvanceStage", &self.request_id))?
        .map_err(|failure| {
            diagnostics::mutation_error("AdvanceStage", &self.request_id, failure)
        })?;
        RpcResponse::ok(sale_wire::AdvanceStageResponse {
            deal: Some(Self::convert(&advanced.saved.deal)?).into(),
            receipt: Some(diagnostics::receipt(
                "AdvanceStage",
                &self.request_id,
                advanced.saved.original_request_id.as_deref(),
                advanced.saved.deal.version(),
                advanced.saved.replayed,
                advanced.saved.d1_statement_changes,
            ))
            .into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn preview_deal_close_credit(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::PreviewDealCloseCreditRequest>,
    ) -> ServiceResult<sale_wire::PreviewDealCloseCreditResponse> {
        let input = req.to_owned_message();
        let id = Self::id(&input.deal_id)?;
        let selection = input
            .selection
            .as_option()
            .map(|value| crm_sales::CloseCreditSelection {
                team_id: value.team_id.clone(),
                leave_team_unallocated: value.leave_team_unallocated,
                territory_id: value.territory_id.clone(),
            })
            .unwrap_or_default();
        let preview = sales_deals::preview_close_credit(
            &self.d1()?,
            &self.tenant,
            &id,
            input.expected_version,
            &selection,
        )
        .await
        .map_err(|_| ConnectError::internal("SALES_CLOSE_CREDIT_READ_FAILED"))?
        .map_err(|error| match error.category {
            crm_module_sdk::ErrorCategory::NotFound => ConnectError::not_found(error.code),
            crm_module_sdk::ErrorCategory::Conflict => ConnectError::aborted(error.code),
            _ => ConnectError::invalid_argument(error.code),
        })?;
        let dimension = |value: &crm_sales::CommercialDimension| sale_wire::CommercialDimension {
            id: value.id.clone(),
            name: value.name.clone(),
            version: value.version,
            ..Default::default()
        };
        RpcResponse::ok(sale_wire::PreviewDealCloseCreditResponse {
            deal_version: preview.deal_version,
            owner: Some(Self::owner_wire(&preview.fact.owner)).into(),
            credited_actor: preview.fact.credited_actor,
            eligible_teams: preview.teams.iter().map(dimension).collect(),
            credited_team: preview.fact.credited_team.as_ref().map(dimension).into(),
            credited_territory: preview
                .fact
                .credited_territory
                .as_ref()
                .map(dimension)
                .into(),
            territory_assigned: preview.territory_assigned,
            expected_decision: preview.fingerprint,
            ..Default::default()
        })
    }

    #[worker::send]
    async fn close_deal(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::CloseDealRequest>,
    ) -> ServiceResult<sale_wire::CloseDealResponse> {
        let input = req.to_owned_message();
        let id = Self::id(&input.deal_id)?;
        let (key, version) = wire_precondition(input.precondition.as_option(), false)?;
        let status = Self::deal_status(input.outcome)?;
        if matches!(status, sales_deals::StatusInput::Open) {
            return Err(ConnectError::invalid_argument(
                "SALES_CLOSE_OUTCOME_REQUIRED",
            ));
        }
        if input.reason_code.is_empty() {
            return Err(ConnectError::invalid_argument(
                "SALES_CLOSE_REASON_REQUIRED",
            ));
        }
        let saved = sales_deals::close_typed(
            sales_deals::CloseInput {
                expected_version: version.unwrap_or_default(),
                outcome: status,
                reason_code: input.reason_code,
                credit_selection: input.credit_selection.as_option().map(|value| {
                    crm_sales::CloseCreditSelection {
                        team_id: value.team_id.clone(),
                        leave_team_unallocated: value.leave_team_unallocated,
                        territory_id: value.territory_id.clone(),
                    }
                }),
                expected_credit_decision: input.expected_credit_decision,
            },
            &key,
            &self.d1()?,
            &self.tenant,
            &id,
            &self.actor,
            &self.request_id,
        )
        .await
        .map_err(|_| diagnostics::mutation_preparation_failure("CloseDeal", &self.request_id))?
        .map_err(|failure| diagnostics::mutation_error("CloseDeal", &self.request_id, failure))?;
        RpcResponse::ok(sale_wire::CloseDealResponse {
            deal: Some(Self::convert(&saved.saved.deal)?).into(),
            receipt: Some(diagnostics::receipt(
                "CloseDeal",
                &self.request_id,
                saved.saved.original_request_id.as_deref(),
                saved.saved.deal.version(),
                saved.saved.replayed,
                saved.saved.d1_statement_changes,
            ))
            .into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn reopen_deal(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::ReopenDealRequest>,
    ) -> ServiceResult<sale_wire::ReopenDealResponse> {
        let input = req.to_owned_message();
        let id = Self::id(&input.deal_id)?;
        let (key, version) = wire_precondition(input.precondition.as_option(), false)?;
        if input.reason_code.is_empty() {
            return Err(ConnectError::invalid_argument(
                "SALES_REOPEN_REASON_REQUIRED",
            ));
        }
        let saved = sales_deals::reopen_typed(
            sales_deals::ReopenInput {
                expected_version: version.unwrap_or_default(),
                reason_code: input.reason_code,
                owner: Self::owner_scope(input.owner.as_option())?,
            },
            &key,
            &self.d1()?,
            &self.tenant,
            &id,
            &self.actor,
            &self.request_id,
        )
        .await
        .map_err(|_| diagnostics::mutation_preparation_failure("ReopenDeal", &self.request_id))?
        .map_err(|failure| diagnostics::mutation_error("ReopenDeal", &self.request_id, failure))?;
        RpcResponse::ok(sale_wire::ReopenDealResponse {
            deal: Some(Self::convert(&saved.deal)?).into(),
            receipt: Some(diagnostics::receipt(
                "ReopenDeal",
                &self.request_id,
                saved.original_request_id.as_deref(),
                saved.deal.version(),
                saved.replayed,
                saved.d1_statement_changes,
            ))
            .into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn list_deal_events(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::ListDealEventsRequest>,
    ) -> ServiceResult<sale_wire::ListDealEventsResponse> {
        let id = Self::id(req.deal_id)?;
        let size = if req.page.page_size == 0 {
            25
        } else {
            req.page.page_size
        };
        if size > 100 {
            return Err(ConnectError::invalid_argument("page_size exceeds 100"));
        }
        let before = if req.page.page_token.is_empty() {
            i64::MAX
        } else {
            let parsed = req
                .page
                .page_token
                .parse::<i64>()
                .map_err(|_| ConnectError::invalid_argument("SALES_HISTORY_CURSOR_INVALID"))?;
            if parsed <= 0 {
                return Err(ConnectError::invalid_argument(
                    "SALES_HISTORY_CURSOR_INVALID",
                ));
            }
            parsed
        };
        let db = self.d1()?;
        if sales_deals::load(&db, &self.tenant, &id)
            .await
            .map_err(|_| {
                diagnostics::read_failure(
                    &self.request_id,
                    "sales_history_owner",
                    diagnostics::ReadFailureReason::InternalReadError,
                );
                ConnectError::internal("Sales history owner lookup failed")
            })?
            .is_none()
        {
            return Err(ConnectError::not_found("SALES_DEAL_NOT_FOUND"));
        }
        let (rows, next) = sales_deals::list_events(&db, &self.tenant, &id, before, size as usize)
            .await
            .map_err(|_| {
                diagnostics::read_failure(
                    &self.request_id,
                    "sales_history_read",
                    diagnostics::ReadFailureReason::InternalReadError,
                );
                ConnectError::internal("Sales D1 history read failed")
            })?;
        let events = rows
            .into_iter()
            .map(|row| {
                let kind = match row.action.as_str() {
                    "created" => sale_wire::DealEventKind::DEAL_EVENT_KIND_CREATED,
                    "updated" => sale_wire::DealEventKind::DEAL_EVENT_KIND_UPDATED,
                    "moved" => sale_wire::DealEventKind::DEAL_EVENT_KIND_MOVED,
                    "closed" => sale_wire::DealEventKind::DEAL_EVENT_KIND_CLOSED,
                    "reopened" => sale_wire::DealEventKind::DEAL_EVENT_KIND_REOPENED,
                    _ => {
                        return Err(ConnectError::internal(
                            "invalid persisted Sales event action",
                        ));
                    }
                };
                let status = |value: &str| match value {
                    "open" => Ok(sale_wire::DealStatus::DEAL_STATUS_OPEN),
                    "won" => Ok(sale_wire::DealStatus::DEAL_STATUS_WON),
                    "lost" => Ok(sale_wire::DealStatus::DEAL_STATUS_LOST),
                    _ => Err(ConnectError::internal(
                        "invalid persisted Sales event status",
                    )),
                };
                let previous_status = match row.previous_status {
                    Some(previous) => status(&previous)?,
                    None => sale_wire::DealStatus::DEAL_STATUS_UNSPECIFIED,
                };
                Ok(sale_wire::DealEvent {
                    deal_version: row.deal_version,
                    kind: kind.into(),
                    pipeline_id: row.pipeline_id,
                    previous_stage_id: row.previous_stage_id.unwrap_or_default(),
                    stage_id: row.stage_id,
                    previous_status: previous_status.into(),
                    status: status(&row.status)?.into(),
                    reason_code: row.reason_code.unwrap_or_default(),
                    actor: row.actor,
                    request_id: row.request_id,
                    occurred_at: Some(ContactPointV2Service::timestamp(
                        row.occurred_at_unix_nanos.parse::<i64>().map_err(|_| {
                            ConnectError::internal("invalid persisted Sales event timestamp")
                        })?,
                    ))
                    .into(),
                    catalog_version: row.catalog_version.unwrap_or_default(),
                    linked_task_id: row.linked_task_id.unwrap_or_default(),
                    commercial_close_fact: row
                        .commercial_close_fact
                        .as_deref()
                        .map(|raw| {
                            let value: crm_sales::CommercialCloseFact = serde_json::from_str(raw)
                                .map_err(|_| {
                                ConnectError::internal("SALES_COMMERCIAL_EVENT_INVALID")
                            })?;
                            Self::commercial_fact_wire(&value)
                        })
                        .transpose()?
                        .into(),
                    initial_routing: row
                        .initial_routing
                        .as_deref()
                        .map(|raw| {
                            let value: crm_sales::SalesRoutingDecision = serde_json::from_str(raw)
                                .map_err(|_| {
                                    ConnectError::internal("SALES_ROUTING_EVENT_INVALID")
                                })?;
                            value.validate().map_err(|_| {
                                ConnectError::internal("SALES_ROUTING_EVENT_INVALID")
                            })?;
                            Ok::<_, ConnectError>(Self::routing_decision_wire(&value))
                        })
                        .transpose()?
                        .into(),
                    ..Default::default()
                })
            })
            .collect::<std::result::Result<Vec<_>, ConnectError>>()?;
        RpcResponse::ok(sale_wire::ListDealEventsResponse {
            events,
            page_info: Some(core_v2::PageInfo {
                next_page_token: next.unwrap_or_default(),
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn get_deal(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::GetDealRequest>,
    ) -> ServiceResult<sale_wire::GetDealResponse> {
        let input = req.to_owned_message();
        let id = Self::id(&input.deal_id)?;
        let db = self.d1()?;
        let deal = sales_deals::load(&db, &self.tenant, &id)
            .await
            .map_err(|_| {
                diagnostics::read_failure(
                    &self.request_id,
                    "sales_deal_read",
                    diagnostics::ReadFailureReason::InternalReadError,
                );
                ConnectError::internal("Sales original D1 read failed")
            })?
            .ok_or_else(|| ConnectError::not_found("SALES_DEAL_NOT_FOUND"))?;
        let confirmed_mutation = match input.receipt_idempotency_key.as_deref() {
            None => None,
            Some(key) if (8..=120).contains(&key.len()) => {
                sales_deals::confirmed_receipt(&db, &self.tenant, &id, key)
                    .await
                    .map_err(|error| {
                        diagnostics::read_failure(&self.request_id, "sales_receipt_lookup", error);
                        ConnectError::unavailable("SALES_RECEIPT_READ_FAILED")
                    })?
                    .map(|(original, version)| core_v2::MutationCommitEvidence {
                        request_id: self.request_id.clone(),
                        resource_version: version,
                        original_request_id: original,
                        ..Default::default()
                    })
            }
            Some(_) => return Err(ConnectError::invalid_argument("IDEMPOTENCY_KEY_REQUIRED")),
        };
        RpcResponse::ok(sale_wire::GetDealResponse {
            deal: Some(Self::convert(&deal)?).into(),
            confirmed_mutation: confirmed_mutation.into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn list_deals(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::ListDealsRequest>,
    ) -> ServiceResult<sale_wire::ListDealsResponse> {
        let size = if req.page.page_size == 0 {
            25
        } else {
            req.page.page_size
        };
        if size > 100 {
            return Err(ConnectError::invalid_argument("page_size exceeds 100"));
        }
        let after = &req.page.page_token;
        if !after.is_empty() {
            Self::id(after)?;
        }
        let pipeline = req.pipeline_id;
        if let Some(id) = pipeline {
            crm_sales::PipelineId::try_new(id)
                .map_err(|_| ConnectError::invalid_argument("invalid pipeline filter"))?;
        }
        let status = match req.status {
            None => None,
            Some(s) => Some(match s {
                x if x == sale_wire::DealStatus::DEAL_STATUS_OPEN => DomainDealStatus::Open,
                x if x == sale_wire::DealStatus::DEAL_STATUS_WON => DomainDealStatus::Won,
                x if x == sale_wire::DealStatus::DEAL_STATUS_LOST => DomainDealStatus::Lost,
                _ => return Err(ConnectError::invalid_argument("invalid DealStatus filter")),
            }),
        };
        let account = req.account_id;
        if let Some(id) = account {
            AccountV2Service::valid_id(id)?;
        }
        let input = req.to_owned_message();
        let owner_filter = Self::owner_scope(input.owner_filter.as_option())?;
        let (list, next) = sales_deals::list_page_typed(
            &self.d1()?,
            &self.tenant,
            sales_deals::DealListQuery {
                after,
                limit: size as usize,
                pipeline,
                status,
                account,
                search: wire_search(req.search)?,
                owner_filter: owner_filter.as_ref(),
            },
        )
        .await
        .map_err(|_| {
            diagnostics::read_failure(
                &self.request_id,
                "sales_deal_list",
                diagnostics::ReadFailureReason::InternalReadError,
            );
            ConnectError::internal("Sales original D1 list failed")
        })?;
        let deals = list
            .iter()
            .map(Self::convert)
            .collect::<std::result::Result<Vec<_>, ConnectError>>()?;
        RpcResponse::ok(sale_wire::ListDealsResponse {
            deals,
            page_info: Some(core_v2::PageInfo {
                next_page_token: next.unwrap_or_default(),
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
    }
}

impl SalesV2Service {
    fn coverage_owner(
        value: Option<&core_v2::ActorOrTeamOwner>,
    ) -> std::result::Result<crm_sales::SalesOwnerScope, ConnectError> {
        Self::owner_scope(value)?
            .ok_or_else(|| ConnectError::invalid_argument("SALES_COVERAGE_OWNER_REQUIRED"))
    }

    fn territory_draft(
        id: String,
        cfg: &sale_wire::TerritoryConfiguration,
    ) -> std::result::Result<sales_territories::Record, ConnectError> {
        Ok(sales_territories::Record::Territory(
            crm_sales::SalesTerritory {
                territory_id: id,
                name: cfg.name.clone(),
                parent_id: cfg.parent_id.clone(),
                default_owner: Self::coverage_owner(cfg.default_owner.as_option())?,
                archived: cfg.archived,
                version: 1,
            },
        ))
    }

    fn assignment_draft(
        id: String,
        cfg: &sale_wire::SalesAccountAssignmentConfiguration,
    ) -> std::result::Result<sales_territories::Record, ConnectError> {
        use crm_sales::SalesOverlayRole as Domain;
        use sale_wire::SalesOverlayRole as Wire;
        let overlays = cfg
            .overlays
            .iter()
            .map(|item| {
                let role = match item.role {
                    v if v == Wire::SALES_OVERLAY_ROLE_SPECIALIST => Domain::Specialist,
                    v if v == Wire::SALES_OVERLAY_ROLE_ACCOUNT_SUPPORT => Domain::AccountSupport,
                    v if v == Wire::SALES_OVERLAY_ROLE_EXECUTIVE_SPONSOR => {
                        Domain::ExecutiveSponsor
                    }
                    _ => return Err(ConnectError::invalid_argument("SALES_OVERLAY_ROLE_INVALID")),
                };
                Ok(crm_sales::SalesAccountOverlay {
                    role,
                    owner: Self::coverage_owner(item.owner.as_option())?,
                })
            })
            .collect::<std::result::Result<Vec<_>, ConnectError>>()?;
        Ok(sales_territories::Record::Assignment(
            crm_sales::SalesAccountAssignment {
                account_id: id,
                territory_id: cfg.territory_id.clone(),
                primary_owner: Self::coverage_owner(cfg.primary_owner.as_option())?,
                overlays,
                archived: cfg.archived,
                version: 1,
            },
        ))
    }

    fn territory_wire(
        record: sales_territories::Record,
    ) -> std::result::Result<sale_wire::SalesTerritory, ConnectError> {
        let sales_territories::Record::Territory(value) = record else {
            return Err(ConnectError::internal("Sales coverage kind mismatch"));
        };
        Ok(sale_wire::SalesTerritory {
            territory_id: value.territory_id,
            version: value.version,
            configuration: Some(sale_wire::TerritoryConfiguration {
                name: value.name,
                parent_id: value.parent_id,
                default_owner: Some(Self::owner_wire(&value.default_owner)).into(),
                archived: value.archived,
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
    }

    fn assignment_wire(
        record: sales_territories::Record,
    ) -> std::result::Result<sale_wire::SalesAccountAssignment, ConnectError> {
        use crm_sales::SalesOverlayRole as Domain;
        use sale_wire::SalesOverlayRole as Wire;
        let sales_territories::Record::Assignment(value) = record else {
            return Err(ConnectError::internal("Sales coverage kind mismatch"));
        };
        Ok(sale_wire::SalesAccountAssignment {
            account_id: value.account_id,
            version: value.version,
            configuration: Some(sale_wire::SalesAccountAssignmentConfiguration {
                territory_id: value.territory_id,
                primary_owner: Some(Self::owner_wire(&value.primary_owner)).into(),
                overlays: value
                    .overlays
                    .into_iter()
                    .map(|item| sale_wire::SalesAccountOverlay {
                        owner: Some(Self::owner_wire(&item.owner)).into(),
                        role: match item.role {
                            Domain::Specialist => Wire::SALES_OVERLAY_ROLE_SPECIALIST,
                            Domain::AccountSupport => Wire::SALES_OVERLAY_ROLE_ACCOUNT_SUPPORT,
                            Domain::ExecutiveSponsor => Wire::SALES_OVERLAY_ROLE_EXECUTIVE_SPONSOR,
                        }
                        .into(),
                        ..Default::default()
                    })
                    .collect(),
                archived: value.archived,
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
    }

    fn coverage_receipt(&self, saved: &sales_territories::Saved) -> core_v2::MutationReceipt {
        core_v2::MutationReceipt {
            request_id: self.request_id.clone(),
            original_request_id: saved.original_request_id.clone(),
            resource_version: saved.record.version(),
            replayed: saved.replayed,
            ..Default::default()
        }
    }
}
