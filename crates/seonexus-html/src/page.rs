//! Extraction of a page's SEO facts.

use std::sync::OnceLock;

use ego_tree::NodeRef;
use scraper::{ElementRef, Html, Node, Selector};
use serde::Serialize;
use url::{Position, Url};

use crate::directives::{Directives, parse_directives};
use crate::text::text_stats;

/// Elements whose text is not visible page content.
const HIDDEN: &[&str] = &[
    "script", "style", "noscript", "template", "svg", "head", "iframe", "object",
];

/// Elements that flow inside a line; every other element separates words.
const INLINE: &[&str] = &[
    "a", "abbr", "b", "bdi", "bdo", "cite", "code", "data", "dfn", "em", "font", "i", "kbd",
    "label", "mark", "q", "s", "samp", "small", "span", "strong", "sub", "sup", "time", "u", "var",
];

/// A compiled selector for a literal, built once per process.
macro_rules! sel {
    ($css:literal) => {{
        static SELECTOR: OnceLock<Selector> = OnceLock::new();
        SELECTOR.get_or_init(|| compile($css))
    }};
}

mod images;
mod link_names;
pub use link_names::LinkName;

/// What a crawler learns about one HTML page.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct PageFacts {
    /// Every `<title>` outside inline SVG (a page should have exactly one).
    pub titles: Vec<String>,
    /// Every `<meta name="description">`.
    pub meta_descriptions: Vec<String>,
    /// `<meta name="keywords">`.
    pub meta_keywords: Option<String>,
    /// Raw values of `<meta name="robots">` and of the crawling agent's own robots meta.
    pub meta_robots: Vec<String>,
    /// Directives of the robots meta tags that apply to the crawling agent.
    pub directives: Directives,
    /// Absolute targets of eligible head canonical elements, one entry per element.
    pub canonicals: Vec<String>,
    /// The first active `<base href>` declaration resolved against the page URL, even when
    /// its data/javascript scheme cannot serve as the effective document base.
    pub base_href: Option<String>,
    /// The `lang` attribute of `<html>`.
    pub lang: Option<String>,
    /// Active head `<link rel="alternate" hreflang>` annotations without `media`.
    pub hreflang: Vec<Hreflang>,
    /// `<link rel="next">` target.
    pub rel_next: Option<String>,
    /// `<link rel="prev">` target.
    pub rel_prev: Option<String>,
    /// `<meta http-equiv="refresh">`.
    pub meta_refresh: Option<MetaRefresh>,
    /// Headings in document order.
    pub headings: Vec<Heading>,
    /// Hyperlinks and resources the page references, in document order.
    pub links: Vec<Link>,
    /// HTTP image candidates from src, srcset and picture, with their owning img's attributes.
    pub images: Vec<Image>,
    /// JSON-LD, microdata and RDFa blocks.
    pub structured_data: Vec<StructuredData>,
    /// Open Graph and Twitter card tags.
    pub social: SocialTags,
    /// `<meta name="viewport">`.
    pub viewport: Option<String>,
    /// Charset declared in the document.
    pub charset: Option<String>,
    /// `<link rel="amphtml">` target.
    pub amp_html: Option<String>,
    /// Words of visible text.
    pub word_count: u32,
    /// Hash of the normalized visible text (equal for exact duplicates; 0 without text).
    pub text_hash: u64,
    /// SimHash of the visible text for near-duplicate detection.
    pub simhash: u64,
    /// Forms of an HTTPS page that submit to plain HTTP.
    pub insecure_forms: u32,
    /// HTTP resources (images, scripts, styles, frames, media) loaded by an HTTPS page.
    pub mixed_content: Vec<String>,
}

impl PageFacts {
    /// The first title.
    pub fn title(&self) -> Option<&str> {
        self.titles.first().map(String::as_str)
    }

    /// The first meta description.
    pub fn meta_description(&self) -> Option<&str> {
        self.meta_descriptions.first().map(String::as_str)
    }

    /// Texts of the headings of `level` (1–6).
    pub fn headings_of(&self, level: u8) -> impl Iterator<Item = &str> {
        self.headings
            .iter()
            .filter(move |h| h.level == level)
            .map(|h| h.text.as_str())
    }

    /// Hyperlinks (`<a>` and `<area>`), without the resources the page loads.
    pub fn hyperlinks(&self) -> impl Iterator<Item = &Link> {
        self.links.iter().filter(|link| link.kind.is_hyperlink())
    }
}

/// A heading element.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Heading {
    /// 1 for `<h1>` through 6 for `<h6>`.
    pub level: u8,
    /// Text with whitespace collapsed.
    pub text: String,
}

/// An hreflang annotation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Hreflang {
    /// Language (and optional region) code as written, or `x-default`.
    pub lang: String,
    /// Absolute URL of the alternate page.
    pub url: String,
}

/// A `<meta http-equiv="refresh">` redirect or reload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MetaRefresh {
    /// Seconds before the refresh.
    pub delay_seconds: u32,
    /// Absolute target; `None` reloads the page itself.
    pub url: Option<String>,
}

/// What kind of element references a URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LinkKind {
    /// `<a href>`: a hyperlink search engines follow.
    Anchor,
    /// `<area href>` in an image map.
    Area,
    /// `<img src>`, `<img srcset>` or a preceding `<picture><source srcset>`.
    Image,
    /// `<script src>`.
    Script,
    /// `<link rel="stylesheet">`.
    Stylesheet,
    /// `<iframe src>`.
    Iframe,
    /// `<video>`, `<audio>` or `<source>` sources.
    Media,
}

impl LinkKind {
    /// Hyperlinks, as opposed to resources the page loads.
    pub fn is_hyperlink(self) -> bool {
        matches!(self, Self::Anchor | Self::Area)
    }

    /// Lowercase name used in storage and exports.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Anchor => "anchor",
            Self::Area => "area",
            Self::Image => "image",
            Self::Script => "script",
            Self::Stylesheet => "stylesheet",
            Self::Iframe => "iframe",
            Self::Media => "media",
        }
    }
}

/// A URL referenced by the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct Link {
    /// Absolute http(s) URL without fragment.
    pub url: String,
    /// The referencing element.
    pub kind: LinkKind,
    /// Descriptive link name; visible text, image alt, explicit label or title fallback.
    /// Resource image references retain their alt text.
    pub anchor: String,
    /// Naming evidence for hyperlinks; absent for resource references.
    pub name: Option<LinkName>,
    /// `rel="nofollow"`.
    pub nofollow: bool,
    /// `rel="ugc"`.
    pub ugc: bool,
    /// `rel="sponsored"`.
    pub sponsored: bool,
}

/// A declared HTTP image candidate and its owning `<img>` attributes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Image {
    /// Absolute image URL.
    pub url: String,
    /// `None` when the `alt` attribute is missing; `Some("")` marks a decorative image.
    pub alt: Option<String>,
    /// Both effective width and height attributes are present; CSS layout is not inferred.
    pub has_dimensions: bool,
    /// `loading="lazy"`.
    pub lazy: bool,
}

/// Syntax of a structured data block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StructuredDataFormat {
    /// `<script type="application/ld+json">`.
    JsonLd,
    /// `itemscope` / `itemtype` attributes.
    Microdata,
    /// `typeof` attributes.
    Rdfa,
}

impl StructuredDataFormat {
    /// Lowercase name used in storage and exports.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::JsonLd => "jsonld",
            Self::Microdata => "microdata",
            Self::Rdfa => "rdfa",
        }
    }
}

/// One top-level block of structured data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StructuredData {
    /// Syntax of the block.
    pub format: StructuredDataFormat,
    /// schema.org types it declares (`Product`, `BreadcrumbList`, ...).
    pub types: Vec<String>,
    /// JSON-LD that does not parse is invalid; other formats are always valid here.
    pub valid: bool,
}

/// Open Graph and Twitter card tags.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct SocialTags {
    /// `og:title`.
    pub og_title: Option<String>,
    /// `og:description`.
    pub og_description: Option<String>,
    /// `og:image`, made absolute.
    pub og_image: Option<String>,
    /// `og:url`, made absolute.
    pub og_url: Option<String>,
    /// `og:type`.
    pub og_type: Option<String>,
    /// `twitter:card`.
    pub twitter_card: Option<String>,
}

/// Extracts the SEO facts of `html` served at `page_url`. `agents` are the lowercase robots
/// tokens of the crawling bot (for example `["googlebot"]`); their own robots meta tags apply
/// besides `<meta name="robots">`.
pub fn parse_page(html: &str, page_url: &Url, agents: &[&str]) -> PageFacts {
    let document = Html::parse_document(html);
    let mut facts = PageFacts::default();

    let base = active_elements(&document, sel!("base[href]"))
        .next()
        .and_then(|base| base.value().attr("href"))
        .and_then(|href| page_url.join(href.trim()).ok());
    facts.base_href = base.as_ref().map(ToString::to_string);
    let base = base
        .filter(|url| !matches!(url.scheme(), "data" | "javascript"))
        .unwrap_or_else(|| page_url.clone());
    let resolve = |href: &str| absolute(&base, href);
    let https = page_url.scheme() == "https";

    facts.lang = document
        .root_element()
        .value()
        .attr("lang")
        .map(str::trim)
        .filter(|lang| !lang.is_empty())
        .map(str::to_owned);
    facts.titles = active_elements(&document, sel!("title"))
        .filter(|title| !in_svg(*title))
        .map(active_text)
        .collect();

    read_meta(&document, &base, agents, &mut facts);
    read_link_elements(&document, &resolve, https, &mut facts);

    for heading in active_elements(&document, sel!("h1, h2, h3, h4, h5, h6")) {
        let level = heading
            .value()
            .name()
            .strip_prefix('h')
            .and_then(|n| n.parse().ok());
        facts.headings.push(Heading {
            level: level.unwrap_or(1),
            text: active_text(heading),
        });
    }

    read_anchors(
        &document,
        &|href| hyperlink_url(&base, page_url, href),
        &mut facts,
    );
    images::read(&document, &resolve, https, &mut facts);

    for (elements, kind) in [
        (
            active_elements(&document, sel!("script[src]")),
            LinkKind::Script,
        ),
        (
            active_elements(&document, sel!("iframe[src]")),
            LinkKind::Iframe,
        ),
        (
            active_elements(&document, sel!("video[src], audio[src], source[src]")),
            LinkKind::Media,
        ),
    ] {
        for element in elements {
            if let Some(url) = element.value().attr("src").and_then(resolve) {
                note_mixed(&mut facts, https, &url);
                facts.links.push(resource(url, kind));
            }
        }
    }

    if https {
        let insecure = active_elements(&document, sel!("form[action]"))
            .filter_map(|form| form.value().attr("action").and_then(resolve))
            .filter(|action| action.starts_with("http:"))
            .count();
        facts.insecure_forms = u32::try_from(insecure).unwrap_or(u32::MAX);
    }

    facts.structured_data = structured_data(&document);

    let mut text = String::new();
    if let Some(body) = active_elements(&document, sel!("body")).next() {
        visible_text(*body, &mut text);
    }
    (facts.word_count, facts.text_hash, facts.simhash) = text_stats(&text);
    facts
}

/// `<a>` and `<area>` hyperlinks with their anchor text and `rel` values.
fn read_anchors(document: &Html, resolve: &impl Fn(&str) -> Option<String>, facts: &mut PageFacts) {
    let names = link_names::Names::new(document);
    for anchor in active_elements(document, sel!("a[href], area[href]")) {
        let element = anchor.value();
        let Some(url) = element.attr("href").and_then(resolve) else {
            continue;
        };
        let rel = element.attr("rel").unwrap_or("").to_ascii_lowercase();
        let has_rel = |token: &str| rel.split_ascii_whitespace().any(|r| r == token);
        let name = names.link(anchor);
        facts.links.push(Link {
            url,
            kind: if element.name() == "area" {
                LinkKind::Area
            } else {
                LinkKind::Anchor
            },
            anchor: name.name.clone(),
            name: Some(name),
            nofollow: has_rel("nofollow"),
            ugc: has_rel("ugc"),
            sponsored: has_rel("sponsored"),
        });
    }
}

/// `<meta>` elements: description, robots, viewport, charset, refresh and social tags.
fn read_meta(document: &Html, base: &Url, agents: &[&str], facts: &mut PageFacts) {
    for meta in active_elements(document, sel!("meta")) {
        let element = meta.value();
        let content = element.attr("content").map(str::trim);
        if let (Some(name), Some(content)) = (element.attr("name"), content) {
            let name = name.trim().to_ascii_lowercase();
            match name.as_str() {
                "description" => facts.meta_descriptions.push(content.to_owned()),
                "keywords" => facts.meta_keywords = Some(content.to_owned()),
                "viewport" => facts.viewport = Some(content.to_owned()),
                "twitter:card" => facts.social.twitter_card = Some(content.to_owned()),
                name if name == "robots" || agents.contains(&name) => {
                    facts.meta_robots.push(content.to_owned());
                    facts.directives.merge(&parse_directives(content, &[]));
                }
                _ => {}
            }
        }
        if let (Some(property), Some(content)) = (element.attr("property"), content) {
            let social = &mut facts.social;
            match property.trim().to_ascii_lowercase().as_str() {
                "og:title" => social.og_title = Some(content.to_owned()),
                "og:description" => social.og_description = Some(content.to_owned()),
                "og:image" => social.og_image = content_url(base, content),
                "og:url" => social.og_url = content_url(base, content),
                "og:type" => social.og_type = Some(content.to_owned()),
                _ => {}
            }
        }
        if let Some(charset) = element.attr("charset") {
            facts.charset = Some(charset.trim().to_owned());
        }
        if let (Some(equiv), Some(content)) = (element.attr("http-equiv"), content) {
            match equiv.trim().to_ascii_lowercase().as_str() {
                "refresh" => facts.meta_refresh = Some(parse_refresh(content, base)),
                "content-type" if facts.charset.is_none() => {
                    facts.charset = content
                        .split(';')
                        .find_map(|part| part.trim().strip_prefix("charset="))
                        .map(|charset| charset.trim().to_owned());
                }
                _ => {}
            }
        }
    }
}

/// `<link>` elements: canonical, hreflang, pagination, AMP and stylesheets.
fn read_link_elements(
    document: &Html,
    resolve: &impl Fn(&str) -> Option<String>,
    https: bool,
    facts: &mut PageFacts,
) {
    for link in active_elements(document, sel!("link[href]")) {
        let element = link.value();
        let Some(url) = element.attr("href").and_then(resolve) else {
            continue;
        };
        let rel = element.attr("rel").unwrap_or("").to_ascii_lowercase();
        // Canonical annotations in the body, inert templates or carrying alternate-version
        // attributes are not canonical signals. Other relations/resources remain independent.
        let in_head = link
            .parent()
            .and_then(ElementRef::wrap)
            .is_some_and(|parent| parent.value().name() == "head");
        if in_head
            && rel
                .split_ascii_whitespace()
                .any(|token| token == "canonical")
            && ["hreflang", "lang", "media", "type"]
                .iter()
                .all(|attribute| element.attr(attribute).is_none())
        {
            facts.canonicals.push(url.clone());
        }
        if in_head
            && element.attr("media").is_none()
            && rel
                .split_ascii_whitespace()
                .any(|token| token == "alternate")
            && let Some(lang) = element.attr("hreflang")
        {
            facts.hreflang.push(Hreflang {
                lang: lang.trim().to_owned(),
                url: url.clone(),
            });
        }
        for token in rel.split_ascii_whitespace() {
            match token {
                "next" => facts.rel_next = Some(url.clone()),
                "prev" | "previous" => facts.rel_prev = Some(url.clone()),
                "amphtml" => facts.amp_html = Some(url.clone()),
                "stylesheet" => {
                    note_mixed(facts, https, &url);
                    facts
                        .links
                        .push(resource(url.clone(), LinkKind::Stylesheet));
                }
                _ => {}
            }
        }
    }
}

fn structured_data(document: &Html) -> Vec<StructuredData> {
    let mut blocks = Vec::new();
    for script in active_elements(document, sel!("script[type]")) {
        let kind = script.value().attr("type").unwrap_or("").trim();
        if !kind.eq_ignore_ascii_case("application/ld+json") {
            continue;
        }
        let json: String = script.text().collect();
        let parsed = serde_json::from_str::<serde_json::Value>(json.trim());
        let mut types = Vec::new();
        if let Ok(value) = &parsed {
            collect_types(value, &mut types);
        }
        blocks.push(StructuredData {
            format: StructuredDataFormat::JsonLd,
            types,
            valid: parsed.is_ok(),
        });
    }
    // Only top-level items: nested ones are properties of the item that contains them.
    for (elements, attribute, format) in [
        (
            active_elements(document, sel!("[itemscope]")),
            "itemtype",
            StructuredDataFormat::Microdata,
        ),
        (
            active_elements(document, sel!("[typeof]")),
            "typeof",
            StructuredDataFormat::Rdfa,
        ),
    ] {
        let scope = if format == StructuredDataFormat::Rdfa {
            "typeof"
        } else {
            "itemscope"
        };
        for item in elements {
            let nested = item
                .ancestors()
                .filter_map(ElementRef::wrap)
                .any(|parent| parent.value().attr(scope).is_some());
            if nested {
                continue;
            }
            let types = item
                .value()
                .attr(attribute)
                .unwrap_or("")
                .split_ascii_whitespace()
                .map(schema_type)
                .collect();
            blocks.push(StructuredData {
                format,
                types,
                valid: true,
            });
        }
    }
    blocks
}

/// `@type` values of a JSON-LD document, including items in `@graph` and arrays.
fn collect_types(value: &serde_json::Value, types: &mut Vec<String>) {
    match value {
        serde_json::Value::Array(items) => {
            for item in items {
                collect_types(item, types);
            }
        }
        serde_json::Value::Object(object) => {
            match object.get("@type") {
                Some(serde_json::Value::String(kind)) => types.push(schema_type(kind)),
                Some(serde_json::Value::Array(kinds)) => {
                    types.extend(
                        kinds
                            .iter()
                            .filter_map(serde_json::Value::as_str)
                            .map(schema_type),
                    );
                }
                _ => {}
            }
            if let Some(graph) = object.get("@graph") {
                collect_types(graph, types);
            }
        }
        _ => {}
    }
}

/// `https://schema.org/Product` → `Product`.
fn schema_type(value: &str) -> String {
    let value = value.trim();
    value
        .rsplit(['/', '#', ':'])
        .next()
        .unwrap_or(value)
        .to_owned()
}

/// Appends the visible text under `node`; block-level elements separate words.
fn visible_text(node: NodeRef<'_, Node>, out: &mut String) {
    for child in node.children() {
        match child.value() {
            Node::Text(text) => out.push_str(text),
            Node::Element(element) if !HIDDEN.contains(&element.name()) => {
                let block = !INLINE.contains(&element.name());
                if block {
                    out.push(' ');
                }
                visible_text(child, out);
                if block {
                    out.push(' ');
                }
            }
            _ => {}
        }
    }
}

/// `5; url=/next` → 5 seconds to `/next` (resolved against `base`).
fn parse_refresh(content: &str, base: &Url) -> MetaRefresh {
    let (delay, rest) = content.split_once([';', ',']).unwrap_or((content, ""));
    let rest = rest.trim();
    let target = match rest.split_once('=') {
        Some((key, value)) if key.trim().eq_ignore_ascii_case("url") => value,
        _ => rest,
    };
    let target = target.trim().trim_matches(['"', '\'']).trim();
    MetaRefresh {
        delay_seconds: delay
            .trim()
            .split('.')
            .next()
            .and_then(|d| d.parse().ok())
            .unwrap_or(0),
        url: absolute(base, target),
    }
}

/// Empty/fragment hyperlinks can navigate to a different document through `<base>`.
/// Keep the existing omission of same-document jumps; they are not new crawl targets.
fn hyperlink_url(base: &Url, page_url: &Url, href: &str) -> Option<String> {
    let href = href.trim();
    if href.is_empty() || href.starts_with('#') {
        let target = &base[..Position::AfterQuery];
        return (matches!(base.scheme(), "http" | "https")
            && target != &page_url[..Position::AfterQuery])
            .then(|| target.to_owned());
    }
    absolute(base, href)
}

/// `href` resolved against `base`: an absolute http(s) URL without fragment, or `None` for
/// other schemes (`mailto:`, `javascript:`, `tel:`, `data:`) and empty or fragment-only links.
fn absolute(base: &Url, href: &str) -> Option<String> {
    let href = href.trim();
    if href.is_empty() || href.starts_with('#') {
        return None;
    }
    let mut url = base.join(href).ok()?;
    if url.scheme() != "http" && url.scheme() != "https" {
        return None;
    }
    url.set_fragment(None);
    Some(url.into())
}

/// A URL-valued meta tag: absolute when it resolves, otherwise as written.
fn content_url(base: &Url, content: &str) -> Option<String> {
    absolute(base, content).or_else(|| (!content.is_empty()).then(|| content.to_owned()))
}

fn resource(url: String, kind: LinkKind) -> Link {
    Link {
        url,
        kind,
        anchor: String::new(),
        name: None,
        nofollow: false,
        ugc: false,
        sponsored: false,
    }
}

fn note_mixed(facts: &mut PageFacts, https: bool, url: &str) {
    if https && url.starts_with("http:") && !facts.mixed_content.iter().any(|seen| seen == url) {
        facts.mixed_content.push(url.to_owned());
    }
}

fn in_svg(element: ElementRef<'_>) -> bool {
    element
        .ancestors()
        .filter_map(ElementRef::wrap)
        .any(|parent| parent.value().name() == "svg")
}

/// The parser exposes template contents in its tree; browsers keep them in a separate fragment.
fn in_template(node: NodeRef<'_, Node>) -> bool {
    node.ancestors()
        .filter_map(ElementRef::wrap)
        .any(|parent| parent.value().name() == "template")
}

fn active_elements<'a>(
    document: &'a Html,
    selector: &'a Selector,
) -> impl Iterator<Item = ElementRef<'a>> + 'a {
    document
        .select(selector)
        .filter(|element| !in_template(**element))
}

/// Active anchors/headings can themselves contain inactive template descendants.
fn active_text(element: ElementRef<'_>) -> String {
    collapse(element.descendants().filter_map(|node| match node.value() {
        Node::Text(text) if !in_template(node) => Some(text.as_ref()),
        _ => None,
    }))
}

/// Joins text fragments, collapsing runs of whitespace into single spaces.
fn collapse<'a>(parts: impl Iterator<Item = &'a str>) -> String {
    let joined: String = parts.collect();
    joined.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[allow(clippy::expect_used)] // Every selector is a literal exercised by the tests.
fn compile(css: &str) -> Selector {
    Selector::parse(css).expect("valid selector")
}

#[cfg(test)]
mod tests;
