//! UTC timestamps with microsecond precision.
//!
//! Log lines carry timestamps in several textual layouts; parsers convert them into
//! [`Timestamp`] using [`CivilDateTime`]. The conversion uses exact integer arithmetic
//! (Howard Hinnant's `days_from_civil`) so it is fast, allocation-free and dependency-free.

use std::fmt;

const MICROS_PER_SECOND: i64 = 1_000_000;
const SECONDS_PER_DAY: i64 = 86_400;
pub(crate) const MICROS_PER_DAY: i64 = SECONDS_PER_DAY * MICROS_PER_SECOND;

/// A UTC instant stored as microseconds since the Unix epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(i64);

impl Timestamp {
    /// Creates a timestamp from microseconds since `1970-01-01T00:00:00Z`.
    pub const fn from_unix_micros(micros: i64) -> Self {
        Self(micros)
    }

    /// Returns microseconds since `1970-01-01T00:00:00Z`.
    pub const fn as_unix_micros(self) -> i64 {
        self.0
    }

    /// The current system time. Clocks before 1970 are clamped to the epoch.
    pub fn now() -> Self {
        let micros = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_micros());
        Self(i64::try_from(micros).unwrap_or(i64::MAX))
    }

    /// Converts a local civil date-time with a UTC offset into a UTC timestamp.
    ///
    /// `offset_seconds` is the zone offset east of UTC (e.g. `+03:00` is `10_800`).
    pub fn from_civil(civil: CivilDateTime, offset_seconds: i32) -> Self {
        let days = days_from_civil(civil.year, civil.month, civil.day);
        let seconds = days * SECONDS_PER_DAY
            + i64::from(civil.hour) * 3_600
            + i64::from(civil.minute) * 60
            + i64::from(civil.second)
            - i64::from(offset_seconds);
        Self(seconds * MICROS_PER_SECOND + i64::from(civil.micros))
    }

    /// Returns the UTC civil date-time of this instant.
    pub fn to_civil(self) -> CivilDateTime {
        let days = self.0.div_euclid(MICROS_PER_DAY);
        let micros_of_day = self.0.rem_euclid(MICROS_PER_DAY);
        let (year, month, day) = civil_from_days(days);
        let seconds_of_day = micros_of_day / MICROS_PER_SECOND;
        CivilDateTime {
            year,
            month,
            day,
            hour: narrow(seconds_of_day / 3_600),
            minute: narrow(seconds_of_day % 3_600 / 60),
            second: narrow(seconds_of_day % 60),
            micros: narrow(micros_of_day % MICROS_PER_SECOND),
        }
    }
}

/// Formats as RFC 3339 in UTC, e.g. `2026-10-07T18:39:53Z` or `2026-10-07T18:39:53.250000Z`.
impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let c = self.to_civil();
        write!(
            f,
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
            c.year, c.month, c.day, c.hour, c.minute, c.second
        )?;
        if c.micros != 0 {
            write!(f, ".{:06}", c.micros)?;
        }
        f.write_str("Z")
    }
}

/// Serialized as RFC 3339 (see [`Display`](fmt::Display)).
impl serde::Serialize for Timestamp {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

/// A validated calendar date and wall-clock time without a zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CivilDateTime {
    year: i32,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
    micros: u32,
}

impl CivilDateTime {
    /// Validates and builds a civil date-time.
    ///
    /// A leap second (`second == 60`) is clamped to `59` because access logs never need
    /// sub-second ordering across a leap second. Returns `None` for out-of-range fields.
    pub fn new(
        year: i32,
        month: u8,
        day: u8,
        hour: u8,
        minute: u8,
        second: u8,
        micros: u32,
    ) -> Option<Self> {
        let valid = (1..=12).contains(&month)
            && day >= 1
            && day <= days_in_month(year, month)
            && hour < 24
            && minute < 60
            && second <= 60
            && micros < 1_000_000;
        valid.then_some(Self {
            year,
            month,
            day,
            hour,
            minute,
            second: second.min(59),
            micros,
        })
    }

    /// Calendar year.
    pub const fn year(&self) -> i32 {
        self.year
    }

    /// Month of year, `1..=12`.
    pub const fn month(&self) -> u8 {
        self.month
    }

    /// Day of month, `1..=31`.
    pub const fn day(&self) -> u8 {
        self.day
    }

    /// Hour of day, `0..=23`.
    pub const fn hour(&self) -> u8 {
        self.hour
    }
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        2 if is_leap_year(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Days since 1970-01-01 for a proleptic Gregorian date.
pub(crate) fn days_from_civil(year: i32, month: u8, day: u8) -> i64 {
    let year = i64::from(year) - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let year_of_era = year.rem_euclid(400);
    let month_from_march = (i64::from(month) + 9) % 12;
    let day_of_year = (153 * month_from_march + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// Inverse of [`days_from_civil`].
pub(crate) fn civil_from_days(days: i64) -> (i32, u8, u8) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_from_march = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_from_march + 2) / 5 + 1;
    let month = if month_from_march < 10 {
        month_from_march + 3
    } else {
        month_from_march - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (narrow(year), narrow(month), narrow(day))
}

/// Narrows a value that is in range by construction; saturates instead of panicking.
fn narrow<T: TryFrom<i64> + Bounded>(value: i64) -> T {
    T::try_from(value).unwrap_or(if value < 0 { T::MIN } else { T::MAX })
}

trait Bounded {
    const MIN: Self;
    const MAX: Self;
}

macro_rules! impl_bounded {
    ($($t:ty),*) => { $(impl Bounded for $t { const MIN: Self = <$t>::MIN; const MAX: Self = <$t>::MAX; })* };
}
impl_bounded!(u8, u32, i32);

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn civil(y: i32, mo: u8, d: u8, h: u8, mi: u8, s: u8, us: u32) -> CivilDateTime {
        CivilDateTime::new(y, mo, d, h, mi, s, us).unwrap()
    }

    #[test]
    fn epoch_is_zero() {
        let ts = Timestamp::from_civil(civil(1970, 1, 1, 0, 0, 0, 0), 0);
        assert_eq!(ts.as_unix_micros(), 0);
    }

    #[test]
    fn known_instant_matches_unix_time() {
        // `date -u -d 2026-10-07T18:39:53Z +%s` == 1791398393
        let ts = Timestamp::from_civil(civil(2026, 10, 7, 18, 39, 53, 0), 0);
        assert_eq!(ts.as_unix_micros(), 1_791_398_393_000_000);
        assert_eq!(ts.to_string(), "2026-10-07T18:39:53Z");
    }

    #[test]
    fn offset_is_subtracted() {
        let local = Timestamp::from_civil(civil(2026, 10, 7, 21, 39, 53, 0), 3 * 3_600);
        let utc = Timestamp::from_civil(civil(2026, 10, 7, 18, 39, 53, 0), 0);
        assert_eq!(local, utc);
    }

    #[test]
    fn display_includes_micros_when_present() {
        let ts = Timestamp::from_civil(civil(2024, 2, 29, 23, 59, 59, 250_000), 0);
        assert_eq!(ts.to_string(), "2024-02-29T23:59:59.250000Z");
    }

    #[test]
    fn rejects_invalid_dates() {
        assert!(CivilDateTime::new(2025, 2, 29, 0, 0, 0, 0).is_none());
        assert!(CivilDateTime::new(2025, 13, 1, 0, 0, 0, 0).is_none());
        assert!(CivilDateTime::new(2025, 4, 31, 0, 0, 0, 0).is_none());
        assert!(CivilDateTime::new(2025, 1, 1, 24, 0, 0, 0).is_none());
        assert!(CivilDateTime::new(2025, 1, 1, 0, 0, 0, 1_000_000).is_none());
    }

    #[test]
    fn leap_second_is_clamped() {
        let leap = Timestamp::from_civil(civil(2016, 12, 31, 23, 59, 60, 0), 0);
        let before = Timestamp::from_civil(civil(2016, 12, 31, 23, 59, 59, 0), 0);
        assert_eq!(leap, before);
    }

    proptest! {
        #[test]
        fn civil_round_trips(micros in -62_135_596_800_000_000_i64..253_402_300_799_999_999) {
            let ts = Timestamp::from_unix_micros(micros);
            prop_assert_eq!(Timestamp::from_civil(ts.to_civil(), 0), ts);
        }
    }
}
