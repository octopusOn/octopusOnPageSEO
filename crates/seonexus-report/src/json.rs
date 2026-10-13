//! JSON export: one array of objects keyed by column name, streamed row by row.

use std::io::{self, Write};

use seonexus_core::table::{Cell, Column, TableSink};

/// Writes `[{"col": value, ...}, ...]`; dates and timestamps become ISO 8601 strings.
#[derive(Debug)]
pub struct JsonWriter<W: Write> {
    out: W,
    keys: Vec<String>,
    rows: u64,
}

impl<W: Write> JsonWriter<W> {
    /// Wraps `out`; pass a buffered writer for files.
    pub fn new(out: W) -> Self {
        Self {
            out,
            keys: Vec::new(),
            rows: 0,
        }
    }

    /// Returns the underlying writer.
    pub fn into_inner(self) -> W {
        self.out
    }
}

impl<W: Write> TableSink for JsonWriter<W> {
    fn begin(&mut self, columns: &[Column]) -> io::Result<()> {
        // Keys are encoded once; every row reuses them.
        self.keys = columns
            .iter()
            .map(|column| serde_json::to_string(&column.name))
            .collect::<Result<_, _>>()?;
        self.out.write_all(b"[")
    }

    fn row(&mut self, cells: &[Cell]) -> io::Result<()> {
        self.out
            .write_all(if self.rows == 0 { b"\n{" } else { b",\n{" })?;
        for (index, (key, cell)) in self.keys.iter().zip(cells).enumerate() {
            if index > 0 {
                self.out.write_all(b",")?;
            }
            self.out.write_all(key.as_bytes())?;
            self.out.write_all(b":")?;
            serde_json::to_writer(&mut self.out, cell)?;
        }
        self.rows += 1;
        self.out.write_all(b"}")
    }

    fn finish(&mut self) -> io::Result<()> {
        self.out.write_all(b"\n]\n")?;
        self.out.flush()
    }
}

#[cfg(test)]
mod tests {
    use seonexus_core::Date;
    use seonexus_core::table::ColumnKind;

    use super::*;

    #[test]
    fn writes_valid_array_of_objects() {
        let mut writer = JsonWriter::new(Vec::new());
        writer
            .begin(&[
                Column::new("day", ColumnKind::Date),
                Column::new("u\"rl", ColumnKind::Text),
            ])
            .unwrap();
        let day = Date::parse("2026-10-07").unwrap();
        writer
            .row(&[Cell::Date(day), Cell::Text("/a\"b".into())])
            .unwrap();
        writer.row(&[Cell::Null, Cell::Null]).unwrap();
        writer.finish().unwrap();

        let json: serde_json::Value = serde_json::from_slice(&writer.into_inner()).unwrap();
        assert_eq!(
            json,
            serde_json::json!([
                {"day": "2026-10-07", "u\"rl": "/a\"b"},
                {"day": null, "u\"rl": null}
            ])
        );
    }

    #[test]
    fn empty_result_is_empty_array() {
        let mut writer = JsonWriter::new(Vec::new());
        writer.begin(&[]).unwrap();
        writer.finish().unwrap();
        let json: serde_json::Value = serde_json::from_slice(&writer.into_inner()).unwrap();
        assert_eq!(json, serde_json::json!([]));
    }
}
