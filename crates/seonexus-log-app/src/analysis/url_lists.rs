//! Reading URL lists: plain text, CSV exports of crawlers, XML sitemaps and sitemap indexes,
//! from files or the web, optionally gzip-compressed.

use std::fmt;
use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;

use flate2::read::MultiGzDecoder;
use seonexus_net::HttpClient;

use super::AnalysisError;
use crate::USER_AGENT;

/// Sitemaps referenced by sitemap indexes that are followed at most.
const MAX_SITEMAPS: usize = 1_000;
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(60);

/// Where a URL list comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UrlSource {
    /// A local file.
    File(PathBuf),
    /// A web address (usually a sitemap).
    Remote(String),
}

impl UrlSource {
    /// Interprets a command-line argument: `http(s)://…` is remote, anything else a file.
    pub fn parse(argument: &str) -> Self {
        let lower = argument.to_ascii_lowercase();
        if lower.starts_with("http://") || lower.starts_with("https://") {
            Self::Remote(argument.to_owned())
        } else {
            Self::File(PathBuf::from(argument))
        }
    }
}

impl fmt::Display for UrlSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::File(path) => write!(f, "{}", path.display()),
            Self::Remote(url) => f.write_str(url),
        }
    }
}

/// URLs of a list plus the other columns of crawler exports (and `lastmod` of sitemaps).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct UrlList {
    pub(crate) urls: Vec<String>,
    /// Names of the extra columns.
    pub(crate) columns: Vec<String>,
    /// Extra values per URL, aligned with `columns`.
    pub(crate) values: Vec<Vec<String>>,
}

impl UrlList {
    fn extend(&mut self, other: Self) {
        if self.columns.is_empty() {
            self.columns = other.columns;
        }
        self.urls.extend(other.urls);
        self.values.extend(other.values);
    }
}

/// Reads every URL of `source`, following sitemap indexes.
#[cfg(test)]
pub(crate) fn read_urls(source: &UrlSource) -> Result<Vec<String>, AnalysisError> {
    Ok(read_list(source)?.urls)
}

/// Reads every URL of `source` with its extra columns, following sitemap indexes.
pub(crate) fn read_list(source: &UrlSource) -> Result<UrlList, AnalysisError> {
    let client = HttpClient::new(USER_AGENT, DOWNLOAD_TIMEOUT);
    let mut list = UrlList::default();
    let mut pending = vec![source.clone()];
    let mut fetched = 0;
    while let Some(next) = pending.pop() {
        fetched += 1;
        if fetched > MAX_SITEMAPS {
            tracing::warn!(
                limit = MAX_SITEMAPS,
                "sitemap index too large; rest ignored"
            );
            break;
        }
        let text = load_text(&next, &client)?;
        match parse_list(&text) {
            Parsed::Urls(found) => list.extend(found),
            Parsed::Sitemaps(children) => {
                pending.extend(children.into_iter().map(|child| UrlSource::parse(&child)));
            }
        }
    }
    Ok(list)
}

fn load_text(source: &UrlSource, client: &HttpClient) -> Result<String, AnalysisError> {
    let bytes = match source {
        UrlSource::File(path) => std::fs::read(path).map_err(|error| AnalysisError::Read {
            list: source.to_string(),
            message: error.to_string(),
        })?,
        UrlSource::Remote(url) => client.get_bytes(url).map_err(|error| AnalysisError::Read {
            list: source.to_string(),
            message: error.to_string(),
        })?,
    };
    let bytes = if bytes.starts_with(&[0x1f, 0x8b]) {
        let mut decoded = Vec::new();
        MultiGzDecoder::new(bytes.as_slice())
            .read_to_end(&mut decoded)
            .map_err(|error| AnalysisError::Read {
                list: source.to_string(),
                message: error.to_string(),
            })?;
        decoded
    } else {
        bytes
    };
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

#[derive(Debug, PartialEq, Eq)]
enum Parsed {
    Urls(UrlList),
    Sitemaps(Vec<String>),
}

fn parse_list(text: &str) -> Parsed {
    let text = text.trim_start_matches('\u{feff}');
    if text.contains("<sitemapindex") {
        return Parsed::Sitemaps(
            xml_entries(text, "sitemap")
                .into_iter()
                .map(|(loc, _)| loc)
                .collect(),
        );
    }
    if text.contains("<urlset") {
        let entries = xml_entries(text, "url");
        let has_lastmod = entries.iter().any(|(_, lastmod)| lastmod.is_some());
        let (urls, lastmods): (Vec<String>, Vec<Option<String>>) = entries.into_iter().unzip();
        return Parsed::Urls(UrlList {
            urls,
            columns: if has_lastmod {
                vec!["lastmod".to_owned()]
            } else {
                Vec::new()
            },
            values: if has_lastmod {
                lastmods
                    .into_iter()
                    .map(|m| vec![m.unwrap_or_default()])
                    .collect()
            } else {
                Vec::new()
            },
        });
    }
    let mut lines = text.lines().filter(|line| !line.trim().is_empty());
    let Some(first) = lines.next() else {
        return Parsed::Urls(UrlList::default());
    };
    if let Some((separator, column)) = csv_url_column(first) {
        let header = split_csv(first, separator);
        let columns: Vec<String> = header
            .iter()
            .enumerate()
            .filter(|(index, name)| *index != column && !name.is_empty())
            .map(|(_, name)| name.clone())
            .collect();
        let kept: Vec<usize> = (0..header.len())
            .filter(|index| *index != column && !header[*index].is_empty())
            .collect();
        let mut list = UrlList {
            columns,
            ..UrlList::default()
        };
        for line in lines {
            let fields = split_csv(line, separator);
            let Some(url) = fields.get(column).filter(|value| !value.is_empty()) else {
                continue;
            };
            list.urls.push(url.clone());
            list.values.push(
                kept.iter()
                    .map(|&index| fields.get(index).cloned().unwrap_or_default())
                    .collect(),
            );
        }
        return Parsed::Urls(list);
    }
    Parsed::Urls(UrlList {
        urls: std::iter::once(first)
            .chain(lines)
            .map(str::trim)
            .filter(|line| !line.starts_with('#'))
            .map(str::to_owned)
            .collect(),
        ..UrlList::default()
    })
}

/// Separator and index of the URL column when `header` is a CSV/TSV header naming one
/// (`Address` in Screaming Frog exports, `URL`, `Loc`, …).
fn csv_url_column(header: &str) -> Option<(char, usize)> {
    [',', '\t', ';'].into_iter().find_map(|separator| {
        let fields = split_csv(header, separator);
        if fields.len() < 2 {
            return None;
        }
        fields
            .iter()
            .position(|field| {
                matches!(
                    field.to_ascii_lowercase().as_str(),
                    "address" | "url" | "urls" | "loc" | "page" | "landing page"
                )
            })
            .map(|column| (separator, column))
    })
}

/// Splits one CSV record, honouring double quotes.
fn split_csv(line: &str, separator: char) -> Vec<String> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            c if c == separator && !quoted => fields.push(std::mem::take(&mut field)),
            c => field.push(c),
        }
    }
    fields.push(field);
    fields.into_iter().map(|f| f.trim().to_owned()).collect()
}

/// `<loc>` and `<lastmod>` of every `<element>` (`url` or `sitemap`), entities decoded.
fn xml_entries(xml: &str, element: &str) -> Vec<(String, Option<String>)> {
    let open = format!("<{element}>");
    let close = format!("</{element}>");
    let mut entries = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find(&open) {
        rest = &rest[start + open.len()..];
        let end = rest.find(&close).unwrap_or(rest.len());
        let block = &rest[..end];
        if let Some(loc) = xml_value(block, "loc") {
            entries.push((loc, xml_value(block, "lastmod")));
        }
        rest = &rest[end..];
    }
    entries
}

/// Text of the first `<tag>` in `block`, CDATA unwrapped and entities decoded.
fn xml_value(block: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let start = block.find(&open)? + open.len();
    let end = start + block[start..].find(&format!("</{tag}>"))?;
    let value = block[start..end].trim();
    let value = value
        .strip_prefix("<![CDATA[")
        .and_then(|v| v.strip_suffix("]]>"))
        .unwrap_or(value);
    Some(unescape_xml(value.trim())).filter(|v| !v.is_empty())
}

fn unescape_xml(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_sitemaps_and_indexes() {
        let urlset = r#"<?xml version="1.0"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
            <url><loc>https://example.com/a?x=1&amp;y=2</loc><lastmod>2026-10-01</lastmod></url>
            <url><loc> <![CDATA[https://example.com/b]]> </loc></url></urlset>"#;
        assert_eq!(
            parse_list(urlset),
            Parsed::Urls(UrlList {
                urls: vec![
                    "https://example.com/a?x=1&y=2".into(),
                    "https://example.com/b".into()
                ],
                columns: vec!["lastmod".into()],
                values: vec![vec!["2026-10-01".into()], vec![String::new()]],
            })
        );
        let index = "<sitemapindex><sitemap><loc>https://example.com/s1.xml.gz</loc></sitemap></sitemapindex>";
        assert_eq!(
            parse_list(index),
            Parsed::Sitemaps(vec!["https://example.com/s1.xml.gz".into()])
        );
    }

    #[test]
    fn reads_crawler_csv_exports_and_plain_lists() {
        let csv = "\u{feff}\"Address\",\"Content Type\",\"Status Code\"\n\
                   \"https://example.com/\",\"text/html\",\"200\"\n\
                   \"https://example.com/a,b\",\"text/html\",\"200\"\n";
        assert_eq!(
            parse_list(csv),
            Parsed::Urls(UrlList {
                urls: vec![
                    "https://example.com/".into(),
                    "https://example.com/a,b".into()
                ],
                columns: vec!["Content Type".into(), "Status Code".into()],
                values: vec![
                    vec!["text/html".into(), "200".into()],
                    vec!["text/html".into(), "200".into()]
                ],
            })
        );
        let tsv = "Status\tURL\n200\t/x\n";
        let Parsed::Urls(list) = parse_list(tsv) else {
            panic!("expected URLs");
        };
        assert_eq!(
            (list.urls, list.columns),
            (vec!["/x".to_owned()], vec!["Status".to_owned()])
        );
        let plain = "# exported\n/a\n\n https://example.com/b \n";
        assert_eq!(
            parse_list(plain),
            Parsed::Urls(UrlList {
                urls: vec!["/a".into(), "https://example.com/b".into()],
                ..UrlList::default()
            })
        );
    }

    #[test]
    fn reads_gzip_files() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("urls.txt.gz");
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(b"/a\n/b\n").unwrap();
        std::fs::write(&path, encoder.finish().unwrap()).unwrap();
        let urls = read_urls(&UrlSource::File(path)).unwrap();
        assert_eq!(urls, ["/a", "/b"]);
        assert!(matches!(
            read_urls(&UrlSource::File(dir.path().join("missing.txt"))),
            Err(AnalysisError::Read { .. })
        ));
        assert_eq!(
            UrlSource::parse("HTTPS://example.com/sitemap.xml"),
            UrlSource::Remote("HTTPS://example.com/sitemap.xml".into())
        );
    }
}
