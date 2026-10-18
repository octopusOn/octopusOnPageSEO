//! Filter expressions → parameterized SQL.

use octopuson_core::Date;

use super::columns::{Joins, field_column, is_nullable};
use super::source::Source;
use super::{CompileContext, SqlValue};
use crate::ast::{CompareOp, Condition, Expr, Literal, Predicate};
use crate::field::Field;

/// SQL text with positional `?` parameters in order of appearance.
#[derive(Debug, Default)]
pub(super) struct SqlWriter {
    pub(super) sql: String,
    pub(super) params: Vec<SqlValue>,
    pub(super) joins: Joins,
}

impl SqlWriter {
    fn push(&mut self, sql: &str) {
        self.sql.push_str(sql);
    }

    fn param(&mut self, value: SqlValue) {
        self.sql.push('?');
        self.params.push(value);
    }
}

pub(super) fn write_expr(out: &mut SqlWriter, expr: &Expr, source: Source, ctx: CompileContext) {
    match expr {
        Expr::And(operands) => write_joined(out, operands, " AND ", source, ctx),
        Expr::Or(operands) => write_joined(out, operands, " OR ", source, ctx),
        Expr::Not(inner) => {
            out.push("NOT ");
            write_expr(out, inner, source, ctx);
        }
        Expr::Condition(condition) => write_condition(out, condition, source, ctx),
    }
}

fn write_joined(
    out: &mut SqlWriter,
    operands: &[Expr],
    separator: &str,
    source: Source,
    ctx: CompileContext,
) {
    out.push("(");
    for (index, operand) in operands.iter().enumerate() {
        if index > 0 {
            out.push(separator);
        }
        write_expr(out, operand, source, ctx);
    }
    out.push(")");
}

fn write_condition(
    out: &mut SqlWriter,
    condition: &Condition,
    source: Source,
    ctx: CompileContext,
) {
    if condition.field == Field::Date && source == Source::Hits {
        write_hit_date(out, &condition.predicate, ctx);
        return;
    }
    let Some(column) = field_column(condition.field, source) else {
        // The planner only picks sources that provide every filtered field.
        out.push("FALSE");
        return;
    };
    out.joins = out.joins.union(column.joins);
    let nullable = is_nullable(condition.field);
    out.push(if nullable { "coalesce((" } else { "(" });
    write_predicate(out, column.sql, &condition.predicate, ctx);
    out.push(if nullable { "), FALSE)" } else { ")" });
}

fn write_predicate(out: &mut SqlWriter, column: &str, predicate: &Predicate, ctx: CompileContext) {
    if let Predicate::Regex(pattern) = predicate {
        out.push("regexp_matches(");
        out.push(column);
        out.push(", ");
        out.param(SqlValue::Text(pattern.clone()));
        out.push(")");
        return;
    }
    out.push(column);
    match predicate {
        Predicate::OneOf(values) => {
            if let [value] = values.as_slice() {
                out.push(" = ");
                out.param(literal(value));
            } else {
                out.push(" IN (");
                for (index, value) in values.iter().enumerate() {
                    if index > 0 {
                        out.push(", ");
                    }
                    out.param(literal(value));
                }
                out.push(")");
            }
        }
        Predicate::Glob(pattern) => {
            out.push(" LIKE ");
            out.param(SqlValue::Text(glob_to_like(pattern)));
            out.push(" ESCAPE '\\'");
        }
        Predicate::Regex(_) => {}
        Predicate::Compare(op, value) => {
            out.push(" ");
            out.push(op.symbol());
            out.push(" ");
            out.param(literal(value));
        }
        Predicate::Between(low, high) => write_between(out, literal(low), literal(high)),
        Predicate::StatusClass(class) => {
            let base = i64::from(*class) * 100;
            write_between(out, SqlValue::Integer(base), SqlValue::Integer(base + 99));
        }
        Predicate::LastDays(days) => {
            let (first, last) = last_days(*days, ctx);
            write_between(out, SqlValue::Date(first), SqlValue::Date(last));
        }
    }
}

fn write_between(out: &mut SqlWriter, low: SqlValue, high: SqlValue) {
    out.push(" BETWEEN ");
    out.param(low);
    out.push(" AND ");
    out.param(high);
}

fn literal(value: &Literal) -> SqlValue {
    match value {
        Literal::Text(text) => SqlValue::Text(text.clone()),
        Literal::Integer(number) => SqlValue::Integer(*number),
        Literal::Date(date) => SqlValue::Date(*date),
        Literal::Boolean(flag) => SqlValue::Boolean(*flag),
    }
}

/// `*` → `%`; literal `%`, `_` and `\` are escaped (used with `ESCAPE '\'`).
fn glob_to_like(pattern: &str) -> String {
    let mut like = String::with_capacity(pattern.len() + 4);
    for c in pattern.chars() {
        match c {
            '*' => like.push('%'),
            '%' | '_' | '\\' => {
                like.push('\\');
                like.push(c);
            }
            c => like.push(c),
        }
    }
    like
}

fn last_days(days: u32, ctx: CompileContext) -> (Date, Date) {
    let back = i32::try_from(days.saturating_sub(1)).unwrap_or(i32::MAX);
    (ctx.today.add_days(-back), ctx.today)
}

/// An inclusive day range; `None` is unbounded.
type DayRange = (Option<Date>, Option<Date>);

/// Date tests on raw hits become timestamp ranges plus a `month` partition bound, so DuckDB
/// can skip whole Parquet files and row groups instead of casting every timestamp.
fn write_hit_date(out: &mut SqlWriter, predicate: &Predicate, ctx: CompileContext) {
    let ranges: Vec<DayRange> = match predicate {
        Predicate::OneOf(values) => values
            .iter()
            .filter_map(|v| match v {
                Literal::Date(d) => Some((Some(*d), Some(*d))),
                _ => None,
            })
            .collect(),
        Predicate::Between(Literal::Date(low), Literal::Date(high)) => {
            vec![(Some(*low), Some(*high))]
        }
        Predicate::Compare(op, Literal::Date(date)) => vec![match op {
            CompareOp::Gt => (Some(date.add_days(1)), None),
            CompareOp::Ge => (Some(*date), None),
            CompareOp::Lt => (None, Some(date.add_days(-1))),
            CompareOp::Le => (None, Some(*date)),
        }],
        Predicate::LastDays(days) => {
            let (first, last) = last_days(*days, ctx);
            vec![(Some(first), Some(last))]
        }
        _ => Vec::new(),
    };
    if ranges.is_empty() {
        out.push("FALSE");
        return;
    }
    out.push("(");
    for (index, range) in ranges.into_iter().enumerate() {
        if index > 0 {
            out.push(" OR ");
        }
        write_hit_day_range(out, range);
    }
    out.push(")");
}

fn write_hit_day_range(out: &mut SqlWriter, (first, last): DayRange) {
    out.push("(TRUE");
    if let Some(first) = first {
        out.push(" AND t.month >= ");
        out.param(SqlValue::Text(month_key(first)));
        out.push(" AND t.ts >= ");
        out.param(SqlValue::Timestamp(first.start()));
    }
    if let Some(last) = last {
        out.push(" AND t.month <= ");
        out.param(SqlValue::Text(month_key(last)));
        out.push(" AND t.ts < ");
        out.param(SqlValue::Timestamp(last.add_days(1).start()));
    }
    out.push(")");
}

/// Partition key of the month containing `date` (`YYYY-MM`).
fn month_key(date: Date) -> String {
    let (year, month, _) = date.ymd();
    format!("{year:04}-{month:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn globs_escape_like_metacharacters() {
        assert_eq!(glob_to_like("/a_b%c\\*"), "/a\\_b\\%c\\\\%");
    }
}
