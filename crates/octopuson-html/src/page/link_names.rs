//! Static link naming evidence, separate from visible text and graphic presence.

use std::collections::HashMap;

use super::{ElementRef, Html, Node, OnceLock, Selector, collapse, compile, in_template};
use serde::Serialize;

/// Recorded naming evidence, not a claim about Google's ranking signals or computed CSS.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct LinkName {
    /// Text content excluding hidden elements and SVG metadata.
    pub visible_text: String,
    /// Best observed descriptive name, including explicit accessibility labels.
    pub name: String,
    /// aria-labelledby, aria-label, content, image-alt, svg-title, title or missing.
    pub source: String,
    /// An active inline SVG exists, including a decorative one.
    pub has_svg: bool,
    /// An active image exists.
    pub has_image: bool,
}

pub(super) struct Names<'a>(HashMap<&'a str, ElementRef<'a>>);

impl<'a> Names<'a> {
    pub(super) fn new(document: &'a Html) -> Self {
        let mut ids = HashMap::new();
        for element in document.select(sel!("[id]")) {
            if !in_template(*element) {
                ids.entry(element.value().attr("id").unwrap_or_default())
                    .or_insert(element);
            }
        }
        Self(ids)
    }

    fn explicit(&self, element: ElementRef<'_>) -> Option<(String, &'static str)> {
        let references = element.value().attr("aria-labelledby").unwrap_or_default();
        let name = references
            .split_ascii_whitespace()
            .filter_map(|id| self.0.get(id))
            // Explicitly referenced hidden labels may name a control. Never recurse into
            // another aria-labelledby chain, so cyclic references cannot loop.
            .map(|label| contents(*label, true, true))
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        if !name.is_empty() {
            return Some((name, "aria-labelledby"));
        }
        element
            .value()
            .attr("aria-label")
            .map(normalize)
            .filter(|label| !label.is_empty())
            .map(|name| (name, "aria-label"))
    }

    pub(super) fn link(&self, anchor: ElementRef<'_>) -> LinkName {
        let has_svg = anchor.select(sel!("svg")).any(|e| !in_template(*e));
        let has_image = anchor.select(sel!("img")).any(|e| !in_template(*e));
        let visible_text = contents(anchor, false, false);
        let (name, source) = self.explicit(anchor).unwrap_or_else(|| {
            let content = contents(anchor, true, false);
            if !content.is_empty() {
                return (
                    content,
                    if visible_text.is_empty() {
                        "image-alt"
                    } else {
                        "content"
                    },
                );
            }
            if anchor.value().name() == "area" {
                let alt = normalize(anchor.value().attr("alt").unwrap_or_default());
                if !alt.is_empty() {
                    return (alt, "image-alt");
                }
            }
            let svg_names = anchor
                .select(sel!("svg"))
                .filter(|svg| !excluded(*svg))
                .filter_map(|svg| {
                    self.explicit(svg).map(|(name, _)| name).or_else(|| {
                        svg.children()
                            .filter_map(ElementRef::wrap)
                            .find(|e| e.value().name() == "title")
                            .map(|title| normalize(&title.text().collect::<String>()))
                            .filter(|text| !text.is_empty())
                    })
                })
                .collect::<Vec<_>>()
                .join(" ");
            if !svg_names.is_empty() {
                return (svg_names, "svg-title");
            }
            let title = normalize(anchor.value().attr("title").unwrap_or_default());
            if !title.is_empty() {
                return (title, "title");
            }
            (String::new(), "missing")
        });
        LinkName {
            visible_text,
            name,
            source: source.into(),
            has_svg,
            has_image,
        }
    }
}

fn normalize(value: &str) -> String {
    collapse(std::iter::once(value))
}

fn hidden(element: ElementRef<'_>) -> bool {
    let value = element.value();
    value.attr("hidden").is_some()
        || value
            .attr("aria-hidden")
            .is_some_and(|v| v.eq_ignore_ascii_case("true"))
        || value.attr("style").is_some_and(|style| {
            style.split(';').any(|rule| {
                let Some((property, value)) = rule.split_once(':') else {
                    return false;
                };
                let value = value.trim().trim_end_matches("!important").trim();
                (property.trim().eq_ignore_ascii_case("display")
                    && value.eq_ignore_ascii_case("none"))
                    || (property.trim().eq_ignore_ascii_case("visibility")
                        && matches!(value.to_ascii_lowercase().as_str(), "hidden" | "collapse"))
            })
        })
}

fn excluded(element: ElementRef<'_>) -> bool {
    hidden(element)
        || element.ancestors().filter_map(ElementRef::wrap).any(|e| {
            hidden(e)
                || matches!(
                    e.value().name(),
                    "template" | "script" | "style" | "noscript"
                )
        })
}

fn contents(element: ElementRef<'_>, images: bool, referenced: bool) -> String {
    let mut text = String::new();
    for node in element.descendants() {
        let skip = std::iter::once(node)
            .chain(node.ancestors())
            .filter_map(ElementRef::wrap)
            .any(|e| {
                matches!(
                    e.value().name(),
                    "template" | "script" | "style" | "noscript"
                ) || (!referenced && (hidden(e) || e.value().name() == "svg"))
            });
        if skip {
            continue;
        }
        match node.value() {
            Node::Text(value) => text.push_str(value),
            Node::Element(value) if images && value.name() == "img" => {
                text.push(' ');
                text.push_str(value.attr("alt").unwrap_or_default());
                text.push(' ');
            }
            _ => {}
        }
    }
    normalize(&text)
}

#[cfg(test)]
mod tests;
