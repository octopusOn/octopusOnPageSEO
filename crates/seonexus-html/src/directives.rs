//! Robots directives from `<meta name="robots">` and the `X-Robots-Tag` header.

use serde::Serialize;

/// Indexing and serving directives that apply to a page.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct Directives {
    /// Keep the page out of the index.
    pub noindex: bool,
    /// Do not follow the page's links.
    pub nofollow: bool,
    /// No cached copy (`noarchive` or `nocache`).
    pub noarchive: bool,
    /// No snippet in results.
    pub nosnippet: bool,
    /// Do not index the page's images.
    pub noimageindex: bool,
    /// Most restrictive `max-snippet:` in characters (`-1` = no explicit limit).
    pub max_snippet: Option<i32>,
    /// `unavailable_after:` as written.
    pub unavailable_after: Option<String>,
}

impl Directives {
    /// Adds the directives of `other` (a page is noindex if any source says so).
    pub fn merge(&mut self, other: &Self) {
        self.noindex |= other.noindex;
        self.nofollow |= other.nofollow;
        self.noarchive |= other.noarchive;
        self.nosnippet |= other.nosnippet;
        self.noimageindex |= other.noimageindex;
        self.max_snippet = strictest_snippet(self.max_snippet, other.max_snippet);
        if other.unavailable_after.is_some() {
            self.unavailable_after.clone_from(&other.unavailable_after);
        }
    }
}

/// Parses a directive list such as `noindex, nofollow` or `max-snippet:50`. In `X-Robots-Tag`
/// values a leading `googlebot:` style user-agent token scopes the rest; pass `agents` to keep
/// only directives for those crawlers (lowercase tokens) and unscoped ones. An empty list
/// accepts only unscoped directives, as for a browser without a crawler identity.
pub fn parse_directives(content: &str, agents: &[&str]) -> Directives {
    let mut directives = Directives::default();
    let mut scope_applies = true;
    for part in content.split(',') {
        let mut token = part.trim();
        // `googlebot: noindex` starts a group for that crawler.
        if let Some((agent, rest)) = token.split_once(':') {
            let agent_lower = agent.trim().to_ascii_lowercase();
            if !is_directive_name(&agent_lower) {
                scope_applies = agents.contains(&agent_lower.as_str());
                token = rest.trim();
            }
        }
        if !scope_applies || token.is_empty() {
            continue;
        }
        let lower = token.to_ascii_lowercase();
        let (name, value) = lower
            .split_once(':')
            .map_or((lower.as_str(), ""), |(n, v)| (n.trim(), v.trim()));
        match name {
            "noindex" => directives.noindex = true,
            "nofollow" => directives.nofollow = true,
            "none" => {
                directives.noindex = true;
                directives.nofollow = true;
            }
            "noarchive" | "nocache" => directives.noarchive = true,
            "nosnippet" => directives.nosnippet = true,
            "noimageindex" => directives.noimageindex = true,
            "max-snippet" => {
                directives.max_snippet =
                    strictest_snippet(directives.max_snippet, value.parse().ok());
            }
            "unavailable_after" => {
                // The date keeps its own colons and case; take it from the original text.
                let original = token.split_once(':').map_or("", |(_, v)| v.trim());
                directives.unavailable_after = Some(original.to_owned());
            }
            _ => {}
        }
    }
    directives
}

/// Unlimited never relaxes a finite limit; malformed/unsupported values add no rule.
fn strictest_snippet(left: Option<i32>, right: Option<i32>) -> Option<i32> {
    [left, right]
        .into_iter()
        .flatten()
        .filter(|limit| *limit >= -1)
        .reduce(|a, b| match (a, b) {
            (-1, _) => b,
            (_, -1) => a,
            _ => a.min(b),
        })
}

fn is_directive_name(name: &str) -> bool {
    matches!(
        name,
        "max-snippet"
            | "max-image-preview"
            | "max-video-preview"
            | "unavailable_after"
            | "indexifembedded"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meta_robots_values() {
        let d = parse_directives("NOINDEX, follow", &[]);
        assert!(d.noindex && !d.nofollow);
        let none = parse_directives("none", &[]);
        assert!(none.noindex && none.nofollow);
        assert_eq!(
            parse_directives("max-snippet:50", &[]).max_snippet,
            Some(50)
        );
        assert_eq!(
            parse_directives("unavailable_after: 2026-12-31T23:59:59+00:00", &[])
                .unavailable_after
                .as_deref(),
            Some("2026-12-31T23:59:59+00:00")
        );
    }

    #[test]
    fn x_robots_tag_user_agent_scopes() {
        let header = "googlebot: noindex, bingbot: nofollow";
        let google = parse_directives(header, &["googlebot"]);
        assert!(google.noindex && !google.nofollow);
        let unscoped = parse_directives(header, &[]);
        assert_eq!(unscoped, Directives::default());
        let other = parse_directives(header, &["otherbot"]);
        assert_eq!(other, Directives::default());
    }

    #[test]
    fn no_crawler_tokens_never_inherit_named_header_scopes() {
        let header = "max-snippet:150, GoogleBot: noindex, nofollow, max-snippet:0, BingBot: none, nosnippet";
        let browser = parse_directives(header, &[]);
        assert_eq!(browser.max_snippet, Some(150));
        assert!(!browser.noindex && !browser.nofollow && !browser.nosnippet);
        let google = parse_directives(header, &["googlebot"]);
        assert!(google.noindex && google.nofollow && !google.nosnippet);
        assert_eq!(google.max_snippet, Some(0));
        let bing = parse_directives(header, &["bingbot"]);
        assert!(bing.noindex && bing.nofollow && bing.nosnippet);
        assert_eq!(bing.max_snippet, Some(150));
    }

    #[test]
    fn each_header_field_starts_unscoped_and_permissive_rules_do_not_relax_it() {
        for agents in [&[][..], &["googlebot"][..], &["bingbot"][..]] {
            let mut page = parse_directives("otherbot: none, max-snippet:0", agents);
            page.merge(&parse_directives("noindex, max-snippet:50", agents));
            page.merge(&parse_directives("index, follow, max-snippet:-1", agents));
            assert!(page.noindex && !page.nofollow);
            assert_eq!(page.max_snippet, Some(50));
        }
    }

    #[test]
    fn merge_keeps_the_strictest() {
        let mut page = parse_directives("index, follow", &[]);
        page.merge(&parse_directives("noindex", &[]));
        assert!(page.noindex && !page.nofollow);
    }

    #[test]
    fn snippet_limits_keep_the_strictest_declaration_independent_of_order() {
        for (left, right, expected) in [
            (-1, 0, 0),
            (-1, 50, 50),
            (-1, i32::MAX, i32::MAX),
            (0, 50, 0),
            (50, 150, 50),
            (-1, -1, -1),
        ] {
            for (a, b) in [(left, right), (right, left)] {
                assert_eq!(
                    parse_directives(&format!("max-snippet:{a}, max-snippet:{b}"), &[]).max_snippet,
                    Some(expected)
                );
                let mut merged = parse_directives(&format!("max-snippet:{a}"), &[]);
                merged.merge(&parse_directives(&format!("max-snippet:{b}"), &[]));
                assert_eq!(merged.max_snippet, Some(expected));
            }
        }
    }

    #[test]
    fn invalid_snippet_values_do_not_erase_a_recorded_restriction() {
        for value in ["", "invalid", "-2", "2147483648"] {
            assert_eq!(
                parse_directives(&format!("max-snippet:{value}"), &[]).max_snippet,
                None
            );
            assert_eq!(
                parse_directives(&format!("max-snippet:0, max-snippet:{value}"), &[]).max_snippet,
                Some(0)
            );
        }
        let scoped = parse_directives("max-snippet:50, otherbot: max-snippet:0", &["googlebot"]);
        assert_eq!(scoped.max_snippet, Some(50));
        let restricted = parse_directives("nosnippet, max-snippet:-1", &[]);
        assert!(restricted.nosnippet);
        assert_eq!(restricted.max_snippet, Some(-1));
    }
}
