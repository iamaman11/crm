//! Versioned commercial targets and exact event-based actuals.
use crate::{CommercialCloseFact, DealStatus, SalesOwnerScope};
use crm_core_contracts::{CalendarDate, CurrencyCode};
use crm_module_sdk::{ErrorCategory, SdkError};
use serde::{Deserialize, Serialize};

fn invalid(code: &'static str) -> SdkError {
    SdkError::new(code, ErrorCategory::InvalidArgument, false, "Invalid Sales quota.")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuotaDate {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

impl QuotaDate {
    fn calendar(self) -> Result<CalendarDate, SdkError> {
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
    fn matches(&self, fact: &CommercialCloseFact) -> bool {
        match self {
            Self::Tenant => true,
            Self::Owner(SalesOwnerScope::Actor(id)) => fact.credited_actor.as_ref() == Some(id),
            Self::Owner(SalesOwnerScope::Team(id)) => fact
                .credited_team
                .as_ref()
                .is_some_and(|team| &team.id == id),
            Self::Territory(id) => fact
                .credited_territory
                .as_ref()
                .is_some_and(|territory| &territory.id == id),
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

#[derive(Debug, Default)]
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
    /// Return the signed contribution for drilldown, if the fact belongs to
    /// this quota. Unknown historical facts are never attributed retroactively.
    pub fn observe(
        &mut self,
        quota: &SalesQuota,
        fact: Option<&CommercialCloseFact>,
        reversal: bool,
    ) -> Result<Option<Option<i128>>, SdkError> {
        let Some(fact) = fact else {
            self.unknown_history_count += 1;
            return Ok(None);
        };
        fact.validate()?;
        if fact
            .amount
            .as_ref()
            .is_some_and(|amount| amount.currency_code != quota.currency_code)
        {
            return Ok(None);
        }
        let unallocated = match &quota.scope {
            SalesQuotaScope::Tenant => false,
            SalesQuotaScope::Owner(SalesOwnerScope::Actor(_)) => fact.credited_actor.is_none(),
            SalesQuotaScope::Owner(SalesOwnerScope::Team(_)) => fact.credited_team.is_none(),
            SalesQuotaScope::Territory(_) => fact.credited_territory.is_none(),
        };
        if unallocated {
            self.unallocated_count += 1;
        }
        if !quota.scope.matches(fact) {
            return Ok(None);
        }
        let amount = fact.booked_minor_units(reversal)?;
        if reversal {
            if fact.outcome == DealStatus::Won {
                self.reversal_count += 1;
            }
        } else if fact.outcome == DealStatus::Won {
            self.won_count += 1;
        } else {
            self.lost_count += 1;
        }
        match amount {
            Some(value) if reversal => {
                self.reversals = self
                    .reversals
                    .checked_sub(value)
                    .ok_or_else(|| invalid("SALES_QUOTA_TOTAL_OVERFLOW"))?;
            }
            Some(value) => {
                self.gross = self
                    .gross
                    .checked_add(value)
                    .ok_or_else(|| invalid("SALES_QUOTA_TOTAL_OVERFLOW"))?;
            }
            None => self.missing_amount_count += 1,
        }
        Ok(Some(amount))
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
