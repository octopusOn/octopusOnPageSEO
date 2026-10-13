//! Data sources a query can be answered from and the planner that picks the cheapest one.

use std::fmt;

use crate::ast::{Expr, Query};
use crate::shape::Shape;

use super::columns::{dimension_column, field_column, metric_column};

/// Where rows come from. Rollups answer most dashboards in time independent of the number of
/// stored hits; raw Parquet hits are used only when a query needs per-hit detail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Source {
    /// Hourly totals per status (`r_hour`).
    HourRollup,
    /// Daily totals per URL, user agent and status (`r_day_url`).
    DayUrlRollup,
    /// Raw hits (Parquet, provided by the executor as the `hits` relation).
    Hits,
}

impl Source {
    /// Whether the executor must provide the `hits` relation.
    pub const fn uses_hits(self) -> bool {
        matches!(self, Self::Hits)
    }

    /// Table aliased as `t` in the generated SQL.
    pub(super) const fn table(self) -> &'static str {
        match self {
            Self::HourRollup => "r_hour",
            Self::DayUrlRollup => "r_day_url",
            Self::Hits => "hits",
        }
    }

    /// Short label shown to users (e.g. "answered from: rollup").
    pub const fn label(self) -> &'static str {
        match self {
            Self::HourRollup => "hourly rollup",
            Self::DayUrlRollup => "daily URL rollup",
            Self::Hits => "raw hits",
        }
    }
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Picks the smallest source able to answer `query`.
pub(super) fn plan(query: &Query, shape: &Shape) -> Source {
    [Source::HourRollup, Source::DayUrlRollup]
        .into_iter()
        .find(|&source| supports(source, query, shape))
        .unwrap_or(Source::Hits)
}

fn supports(source: Source, query: &Query, shape: &Shape) -> bool {
    let Shape::Aggregate { group, metrics } = shape else {
        return source == Source::Hits;
    };
    query
        .filter
        .as_ref()
        .is_none_or(|filter| filter_supported(filter, source))
        && group.iter().all(|&d| dimension_column(d, source).is_some())
        && metrics.iter().all(|&m| metric_column(m, source).is_some())
}

fn filter_supported(expr: &Expr, source: Source) -> bool {
    match expr {
        Expr::And(operands) | Expr::Or(operands) => {
            operands.iter().all(|e| filter_supported(e, source))
        }
        Expr::Not(inner) => filter_supported(inner, source),
        Expr::Condition(condition) => field_column(condition.field, source).is_some(),
    }
}
