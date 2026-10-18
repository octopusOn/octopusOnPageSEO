//! Structural sitemap XML parsing. Extension vocabularies never become page URLs.

use quick_xml::events::{BytesRef, BytesStart, Event};
use quick_xml::name::ResolveResult;
use quick_xml::{NsReader, XmlVersion};

use super::{ParsedSitemap, SitemapEntry, SitemapKind};

const NAMESPACE: &str = "http://www.sitemaps.org/schemas/sitemap/0.9";
const MAX_DEPTH: usize = 128;

pub(super) fn parse(text: &str) -> Result<ParsedSitemap, String> {
    if !text.chars().all(xml_char) {
        return Err("invalid XML character in sitemap".into());
    }
    let mut reader = NsReader::from_str(text);
    reader.config_mut().expand_empty_elements = true;
    reader.config_mut().check_comments = true;
    let mut document = Document::default();
    let mut declared = false;
    loop {
        let (namespace, event) = reader.read_resolved_event().map_err(xml_error)?;
        let namespace = match namespace {
            ResolveResult::Unbound => Some(false),
            ResolveResult::Bound(namespace) => (namespace.as_ref() == NAMESPACE).then_some(true),
            ResolveResult::Unknown(prefix) => {
                return Err(format!("unknown XML namespace prefix `{prefix}`"));
            }
        };
        match event {
            Event::Start(element) => {
                attributes(&element, &reader)?;
                document.start(namespace, element.local_name().as_ref())?;
            }
            Event::End(_) => document.end()?,
            Event::Text(value) => document.text(&value.xml10_content())?,
            Event::CData(value) => {
                if document.depth == 0 {
                    return Err("CDATA outside sitemap root".into());
                }
                document.text(&value.xml10_content())?;
            }
            Event::GeneralRef(value) => document.text(&reference(&value)?.to_string())?,
            Event::Decl(declaration) => {
                if declared || document.parsed.is_some() {
                    return Err("misplaced XML declaration".into());
                }
                declared = true;
                if declaration.version().map_err(xml_error)? != "1.0" {
                    return Err("sitemap requires XML 1.0".into());
                }
                if let Some(encoding) = declaration.encoding()
                    && !encoding.map_err(xml_error)?.eq_ignore_ascii_case("utf-8")
                {
                    return Err("sitemap requires UTF-8 encoding".into());
                }
            }
            Event::DocType(_) => return Err("DOCTYPE is not supported in sitemaps".into()),
            Event::Eof => return document.finish(),
            Event::Comment(_) | Event::PI(_) => {}
            Event::Empty(_) => unreachable!("empty XML elements are expanded"),
        }
    }
}

fn attributes(element: &BytesStart<'_>, reader: &NsReader<&[u8]>) -> Result<(), String> {
    for attribute in element.attributes() {
        let attribute = attribute.map_err(xml_error)?;
        if let (ResolveResult::Unknown(prefix), _) =
            reader.resolver().resolve_attribute(attribute.key)
        {
            return Err(format!("unknown XML attribute prefix `{prefix}`"));
        }
        let value = attribute
            .normalized_value(XmlVersion::Implicit1_0)
            .map_err(xml_error)?;
        if !value.chars().all(xml_char) {
            return Err("invalid XML character in sitemap attribute".into());
        }
    }
    Ok(())
}

fn reference(value: &BytesRef<'_>) -> Result<char, String> {
    let character = if let Some(character) = value.resolve_char_ref().map_err(xml_error)? {
        character
    } else {
        match value.as_ref() {
            "amp" => '&',
            "lt" => '<',
            "gt" => '>',
            "apos" => '\'',
            "quot" => '"',
            other => return Err(format!("unknown XML entity `&{other};`")),
        }
    };
    if !xml_char(character) {
        return Err("invalid XML character reference".into());
    }
    Ok(character)
}

fn xml_char(value: char) -> bool {
    matches!(value, '\t' | '\n' | '\r' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}')
}

fn xml_error(error: impl std::fmt::Display) -> String {
    format!("invalid sitemap XML: {error}")
}

#[derive(Default)]
struct Document {
    parsed: Option<ParsedSitemap>,
    /// Some(false) preserves legacy unnamespaced sitemap support.
    namespace: Option<bool>,
    depth: usize,
    closed: bool,
    entry: Option<Entry>,
    capture: Option<(Field, String)>,
}

#[derive(Default)]
struct Entry {
    loc: Option<String>,
    lastmod: Option<String>,
}

enum Field {
    Loc,
    Lastmod,
}

impl Document {
    fn start(&mut self, namespace: Option<bool>, name: &str) -> Result<(), String> {
        if self.closed {
            return Err("multiple sitemap root elements".into());
        }
        if self.capture.is_some() {
            return Err("sitemap loc/lastmod must contain text, not nested elements".into());
        }
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err("sitemap XML nesting limit exceeded".into());
        }
        if self.depth == 1 {
            let kind = match (namespace, name) {
                (Some(_), "urlset") => SitemapKind::UrlSet,
                (Some(_), "sitemapindex") => SitemapKind::Index,
                _ => return Err("not a sitemap: expected urlset or sitemapindex root".into()),
            };
            self.namespace = namespace;
            self.parsed = Some(ParsedSitemap {
                kind,
                entries: Vec::new(),
                children: Vec::new(),
            });
        } else if namespace == self.namespace {
            let kind = self.parsed.as_ref().map(|parsed| parsed.kind);
            if self.depth == 2
                && matches!(
                    (kind, name),
                    (Some(SitemapKind::UrlSet), "url") | (Some(SitemapKind::Index), "sitemap")
                )
            {
                self.entry = Some(Entry::default());
            } else if self.depth == 3 && self.entry.is_some() {
                let field = match name {
                    "loc" => Some(Field::Loc),
                    "lastmod" => Some(Field::Lastmod),
                    _ => None,
                };
                self.capture = field.map(|field| (field, String::new()));
            }
        }
        Ok(())
    }

    fn text(&mut self, text: &str) -> Result<(), String> {
        if self.depth == 0
            && !text
                .chars()
                .all(|value| matches!(value, ' ' | '\t' | '\r' | '\n'))
        {
            return Err("text outside sitemap root".into());
        }
        if let Some((_, value)) = self.capture.as_mut() {
            value.push_str(text);
        }
        Ok(())
    }

    fn end(&mut self) -> Result<(), String> {
        if self.depth == 0 {
            return Err("unexpected sitemap closing tag".into());
        }
        if self.depth == 3
            && let Some((field, value)) = self.capture.take()
        {
            let entry = self
                .entry
                .as_mut()
                .ok_or("sitemap field without an entry")?;
            let destination = match field {
                Field::Loc => &mut entry.loc,
                Field::Lastmod => &mut entry.lastmod,
            };
            if destination.is_none() {
                *destination = Some(value.trim().to_owned());
            }
        }
        if self.depth == 2
            && let Some(entry) = self.entry.take()
        {
            let parsed = self.parsed.as_mut().ok_or("sitemap entry without a root")?;
            if let Some(url) = entry.loc.filter(|url| !url.is_empty()) {
                match parsed.kind {
                    SitemapKind::UrlSet => parsed.entries.push(SitemapEntry {
                        url,
                        lastmod: entry.lastmod,
                    }),
                    SitemapKind::Index => parsed.children.push(url),
                    SitemapKind::Text => unreachable!("XML root cannot be text format"),
                }
            }
        }
        self.depth -= 1;
        self.closed = self.depth == 0;
        Ok(())
    }

    fn finish(self) -> Result<ParsedSitemap, String> {
        if self.depth != 0 || !self.closed {
            return Err("incomplete sitemap XML document".into());
        }
        self.parsed
            .ok_or_else(|| "not a sitemap: missing XML root".into())
    }
}
