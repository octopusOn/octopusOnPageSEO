use std::io::{self, Write};

use seonexus_core::table::{Cell, Column, ColumnKind, TableSink};

/// Widest rendered cell, in characters; longer values are shortened with `…`.
const MAX_CELL_CHARS: usize = 72;

/// Buffers a page of rows and renders them as an aligned text table.
#[derive(Debug, Default)]
pub(crate) struct TextTable {
    columns: Vec<Column>,
    rows: Vec<Vec<String>>,
}

impl TableSink for TextTable {
    fn begin(&mut self, columns: &[Column]) -> io::Result<()> {
        self.columns = columns.to_vec();
        Ok(())
    }

    fn row(&mut self, cells: &[Cell]) -> io::Result<()> {
        self.rows.push(cells.iter().map(render_cell).collect());
        Ok(())
    }

    fn finish(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl TextTable {
    /// Number of buffered rows.
    pub(crate) fn len(&self) -> usize {
        self.rows.len()
    }

    /// Writes the header and at most `limit` rows.
    pub(crate) fn write(&self, out: &mut dyn Write, limit: usize) -> io::Result<()> {
        let rows = &self.rows[..self.rows.len().min(limit)];
        let widths: Vec<usize> = self
            .columns
            .iter()
            .enumerate()
            .map(|(index, column)| {
                rows.iter()
                    .map(|row| row[index].chars().count())
                    .chain([column.name.chars().count()])
                    .max()
                    .unwrap_or(0)
            })
            .collect();
        let header: Vec<String> = self.columns.iter().map(|c| c.name.clone()).collect();
        self.write_line(out, &header, &widths)?;
        let rule: Vec<String> = widths.iter().map(|&w| "─".repeat(w)).collect();
        self.write_line(out, &rule, &widths)?;
        for row in rows {
            self.write_line(out, row, &widths)?;
        }
        Ok(())
    }

    fn write_line(
        &self,
        out: &mut dyn Write,
        cells: &[String],
        widths: &[usize],
    ) -> io::Result<()> {
        let last = cells.len().saturating_sub(1);
        for (index, ((cell, &width), column)) in
            cells.iter().zip(widths).zip(&self.columns).enumerate()
        {
            if index > 0 {
                out.write_all(b"  ")?;
            }
            let numeric = matches!(column.kind, ColumnKind::Integer | ColumnKind::Float);
            if numeric {
                write!(out, "{cell:>width$}")?;
            } else if index == last {
                // No trailing padding after the last column.
                out.write_all(cell.as_bytes())?;
            } else {
                write!(out, "{cell:<width$}")?;
            }
        }
        out.write_all(b"\n")
    }
}

fn render_cell(cell: &Cell) -> String {
    let text = match cell {
        Cell::Null => "-".to_owned(),
        other => other.to_string(),
    };
    // Control characters in page text would break the layout or the terminal.
    let clean: String = text
        .chars()
        .map(|c| if c.is_control() { '·' } else { c })
        .collect();
    if clean.chars().count() <= MAX_CELL_CHARS {
        return clean;
    }
    let mut short: String = clean.chars().take(MAX_CELL_CHARS - 1).collect();
    short.push('…');
    short
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aligns_text_left_and_numbers_right() {
        let mut table = TextTable::default();
        table
            .begin(&[
                Column::new("url", ColumnKind::Text),
                Column::new("hits", ColumnKind::Integer),
            ])
            .unwrap();
        table
            .row(&[Cell::Text("/a".into()), Cell::Integer(5)])
            .unwrap();
        table
            .row(&[Cell::Text("/long\npath".into()), Cell::Integer(12_345)])
            .unwrap();
        table.row(&[Cell::Null, Cell::Integer(1)]).unwrap();
        let mut out = Vec::new();
        table.write(&mut out, 2).unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "url          hits\n──────────  ─────\n/a              5\n/long·path  12345\n"
        );
    }

    #[test]
    fn shortens_long_cells() {
        let rendered = render_cell(&Cell::Text("x".repeat(100)));
        assert_eq!(rendered.chars().count(), MAX_CELL_CHARS);
        assert!(rendered.ends_with('…'));
    }
}
