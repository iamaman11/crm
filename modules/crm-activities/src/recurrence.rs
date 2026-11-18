//! Finite, date-only recurrence policy for the original Activities owner.
//! Not an RFC 5545 parser, timezone scheduler, or reminder delivery mechanism.
//! An occurrence is a civil date; a later owner operation must resolve its
//! local time explicitly before creating a UTC instant or external effect.

use crm_core_contracts::CalendarDate;
use crm_module_sdk::{ErrorCategory, FieldName, FieldViolation, SdkError};

/// A deliberately bounded subset of recurring task dates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskRecurrenceFrequency {
    Daily,
    Weekly,
    Monthly,
}

/// The first occurrence is always `start`. Further occurrences are
/// calculated from that original anchor, never from a previously clamped date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskRecurrence {
    frequency: TaskRecurrenceFrequency,
    interval: u16,
    count: u16,
}

impl TaskRecurrence {
    pub const MAX_INTERVAL: u16 = 120;
    pub const MAX_OCCURRENCES: u16 = 256;

    pub fn try_new(
        frequency: TaskRecurrenceFrequency,
        interval: u16,
        count: u16,
    ) -> Result<Self, SdkError> {
        if !(1..=Self::MAX_INTERVAL).contains(&interval) {
            return Err(invalid(
                "ACTIVITIES_RECURRENCE_INTERVAL_INVALID",
                "task.recurrence.interval",
                "recurrence interval must be between 1 and 120",
            ));
        }
        if !(1..=Self::MAX_OCCURRENCES).contains(&count) {
            return Err(invalid(
                "ACTIVITIES_RECURRENCE_COUNT_INVALID",
                "task.recurrence.count",
                "recurrence count must be between 1 and 256",
            ));
        }
        Ok(Self {
            frequency,
            interval,
            count,
        })
    }

    pub const fn frequency(self) -> TaskRecurrenceFrequency {
        self.frequency
    }

    pub const fn interval(self) -> u16 {
        self.interval
    }

    pub const fn count(self) -> u16 {
        self.count
    }

    /// Expand a finite series of civil dates, with no I/O or implicit clock.
    ///
    /// Monthly anchoring is intentional: Jan 31 -> Feb 28 -> Mar 31, not
    /// Jan 31 -> Feb 28 -> Mar 28. Crossing the 1..=9999 range fails in full.
    pub fn dates(self, start: CalendarDate) -> Result<Vec<CalendarDate>, SdkError> {
        let mut dates = Vec::with_capacity(usize::from(self.count));
        for occurrence in 0..self.count {
            // The constructor bounds the largest day offset to 255*120*7
            // and the largest month offset to 255*120; both fit i32 and i16.
            let offset = i32::from(occurrence) * i32::from(self.interval);
            let date = match self.frequency {
                TaskRecurrenceFrequency::Daily => start.checked_add_days(offset),
                TaskRecurrenceFrequency::Weekly => start.checked_add_days(offset * 7),
                TaskRecurrenceFrequency::Monthly => {
                    let months = i16::try_from(offset).map_err(|_| range_error())?;
                    start.checked_add_months_clamped(months)
                }
            }
            .map_err(|_| range_error())?;
            dates.push(date);
        }
        Ok(dates)
    }
}

fn invalid(code: &'static str, field: &'static str, message: &'static str) -> SdkError {
    let mut error = SdkError::new(
        code,
        ErrorCategory::InvalidArgument,
        false,
        "The task recurrence contains invalid data.",
    );
    error.field_violations.push(FieldViolation {
        field: FieldName::try_new(field).expect("static recurrence field path is valid"),
        code: code.to_owned(),
        safe_message: message.to_owned(),
    });
    error
}

fn range_error() -> SdkError {
    invalid(
        "ACTIVITIES_RECURRENCE_DATE_RANGE",
        "task.recurrence",
        "recurrence exceeds supported civil-date range",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(year: i32, month: u8, day: u8) -> CalendarDate {
        CalendarDate::try_new(year, month, day).unwrap()
    }

    fn rule(frequency: TaskRecurrenceFrequency, interval: u16, count: u16) -> TaskRecurrence {
        TaskRecurrence::try_new(frequency, interval, count).unwrap()
    }

    #[test]
    fn daily_crosses_leap_day_with_civil_not_elapsed_time() {
        let dates = rule(TaskRecurrenceFrequency::Daily, 1, 4)
            .dates(date(2024, 2, 27))
            .unwrap();
        assert_eq!(
            dates,
            vec![
                date(2024, 2, 27),
                date(2024, 2, 28),
                date(2024, 2, 29),
                date(2024, 3, 1),
            ]
        );
    }

    #[test]
    fn weekly_preserves_weekday_across_new_year_and_dst_season() {
        assert_eq!(
            rule(TaskRecurrenceFrequency::Weekly, 1, 3)
                .dates(date(2024, 12, 30))
                .unwrap(),
            vec![date(2024, 12, 30), date(2025, 1, 6), date(2025, 1, 13)]
        );
        assert_eq!(
            rule(TaskRecurrenceFrequency::Weekly, 1, 3)
                .dates(date(2026, 3, 22))
                .unwrap(),
            vec![date(2026, 3, 22), date(2026, 3, 29), date(2026, 4, 5)]
        );
    }

    #[test]
    fn monthly_uses_original_anchor_when_february_clamps() {
        assert_eq!(
            rule(TaskRecurrenceFrequency::Monthly, 1, 4)
                .dates(date(2023, 1, 31))
                .unwrap(),
            vec![
                date(2023, 1, 31),
                date(2023, 2, 28),
                date(2023, 3, 31),
                date(2023, 4, 30),
            ]
        );
        assert_eq!(
            rule(TaskRecurrenceFrequency::Monthly, 1, 3)
                .dates(date(2024, 1, 31))
                .unwrap(),
            vec![date(2024, 1, 31), date(2024, 2, 29), date(2024, 3, 31)]
        );
    }

    #[test]
    fn finite_bounds_are_enforced_without_allocating_unbounded_series() {
        for invalid_interval in [0, 121, u16::MAX] {
            assert_eq!(
                TaskRecurrence::try_new(TaskRecurrenceFrequency::Daily, invalid_interval, 1)
                    .unwrap_err()
                    .code,
                "ACTIVITIES_RECURRENCE_INTERVAL_INVALID"
            );
        }
        for invalid_count in [0, 257, u16::MAX] {
            assert_eq!(
                TaskRecurrence::try_new(TaskRecurrenceFrequency::Daily, 1, invalid_count)
                    .unwrap_err()
                    .code,
                "ACTIVITIES_RECURRENCE_COUNT_INVALID"
            );
        }
        let single = rule(TaskRecurrenceFrequency::Monthly, 120, 1);
        assert_eq!(
            single.dates(date(9999, 12, 31)).unwrap(),
            vec![date(9999, 12, 31)]
        );
        let maximum = rule(TaskRecurrenceFrequency::Daily, 1, 256);
        assert_eq!(maximum.dates(date(2026, 1, 1)).unwrap().len(), 256);
    }

    #[test]
    fn invalid_input_reports_original_field_without_leaking_values() {
        let interval = TaskRecurrence::try_new(TaskRecurrenceFrequency::Daily, 0, 1)
            .unwrap_err();
        assert_eq!(interval.category, ErrorCategory::InvalidArgument);
        assert_eq!(interval.field_violations.len(), 1);
        assert_eq!(
            interval.field_violations[0].field.as_str(),
            "task.recurrence.interval"
        );
        assert_eq!(
            interval.field_violations[0].code,
            "ACTIVITIES_RECURRENCE_INTERVAL_INVALID"
        );
        let count = TaskRecurrence::try_new(TaskRecurrenceFrequency::Daily, 1, 0)
            .unwrap_err();
        assert_eq!(count.field_violations.len(), 1);
        assert_eq!(
            count.field_violations[0].field.as_str(),
            "task.recurrence.count"
        );
        let range = rule(TaskRecurrenceFrequency::Daily, 1, 2)
            .dates(date(9999, 12, 31))
            .unwrap_err();
        assert_eq!(range.field_violations.len(), 1);
        assert_eq!(
            range.field_violations[0].field.as_str(),
            "task.recurrence"
        );
    }

    #[test]
    fn out_of_calendar_range_is_rejected_not_truncated() {
        assert_eq!(
            rule(TaskRecurrenceFrequency::Daily, 1, 2)
                .dates(date(9999, 12, 31))
                .unwrap_err()
                .code,
            "ACTIVITIES_RECURRENCE_DATE_RANGE"
        );
        assert_eq!(
            rule(TaskRecurrenceFrequency::Monthly, 1, 2)
                .dates(date(9999, 12, 31))
                .unwrap_err()
                .code,
            "ACTIVITIES_RECURRENCE_DATE_RANGE"
        );
        assert_eq!(
            rule(TaskRecurrenceFrequency::Weekly, 1, 2)
                .dates(date(1, 1, 1))
                .unwrap()[0],
            date(1, 1, 1)
        );
    }

    #[test]
    fn interval_and_count_are_explicitly_inspectable() {
        let repeat = rule(TaskRecurrenceFrequency::Weekly, 2, 4);
        assert_eq!(repeat.frequency(), TaskRecurrenceFrequency::Weekly);
        assert_eq!(repeat.interval(), 2);
        assert_eq!(repeat.count(), 4);
        assert_eq!(
            repeat.dates(date(2026, 11, 18)).unwrap(),
            vec![
                date(2026, 11, 18),
                date(2026, 12, 2),
                date(2026, 12, 16),
                date(2026, 12, 30),
            ]
        );
    }
}
