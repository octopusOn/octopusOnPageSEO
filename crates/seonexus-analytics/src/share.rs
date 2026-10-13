use serde::Serialize;

/// A named part of a total.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Share {
    /// What is counted, e.g. `4xx` or `search`.
    pub name: String,
    /// Count.
    pub value: u64,
    /// `value / total` in `0.0..=1.0`; 0 for an empty total.
    pub ratio: f64,
}

impl Share {
    /// `value` as a share of `total`.
    pub fn of(name: impl Into<String>, value: u64, total: u64) -> Self {
        Self {
            name: name.into(),
            value,
            ratio: ratio(value, total),
        }
    }
}

/// `part / total`, 0 when `total` is 0.
#[allow(
    clippy::cast_precision_loss,
    reason = "ratios for display; counts stay exact up to 2^53"
)]
pub(crate) fn ratio(part: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        part as f64 / total as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shares_handle_empty_totals() {
        assert!((Share::of("a", 1, 4).ratio - 0.25).abs() < f64::EPSILON);
        assert!(Share::of("a", 0, 0).ratio.abs() < f64::EPSILON);
    }
}
