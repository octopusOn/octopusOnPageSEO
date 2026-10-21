//! Rank tracking measures: how often keywords are checked, the click share of a position, a
//! keyword's trend over its checks, visibility by day and the shares of the domains ranking for
//! a project's keywords.
//!
//! The SERP Checker and the GEO-SEO Platform store their checks differently; both hand them
//! here as [`Check`]s and [`ResultPage`]s, so they measure the same way.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::hash::Hash;

use serde::{Deserialize, Serialize};

use crate::difficulty::CTR;

/// Other domains shown when a project lists no competitors.
const MOST_VISIBLE: usize = 5;

/// How often a project is checked.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Schedule {
    /// Every day.
    #[default]
    Daily,
    /// Every week.
    Weekly,
    /// Only when asked.
    Manual,
}

impl Schedule {
    /// Stored name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Daily => "daily",
            Self::Weekly => "weekly",
            Self::Manual => "manual",
        }
    }

    /// The schedule of a stored name; daily for anything else.
    pub fn parse(text: &str) -> Self {
        match text {
            "weekly" => Self::Weekly,
            "manual" => Self::Manual,
            _ => Self::Daily,
        }
    }

    /// Seconds after a check when the next one is due (a little early, so a daily check at a
    /// varying time does not drift by a day).
    pub const fn interval_secs(self) -> Option<i64> {
        match self {
            Self::Daily => Some(20 * 3_600),
            Self::Weekly => Some(6 * 86_400 + 12 * 3_600),
            Self::Manual => None,
        }
    }

    /// Checks in thirty days.
    pub const fn per_month(self) -> u32 {
        match self {
            Self::Daily => 30,
            Self::Weekly => 4,
            Self::Manual => 0,
        }
    }
}

/// Click share of a position: the curve of the difficulty model, 1% on page two.
pub fn ctr(position: u16) -> f64 {
    match position {
        1..=10 => CTR[usize::from(position - 1)],
        11..=20 => 0.01,
        _ => 0.0,
    }
}

/// Percentage with one decimal; 0 without a whole.
pub fn share_of(part: f64, whole: f64) -> f64 {
    if whole > 0.0 {
        (part / whole * 1000.0).round() / 10.0
    } else {
        0.0
    }
}

/// One stored check of a keyword on one tracker, as the measures read it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Check<'a> {
    /// RFC 3339, UTC.
    pub checked_at: &'a str,
    /// Best position of the site; `None` when it does not rank.
    pub position: Option<u16>,
    /// Its page at that position.
    pub url: Option<&'a str>,
    /// How many of the site's pages rank.
    pub pages: usize,
    /// Why the check failed; a failed check counts only as the latest attempt.
    pub error: Option<&'a str>,
}

/// A keyword's positions over its checks.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trend {
    /// Latest position.
    pub position: Option<u16>,
    /// Position of the check before.
    pub previous: Option<u16>,
    /// Places gained since then (positive is better); `None` without two positions.
    pub change: Option<i32>,
    /// Best position ever.
    pub best: Option<u16>,
    /// URL that ranks.
    pub url: Option<String>,
    /// More than one page of the site ranks (cannibalisation).
    pub cannibalised: bool,
    /// When it was last checked.
    pub checked_at: Option<String>,
    /// Daily positions (last check of each day), oldest first.
    pub history: Vec<(String, Option<u16>)>,
    /// Why the last check failed.
    pub error: Option<String>,
}

/// The trend of one keyword on one tracker from its checks, oldest first.
pub fn trend(checks: &[Check<'_>]) -> Trend {
    let successful: Vec<&Check<'_>> = checks
        .iter()
        .filter(|check| check.error.is_none())
        .collect();
    let latest = checks.last();
    let current = successful.last();
    let previous = successful
        .len()
        .checked_sub(2)
        .and_then(|index| successful.get(index));
    let mut history: BTreeMap<String, Option<u16>> = BTreeMap::new();
    for check in &successful {
        history.insert(
            check.checked_at.get(..10).unwrap_or_default().to_owned(),
            check.position,
        );
    }
    let position = current.and_then(|check| check.position);
    let before = previous.and_then(|check| check.position);
    Trend {
        position,
        previous: before,
        change: position
            .zip(before)
            .map(|(now, before)| i32::from(before) - i32::from(now)),
        best: successful.iter().filter_map(|check| check.position).min(),
        url: current.and_then(|check| check.url.map(str::to_owned)),
        cannibalised: current.is_some_and(|check| check.pages > 1),
        checked_at: latest.map(|check| check.checked_at.to_owned()),
        history: history.into_iter().collect(),
        error: latest.and_then(|check| check.error.map(str::to_owned)),
    }
}

/// Visibility on one day.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisibilityPoint {
    /// `YYYY-MM-DD`.
    pub date: String,
    /// Share of the clicks the site would get at its positions, against first place
    /// everywhere, weighted by volume (0–100).
    pub visibility: f64,
    /// Keywords in the top 3.
    pub top3: u32,
    /// Keywords in the top 10.
    pub top10: u32,
    /// Keywords ranking at all.
    pub ranked: u32,
}

/// Visibility by day: the last successful check that day of each keyword and tracker (`K`),
/// weighted by `weight` (its volume, at least 1).
pub fn visibility<'a, K: Copy + Eq + Hash>(
    checks: impl IntoIterator<Item = (K, Check<'a>)>,
    weight: &dyn Fn(&K) -> f64,
) -> Vec<VisibilityPoint> {
    let mut days: BTreeMap<String, HashMap<K, Option<u16>>> = BTreeMap::new();
    for (key, check) in checks {
        if check.error.is_some() {
            continue;
        }
        days.entry(check.checked_at.get(..10).unwrap_or_default().to_owned())
            .or_default()
            .insert(key, check.position);
    }
    days.into_iter()
        .map(|(date, positions)| {
            let best: f64 = positions.keys().map(|key| ctr(1) * weight(key)).sum();
            let got: f64 = positions
                .iter()
                .map(|(key, position)| position.map_or(0.0, ctr) * weight(key))
                .sum();
            let count = |limit: u16| {
                u32::try_from(
                    positions
                        .values()
                        .filter(|position| position.is_some_and(|position| position <= limit))
                        .count(),
                )
                .unwrap_or(u32::MAX)
            };
            VisibilityPoint {
                date,
                visibility: share_of(got, best),
                top3: count(3),
                top10: count(10),
                ranked: count(u16::MAX),
            }
        })
        .collect()
}

/// A result page for the shares: the weight of its keyword (volume, at least 1) and the
/// domains on it by position. A domain counts once, at its first position.
#[derive(Debug, Clone, PartialEq)]
pub struct ResultPage {
    /// Weight of the keyword.
    pub weight: f64,
    /// Domains by position.
    pub domains: Vec<(u16, String)>,
}

/// A domain's share of the project's keywords.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompetitorShare {
    /// Domain.
    pub domain: String,
    /// Visibility over the pages (0–100).
    pub visibility: f64,
    /// Keywords it ranks for.
    pub keywords: u32,
    /// Its average position where it ranks.
    pub average_position: Option<f64>,
    /// Whether it is the project's own site.
    pub own: bool,
}

/// Clicks, keywords and positions of a domain.
type Share = (f64, u32, Vec<u16>);

/// Every domain's clicks over the pages, and the clicks of first place everywhere.
fn tally(pages: &[ResultPage]) -> (HashMap<String, Share>, f64) {
    let mut shares: HashMap<String, Share> = HashMap::new();
    let mut best_total = 0.0;
    for page in pages {
        best_total += ctr(1) * page.weight;
        let mut seen = HashSet::new();
        for (position, domain) in &page.domains {
            if !seen.insert(domain) {
                continue;
            }
            let entry = shares.entry(domain.clone()).or_default();
            entry.0 += ctr(*position) * page.weight;
            entry.1 += 1;
            entry.2.push(*position);
        }
    }
    (shares, best_total)
}

#[allow(clippy::cast_precision_loss)] // position counts
fn share_rows(
    shares: &HashMap<String, Share>,
    best_total: f64,
    domains: Vec<String>,
    own: &str,
) -> Vec<CompetitorShare> {
    domains
        .into_iter()
        .map(|domain| {
            let (clicks, keywords, positions) = shares.get(&domain).cloned().unwrap_or_default();
            CompetitorShare {
                visibility: share_of(clicks, best_total),
                keywords,
                average_position: (!positions.is_empty()).then(|| {
                    let sum: f64 = positions.iter().map(|position| f64::from(*position)).sum();
                    (sum / positions.len() as f64 * 10.0).round() / 10.0
                }),
                own: domain == own,
                domain,
            }
        })
        .collect()
}

/// Shares of the project's site (`own`, first) and its competitors over `pages`; without
/// `listed` competitors, the five most visible other domains.
pub fn competitor_shares(
    pages: &[ResultPage],
    own: &str,
    listed: &[String],
) -> Vec<CompetitorShare> {
    let (shares, best_total) = tally(pages);
    let mut domains: Vec<String> = listed.to_vec();
    if domains.is_empty() {
        let mut frequent: Vec<(&String, &Share)> =
            shares.iter().filter(|(domain, _)| *domain != own).collect();
        frequent.sort_by(|a, b| b.1.0.total_cmp(&a.1.0).then_with(|| a.0.cmp(b.0)));
        domains = frequent
            .into_iter()
            .take(MOST_VISIBLE)
            .map(|(domain, _)| domain.clone())
            .collect();
    }
    domains.insert(0, own.to_owned());
    share_rows(&shares, best_total, domains, own)
}

/// Shares of exactly `domains`, in that order, over `pages` (`own` marks the project's site).
pub fn shares_of(pages: &[ResultPage], own: &str, domains: &[String]) -> Vec<CompetitorShare> {
    let (shares, best_total) = tally(pages);
    share_rows(&shares, best_total, domains.to_vec(), own)
}

/// Days from 1970-01-01 to a civil date (proleptic Gregorian).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// Seconds since the Unix epoch of an RFC 3339 UTC timestamp (`2026-10-18T09:30:00Z`).
pub fn seconds_of(timestamp: &str) -> Option<i64> {
    let number = |range: std::ops::Range<usize>| -> Option<i64> {
        let text = timestamp.get(range)?;
        text.bytes()
            .all(|byte| byte.is_ascii_digit())
            .then(|| text.parse().ok())?
    };
    let separators = [(4, b'-'), (7, b'-'), (13, b':'), (16, b':')];
    if separators
        .iter()
        .any(|(index, byte)| timestamp.as_bytes().get(*index) != Some(byte))
    {
        return None;
    }
    let (year, month, day) = (number(0..4)?, number(5..7)?, number(8..10)?);
    let (hours, minutes, seconds) = (number(11..13)?, number(14..16)?, number(17..19)?);
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let month_days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return None,
    };
    if day < 1 || day > month_days || hours > 23 || minutes > 59 || seconds > 60 {
        return None;
    }
    Some(days_from_civil(year, month, day) * 86_400 + hours * 3_600 + minutes * 60 + seconds)
}
