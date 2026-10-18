//! Visible text of an HTML page and its main content.

use ego_tree::NodeRef;
use scraper::{ElementRef, Html, Node, Selector};
use serde::{Deserialize, Serialize};

/// Elements whose content is never visible text.
const HIDDEN: &[&str] = &[
    "script", "style", "noscript", "template", "svg", "iframe", "object", "head", "canvas",
    "select", "option", "button",
];

/// Page furniture left out of the main content.
const BOILERPLATE: &[&str] = &["nav", "header", "footer", "aside", "form", "dialog"];

/// Elements that end a line of text.
const BLOCKS: &[&str] = &[
    "address",
    "article",
    "aside",
    "blockquote",
    "br",
    "dd",
    "div",
    "dl",
    "dt",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "header",
    "hr",
    "li",
    "main",
    "nav",
    "ol",
    "p",
    "pre",
    "section",
    "table",
    "td",
    "th",
    "tr",
    "ul",
];

/// What a page says.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageText {
    /// `<title>`.
    pub title: Option<String>,
    /// All visible text, one block per line.
    pub text: String,
    /// The main content: `<main>`, else the longest `<article>`, else the body without
    /// navigation, header, footer, sidebars and forms.
    pub main: String,
    /// Paragraphs of the main content.
    pub paragraphs: Vec<String>,
    /// Headings of the page in order, with their level.
    pub headings: Vec<(u8, String)>,
    /// List items in the main content.
    pub list_items: u32,
    /// Tables in the main content.
    pub tables: u32,
}

impl PageText {
    /// Words of the main content.
    pub fn main_words(&self) -> usize {
        self.main.split_whitespace().count()
    }

    /// The first `n` words of the main content.
    pub fn opening(&self, n: usize) -> String {
        self.main
            .split_whitespace()
            .take(n)
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Extracts the text of an HTML document.
pub fn extract(html: &str) -> PageText {
    let document = Html::parse_document(html);
    let select = |css: &str| Selector::parse(css).ok();
    let title = select("title")
        .and_then(|selector| document.select(&selector).next())
        .map(|title| squash(&title.text().collect::<String>()))
        .filter(|title| !title.is_empty());
    let body = select("body")
        .and_then(|selector| document.select(&selector).next())
        .unwrap_or_else(|| document.root_element());
    let text = visible(body, false);

    let main = select("main, [role=main]")
        .and_then(|selector| document.select(&selector).next())
        .or_else(|| {
            select("article").and_then(|selector| {
                document
                    .select(&selector)
                    .max_by_key(|article| visible(*article, true).len())
            })
        });
    let (main_element, skip_boilerplate) = match main {
        Some(element) => (element, false),
        None => (body, true),
    };
    let main_text = visible(main_element, skip_boilerplate);

    let within_main = |selector: Option<Selector>| -> Vec<ElementRef<'_>> {
        selector.map_or_else(Vec::new, |selector| {
            main_element
                .select(&selector)
                .filter(|element| !is_hidden(*element, main_element))
                .filter(|element| !skip_boilerplate || !inside_boilerplate(*element, main_element))
                .collect()
        })
    };
    let paragraphs = within_main(select("p"))
        .into_iter()
        .map(|paragraph| squash(&visible(paragraph, false)))
        .filter(|paragraph| !paragraph.is_empty())
        .collect();
    let headings = select("h1, h2, h3, h4, h5, h6")
        .map(|selector| {
            document
                .select(&selector)
                .filter_map(|heading| {
                    let level = heading.value().name().strip_prefix('h')?.parse().ok()?;
                    let text = squash(&visible(heading, false));
                    (!text.is_empty()).then_some((level, text))
                })
                .collect()
        })
        .unwrap_or_default();
    let count = |css: &str| u32::try_from(within_main(select(css)).len()).unwrap_or(u32::MAX);
    PageText {
        title,
        text,
        main: main_text,
        paragraphs,
        headings,
        list_items: count("li"),
        tables: count("table"),
    }
}

/// Whether an element or one of its ancestors below `root` is hidden.
fn is_hidden(element: ElementRef<'_>, root: ElementRef<'_>) -> bool {
    std::iter::once(element)
        .chain(
            element
                .ancestors()
                .take_while(|ancestor| ancestor.id() != root.id())
                .filter_map(ElementRef::wrap),
        )
        .any(|element| {
            let value = element.value();
            HIDDEN.contains(&value.name())
                || value.attr("hidden").is_some()
                || value.attr("aria-hidden") == Some("true")
        })
}

fn inside_boilerplate(element: ElementRef<'_>, root: ElementRef<'_>) -> bool {
    element
        .ancestors()
        .take_while(|ancestor| ancestor.id() != root.id())
        .filter_map(ElementRef::wrap)
        .any(|ancestor| BOILERPLATE.contains(&ancestor.value().name()))
}

/// Visible text below `root`, blocks on separate lines, optionally without page furniture.
fn visible(root: ElementRef<'_>, skip_boilerplate: bool) -> String {
    let mut out = String::new();
    walk(*root, skip_boilerplate, &mut out);
    out.lines()
        .map(squash)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn walk(node: NodeRef<'_, Node>, skip_boilerplate: bool, out: &mut String) {
    for child in node.children() {
        match child.value() {
            Node::Text(text) => out.push_str(text),
            Node::Element(element) => {
                let name = element.name();
                if HIDDEN.contains(&name)
                    || (skip_boilerplate && BOILERPLATE.contains(&name))
                    || element.attr("hidden").is_some()
                    || element.attr("aria-hidden") == Some("true")
                {
                    continue;
                }
                let block = BLOCKS.contains(&name);
                if block {
                    out.push('\n');
                }
                walk(child, skip_boilerplate, out);
                if block {
                    out.push('\n');
                } else if matches!(name, "td" | "th" | "img") {
                    out.push(' ');
                }
            }
            _ => {}
        }
    }
}

fn squash(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = r#"<!doctype html><html><head><title> Türk Kahvesi Tarifi </title>
        <style>.x{}</style><script>var a = 1;</script></head><body>
        <header><nav><a href="/">Ana sayfa</a> <a href="/blog">Blog</a></nav></header>
        <main>
          <h1>Türk Kahvesi Nasıl Yapılır?</h1>
          <p>Türk kahvesi <b>cezvede</b> pişirilir.</p>
          <h2>Malzemeler</h2>
          <ul><li>Kahve</li><li>Su</li></ul>
          <table><tr><td>1</td><td>fincan</td></tr></table>
          <p hidden>Gizli</p><p aria-hidden="true">Bu da gizli</p>
        </main>
        <aside><h3>Popüler yazılar</h3></aside>
        <footer>© 2026</footer></body></html>"#;

    #[test]
    fn extracts_main_content_and_structure() {
        let text = extract(PAGE);
        assert_eq!(text.title.as_deref(), Some("Türk Kahvesi Tarifi"));
        assert!(text.text.contains("Ana sayfa"));
        assert!(!text.text.contains("var a"));
        assert!(!text.main.contains("Ana sayfa") && !text.main.contains("2026"));
        assert!(text.main.contains("Türk kahvesi cezvede pişirilir."));
        assert!(!text.main.contains("Gizli"));
        assert_eq!(text.paragraphs, ["Türk kahvesi cezvede pişirilir."]);
        assert_eq!(
            text.headings,
            [
                (1, "Türk Kahvesi Nasıl Yapılır?".to_owned()),
                (2, "Malzemeler".to_owned()),
                (3, "Popüler yazılar".to_owned())
            ]
        );
        assert_eq!((text.list_items, text.tables), (2, 1));
        assert!(text.main.contains("Kahve\nSu"));
        assert_eq!(text.opening(3), "Türk Kahvesi Nasıl");
    }

    #[test]
    fn falls_back_to_the_body_without_furniture() {
        let html = "<html><body><nav>Menü</nav><div><p>İçerik bir</p><p>İçerik iki</p></div><footer>Alt</footer></body></html>";
        let text = extract(html);
        assert_eq!(text.main, "İçerik bir\nİçerik iki");
        assert_eq!(text.paragraphs.len(), 2);
        let articles = "<html><body><article><p>kısa</p></article><article><p>daha uzun bir makale</p></article></body></html>";
        assert_eq!(extract(articles).main, "daha uzun bir makale");
    }
}
