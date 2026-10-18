//! Editable defaults for the existing URL exclusion field.

/// Documented Cloudflare infrastructure; deliberately not a blanket `/cdn-cgi/` rule.
pub const DEFAULT_URL_EXCLUSIONS: [&str; 4] = [
    r"^https?://[^/?#]+/cdn-cgi/l/email-protection(?:[?#]|$)",
    r"^https?://[^/?#]+/cdn-cgi/scripts/[0-9a-fA-F]+/cloudflare-static/email-decode\.min\.js(?:[?#]|$)",
    r"^https?://[^/?#]+/cdn-cgi/challenge-platform(?:/|[?#]|$)",
    r"^https?://[^/?#]+/cdn-cgi/(?:trace|rum|script_monitor/report)(?:[?#]|$)",
];

#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;

    #[test]
    fn cloudflare_defaults_have_exact_boundaries_and_preserve_seo_resources() {
        let saved: crate::CrawlConfig = serde_json::from_str(r#"{"urls":{"exclude":[]}}"#).unwrap();
        assert_eq!(saved.urls.exclude, Vec::<String>::new());
        assert_eq!(
            crate::CrawlConfig::default().urls.exclude,
            DEFAULT_URL_EXCLUSIONS
        );
        let patterns = DEFAULT_URL_EXCLUSIONS.map(|pattern| Regex::new(pattern).unwrap());
        let excluded = |url: &str| patterns.iter().any(|pattern| pattern.is_match(url));
        for path in [
            "/cdn-cgi/l/email-protection?x=1#abcd",
            "/cdn-cgi/scripts/5c5dd728/cloudflare-static/email-decode.min.js",
            "/cdn-cgi/challenge-platform",
            "/cdn-cgi/challenge-platform/h/g/",
            "/cdn-cgi/trace",
            "/cdn-cgi/rum?ray=123",
            "/cdn-cgi/script_monitor/report",
        ] {
            assert!(excluded(&format!("https://example.com{path}")), "{path}");
        }
        for path in [
            "/cdn-cgi/image/width=800/photo.jpg",
            "/photo.jpg?next=/cdn-cgi/rum",
            "/cdn-cgi/l/email-protection-guide",
            "/cdn-cgi/l/email-protection/real-page",
            "/cdn-cgi/challenge-platform-guide",
            "/cdn-cgi/rum.html",
            "/cdn-cgi/scripts/not-hex/cloudflare-static/email-decode.min.js",
            "/CDN-CGI/trace",
            "/guide/cdn-cgi/trace",
        ] {
            assert!(!excluded(&format!("https://example.com{path}")), "{path}");
        }
    }
}
