//! The HTTP `Link` header (canonical and hreflang annotations of HTML and other files).

use url::Url;

/// One `<url>; rel="..."` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HeaderLink {
    pub(crate) url: String,
    pub(crate) rel: Vec<String>,
    pub(crate) hreflang: Vec<String>,
}

/// Parses web links for the fetched document, resolving targets against `base`.
pub(crate) fn parse_link_header<'a>(
    values: impl Iterator<Item = &'a str>,
    base: &Url,
) -> Vec<HeaderLink> {
    let mut links = Vec::new();
    for value in values {
        for entry in split_entries(value) {
            let entry = entry.trim();
            let Some(rest) = entry.strip_prefix('<') else {
                continue;
            };
            let Some((target, params)) = rest.split_once('>') else {
                continue;
            };
            let Ok(mut url) = base.join(target.trim()) else {
                continue;
            };
            if !matches!(url.scheme(), "http" | "https") {
                continue;
            }
            let Some(params) = parse_parameters(params) else {
                continue;
            };
            // RFC 8288: only the first rel/anchor parameter supplies the relation/context.
            // This crawler cannot attribute a response's annotations to another resource.
            if let Some((_, anchor)) = params.iter().find(|(name, _)| name == "anchor")
                && base.join(anchor).ok().as_ref() != Some(base)
            {
                continue;
            }
            let Some((_, relations)) = params.iter().find(|(name, _)| name == "rel") else {
                continue;
            };
            let rel: Vec<String> = relations
                .split_ascii_whitespace()
                .map(str::to_ascii_lowercase)
                .collect();
            if rel.is_empty() {
                continue;
            }
            url.set_fragment(None);
            links.push(HeaderLink {
                url: url.into(),
                rel,
                hreflang: params
                    .into_iter()
                    .filter(|(name, _)| name == "hreflang")
                    .map(|(_, value)| value.trim().to_owned())
                    .collect(),
            });
        }
    }
    links
}

/// Splits on commas outside `<...>` and quotes.
fn split_entries(value: &str) -> Vec<&str> {
    let mut entries = Vec::new();
    let (mut start, mut in_url, mut in_quotes, mut escaped) = (0, false, false, false);
    for (index, c) in value.char_indices() {
        if in_quotes {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_quotes = false;
            }
            continue;
        }
        match c {
            '<' => in_url = true,
            '>' => in_url = false,
            '"' if !in_url => in_quotes = true,
            ',' if !in_url => {
                entries.push(&value[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    entries.push(&value[start..]);
    entries
}

/// Reads semicolon parameters without interpreting delimiters inside quoted strings.
/// A malformed entry is discarded rather than partially applying its annotations.
fn parse_parameters(mut input: &str) -> Option<Vec<(String, String)>> {
    let mut params = Vec::new();
    loop {
        input = input.trim_start_matches([' ', '\t']);
        if input.is_empty() {
            return Some(params);
        }
        input = input.strip_prefix(';')?.trim_start_matches([' ', '\t']);
        let end = input
            .find(|c: char| c.is_ascii_whitespace() || matches!(c, '=' | ';'))
            .unwrap_or(input.len());
        let name = &input[..end];
        if name.is_empty()
            || !name
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&c))
        {
            return None;
        }
        input = input[end..].trim_start_matches([' ', '\t']);
        let value = if let Some(rest) = input.strip_prefix('=') {
            input = rest.trim_start_matches([' ', '\t']);
            if input.starts_with('"') {
                let (value, rest) = quoted_value(input)?;
                input = rest;
                value
            } else {
                let end = input.find(';').unwrap_or(input.len());
                let value = input[..end].trim_end_matches([' ', '\t']);
                if value.contains('"') {
                    return None;
                }
                input = &input[end..];
                value.to_owned()
            }
        } else {
            String::new()
        };
        params.push((name.to_ascii_lowercase(), value));
    }
}

/// Decodes HTTP quoted-pairs and returns the unconsumed parameter tail.
fn quoted_value(input: &str) -> Option<(String, &str)> {
    let mut chars = input.strip_prefix('"')?.chars();
    let mut value = String::new();
    while let Some(c) = chars.next() {
        match c {
            '\\' => value.push(chars.next()?),
            '"' => return Some((value, chars.as_str())),
            _ => value.push(c),
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_and_hreflang_entries() {
        let base = Url::parse("https://example.com/files/a.pdf").unwrap();
        let header = r#"<https://example.com/a,b.pdf>; rel="canonical", </en/a.pdf>; rel="alternate"; hreflang="en""#;
        let links = parse_link_header([header].into_iter(), &base);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].url, "https://example.com/a,b.pdf");
        assert_eq!(links[0].rel, ["canonical"]);
        assert_eq!(links[1].url, "https://example.com/en/a.pdf");
        assert_eq!(links[1].hreflang, ["en"]);
    }

    #[test]
    fn quoted_titles_cannot_invent_relations_or_hide_following_links() {
        let base = Url::parse("https://example.com/files/a.pdf").unwrap();
        let header = r#"</ignored>; rel=preload; title="Manual; rel=canonical; still a title", </real>; title="A \"quoted, value\"; title"; rel="CANONICAL", </en>; rel=alternate; hreflang="e\n""#;
        let links = parse_link_header([header].into_iter(), &base);
        assert_eq!(links.len(), 3);
        assert_eq!(links[0].rel, ["preload"]);
        assert_eq!(links[1].rel, ["canonical"]);
        assert_eq!(links[2].hreflang, ["en"]);
    }

    #[test]
    fn first_relation_parameter_wins_including_empty_or_valueless_rel() {
        let base = Url::parse("https://example.com/").unwrap();
        let headers = [
            r#"</first>; REL="can\onical alternate"; rel=preload; hreflang=en"#,
            "</ignored>; rel=preload; rel=canonical",
            "</empty>; rel=\"\"; rel=canonical",
            "</valueless>; rel; rel=canonical",
        ];
        let links = parse_link_header(headers.into_iter(), &base);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].rel, ["canonical", "alternate"]);
        assert_eq!(links[0].hreflang, ["en"]);
        assert_eq!(links[1].rel, ["preload"]);
    }

    #[test]
    fn annotations_must_belong_to_the_fetched_document() {
        let base = Url::parse("https://example.com/files/a.pdf").unwrap();
        let headers = [
            r#"<target.pdf#page=1>; rel=canonical; anchor="/files/a.pdf""#,
            r#"</en>; rel=alternate; hreflang=en; anchor="https://EXAMPLE.com/files/a.pdf""#,
            r#"</empty>; rel=canonical; anchor="""#,
            r#"</other>; rel=canonical; anchor="/other.pdf"; anchor="/files/a.pdf""#,
            r##"</fragment>; rel=canonical; anchor="#section""##,
            r#"</invalid>; rel=canonical; anchor="http://[""#,
        ];
        let links = parse_link_header(headers.into_iter(), &base);
        assert_eq!(links.len(), 3);
        assert_eq!(links[0].url, "https://example.com/files/target.pdf");
        assert_eq!(links[1].hreflang, ["en"]);
        assert_eq!(links[2].url, "https://example.com/empty");
    }

    #[test]
    fn malformed_parameters_and_non_web_targets_do_not_become_annotations() {
        let base = Url::parse("https://example.com/a.pdf").unwrap();
        let headers = [
            r#"</bad>; rel=canonical; title="unterminated; rel=alternate"#,
            r#"</bad-tail>; rel="canonical" garbage"#,
            r#"</missing>; title="Description; rel=canonical""#,
            "<javascript:alert(1)>; rel=canonical",
            "<mailto:seo@example.com>; rel=canonical",
            "<ftp://example.com/file>; rel=alternate; hreflang=en",
            r#"</valid>; title="Closing \\"; rel=canonical"#,
        ];
        let links = parse_link_header(headers.into_iter(), &base);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].url, "https://example.com/valid");
        assert_eq!(links[0].rel, ["canonical"]);
    }

    #[test]
    fn repeated_hreflang_parameters_preserve_each_language_and_invalid_empty_codes() {
        let base = Url::parse("https://example.com/file.pdf").unwrap();
        let header = r#"<file.pdf>; rel=alternate; hreflang=" en "; HREFLANG=en-GB; hreflang"#;
        let links = parse_link_header([header].into_iter(), &base);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].hreflang, ["en", "en-GB", ""]);
    }
}
