//! A report's numbers as an Excel workbook: its headline tiles on the first sheet, then one
//! sheet per table, numbers as numbers.

use rust_xlsxwriter::{Format, Workbook, XlsxError};

use super::{Cell, ReportView};

/// Excel's longest sheet name.
const SHEET_NAME: usize = 31;

/// A sheet name Excel accepts, unique among `taken`.
fn sheet_name(title: &str, taken: &mut Vec<String>) -> String {
    let clean: String = title
        .chars()
        .map(|c| {
            if matches!(c, '[' | ']' | ':' | '*' | '?' | '/' | '\\') {
                ' '
            } else {
                c
            }
        })
        .collect();
    let clean = clean.trim().trim_matches('\'').to_owned();
    let base: String = if clean.is_empty() {
        "Sheet".to_owned()
    } else {
        clean
    }
    .chars()
    .take(SHEET_NAME)
    .collect();
    let mut name = base.clone();
    let mut counter = 2;
    while taken.iter().any(|known| known.eq_ignore_ascii_case(&name)) {
        let suffix = format!(" {counter}");
        name = format!(
            "{}{suffix}",
            base.chars()
                .take(SHEET_NAME - suffix.len())
                .collect::<String>()
        );
        counter += 1;
    }
    taken.push(name.clone());
    name
}

/// The workbook's bytes.
///
/// # Errors
/// [`XlsxError`] when the workbook cannot be written.
pub(crate) fn workbook(view: &ReportView) -> Result<Vec<u8>, XlsxError> {
    let mut book = Workbook::new();
    let bold = Format::new().set_bold();
    let mut taken = Vec::new();
    {
        let sheet = book.add_worksheet();
        sheet.set_name(sheet_name(&view.name, &mut taken))?;
        sheet.write_string_with_format(0, 0, &view.name, &bold)?;
        sheet.write_string(1, 0, format!("{} · {}", view.project, view.domain))?;
        sheet.write_string(2, 0, format!("{} – {}", view.period.0, view.period.1))?;
        let mut row = 4;
        for widget in &view.widgets {
            if widget.tiles.is_empty() && widget.lines.is_empty() {
                continue;
            }
            sheet.write_string_with_format(row, 0, &widget.title, &bold)?;
            row += 1;
            if let Some(note) = widget.note.as_ref().filter(|_| !widget.lines.is_empty()) {
                sheet.write_string(row, 0, note)?;
                row += 1;
            }
            for line in &widget.lines {
                sheet.write_string(row, 0, &line.text)?;
                row += 1;
            }
            for tile in &widget.tiles {
                sheet.write_string(row, 0, &tile.label)?;
                sheet.write_string(row, 1, &tile.value)?;
                if let Some(change) = &tile.change {
                    sheet.write_string(row, 2, &change.text)?;
                }
                row += 1;
            }
            row += 1;
        }
        sheet.set_column_width(0, 34)?;
        sheet.set_column_width(1, 18)?;
    }
    for widget in &view.widgets {
        for table in &widget.tables {
            if table.rows.is_empty() {
                continue;
            }
            let sheet = book.add_worksheet();
            sheet.set_name(sheet_name(&table.title, &mut taken))?;
            for (column, name) in table.columns.iter().enumerate() {
                let column = u16::try_from(column).unwrap_or(u16::MAX);
                sheet.write_string_with_format(0, column, name, &bold)?;
                sheet.set_column_width(column, if column == 0 { 40 } else { 14 })?;
            }
            for (index, cells) in table.rows.iter().enumerate() {
                let row = u32::try_from(index + 1).unwrap_or(u32::MAX);
                for (column, cell) in cells.iter().enumerate() {
                    let column = u16::try_from(column).unwrap_or(u16::MAX);
                    match cell {
                        Cell::Text(text) => {
                            sheet.write_string(row, column, text)?;
                        }
                        Cell::Number { value, .. } => {
                            sheet.write_number(row, column, *value)?;
                        }
                    }
                }
            }
        }
    }
    book.save_to_buffer()
}

#[cfg(test)]
mod tests {
    use super::sheet_name;

    #[test]
    fn sheet_names_are_short_clean_and_unique() {
        let mut taken = Vec::new();
        assert_eq!(
            sheet_name("Search Console: top/pages?", &mut taken),
            "Search Console  top pages"
        );
        assert_eq!(
            sheet_name("search console  top pages", &mut taken),
            "search console  top pages 2"
        );
        let long = "x".repeat(40);
        assert_eq!(sheet_name(&long, &mut taken).len(), 31);
        assert_eq!(
            sheet_name(&long, &mut taken),
            format!("{} 2", "x".repeat(29))
        );
        assert_eq!(sheet_name("  ", &mut taken), "Sheet");
    }
}
