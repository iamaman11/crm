//! Original Sales owner: reproducible commercial performance and forecast.
use crate::{
    CloseCreditContext, CommercialAmount, CommercialDimension, Deal, DealStatus, ForecastCategory,
    ProbabilitySource, QuotaActuals, QuotaDate, SalesOwnerScope, SalesQuota, SalesQuotaScope,
};
use crm_module_sdk::{ErrorCategory, SdkError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

fn invalid(code: &'static str) -> SdkError {
    SdkError::new(code, ErrorCategory::InvalidArgument, false, "Invalid Sales performance report.")
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
        for dimension in [&self.credited_team, &self.credited_territory].into_iter().flatten() {
            if dimension.id.is_empty() || dimension.name.is_empty() || dimension.version < 1 {
                return Err(invalid("SALES_FORECAST_SOURCE_INVALID"));
            }
        }
        if self.disposition == ForecastDisposition::Included
            && (self.amount.is_none() || self.expected_close_date.is_none() || self.category == ForecastCategory::Omitted)
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
            || quota.pipeline_id.as_deref().is_some_and(|id| id != deal.stage().pipeline_id().as_str())
        {
            return Ok(None);
        }
        let date = deal.expected_close_date();
        if let Some(date) = date
            && (date < quota.period.start_date.calendar()? || date > quota.period.end_date.calendar()?)
        {
            return Ok(None);
        }
        let credit = context.resolve(deal.owner());
        let dimension = match &quota.scope {
            SalesQuotaScope::Tenant => None,
            SalesQuotaScope::Owner(SalesOwnerScope::Actor(_)) => credit.actor.as_deref(),
            SalesQuotaScope::Owner(SalesOwnerScope::Team(_)) => credit.team.as_ref().map(|team| team.id.as_str()),
            SalesQuotaScope::Territory(_) => credit.territory.as_ref().map(|territory| territory.id.as_str()),
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

impl ForecastCoverage {
    pub fn new(actual: &PerformanceActuals, sources: &[ForecastSource]) -> Result<Self, SdkError> {
        let mut value = Self {
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
        let mut total = 0i128;
        let mut weighted = 0i128;
        let mut buckets: BTreeMap<ForecastCategory, (u64, i128, i128)> = BTreeMap::new();
        for source in sources {
            source.validate()?;
            let counter = match source.disposition {
                ForecastDisposition::Included => &mut value.included_count,
                ForecastDisposition::Unallocated => &mut value.unallocated_count,
                ForecastDisposition::Undated => &mut value.undated_count,
                ForecastDisposition::Omitted => &mut value.omitted_count,
                ForecastDisposition::MissingAmount => &mut value.missing_amount_count,
                ForecastDisposition::OtherCurrency => &mut value.other_currency_count,
            };
            *counter = counter.checked_add(1).ok_or_else(overflow)?;
            if source.disposition != ForecastDisposition::Included {
                continue;
            }
            let units = source.amount.as_ref().ok_or_else(overflow)?.minor_units.parse::<i128>().map_err(|_| overflow())?;
            if units < 0 || source.probability_basis_points > 10_000 {
                return Err(invalid("SALES_FORECAST_SOURCE_INVALID"));
            }
            let weight = units.checked_mul(i128::from(source.probability_basis_points)).ok_or_else(overflow)?;
            total = total.checked_add(units).ok_or_else(overflow)?;
            weighted = weighted.checked_add(weight).ok_or_else(overflow)?;
            let entry = buckets.entry(source.category).or_default();
            entry.0 = entry.0.checked_add(1).ok_or_else(overflow)?;
            entry.1 = entry.1.checked_add(units).ok_or_else(overflow)?;
            entry.2 = entry.2.checked_add(weight).ok_or_else(overflow)?;
        }
        value.open = total.to_string();
        value.weighted = weighted.to_string();
        let remaining = actual.remaining.parse::<i128>().map_err(|_| overflow())?;
        if remaining > 0 {
            value.open_coverage = Some((total.checked_mul(10_000).ok_or_else(overflow)? / remaining).to_string());
            value.weighted_coverage = Some((weighted / remaining).to_string());
        }
        value.buckets = buckets.into_iter().map(|(category, (count, open, weighted))| ForecastBucket {
            category,
            count,
            open: open.to_string(),
            weighted: weighted.to_string(),
        }).collect();
        Ok(value)
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
        [self.included_count, self.unallocated_count, self.undated_count, self.omitted_count, self.missing_amount_count, self.other_currency_count]
            .into_iter().try_fold(0u64, u64::checked_add)
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
        if name.is_empty() || name.trim() != name || name.len() > 240 || name.chars().any(char::is_control) {
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
        if self.formula_version != 1 || self.captured_by.is_empty()
            || self.captured_at.parse::<i64>().ok().is_none_or(|n| n < 0)
            || self.event_sequence.parse::<i64>().ok().is_none_or(|n| n < 0)
            || self.forecast.source_count().is_none()
        {
            return Err(invalid("SALES_PERFORMANCE_REPORT_INVALID"));
        }
        Ok(())
    }

    pub fn decode(raw: &str) -> Result<Self, SdkError> {
        let value: Self = serde_json::from_str(raw).map_err(|_| invalid("SALES_PERFORMANCE_REPORT_INVALID"))?;
        value.validate()?;
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            expected_close_date: Some(QuotaDate { year: 2026, month: 11, day: 24 }),
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
        let totals = ForecastCoverage::new(&actual, &[source, excluded]).unwrap();
        assert_eq!(totals.open, "9007199254740993");
        assert_eq!(totals.weighted, (9007199254740993i128 * 3333).to_string());
        assert_eq!(totals.source_count(), Some(2));
        assert_eq!(totals.unallocated_count, 1);
        let mut complete = actual;
        complete.remaining = "0".into();
        assert_eq!(ForecastCoverage::new(&complete, &[]).unwrap().weighted_coverage, None);
    }
}
