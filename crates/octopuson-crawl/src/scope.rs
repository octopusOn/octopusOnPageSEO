//! Which URLs belong to the crawled site and which may be crawled.

use regex::Regex;
use url::Url;

use crate::config::CrawlConfig;

/// Compiled scope settings.
#[derive(Debug, Clone)]
pub(crate) struct ScopeRules {
    /// Start hosts without a leading `www.`.
    hosts: Vec<String>,
    subdomains: bool,
    /// Path prefixes (one per start URL) when crawling stays in the start folder.
    folders: Vec<(String, String)>,
    include: Vec<Regex>,
    exclude: Vec<Regex>,
    strip_params: Vec<String>,
    strip_all_params: bool,
    max_url_length: usize,
    max_query_params: Option<usize>,
}

/// Why an internal URL is not crawled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OutOfScope {
    /// Outside the start folder.
    Folder,
    /// No include pattern matches.
    NotIncluded,
    /// An exclude pattern matches.
    Excluded,
    /// Longer than the URL length limit.
    TooLong,
    /// More query parameters than allowed.
    TooManyParams,
}

impl ScopeRules {
    pub(crate) fn new(
        config: &CrawlConfig,
        start: &[Url],
        include: Vec<Regex>,
        exclude: Vec<Regex>,
    ) -> Self {
        let mut hosts: Vec<String> = start
            .iter()
            .filter_map(Url::host_str)
            .map(bare_host)
            .collect();
        hosts.dedup();
        let folders = if config.urls.start_folder_only {
            start
                .iter()
                .filter_map(|url| {
                    let path = url.path();
                    let folder = &path[..=path.rfind('/').unwrap_or(0)];
                    Some((bare_host(url.host_str()?), folder.to_owned()))
                })
                .collect()
        } else {
            Vec::new()
        };
        Self {
            hosts,
            subdomains: config.urls.subdomains,
            folders,
            include,
            exclude,
            strip_params: config
                .urls
                .strip_params
                .iter()
                .map(|p| p.to_ascii_lowercase())
                .collect(),
            strip_all_params: config.urls.strip_all_params,
            max_url_length: config.limits.max_url_length,
            max_query_params: config.limits.max_query_params,
        }
    }

    /// Whether `url` is on the crawled site: a start host with or without `www.`, over http
    /// or https, or a subdomain of one when subdomains are included.
    pub(crate) fn is_internal(&self, url: &Url) -> bool {
        if !is_http_url(url) {
            return false;
        }
        let Some(host) = url.host_str() else {
            return false;
        };
        let host = bare_host(host);
        self.hosts.iter().any(|start| {
            host == *start
                || (self.subdomains
                    && host
                        .strip_suffix(start.as_str())
                        .is_some_and(|rest| rest.ends_with('.')))
        })
    }

    /// Whether an internal `url` may be crawled.
    pub(crate) fn check(&self, url: &Url) -> Result<(), OutOfScope> {
        let text = url.as_str();
        if text.len() > self.max_url_length {
            return Err(OutOfScope::TooLong);
        }
        if let Some(max) = self.max_query_params
            && url.query_pairs().count() > max
        {
            return Err(OutOfScope::TooManyParams);
        }
        if !self.folders.is_empty() {
            let host = url.host_str().map(bare_host).unwrap_or_default();
            let inside = self.folders.iter().any(|(folder_host, folder)| {
                *folder_host == host && url.path().starts_with(folder)
            });
            if !inside {
                return Err(OutOfScope::Folder);
            }
        }
        if !self.include.is_empty() && !self.include.iter().any(|re| re.is_match(text)) {
            return Err(OutOfScope::NotIncluded);
        }
        if self.exclude.iter().any(|re| re.is_match(text)) {
            return Err(OutOfScope::Excluded);
        }
        Ok(())
    }

    /// Removes the fragment and the configured query parameters.
    pub(crate) fn normalize(&self, mut url: Url) -> Url {
        url.set_fragment(None);
        if url.query().is_none() {
            return url;
        }
        if self.strip_all_params {
            url.set_query(None);
        } else if !self.strip_params.is_empty() {
            let kept: Vec<(String, String)> = url
                .query_pairs()
                .filter(|(name, _)| !self.strip_params.contains(&name.to_ascii_lowercase()))
                .map(|(name, value)| (name.into_owned(), value.into_owned()))
                .collect();
            if kept.is_empty() {
                url.set_query(None);
            } else if kept.len() != url.query_pairs().count() {
                url.query_pairs_mut().clear().extend_pairs(kept);
            }
        }
        url
    }
}

/// Only HTTP resources belong to a web crawl, including redirect destinations.
pub(crate) fn is_http_url(url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https")
}

fn bare_host(host: &str) -> String {
    let host = host.to_ascii_lowercase();
    host.strip_prefix("www.").map(str::to_owned).unwrap_or(host)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(edit: impl FnOnce(&mut CrawlConfig)) -> ScopeRules {
        let mut config = CrawlConfig::default();
        edit(&mut config);
        let start = [Url::parse("https://www.example.com/blog/index.html").unwrap()];
        let compile = |p: &[String]| p.iter().map(|p| Regex::new(p).unwrap()).collect();
        let (include, exclude) = (compile(&config.urls.include), compile(&config.urls.exclude));
        ScopeRules::new(&config, &start, include, exclude)
    }

    fn url(text: &str) -> Url {
        Url::parse(text).unwrap()
    }

    #[test]
    fn internal_hosts() {
        let plain = rules(|_| {});
        assert!(plain.is_internal(&url("http://example.com/x")));
        assert!(plain.is_internal(&url("https://WWW.example.com/")));
        assert!(!plain.is_internal(&url("https://shop.example.com/")));
        assert!(!plain.is_internal(&url("https://notexample.com/")));
        let subdomains = rules(|c| c.urls.subdomains = true);
        assert!(subdomains.is_internal(&url("https://shop.example.com/")));
        assert!(!subdomains.is_internal(&url("https://notexample.com/")));
    }

    #[test]
    fn a_matching_host_does_not_make_non_http_schemes_internal() {
        for rules in [rules(|_| {}), rules(|c| c.urls.subdomains = true)] {
            for raw in [
                "ftp://example.com/file",
                "ws://example.com/socket",
                "wss://www.example.com/socket",
                "file://example.com/file",
                "custom://shop.example.com/file",
                "mailto:hello@example.com",
                "data:text/plain,example.com",
            ] {
                assert!(!rules.is_internal(&url(raw)), "{raw}");
            }
            assert!(rules.is_internal(&url("http://example.com:8080/page")));
            assert!(rules.is_internal(&url("https://www.example.com/page")));
        }
    }

    #[test]
    fn scope_checks() {
        let folder = rules(|c| c.urls.start_folder_only = true);
        assert_eq!(folder.check(&url("https://example.com/blog/a")), Ok(()));
        assert_eq!(
            folder.check(&url("https://example.com/shop")),
            Err(OutOfScope::Folder)
        );
        let patterns = rules(|c| {
            c.urls.include = vec!["/blog/".into()];
            c.urls.exclude = vec![r"\?replytocom=".into()];
            c.limits.max_query_params = Some(1);
        });
        assert_eq!(patterns.check(&url("https://example.com/blog/a")), Ok(()));
        assert_eq!(
            patterns.check(&url("https://example.com/a")),
            Err(OutOfScope::NotIncluded)
        );
        assert_eq!(
            patterns.check(&url("https://example.com/blog/a?replytocom=1")),
            Err(OutOfScope::Excluded)
        );
        assert_eq!(
            patterns.check(&url("https://example.com/blog/a?x=1&y=2")),
            Err(OutOfScope::TooManyParams)
        );
    }

    #[test]
    fn normalization() {
        let strip = rules(|c| c.urls.strip_params = vec!["UTM_SOURCE".into(), "sid".into()]);
        assert_eq!(
            strip
                .normalize(url("https://example.com/a?utm_source=x&page=2#top"))
                .as_str(),
            "https://example.com/a?page=2"
        );
        assert_eq!(
            strip.normalize(url("https://example.com/a?sid=1")).as_str(),
            "https://example.com/a"
        );
        let all = rules(|c| c.urls.strip_all_params = true);
        assert_eq!(
            all.normalize(url("https://example.com/a?b=1")).as_str(),
            "https://example.com/a"
        );
    }
}
