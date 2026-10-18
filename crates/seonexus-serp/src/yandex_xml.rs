//! The XML result format of the Yandex Search API.
//!
//! ```xml
//! <yandexsearch><response>
//!   <found priority="all">456</found>
//!   <results><grouping><group><doc>
//!     <url>https://example.com/</url><title>Example <hlword>title</hlword></title>
//!     <headline>...</headline><passages><passage>...</passage></passages>
//!     <modtime>20240101T000000</modtime>
//!   </doc></group></grouping></results>
//! </response></yandexsearch>
//! ```
//!
//! `<error code="15">` means the search found nothing; other error codes are failures.

use std::collections::HashSet;

use quick_xml::Reader;
use quick_xml::events::{BytesRef, Event};
use url::Url;

use crate::mapping::{Coverage, ParsedPage};
use crate::text::{non_empty, squash};
use crate::{Organic, SerpError, registrable_domain};

/// Profile key recorded with snapshots parsed from Yandex XML.
pub const PARSER_REV: &str = "yandex-xml@1";

/// Yandex's "nothing found" error code.
const NO_RESULTS: &str = "15";

#[derive(Default)]
struct Doc {
    url: String,
    title: String,
    headline: String,
    passages: Vec<String>,
    modtime: String,
}

/// Reads result page `page` (from 1) of an XML response.
///
/// # Errors
/// [`SerpError::Parse`] for malformed XML or an `<error>` other than "no results".
pub fn parse(xml: &str, page: u16) -> Result<ParsedPage, SerpError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().expand_empty_elements = true;
    let mut reading = Reading::default();
    loop {
        let event = reader
            .read_event()
            .map_err(|error| SerpError::Parse(format!("invalid Yandex XML: {error}")))?;
        match event {
            Event::Start(element) => {
                let name = element.local_name().as_ref().to_owned();
                let attribute = |wanted: &str| {
                    element
                        .attributes()
                        .flatten()
                        .find(|attribute| attribute.key.as_ref() == wanted)
                        .map(|attribute| attribute.value.into_owned())
                };
                reading.start(name, attribute("priority").as_deref(), attribute("code"));
            }
            Event::End(_) => reading.end(),
            Event::Text(text) => reading.text(&text.xml10_content()),
            Event::CData(data) => reading.text(data.as_ref()),
            Event::GeneralRef(reference) => reading.text(&entity(&reference).to_string()),
            Event::Eof => break,
            Event::Empty(_)
            | Event::Comment(_)
            | Event::Decl(_)
            | Event::PI(_)
            | Event::DocType(_) => {}
        }
    }
    reading.finish(page)
}

/// State while reading a response.
#[derive(Default)]
struct Reading {
    path: Vec<String>,
    docs: Vec<Doc>,
    current: Option<Doc>,
    total: Option<u64>,
    in_total: bool,
    error_code: Option<String>,
    error_text: String,
    in_error: bool,
}

impl Reading {
    fn start(&mut self, name: String, priority: Option<&str>, code: Option<String>) {
        match name.as_str() {
            "doc" => self.current = Some(Doc::default()),
            "passage" => {
                if let Some(doc) = self.current.as_mut() {
                    doc.passages.push(String::new());
                }
            }
            "found" if self.path.last().is_some_and(|parent| parent == "response") => {
                self.in_total = priority == Some("all");
            }
            "error" => {
                self.in_error = true;
                self.error_code = code;
            }
            _ => {}
        }
        self.path.push(name);
    }

    fn end(&mut self) {
        if let Some(name) = self.path.pop() {
            match name.as_str() {
                "doc" => self.docs.extend(self.current.take()),
                "error" => self.in_error = false,
                "found" => self.in_total = false,
                _ => {}
            }
        }
    }

    fn text(&mut self, text: &str) {
        if self.in_error {
            self.error_text.push_str(text);
            return;
        }
        if self.in_total && self.total.is_none() {
            self.total = text.trim().parse().ok();
            return;
        }
        let Some(doc) = self.current.as_mut() else {
            return;
        };
        // The innermost field of the doc the text belongs to; `hlword` highlights are inline.
        let field = self
            .path
            .iter()
            .rev()
            .find(|name| name.as_str() != "hlword")
            .map(String::as_str);
        match field {
            Some("url") => doc.url.push_str(text),
            Some("title") => doc.title.push_str(text),
            Some("headline") => doc.headline.push_str(text),
            Some("passage") => {
                if let Some(last) = doc.passages.last_mut() {
                    last.push_str(text);
                }
            }
            Some("modtime") => doc.modtime.push_str(text),
            _ => {}
        }
    }

    fn finish(self, page: u16) -> Result<ParsedPage, SerpError> {
        if let Some(code) = &self.error_code
            && code != NO_RESULTS
        {
            return Err(SerpError::Parse(format!(
                "Yandex error {code}: {}",
                squash(&self.error_text)
            )));
        }
        let matched = self.docs.len();
        let mut seen = HashSet::new();
        let mut organic = Vec::new();
        for doc in self.docs {
            let Ok(url) = Url::parse(doc.url.trim()) else {
                continue;
            };
            let (Some(host), Some(title)) = (url.host_str(), non_empty(&doc.title)) else {
                continue;
            };
            if !matches!(url.scheme(), "http" | "https") || !seen.insert(url.to_string()) {
                continue;
            }
            let host = host.trim_end_matches('.').to_ascii_lowercase();
            let snippet = non_empty(&doc.passages.join(" ")).or_else(|| non_empty(&doc.headline));
            organic.push(Organic {
                position: u16::try_from(organic.len() + 1).unwrap_or(u16::MAX),
                absolute_position: None,
                page,
                domain: registrable_domain(&host),
                host,
                url: url.to_string(),
                title,
                snippet,
                displayed_url: None,
                date: non_empty(&doc.modtime),
                site_only: false,
                sitelinks: Vec::new(),
            });
        }
        let usable = organic.len();
        Ok(ParsedPage {
            page,
            organic,
            features: Vec::new(),
            total_results: self.total,
            error: self.error_code.map(|_| squash(&self.error_text)),
            challenge: None,
            coverage: Coverage { matched, usable },
        })
    }
}

fn entity(reference: &BytesRef<'_>) -> char {
    if let Ok(Some(character)) = reference.resolve_char_ref() {
        return character;
    }
    match reference.as_ref() {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "apos" => '\'',
        "quot" => '"',
        _ => '\u{fffd}',
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RESPONSE: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<yandexsearch version="1.0"><request><query>kahve</query></request>
<response date="20261018T101010">
  <found priority="phrase">90</found>
  <found priority="all">1234</found>
  <results><grouping attr="d" mode="deep" groups-on-page="10" docs-in-group="1">
    <group><doc id="a">
      <url>https://www.example.com.tr/kahve</url>
      <domain>www.example.com.tr</domain>
      <title>Taze <hlword>kahve</hlword> &amp; çekirdek</title>
      <headline>Manşet</headline>
      <modtime>20260101T000000</modtime>
      <passages><passage>Türk <hlword>kahvesi</hlword> nasıl</passage><passage>yapılır</passage></passages>
    </doc></group>
    <group><doc id="b">
      <url>https://kahve.example.org/</url><title>İkinci</title><headline>Sadece manşet</headline>
    </doc></group>
    <group><doc id="c"><url>not a url</url><title>Bozuk</title></doc></group>
  </grouping></results>
</response></yandexsearch>"#;

    #[test]
    fn reads_documents_counts_and_highlights() {
        let page = parse(RESPONSE, 1).unwrap();
        assert_eq!(page.total_results, Some(1234));
        assert_eq!(
            page.coverage,
            Coverage {
                matched: 3,
                usable: 2
            }
        );
        let first = &page.organic[0];
        assert_eq!(first.title, "Taze kahve & çekirdek");
        assert_eq!(first.domain, "example.com.tr");
        assert_eq!(first.snippet.as_deref(), Some("Türk kahvesi nasıl yapılır"));
        assert_eq!(first.date.as_deref(), Some("20260101T000000"));
        assert_eq!(page.organic[1].snippet.as_deref(), Some("Sadece manşet"));
        assert_eq!(page.organic[1].position, 2);
    }

    #[test]
    fn treats_code_15_as_no_results_and_others_as_errors() {
        let empty = r#"<yandexsearch><response><error code="15">Sorry, there are no results</error></response></yandexsearch>"#;
        let page = parse(empty, 1).unwrap();
        assert_eq!(page.organic.len(), 0);
        assert!(page.error.is_some());

        let limit = r#"<yandexsearch><response><error code="32">Limit exceeded</error></response></yandexsearch>"#;
        let error = parse(limit, 1).unwrap_err().to_string();
        assert!(error.contains("32") && error.contains("Limit exceeded"));
        assert!(parse("<yandexsearch><response>", 1).is_err() || parse("<a></b>", 1).is_err());
    }
}
