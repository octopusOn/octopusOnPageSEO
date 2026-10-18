//! UTC calendar dates.

use std::fmt;

use crate::time::{MICROS_PER_DAY, Timestamp, civil_from_days, days_from_civil};

/// A proleptic Gregorian calendar date, stored as days since `1970-01-01`.
///
/// Serialized as `YYYY-MM-DD`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date(i32);

/// Years outside this range are rejected so day arithmetic and formatting never overflow.
const MIN_YEAR: i32 = 1;
const MAX_YEAR: i32 = 9_999;

impl Date {
    /// Builds a date from calendar fields; `None` if the date does not exist or the year
    /// is outside `1..=9999`.
    pub fn from_ymd(year: i32, month: u8, day: u8) -> Option<Self> {
        let valid_year = (MIN_YEAR..=MAX_YEAR).contains(&year);
        let civil = crate::CivilDateTime::new(year, month, day, 0, 0, 0, 0)?;
        let days = i32::try_from(days_from_civil(civil.year(), civil.month(), civil.day())).ok()?;
        valid_year.then_some(Self(days))
    }

    /// Parses strict `YYYY-MM-DD`.
    pub fn parse(text: &str) -> Option<Self> {
        let bytes = text.as_bytes();
        if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
            return None;
        }
        let year = parse_digits(&text[0..4])?;
        let month = parse_digits(&text[5..7])?;
        let day = parse_digits(&text[8..10])?;
        Self::from_ymd(
            i32::try_from(year).ok()?,
            u8::try_from(month).ok()?,
            u8::try_from(day).ok()?,
        )
    }

    /// Days since `1970-01-01` (negative before).
    pub const fn days_since_epoch(self) -> i32 {
        self.0
    }

    /// Inverse of [`Self::days_since_epoch`]; `None` outside years `1..=9999`.
    pub fn from_days_since_epoch(days: i32) -> Option<Self> {
        let min = Self::from_ymd(MIN_YEAR, 1, 1)?.0;
        let max = Self::from_ymd(MAX_YEAR, 12, 31)?.0;
        (min..=max).contains(&days).then_some(Self(days))
    }

    /// The UTC date containing `timestamp`.
    pub fn of(timestamp: Timestamp) -> Self {
        let days = timestamp.as_unix_micros().div_euclid(MICROS_PER_DAY);
        Self(i32::try_from(days).unwrap_or(if days < 0 { i32::MIN } else { i32::MAX }))
    }

    /// Today in UTC.
    pub fn today() -> Self {
        Self::of(Timestamp::now())
    }

    /// Midnight UTC at the start of this date.
    pub fn start(self) -> Timestamp {
        Timestamp::from_unix_micros(i64::from(self.0) * MICROS_PER_DAY)
    }

    /// The date `days` later (earlier when negative), saturating at the supported range.
    #[must_use]
    pub fn add_days(self, days: i32) -> Self {
        let min = Self::from_ymd(MIN_YEAR, 1, 1).map_or(i32::MIN, |d| d.0);
        let max = Self::from_ymd(MAX_YEAR, 12, 31).map_or(i32::MAX, |d| d.0);
        Self(self.0.saturating_add(days).clamp(min, max))
    }

    /// Calendar fields `(year, month, day)`.
    pub fn ymd(self) -> (i32, u8, u8) {
        civil_from_days(i64::from(self.0))
    }
}

fn parse_digits(text: &str) -> Option<u32> {
    text.bytes()
        .all(|b| b.is_ascii_digit())
        .then(|| text.parse().ok())
        .flatten()
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (year, month, day) = self.ymd();
        write!(f, "{year:04}-{month:02}-{day:02}")
    }
}

impl serde::Serialize for Date {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> serde::Deserialize<'de> for Date {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = <std::borrow::Cow<'de, str>>::deserialize(deserializer)?;
        Self::parse(&text).ok_or_else(|| {
            serde::de::Error::custom(format!("invalid date `{text}`, expected YYYY-MM-DD"))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn parses_and_formats() {
        let date = Date::parse("2026-10-07").unwrap();
        assert_eq!(date.to_string(), "2026-10-07");
        assert_eq!(date.ymd(), (2026, 10, 7));
        assert_eq!(Date::parse("1970-01-01").unwrap().days_since_epoch(), 0);
    }

    #[test]
    fn rejects_malformed_dates() {
        for text in [
            "2026-02-30",
            "2026-1-01",
            "2026/01/01",
            "+026-01-01",
            "0000-01-01",
            "",
        ] {
            assert_eq!(Date::parse(text), None, "{text}");
        }
    }

    #[test]
    fn converts_timestamps() {
        let date = Date::parse("2026-10-07").unwrap();
        assert_eq!(date.start().to_string(), "2026-10-07T00:00:00Z");
        let later = Timestamp::from_unix_micros(date.start().as_unix_micros() + 1);
        assert_eq!(Date::of(later), date);
        assert_eq!(date.add_days(-7).to_string(), "2026-09-30");
    }

    #[test]
    fn serializes_as_string() {
        let date = Date::parse("2024-02-29").unwrap();
        let json = serde_json::to_string(&date).unwrap();
        assert_eq!(json, "\"2024-02-29\"");
        assert_eq!(serde_json::from_str::<Date>(&json).unwrap(), date);
        assert!(serde_json::from_str::<Date>("\"2024-02-30\"").is_err());
    }

    proptest! {
        #[test]
        fn display_round_trips(days in -719_162_i32..2_932_896) {
            let date = Date(days);
            prop_assert_eq!(Date::parse(&date.to_string()), Some(date));
        }
    }
}
