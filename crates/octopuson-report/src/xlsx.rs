//! Excel (XLSX) export with native number, date and timestamp cells.

use std::io::{self, Seek, Write};

use octopuson_core::table::{Cell, Column, ColumnKind, TableSink};
use rust_xlsxwriter::{Format, Workbook, Worksheet, XlsxError};

/// Data rows fitting on one sheet (Excel's 1,048,576 rows minus the header).
pub const MAX_XLSX_ROWS: u32 = 1_048_575;

/// Longest text Excel stores in a cell; longer values are truncated.
const MAX_CELL_CHARS: usize = 32_767;

/// Days between Excel's epoch (1899-12-30) and the Unix epoch.
const EXCEL_UNIX_EPOCH_DAYS: f64 = 25_569.0;
const MICROS_PER_DAY: f64 = 86_400_000_000.0;

/// Writes a table to a single worksheet in constant-memory mode, so exports of up to
/// [`MAX_XLSX_ROWS`] rows stream to disk instead of being held in memory.
pub struct XlsxWriter<W: Write + Seek + Send> {
    workbook: Workbook,
    formats: Formats,
    out: Option<W>,
    next_row: u32,
}

struct Formats {
    header: Format,
    date: Format,
    timestamp: Format,
    decimal: Format,
}

impl<W: Write + Seek + Send> XlsxWriter<W> {
    /// Creates a workbook that is written to `out` by [`TableSink::finish`].
    pub fn new(out: W) -> Self {
        Self {
            workbook: Workbook::new(),
            formats: Formats {
                header: Format::new().set_bold(),
                date: Format::new().set_num_format("yyyy-mm-dd"),
                timestamp: Format::new().set_num_format("yyyy-mm-dd hh:mm:ss"),
                decimal: Format::new().set_num_format("0.00"),
            },
            out: Some(out),
            next_row: 0,
        }
    }
}

impl<W: Write + Seek + Send> TableSink for XlsxWriter<W> {
    fn begin(&mut self, columns: &[Column]) -> io::Result<()> {
        let header = self.formats.header.clone();
        let sheet = self
            .workbook
            .add_worksheet_with_constant_memory()
            .map_err(to_io)?;
        // Layout must be set before the first row in constant-memory mode.
        sheet.set_freeze_panes(1, 0).map_err(to_io)?;
        for (index, column) in (0_u16..).zip(columns) {
            sheet
                .set_column_width(index, column_width(column.kind))
                .map_err(to_io)?;
            sheet
                .write_string_with_format(0, index, &column.name, &header)
                .map_err(to_io)?;
        }
        self.next_row = 1;
        Ok(())
    }

    fn row(&mut self, cells: &[Cell]) -> io::Result<()> {
        if self.next_row > MAX_XLSX_ROWS {
            return Err(io::Error::other(format!(
                "XLSX holds at most {MAX_XLSX_ROWS} rows; export as CSV or add a limit"
            )));
        }
        let row = self.next_row;
        let Self {
            workbook, formats, ..
        } = self;
        let sheet = workbook.worksheet_from_index(0).map_err(to_io)?;
        for (index, cell) in (0_u16..).zip(cells) {
            write_cell(sheet, formats, row, index, cell).map_err(to_io)?;
        }
        self.next_row += 1;
        Ok(())
    }

    fn finish(&mut self) -> io::Result<()> {
        let out = self
            .out
            .take()
            .ok_or_else(|| io::Error::other("XLSX export was already finished"))?;
        self.workbook.save_to_writer(out).map_err(to_io)
    }
}

impl<W: Write + Seek + Send> std::fmt::Debug for XlsxWriter<W> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("XlsxWriter")
            .field("next_row", &self.next_row)
            .finish_non_exhaustive()
    }
}

fn write_cell(
    sheet: &mut Worksheet,
    formats: &Formats,
    row: u32,
    col: u16,
    cell: &Cell,
) -> Result<(), XlsxError> {
    #[allow(clippy::cast_precision_loss)] // Exact up to 2^53, far beyond log counters.
    match cell {
        Cell::Null => return Ok(()),
        Cell::Text(text) => sheet.write_string(row, col, truncate(text)),
        Cell::Integer(value) => sheet.write_number(row, col, *value as f64),
        Cell::Float(value) => sheet.write_number_with_format(row, col, *value, &formats.decimal),
        Cell::Boolean(value) => sheet.write_boolean(row, col, *value),
        Cell::Date(date) => sheet.write_number_with_format(
            row,
            col,
            f64::from(date.days_since_epoch()) + EXCEL_UNIX_EPOCH_DAYS,
            &formats.date,
        ),
        Cell::Timestamp(ts) => sheet.write_number_with_format(
            row,
            col,
            ts.as_unix_micros() as f64 / MICROS_PER_DAY + EXCEL_UNIX_EPOCH_DAYS,
            &formats.timestamp,
        ),
    }?;
    Ok(())
}

fn truncate(text: &str) -> &str {
    text.char_indices()
        .nth(MAX_CELL_CHARS)
        .map_or(text, |(end, _)| &text[..end])
}

fn column_width(kind: ColumnKind) -> f64 {
    match kind {
        ColumnKind::Text => 48.0,
        ColumnKind::Timestamp => 20.0,
        ColumnKind::Integer | ColumnKind::Float | ColumnKind::Boolean | ColumnKind::Date => 12.0,
    }
}

fn to_io(error: XlsxError) -> io::Error {
    io::Error::other(error)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use octopuson_core::Date;

    use super::*;

    fn columns() -> Vec<Column> {
        vec![
            Column::new("day", ColumnKind::Date),
            Column::new("url", ColumnKind::Text),
            Column::new("hits", ColumnKind::Integer),
        ]
    }

    #[test]
    fn writes_a_zip_container() {
        let mut out = Cursor::new(Vec::new());
        let mut writer = XlsxWriter::new(&mut out);
        writer.begin(&columns()).unwrap();
        let day = Date::parse("2026-10-07").unwrap();
        writer
            .row(&[Cell::Date(day), Cell::Text("/a".into()), Cell::Integer(3)])
            .unwrap();
        writer.row(&[Cell::Null, Cell::Null, Cell::Null]).unwrap();
        writer.finish().unwrap();
        assert!(out.get_ref().starts_with(b"PK\x03\x04"));
    }

    #[test]
    fn refuses_rows_beyond_the_sheet_limit() {
        let mut writer = XlsxWriter::new(Cursor::new(Vec::new()));
        writer.begin(&columns()).unwrap();
        writer.next_row = MAX_XLSX_ROWS + 1;
        let error = writer
            .row(&[Cell::Null, Cell::Null, Cell::Null])
            .unwrap_err();
        assert!(error.to_string().contains("CSV"));
    }

    #[test]
    fn reports_unavailable_temporary_directory_without_panicking() {
        let dir = tempfile::tempdir().unwrap();
        let mut writer = XlsxWriter::new(Cursor::new(Vec::new()));
        writer.workbook.set_tempdir(dir.path()).unwrap();
        dir.close().unwrap();
        assert!(writer.begin(&columns()).is_err());
    }

    #[test]
    fn truncates_overlong_text_on_char_boundaries() {
        let long = "ğ".repeat(MAX_CELL_CHARS + 5);
        assert_eq!(truncate(&long).chars().count(), MAX_CELL_CHARS);
        assert_eq!(truncate("short"), "short");
    }
}
