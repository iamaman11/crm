//! Bounded native CSV presentation of the existing typed Sales reads.
use super::*;
use crm_customer_data_operations::{
    CanonicalCsvCell, PARTY_EXPORT_CSV_MEDIA_TYPE, encode_typed_csv_record, party_export_hex,
    party_export_sha256,
};
use sale_wire::SalesAnalyticsExportKind as Kind;
use sale_wire::export_sales_analytics_request::Scope;

const SPEC: &str = "sales-analytics-csv/v1";
const MAX_BYTES: usize = 2 * 1024 * 1024;
const MAX_DETAIL: usize = 1000;

type ExportResult<T> = std::result::Result<T, ConnectError>;

fn invalid() -> ConnectError {
    ConnectError::invalid_argument("SALES_EXPORT_SCOPE_INVALID")
}
fn incomplete() -> ConnectError {
    ConnectError::aborted("SALES_EXPORT_HISTORY_INCOMPLETE")
}
fn capacity() -> ConnectError {
    ConnectError::resource_exhausted("SALES_EXPORT_CAPACITY")
}
fn malformed() -> ConnectError {
    ConnectError::internal("SALES_EXPORT_SOURCE_INVALID")
}

// Only a validated canonical decimal integer may bypass text formula protection.
enum Cell {
    Text(String),
    Integer(String),
}
fn t(value: impl ToString) -> Cell {
    Cell::Text(value.to_string())
}
fn n(value: impl ToString) -> Cell {
    Cell::Integer(value.to_string())
}
fn opt<T: ToString>(value: Option<T>) -> Cell {
    value.map_or_else(|| t(""), n)
}

struct Csv {
    bytes: Vec<u8>,
    rows: u32,
}
impl Csv {
    fn new() -> ExportResult<Self> {
        let mut value = Self {
            bytes: Vec::new(),
            rows: 0,
        };
        value.row([t("section"), t("record_id"), t("field"), t("value")])?;
        value.rows = 0;
        value.fields(
            "metadata",
            "",
            [
                ("specification_version", t(SPEC)),
                ("money_unit", t("currency_minor_units")),
                ("weighted_minor_units_divisor", n(10000)),
                ("basis_points_denominator", n(10000)),
            ],
        )?;
        Ok(value)
    }
    fn row(&mut self, cells: [Cell; 4]) -> ExportResult<()> {
        let cells = cells.each_ref().map(|cell| match cell {
            Cell::Text(value) => CanonicalCsvCell::Text(value),
            Cell::Integer(value) => CanonicalCsvCell::Integer(value),
        });
        let row = encode_typed_csv_record(&cells).ok_or_else(malformed)?;
        if self.bytes.len().saturating_add(row.len()) > MAX_BYTES {
            return Err(capacity());
        }
        self.bytes.extend_from_slice(&row);
        self.rows += 1;
        Ok(())
    }
    fn field(&mut self, section: &str, id: &str, field: &str, value: Cell) -> ExportResult<()> {
        self.row([t(section), t(id), t(field), value])
    }
    fn fields<const N: usize>(
        &mut self,
        section: &str,
        id: &str,
        fields: [(&str, Cell); N],
    ) -> ExportResult<()> {
        for (field, value) in fields {
            self.field(section, id, field, value)?;
        }
        Ok(())
    }
    fn finish(self, filename: &str) -> sale_wire::ExportSalesAnalyticsResponse {
        sale_wire::ExportSalesAnalyticsResponse {
            csv_sha256_hex: party_export_hex(&party_export_sha256(&self.bytes)),
            csv: self.bytes,
            specification_version: SPEC.into(),
            filename: filename.into(),
            media_type: PARTY_EXPORT_CSV_MEDIA_TYPE.into(),
            row_count: self.rows,
            ..Default::default()
        }
    }
}

fn date(value: Option<&core_v2::CalendarDate>) -> String {
    value.map_or_else(String::new, |d| {
        format!("{:04}-{:02}-{:02}", d.year, d.month, d.day)
    })
}

fn quota(
    csv: &mut Csv,
    section: &str,
    id: &str,
    q: Option<&sale_wire::SalesQuota>,
) -> ExportResult<()> {
    let Some(q) = q else {
        return csv.field(section, id, "quota_id", t(""));
    };
    let cfg = q.configuration.as_option().ok_or_else(malformed)?;
    let target = cfg.target.as_option().ok_or_else(malformed)?;
    let period = cfg.period.as_option().ok_or_else(malformed)?;
    use core_v2::actor_or_team_owner::Owner;
    use sale_wire::sales_quota_scope::Scope as Q;
    let (scope, scope_id) = match cfg.scope.as_option().and_then(|s| s.scope.as_ref()) {
        Some(Q::TenantWide(true)) => ("organization", ""),
        Some(Q::TerritoryId(id)) => ("territory", id.as_str()),
        Some(Q::Owner(owner)) => match owner.owner.as_ref() {
            Some(Owner::AssigneeIdentity(id)) => ("actor", id.as_str()),
            Some(Owner::TeamId(id)) => ("team", id.as_str()),
            None => return Err(malformed()),
        },
        _ => return Err(malformed()),
    };
    csv.fields(
        section,
        id,
        [
            ("quota_id", t(&q.quota_id)),
            ("quota_version", n(q.version)),
            ("quota_name", t(&cfg.name)),
            ("quota_archived", t(cfg.archived)),
            ("quota_scope", t(scope)),
            ("quota_scope_id", t(scope_id)),
            (
                "quota_pipeline_id",
                t(cfg.pipeline_id.as_deref().unwrap_or("")),
            ),
            ("quota_start", t(date(period.start_date.as_option()))),
            ("quota_end", t(date(period.end_date.as_option()))),
            ("quota_time_zone", t(&period.time_zone)),
            ("target_minor_units", n(target.minor_units)),
            ("currency_code", t(&target.currency_code)),
        ],
    )
}

fn report(csv: &mut Csv, section: &str, r: &sale_wire::SalesPerformanceReport) -> ExportResult<()> {
    let id = &r.report_id;
    let a = r.actuals.as_option().ok_or_else(malformed)?;
    let f = r.forecast.as_option().ok_or_else(malformed)?;
    quota(csv, section, id, r.quota.as_option())?;
    csv.fields(
        section,
        id,
        [
            ("name", t(&r.name)),
            ("captured_at_unix_nanos", n(r.captured_at_unix_nanos)),
            ("event_sequence", n(r.event_sequence)),
            ("captured_by", t(&r.captured_by)),
            ("formula_version", n(r.formula_version)),
            ("actual_candidate_count", n(r.actual_candidate_count)),
            ("gross_minor_units", n(&a.gross_minor_units)),
            ("reversal_minor_units", n(&a.reversal_minor_units)),
            ("net_minor_units", n(&a.net_minor_units)),
            ("remaining_minor_units", n(&a.remaining_minor_units)),
            ("attainment_basis_points", n(&a.attainment_basis_points)),
            ("won_count", n(a.won_count)),
            ("lost_count", n(a.lost_count)),
            ("reversal_count", n(a.reversal_count)),
            ("missing_amount_count", n(a.missing_amount_count)),
            ("unknown_history_count", n(a.unknown_history_count)),
            ("unallocated_count", n(a.unallocated_count)),
            ("open_minor_units", n(&f.open_minor_units)),
            (
                "weighted_minor_units_times_basis_points",
                n(&f.weighted_minor_units_times_basis_points),
            ),
            (
                "open_coverage_basis_points",
                opt(f.open_coverage_basis_points.as_deref()),
            ),
            (
                "weighted_coverage_basis_points",
                opt(f.weighted_coverage_basis_points.as_deref()),
            ),
            ("forecast_included_count", n(f.included_count)),
            ("forecast_unallocated_count", n(f.unallocated_count)),
            ("forecast_undated_count", n(f.undated_count)),
            ("forecast_omitted_count", n(f.omitted_count)),
            ("forecast_missing_amount_count", n(f.missing_amount_count)),
            ("forecast_other_currency_count", n(f.other_currency_count)),
        ],
    )?;
    for b in &f.buckets {
        let bucket = format!("{id}:{}", category(b.category));
        csv.fields(
            "forecast_bucket",
            &bucket,
            [
                ("deal_count", n(b.deal_count)),
                ("open_minor_units", n(&b.open_minor_units)),
                (
                    "weighted_minor_units_times_basis_points",
                    n(&b.weighted_minor_units_times_basis_points),
                ),
            ],
        )?;
    }
    Ok(())
}

fn category(v: impl PartialEq<sale_wire::ForecastCategory>) -> &'static str {
    use sale_wire::ForecastCategory as C;
    if v == C::FORECAST_CATEGORY_PIPELINE {
        "pipeline"
    } else if v == C::FORECAST_CATEGORY_BEST_CASE {
        "best_case"
    } else if v == C::FORECAST_CATEGORY_COMMIT {
        "commit"
    } else if v == C::FORECAST_CATEGORY_OMITTED {
        "omitted"
    } else {
        "unspecified"
    }
}

fn comparison(
    csv: &mut Csv,
    current: &str,
    c: &sale_wire::SalesPerformanceComparison,
) -> ExportResult<()> {
    let baseline = c.baseline.as_option().ok_or_else(malformed)?;
    report(csv, "baseline_report", baseline)?;
    use sale_wire::SalesPerformanceComparisonStatus as Status;
    let status = if c.status == Status::SALES_PERFORMANCE_COMPARISON_STATUS_COMPARABLE {
        "comparable"
    } else if c.status == Status::SALES_PERFORMANCE_COMPARISON_STATUS_DIFFERENT_SERIES {
        "different_series"
    } else if c.status == Status::SALES_PERFORMANCE_COMPARISON_STATUS_DIFFERENT_FORMULA {
        "different_formula"
    } else {
        return Err(malformed());
    };
    csv.fields(
        "comparison",
        current,
        [
            ("baseline_report_id", t(&baseline.report_id)),
            ("status", t(status)),
        ],
    )?;
    if let Some(d) = c.delta.as_option() {
        csv.fields(
            "delta",
            current,
            [
                ("target_minor_units", n(&d.target_minor_units)),
                ("gross_minor_units", n(&d.gross_minor_units)),
                ("reversal_minor_units", n(&d.reversal_minor_units)),
                ("net_minor_units", n(&d.net_minor_units)),
                ("remaining_minor_units", n(&d.remaining_minor_units)),
                ("attainment_basis_points", n(&d.attainment_basis_points)),
                ("open_minor_units", n(&d.open_minor_units)),
                (
                    "weighted_minor_units_times_basis_points",
                    n(&d.weighted_minor_units_times_basis_points),
                ),
                (
                    "open_coverage_basis_points",
                    opt(d.open_coverage_basis_points.as_deref()),
                ),
                (
                    "weighted_coverage_basis_points",
                    opt(d.weighted_coverage_basis_points.as_deref()),
                ),
                ("won_count", n(d.won_count)),
                ("lost_count", n(d.lost_count)),
                ("reversal_count", n(d.reversal_count)),
                ("missing_amount_count", n(d.missing_amount_count)),
                ("unknown_history_count", n(d.unknown_history_count)),
                ("unallocated_count", n(d.unallocated_count)),
            ],
        )?;
    }
    Ok(())
}

fn dimension(
    csv: &mut Csv,
    section: &str,
    id: &str,
    axis: &str,
    v: Option<&sale_wire::CommercialDimension>,
) -> ExportResult<()> {
    if let Some(v) = v {
        csv.field(section, id, &format!("{axis}_id"), t(&v.id))?;
        csv.field(section, id, &format!("{axis}_name"), t(&v.name))?;
        csv.field(section, id, &format!("{axis}_version"), n(v.version))?;
    }
    Ok(())
}

fn fact(csv: &mut Csv, f: &sale_wire::SalesQuotaFact) -> ExportResult<()> {
    let id = format!("{}:{}", f.deal_id, f.deal_version);
    let section = "actual_event";
    csv.fields(
        section,
        &id,
        [
            ("deal_id", t(&f.deal_id)),
            ("deal_version", n(f.deal_version)),
            ("event_at_unix_nanos", n(f.event_at_unix_nanos)),
            ("reversal", t(f.reversal)),
            ("signed_minor_units", opt(f.signed_minor_units.as_deref())),
            ("history_known", t(f.close_fact.as_option().is_some())),
        ],
    )?;
    if let Some(c) = f.close_fact.as_option() {
        csv.fields(
            section,
            &id,
            [
                ("close_version", n(c.close_version)),
                ("pipeline_id", t(&c.pipeline_id)),
                ("stage_id", t(&c.stage_id)),
                (
                    "credited_actor",
                    t(c.credited_actor.as_deref().unwrap_or("")),
                ),
                (
                    "outcome",
                    t(if c.outcome == sale_wire::DealStatus::DEAL_STATUS_WON {
                        "won"
                    } else {
                        "lost"
                    }),
                ),
                (
                    "currency_code",
                    t(c.amount
                        .as_option()
                        .map_or("", |a| a.currency_code.as_str())),
                ),
                ("assignment_version", opt(c.assignment_version)),
                ("catalog_version", opt(c.catalog_version)),
            ],
        )?;
        dimension(
            csv,
            section,
            &id,
            "credited_team",
            c.credited_team.as_option(),
        )?;
        dimension(
            csv,
            section,
            &id,
            "credited_territory",
            c.credited_territory.as_option(),
        )?;
    }
    Ok(())
}

fn forecast(csv: &mut Csv, f: &sale_wire::SalesForecastSource) -> ExportResult<()> {
    let section = "forecast_source";
    let id = &f.deal_id;
    use sale_wire::SalesForecastDisposition as D;
    let disposition = if f.disposition == D::SALES_FORECAST_DISPOSITION_INCLUDED {
        "included"
    } else if f.disposition == D::SALES_FORECAST_DISPOSITION_UNALLOCATED {
        "unallocated"
    } else if f.disposition == D::SALES_FORECAST_DISPOSITION_UNDATED {
        "undated"
    } else if f.disposition == D::SALES_FORECAST_DISPOSITION_OMITTED {
        "omitted"
    } else if f.disposition == D::SALES_FORECAST_DISPOSITION_MISSING_AMOUNT {
        "missing_amount"
    } else if f.disposition == D::SALES_FORECAST_DISPOSITION_OTHER_CURRENCY {
        "other_currency"
    } else {
        return Err(malformed());
    };
    csv.fields(
        section,
        id,
        [
            ("deal_version", n(f.deal_version)),
            ("pipeline_id", t(&f.pipeline_id)),
            (
                "expected_close_date",
                t(date(f.expected_close_date.as_option())),
            ),
            (
                "minor_units",
                opt(f.amount.as_option().map(|a| a.minor_units)),
            ),
            (
                "currency_code",
                t(f.amount
                    .as_option()
                    .map_or("", |a| a.currency_code.as_str())),
            ),
            ("probability_basis_points", n(f.probability_basis_points)),
            (
                "probability_source",
                t(
                    if f.probability_source
                        == sale_wire::ProbabilitySource::PROBABILITY_SOURCE_MANUAL
                    {
                        "manual"
                    } else {
                        "stage_default"
                    },
                ),
            ),
            ("category", t(category(f.category))),
            ("disposition", t(disposition)),
            (
                "credited_actor",
                t(f.credited_actor.as_deref().unwrap_or("")),
            ),
        ],
    )?;
    dimension(
        csv,
        section,
        id,
        "credited_team",
        f.credited_team.as_option(),
    )?;
    dimension(
        csv,
        section,
        id,
        "credited_territory",
        f.credited_territory.as_option(),
    )
}

fn score_metadata(
    csv: &mut Csv,
    request: &sale_wire::GetSalesScorecardRequest,
    response: &sale_wire::GetSalesScorecardResponse,
) -> ExportResult<()> {
    use sale_wire::SalesScorecardAxis as A;
    let axis = if response.axis == A::SALES_SCORECARD_AXIS_ORGANIZATION {
        "organization"
    } else if response.axis == A::SALES_SCORECARD_AXIS_ACTOR {
        "actor"
    } else if response.axis == A::SALES_SCORECARD_AXIS_TEAM {
        "team"
    } else if response.axis == A::SALES_SCORECARD_AXIS_TERRITORY {
        "territory"
    } else {
        return Err(malformed());
    };
    csv.fields(
        "metadata",
        "",
        [
            ("axis", t(axis)),
            ("currency_code", t(&response.currency_code)),
            (
                "pipeline_id",
                t(request.pipeline_id.as_deref().unwrap_or("")),
            ),
            ("first_date", t(date(request.first_date.as_option()))),
            ("time_zone", t(&request.time_zone)),
            (
                "cadence",
                t(
                    if request.cadence == sale_wire::SalesQuotaCadence::SALES_QUOTA_CADENCE_MONTHLY
                    {
                        "monthly"
                    } else {
                        "quarterly"
                    },
                ),
            ),
            ("period_count", n(request.period_count)),
            ("as_of_unix_nanos", n(response.as_of_unix_nanos)),
            ("event_sequence", n(response.event_sequence)),
        ],
    )
}

fn score_cell(
    csv: &mut Csv,
    id: &str,
    bucket: &sale_wire::SalesScorecardBucket,
    scope: &str,
    dimension_id: &str,
    c: &sale_wire::SalesScorecardCell,
) -> ExportResult<()> {
    let p = bucket.period.as_option().ok_or_else(malformed)?;
    let section = "scorecard_cell";
    csv.fields(
        section,
        id,
        [
            ("scope", t(scope)),
            ("dimension_id", t(dimension_id)),
            ("period_start", t(date(p.start_date.as_option()))),
            ("period_end", t(date(p.end_date.as_option()))),
            ("gross_minor_units", n(&c.gross_minor_units)),
            ("reversal_minor_units", n(&c.reversal_minor_units)),
            ("net_minor_units", n(&c.net_minor_units)),
            ("won_count", n(c.won_count)),
            ("lost_count", n(c.lost_count)),
            ("reversal_count", n(c.reversal_count)),
            ("missing_amount_count", n(c.missing_amount_count)),
            ("unknown_history_count", n(c.unknown_history_count)),
            ("unallocated_count", n(c.unallocated_count)),
            ("win_rate_basis_points", opt(c.win_rate_basis_points)),
            (
                "remaining_minor_units",
                opt(c.remaining_minor_units.as_deref()),
            ),
            (
                "attainment_basis_points",
                opt(c.attainment_basis_points.as_deref()),
            ),
            ("source_event_count", n(c.source_event_count)),
        ],
    )?;
    quota(csv, section, id, c.quota.as_option())
}

impl SalesV2Service {
    pub(super) async fn export_analytics(
        &self,
        input: sale_wire::ExportSalesAnalyticsRequest,
    ) -> ExportResult<sale_wire::ExportSalesAnalyticsResponse> {
        let mut csv = Csv::new()?;
        match input.scope.as_ref() {
            Some(Scope::Report(scope)) => {
                let kind = input.kind;
                let compare = kind == Kind::SALES_ANALYTICS_EXPORT_KIND_COMPARISON;
                let actual = kind == Kind::SALES_ANALYTICS_EXPORT_KIND_REPORT_ACTUALS;
                let sources = kind == Kind::SALES_ANALYTICS_EXPORT_KIND_REPORT_FORECAST;
                if (!compare
                    && !actual
                    && !sources
                    && kind != Kind::SALES_ANALYTICS_EXPORT_KIND_REPORT)
                    || compare != scope.baseline_report_id.is_some()
                {
                    return Err(invalid());
                }
                let mut request = sale_wire::GetSalesPerformanceReportRequest {
                    report_id: scope.report_id.clone(),
                    baseline_report_id: scope.baseline_report_id.clone(),
                    page_size: if actual || sources { 100 } else { 1 },
                    ..Default::default()
                };
                let mut page = self.read_performance_report(request.clone()).await?;
                let r = page.report.as_option().ok_or_else(malformed)?;
                report(&mut csv, "report", r)?;
                csv.field(
                    "metadata",
                    "",
                    "actual_history_available",
                    t(page.actual_history_available),
                )?;
                if compare {
                    comparison(
                        &mut csv,
                        &r.report_id,
                        page.comparison.as_option().ok_or_else(malformed)?,
                    )?;
                }
                let mut count = 0usize;
                if actual || sources {
                    if actual && !page.actual_history_available {
                        return Err(incomplete());
                    }
                    if actual && r.actual_candidate_count > MAX_DETAIL as u64 {
                        return Err(capacity());
                    }
                    loop {
                        if actual {
                            if !page.actual_history_available {
                                return Err(incomplete());
                            }
                            count += page.actual_facts.len();
                            if count > MAX_DETAIL {
                                return Err(capacity());
                            }
                            for f in &page.actual_facts {
                                fact(&mut csv, f)?;
                            }
                            if page.actual_next.as_option().is_none() {
                                break;
                            }
                            request.actual_after = page.actual_next;
                        } else {
                            count += page.forecast_sources.len();
                            if count > MAX_DETAIL {
                                return Err(capacity());
                            }
                            for f in &page.forecast_sources {
                                forecast(&mut csv, f)?;
                            }
                            if page.forecast_next.is_empty() {
                                break;
                            }
                            request.forecast_after = page.forecast_next;
                        }
                        if count >= MAX_DETAIL {
                            return Err(capacity());
                        }
                        page = self.read_performance_report(request.clone()).await?;
                        if (actual
                            && page.actual_history_available
                            && page.actual_facts.is_empty()
                            && page.actual_next.as_option().is_some())
                            || (sources
                                && page.forecast_sources.is_empty()
                                && !page.forecast_next.is_empty())
                        {
                            return Err(incomplete());
                        }
                    }
                    csv.field("metadata", "", "source_count", n(count))?;
                }
                let filename = if compare {
                    "crm-sales-comparison.csv"
                } else if actual {
                    "crm-sales-actuals.csv"
                } else if sources {
                    "crm-sales-forecast.csv"
                } else {
                    "crm-sales-report.csv"
                };
                Ok(csv.finish(filename))
            }
            Some(Scope::Scorecard(scope)) => {
                let mut request = scope.as_ref().clone();
                if input.kind == Kind::SALES_ANALYTICS_EXPORT_KIND_SCORECARD {
                    if request.detail.as_option().is_some() {
                        return Err(invalid());
                    }
                    let data = self.read_scorecard(request.clone()).await?;
                    score_metadata(&mut csv, &request, &data)?;
                    for (i, b) in data.buckets.iter().enumerate() {
                        score_cell(
                            &mut csv,
                            &format!("{i}:organization"),
                            b,
                            "organization",
                            "",
                            b.organization.as_option().ok_or_else(malformed)?,
                        )?;
                        for (j, d) in b.dimensions.iter().enumerate() {
                            score_cell(
                                &mut csv,
                                &format!("{i}:dimension:{j}"),
                                b,
                                if d.dimension_id.is_some() {
                                    "dimension"
                                } else {
                                    "unallocated"
                                },
                                d.dimension_id.as_deref().unwrap_or(""),
                                d.cell.as_option().ok_or_else(malformed)?,
                            )?;
                        }
                    }
                    let mut result = csv.finish("crm-sales-scorecard.csv");
                    result.scorecard = Some(data).into();
                    Ok(result)
                } else if input.kind == Kind::SALES_ANALYTICS_EXPORT_KIND_SCORECARD_DETAIL {
                    let mut detail = request.detail.as_option().ok_or_else(invalid)?.clone();
                    if detail.expected_event_count > MAX_DETAIL as u64 {
                        return Err(capacity());
                    }
                    detail.after = Default::default();
                    detail.page_size = 100;
                    csv.fields(
                        "metadata",
                        "",
                        [
                            ("bucket_index", n(detail.bucket_index)),
                            ("organization", t(detail.organization)),
                            (
                                "dimension_id",
                                t(detail.dimension_id.as_deref().unwrap_or("")),
                            ),
                            ("source_count", n(detail.expected_event_count)),
                        ],
                    )?;
                    let mut count = 0usize;
                    loop {
                        request.detail = Some(detail.clone()).into();
                        let response = self.read_scorecard(request.clone()).await?;
                        if count == 0 {
                            score_metadata(&mut csv, &request, &response)?;
                        }
                        let page = response.detail.as_option().ok_or_else(malformed)?;
                        count += page.facts.len();
                        if count > MAX_DETAIL {
                            return Err(capacity());
                        }
                        for f in &page.facts {
                            fact(&mut csv, f)?;
                        }
                        if page.next.as_option().is_none() {
                            break;
                        }
                        if count >= MAX_DETAIL || page.facts.is_empty() {
                            return Err(capacity());
                        }
                        detail.after = page.next.clone();
                    }
                    if count as u64 != detail.expected_event_count {
                        return Err(incomplete());
                    }
                    Ok(csv.finish("crm-sales-scorecard-events.csv"))
                } else {
                    Err(invalid())
                }
            }
            None => Err(invalid()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn text_is_neutralized_and_signed_integers_remain_exact() {
        let mut csv = Csv::new().unwrap();
        csv.field("test", "1", "text", t("=SUM(1,2)")).unwrap();
        csv.field("test", "1", "negative", n("-9007199254740993"))
            .unwrap();
        assert!(csv.field("test", "1", "invalid", n("-1+2")).is_err());
        let data = csv.finish("test.csv");
        let content = String::from_utf8(data.csv.clone()).unwrap();
        assert!(content.contains("\"'=SUM(1,2)\""));
        assert!(content.contains("negative,-9007199254740993\n"));
        assert_eq!(
            data.csv_sha256_hex,
            party_export_hex(&party_export_sha256(&data.csv))
        );
    }
}
