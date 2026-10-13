use serde::{Deserialize, Serialize};

use crate::encoding::normalize_escapes;
use crate::query::{filter_parameters, is_tracking_parameter};

/// What happens to query strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum QueryMode {
    /// Keep every parameter.
    Keep,
    /// Remove tracking parameters (`utm_*`, `gclid`, …) and keep the rest.
    #[default]
    DropTracking,
    /// Remove the whole query string.
    DropAll,
}

/// Normalization rules beyond the always-safe ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct NormalizeOptions {
    /// Query-string handling.
    pub query: QueryMode,
    /// Treat `/a/` and `/a` as the same URL.
    pub ignore_trailing_slash: bool,
    /// Treat paths case-insensitively (for case-insensitive servers such as IIS).
    pub ignore_case: bool,
}

/// A normalized URL: an optional host plus the comparable path-and-query key.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
pub struct NormalizedUrl {
    /// Lower-case host (with a non-default port), when the input was absolute.
    pub host: Option<String>,
    /// Path and query, always starting with `/`.
    pub key: String,
}

/// Normalizes an absolute URL (`https://host/path`) or a request target (`/path?query`, also
/// `//path`). Returns `None` for anything else, such as `*` or text.
pub fn normalize(raw: &str, options: &NormalizeOptions) -> Option<NormalizedUrl> {
    let raw = raw.trim();
    let raw = raw.split_once('#').map_or(raw, |(before, _)| before);
    let (host, target) = split_authority(raw)?;
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let path = if path.is_empty() { "/" } else { path };

    let mut path = remove_dot_segments(&normalize_escapes(path));
    if options.ignore_case {
        path = path.to_lowercase();
    }
    if options.ignore_trailing_slash && path.len() > 1 && path.ends_with('/') {
        let trimmed = path.trim_end_matches('/');
        path = if trimmed.is_empty() {
            "/".to_owned()
        } else {
            trimmed.to_owned()
        };
    }
    let query = match options.query {
        QueryMode::Keep => filter_parameters(&normalize_escapes(query), |_| true),
        QueryMode::DropTracking => filter_parameters(&normalize_escapes(query), |name| {
            !is_tracking_parameter(name)
        }),
        QueryMode::DropAll => String::new(),
    };
    let key = if query.is_empty() {
        path
    } else {
        format!("{path}?{query}")
    };
    Some(NormalizedUrl { host, key })
}

/// Splits `https://Host:443/path` into (`host`, `/path`); request targets have no host.
fn split_authority(raw: &str) -> Option<(Option<String>, &str)> {
    let lower = raw.get(..8).unwrap_or(raw).to_ascii_lowercase();
    let (rest, default_port) = if lower.starts_with("https://") {
        (&raw[8..], ":443")
    } else if lower.starts_with("http://") {
        (&raw[7..], ":80")
    } else if raw.starts_with('/') {
        // Includes `//x`: in request targets that is a path (scanners request
        // `//wp-login.php`), and the sources compared here never use scheme-relative URLs.
        return Some((None, raw));
    } else {
        return None;
    };
    let end = rest.find(['/', '?']).unwrap_or(rest.len());
    let authority = &rest[..end];
    // Drop credentials (`user:pass@host`).
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let host = host.strip_suffix(default_port).unwrap_or(host);
    if host.is_empty() {
        return None;
    }
    // `https://host` and `https://host?x` have the path `/`.
    let target = match &rest[end..] {
        "" => "/",
        target => target,
    };
    Some((Some(host.to_ascii_lowercase()), target))
}

/// RFC 3986 section 5.2.4: resolves `.` and `..` segments.
fn remove_dot_segments(path: &str) -> String {
    if !path.contains("/.") {
        return path.to_owned();
    }
    let mut output: Vec<&str> = Vec::new();
    let segments: Vec<&str> = path.split('/').skip(1).collect();
    let last = segments.len().saturating_sub(1);
    let mut trailing_slash = false;
    for (index, segment) in segments.iter().enumerate() {
        match *segment {
            "." => trailing_slash = index == last,
            ".." => {
                output.pop();
                trailing_slash = index == last;
            }
            other => {
                output.push(other);
                trailing_slash = false;
            }
        }
    }
    let mut result = format!("/{}", output.join("/"));
    if trailing_slash && !result.ends_with('/') {
        result.push('/');
    }
    result
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn key(raw: &str) -> Option<String> {
        normalize(raw, &NormalizeOptions::default()).map(|url| url.key)
    }

    #[test]
    fn absolute_and_relative_forms_meet() {
        let absolute = normalize(
            "HTTPS://WWW.Example.com:443/Blog/%7Epost?utm_source=x&page=2#top",
            &NormalizeOptions::default(),
        )
        .unwrap();
        assert_eq!(absolute.host.as_deref(), Some("www.example.com"));
        assert_eq!(absolute.key, "/Blog/~post?page=2");
        assert_eq!(
            key("/Blog/~post?page=2&gclid=abc").as_deref(),
            Some("/Blog/~post?page=2")
        );
        assert_eq!(key("http://example.com").as_deref(), Some("/"));
        assert_eq!(key("https://example.com?x=1").as_deref(), Some("/?x=1"));
        let double = normalize("//wp-login.php", &NormalizeOptions::default()).unwrap();
        assert_eq!((double.host, double.key.as_str()), (None, "//wp-login.php"));
        assert_eq!(
            normalize("https://Example.com:8443/a", &NormalizeOptions::default())
                .unwrap()
                .host
                .as_deref(),
            Some("example.com:8443")
        );
    }

    #[test]
    fn rejects_non_urls() {
        for raw in [
            "",
            "*",
            "example.com/a",
            "mailto:x@y",
            "https://",
            "https:///a",
        ] {
            assert_eq!(key(raw), None, "{raw}");
        }
    }

    #[test]
    fn resolves_dot_segments() {
        assert_eq!(key("/a/./b/../c").as_deref(), Some("/a/c"));
        assert_eq!(key("/a/b/..").as_deref(), Some("/a/"));
        assert_eq!(key("/../a").as_deref(), Some("/a"));
        assert_eq!(key("/a/.well-known/x").as_deref(), Some("/a/.well-known/x"));
    }

    #[test]
    fn optional_rules() {
        let options = NormalizeOptions {
            query: QueryMode::DropAll,
            ignore_trailing_slash: true,
            ignore_case: true,
        };
        let url = normalize("/Shop/Shoes/?color=red", &options).unwrap();
        assert_eq!(url.key, "/shop/shoes");
        assert_eq!(normalize("/", &options).unwrap().key, "/");
        // Found by the idempotence property: every trailing slash goes at once.
        assert_eq!(normalize("/A//?", &options).unwrap().key, "/a");
        assert_eq!(normalize("//", &options).unwrap().key, "/");
        assert_eq!(normalize("//a//", &options).unwrap().key, "//a");
        assert_eq!(normalize("/x///", &options).unwrap().key, "/x");
        let keep = NormalizeOptions {
            query: QueryMode::Keep,
            ..NormalizeOptions::default()
        };
        assert_eq!(
            normalize("/a?utm_source=x&", &keep).unwrap().key,
            "/a?utm_source=x"
        );
        assert_eq!(key("/a?").as_deref(), Some("/a"));
    }

    proptest! {
        #[test]
        fn normalization_is_idempotent(path in "/[a-zA-Z0-9%./_~?&=-]{0,40}") {
            for options in [
                NormalizeOptions::default(),
                NormalizeOptions { query: QueryMode::DropAll, ignore_trailing_slash: true, ignore_case: true },
            ] {
                if let Some(once) = normalize(&path, &options) {
                    let twice = normalize(&once.key, &options).unwrap();
                    prop_assert_eq!(once.key, twice.key);
                }
            }
        }
    }
}
