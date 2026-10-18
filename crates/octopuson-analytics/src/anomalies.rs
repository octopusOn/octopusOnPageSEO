//! Unusual days in daily series: crawl drops and spikes, and server-error bursts.
//!
//! Each day is compared with the median of the previous [`WINDOW`] days. A day is unusual
//! when it differs both by a large factor and by many median absolute deviations, so normal
//! weekly rhythms and noise on small sites stay quiet. Days without hits count as zero (a bot
//! that stops crawling is the most important drop).

use octopuson_core::Date;
use serde::Serialize;

/// Days of history a day is compared with.
pub const WINDOW: usize = 14;
/// Days of history needed before judging a day.
const MIN_HISTORY: usize = 7;
/// Baselines below this are too small to judge.
const MIN_BASELINE: f64 = 20.0;
/// Robust z-score beyond which a day counts as unusual.
const MIN_Z: f64 = 3.5;
/// Factor by which a spike exceeds / a drop falls below the baseline.
const SPIKE_FACTOR: f64 = 2.0;
const DROP_FACTOR: f64 = 0.4;
/// Server-error share that is always worth reporting, and the minimum error count.
const ERROR_SHARE: f64 = 0.05;
const MIN_ERRORS: u64 = 20;

/// What is unusual.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AnomalyKind {
    /// Far more hits than usual.
    Spike,
    /// Far fewer hits than usual (or none).
    Drop,
    /// An unusual share of 5xx responses.
    ServerErrors,
}

/// One unusual day.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Anomaly {
    /// Series name given by the caller, e.g. `googlebot`.
    pub series: String,
    /// The day.
    pub day: Date,
    /// What is unusual.
    pub kind: AnomalyKind,
    /// The day's value (hits, or 5xx share for [`AnomalyKind::ServerErrors`]).
    pub value: f64,
    /// The usual value (median of the previous days).
    pub baseline: f64,
}

/// Hits and 5xx responses of one day.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DayCount {
    /// The day.
    pub day: Date,
    /// Hits.
    pub hits: u64,
    /// Hits answered with 5xx.
    pub server_errors: u64,
}

/// Unusual days of one series (days missing from `days` count as zero hits).
pub fn detect(series: &str, days: &[DayCount]) -> Vec<Anomaly> {
    let filled = fill_gaps(days);
    let mut found = Vec::new();
    for (index, today) in filled.iter().enumerate() {
        let history = &filled[index.saturating_sub(WINDOW)..index];
        if history.len() < MIN_HISTORY {
            continue;
        }
        #[allow(
            clippy::cast_precision_loss,
            reason = "daily hit counts are far below 2^52"
        )]
        let hits: Vec<f64> = history.iter().map(|d| d.hits as f64).collect();
        let baseline = median(&hits);
        if baseline >= MIN_BASELINE {
            let deviations: Vec<f64> = hits.iter().map(|h| (h - baseline).abs()).collect();
            let spread = 1.4826 * median(&deviations) + 1.0;
            #[allow(
                clippy::cast_precision_loss,
                reason = "daily hit counts are far below 2^52"
            )]
            let value = today.hits as f64;
            let z = (value - baseline) / spread;
            let kind = if value >= baseline * SPIKE_FACTOR && z >= MIN_Z {
                Some(AnomalyKind::Spike)
            } else if value <= baseline * DROP_FACTOR && z <= -MIN_Z {
                Some(AnomalyKind::Drop)
            } else {
                None
            };
            if let Some(kind) = kind {
                found.push(Anomaly {
                    series: series.to_owned(),
                    day: today.day,
                    kind,
                    value,
                    baseline,
                });
            }
        }
        if let Some(anomaly) = error_burst(series, today, history) {
            found.push(anomaly);
        }
    }
    found
}

fn error_burst(series: &str, today: &DayCount, history: &[DayCount]) -> Option<Anomaly> {
    if today.server_errors < MIN_ERRORS || today.hits == 0 {
        return None;
    }
    #[allow(
        clippy::cast_precision_loss,
        reason = "daily hit counts are far below 2^52"
    )]
    let share = |d: &DayCount| {
        if d.hits == 0 {
            0.0
        } else {
            d.server_errors as f64 / d.hits as f64
        }
    };
    let shares: Vec<f64> = history.iter().map(share).collect();
    let baseline = median(&shares);
    let value = share(today);
    (value >= ERROR_SHARE.max(baseline * 3.0)).then(|| Anomaly {
        series: series.to_owned(),
        day: today.day,
        kind: AnomalyKind::ServerErrors,
        value,
        baseline,
    })
}

/// Consecutive days from the first to the last given day; missing days are zero.
fn fill_gaps(days: &[DayCount]) -> Vec<DayCount> {
    let mut sorted = days.to_vec();
    sorted.sort_by_key(|d| d.day);
    let (Some(first), Some(last)) = (sorted.first(), sorted.last()) else {
        return Vec::new();
    };
    let span = last.day.days_since_epoch() - first.day.days_since_epoch();
    let mut filled = Vec::with_capacity(usize::try_from(span + 1).unwrap_or(0));
    let mut next = sorted.iter().peekable();
    for offset in 0..=span {
        let day = first.day.add_days(offset);
        match next.peek() {
            Some(found) if found.day == day => {
                filled.push(**found);
                next.next();
            }
            _ => filled.push(DayCount {
                day,
                hits: 0,
                server_errors: 0,
            }),
        }
    }
    filled
}

fn median(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let middle = sorted.len() / 2;
    if sorted.len().is_multiple_of(2) {
        f64::midpoint(sorted[middle - 1], sorted[middle])
    } else {
        sorted[middle]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn days(values: &[(u64, u64)]) -> Vec<DayCount> {
        let start = Date::parse("2026-09-01").unwrap();
        values
            .iter()
            .enumerate()
            .map(|(offset, &(hits, server_errors))| DayCount {
                day: start.add_days(i32::try_from(offset).unwrap()),
                hits,
                server_errors,
            })
            .collect()
    }

    #[test]
    fn steady_series_with_weekly_rhythm_is_quiet() {
        let series: Vec<(u64, u64)> = (0..40)
            .map(|i| (if i % 7 < 5 { 1_000 } else { 700 }, 1))
            .collect();
        assert_eq!(detect("search", &days(&series)), []);
    }

    #[test]
    fn drops_spikes_and_missing_days_are_found() {
        let mut series: Vec<(u64, u64)> = (0..20).map(|i| (1_000 + (i % 3) * 20, 2)).collect();
        series.push((150, 1)); // drop
        series.push((4_000, 3)); // spike
        let found = detect("googlebot", &days(&series));
        let kinds: Vec<AnomalyKind> = found.iter().map(|a| a.kind).collect();
        assert_eq!(kinds, [AnomalyKind::Drop, AnomalyKind::Spike]);
        assert_eq!(found[0].day.to_string(), "2026-09-21");
        assert!((found[0].baseline - 1_020.0).abs() < 1e-9);

        // A day with no hits at all is a drop too.
        let mut gap = days(&(0..14).map(|_| (500, 0)).collect::<Vec<_>>());
        let last = gap.last().unwrap().day;
        gap.push(DayCount {
            day: last.add_days(2),
            hits: 500,
            server_errors: 0,
        });
        let found = detect("ai", &gap);
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].kind, found[0].value), (AnomalyKind::Drop, 0.0));
    }

    #[test]
    fn server_error_bursts_are_found() {
        let mut series: Vec<(u64, u64)> = (0..10).map(|_| (1_000, 3)).collect();
        series.push((1_000, 90));
        series.push((1_000, 10)); // too few errors to matter
        let found = detect("all", &days(&series));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].kind, AnomalyKind::ServerErrors);
        assert!((found[0].value - 0.09).abs() < 1e-9);
    }

    #[test]
    fn small_or_short_series_are_not_judged() {
        assert_eq!(detect("x", &days(&[(5, 0); 30])), []);
        assert_eq!(detect("x", &days(&[(1_000, 0), (1, 0)])), []);
        assert_eq!(detect("x", &[]), []);
    }
}
