//! Conversions between DuckDB values and the shared table/parameter types.

use duckdb::types::{TimeUnit, Value, ValueRef};
use octopuson_core::table::Cell;
use octopuson_core::{Date, Timestamp};
use octopuson_query::sql::SqlValue;

/// Bound parameter for a compiled query value.
pub(super) fn to_value(value: &SqlValue) -> Value {
    match value {
        SqlValue::Text(text) => Value::Text(text.clone()),
        SqlValue::Integer(number) => Value::BigInt(*number),
        SqlValue::Boolean(flag) => Value::Boolean(*flag),
        SqlValue::Date(date) => Value::Date32(date.days_since_epoch()),
        SqlValue::Timestamp(ts) => Value::Timestamp(TimeUnit::Microsecond, ts.as_unix_micros()),
    }
}

/// Result cell. The compiler casts every column to its declared kind, so only a few
/// physical types occur; anything else is rendered as text rather than dropped.
pub(super) fn cell(value: ValueRef<'_>) -> Cell {
    match value {
        ValueRef::Null => Cell::Null,
        ValueRef::Boolean(flag) => Cell::Boolean(flag),
        ValueRef::TinyInt(n) => Cell::Integer(n.into()),
        ValueRef::SmallInt(n) => Cell::Integer(n.into()),
        ValueRef::Int(n) => Cell::Integer(n.into()),
        ValueRef::BigInt(n) => Cell::Integer(n),
        ValueRef::UTinyInt(n) => Cell::Integer(n.into()),
        ValueRef::USmallInt(n) => Cell::Integer(n.into()),
        ValueRef::UInt(n) => Cell::Integer(n.into()),
        ValueRef::UBigInt(n) => {
            i64::try_from(n).map_or_else(|_| Cell::Text(n.to_string()), Cell::Integer)
        }
        ValueRef::HugeInt(n) => {
            i64::try_from(n).map_or_else(|_| Cell::Text(n.to_string()), Cell::Integer)
        }
        ValueRef::Float(f) => Cell::Float(f.into()),
        ValueRef::Double(f) => Cell::Float(f),
        ValueRef::Text(bytes) => Cell::Text(String::from_utf8_lossy(bytes).into_owned()),
        ValueRef::Date32(days) => Date::from_days_since_epoch(days).map_or(Cell::Null, Cell::Date),
        ValueRef::Timestamp(unit, raw) => {
            Cell::Timestamp(Timestamp::from_unix_micros(to_micros(unit, raw)))
        }
        other => Cell::Text(format!("{other:?}")),
    }
}

fn to_micros(unit: TimeUnit, raw: i64) -> i64 {
    match unit {
        TimeUnit::Second => raw.saturating_mul(1_000_000),
        TimeUnit::Millisecond => raw.saturating_mul(1_000),
        TimeUnit::Microsecond => raw,
        TimeUnit::Nanosecond => raw / 1_000,
    }
}
