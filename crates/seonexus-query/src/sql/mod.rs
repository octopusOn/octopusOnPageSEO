//! Compilation of a [`Query`] into parameterized DuckDB SQL.
//!
//! The generated SQL reads these relations, which the storage adapter must provide:
//!
//! | Relation | Columns used |
//! |---|---|
//! | `hits` (only when [`Source::uses_hits`]) | Parquet hit columns plus `month` (`VARCHAR`, `YYYY-MM`) |
//! | `urls` | `url_id`, `raw_url` |
//! | `user_agents` | `ua_id`, `raw`, `bot`, `bot_category` |
//! | `bot_ips` | `ip`, `bot`, `verdict` (one row per pair) |
//! | `r_day_url` | `day, url_id, ua_id, status, hits, bytes, duration_sum_us, duration_count, duration_max_us, first_ts, last_ts, verification` |
//! | `r_hour` | `hour, status, hits, bytes` |
//! | `site_meta` | `key`, `value` (`site_host` for internal referers) |
//! | `ip_geo` | `ip`, `country`, `asn`, `as_org` (one row per address) |
//!
//! Identifiers come only from constant tables and every user-supplied value is a bound
//! parameter, so query text cannot inject SQL.

mod columns;
mod filter;
mod source;

pub use source::Source;

use std::fmt::Write as _;

use seonexus_core::table::Column;
use seonexus_core::{Date, Timestamp};

use crate::ast::{Query, SortDirection};
use crate::shape::Shape;
use crate::validate::{ValidationError, validate};
use columns::{Joins, SqlExpr, cast_to, dimension_column, metric_column, row_column};
use filter::{SqlWriter, write_expr};

/// A bound parameter value.
#[derive(Debug, Clone, PartialEq)]
pub enum SqlValue {
    /// Text.
    Text(String),
    /// Integer.
    Integer(i64),
    /// Boolean.
    Boolean(bool),
    /// Date.
    Date(Date),
    /// Timestamp (UTC, microseconds).
    Timestamp(Timestamp),
}

/// Environment of a compilation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompileContext {
    /// "Today" for relative dates such as `last7d` (UTC).
    pub today: Date,
}

/// Ready-to-run SQL.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledQuery {
    /// SQL text with positional `?` parameters, without `LIMIT` (see [`Self::sql_for`]).
    pub sql: String,
    /// Parameter values in order.
    pub params: Vec<SqlValue>,
    /// Result columns; every row has exactly these, cast to the declared kinds.
    pub columns: Vec<Column>,
    /// Data source chosen by the planner.
    pub source: Source,
    /// The query's own row limit.
    pub limit: Option<u64>,
}

/// A window of result rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Page {
    /// Rows to skip.
    pub offset: u64,
    /// Maximum rows to return.
    pub size: u64,
}

impl CompiledQuery {
    /// Full SQL for `page` (or all rows), honouring the query's own limit.
    pub fn sql_for(&self, page: Option<Page>) -> String {
        let (limit, offset) = match (self.limit, page) {
            (limit, None) => (limit, 0),
            (None, Some(page)) => (Some(page.size), page.offset),
            (Some(limit), Some(page)) => (
                Some(page.size.min(limit.saturating_sub(page.offset))),
                page.offset,
            ),
        };
        let mut sql = self.sql.clone();
        if let Some(limit) = limit {
            let _ = write!(sql, " LIMIT {limit}");
        }
        if offset > 0 {
            let _ = write!(sql, " OFFSET {offset}");
        }
        sql
    }
}

/// Validates, plans and compiles `query`.
///
/// # Errors
/// [`ValidationError`] when the query is not meaningful.
pub fn compile(query: &Query, ctx: CompileContext) -> Result<CompiledQuery, ValidationError> {
    validate(query)?;
    let shape = Shape::of(query);
    let source = source::plan(query, &shape);
    let columns = shape.columns();
    let selected = selected_exprs(&shape, source);

    let mut filter = SqlWriter::default();
    if let Some(expr) = &query.filter {
        write_expr(&mut filter, expr, source, ctx);
    }
    let joins = selected
        .iter()
        .fold(filter.joins, |joins, expr| joins.union(expr.joins));

    let mut sql = String::from("SELECT ");
    for (index, (expr, column)) in selected.iter().zip(&columns).enumerate() {
        if index > 0 {
            sql.push_str(", ");
        }
        let _ = write!(
            sql,
            "{} AS \"{}\"",
            cast_to(column.kind, expr.sql),
            column.name
        );
    }
    let _ = write!(sql, " FROM {} t", source.table());
    write_joins(&mut sql, joins);
    if query.filter.is_some() {
        sql.push_str(" WHERE ");
        sql.push_str(&filter.sql);
    }
    if let Shape::Aggregate { group, .. } = &shape
        && !group.is_empty()
    {
        let positions: Vec<String> = (1..=group.len()).map(|p| p.to_string()).collect();
        let _ = write!(sql, " GROUP BY {}", positions.join(", "));
    }
    write_order(&mut sql, query, &shape, &columns);
    Ok(CompiledQuery {
        sql,
        params: filter.params,
        columns,
        source,
        limit: query.limit,
    })
}

fn selected_exprs(shape: &Shape, source: Source) -> Vec<SqlExpr> {
    match shape {
        Shape::Rows => crate::shape::ROW_COLUMNS
            .iter()
            .filter_map(|(name, _)| row_column(name))
            .collect(),
        Shape::Aggregate { group, metrics } => group
            .iter()
            .filter_map(|&d| dimension_column(d, source))
            .chain(metrics.iter().filter_map(|&m| metric_column(m, source)))
            .collect(),
    }
}

fn write_joins(sql: &mut String, joins: Joins) {
    if joins.urls {
        sql.push_str(" JOIN urls u ON u.url_id = t.url_id");
    }
    if joins.user_agents || joins.bot_ips {
        sql.push_str(" LEFT JOIN user_agents a ON a.ua_id = t.ua_id");
    }
    if joins.bot_ips {
        sql.push_str(" LEFT JOIN bot_ips v ON v.ip = t.ip AND v.bot = a.bot");
    }
    if joins.geo {
        sql.push_str(" LEFT JOIN ip_geo g ON g.ip = t.ip");
    }
}

/// Orders by result position (never by name, which could resolve to a base column), then by
/// the remaining grouping keys so pagination is deterministic.
fn write_order(sql: &mut String, query: &Query, shape: &Shape, columns: &[Column]) {
    let position = |name: &str| columns.iter().position(|c| c.name == name).map(|p| p + 1);
    let mut keys: Vec<(usize, SortDirection)> = if query.sort.is_empty() {
        let key = shape.default_sort();
        position(&key.column)
            .map(|p| vec![(p, key.direction)])
            .unwrap_or_default()
    } else {
        query
            .sort
            .iter()
            .filter_map(|key| position(&key.column).map(|p| (p, key.direction)))
            .collect()
    };
    if let Shape::Aggregate { group, .. } = shape {
        for p in 1..=group.len() {
            if !keys.iter().any(|&(k, _)| k == p) {
                keys.push((p, SortDirection::Asc));
            }
        }
    }
    let rendered: Vec<String> = keys
        .into_iter()
        .map(|(p, direction)| match direction {
            SortDirection::Asc => format!("{p} ASC NULLS LAST"),
            SortDirection::Desc => format!("{p} DESC NULLS LAST"),
        })
        .collect();
    if !rendered.is_empty() {
        let _ = write!(sql, " ORDER BY {}", rendered.join(", "));
    }
}

#[cfg(test)]
mod tests;
