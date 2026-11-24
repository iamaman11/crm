//! Original Sales owner: reproducible commercial performance and forecast.
use crate::{
    CloseCreditContext, CommercialAmount, CommercialDimension, Deal, DealStatus, ForecastCategory,
    ProbabilitySource, QuotaActuals, QuotaDate, SalesOwnerScope, SalesQuota, SalesQuotaScope,
};
use crm_module_sdk::{ErrorCategory, SdkError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

fn invalid(code: &'static str) -> SdkError {
    SdkError::new(
        code,
        ErrorCategory::InvalidArgument,
        false,
        "Invalid Sales performance report.",
    )
}

fn overflow() -> SdkError {
    invalid("SALES_PERFORMANCE_OVERFLOW")
}

fn exact(value: &str) -> Result<i128, SdkError> {
    let parsed = value.parse::<i128>().map_err(|_| overflow())?;
    if parsed.to_string() != value {
        return Err(invalid("SALES_PERFORMANCE_REPORT_INVALID"));
    }
    Ok(parsed)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PerformanceActuals {
    pub gross: String,
    pub reversals: String,
    pub net: String,
    pub remaining: String,
    pub attainment: String,
    pub won_count: u64,
    pub lost_count: u64,
    pub reversal_count: u64,
    pub missing_amount_count: u64,
    pub unknown_history_count: u64,
    pub unallocated_count: u64,
}

impl PerformanceActuals {
    fn validate(&self, quota: &SalesQuota) -> Result<(), SdkError> {
        let gross = exact(&self.gross)?;
        let reversals = exact(&self.reversals)?;
        let net = exact(&self.net)?;
        let target = quota.target()?;
        if gross < 0
            || reversals < 0
            || gross.checked_sub(reversals) != Some(net)
            || target.checked_sub(net) != Some(exact(&self.remaining)?)
            || net.checked_mul(10_000).ok_or_else(overflow)? / target != exact(&self.attainment)?
        {
            return Err(invalid("SALES_PERFORMANCE_REPORT_INVALID"));
        }
        Ok(())
    }

    pub fn new(quota: &SalesQuota, actual: &QuotaActuals) -> Result<Self, SdkError> {
        let net = actual.net()?;
        let target = quota.target()?;
        let remaining = target.checked_sub(net).ok_or_else(overflow)?;
        let attainment = net.checked_mul(10_000).ok_or_else(overflow)? / target;
        Ok(Self {
            gross: actual.gross.to_string(),
            reversals: actual.reversals.to_string(),
            net: net.to_string(),
            remaining: remaining.to_string(),
            attainment: attainment.to_string(),
            won_count: actual.won_count,
            lost_count: actual.lost_count,
            reversal_count: actual.reversal_count,
            missing_amount_count: actual.missing_amount_count,
            unknown_history_count: actual.unknown_history_count,
            unallocated_count: actual.unallocated_count,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForecastDisposition {
    Included,
    Unallocated,
    Undated,
    Omitted,
    MissingAmount,
    OtherCurrency,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForecastSource {
    pub deal_id: String,
    pub deal_version: i64,
    pub pipeline_id: String,
    pub expected_close_date: Option<QuotaDate>,
    pub amount: Option<CommercialAmount>,
    pub probability_basis_points: u16,
    pub probability_source: ProbabilitySource,
    pub category: ForecastCategory,
    pub credited_actor: Option<String>,
    pub credited_team: Option<CommercialDimension>,
    pub credited_territory: Option<CommercialDimension>,
    pub disposition: ForecastDisposition,
}

impl ForecastSource {
    pub fn validate(&self) -> Result<(), SdkError> {
        crate::territories::check_id(&self.deal_id)?;
        crate::territories::check_id(&self.pipeline_id)?;
        if self.deal_version < 1 || self.probability_basis_points > 10_000 {
            return Err(invalid("SALES_FORECAST_SOURCE_INVALID"));
        }
        if let Some(date) = self.expected_close_date {
            date.calendar()?;
        }
        if let Some(amount) = &self.amount {
            if exact(&amount.minor_units)? < 0 {
                return Err(invalid("SALES_FORECAST_SOURCE_INVALID"));
            }
            crm_core_contracts::CurrencyCode::try_new(amount.currency_code.clone())
                .map_err(|_| invalid("SALES_FORECAST_SOURCE_INVALID"))?;
        }
        for dimension in [&self.credited_team, &self.credited_territory]
            .into_iter()
            .flatten()
        {
            if dimension.id.is_empty() || dimension.name.is_empty() || dimension.version < 1 {
                return Err(invalid("SALES_FORECAST_SOURCE_INVALID"));
            }
        }
        if self.disposition == ForecastDisposition::Included
            && (self.amount.is_none()
                || self.expected_close_date.is_none()
                || self.category == ForecastCategory::Omitted)
        {
            return Err(invalid("SALES_FORECAST_SOURCE_INVALID"));
        }
        Ok(())
    }

    /// Current automatic attribution is an estimate, not a historical close.
    /// A known different scope or a date outside the period is not a candidate.
    /// Remaining exclusion reasons are disjoint and ordered deliberately.
    pub fn capture(
        quota: &SalesQuota,
        deal: &Deal,
        context: CloseCreditContext,
    ) -> Result<Option<Self>, SdkError> {
        if deal.status() != DealStatus::Open
            || quota
                .pipeline_id
                .as_deref()
                .is_some_and(|id| id != deal.stage().pipeline_id().as_str())
        {
            return Ok(None);
        }
        let date = deal.expected_close_date();
        if let Some(date) = date
            && (date < quota.period.start_date.calendar()?
                || date > quota.period.end_date.calendar()?)
        {
            return Ok(None);
        }
        let credit = context.resolve(deal.owner());
        let dimension = match &quota.scope {
            SalesQuotaScope::Tenant => None,
            SalesQuotaScope::Owner(SalesOwnerScope::Actor(_)) => credit.actor.as_deref(),
            SalesQuotaScope::Owner(SalesOwnerScope::Team(_)) => {
                credit.team.as_ref().map(|team| team.id.as_str())
            }
            SalesQuotaScope::Territory(_) => credit
                .territory
                .as_ref()
                .map(|territory| territory.id.as_str()),
        };
        if let Some(id) = quota.scope.dimension_id()
            && dimension.is_some_and(|value| value != id)
        {
            return Ok(None);
        }
        let disposition = if quota.scope.dimension_id().is_some() && dimension.is_none() {
            ForecastDisposition::Unallocated
        } else if date.is_none() {
            ForecastDisposition::Undated
        } else if deal.forecast_category() == ForecastCategory::Omitted {
            ForecastDisposition::Omitted
        } else if let Some(amount) = deal.amount() {
            if amount.currency().as_str() == quota.currency_code {
                ForecastDisposition::Included
            } else {
                ForecastDisposition::OtherCurrency
            }
        } else {
            ForecastDisposition::MissingAmount
        };
        Ok(Some(Self {
            deal_id: deal.deal_id().as_str().to_owned(),
            deal_version: deal.version(),
            pipeline_id: deal.stage().pipeline_id().as_str().to_owned(),
            expected_close_date: date.map(|value| QuotaDate {
                year: value.year(),
                month: value.month(),
                day: value.day(),
            }),
            amount: deal.amount().map(|amount| CommercialAmount {
                minor_units: amount.minor_units().to_string(),
                currency_code: amount.currency().as_str().to_owned(),
            }),
            probability_basis_points: deal.probability().get(),
            probability_source: deal.probability_source(),
            category: deal.forecast_category(),
            credited_actor: credit.actor,
            credited_team: credit.team,
            credited_territory: credit.territory,
            disposition,
        }))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForecastBucket {
    pub category: ForecastCategory,
    pub count: u64,
    pub open: String,
    pub weighted: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForecastCoverage {
    pub open: String,
    pub weighted: String,
    pub open_coverage: Option<String>,
    pub weighted_coverage: Option<String>,
    pub included_count: u64,
    pub unallocated_count: u64,
    pub undated_count: u64,
    pub omitted_count: u64,
    pub missing_amount_count: u64,
    pub other_currency_count: u64,
    pub buckets: Vec<ForecastBucket>,
}

/// Constant-size running totals; pages do not own or duplicate formula logic.
pub struct ForecastAccumulator {
    value: ForecastCoverage,
    total: i128,
    weighted: i128,
    buckets: BTreeMap<ForecastCategory, (u64, i128, i128)>,
}

impl Default for ForecastAccumulator {
    fn default() -> Self {
        let value = ForecastCoverage {
            open: "0".into(),
            weighted: "0".into(),
            open_coverage: None,
            weighted_coverage: None,
            included_count: 0,
            unallocated_count: 0,
            undated_count: 0,
            omitted_count: 0,
            missing_amount_count: 0,
            other_currency_count: 0,
            buckets: Vec::new(),
        };
        Self {
            value,
            total: 0,
            weighted: 0,
            buckets: BTreeMap::new(),
        }
    }
}

impl ForecastAccumulator {
    pub fn observe(&mut self, source: &ForecastSource) -> Result<(), SdkError> {
        source.validate()?;
        let counter = match source.disposition {
            ForecastDisposition::Included => &mut self.value.included_count,
            ForecastDisposition::Unallocated => &mut self.value.unallocated_count,
            ForecastDisposition::Undated => &mut self.value.undated_count,
            ForecastDisposition::Omitted => &mut self.value.omitted_count,
            ForecastDisposition::MissingAmount => &mut self.value.missing_amount_count,
            ForecastDisposition::OtherCurrency => &mut self.value.other_currency_count,
        };
        *counter = counter.checked_add(1).ok_or_else(overflow)?;
        if source.disposition != ForecastDisposition::Included {
            return Ok(());
        }
        let units = source
            .amount
            .as_ref()
            .ok_or_else(overflow)?
            .minor_units
            .parse::<i128>()
            .map_err(|_| overflow())?;
        if units < 0 || source.probability_basis_points > 10_000 {
            return Err(invalid("SALES_FORECAST_SOURCE_INVALID"));
        }
        let weight = units
            .checked_mul(i128::from(source.probability_basis_points))
            .ok_or_else(overflow)?;
        self.total = self.total.checked_add(units).ok_or_else(overflow)?;
        self.weighted = self.weighted.checked_add(weight).ok_or_else(overflow)?;
        let entry = self.buckets.entry(source.category).or_default();
        entry.0 = entry.0.checked_add(1).ok_or_else(overflow)?;
        entry.1 = entry.1.checked_add(units).ok_or_else(overflow)?;
        entry.2 = entry.2.checked_add(weight).ok_or_else(overflow)?;
        Ok(())
    }

    pub fn finish(self, actual: &PerformanceActuals) -> Result<ForecastCoverage, SdkError> {
        let mut value = self.value;
        let total = self.total;
        let weighted = self.weighted;
        value.open = total.to_string();
        value.weighted = weighted.to_string();
        let remaining = actual.remaining.parse::<i128>().map_err(|_| overflow())?;
        if remaining > 0 {
            value.open_coverage =
                Some((total.checked_mul(10_000).ok_or_else(overflow)? / remaining).to_string());
            value.weighted_coverage = Some((weighted / remaining).to_string());
        }
        value.buckets = self
            .buckets
            .into_iter()
            .map(|(category, (count, open, weighted))| ForecastBucket {
                category,
                count,
                open: open.to_string(),
                weighted: weighted.to_string(),
            })
            .collect();
        Ok(value)
    }
}

impl ForecastCoverage {
    pub fn new(actual: &PerformanceActuals, sources: &[ForecastSource]) -> Result<Self, SdkError> {
        let mut accumulator = ForecastAccumulator::default();
        for source in sources {
            accumulator.observe(source)?;
        }
        accumulator.finish(actual)
    }

    fn validate(&self, actual: &PerformanceActuals) -> Result<(), SdkError> {
        let open = exact(&self.open)?;
        let weighted = exact(&self.weighted)?;
        let mut sum = (0u64, 0i128, 0i128);
        let mut previous = None;
        for bucket in &self.buckets {
            let amount = exact(&bucket.open)?;
            let weight = exact(&bucket.weighted)?;
            if bucket.count == 0
                || bucket.category == ForecastCategory::Omitted
                || previous.is_some_and(|p| p >= bucket.category)
                || amount < 0
                || weight < 0
                || weight > amount.checked_mul(10_000).ok_or_else(overflow)?
            {
                return Err(invalid("SALES_PERFORMANCE_REPORT_INVALID"));
            }
            previous = Some(bucket.category);
            sum.0 = sum.0.checked_add(bucket.count).ok_or_else(overflow)?;
            sum.1 = sum.1.checked_add(amount).ok_or_else(overflow)?;
            sum.2 = sum.2.checked_add(weight).ok_or_else(overflow)?;
        }
        let remaining = exact(&actual.remaining)?;
        let coverage = if remaining > 0 {
            (
                Some((open.checked_mul(10_000).ok_or_else(overflow)? / remaining).to_string()),
                Some((weighted / remaining).to_string()),
            )
        } else {
            (None, None)
        };
        if sum != (self.included_count, open, weighted)
            || coverage != (self.open_coverage.clone(), self.weighted_coverage.clone())
            || self.source_count().is_none()
        {
            return Err(invalid("SALES_PERFORMANCE_REPORT_INVALID"));
        }
        Ok(())
    }

    pub fn source_count(&self) -> Option<u64> {
        [
            self.included_count,
            self.unallocated_count,
            self.undated_count,
            self.omitted_count,
            self.missing_amount_count,
            self.other_currency_count,
        ]
        .into_iter()
        .try_fold(0u64, u64::checked_add)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PerformanceReport {
    pub report_id: String,
    pub name: String,
    pub quota: SalesQuota,
    pub captured_at: String,
    pub captured_by: String,
    pub event_sequence: String,
    pub formula_version: u32,
    pub actuals: PerformanceActuals,
    pub forecast: ForecastCoverage,
    pub actual_candidate_count: u64,
}

impl PerformanceReport {
    pub fn validate_name(name: &str) -> Result<(), SdkError> {
        if name.is_empty()
            || name.trim() != name
            || name.len() > 240
            || name.chars().any(char::is_control)
        {
            return Err(invalid("SALES_PERFORMANCE_REPORT_NAME_INVALID"));
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<(), SdkError> {
        crate::territories::check_id(&self.report_id)?;
        Self::validate_name(&self.name)?;
        self.quota.validate()?;
        self.actuals.validate(&self.quota)?;
        self.forecast.validate(&self.actuals)?;
        if self.formula_version != 1
            || self.captured_by.is_empty()
            || self.captured_at.parse::<i64>().ok().is_none_or(|n| n < 0)
            || self
                .event_sequence
                .parse::<i64>()
                .ok()
                .is_none_or(|n| n < 0)
            || self.forecast.source_count().is_none()
        {
            return Err(invalid("SALES_PERFORMANCE_REPORT_INVALID"));
        }
        Ok(())
    }

    pub fn decode(raw: &str) -> Result<Self, SdkError> {
        let value: Self =
            serde_json::from_str(raw).map_err(|_| invalid("SALES_PERFORMANCE_REPORT_INVALID"))?;
        value.validate()?;
        Ok(value)
    }
}

/// Comparing captures is not a replay of the mutable commercial ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonStatus {
    Comparable,
    DifferentSeries,
    DifferentFormula,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerformanceDelta {
    pub target: String,
    pub gross: String,
    pub reversals: String,
    pub net: String,
    pub remaining: String,
    pub attainment: String,
    pub open: String,
    pub weighted: String,
    pub open_coverage: Option<String>,
    pub weighted_coverage: Option<String>,
    pub won_count: i64,
    pub lost_count: i64,
    pub reversal_count: i64,
    pub missing_amount_count: i64,
    pub unknown_history_count: i64,
    pub unallocated_count: i64,
}

impl PerformanceReport {
    /// Selected capture minus baseline; no inferred time order or FX.
    pub fn compare(
        &self,
        baseline: &Self,
    ) -> Result<(ComparisonStatus, Option<PerformanceDelta>), SdkError> {
        if self.formula_version != baseline.formula_version {
            return Ok((ComparisonStatus::DifferentFormula, None));
        }
        self.validate()?;
        baseline.validate()?;
        let a = &self.quota;
        let b = &baseline.quota;
        if a.quota_id != b.quota_id
            || a.scope != b.scope
            || a.pipeline_id != b.pipeline_id
            || a.period != b.period
            || a.currency_code != b.currency_code
        {
            return Ok((ComparisonStatus::DifferentSeries, None));
        }
        let delta = |a: &str, b: &str| -> Result<String, SdkError> {
            Ok(exact(a)?
                .checked_sub(exact(b)?)
                .ok_or_else(overflow)?
                .to_string())
        };
        let optional = |a: &Option<String>, b: &Option<String>| {
            a.as_deref()
                .zip(b.as_deref())
                .map(|(a, b)| delta(a, b))
                .transpose()
        };
        let count =
            |a: u64, b: u64| i64::try_from(i128::from(a) - i128::from(b)).map_err(|_| overflow());
        let x = &self.actuals;
        let y = &baseline.actuals;
        let f = &self.forecast;
        let g = &baseline.forecast;
        Ok((
            ComparisonStatus::Comparable,
            Some(PerformanceDelta {
                target: delta(&a.target_minor_units, &b.target_minor_units)?,
                gross: delta(&x.gross, &y.gross)?,
                reversals: delta(&x.reversals, &y.reversals)?,
                net: delta(&x.net, &y.net)?,
                remaining: delta(&x.remaining, &y.remaining)?,
                attainment: delta(&x.attainment, &y.attainment)?,
                open: delta(&f.open, &g.open)?,
                weighted: delta(&f.weighted, &g.weighted)?,
                open_coverage: optional(&f.open_coverage, &g.open_coverage)?,
                weighted_coverage: optional(&f.weighted_coverage, &g.weighted_coverage)?,
                won_count: count(x.won_count, y.won_count)?,
                lost_count: count(x.lost_count, y.lost_count)?,
                reversal_count: count(x.reversal_count, y.reversal_count)?,
                missing_amount_count: count(x.missing_amount_count, y.missing_amount_count)?,
                unknown_history_count: count(x.unknown_history_count, y.unknown_history_count)?,
                unallocated_count: count(x.unallocated_count, y.unallocated_count)?,
            }),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frozen_comparison_preserves_signed_exact_deltas_and_series() {
        let quota = SalesQuota {
            quota_id: "quota-a".into(),
            name: "Quota".into(),
            scope: SalesQuotaScope::Tenant,
            pipeline_id: None,
            period: crate::QuotaPeriod::new(
                QuotaDate {
                    year: 2026,
                    month: 1,
                    day: 1,
                },
                QuotaDate {
                    year: 2026,
                    month: 1,
                    day: 31,
                },
                "UTC".into(),
                crate::QuotaCadence::Monthly,
            )
            .unwrap(),
            currency_code: "EUR".into(),
            target_minor_units: "1000".into(),
            published: true,
            archived: false,
            revision_reason: "Initial".into(),
            revision_at_unix_nanos: "1".into(),
            version: 1,
        };
        let actuals = PerformanceActuals {
            gross: "0".into(),
            reversals: "0".into(),
            net: "0".into(),
            remaining: "1000".into(),
            attainment: "0".into(),
            won_count: 0,
            lost_count: 0,
            reversal_count: 0,
            missing_amount_count: 0,
            unknown_history_count: 0,
            unallocated_count: 0,
        };
        let baseline = PerformanceReport {
            report_id: "report-a".into(),
            name: "Baseline".into(),
            quota,
            captured_at: "1".into(),
            captured_by: "actor".into(),
            event_sequence: "0".into(),
            formula_version: 1,
            forecast: ForecastCoverage::new(&actuals, &[]).unwrap(),
            actuals,
            actual_candidate_count: 0,
        };
        let mut selected = baseline.clone();
        selected.report_id = "report-b".into();
        selected.quota.version = 2;
        selected.quota.target_minor_units = "2000".into();
        selected.actuals.gross = "9007199254740993".into();
        selected.actuals.net = selected.actuals.gross.clone();
        selected.actuals.remaining = (2000i128 - 9007199254740993i128).to_string();
        selected.actuals.attainment = (9007199254740993i128 * 10_000 / 2000).to_string();
        selected.actuals.won_count = 1;
        selected.forecast = ForecastCoverage::new(&selected.actuals, &[]).unwrap();
        let (status, delta) = selected.compare(&baseline).unwrap();
        assert_eq!(status, ComparisonStatus::Comparable);
        let delta = delta.unwrap();
        assert_eq!(delta.target, "1000");
        assert_eq!(delta.net, "9007199254740993");
        assert_eq!(delta.won_count, 1);
        assert_eq!(delta.weighted_coverage, None);
        assert_eq!(
            baseline.compare(&selected).unwrap().1.unwrap().net,
            "-9007199254740993"
        );
        let unchanged = baseline.compare(&baseline).unwrap().1.unwrap();
        assert_eq!(unchanged.net, "0");
        assert_eq!(unchanged.open_coverage.as_deref(), Some("0"));
        assert_eq!(unchanged.weighted_coverage.as_deref(), Some("0"));
        let mut different_formula = selected.clone();
        different_formula.formula_version = 2;
        assert_eq!(
            different_formula.compare(&baseline).unwrap(),
            (ComparisonStatus::DifferentFormula, None)
        );
        selected.quota.currency_code = "USD".into();
        assert_eq!(
            selected.compare(&baseline).unwrap(),
            (ComparisonStatus::DifferentSeries, None)
        );
    }

    #[test]
    fn exact_weighted_coverage_and_disjoint_exclusions() {
        let actual = PerformanceActuals {
            gross: "0".into(),
            reversals: "0".into(),
            net: "0".into(),
            remaining: "1000".into(),
            attainment: "0".into(),
            won_count: 0,
            lost_count: 0,
            reversal_count: 0,
            missing_amount_count: 0,
            unknown_history_count: 0,
            unallocated_count: 0,
        };
        let source = ForecastSource {
            deal_id: "deal-a".into(),
            deal_version: 1,
            pipeline_id: "pipeline-a".into(),
            expected_close_date: Some(QuotaDate {
                year: 2026,
                month: 11,
                day: 24,
            }),
            amount: Some(CommercialAmount {
                minor_units: "9007199254740993".into(),
                currency_code: "EUR".into(),
            }),
            probability_basis_points: 3333,
            probability_source: ProbabilitySource::Manual,
            category: ForecastCategory::Commit,
            credited_actor: None,
            credited_team: None,
            credited_territory: None,
            disposition: ForecastDisposition::Included,
        };
        let mut excluded = source.clone();
        excluded.deal_id = "deal-b".into();
        excluded.disposition = ForecastDisposition::Unallocated;
        let mut streaming = ForecastAccumulator::default();
        streaming.observe(&source).unwrap();
        streaming.observe(&excluded).unwrap();
        let totals = ForecastCoverage::new(&actual, &[source, excluded]).unwrap();
        assert_eq!(streaming.finish(&actual).unwrap(), totals);
        assert_eq!(totals.open, "9007199254740993");
        assert_eq!(totals.weighted, (9007199254740993i128 * 3333).to_string());
        assert_eq!(totals.source_count(), Some(2));
        assert_eq!(totals.unallocated_count, 1);
        let mut complete = actual;
        complete.remaining = "0".into();
        assert_eq!(
            ForecastCoverage::new(&complete, &[])
                .unwrap()
                .weighted_coverage,
            None
        );
    }
}
