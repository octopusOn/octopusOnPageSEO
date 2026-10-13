//! Tabular results shared by the query engine, storage adapters and exporters.

use std::fmt;
use std::io;

use crate::{Date, Timestamp};

/// Value type of a result column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColumnKind {
    /// UTF-8 text.
    Text,
    /// Signed 64-bit integer.
    Integer,
    /// 64-bit float.
    Float,
    /// Boolean.
    Boolean,
    /// Calendar date.
    Date,
    /// UTC instant.
    Timestamp,
}

/// A named, typed result column.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Column {
    /// Stable machine name, e.g. `hits` or `url`.
    pub name: String,
    /// Value type of every non-null cell.
    pub kind: ColumnKind,
}

impl Column {
    /// Creates a column.
    pub fn new(name: impl Into<String>, kind: ColumnKind) -> Self {
        Self {
            name: name.into(),
            kind,
        }
    }
}

/// One result value.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(untagged)]
pub enum Cell {
    /// Missing value.
    Null,
    /// Text.
    Text(String),
    /// Integer.
    Integer(i64),
    /// Float.
    Float(f64),
    /// Boolean.
    Boolean(bool),
    /// Date (serialized as `YYYY-MM-DD`).
    Date(Date),
    /// Instant (serialized as RFC 3339).
    Timestamp(Timestamp),
}

/// Plain-text rendering used by CSV and terminal output; `Null` is empty.
impl fmt::Display for Cell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => Ok(()),
            Self::Text(text) => f.write_str(text),
            Self::Integer(value) => write!(f, "{value}"),
            // `+ 0.0` turns a rounded `-0` into `0`.
            Self::Float(value) => write!(f, "{}", (value * 100.0).round() / 100.0 + 0.0),
            Self::Boolean(value) => write!(f, "{value}"),
            Self::Date(date) => write!(f, "{date}"),
            Self::Timestamp(timestamp) => write!(f, "{timestamp}"),
        }
    }
}

/// Receives a result table row by row, so large results stream without being buffered.
pub trait TableSink {
    /// Called once before the first row.
    ///
    /// # Errors
    /// Output failures.
    fn begin(&mut self, columns: &[Column]) -> io::Result<()>;

    /// Called for every row; `cells` has one entry per column.
    ///
    /// # Errors
    /// Output failures.
    fn row(&mut self, cells: &[Cell]) -> io::Result<()>;

    /// Called once after the last row.
    ///
    /// # Errors
    /// Output failures.
    fn finish(&mut self) -> io::Result<()>;
}

/// Collects a (small) table in memory.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TableBuffer {
    /// Columns.
    pub columns: Vec<Column>,
    /// Rows.
    pub rows: Vec<Vec<Cell>>,
}

impl TableSink for TableBuffer {
    fn begin(&mut self, columns: &[Column]) -> io::Result<()> {
        self.columns = columns.to_vec();
        Ok(())
    }

    fn row(&mut self, cells: &[Cell]) -> io::Result<()> {
        self.rows.push(cells.to_vec());
        Ok(())
    }

    fn finish(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_render_as_plain_text() {
        let date = Date::parse("2026-10-07").unwrap();
        let cells = [
            Cell::Null,
            Cell::Text("a".into()),
            Cell::Integer(-3),
            Cell::Float(1.23456),
            Cell::Float(-0.001),
            Cell::Boolean(true),
            Cell::Date(date),
            Cell::Timestamp(date.start()),
        ];
        let rendered: Vec<String> = cells.iter().map(ToString::to_string).collect();
        assert_eq!(
            rendered,
            [
                "",
                "a",
                "-3",
                "1.23",
                "0",
                "true",
                "2026-10-07",
                "2026-10-07T00:00:00Z"
            ]
        );
    }

    #[test]
    fn cells_serialize_untagged() {
        let date = Date::parse("2026-10-07").unwrap();
        let json = serde_json::to_string(&[
            Cell::Null,
            Cell::Integer(5),
            Cell::Date(date),
            Cell::Timestamp(date.start()),
        ])
        .unwrap();
        assert_eq!(json, r#"[null,5,"2026-10-07","2026-10-07T00:00:00Z"]"#);
    }
}
