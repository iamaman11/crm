#![forbid(unsafe_code)]

use std::fmt;

pub use rust_decimal::{Decimal, RoundingStrategy};

/// Architecture marker for `crm-core-contracts`.
pub const CRATE_NAME: &str = "crm-core-contracts";

pub const MAX_CURSOR_BYTES: usize = 1024;
pub const MAX_PAGE_SIZE: u16 = 200;
pub const DEFAULT_PAGE_SIZE: u16 = 50;

/// NFC for human-language comparison keys only, never for entity identity,
/// credentials, local parts of email addresses or opaque provider identifiers.
/// The owner must retain the original display text and enforce its own bounds.
pub fn nfc_text_key(value: &str) -> String {
    use unicode_normalization::UnicodeNormalization as _;
    value.nfc().collect()
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{code}: {message}")]
pub struct ContractError {
    pub code: &'static str,
    pub message: String,
}

impl ContractError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

/// Current ISO 4217 currency with an assigned decimal minor unit.
/// Source: iso4217-static's embedded SIX List One. Reject non-monetary codes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CurrencyCode {
    code: String,
    exponent: u8,
}

impl CurrencyCode {
    pub fn try_new(value: impl Into<String>) -> Result<Self, ContractError> {
        let code = value.into();
        let invalid = || {
            ContractError::new(
                "CONTRACT_CURRENCY_INVALID",
                "currency must be a current ISO 4217 code with an assigned minor unit",
            )
        };
        if code.len() != 3 || !code.bytes().all(|byte| byte.is_ascii_uppercase()) {
            return Err(invalid());
        }
        let currency = iso4217_static::Currency::try_from(code.as_str()).map_err(|_| invalid())?;
        let exponent = currency.minor_unit().ok_or_else(invalid)?;
        Ok(Self { code, exponent })
    }

    pub fn as_str(&self) -> &str {
        &self.code
    }

    pub const fn exponent(&self) -> u8 {
        self.exponent
    }

    /// Enumerate assigned ISO 4217 numeric codes from the same pinned dataset.
    /// No duplicated alphabetic table or browser currency authority.
    pub fn catalog() -> Vec<Self> {
        let mut currencies: Vec<_> = (1..=999)
            .filter_map(|numeric| iso4217_static::Currency::from_u16(numeric).ok())
            .filter_map(|currency| Self::try_new(currency.as_str()).ok())
            .collect();
        currencies.sort_unstable();
        currencies
    }
}

impl fmt::Display for CurrencyCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Exact monetary value represented in the currency's minor units.
///
/// Binary floating point is intentionally impossible at this boundary.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Money {
    minor_units: i128,
    currency: CurrencyCode,
}

impl Money {
    pub fn new(minor_units: i128, currency: CurrencyCode) -> Self {
        Self {
            minor_units,
            currency,
        }
    }

    pub const fn minor_units(&self) -> i128 {
        self.minor_units
    }

    pub const fn currency(&self) -> &CurrencyCode {
        &self.currency
    }

    /// Optional decimal view for pricing; never narrow canonical Money implicitly.
    pub fn try_decimal(&self) -> Result<Decimal, ContractError> {
        Decimal::try_from_i128_with_scale(self.minor_units, u32::from(self.currency.exponent()))
            .map_err(|_| {
                ContractError::new("CONTRACT_DECIMAL_RANGE", "amount exceeds decimal range")
            })
    }

    /// Convert decimal major units to exact minor units, rejecting fractional minor units.
    pub fn from_decimal_exact(
        value: Decimal,
        currency: CurrencyCode,
    ) -> Result<Self, ContractError> {
        let exponent = u32::from(currency.exponent());
        let scale = value.scale();
        let mantissa = value.mantissa();
        let range =
            || ContractError::new("CONTRACT_MONEY_RANGE", "amount exceeds minor-unit range");
        let minor_units = if scale <= exponent {
            let factor = 10_i128.checked_pow(exponent - scale).ok_or_else(range)?;
            mantissa.checked_mul(factor).ok_or_else(range)?
        } else {
            let factor = 10_i128.checked_pow(scale - exponent).ok_or_else(range)?;
            if mantissa % factor != 0 {
                return Err(ContractError::new(
                    "CONTRACT_MONEY_PRECISION",
                    "amount has fractional minor units",
                ));
            }
            mantissa / factor
        };
        Ok(Self::new(minor_units, currency))
    }

    /// The original commercial owner chooses the rounding policy explicitly.
    pub fn from_decimal_rounded(
        value: Decimal,
        currency: CurrencyCode,
        strategy: RoundingStrategy,
    ) -> Result<Self, ContractError> {
        let rounded = value.round_dp_with_strategy(u32::from(currency.exponent()), strategy);
        Self::from_decimal_exact(rounded, currency)
    }

    pub fn non_negative(self) -> Result<Self, ContractError> {
        if self.minor_units < 0 {
            return Err(ContractError::new(
                "CONTRACT_MONEY_NEGATIVE",
                "money must not be negative for this use",
            ));
        }
        Ok(self)
    }
}

/// Inclusive probability or percentage value in basis points (0..=10,000).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BasisPoints(u16);

impl BasisPoints {
    pub fn try_new(value: u16) -> Result<Self, ContractError> {
        if value > 10_000 {
            return Err(ContractError::new(
                "CONTRACT_BASIS_POINTS_INVALID",
                "basis points must be between 0 and 10,000",
            ));
        }
        Ok(Self(value))
    }

    pub const fn get(self) -> u16 {
        self.0
    }
}

/// A Gregorian date in CRM's 1..=9999 civil calendar, without a time zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CalendarDate(jiff::civil::Date);

impl CalendarDate {
    pub fn year(self) -> i32 {
        i32::from(self.0.year())
    }

    pub fn month(self) -> u8 {
        self.0.month() as u8
    }

    pub fn day(self) -> u8 {
        self.0.day() as u8
    }

    pub fn try_new(year: i32, month: u8, day: u8) -> Result<Self, ContractError> {
        if !(1..=9999).contains(&year) {
            return Err(ContractError::new(
                "CONTRACT_DATE_YEAR_INVALID",
                "year must be between 1 and 9999",
            ));
        }
        if !(1..=12).contains(&month) {
            return Err(ContractError::new(
                "CONTRACT_DATE_MONTH_INVALID",
                "month must be between 1 and 12",
            ));
        }
        let invalid = || ContractError::new("CONTRACT_DATE_DAY_INVALID", "invalid calendar day");
        let first = jiff::civil::Date::new(year as i16, month as i8, 1).map_err(|_| invalid())?;
        let maximum_day = first.days_in_month() as u8;
        if day == 0 || day > maximum_day {
            return Err(ContractError::new(
                "CONTRACT_DATE_DAY_INVALID",
                format!("day must be between 1 and {maximum_day}"),
            ));
        }
        jiff::civil::Date::new(year as i16, month as i8, day as i8)
            .map(Self)
            .map_err(|_| invalid())
    }

    pub fn days_until(self, other: Self) -> i64 {
        self.0.duration_until(other.0).as_secs() / 86_400
    }

    pub fn from_utc_unix_nanos(nanos: i64) -> Result<Self, ContractError> {
        Ok(UtcInstant::from_unix_nanos(nanos).utc_date())
    }

    pub fn last_of_month(self) -> Self {
        Self(self.0.last_of_month())
    }

    pub fn weekday(self) -> jiff::civil::Weekday {
        self.0.weekday()
    }

    pub fn checked_add_months_clamped(self, months: i16) -> Result<Self, ContractError> {
        let invalid = || ContractError::new("CONTRACT_DATE_RANGE", "date arithmetic exceeds range");
        let date = self
            .0
            .checked_add(jiff::Span::new().months(months))
            .map_err(|_| invalid())?;
        if date.year() < 1 {
            return Err(invalid());
        }
        Ok(Self(date))
    }
}

/// Explicit choice for a skipped or repeated civil time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalTimeResolution {
    Reject,
    Earlier,
    Later,
}

/// Exact UTC instant constrained to the existing signed Unix-nanosecond boundary.
/// Every i64 nanosecond value fits Jiff's much wider supported instant range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UtcInstant(i64);

impl UtcInstant {
    pub const fn from_unix_nanos(nanos: i64) -> Self {
        Self(nanos)
    }

    pub const fn unix_nanos(self) -> i64 {
        self.0
    }

    pub fn from_proto_parts(seconds: i64, nanos: i32) -> Result<Self, ContractError> {
        if !(0..1_000_000_000).contains(&nanos) {
            return Err(ContractError::new(
                "CONTRACT_INSTANT_NANOS_INVALID",
                "invalid timestamp nanos",
            ));
        }
        // Widen first: i64::MIN is a valid result even though its whole
        // seconds component multiplied by 1e9 does not fit an i64.
        let total = i128::from(seconds) * 1_000_000_000 + i128::from(nanos);
        i64::try_from(total)
            .map(Self)
            .map_err(|_| Self::range_error())
    }

    pub fn proto_parts(self) -> (i64, i32) {
        (
            self.0.div_euclid(1_000_000_000),
            self.0.rem_euclid(1_000_000_000) as i32,
        )
    }

    fn range_error() -> ContractError {
        ContractError::new("CONTRACT_INSTANT_RANGE", "timestamp outside domain range")
    }

    fn timestamp(self) -> jiff::Timestamp {
        jiff::Timestamp::from_nanosecond(i128::from(self.0))
            .expect("all signed i64 Unix nanoseconds fit Jiff")
    }

    fn from_timestamp(timestamp: jiff::Timestamp) -> Result<Self, ContractError> {
        i64::try_from(timestamp.as_nanosecond())
            .map(Self)
            .map_err(|_| Self::range_error())
    }

    pub fn utc_date(self) -> CalendarDate {
        CalendarDate(self.timestamp().to_zoned(jiff::tz::TimeZone::UTC).date())
    }

    pub fn duration_until(self, other: Self) -> jiff::SignedDuration {
        self.timestamp().duration_until(other.timestamp())
    }

    pub fn checked_add_elapsed(
        self,
        duration: jiff::SignedDuration,
    ) -> Result<Self, ContractError> {
        let timestamp = self
            .timestamp()
            .checked_add(duration)
            .map_err(|_| Self::range_error())?;
        Self::from_timestamp(timestamp)
    }

    /// Use only the lockfile-pinned bundled IANA database, never host settings.
    pub fn resolve_local(
        local: jiff::civil::DateTime,
        zone_name: &str,
        resolution: LocalTimeResolution,
    ) -> Result<Self, ContractError> {
        let zone = jiff::tz::TimeZoneDatabase::bundled()
            .get(zone_name)
            .map_err(|_| ContractError::new("CONTRACT_TIME_ZONE_INVALID", "unknown time zone"))?;
        let ambiguous = zone.to_ambiguous_timestamp(local);
        let timestamp = match resolution {
            LocalTimeResolution::Reject => ambiguous.unambiguous(),
            LocalTimeResolution::Earlier => ambiguous.earlier(),
            LocalTimeResolution::Later => ambiguous.later(),
        }
        .map_err(|_| {
            ContractError::new(
                "CONTRACT_LOCAL_TIME_AMBIGUOUS",
                "local time requires explicit resolution",
            )
        })?;
        Self::from_timestamp(timestamp)
    }
}

/// Explicit update semantics. `Keep`, `Set` and `Clear` cannot be confused.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Patch<T> {
    #[default]
    Keep,
    Set(T),
    Clear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PageSize(u16);

impl PageSize {
    pub fn try_new(value: u16) -> Result<Self, ContractError> {
        if value == 0 || value > MAX_PAGE_SIZE {
            return Err(ContractError::new(
                "CONTRACT_PAGE_SIZE_INVALID",
                format!("page size must be between 1 and {MAX_PAGE_SIZE}"),
            ));
        }
        Ok(Self(value))
    }

    pub const fn get(self) -> u16 {
        self.0
    }
}

impl Default for PageSize {
    fn default() -> Self {
        Self(DEFAULT_PAGE_SIZE)
    }
}

/// Opaque bounded cursor. Its internal encoding is owned by the query provider.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Cursor(String);

impl Cursor {
    pub fn try_new(value: impl Into<String>) -> Result<Self, ContractError> {
        let value = value.into();
        if value.is_empty() {
            return Err(ContractError::new(
                "CONTRACT_CURSOR_EMPTY",
                "cursor must not be empty",
            ));
        }
        if value.len() > MAX_CURSOR_BYTES {
            return Err(ContractError::new(
                "CONTRACT_CURSOR_TOO_LONG",
                format!("cursor must not exceed {MAX_CURSOR_BYTES} bytes"),
            ));
        }
        if value.chars().any(char::is_control) {
            return Err(ContractError::new(
                "CONTRACT_CURSOR_INVALID",
                "cursor must not contain control characters",
            ));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PageRequest {
    pub cursor: Option<Cursor>,
    pub page_size: PageSize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<Cursor>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_error_preserves_code_message_and_standard_error() {
        let error = ContractError::new("CONTRACT_TEST", "safe detail");
        assert_eq!(error.code, "CONTRACT_TEST");
        assert_eq!(error.to_string(), "CONTRACT_TEST: safe detail");
        assert!(std::error::Error::source(&error).is_none());
    }

    #[test]
    fn money_is_exact_and_currency_is_strict() {
        let money = Money::new(12_345, CurrencyCode::try_new("USD").unwrap())
            .non_negative()
            .unwrap();
        assert_eq!(money.minor_units(), 12_345);
        assert_eq!(money.currency.as_str(), "USD");
        assert!(CurrencyCode::try_new("usd").is_err());
        assert!(
            Money::new(-1, CurrencyCode::try_new("EUR").unwrap())
                .non_negative()
                .is_err()
        );
    }

    #[test]
    fn currency_data_rejects_non_current_and_non_monetary_codes() {
        for (code, exponent) in [
            ("JPY", 0),
            ("EUR", 2),
            ("PLN", 2),
            ("KWD", 3),
            ("CLF", 4),
            ("XCG", 2),
        ] {
            assert_eq!(CurrencyCode::try_new(code).unwrap().exponent(), exponent);
        }
        for code in [
            "ZZZ", "XXX", "XAU", "BGN", "HRK", "CUC", "SLL", "ZWL", "usd",
        ] {
            assert!(CurrencyCode::try_new(code).is_err(), "{code}");
        }
    }

    #[test]
    fn catalog_contains_exactly_the_pinned_six_monetary_codes() {
        let catalog = CurrencyCode::catalog();
        // SIX List One 2026-09-17: 178 codes, 165 assigned minor units.
        assert_eq!(catalog.len(), 165);
        assert!(catalog.windows(2).all(|pair| pair[0] < pair[1]));
        for entry in &catalog {
            assert_eq!(CurrencyCode::try_new(entry.as_str()).unwrap(), *entry);
        }
        for code in ["USD", "EUR", "PLN", "JPY", "KWD", "CLF", "XCG"] {
            assert!(catalog.contains(&CurrencyCode::try_new(code).unwrap()));
        }
    }

    #[test]
    fn exact_decimal_pricing_never_narrows_canonical_money() {
        let eur = CurrencyCode::try_new("EUR").unwrap();
        let quantity = Decimal::from_str_exact("2.5").unwrap();
        let price = Decimal::from_str_exact("19.99").unwrap();
        let subtotal = quantity.checked_mul(price).unwrap();
        assert_eq!(subtotal, Decimal::from_str_exact("49.975").unwrap());
        assert!(Money::from_decimal_exact(subtotal, eur.clone()).is_err());
        let rounded = Money::from_decimal_rounded(
            subtotal,
            eur.clone(),
            RoundingStrategy::MidpointNearestEven,
        )
        .unwrap();
        assert_eq!(rounded.minor_units(), 4_998);
        assert_eq!(
            rounded.try_decimal().unwrap(),
            Decimal::from_str_exact("49.98").unwrap()
        );
        let tax = rounded
            .try_decimal()
            .unwrap()
            .checked_mul(Decimal::from_str_exact("0.23").unwrap())
            .unwrap();
        let tax_money =
            Money::from_decimal_rounded(tax, eur.clone(), RoundingStrategy::MidpointAwayFromZero)
                .unwrap();
        assert_eq!(tax_money.minor_units(), 1_150);
        let half_cent = Decimal::from_str_exact("0.005").unwrap();
        assert_eq!(
            Money::from_decimal_rounded(
                half_cent,
                eur.clone(),
                RoundingStrategy::MidpointNearestEven
            )
            .unwrap()
            .minor_units(),
            0
        );
        assert_eq!(
            Money::from_decimal_rounded(
                half_cent,
                eur.clone(),
                RoundingStrategy::MidpointAwayFromZero
            )
            .unwrap()
            .minor_units(),
            1
        );
        assert_eq!(
            Money::from_decimal_exact(Decimal::from_str_exact("-1.2500").unwrap(), eur.clone())
                .unwrap()
                .minor_units(),
            -125
        );
        assert!(Decimal::MAX.checked_mul(Decimal::from(2)).is_none());
        let huge = Money::new(i128::MAX, eur);
        assert_eq!(huge.minor_units(), i128::MAX);
        assert!(huge.try_decimal().is_err());
    }

    #[test]
    fn calendar_date_uses_gregorian_leap_year_rules() {
        assert!(CalendarDate::try_new(2024, 2, 29).is_ok());
        assert!(CalendarDate::try_new(2100, 2, 29).is_err());
        assert!(CalendarDate::try_new(2000, 2, 29).is_ok());
    }

    #[test]
    fn civil_calendar_month_end_and_utc_date_use_jiff() {
        let jan = CalendarDate::try_new(2024, 1, 31).unwrap();
        let feb = CalendarDate::try_new(2024, 2, 29).unwrap();
        assert_eq!(jan.checked_add_months_clamped(1).unwrap(), feb);
        assert_eq!(feb.last_of_month(), feb);
        assert_eq!(jan.days_until(feb), 29);
        assert_eq!(feb.days_until(jan), -29);
        assert_eq!(
            CalendarDate::from_utc_unix_nanos(-1).unwrap(),
            CalendarDate::try_new(1969, 12, 31).unwrap()
        );
        assert_eq!(
            CalendarDate::from_utc_unix_nanos(0).unwrap(),
            CalendarDate::try_new(1970, 1, 1).unwrap()
        );
        assert!(
            CalendarDate::try_new(1, 1, 1)
                .unwrap()
                .checked_add_months_clamped(-1)
                .is_err()
        );
        assert!(
            CalendarDate::try_new(9999, 12, 31)
                .unwrap()
                .checked_add_months_clamped(1)
                .is_err()
        );
    }

    #[test]
    fn utc_round_trip_is_lossless_at_storage_and_wire_boundaries() {
        for nanos in [i64::MIN, -1, 0, 1, i64::MAX] {
            let instant = UtcInstant::from_unix_nanos(nanos);
            let (seconds, part) = instant.proto_parts();
            assert_eq!(
                UtcInstant::from_proto_parts(seconds, part).unwrap(),
                instant
            );
        }
        assert_eq!(
            UtcInstant::from_unix_nanos(-1).proto_parts(),
            (-1, 999_999_999)
        );
        assert!(UtcInstant::from_proto_parts(0, -1).is_err());
        assert!(UtcInstant::from_proto_parts(0, 1_000_000_000).is_err());
        assert!(UtcInstant::from_proto_parts(i64::MAX, 0).is_err());
        assert!(
            UtcInstant::from_unix_nanos(i64::MAX)
                .checked_add_elapsed(jiff::SignedDuration::from_nanos(1))
                .is_err()
        );
        assert_eq!(
            UtcInstant::from_unix_nanos(0)
                .checked_add_elapsed(jiff::SignedDuration::from_secs(1))
                .unwrap()
                .unix_nanos(),
            1_000_000_000
        );
    }

    #[test]
    fn bundled_zone_requires_explicit_dst_resolution() {
        for value in ["2024-03-10T02:30:00", "2024-11-03T01:30:00"] {
            let local: jiff::civil::DateTime = value.parse().unwrap();
            let zone = "America/New_York";
            assert!(UtcInstant::resolve_local(local, zone, LocalTimeResolution::Reject).is_err());
            let earlier =
                UtcInstant::resolve_local(local, zone, LocalTimeResolution::Earlier).unwrap();
            let later = UtcInstant::resolve_local(local, zone, LocalTimeResolution::Later).unwrap();
            assert_eq!(earlier.duration_until(later).as_secs(), 3600);
        }
        let local: jiff::civil::DateTime = "2024-01-01T12:00:00".parse().unwrap();
        let warsaw =
            UtcInstant::resolve_local(local, "Europe/Warsaw", LocalTimeResolution::Reject).unwrap();
        assert_eq!(warsaw.proto_parts(), (1_704_106_800, 0));
        assert!(
            UtcInstant::resolve_local(local, "Invalid/Zone", LocalTimeResolution::Reject).is_err()
        );
    }

    #[test]
    fn pagination_is_bounded_and_cursor_is_opaque() {
        assert_eq!(PageSize::default().get(), 50);
        assert!(PageSize::try_new(0).is_err());
        assert!(PageSize::try_new(MAX_PAGE_SIZE + 1).is_err());
        assert_eq!(
            Cursor::try_new("signed.cursor").unwrap().as_str(),
            "signed.cursor"
        );
        assert!(Cursor::try_new("\n").is_err());
    }

    #[test]
    fn basis_points_are_bounded() {
        assert_eq!(BasisPoints::try_new(10_000).unwrap().get(), 10_000);
        assert!(BasisPoints::try_new(10_001).is_err());
    }
}
