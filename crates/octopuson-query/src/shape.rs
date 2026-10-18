//! Result shape of a query: individual hits or aggregated groups.

use octopuson_core::table::{Column, ColumnKind};

use crate::ast::{Query, SortDirection, SortKey};
use crate::measure::{Dimension, Metric};

/// Columns of a hit listing, in output order.
pub const ROW_COLUMNS: [(&str, ColumnKind); 12] = [
    ("ts", ColumnKind::Timestamp),
    ("ip", ColumnKind::Text),
    ("method", ColumnKind::Text),
    ("url", ColumnKind::Text),
    ("status", ColumnKind::Integer),
    ("bytes", ColumnKind::Integer),
    ("response_ms", ColumnKind::Float),
    ("ua", ColumnKind::Text),
    ("bot", ColumnKind::Text),
    ("verification", ColumnKind::Text),
    ("referer", ColumnKind::Text),
    ("host", ColumnKind::Text),
];

/// What a query returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shape {
    /// One row per hit.
    Rows,
    /// One row per group with aggregates.
    Aggregate {
        /// Grouping keys (may be empty for a single total row).
        group: Vec<Dimension>,
        /// Aggregates (never empty).
        metrics: Vec<Metric>,
    },
}

impl Shape {
    /// Derives the shape; grouping without metrics implies `hits`.
    pub fn of(query: &Query) -> Self {
        if query.group.is_empty() && query.metrics.is_empty() {
            return Self::Rows;
        }
        let metrics = if query.metrics.is_empty() {
            vec![Metric::Hits]
        } else {
            query.metrics.clone()
        };
        Self::Aggregate {
            group: query.group.clone(),
            metrics,
        }
    }

    /// Result columns in order.
    pub fn columns(&self) -> Vec<Column> {
        match self {
            Self::Rows => ROW_COLUMNS
                .iter()
                .map(|&(name, kind)| Column::new(name, kind))
                .collect(),
            Self::Aggregate { group, metrics } => group
                .iter()
                .map(|d| Column::new(d.name(), d.column_kind()))
                .chain(
                    metrics
                        .iter()
                        .map(|m| Column::new(m.name(), m.column_kind())),
                )
                .collect(),
        }
    }

    /// Ordering used when the query has none: newest hits, or the first metric descending.
    pub fn default_sort(&self) -> SortKey {
        match self {
            Self::Rows => SortKey {
                column: "ts".to_owned(),
                direction: SortDirection::Desc,
            },
            Self::Aggregate { metrics, .. } => SortKey {
                column: metrics.first().map_or("hits", |m| m.name()).to_owned(),
                direction: SortDirection::Desc,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grouping_implies_hits() {
        let query = Query {
            group: vec![Dimension::Url],
            ..Query::default()
        };
        let names: Vec<String> = Shape::of(&query)
            .columns()
            .into_iter()
            .map(|c| c.name)
            .collect();
        assert_eq!(names, ["url", "hits"]);
    }

    #[test]
    fn empty_query_lists_rows() {
        assert_eq!(Shape::of(&Query::default()), Shape::Rows);
        assert_eq!(Shape::Rows.default_sort().column, "ts");
    }
}
