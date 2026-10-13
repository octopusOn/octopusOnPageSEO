//! Timestamp layouts found in access logs.

use seonexus_core::{CivilDateTime, Timestamp};

use crate::values::parse_scaled_decimal;

/// Parses the Common Log Format layout `07/Oct/2026:18:39:53 +0000`.
pub(crate) fn parse_clf(value: &str) -> Option<Timestamp> {
    let b = value.as_bytes();
    if b.len() != 26 || b[2] != b'/' || b[6] != b'/' || b[11] != b':' || b[20] != b' ' {
        return None;
    }
    let civil = CivilDateTime::new(
        digits_i32(&b[7..11])?,
        month_from_abbrev(&b[3..6])?,
        digits_u8(&b[0..2])?,
        digits_u8(&b[12..14])?,
        expect_colon_then(b, 14, 15)?,
        expect_colon_then(b, 17, 18)?,
        0,
    )?;
    let offset = parse_offset(&b[21..])?;
    Some(Timestamp::from_civil(civil, offset))
}

/// Parses ISO 8601 / RFC 3339 such as `2026-10-07T18:39:53.207245731Z` or
/// `2026-10-07T21:39:53+03:00`. A space is accepted instead of `T`.
pub(crate) fn parse_iso8601(value: &str) -> Option<Timestamp> {
    let b = value.as_bytes();
    if b.len() < 20 || b[4] != b'-' || b[7] != b'-' || !matches!(b[10], b'T' | b' ') {
        return None;
    }
    let (micros, zone_start) = parse_fraction(b, 19)?;
    let civil = CivilDateTime::new(
        digits_i32(&b[0..4])?,
        digits_u8(&b[5..7])?,
        digits_u8(&b[8..10])?,
        digits_u8(&b[11..13])?,
        expect_colon_then(b, 13, 14)?,
        expect_colon_then(b, 16, 17)?,
        micros,
    )?;
    let offset = match &b[zone_start..] {
        b"Z" | b"z" => 0,
        zone => parse_offset(zone)?,
    };
    Some(Timestamp::from_civil(civil, offset))
}

/// Parses Unix epoch seconds with optional fraction, e.g. Nginx `$msec` `1791398393.123`.
pub(crate) fn parse_epoch_seconds(value: &str) -> Option<Timestamp> {
    let micros = parse_scaled_decimal(value, 6)?;
    i64::try_from(micros).ok().map(Timestamp::from_unix_micros)
}

/// Parses `YYYY-MM-DD` plus `HH:MM:SS[.fff]` given as separate values, in UTC (W3C extended
/// logs always use UTC).
pub(crate) fn parse_date_time_utc(date: &str, time: &str) -> Option<Timestamp> {
    let d = date.as_bytes();
    let t = time.as_bytes();
    if d.len() != 10 || d[4] != b'-' || d[7] != b'-' || t.len() < 8 {
        return None;
    }
    let (micros, end) = parse_fraction(t, 8)?;
    if end != t.len() {
        return None;
    }
    let civil = CivilDateTime::new(
        digits_i32(&d[0..4])?,
        digits_u8(&d[5..7])?,
        digits_u8(&d[8..10])?,
        digits_u8(&t[0..2])?,
        expect_colon_then(t, 2, 3)?,
        expect_colon_then(t, 5, 6)?,
        micros,
    )?;
    Some(Timestamp::from_civil(civil, 0))
}

/// Parses HAProxy's accept date `06/Feb/2026:12:14:14.655`. HAProxy logs no zone; the value
/// is taken as UTC.
pub(crate) fn parse_haproxy(value: &str) -> Option<Timestamp> {
    let b = value.as_bytes();
    if b.len() < 20 || b[2] != b'/' || b[6] != b'/' || b[11] != b':' {
        return None;
    }
    let (micros, end) = parse_fraction(b, 20)?;
    if end != b.len() {
        return None;
    }
    let civil = CivilDateTime::new(
        digits_i32(&b[7..11])?,
        month_from_abbrev(&b[3..6])?,
        digits_u8(&b[0..2])?,
        digits_u8(&b[12..14])?,
        expect_colon_then(b, 14, 15)?,
        expect_colon_then(b, 17, 18)?,
        micros,
    )?;
    Some(Timestamp::from_civil(civil, 0))
}

/// Parses whatever timestamp a structured log (CSV, JSON) may carry: ISO 8601 with or without
/// zone (no zone = UTC), the Common Log Format layout, or Unix epoch seconds, milliseconds,
/// microseconds or nanoseconds (picked by magnitude).
pub(crate) fn parse_flexible(value: &str) -> Option<Timestamp> {
    let value = value.trim();
    if let Some(ts) = parse_iso8601(value) {
        return Some(ts);
    }
    let b = value.as_bytes();
    if b.len() >= 19 && b[4] == b'-' && matches!(b[10], b'T' | b' ') {
        let (_, end) = parse_fraction(b, 19)?;
        if end == b.len() {
            return parse_date_time_utc(&value[..10], &value[11..]);
        }
    }
    if let Some(ts) = parse_clf(value) {
        return Some(ts);
    }
    parse_epoch_any(value)
}

/// Unix epoch with an optional fraction; integers of 13, 16 or 19 digits are milliseconds,
/// microseconds or nanoseconds.
fn parse_epoch_any(value: &str) -> Option<Timestamp> {
    let integer_digits = value.split('.').next().unwrap_or("").len();
    let scale = match integer_digits {
        9 | 10 => 6,
        12 | 13 => 3,
        15 | 16 => 0,
        18 | 19 => {
            let nanos = crate::values::parse_u64(value)?;
            return i64::try_from(nanos / 1_000)
                .ok()
                .map(Timestamp::from_unix_micros);
        }
        _ => return None,
    };
    let micros = parse_scaled_decimal(value, scale)?;
    i64::try_from(micros).ok().map(Timestamp::from_unix_micros)
}

/// Parses an optional `.ddd…` fraction starting at `start`; returns micros and the index
/// after it.
fn parse_fraction(b: &[u8], start: usize) -> Option<(u32, usize)> {
    if b.get(start) != Some(&b'.') {
        return Some((0, start));
    }
    let digits_start = start + 1;
    let digits_len = b[digits_start..]
        .iter()
        .take_while(|c| c.is_ascii_digit())
        .count();
    if digits_len == 0 {
        return None;
    }
    let mut micros = 0_u32;
    for i in 0..6 {
        let digit = if i < digits_len {
            u32::from(b[digits_start + i] - b'0')
        } else {
            0
        };
        micros = micros * 10 + digit;
    }
    Some((micros, digits_start + digits_len))
}

/// Parses `+HHMM`, `-HHMM`, `+HH:MM` or `-HH:MM` into seconds east of UTC.
fn parse_offset(b: &[u8]) -> Option<i32> {
    let sign = match b.first()? {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    let (hours, minutes) = match b.len() {
        5 => (&b[1..3], &b[3..5]),
        6 if b[3] == b':' => (&b[1..3], &b[4..6]),
        _ => return None,
    };
    let hours = i32::from(digits_u8(hours)?);
    let minutes = i32::from(digits_u8(minutes)?);
    (hours <= 23 && minutes <= 59).then_some(sign * (hours * 3_600 + minutes * 60))
}

/// Requires `b[colon] == ':'` and parses two digits at `digits..digits + 2`.
fn expect_colon_then(b: &[u8], colon: usize, digits: usize) -> Option<u8> {
    (b[colon] == b':').then_some(())?;
    digits_u8(&b[digits..digits + 2])
}

fn digits_u8(b: &[u8]) -> Option<u8> {
    b.iter().try_fold(0_u8, |acc, &c| {
        c.is_ascii_digit()
            .then(|| acc.checked_mul(10)?.checked_add(c - b'0'))
            .flatten()
    })
}

fn digits_i32(b: &[u8]) -> Option<i32> {
    b.iter().try_fold(0_i32, |acc, &c| {
        c.is_ascii_digit()
            .then(|| acc.checked_mul(10)?.checked_add(i32::from(c - b'0')))
            .flatten()
    })
}

fn month_from_abbrev(b: &[u8]) -> Option<u8> {
    const MONTHS: [&[u8; 3]; 12] = [
        b"Jan", b"Feb", b"Mar", b"Apr", b"May", b"Jun", b"Jul", b"Aug", b"Sep", b"Oct", b"Nov",
        b"Dec",
    ];
    MONTHS
        .iter()
        .position(|m| m.as_slice() == b)
        .and_then(|i| u8::try_from(i + 1).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    const REFERENCE: i64 = 1_791_398_393_000_000; // 2026-10-07T18:39:53Z

    #[test]
    fn clf_with_offsets() {
        assert_eq!(
            parse_clf("07/Oct/2026:18:39:53 +0000").map(Timestamp::as_unix_micros),
            Some(REFERENCE)
        );
        assert_eq!(
            parse_clf("07/Oct/2026:21:39:53 +0300").map(Timestamp::as_unix_micros),
            Some(REFERENCE)
        );
        assert_eq!(
            parse_clf("07/Oct/2026:13:39:53 -0500").map(Timestamp::as_unix_micros),
            Some(REFERENCE)
        );
    }

    #[test]
    fn clf_rejects_garbage() {
        assert!(parse_clf("07/Foo/2026:18:39:53 +0000").is_none());
        assert!(parse_clf("32/Oct/2026:18:39:53 +0000").is_none());
        assert!(parse_clf("07/Oct/2026 18:39:53 +0000").is_none());
        assert!(parse_clf("07/Oct/2026:18:39:53").is_none());
        assert!(parse_clf("").is_none());
    }

    #[test]
    fn iso8601_variants() {
        let expect = Some(REFERENCE);
        let ts = |s| parse_iso8601(s).map(Timestamp::as_unix_micros);
        assert_eq!(ts("2026-10-07T18:39:53Z"), expect);
        assert_eq!(ts("2026-10-07T21:39:53+03:00"), expect);
        assert_eq!(ts("2026-10-07 18:39:53+0000"), expect);
        assert_eq!(
            ts("2026-10-07T18:39:53.207245731Z"),
            Some(REFERENCE + 207_245)
        );
        assert_eq!(ts("2026-10-07T18:39:53.5Z"), Some(REFERENCE + 500_000));
        assert_eq!(ts("2026-10-07T18:39:53."), None);
        assert_eq!(ts("2026-10-07T18:39:53"), None);
    }

    #[test]
    fn separate_date_and_time_and_haproxy_dates() {
        assert_eq!(
            parse_date_time_utc("2026-10-07", "18:39:53").map(Timestamp::as_unix_micros),
            Some(REFERENCE)
        );
        assert_eq!(
            parse_date_time_utc("2026-10-07", "18:39:53.25").map(Timestamp::as_unix_micros),
            Some(REFERENCE + 250_000)
        );
        assert!(parse_date_time_utc("2026-10-07", "18:39").is_none());
        assert_eq!(
            parse_haproxy("07/Oct/2026:18:39:53.655").map(Timestamp::as_unix_micros),
            Some(REFERENCE + 655_000)
        );
        assert_eq!(
            parse_haproxy("07/Oct/2026:18:39:53").map(Timestamp::as_unix_micros),
            Some(REFERENCE)
        );
    }

    #[test]
    fn flexible_timestamps() {
        let ts = |s| parse_flexible(s).map(Timestamp::as_unix_micros);
        assert_eq!(ts("2026-10-07T18:39:53Z"), Some(REFERENCE));
        assert_eq!(ts("2026-10-07 18:39:53"), Some(REFERENCE));
        assert_eq!(ts("2026-10-07T18:39:53.5"), Some(REFERENCE + 500_000));
        assert_eq!(ts("07/Oct/2026:18:39:53 +0000"), Some(REFERENCE));
        assert_eq!(ts("1791398393"), Some(REFERENCE));
        assert_eq!(ts("1791398393.5"), Some(REFERENCE + 500_000));
        assert_eq!(ts("1791398393000"), Some(REFERENCE));
        assert_eq!(ts("1791398393000000"), Some(REFERENCE));
        assert_eq!(ts("1791398393000000000"), Some(REFERENCE));
        assert_eq!(ts("42"), None);
        assert_eq!(ts("yesterday"), None);
    }

    #[test]
    fn epoch_seconds() {
        assert_eq!(
            parse_epoch_seconds("1791398393.123").map(Timestamp::as_unix_micros),
            Some(REFERENCE + 123_000)
        );
    }
}
