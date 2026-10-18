//! Audit every declared responsive alternative, independent of the browser viewport.

use std::collections::HashMap;
use std::sync::OnceLock;

use scraper::{ElementRef, Html, Selector};

use super::{Image, Link, LinkKind, PageFacts, active_elements, compile, note_mixed, resource};

mod srcset;

pub(super) fn read(
    document: &Html,
    resolve: &impl Fn(&str) -> Option<String>,
    https: bool,
    facts: &mut PageFacts,
) {
    for img in active_elements(document, sel!("img")) {
        let element = img.value();
        let owner = Image {
            url: String::new(),
            alt: element.attr("alt").map(|alt| alt.trim().to_owned()),
            has_dimensions: element.attr("width").is_some() && element.attr("height").is_some(),
            lazy: element
                .attr("loading")
                .is_some_and(|value| value.trim().eq_ignore_ascii_case("lazy")),
        };
        // Deduplicate within one img, preserving occurrences with different alt attributes.
        let mut candidates = Vec::<Image>::new();
        let mut seen = HashMap::<String, usize>::new();
        let mut add = |path: &str, has_dimensions: bool| {
            let Some(url) = resolve(path) else { return };
            if let Some(&index) = seen.get(&url) {
                // A source can override only one dimension of a duplicate fallback URL.
                candidates[index].has_dimensions &= has_dimensions;
            } else {
                seen.insert(url.clone(), candidates.len());
                candidates.push(Image {
                    url,
                    has_dimensions,
                    ..owner.clone()
                });
            }
        };
        if let Some(src) = element
            .attr("src")
            .filter(|src| !src.trim().is_empty() && !src.trim_start().starts_with("data:"))
            .or_else(|| element.attr("data-src"))
        {
            add(src, owner.has_dimensions);
        }
        for path in srcset::urls(element.attr("srcset").unwrap_or_default()) {
            add(path, owner.has_dimensions);
        }
        if let Some(picture) = img
            .parent()
            .and_then(ElementRef::wrap)
            .filter(|parent| parent.value().name() == "picture")
        {
            for source in picture
                .children()
                .take_while(|child| child.id() != img.id())
                .filter_map(ElementRef::wrap)
                .filter(|child| child.value().name() == "source")
            {
                let source = source.value();
                let dimensions = match (source.attr("width"), source.attr("height")) {
                    (None, None) => owner.has_dimensions,
                    (width, height) => width.is_some() && height.is_some(),
                };
                for path in srcset::urls(source.attr("srcset").unwrap_or_default()) {
                    add(path, dimensions);
                }
            }
        }
        for image in candidates {
            note_mixed(facts, https, &image.url);
            facts.links.push(Link {
                anchor: image.alt.clone().unwrap_or_default(),
                ..resource(image.url.clone(), LinkKind::Image)
            });
            facts.images.push(image);
        }
    }
}
