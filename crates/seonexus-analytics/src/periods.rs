//! Comparing two periods per key.

use std::collections::HashMap;
use std::hash::BuildHasher;

use seonexus_core::table::{Cell, Column, ColumnKind};
use serde::Serialize;

use crate::share::ratio;

/// How a key changed between the periods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Trend {
    /// Only in the later period.
    New,
    /// Only in the earlier period.
    Lost,
    /// More hits.
    Up,
    /// Fewer hits.
    Down,
    /// Same hits.
    Same,
}

impl Trend {
    /// Stable machine name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Lost => "lost",
            Self::Up => "up",
            Self::Down => "down",
            Self::Same => "same",
        }
    }
}

/// One compared key.
#[derive(Debug, Clone, PartialEq)]
pub struct PeriodRow {
    /// Grouping value (URL, directory, bot, …).
    pub key: String,
    /// Hits in the earlier period.
    pub before: u64,
    /// Hits in the later period.
    pub after: u64,
    /// Change.
    pub trend: Trend,
}

impl PeriodRow {
    /// `after - before`.
    pub fn change(&self) -> i64 {
        let signed = |v: u64| i64::try_from(v).unwrap_or(i64::MAX);
        signed(self.after) - signed(self.before)
    }

    /// Relative change, `None` when the key is new.
    pub fn change_ratio(&self) -> Option<f64> {
        (self.before > 0).then(|| ratio(self.after, self.before) - 1.0)
    }

    /// Result columns named after the grouping key.
    pub fn columns(key: &str) -> Vec<Column> {
        vec![
            Column::new(key, ColumnKind::Text),
            Column::new("hits_before", ColumnKind::Integer),
            Column::new("hits_after", ColumnKind::Integer),
            Column::new("change", ColumnKind::Integer),
            Column::new("change_pct", ColumnKind::Float),
            Column::new("trend", ColumnKind::Text),
        ]
    }

    /// Cells in column order.
    pub fn cells(&self) -> Vec<Cell> {
        let signed = |v: u64| Cell::Integer(i64::try_from(v).unwrap_or(i64::MAX));
        vec![
            Cell::Text(self.key.clone()),
            signed(self.before),
            signed(self.after),
            Cell::Integer(self.change()),
            self.change_ratio()
                .map_or(Cell::Null, |r| Cell::Float(r * 100.0)),
            Cell::Text(self.trend.name().to_owned()),
        ]
    }
}

/// Totals of a comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct PeriodSummary {
    /// Hits in the earlier period.
    pub before_hits: u64,
    /// Hits in the later period.
    pub after_hits: u64,
    /// Keys only in the later period.
    pub new: u64,
    /// Keys only in the earlier period.
    pub lost: u64,
    /// Keys with more hits.
    pub up: u64,
    /// Keys with fewer hits.
    pub down: u64,
}

/// Compares hits per key; rows are ordered by the largest absolute change.
pub fn compare<S: BuildHasher>(
    before: &HashMap<String, u64, S>,
    after: &HashMap<String, u64, S>,
) -> (Vec<PeriodRow>, PeriodSummary) {
    let mut summary = PeriodSummary {
        before_hits: before.values().sum(),
        after_hits: after.values().sum(),
        ..PeriodSummary::default()
    };
    let keys = before
        .keys()
        .chain(after.keys().filter(|k| !before.contains_key(*k)));
    let mut rows: Vec<PeriodRow> = keys
        .map(|key| {
            let (b, a) = (
                before.get(key).copied().unwrap_or(0),
                after.get(key).copied().unwrap_or(0),
            );
            let trend = match (b, a) {
                (0, _) => Trend::New,
                (_, 0) => Trend::Lost,
                (b, a) if a > b => Trend::Up,
                (b, a) if a < b => Trend::Down,
                _ => Trend::Same,
            };
            match trend {
                Trend::New => summary.new += 1,
                Trend::Lost => summary.lost += 1,
                Trend::Up => summary.up += 1,
                Trend::Down => summary.down += 1,
                Trend::Same => {}
            }
            PeriodRow {
                key: key.clone(),
                before: b,
                after: a,
                trend,
            }
        })
        .collect();
    rows.sort_by(|x, y| {
        y.change()
            .unsigned_abs()
            .cmp(&x.change().unsigned_abs())
            .then_with(|| x.key.cmp(&y.key))
    });
    (rows, summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, u64)]) -> HashMap<String, u64> {
        pairs.iter().map(|&(k, v)| (k.to_owned(), v)).collect()
    }

    #[test]
    fn compares_keys_across_periods() {
        let before = map(&[("/a", 10), ("/b", 5), ("/gone", 3), ("/same", 2)]);
        let after = map(&[("/a", 4), ("/b", 8), ("/new", 20), ("/same", 2)]);
        let (rows, summary) = compare(&before, &after);
        let trends: Vec<(&str, Trend)> = rows.iter().map(|r| (r.key.as_str(), r.trend)).collect();
        assert_eq!(
            trends,
            [
                ("/new", Trend::New),
                ("/a", Trend::Down),
                ("/b", Trend::Up),
                ("/gone", Trend::Lost),
                ("/same", Trend::Same),
            ]
        );
        assert_eq!(
            summary,
            PeriodSummary {
                before_hits: 20,
                after_hits: 34,
                new: 1,
                lost: 1,
                up: 1,
                down: 1,
            }
        );
        assert_eq!(rows[1].change(), -6);
        assert_eq!(rows[0].change_ratio(), None);
        assert_eq!(rows[1].cells()[4], Cell::Float(-60.0));
    }
}
