use super::{digest, xlsx};
use std::fs;
use std::io::Read;
use std::path::Path;

pub(crate) const FILE_LIMIT: u64 = 25 * 1024 * 1024;
pub(crate) const ROW_LIMIT: usize = 100_000;
#[derive(Debug, Clone)]
pub(crate) struct ImportCell {
    pub text: String,
    pub numeric: bool,
}
pub(crate) struct TabularFile {
    pub sheets: Vec<String>,
    pub rows: Vec<Vec<ImportCell>>,
    pub digest: String,
}

#[allow(clippy::too_many_lines)] // Bounded decoding and delimited-record parsing share one import boundary.
pub(crate) fn read(path: &Path, sheet: Option<&str>) -> Result<TabularFile, String> {
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() > FILE_LIMIT {
        return Err("File exceeds 25 MiB".into());
    }
    let mut bytes = Vec::new();
    file.take(FILE_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > FILE_LIMIT {
        return Err("File exceeds 25 MiB".into());
    }
    let hash = digest(&bytes);
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("xlsx"))
    {
        let (sheets, rows) = xlsx::read(&bytes, sheet)?;
        return Ok(TabularFile {
            sheets,
            rows,
            digest: hash,
        });
    }
    if !path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("csv") || e.eq_ignore_ascii_case("tsv"))
    {
        return Err("Choose a CSV, TSV or XLSX file".into());
    }
    let text = if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
        let encoding = if bytes[0] == 0xff {
            encoding_rs::UTF_16LE
        } else {
            encoding_rs::UTF_16BE
        };
        let (text, _, errors) = encoding.decode(&bytes[2..]);
        if errors {
            return Err("Invalid text encoding".into());
        }
        text.into_owned()
    } else {
        String::from_utf8(bytes)
            .map_err(|_| "Choose a UTF-8 or UTF-16 CSV file")?
            .trim_start_matches('\u{feff}')
            .to_owned()
    };
    let (text, delimiter) = if text.starts_with("sep=")
        && text
            .as_bytes()
            .get(4)
            .is_some_and(|b| matches!(b, b',' | b';' | b'\t'))
    {
        let delimiter = text.as_bytes()[4];
        (
            text.split_once('\n').map_or("", |(_, rest)| rest),
            delimiter,
        )
    } else {
        let first = text
            .lines()
            .find(|s| !s.trim().is_empty() && !s.starts_with('#'))
            .unwrap_or("");
        let delimiter = if first.contains('\t') {
            b'\t'
        } else if first.matches(';').count() > first.matches(',').count() {
            b';'
        } else {
            b','
        };
        (text.as_str(), delimiter)
    };
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .comment(Some(b'#'))
        .delimiter(delimiter)
        .from_reader(text.as_bytes());
    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record.map_err(|e| e.to_string())?;
        if record.iter().all(|s| s.trim().is_empty()) {
            continue;
        }
        if rows.len() > ROW_LIMIT + 30 || record.len() > 128 {
            return Err("File exceeds 100,000 rows or 128 columns".into());
        }
        rows.push(
            record
                .iter()
                .map(|text| ImportCell {
                    text: text.into(),
                    numeric: false,
                })
                .collect(),
        );
    }
    Ok(TabularFile {
        sheets: Vec::new(),
        rows,
        digest: hash,
    })
}
