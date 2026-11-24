//! Versioned commercial targets and exact event-based actuals.
use crate::{CommercialCloseFact, DealStatus, SalesOwnerScope};
use crm_core_contracts::{CalendarDate, CurrencyCode};
use crm_module_sdk::{ErrorCategory, SdkError};
use serde::{Deserialize, Serialize};

fn invalid(code: &'static str) -> SdkError {
    SdkError::new(
        code,
        ErrorCategory::InvalidArgument,
        false,
        "Invalid Sales quota.",
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuotaDate {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

impl QuotaDate {
    pub(crate) fn calendar(self) -> Result<CalendarDate, SdkError> {
        CalendarDate::try_new(self.year, self.month, self.day)
            .map_err(|_| invalid("SALES_QUOTA_DATE_INVALID"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuotaCadence {
    Monthly,
    Quarterly,
    Custom,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuotaPeriod {
    pub start_date: QuotaDate,
    pub end_date: QuotaDate,
    pub time_zone: String,
    pub cadence: QuotaCadence,
    pub start_unix_nanos: String,
    pub end_exclusive_unix_nanos: String,
}

impl QuotaPeriod {
    pub fn new(
        start_date: QuotaDate,
        end_date: QuotaDate,
        time_zone: String,
        cadence: QuotaCadence,
    ) -> Result<Self, SdkError> {
        let start = start_date.calendar()?;
        let end = end_date.calendar()?;
        let after = end
            .checked_add_days(1)
            .map_err(|_| invalid("SALES_QUOTA_PERIOD_INVALID"))?;
        if end < start
            || start.days_until(after) > 3660
            || time_zone.is_empty()
            || time_zone.len() > 128
        {
            return Err(invalid("SALES_QUOTA_PERIOD_INVALID"));
        }
        let months = match cadence {
            QuotaCadence::Monthly => Some(1),
            QuotaCadence::Quarterly => Some(3),
            QuotaCadence::Custom => None,
        };
        if let Some(months) = months {
            if start.day() != 1
                || (months == 3 && (start.month() - 1) % 3 != 0)
                || start.checked_add_months_clamped(months).ok() != Some(after)
            {
                return Err(invalid("SALES_QUOTA_CADENCE_INVALID"));
            }
        }
        let first = start
            .start_of_day(&time_zone)
            .map_err(|_| invalid("SALES_QUOTA_BOUNDARY_INVALID"))?
            .unix_nanos();
        let last = after
            .start_of_day(&time_zone)
            .map_err(|_| invalid("SALES_QUOTA_BOUNDARY_INVALID"))?
            .unix_nanos();
        if first >= last {
            return Err(invalid("SALES_QUOTA_PERIOD_INVALID"));
        }
        Ok(Self {
            start_date,
            end_date,
            time_zone,
            cadence,
            start_unix_nanos: first.to_string(),
            end_exclusive_unix_nanos: last.to_string(),
        })
    }

    pub fn bounds(&self) -> Result<(i64, i64), SdkError> {
        let start = self
            .start_unix_nanos
            .parse::<i64>()
            .map_err(|_| invalid("SALES_QUOTA_BOUNDARY_INVALID"))?;
        let end = self
            .end_exclusive_unix_nanos
            .parse::<i64>()
            .map_err(|_| invalid("SALES_QUOTA_BOUNDARY_INVALID"))?;
        if start >= end {
            return Err(invalid("SALES_QUOTA_PERIOD_INVALID"));
        }
        Ok((start, end))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SalesQuotaScope {
    Tenant,
    Owner(SalesOwnerScope),
    Territory(String),
}

impl SalesQuotaScope {
    pub fn dimension_id(&self) -> Option<&str> {
        match self {
            Self::Tenant => None,
            Self::Owner(SalesOwnerScope::Actor(id) | SalesOwnerScope::Team(id))
            | Self::Territory(id) => Some(id),
        }
    }

    fn credited_dimension<'a>(&self, fact: &'a CommercialCloseFact) -> Option<&'a str> {
        match self {
            Self::Tenant => None,
            Self::Owner(SalesOwnerScope::Actor(_)) => fact.credited_actor.as_deref(),
            Self::Owner(SalesOwnerScope::Team(_)) => {
                fact.credited_team.as_ref().map(|team| team.id.as_str())
            }
            Self::Territory(_) => fact
                .credited_territory
                .as_ref()
                .map(|territory| territory.id.as_str()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SalesQuota {
    pub quota_id: String,
    pub name: String,
    pub scope: SalesQuotaScope,
    pub pipeline_id: Option<String>,
    pub period: QuotaPeriod,
    pub currency_code: String,
    pub target_minor_units: String,
    pub published: bool,
    pub archived: bool,
    pub revision_reason: String,
    pub revision_at_unix_nanos: String,
    pub version: i64,
}

impl SalesQuota {
    pub fn target(&self) -> Result<i128, SdkError> {
        let value = self
            .target_minor_units
            .parse::<i64>()
            .map_err(|_| invalid("SALES_QUOTA_TARGET_INVALID"))?;
        if value <= 0 || value.to_string() != self.target_minor_units {
            return Err(invalid("SALES_QUOTA_TARGET_INVALID"));
        }
        Ok(i128::from(value))
    }

    pub fn validate(&self) -> Result<(), SdkError> {
        crate::territories::check_id(&self.quota_id)?;
        if self.version < 1
            || self.name.is_empty()
            || self.name.trim() != self.name
            || self.name.len() > 240
            || self.name.chars().any(char::is_control)
            || self.revision_reason.is_empty()
            || self.revision_reason.trim() != self.revision_reason
            || self.revision_reason.len() > 240
            || self.revision_reason.chars().any(char::is_control)
        {
            return Err(invalid("SALES_QUOTA_INVALID"));
        }
        match &self.scope {
            SalesQuotaScope::Tenant => {}
            SalesQuotaScope::Owner(owner) => owner.validate()?,
            SalesQuotaScope::Territory(id) => crate::territories::check_id(id)?,
        }
        if let Some(id) = &self.pipeline_id {
            crate::territories::check_id(id)?;
        }
        CurrencyCode::try_new(self.currency_code.clone())
            .map_err(|_| invalid("SALES_QUOTA_CURRENCY_INVALID"))?;
        self.target()?;
        self.period.bounds()?;
        self.period.start_date.calendar()?;
        self.period.end_date.calendar()?;
        self.revision_at_unix_nanos
            .parse::<i64>()
            .map_err(|_| invalid("SALES_QUOTA_REVISION_INVALID"))?;
        Ok(())
    }

    pub fn revise(
        mut self,
        old: Option<&Self>,
        expected: Option<i64>,
        now: i64,
    ) -> Result<Self, SdkError> {
        self.version = crate::territories::next_version(old.map(|q| q.version), expected)?;
        self.revision_at_unix_nanos = now.to_string();
        if let Some(old) = old {
            if old.archived || old.quota_id != self.quota_id {
                return Err(invalid("SALES_QUOTA_IMMUTABLE"));
            }
            if old.published
                && (!self.published
                    || old.scope != self.scope
                    || old.pipeline_id != self.pipeline_id
                    || old.period != self.period
                    || old.currency_code != self.currency_code)
            {
                return Err(invalid("SALES_QUOTA_PUBLISHED_SERIES_IMMUTABLE"));
            }
            if self.archived
                && (old.name != self.name
                    || old.scope != self.scope
                    || old.pipeline_id != self.pipeline_id
                    || old.period != self.period
                    || old.currency_code != self.currency_code
                    || old.target_minor_units != self.target_minor_units
                    || old.published != self.published)
            {
                return Err(invalid("SALES_QUOTA_ARCHIVE_ONLY"));
            }
        } else if self.archived {
            return Err(invalid("SALES_QUOTA_CREATE_ARCHIVED"));
        }
        self.validate()?;
        Ok(self)
    }

    pub fn decode(raw: &str) -> Result<Self, SdkError> {
        let value: Self =
            serde_json::from_str(raw).map_err(|_| invalid("SALES_QUOTA_STATE_INVALID"))?;
        value.validate()?;
        Ok(value)
    }
}

/// Homogeneous persisted event group. Storage supplies counts and exact
/// nonnegative decimal components; only Sales decides attribution and signs.
#[derive(Clone, Copy)]
pub struct QuotaFactGroup<'a> {
    pub outcome: Option<DealStatus>,
    pub dimension: Option<&'a str>,
    pub currency: Option<&'a str>,
    pub reversal: bool,
    pub booked_minor_units: Option<i128>,
    pub count: u64,
}

/// Reconstruct summed base-10^9 limbs (most significant first). Each limb
/// may include carry from many source amounts; no floating-point conversion.
pub fn quota_sum_limbs(limbs: [&str; 5]) -> Result<i128, SdkError> {
    limbs.into_iter().try_fold(0i128, |total, limb| {
        let value = limb
            .parse::<i128>()
            .map_err(|_| invalid("SALES_QUOTA_TOTAL_INVALID"))?;
        if value < 0 {
            return Err(invalid("SALES_QUOTA_TOTAL_INVALID"));
        }
        total
            .checked_mul(1_000_000_000)
            .and_then(|total| total.checked_add(value))
            .ok_or_else(|| invalid("SALES_QUOTA_TOTAL_OVERFLOW"))
    })
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct QuotaActuals {
    pub gross: i128,
    pub reversals: i128,
    pub won_count: u64,
    pub lost_count: u64,
    pub reversal_count: u64,
    pub missing_amount_count: u64,
    pub unknown_history_count: u64,
    pub unallocated_count: u64,
}

impl QuotaActuals {
    /// The same owner semantics are used for individual drilldown facts and
    /// grouped full-period totals. Unknown legacy facts are never attributed.
    pub fn observe_group(
        &mut self,
        quota: &SalesQuota,
        group: QuotaFactGroup<'_>,
    ) -> Result<bool, SdkError> {
        self.observe_scope(&quota.currency_code, quota.scope.dimension_id(), group)
    }

    /// Shared commercial calculation for quota and scorecard reads. A missing
    /// scope restriction means all known credit, not an inferred attribution.
    pub(crate) fn observe_scope(
        &mut self,
        currency: &str,
        dimension: Option<&str>,
        group: QuotaFactGroup<'_>,
    ) -> Result<bool, SdkError> {
        fn add(total: &mut u64, count: u64) -> Result<(), SdkError> {
            *total = total
                .checked_add(count)
                .ok_or_else(|| invalid("SALES_QUOTA_TOTAL_OVERFLOW"))?;
            Ok(())
        }
        let Some(outcome) = group.outcome else {
            add(&mut self.unknown_history_count, group.count)?;
            return Ok(false);
        };
        if outcome == DealStatus::Open
            || group.count == 0
            || group.booked_minor_units.is_some_and(|amount| amount < 0)
        {
            return Err(invalid("SALES_QUOTA_TOTAL_INVALID"));
        }
        if group
            .currency
            .is_some_and(|value| value != currency)
        {
            return Ok(false);
        }
        if let Some(dimension) = dimension {
            if group.dimension.is_none() {
                add(&mut self.unallocated_count, group.count)?;
            }
            if group.dimension != Some(dimension) {
                return Ok(false);
            }
        }
        if group.reversal {
            if outcome == DealStatus::Won {
                add(&mut self.reversal_count, group.count)?;
            }
        } else if outcome == DealStatus::Won {
            add(&mut self.won_count, group.count)?;
        } else {
            add(&mut self.lost_count, group.count)?;
        }
        let amount = if outcome == DealStatus::Lost {
            Some(0)
        } else {
            group.booked_minor_units
        };
        match amount {
            Some(value) if group.reversal => {
                self.reversals = self
                    .reversals
                    .checked_add(value)
                    .ok_or_else(|| invalid("SALES_QUOTA_TOTAL_OVERFLOW"))?;
            }
            Some(value) => {
                self.gross = self
                    .gross
                    .checked_add(value)
                    .ok_or_else(|| invalid("SALES_QUOTA_TOTAL_OVERFLOW"))?;
            }
            None => add(&mut self.missing_amount_count, group.count)?,
        }
        Ok(true)
    }

    pub fn observe(
        &mut self,
        quota: &SalesQuota,
        fact: Option<&CommercialCloseFact>,
        reversal: bool,
    ) -> Result<Option<Option<i128>>, SdkError> {
        let booked = fact
            .map(|fact| fact.booked_minor_units(false))
            .transpose()?
            .flatten();
        let included = self.observe_group(
            quota,
            QuotaFactGroup {
                outcome: fact.map(|fact| fact.outcome),
                dimension: fact.and_then(|fact| quota.scope.credited_dimension(fact)),
                currency: fact.and_then(|fact| {
                    fact.amount
                        .as_ref()
                        .map(|amount| amount.currency_code.as_str())
                }),
                reversal,
                booked_minor_units: booked,
                count: 1,
            },
        )?;
        Ok(included.then(|| booked.map(|value| if reversal { -value } else { value })))
    }

    pub fn target_progress(&self, quota: &SalesQuota) -> Result<(i128, i128), SdkError> {
        let net = self.net()?;
        let target = quota.target()?;
        let overflow = || invalid("SALES_QUOTA_TOTAL_OVERFLOW");
        let remaining = target.checked_sub(net).ok_or_else(overflow)?;
        let attainment = net.checked_mul(10_000).ok_or_else(overflow)? / target;
        Ok((remaining, attainment))
    }

    pub fn net(&self) -> Result<i128, SdkError> {
        self.gross
            .checked_sub(self.reversals)
            .ok_or_else(|| invalid("SALES_QUOTA_TOTAL_OVERFLOW"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(year: i32, month: u8, day: u8) -> QuotaDate {
        QuotaDate { year, month, day }
    }

    #[test]
    fn grouped_decimal_limbs_preserve_carry_and_overflow() {
        assert_eq!(
            quota_sum_limbs(["0", "0", "0", "18014398", "509481986"]).unwrap(),
            18_014_398_509_481_986
        );
        assert_eq!(
            quota_sum_limbs(["0", "0", "0", "2", "2000000001"]).unwrap(),
            4_000_000_001
        );
        assert!(
            quota_sum_limbs(["999", "999999999", "999999999", "999999999", "999999999"]).is_err()
        );
        assert!(quota_sum_limbs(["0", "0", "0", "0", "-1"]).is_err());
    }

    #[test]
    fn calendar_boundaries_use_pinned_dst_and_exclusive_end() {
        let period = QuotaPeriod::new(
            date(2026, 3, 1),
            date(2026, 3, 31),
            "Europe/Warsaw".into(),
            QuotaCadence::Monthly,
        )
        .unwrap();
        let (start, end) = period.bounds().unwrap();
        assert_eq!(end - start, (31 * 24 - 1) * 3_600_000_000_000);
        assert!(
            QuotaPeriod::new(
                date(2026, 3, 2),
                date(2026, 3, 31),
                "UTC".into(),
                QuotaCadence::Monthly,
            )
            .is_err()
        );
    }
}
