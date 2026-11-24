//! Sales scorecards: one historical credit axis, exact calendar buckets and
//! optional matching goals. Storage supplies grouped facts, never business totals.
use crate::{
    QuotaActuals, QuotaCadence, QuotaDate, QuotaFactGroup, QuotaPeriod, SalesOwnerScope,
    SalesQuota, SalesQuotaScope,
};
use crm_core_contracts::CurrencyCode;
use crm_module_sdk::{ErrorCategory, SdkError};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_SCORECARD_PERIODS: usize = 24;
pub const MAX_SCORECARD_CELLS: usize = 2048;

fn invalid(code: &'static str) -> SdkError {
    SdkError::new(
        code,
        ErrorCategory::InvalidArgument,
        false,
        "Invalid Sales scorecard.",
    )
}

fn overflow() -> SdkError {
    invalid("SALES_SCORECARD_OVERFLOW")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScorecardAxis {
    Organization,
    Actor,
    Team,
    Territory,
}

impl ScorecardAxis {
    fn matches(self, scope: &SalesQuotaScope) -> bool {
        matches!(
            (self, scope),
            (Self::Organization, SalesQuotaScope::Tenant)
                | (
                    Self::Actor,
                    SalesQuotaScope::Owner(SalesOwnerScope::Actor(_))
                )
                | (Self::Team, SalesQuotaScope::Owner(SalesOwnerScope::Team(_)))
                | (Self::Territory, SalesQuotaScope::Territory(_))
        )
    }
}

#[derive(Debug)]
pub struct ScorecardCell {
    pub actual: QuotaActuals,
    pub source_event_count: u64,
    pub quota: Option<SalesQuota>,
}

impl ScorecardCell {
    fn empty() -> Self {
        Self {
            actual: QuotaActuals::default(),
            source_event_count: 0,
            quota: None,
        }
    }

    pub fn target_progress(&self) -> Result<Option<(i128, i128)>, SdkError> {
        self.quota
            .as_ref()
            .map(|quota| self.actual.target_progress(quota))
            .transpose()
    }

    /// Known close events, not unique deals or reversal-adjusted wins.
    pub fn win_rate_basis_points(&self) -> Option<u32> {
        let won = u128::from(self.actual.won_count);
        let closed = won + u128::from(self.actual.lost_count);
        (closed != 0).then(|| (won * 10_000 / closed) as u32)
    }
}

#[derive(Debug)]
pub struct ScorecardBucket {
    pub period: QuotaPeriod,
    /// Independent organization totals, never a sum of goals.
    pub organization: ScorecardCell,
    /// None is known events with no historical credit for this axis.
    /// Unknown legacy history is counted only in organization.
    pub dimensions: BTreeMap<Option<String>, ScorecardCell>,
}

#[derive(Debug)]
pub struct Scorecard {
    axis: ScorecardAxis,
    currency: String,
    pipeline_id: Option<String>,
    cells: usize,
    buckets: Vec<ScorecardBucket>,
}

impl Scorecard {
    /// Calendar windows are contiguous, full monthly/quarterly periods.
    /// Rust owns DST and local-day boundaries; SQL consumes exact instants.
    pub fn new(
        axis: ScorecardAxis,
        currency: String,
        pipeline_id: Option<String>,
        first: QuotaDate,
        time_zone: String,
        cadence: QuotaCadence,
        period_count: usize,
    ) -> Result<Self, SdkError> {
        CurrencyCode::try_new(currency.clone())
            .map_err(|_| invalid("SALES_SCORECARD_CURRENCY_INVALID"))?;
        if let Some(id) = &pipeline_id {
            crate::territories::check_id(id)?;
        }
        if !(1..=MAX_SCORECARD_PERIODS).contains(&period_count) {
            return Err(invalid("SALES_SCORECARD_PERIOD_COUNT_INVALID"));
        }
        let months = match cadence {
            QuotaCadence::Monthly => 1,
            QuotaCadence::Quarterly => 3,
            QuotaCadence::Custom => return Err(invalid("SALES_SCORECARD_CADENCE_INVALID")),
        };
        let mut start = first.calendar()?;
        let to_date = |value: crm_core_contracts::CalendarDate| QuotaDate {
            year: value.year(),
            month: value.month(),
            day: value.day(),
        };
        let date_error = |_| invalid("SALES_SCORECARD_PERIOD_INVALID");
        let mut buckets = Vec::with_capacity(period_count);
        for _ in 0..period_count {
            let after = start
                .checked_add_months_clamped(months)
                .map_err(date_error)?;
            let end = after.checked_add_days(-1).map_err(date_error)?;
            let period =
                QuotaPeriod::new(to_date(start), to_date(end), time_zone.clone(), cadence)?;
            buckets.push(ScorecardBucket {
                period,
                organization: ScorecardCell::empty(),
                dimensions: BTreeMap::new(),
            });
            start = after;
        }
        Ok(Self {
            axis,
            currency,
            pipeline_id,
            cells: period_count,
            buckets,
        })
    }

    pub fn buckets(&self) -> &[ScorecardBucket] {
        &self.buckets
    }

    pub fn into_buckets(self) -> Vec<ScorecardBucket> {
        self.buckets
    }

    fn dimension_cell(
        &mut self,
        bucket: usize,
        dimension: Option<String>,
    ) -> Result<&mut ScorecardCell, SdkError> {
        let row = self
            .buckets
            .get_mut(bucket)
            .ok_or_else(|| invalid("SALES_SCORECARD_BUCKET_INVALID"))?;
        if !row.dimensions.contains_key(&dimension) {
            if self.cells >= MAX_SCORECARD_CELLS {
                return Err(invalid("SALES_SCORECARD_CAPACITY"));
            }
            self.cells += 1;
        }
        Ok(row
            .dimensions
            .entry(dimension)
            .or_insert_with(ScorecardCell::empty))
    }

    /// Storage provides a homogeneous group with the selected axis's frozen
    /// credit. The same quota owner calculation handles signs and currencies.
    pub fn observe_group(
        &mut self,
        bucket: usize,
        group: QuotaFactGroup<'_>,
    ) -> Result<(), SdkError> {
        let row = self
            .buckets
            .get_mut(bucket)
            .ok_or_else(|| invalid("SALES_SCORECARD_BUCKET_INVALID"))?;
        if group.count == 0 {
            return Err(invalid("SALES_SCORECARD_GROUP_INVALID"));
        }
        let included = row
            .organization
            .actual
            .observe_scope(&self.currency, None, group)?;
        if included || group.outcome.is_none() {
            row.organization.source_event_count = row
                .organization
                .source_event_count
                .checked_add(group.count)
                .ok_or_else(overflow)?;
        }
        if !included || self.axis == ScorecardAxis::Organization {
            return Ok(());
        }
        if group.dimension.is_none() {
            row.organization.actual.unallocated_count = row
                .organization
                .actual
                .unallocated_count
                .checked_add(group.count)
                .ok_or_else(overflow)?;
        }
        let currency = self.currency.clone();
        let cell = self.dimension_cell(bucket, group.dimension.map(str::to_owned))?;
        cell.actual.observe_scope(&currency, None, group)?;
        cell.source_event_count = cell
            .source_event_count
            .checked_add(group.count)
            .ok_or_else(overflow)?;
        Ok(())
    }

    /// Complete the same dimension universe across periods. Catalog IDs are
    /// current display membership only; attribution is always from the fact.
    pub fn complete_dimensions(&mut self, active_ids: Vec<String>) -> Result<(), SdkError> {
        if self.axis == ScorecardAxis::Organization {
            return Ok(());
        }
        let mut ids: BTreeSet<Option<String>> = active_ids.into_iter().map(Some).collect();
        for row in &self.buckets {
            ids.extend(row.dimensions.keys().cloned());
        }
        let total = self
            .buckets
            .len()
            .checked_mul(ids.len().checked_add(1).ok_or_else(overflow)?)
            .ok_or_else(overflow)?;
        if total > MAX_SCORECARD_CELLS {
            return Err(invalid("SALES_SCORECARD_CAPACITY"));
        }
        for index in 0..self.buckets.len() {
            for id in &ids {
                self.dimension_cell(index, id.clone())?;
            }
        }
        Ok(())
    }

    /// One exact matching published goal per scope and calendar bucket.
    /// No prorating, fallback to a different scope, or summing of overlapping
    /// quotas. A valid unrelated goal is simply not part of this view.
    pub fn attach_quota(&mut self, quota: SalesQuota) -> Result<bool, SdkError> {
        quota.validate()?;
        if !quota.published
            || quota.currency_code != self.currency
            || quota.pipeline_id != self.pipeline_id
        {
            return Ok(false);
        }
        let Some(bucket) = self
            .buckets
            .iter()
            .position(|row| row.period == quota.period)
        else {
            return Ok(false);
        };
        let cell = if quota.scope == SalesQuotaScope::Tenant {
            &mut self.buckets[bucket].organization
        } else if self.axis.matches(&quota.scope) {
            self.dimension_cell(bucket, quota.scope.dimension_id().map(str::to_owned))?
        } else {
            return Ok(false);
        };
        if cell.quota.is_some() {
            return Err(invalid("SALES_SCORECARD_DUPLICATE_QUOTA"));
        }
        cell.quota = Some(quota);
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DealStatus;

    fn scorecard(axis: ScorecardAxis) -> Scorecard {
        Scorecard::new(
            axis,
            "EUR".into(),
            None,
            QuotaDate {
                year: 2026,
                month: 3,
                day: 1,
            },
            "Europe/Warsaw".into(),
            QuotaCadence::Monthly,
            2,
        )
        .unwrap()
    }

    fn group<'a>(
        outcome: Option<DealStatus>,
        dimension: Option<&'a str>,
        currency: Option<&'a str>,
        reversal: bool,
        booked_minor_units: Option<i128>,
        count: u64,
    ) -> QuotaFactGroup<'a> {
        QuotaFactGroup {
            outcome,
            dimension,
            currency,
            reversal,
            booked_minor_units,
            count,
        }
    }

    #[test]
    fn scorecard_reconciles_frozen_credit_and_cross_period_reversals() {
        let mut card = scorecard(ScorecardAxis::Team);
        let (start, end) = card.buckets()[0].period.bounds().unwrap();
        assert_eq!(end - start, (31 * 24 - 1) * 3_600_000_000_000);
        assert_eq!(end, card.buckets()[1].period.bounds().unwrap().0);
        let exact = 9_007_199_254_740_993i128;
        card.observe_group(
            0,
            group(
                Some(DealStatus::Won),
                Some("team-a"),
                Some("EUR"),
                false,
                Some(exact),
                1,
            ),
        )
        .unwrap();
        card.observe_group(
            0,
            group(Some(DealStatus::Lost), Some("team-a"), None, false, None, 1),
        )
        .unwrap();
        card.observe_group(
            0,
            group(Some(DealStatus::Won), None, Some("EUR"), false, Some(20), 1),
        )
        .unwrap();
        card.observe_group(0, group(None, None, None, false, None, 3))
            .unwrap();
        card.observe_group(
            0,
            group(
                Some(DealStatus::Won),
                Some("team-b"),
                Some("USD"),
                false,
                Some(999),
                1,
            ),
        )
        .unwrap();
        card.observe_group(
            1,
            group(
                Some(DealStatus::Won),
                Some("team-a"),
                Some("EUR"),
                true,
                Some(exact),
                1,
            ),
        )
        .unwrap();
        card.complete_dimensions(vec!["team-empty".into()]).unwrap();
        let first = &card.buckets()[0];
        let team = &first.dimensions[&Some("team-a".into())];
        assert_eq!(team.actual.net().unwrap(), exact);
        assert_eq!(team.win_rate_basis_points(), Some(5000));
        assert_eq!(first.organization.actual.net().unwrap(), exact + 20);
        assert_eq!(first.organization.actual.unknown_history_count, 3);
        assert_eq!(first.organization.source_event_count, 6);
        assert_eq!(team.source_event_count, 2);
        assert_eq!(card.buckets()[1].dimensions[&None].source_event_count, 0);
        assert_eq!(first.organization.actual.unallocated_count, 1);
        assert_eq!(first.dimensions[&None].actual.net().unwrap(), 20);
        assert!(!first.dimensions.contains_key(&Some("team-b".into())));
        assert_eq!(team.target_progress().unwrap(), None);
        let empty = &first.dimensions[&Some("team-empty".into())];
        assert_eq!(empty.win_rate_basis_points(), None);
        assert_eq!(empty.actual.net().unwrap(), 0);
        let next = &card.buckets()[1].dimensions[&Some("team-a".into())];
        assert_eq!(next.actual.net().unwrap(), -exact);
        assert_eq!(next.win_rate_basis_points(), None);
        assert_eq!(next.actual.reversal_count, 1);
    }

    #[test]
    fn scorecard_refuses_incomplete_dimension_universe() {
        let mut card = scorecard(ScorecardAxis::Team);
        let ids = (0..MAX_SCORECARD_CELLS)
            .map(|i| format!("team-{i}"))
            .collect();
        assert_eq!(
            card.complete_dimensions(ids).unwrap_err().code,
            "SALES_SCORECARD_CAPACITY"
        );
        assert!(card.buckets().iter().all(|row| row.dimensions.is_empty()));
    }

    #[test]
    fn scorecard_matches_exact_goal_without_summing_scopes() {
        let mut card = scorecard(ScorecardAxis::Actor);
        let goal = SalesQuota {
            quota_id: "quota-a".into(),
            name: "Organization".into(),
            scope: SalesQuotaScope::Tenant,
            pipeline_id: None,
            period: card.buckets()[0].period.clone(),
            currency_code: "EUR".into(),
            target_minor_units: "1000".into(),
            published: true,
            archived: true,
            revision_reason: "Archived".into(),
            revision_at_unix_nanos: "1".into(),
            version: 2,
        };
        assert!(card.attach_quota(goal.clone()).unwrap());
        let mut other_currency = goal.clone();
        other_currency.currency_code = "USD".into();
        assert!(!card.attach_quota(other_currency).unwrap());
        assert!(card.attach_quota(goal).is_err());
        let first = &card.buckets()[0];
        assert_eq!(
            first.organization.target_progress().unwrap(),
            Some((1000, 0))
        );
        assert!(first.dimensions.is_empty());
        assert_eq!(
            card.buckets()[1].organization.target_progress().unwrap(),
            None
        );
    }
}
