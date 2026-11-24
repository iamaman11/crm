//! Historical lifecycle intelligence over original immutable Deal events.
use crate::{DealStatus, QuotaCadence, QuotaDate, QuotaPeriod};
use crm_module_sdk::SdkError;
use serde::Deserialize;
use std::collections::BTreeMap;

pub const MAX_HISTORY_ROWS: usize = 4096;
pub const MAX_HISTORY_DEALS: usize = 512;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoryKind {
    Created,
    Updated,
    Moved,
    Closed,
    Reopened,
}

#[derive(Debug)]
pub struct HistoryEvent {
    pub version: i64,
    pub kind: HistoryKind,
    pub pipeline_id: String,
    pub stage_id: String,
    pub previous_stage_id: Option<String>,
    pub status: DealStatus,
    pub previous_status: Option<DealStatus>,
    pub reason_code: Option<String>,
    pub at: i64,
    pub catalog_version: Option<i64>,
}

#[derive(Debug)]
pub struct DealHistory {
    pub id: String,
    pub pipeline_id: String,
    pub version: i64,
    pub created_at: i64,
    pub events: Vec<HistoryEvent>,
}

impl DealHistory {
    // Conservative: an incomplete retained stream cannot establish an
    // authoritative historical state. Never fill a hole from current state.
    fn complete(&self) -> bool {
        if self.version <= 0 || self.events.len() as i64 != self.version {
            return false;
        }
        let mut previous: Option<&HistoryEvent> = None;
        for (index, event) in self.events.iter().enumerate() {
            if event.version != index as i64 + 1
                || event.at <= 0
                || event.pipeline_id != self.pipeline_id
                || event.stage_id.is_empty()
            {
                return false;
            }
            let valid = if let Some(old) = previous {
                event.at >= old.at
                    && event.previous_status == Some(old.status)
                    && event.previous_stage_id.as_deref() == Some(old.stage_id.as_str())
                    && match event.kind {
                        HistoryKind::Created => false,
                        HistoryKind::Updated => {
                            old.status == event.status && old.stage_id == event.stage_id
                        }
                        HistoryKind::Moved => {
                            old.status == DealStatus::Open
                                && event.status == DealStatus::Open
                                && old.stage_id != event.stage_id
                        }
                        HistoryKind::Closed => {
                            old.status == DealStatus::Open
                                && event.status != DealStatus::Open
                                && old.stage_id == event.stage_id
                        }
                        HistoryKind::Reopened => {
                            old.status != DealStatus::Open
                                && event.status == DealStatus::Open
                                && old.stage_id == event.stage_id
                        }
                    }
            } else {
                event.kind == HistoryKind::Created
                    && event.at == self.created_at
                    && event.status == DealStatus::Open
                    && event.previous_status.is_none()
                    && event.previous_stage_id.is_none()
            };
            if !valid {
                return false;
            }
            previous = Some(event);
        }
        true
    }
}

pub fn previous_history_period(period: &QuotaPeriod) -> Result<QuotaPeriod, SdkError> {
    let start = period.start_date.calendar()?;
    let end = period.end_date.calendar()?;
    // QuotaPeriod already bounds the inclusive span to 3660 days.
    let days = (start.days_until(end) + 1) as i32;
    let invalid = |_| {
        SdkError::new(
            "SALES_HISTORY_PERIOD_INVALID",
            crm_module_sdk::ErrorCategory::InvalidArgument,
            false,
            "Invalid historical period.",
        )
    };
    let date = |value: crm_core_contracts::CalendarDate| QuotaDate {
        year: value.year(),
        month: value.month(),
        day: value.day(),
    };
    QuotaPeriod::new(
        date(start.checked_add_days(-days).map_err(invalid)?),
        date(start.checked_add_days(-1).map_err(invalid)?),
        period.time_zone.clone(),
        QuotaCadence::Custom,
    )
}

#[derive(Debug, Default)]
pub struct HistoryDuration {
    pub count: u64,
    pub mean_seconds: Option<u64>,
    pub median_seconds: Option<u64>,
    pub p90_seconds: Option<u64>,
}

impl HistoryDuration {
    fn from_nanos(mut values: Vec<i64>) -> Self {
        if values.is_empty() {
            return Self::default();
        }
        values.sort_unstable();
        let len = values.len();
        let total: i128 = values.iter().map(|value| i128::from(*value)).sum();
        let rank = |percent: usize| {
            let index = (len * percent).div_ceil(100) - 1;
            (values[index] / 1_000_000_000) as u64
        };
        Self {
            count: len as u64,
            mean_seconds: Some((total / len as i128 / 1_000_000_000) as u64),
            median_seconds: Some(rank(50)),
            p90_seconds: Some(rank(90)),
        }
    }
}

#[derive(Debug, Default)]
pub struct HistoryOutcomes {
    pub open: u64,
    pub won: u64,
    pub lost: u64,
}

impl HistoryOutcomes {
    fn add(&mut self, status: DealStatus) {
        match status {
            DealStatus::Open => self.open += 1,
            DealStatus::Won => self.won += 1,
            DealStatus::Lost => self.lost += 1,
        }
    }

    pub fn denominator(&self) -> u64 {
        self.won + self.lost
    }

    pub fn win_rate_basis_points(&self) -> Option<u32> {
        let denominator = self.denominator();
        (denominator > 0).then(|| (self.won * 10_000 / denominator) as u32)
    }

    pub fn total(&self) -> u64 {
        self.open + self.won + self.lost
    }

    pub fn cohort_conversion_basis_points(&self) -> Option<u32> {
        let total = self.total();
        (total > 0).then(|| (self.won * 10_000 / total) as u32)
    }
}

#[derive(Debug)]
pub struct HistoryVisit {
    pub stage_id: String,
    pub catalog_version: Option<i64>,
    pub entry_version: i64,
    pub exit_version: i64,
    pub entry_at: i64,
    pub exit_at: i64,
}

impl HistoryVisit {
    pub fn seconds(&self) -> u64 {
        ((self.exit_at - self.entry_at) / 1_000_000_000) as u64
    }
}

#[derive(Debug)]
pub struct HistoricalDeal {
    pub id: String,
    pub pipeline_id: String,
    pub status: Option<DealStatus>,
    pub created_at: Option<i64>,
    pub last_version: Option<i64>,
    pub cohort: bool,
    pub final_close_in_period: bool,
    pub final_close_version: Option<i64>,
    pub final_close_at: Option<i64>,
    pub reason: Option<String>,
    pub cycle_seconds: Option<u64>,
    pub episode_seconds: Option<u64>,
    pub close_versions: Vec<i64>,
    pub reopen_versions: Vec<i64>,
    pub visits: Vec<HistoryVisit>,
}

#[derive(Debug)]
pub struct HistoryReason {
    pub won: bool,
    pub code: Option<String>,
    pub count: u64,
}

#[derive(Debug)]
pub struct HistoryStage {
    pub pipeline_id: String,
    pub stage_id: String,
    pub catalog_version: Option<i64>,
    pub duration: HistoryDuration,
}

#[derive(Debug)]
pub struct HistoryWindow {
    pub lo: i64,
    pub hi: i64,
    pub cutoff: i64,
    pub complete: bool,
    pub future: bool,
    pub state: HistoryOutcomes,
    pub final_closes: HistoryOutcomes,
    pub cohort: HistoryOutcomes,
    pub unknown_history: u64,
    pub close_events: u64,
    pub reopen_events: u64,
    pub cycle: HistoryDuration,
    pub episode: HistoryDuration,
    pub reasons: Vec<HistoryReason>,
    pub stages: Vec<HistoryStage>,
    pub deals: Vec<HistoricalDeal>,
}

/// Only complete periods with no detected retained-history gaps are eligible
/// for a rate comparison. A missing denominator is never treated as zero.
pub fn history_comparison(
    current: &HistoryWindow,
    previous: Option<&HistoryWindow>,
) -> (bool, Option<i32>) {
    let Some(previous) = previous else {
        return (false, None);
    };
    let comparable = previous.complete
        && current.complete
        && previous.unknown_history == 0
        && current.unknown_history == 0;
    let delta = if comparable {
        current
            .final_closes
            .win_rate_basis_points()
            .zip(previous.final_closes.win_rate_basis_points())
            .map(|(now, old)| now as i32 - old as i32)
    } else {
        None
    };
    (comparable, delta)
}

/// Both windows consume the same bounded, tenant-scoped retained history.
/// Cutoffs are exclusive; equal-time events are ordered by Deal version.
pub fn historical_window(
    histories: &[DealHistory],
    period: &QuotaPeriod,
    as_of: i64,
) -> Result<HistoryWindow, SdkError> {
    let (lo, hi) = period.bounds()?;
    let cutoff = hi.min(as_of.saturating_add(1));
    let future = cutoff <= lo;
    let mut result = HistoryWindow {
        lo,
        hi,
        cutoff,
        complete: hi <= as_of.saturating_add(1),
        future,
        state: HistoryOutcomes::default(),
        final_closes: HistoryOutcomes::default(),
        cohort: HistoryOutcomes::default(),
        unknown_history: 0,
        close_events: 0,
        reopen_events: 0,
        cycle: HistoryDuration::default(),
        episode: HistoryDuration::default(),
        reasons: Vec::new(),
        stages: Vec::new(),
        deals: Vec::new(),
    };
    if future {
        return Ok(result);
    }
    let mut cycles = Vec::new();
    let mut episodes = Vec::new();
    let mut reasons = BTreeMap::<(bool, Option<String>), u64>::new();
    let mut stages = BTreeMap::<(String, String, Option<i64>), Vec<i64>>::new();
    let in_window = |at: i64| at >= lo && at < cutoff;
    for history in histories {
        // Immutable creation time establishes membership even when the event
        // stream is incomplete. A Deal not yet created cannot be an unknown
        // historical Deal or contaminate an earlier period comparison.
        if history.created_at >= cutoff {
            continue;
        }
        let mut detail = HistoricalDeal {
            id: history.id.clone(),
            pipeline_id: history.pipeline_id.clone(),
            status: None,
            created_at: Some(history.created_at),
            last_version: None,
            cohort: false,
            final_close_in_period: false,
            final_close_version: None,
            final_close_at: None,
            reason: None,
            cycle_seconds: None,
            episode_seconds: None,
            close_versions: Vec::new(),
            reopen_versions: Vec::new(),
            visits: Vec::new(),
        };
        if !history.complete() {
            result.unknown_history += 1;
            result.deals.push(detail);
            continue;
        }
        let first = &history.events[0];
        detail.cohort = in_window(first.at);
        let mut entry = Some(first);
        let mut episode_start = first.at;
        let mut final_close: Option<&HistoryEvent> = None;
        for event in history.events.iter().take_while(|event| event.at < cutoff) {
            detail.status = Some(event.status);
            detail.last_version = Some(event.version);
            match event.kind {
                HistoryKind::Created | HistoryKind::Updated => (),
                HistoryKind::Moved | HistoryKind::Closed => {
                    if let Some(entered) = entry.take()
                        && in_window(event.at)
                    {
                        let duration = event.at - entered.at;
                        stages
                            .entry((
                                history.pipeline_id.clone(),
                                entered.stage_id.clone(),
                                entered.catalog_version,
                            ))
                            .or_default()
                            .push(duration);
                        detail.visits.push(HistoryVisit {
                            stage_id: entered.stage_id.clone(),
                            catalog_version: entered.catalog_version,
                            entry_version: entered.version,
                            exit_version: event.version,
                            entry_at: entered.at,
                            exit_at: event.at,
                        });
                    }
                    if event.kind == HistoryKind::Moved {
                        entry = Some(event);
                    } else {
                        final_close = Some(event);
                        if in_window(event.at) {
                            result.close_events += 1;
                            detail.close_versions.push(event.version);
                            episodes.push(event.at - episode_start);
                        }
                    }
                }
                HistoryKind::Reopened => {
                    final_close = None;
                    entry = Some(event);
                    episode_start = event.at;
                    if in_window(event.at) {
                        result.reopen_events += 1;
                        detail.reopen_versions.push(event.version);
                    }
                }
            }
        }
        if let Some(status) = detail.status {
            result.state.add(status);
            if detail.cohort {
                result.cohort.add(status);
            }
        }
        if let Some(closed) = final_close {
            detail.final_close_version = Some(closed.version);
            detail.final_close_at = Some(closed.at);
            detail.reason = closed.reason_code.clone().filter(|code| !code.is_empty());
            if in_window(closed.at) {
                detail.final_close_in_period = true;
                result.final_closes.add(closed.status);
                let cycle = closed.at - first.at;
                cycles.push(cycle);
                detail.cycle_seconds = Some((cycle / 1_000_000_000) as u64);
                detail.episode_seconds = Some(((closed.at - episode_start) / 1_000_000_000) as u64);
                *reasons
                    .entry((closed.status == DealStatus::Won, detail.reason.clone()))
                    .or_default() += 1;
            }
        }
        result.deals.push(detail);
    }
    result.cycle = HistoryDuration::from_nanos(cycles);
    result.episode = HistoryDuration::from_nanos(episodes);
    result.reasons = reasons
        .into_iter()
        .map(|((won, code), count)| HistoryReason { won, code, count })
        .collect();
    result.stages = stages
        .into_iter()
        .map(
            |((pipeline_id, stage_id, catalog_version), durations)| HistoryStage {
                pipeline_id,
                stage_id,
                catalog_version,
                duration: HistoryDuration::from_nanos(durations),
            },
        )
        .collect();
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(
        version: i64,
        kind: HistoryKind,
        at: i64,
        status: DealStatus,
        previous_status: Option<DealStatus>,
        stage: &str,
        previous_stage: Option<&str>,
    ) -> HistoryEvent {
        HistoryEvent {
            version,
            kind,
            pipeline_id: "p".into(),
            stage_id: stage.into(),
            previous_stage_id: previous_stage.map(str::to_owned),
            status,
            previous_status,
            reason_code: (kind == HistoryKind::Closed).then(|| "reason".into()),
            at,
            catalog_version: Some(1),
        }
    }

    #[test]
    fn historical_reopen_and_incomplete_history_are_not_current_state() {
        use DealStatus::{Lost, Open, Won};
        use HistoryKind::{Closed, Created, Moved, Reopened, Updated};
        let date = QuotaDate {
            year: 2026,
            month: 11,
            day: 1,
        };
        let period = QuotaPeriod::new(date, date, "UTC".into(), QuotaCadence::Custom).unwrap();
        let (lo, hi) = period.bounds().unwrap();
        let second = 1_000_000_000;
        let histories = [
            DealHistory {
                id: "a".into(),
                pipeline_id: "p".into(),
                version: 6,
                created_at: lo,
                events: vec![
                    event(1, Created, lo, Open, None, "lead", None),
                    event(
                        2,
                        Updated,
                        lo + second,
                        Open,
                        Some(Open),
                        "lead",
                        Some("lead"),
                    ),
                    event(
                        3,
                        Moved,
                        lo + 2 * second,
                        Open,
                        Some(Open),
                        "offer",
                        Some("lead"),
                    ),
                    event(
                        4,
                        Closed,
                        lo + 3 * second,
                        Won,
                        Some(Open),
                        "offer",
                        Some("offer"),
                    ),
                    event(5, Reopened, hi, Open, Some(Won), "offer", Some("offer")),
                    event(
                        6,
                        Closed,
                        hi + second,
                        Lost,
                        Some(Open),
                        "offer",
                        Some("offer"),
                    ),
                ],
            },
            DealHistory {
                id: "legacy".into(),
                pipeline_id: "p".into(),
                version: 2,
                created_at: lo - second,
                events: vec![],
            },
            DealHistory {
                id: "at-cutoff".into(),
                pipeline_id: "p".into(),
                version: 2,
                created_at: hi,
                events: vec![],
            },
            DealHistory {
                id: "after-cutoff".into(),
                pipeline_id: "p".into(),
                version: 1,
                created_at: hi + 2 * second,
                events: vec![],
            },
        ];
        let first = historical_window(&histories, &period, hi + second).unwrap();
        assert_eq!(first.final_closes.won, 1);
        assert_eq!(first.final_closes.lost, 0);
        assert_eq!(first.unknown_history, 1);
        assert_eq!(first.deals.len(), 2);
        assert_eq!(first.deals[1].created_at, Some(lo - second));
        assert_eq!(first.cohort.won, 1);
        assert_eq!(first.cycle.mean_seconds, Some(3));
        assert_eq!(first.deals[0].visits[0].seconds(), 2);
        assert_eq!(first.deals[0].visits[1].seconds(), 1);
        assert_eq!(first.deals[0].final_close_version, Some(4));
        let next_date = QuotaDate {
            year: 2026,
            month: 11,
            day: 2,
        };
        let next =
            QuotaPeriod::new(next_date, next_date, "UTC".into(), QuotaCadence::Custom).unwrap();
        let later = historical_window(&histories, &next, hi + second).unwrap();
        assert_eq!(later.final_closes.lost, 1);
        assert_eq!(later.unknown_history, 2);
        assert_eq!(later.deals.len(), 3);
        assert_eq!(later.reopen_events, 1);
        assert_eq!(later.episode.mean_seconds, Some(1));
        assert_eq!(later.cohort.total(), 0);
        assert!(!later.complete);
        assert_eq!(previous_history_period(&next).unwrap(), period);
    }

    #[test]
    fn immutable_creation_must_match_the_first_event() {
        let date = QuotaDate {
            year: 2026,
            month: 11,
            day: 1,
        };
        let period = QuotaPeriod::new(date, date, "UTC".into(), QuotaCadence::Custom).unwrap();
        let (lo, hi) = period.bounds().unwrap();
        let histories = [DealHistory {
            id: "inconsistent".into(),
            pipeline_id: "p".into(),
            version: 1,
            created_at: lo,
            events: vec![event(
                1,
                HistoryKind::Created,
                lo + 1,
                DealStatus::Open,
                None,
                "lead",
                None,
            )],
        }];
        let result = historical_window(&histories, &period, hi).unwrap();
        assert_eq!(result.unknown_history, 1);
        assert_eq!(result.state.total(), 0);
        assert_eq!(result.cohort.total(), 0);
        assert_eq!(result.deals[0].created_at, Some(lo));
        assert_eq!(result.deals[0].status, None);
    }
}
