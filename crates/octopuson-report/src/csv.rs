//! RFC 4180 CSV export.

use std::io::{self, Write};

use octopuson_core::table::{Cell, Column, TableSink};

/// Writes a table as comma-separated values with a header row and CRLF line endings.
///
/// Text cells that a spreadsheet would evaluate as a formula (`=`, `+`, `-`, `@`, tab or
/// carriage return first) are prefixed with `'`, following the OWASP guidance on CSV
/// injection: log lines are attacker-controlled (user agents, referers, URLs).
#[derive(Debug)]
pub struct CsvWriter<W: Write> {
    out: W,
}

impl<W: Write> CsvWriter<W> {
    /// Wraps `out`; pass a buffered writer for files.
    pub fn new(out: W) -> Self {
        Self { out }
    }

    /// Returns the underlying writer.
    pub fn into_inner(self) -> W {
        self.out
    }

    fn record<'a>(&mut self, fields: impl Iterator<Item = Field<'a>>) -> io::Result<()> {
        for (index, field) in fields.enumerate() {
            if index > 0 {
                self.out.write_all(b",")?;
            }
            match field {
                Field::Text(text) => write_text(&mut self.out, text)?,
                Field::Cell(cell) => write!(self.out, "{cell}")?,
            }
        }
        self.out.write_all(b"\r\n")
    }
}

enum Field<'a> {
    Text(&'a str),
    Cell(&'a Cell),
}

impl<'a> Field<'a> {
    fn of(cell: &'a Cell) -> Self {
        match cell {
            Cell::Text(text) => Self::Text(text),
            other => Self::Cell(other),
        }
    }
}

impl<W: Write> TableSink for CsvWriter<W> {
    fn begin(&mut self, columns: &[Column]) -> io::Result<()> {
        self.record(columns.iter().map(|column| Field::Text(&column.name)))
    }

    fn row(&mut self, cells: &[Cell]) -> io::Result<()> {
        self.record(cells.iter().map(Field::of))
    }

    fn finish(&mut self) -> io::Result<()> {
        self.out.flush()
    }
}

fn write_text(out: &mut impl Write, text: &str) -> io::Result<()> {
    let guard = is_formula_like(text);
    if !guard && !needs_quotes(text) {
        return out.write_all(text.as_bytes());
    }
    out.write_all(b"\"")?;
    if guard {
        out.write_all(b"'")?;
    }
    for (index, part) in text.split('"').enumerate() {
        if index > 0 {
            out.write_all(b"\"\"")?;
        }
        out.write_all(part.as_bytes())?;
    }
    out.write_all(b"\"")
}

fn needs_quotes(text: &str) -> bool {
    text.contains([',', '"', '\r', '\n']) || text.starts_with(' ') || text.ends_with(' ')
}

/// A lone `-` (the usual "no value" marker in logs) cannot start a formula.
fn is_formula_like(text: &str) -> bool {
    text.len() > 1 && text.starts_with(['=', '+', '-', '@', '\t', '\r'])
}

#[cfg(test)]
mod tests {
    use octopuson_core::table::ColumnKind;

    use super::*;

    fn render(rows: &[Vec<Cell>]) -> String {
        let mut writer = CsvWriter::new(Vec::new());
        writer
            .begin(&[
                Column::new("url", ColumnKind::Text),
                Column::new("hits", ColumnKind::Integer),
            ])
            .unwrap();
        for row in rows {
            writer.row(row).unwrap();
        }
        writer.finish().unwrap();
        String::from_utf8(writer.into_inner()).unwrap()
    }

    #[test]
    fn writes_header_and_plain_rows() {
        let csv = render(&[vec![Cell::Text("/a".into()), Cell::Integer(3)]]);
        assert_eq!(csv, "url,hits\r\n/a,3\r\n");
    }

    #[test]
    fn quotes_separators_quotes_and_newlines() {
        let csv = render(&[
            vec![Cell::Text("/a,b".into()), Cell::Null],
            vec![Cell::Text("say \"hi\"".into()), Cell::Integer(1)],
            vec![Cell::Text("x\ny".into()), Cell::Integer(2)],
        ]);
        assert_eq!(
            csv,
            "url,hits\r\n\"/a,b\",\r\n\"say \"\"hi\"\"\",1\r\n\"x\ny\",2\r\n"
        );
    }

    #[test]
    fn neutralizes_formula_like_text() {
        let csv = render(&[
            vec![Cell::Text("=HYPERLINK(\"x\")".into()), Cell::Integer(1)],
            vec![Cell::Text("-".into()), Cell::Integer(-1)],
        ]);
        assert_eq!(csv, "url,hits\r\n\"'=HYPERLINK(\"\"x\"\")\",1\r\n-,-1\r\n");
    }
}
