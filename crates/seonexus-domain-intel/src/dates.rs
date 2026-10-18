//! Calendar dates as registries write them.

/// A calendar date, `YYYY-MM-DD` when displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Day {
    /// Year.
    pub year: i32,
    /// Month, 1–12.
    pub month: u8,
    /// Day, 1–31.
    pub day: u8,
}

impl std::fmt::Display for Day {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

impl Day {
    /// A checked date (years 1980–2100: the life span of the domain name system).
    pub fn new(year: i32, month: u8, day: u8) -> Option<Self> {
        let valid = (1980..=2100).contains(&year)
            && (1..=12).contains(&month)
            && (1..=days_in(year, month)).contains(&day);
        valid.then_some(Self { year, month, day })
    }

    /// Days since 1970-01-01.
    pub fn days(self) -> i64 {
        // Howard Hinnant's days_from_civil.
        let year = i64::from(self.year) - i64::from(self.month <= 2);
        let era = year.div_euclid(400);
        let year_of_era = year - era * 400;
        let month = i64::from(self.month);
        let day_of_year =
            (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + i64::from(self.day) - 1;
        let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
        era * 146_097 + day_of_era - 719_468
    }

    /// Years from `self` to `today`, with a fraction.
    #[allow(clippy::cast_precision_loss)] // day counts
    pub fn years_until(self, today: Self) -> f64 {
        ((today.days() - self.days()) as f64 / 365.2425).max(0.0)
    }

    /// Reads `YYYY-MM-DD` (and longer ISO timestamps).
    pub fn parse_iso(text: &str) -> Option<Self> {
        let text = text.trim();
        let year = text.get(..4)?.parse().ok()?;
        let month = text.get(5..7)?.parse().ok()?;
        let day = text.get(8..10)?.parse().ok()?;
        Self::new(year, month, day)
    }
}

fn days_in(year: i32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        _ => 28,
    }
}

fn month_of(name: &str) -> Option<u8> {
    let name = name.to_ascii_lowercase();
    let months = [
        ["jan", "oca"],
        ["feb", "şub"],
        ["mar", "mar"],
        ["apr", "nis"],
        ["may", "may"],
        ["jun", "haz"],
        ["jul", "tem"],
        ["aug", "ağu"],
        ["sep", "eyl"],
        ["oct", "eki"],
        ["nov", "kas"],
        ["dec", "ara"],
    ];
    let prefix: String = name.chars().take(3).collect();
    months
        .iter()
        .position(|names| names.contains(&prefix.as_str()))
        .and_then(|index| u8::try_from(index + 1).ok())
}

/// The first date in a WHOIS value, in any of the layouts registries use: `2001-03-15`,
/// `2001-Mar-15`, `15-Mar-2001`, `15.03.2001`, `2001.03.15`, `2001/03/15`, `15/03/2001`,
/// `Mar 15 2001`, `20010315`, ISO timestamps.
pub fn parse_date(text: &str) -> Option<Day> {
    // Split on punctuation and between digits and letters (`15T04` → `15`, `T`, `04`).
    let mut tokens: Vec<String> = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        let boundary = current
            .chars()
            .last()
            .is_some_and(|last| last.is_ascii_digit() != ch.is_ascii_digit());
        if !ch.is_alphanumeric() || boundary {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            if !ch.is_alphanumeric() {
                continue;
            }
        }
        current.push(ch);
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    let tokens: Vec<&str> = tokens.iter().map(String::as_str).collect();
    (0..tokens.len()).find_map(|start| from_tokens(&tokens[start..]))
}

fn from_tokens(tokens: &[&str]) -> Option<Day> {
    let number = |token: &str| token.parse::<i32>().ok();
    let first = tokens.first()?;
    if first.len() == 8 && first.chars().all(|ch| ch.is_ascii_digit()) {
        return Day::new(
            first.get(..4)?.parse().ok()?,
            first.get(4..6)?.parse().ok()?,
            first.get(6..8)?.parse().ok()?,
        );
    }
    let [a, b, c, ..] = tokens else {
        return None;
    };
    let small = |value: i32| u8::try_from(value).ok();
    match (number(a), number(b), number(c)) {
        // 2001 03 15
        (Some(year), Some(month), Some(day)) if a.len() == 4 => {
            Day::new(year, small(month)?, small(day)?)
        }
        // 15 03 2001 (day first, as registries outside the US write it), else 03 15 2001
        (Some(day), Some(month), Some(year)) if c.len() == 4 => {
            Day::new(year, small(month)?, small(day)?)
                .or_else(|| Day::new(year, small(day)?, small(month)?))
        }
        // 2001 Mar 15
        (Some(year), None, Some(day)) if a.len() == 4 => Day::new(year, month_of(b)?, small(day)?),
        // 15 Mar 2001
        (Some(day), None, Some(year)) if c.len() == 4 => Day::new(year, month_of(b)?, small(day)?),
        // Mar 15 2001
        (None, Some(day), Some(year)) if c.len() == 4 => Day::new(year, month_of(a)?, small(day)?),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_registry_date_layouts() {
        let expected = Day::new(2001, 3, 15).unwrap();
        for text in [
            "2001-03-15T04:00:00Z",
            "2001-Mar-15.",
            "15-mar-2001",
            "15.03.2001",
            "2001.03.15",
            "2001/03/15",
            "15/03/2001",
            "Mar 15 2001",
            "Thu Mar 15 2001",
            "20010315",
            "03/15/2001",
            "Created on..............: 2001-Mar-15.",
        ] {
            assert_eq!(parse_date(text), Some(expected), "{text}");
        }
        assert_eq!(parse_date("no date"), None);
        assert_eq!(parse_date("2001-02-30"), None);
        assert_eq!(
            Day::parse_iso("1997-09-15T04:00:00Z")
                .map(|day| day.to_string())
                .as_deref(),
            Some("1997-09-15")
        );
    }

    #[test]
    fn computes_ages() {
        let created = Day::new(2016, 10, 18).unwrap();
        let today = Day::new(2026, 10, 18).unwrap();
        assert!((created.years_until(today) - 10.0).abs() < 0.01);
        assert_eq!(Day::new(1970, 1, 1), None);
        assert_eq!(
            Day::new(2000, 3, 1).unwrap().days() - Day::new(2000, 2, 28).unwrap().days(),
            2
        );
    }
}
