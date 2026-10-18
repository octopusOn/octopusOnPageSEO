use std::io::{self, Write};

/// Widest rendered cell, in characters; longer values are shortened with `…`.
const MAX_CELL_CHARS: usize = 64;

/// Writes rows as an aligned text table.
pub(crate) fn table(out: &mut dyn Write, header: &[&str], rows: &[Vec<String>]) -> io::Result<()> {
    let cells: Vec<Vec<String>> = rows
        .iter()
        .map(|row| row.iter().map(|cell| shorten(cell)).collect())
        .collect();
    let widths: Vec<usize> = header
        .iter()
        .enumerate()
        .map(|(index, name)| {
            cells
                .iter()
                .filter_map(|row| row.get(index))
                .map(|cell| cell.chars().count())
                .chain([name.chars().count()])
                .max()
                .unwrap_or(0)
        })
        .collect();
    let line = |out: &mut dyn Write, values: &[String]| -> io::Result<()> {
        let mut text = String::new();
        for (index, width) in widths.iter().enumerate() {
            let value = values.get(index).map_or("", String::as_str);
            text.push_str(value);
            if index + 1 < widths.len() {
                text.push_str(&" ".repeat(width.saturating_sub(value.chars().count()) + 2));
            }
        }
        writeln!(out, "{}", text.trim_end())
    };
    line(
        out,
        &header
            .iter()
            .map(|name| (*name).to_owned())
            .collect::<Vec<_>>(),
    )?;
    line(
        out,
        &widths
            .iter()
            .map(|width| "─".repeat(*width))
            .collect::<Vec<_>>(),
    )?;
    for row in &cells {
        line(out, row)?;
    }
    Ok(())
}

fn shorten(cell: &str) -> String {
    if cell.chars().count() <= MAX_CELL_CHARS {
        return cell.to_owned();
    }
    let mut short: String = cell.chars().take(MAX_CELL_CHARS - 1).collect();
    short.push('…');
    short
}

/// US dollars with enough digits for per-request prices (`0.015`, `0.0004`, `12.50`).
pub(crate) fn usd(value: f64) -> String {
    if value == 0.0 {
        return "0".to_owned();
    }
    if value >= 1.0 {
        return format!("{value:.2}");
    }
    let text = format!("{value:.4}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_prices() {
        assert_eq!(usd(0.0), "0");
        assert_eq!(usd(0.015), "0.015");
        assert_eq!(usd(0.0004), "0.0004");
        assert_eq!(usd(0.1), "0.1");
        assert_eq!(usd(12.5), "12.50");
    }
}
