//! Bounded XLSX cached-cell reader using the repository's ZIP 9 parser.
//! Never executes formulas, macros, external relationships or embedded content.
use super::files::{ImportCell, ROW_LIMIT};
use quick_xml::{Reader, events::Event};
use std::collections::BTreeMap;
use std::io::{Cursor, Read};

fn part(archive: &mut zip::ZipArchive<Cursor<&[u8]>>, path: &str) -> Result<Vec<u8>, String> {
    let entry = archive.by_name(path).map_err(|e| e.to_string())?;
    if entry.size() > 64 * 1024 * 1024 {
        return Err("Expanded XLSX part exceeds 64 MiB".into());
    }
    let mut bytes = Vec::new();
    entry
        .take(64 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 64 * 1024 * 1024 {
        return Err("Expanded XLSX part exceeds 64 MiB".into());
    }
    Ok(bytes)
}
fn attributes(element: &quick_xml::events::BytesStart<'_>) -> BTreeMap<String, String> {
    element
        .attributes()
        .flatten()
        .filter_map(|a| {
            Some((
                a.key.as_ref().to_owned(),
                a.normalized_value(quick_xml::XmlVersion::Implicit1_0)
                    .ok()?
                    .into_owned(),
            ))
        })
        .collect()
}
fn content(text: &quick_xml::events::BytesText<'_>) -> Result<String, String> {
    let decoded = text.xml10_content();
    quick_xml::escape::unescape(&decoded)
        .map(std::borrow::Cow::into_owned)
        .map_err(|e| e.to_string())
}

pub(crate) fn read(
    bytes: &[u8],
    selected: Option<&str>,
) -> Result<(Vec<String>, Vec<Vec<ImportCell>>), String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    if archive.len() > 10_000 {
        return Err("XLSX contains too many parts".into());
    }
    let mut total = 0_u64;
    for index in 0..archive.len() {
        total = total.saturating_add(archive.by_index(index).map_err(|e| e.to_string())?.size());
    }
    if total > 128 * 1024 * 1024 {
        return Err("Expanded XLSX exceeds 128 MiB".into());
    }
    let workbook = part(&mut archive, "xl/workbook.xml")?;
    let mut reader = Reader::from_reader(workbook.as_slice());
    let mut sheets = Vec::new();
    loop {
        match reader.read_event().map_err(|e| e.to_string())? {
            Event::Start(e) | Event::Empty(e)
                if e.local_name().as_ref() == "workbookPr"
                    && attributes(&e)
                        .get("date1904")
                        .is_some_and(|v| v == "1" || v == "true") =>
            {
                return Err(
                    "Excel 1904 date system is unsupported; export dates as YYYY-MM-DD or CSV"
                        .into(),
                );
            }
            Event::Start(e) | Event::Empty(e) if e.local_name().as_ref() == "sheet" => {
                let a = attributes(&e);
                if let (Some(name), Some(id)) = (a.get("name"), a.get("r:id")) {
                    sheets.push((name.clone(), id.clone()));
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    let chosen = sheets
        .iter()
        .find(|(name, _)| selected.is_some_and(|s| s == name))
        .or_else(|| selected.is_none().then(|| sheets.first()).flatten())
        .ok_or("Choose an existing worksheet")?;
    let relationships = part(&mut archive, "xl/_rels/workbook.xml.rels")?;
    let mut reader = Reader::from_reader(relationships.as_slice());
    let mut target = None;
    loop {
        match reader.read_event().map_err(|e| e.to_string())? {
            Event::Start(e) | Event::Empty(e) if e.local_name().as_ref() == "Relationship" => {
                let a = attributes(&e);
                if a.get("Id") == Some(&chosen.1)
                    && a.get("TargetMode").is_none_or(|m| m != "External")
                {
                    target = a.get("Target").cloned();
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    let target = target.ok_or("Worksheet relationship is missing")?;
    if target.contains("..") || target.contains(':') || target.contains('\\') {
        return Err("Unsupported worksheet path".into());
    }
    let target = if target.starts_with("/xl/") {
        target.trim_start_matches('/').into()
    } else if target.starts_with("xl/") {
        target
    } else {
        format!("xl/{target}")
    };
    let strings = if archive
        .file_names()
        .flatten()
        .any(|name| name == "xl/sharedStrings.xml")
    {
        shared_strings(&part(&mut archive, "xl/sharedStrings.xml")?)?
    } else {
        Vec::new()
    };
    let rows = sheet_rows(&part(&mut archive, &target)?, &strings)?;
    Ok((sheets.into_iter().map(|(name, _)| name).collect(), rows))
}
fn shared_strings(bytes: &[u8]) -> Result<Vec<String>, String> {
    let mut reader = Reader::from_reader(bytes);
    let (mut strings, mut current, mut in_text) = (Vec::new(), String::new(), false);
    loop {
        match reader.read_event().map_err(|e| e.to_string())? {
            Event::Start(e) if e.local_name().as_ref() == "si" => current.clear(),
            Event::Start(e) if e.local_name().as_ref() == "t" => in_text = true,
            Event::End(e) if e.local_name().as_ref() == "t" => in_text = false,
            Event::Text(t) if in_text => current.push_str(&content(&t)?),
            Event::GeneralRef(reference) if in_text => current.push_str(&entity(&reference)?),
            Event::End(e) if e.local_name().as_ref() == "si" => {
                if strings.len() > 1_000_000 {
                    return Err("Too many XLSX strings".into());
                }
                strings.push(current.clone());
            }
            Event::DocType(_) => return Err("XLSX document types are unsupported".into()),
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(strings)
}
fn column(reference: &str) -> usize {
    reference
        .bytes()
        .take_while(u8::is_ascii_alphabetic)
        .fold(0_usize, |n, b| {
            n.saturating_mul(26)
                .saturating_add(usize::from(b.to_ascii_uppercase() - b'A' + 1))
        })
        .saturating_sub(1)
}
fn sheet_rows(bytes: &[u8], strings: &[String]) -> Result<Vec<Vec<ImportCell>>, String> {
    let mut reader = Reader::from_reader(bytes);
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut text = String::new();
    let mut kind = String::new();
    let mut index = 0;
    let mut capture = false;
    loop {
        match reader.read_event().map_err(|e| e.to_string())? {
            Event::Start(e) if e.local_name().as_ref() == "row" => row.clear(),
            Event::Start(e) if e.local_name().as_ref() == "c" => {
                let a = attributes(&e);
                index = a.get("r").map_or(row.len(), |s| column(s));
                if index >= 128 {
                    return Err("XLSX exceeds 128 columns".into());
                }
                kind = a.get("t").cloned().unwrap_or_default();
                text.clear();
            }
            Event::Start(e) if matches!(e.local_name().as_ref(), "v" | "t") => capture = true,
            Event::End(e) if matches!(e.local_name().as_ref(), "v" | "t") => capture = false,
            Event::Text(t) if capture => text.push_str(&content(&t)?),
            Event::GeneralRef(reference) if capture => text.push_str(&entity(&reference)?),
            Event::End(e) if e.local_name().as_ref() == "c" => {
                let value = if kind == "s" {
                    let at = text
                        .parse::<usize>()
                        .map_err(|_| "Invalid shared string index")?;
                    strings.get(at).ok_or("Missing shared string")?.clone()
                } else {
                    text.clone()
                };
                while row.len() <= index {
                    row.push(ImportCell {
                        text: String::new(),
                        numeric: false,
                    });
                }
                row[index] = ImportCell {
                    text: value,
                    numeric: kind.is_empty() || kind == "n",
                };
            }
            Event::End(e) if e.local_name().as_ref() == "row" => {
                if row.iter().any(|c| !c.text.is_empty()) {
                    if rows.len() > ROW_LIMIT + 30 {
                        return Err("XLSX exceeds 100,000 rows".into());
                    }
                    rows.push(row.clone());
                }
            }
            Event::DocType(_) => return Err("XLSX document types are unsupported".into()),
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(rows)
}

fn entity(reference: &quick_xml::events::BytesRef<'_>) -> Result<String, String> {
    quick_xml::escape::unescape(&format!("&{};", reference.xml10_content()))
        .map(std::borrow::Cow::into_owned)
        .map_err(|e| e.to_string())
}
