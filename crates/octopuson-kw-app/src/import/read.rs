//! Files to rows of cells: delimited text in any common encoding, the first sheet of an
//! Excel workbook, and the CSV inside a Search Console export archive.
//!
//! Workbooks are read for their cached cell values only: formulas, macros, external links
//! and embedded objects are never run or followed. Archive parts are size-limited.

use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::path::Path;

use quick_xml::Reader;
use quick_xml::events::{BytesRef, BytesStart, Event};

/// Largest file read, bytes.
pub(crate) const MAX_FILE: u64 = 256 * 1024 * 1024;
/// Largest expanded part of an archive, bytes.
const MAX_PART: u64 = 256 * 1024 * 1024;
/// Most rows read.
pub(crate) const MAX_ROWS: usize = 250_000;
/// Most columns read per row.
const MAX_COLUMNS: usize = 200;

/// Kind of file, from its extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FileKind {
    /// One keyword per line.
    Lines,
    /// Comma, semicolon or tab separated.
    Delimited,
    /// Excel workbook.
    Workbook,
    /// ZIP archive with a CSV (Search Console export).
    Archive,
}

impl FileKind {
    pub(crate) fn of(extension: &str) -> Option<Self> {
        Some(match extension.to_ascii_lowercase().as_str() {
            "txt" => Self::Lines,
            "csv" | "tsv" | "tab" => Self::Delimited,
            "xlsx" | "xlsm" => Self::Workbook,
            "zip" => Self::Archive,
            _ => return None,
        })
    }
}

/// Rows of a file.
pub(crate) fn rows(bytes: &[u8], kind: FileKind) -> Result<Vec<Vec<String>>, String> {
    match kind {
        FileKind::Lines => Ok(decode(bytes)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .take(MAX_ROWS)
            .map(|line| vec![line.to_owned()])
            .collect()),
        FileKind::Delimited => Ok(delimited(&decode(bytes))),
        FileKind::Workbook => workbook(bytes),
        FileKind::Archive => archive_csv(bytes),
    }
}

/// Text of a file: UTF-8 or UTF-16 by their byte order marks, UTF-16 without one when every
/// other byte is zero, UTF-8 when valid, else Windows-1254 (Turkish).
pub(crate) fn decode(bytes: &[u8]) -> String {
    if let Some((encoding, length)) = encoding_rs::Encoding::for_bom(bytes) {
        return encoding
            .decode_without_bom_handling(&bytes[length..])
            .0
            .into_owned();
    }
    let sample = &bytes[..bytes.len().min(512)];
    if sample.len() >= 4 {
        let zeros = |offset: usize| {
            sample
                .iter()
                .skip(offset)
                .step_by(2)
                .filter(|&&byte| byte == 0)
                .count()
        };
        let half = sample.len() / 2;
        if zeros(1) * 10 >= half * 9 {
            return encoding_rs::UTF_16LE
                .decode_without_bom_handling(bytes)
                .0
                .into_owned();
        }
        if zeros(0) * 10 >= half * 9 {
            return encoding_rs::UTF_16BE
                .decode_without_bom_handling(bytes)
                .0
                .into_owned();
        }
    }
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_owned(),
        Err(_) => encoding_rs::WINDOWS_1254.decode(bytes).0.into_owned(),
    }
}

/// The separator used most outside quotes in the first lines: tab, semicolon or comma.
fn separator(text: &str) -> char {
    let mut counts = [(0_usize, '\t'), (0, ';'), (0, ',')];
    let mut quoted = false;
    for character in text.chars().take(20_000) {
        match character {
            '"' => quoted = !quoted,
            _ if quoted => {}
            _ => {
                for (count, candidate) in &mut counts {
                    if character == *candidate {
                        *count += 1;
                    }
                }
            }
        }
    }
    // Ties go to the earlier candidate: tabs and semicolons rarely appear in keywords.
    counts
        .iter()
        .fold((0, ','), |best, &(count, candidate)| {
            if count > best.0 {
                (count, candidate)
            } else {
                best
            }
        })
        .1
}

/// Rows of delimited text (RFC 4180 quoting, any line ending).
pub(crate) fn delimited(text: &str) -> Vec<Vec<String>> {
    let separator = separator(text);
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut cell = String::new();
    let mut quoted = false;
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        if quoted {
            if character == '"' {
                if characters.next_if_eq(&'"').is_some() {
                    cell.push('"');
                } else {
                    quoted = false;
                }
            } else {
                cell.push(character);
            }
            continue;
        }
        match character {
            '"' if cell.trim().is_empty() => {
                cell.clear();
                quoted = true;
            }
            '\r' | '\n' => {
                if character == '\r' {
                    characters.next_if_eq(&'\n');
                }
                push_cell(&mut row, &mut cell);
                if row.iter().any(|value| !value.is_empty()) {
                    rows.push(std::mem::take(&mut row));
                    if rows.len() >= MAX_ROWS {
                        return rows;
                    }
                } else {
                    row.clear();
                }
            }
            _ if character == separator => push_cell(&mut row, &mut cell),
            _ => cell.push(character),
        }
    }
    push_cell(&mut row, &mut cell);
    if row.iter().any(|value| !value.is_empty()) {
        rows.push(row);
    }
    rows
}

fn push_cell(row: &mut Vec<String>, cell: &mut String) {
    if row.len() < MAX_COLUMNS {
        row.push(cell.trim().to_owned());
    }
    cell.clear();
}

fn part(archive: &mut zip::ZipArchive<Cursor<&[u8]>>, name: &str) -> Result<Vec<u8>, String> {
    let entry = archive.by_name(name).map_err(|error| error.to_string())?;
    if entry.size() > MAX_PART {
        return Err(format!("`{name}` is larger than {} MiB", MAX_PART >> 20));
    }
    let mut bytes = Vec::new();
    entry
        .take(MAX_PART + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > MAX_PART {
        return Err(format!("`{name}` is larger than {} MiB", MAX_PART >> 20));
    }
    Ok(bytes)
}

/// The query table of a Search Console export (`Queries.csv`), else its first CSV.
fn archive_csv(bytes: &[u8]) -> Result<Vec<Vec<String>>, String> {
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|error| error.to_string())?;
    let names: Vec<String> = archive
        .file_names()
        .flatten()
        .map(std::borrow::Cow::into_owned)
        .collect();
    let csv = |name: &&String| name.to_ascii_lowercase().ends_with(".csv");
    let chosen = names
        .iter()
        .filter(csv)
        .find(|name| {
            let lower = name.to_ascii_lowercase();
            lower.ends_with("queries.csv") || lower.ends_with("sorgular.csv")
        })
        .or_else(|| names.iter().find(csv))
        .ok_or("the archive has no CSV file")?
        .clone();
    Ok(delimited(&decode(&part(&mut archive, &chosen)?)))
}

/// Cells of the first sheet of a workbook, by their cached values.
fn workbook(bytes: &[u8]) -> Result<Vec<Vec<String>>, String> {
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|error| error.to_string())?;
    let names: Vec<String> = archive
        .file_names()
        .flatten()
        .map(std::borrow::Cow::into_owned)
        .collect();
    let sheet = first_sheet(&mut archive, &names)?;
    let shared = if names.iter().any(|name| name == "xl/sharedStrings.xml") {
        shared_strings(&part(&mut archive, "xl/sharedStrings.xml")?)?
    } else {
        Vec::new()
    };
    sheet_rows(&part(&mut archive, &sheet)?, &shared)
}

/// The part of the workbook's first sheet.
fn first_sheet(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
    names: &[String],
) -> Result<String, String> {
    let mut from_workbook = || -> Result<Option<String>, String> {
        let workbook = part(archive, "xl/workbook.xml")?;
        let Some(id) = first_attribute(&workbook, "sheet", "r:id")? else {
            return Ok(None);
        };
        let relations = part(archive, "xl/_rels/workbook.xml.rels")?;
        Ok(relation_target(&relations, &id)?.map(|target| {
            let target = target.trim_start_matches('/');
            if target.starts_with("xl/") {
                target.to_owned()
            } else {
                format!("xl/{target}")
            }
        }))
    };
    if let Ok(Some(sheet)) = from_workbook()
        && names.contains(&sheet)
    {
        return Ok(sheet);
    }
    let mut sheets: Vec<&String> = names
        .iter()
        .filter(|name| {
            name.starts_with("xl/worksheets/")
                && Path::new(name.as_str())
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("xml"))
        })
        .collect();
    sheets.sort();
    sheets
        .first()
        .map(|name| (*name).clone())
        .ok_or_else(|| "the workbook has no sheet".to_owned())
}

fn reader(bytes: &[u8]) -> Reader<&[u8]> {
    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().expand_empty_elements = true;
    reader
}

fn attribute(element: &BytesStart<'_>, name: &str) -> Option<String> {
    element
        .attributes()
        .flatten()
        .find(|attribute| attribute.key.as_ref() == name)
        .and_then(|attribute| {
            attribute
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .ok()
                .map(std::borrow::Cow::into_owned)
        })
}

fn first_attribute(bytes: &[u8], element: &str, name: &str) -> Result<Option<String>, String> {
    let mut reader = reader(bytes);
    loop {
        match reader.read_event().map_err(|error| error.to_string())? {
            Event::Start(start) if start.local_name().as_ref() == element => {
                return Ok(attribute(&start, name));
            }
            Event::Eof => return Ok(None),
            _ => {}
        }
    }
}

fn relation_target(bytes: &[u8], id: &str) -> Result<Option<String>, String> {
    let mut reader = reader(bytes);
    loop {
        match reader.read_event().map_err(|error| error.to_string())? {
            Event::Start(start)
                if start.local_name().as_ref() == "Relationship"
                    && attribute(&start, "Id").as_deref() == Some(id) =>
            {
                return Ok(attribute(&start, "Target"));
            }
            Event::Eof => return Ok(None),
            _ => {}
        }
    }
}

fn entity(reference: &BytesRef<'_>) -> String {
    if let Ok(Some(character)) = reference.resolve_char_ref() {
        return character.to_string();
    }
    match reference.as_ref() {
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "apos" => "'",
        "quot" => "\"",
        _ => "",
    }
    .to_owned()
}

/// Texts of `sharedStrings.xml`, rich-text runs joined; phonetic hints left out.
fn shared_strings(bytes: &[u8]) -> Result<Vec<String>, String> {
    let mut reader = reader(bytes);
    let mut strings = Vec::new();
    let mut current = String::new();
    let (mut in_text, mut in_phonetic) = (false, false);
    loop {
        match reader.read_event().map_err(|error| error.to_string())? {
            Event::Start(start) => match start.local_name().as_ref() {
                "si" => current.clear(),
                "t" => in_text = true,
                "rPh" => in_phonetic = true,
                _ => {}
            },
            Event::End(end) => match end.local_name().as_ref() {
                "si" => strings.push(std::mem::take(&mut current)),
                "t" => in_text = false,
                "rPh" => in_phonetic = false,
                _ => {}
            },
            Event::Text(text) if in_text && !in_phonetic => {
                current.push_str(&text.xml10_content());
            }
            Event::GeneralRef(reference) if in_text && !in_phonetic => {
                current.push_str(&entity(&reference));
            }
            Event::Eof => return Ok(strings),
            _ => {}
        }
    }
}

/// Column number of a cell reference (`B7` → 1).
fn column_of(reference: &str) -> Option<usize> {
    let mut column = 0_usize;
    let mut letters = 0;
    for byte in reference.bytes().take_while(u8::is_ascii_alphabetic) {
        column = column * 26 + usize::from(byte.to_ascii_uppercase() - b'A') + 1;
        letters += 1;
    }
    (letters > 0).then(|| column - 1)
}

/// What is being read inside a sheet cell.
#[derive(Default)]
struct CellState {
    column: usize,
    kind: Option<String>,
    value: String,
    in_value: bool,
}

/// Rows of a sheet part.
fn sheet_rows(bytes: &[u8], shared: &[String]) -> Result<Vec<Vec<String>>, String> {
    let mut reader = reader(bytes);
    let mut rows = Vec::new();
    let mut row: HashMap<usize, String> = HashMap::new();
    let mut cell = CellState::default();
    let mut next_column = 0;
    loop {
        match reader.read_event().map_err(|error| error.to_string())? {
            Event::Start(start) => match start.local_name().as_ref() {
                "row" => {
                    row.clear();
                    next_column = 0;
                }
                "c" => {
                    let column = attribute(&start, "r")
                        .and_then(|reference| column_of(&reference))
                        .unwrap_or(next_column);
                    cell = CellState {
                        column,
                        kind: attribute(&start, "t"),
                        ..CellState::default()
                    };
                    next_column = column + 1;
                }
                "v" | "t" => cell.in_value = true,
                _ => {}
            },
            Event::Text(text) if cell.in_value => {
                cell.value.push_str(&text.xml10_content());
            }
            Event::GeneralRef(reference) if cell.in_value => {
                cell.value.push_str(&entity(&reference));
            }
            Event::End(end) => match end.local_name().as_ref() {
                "v" | "t" => cell.in_value = false,
                "c" => {
                    let value = match cell.kind.as_deref() {
                        Some("s") => cell
                            .value
                            .trim()
                            .parse::<usize>()
                            .ok()
                            .and_then(|index| shared.get(index).cloned())
                            .unwrap_or_default(),
                        Some("b") => (if cell.value.trim() == "1" {
                            "TRUE"
                        } else {
                            "FALSE"
                        })
                        .to_owned(),
                        _ => std::mem::take(&mut cell.value),
                    };
                    if cell.column < MAX_COLUMNS {
                        row.insert(cell.column, value.trim().to_owned());
                    }
                }
                "row" => {
                    let width = row.keys().max().map_or(0, |last| last + 1);
                    let cells: Vec<String> = (0..width)
                        .map(|column| row.remove(&column).unwrap_or_default())
                        .collect();
                    if cells.iter().any(|value| !value.is_empty()) {
                        rows.push(cells);
                        if rows.len() >= MAX_ROWS {
                            return Ok(rows);
                        }
                    }
                }
                _ => {}
            },
            Event::Eof => return Ok(rows),
            _ => {}
        }
    }
}
