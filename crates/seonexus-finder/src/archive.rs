//! Free Common Crawl CDX index, XML Sitemap discovery, and SerpApi connectors.

use std::collections::BTreeSet;
use std::io::Read;

use seonexus_core::http::HttpRequest;
use ureq::Agent;

use crate::IndexEngine;
use crate::serp::{ApiCalls, RawSerpHit, url_encode};

/// Queries the free Common Crawl CDX JSONL index for indexed HTML URLs belonging to `base_host`.
pub fn fetch_commoncrawl_cdx(
    agent: &Agent,
    base_host: &str,
    include_subdomains: bool,
) -> Vec<RawSerpHit> {
    let url_pattern = if include_subdomains {
        format!("*.{base_host}/*")
    } else {
        format!("{base_host}/*")
    };
    let endpoint = format!(
        "https://index.commoncrawl.org/CC-MAIN-2024-51-index?url={}&output=json&fl=url,mime,status&limit=1000",
        url_encode(&url_pattern)
    );
    let Some(body) = get_text_bounded(agent, &endpoint) else {
        return Vec::new();
    };
    parse_commoncrawl_cdx_jsonl(&body)
}

/// Parses newline-delimited JSON objects returned by the Common Crawl CDX Server API.
pub fn parse_commoncrawl_cdx_jsonl(jsonl: &str) -> Vec<RawSerpHit> {
    let mut seen = BTreeSet::new();
    let mut hits = Vec::new();
    for line in jsonl.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || !trimmed.starts_with('{') {
            continue;
        }
        let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) else {
            continue;
        };
        let url = val
            .get("url")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .trim();
        let mime = val
            .get("mime")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("text/html")
            .to_ascii_lowercase();
        if url.starts_with("http")
            && (mime.contains("html") || mime.is_empty() || mime == "unk")
            && seen.insert(url.to_owned())
        {
            let status = val
                .get("status")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("200");
            hits.push(RawSerpHit {
                url: url.to_owned(),
                title: String::new(),
                snippet: format!("Common Crawl CDX (HTTP {status})"),
            });
        }
    }
    hits
}

/// Discovers URLs listed in `robots.txt` sitemaps, `/sitemap.xml`, `/sitemap_index.xml`, and `/wp-sitemap.xml`.
pub fn fetch_sitemap_urls(agent: &Agent, base_host: &str) -> Vec<RawSerpHit> {
    let robots_url = format!("https://{base_host}/robots.txt");
    let mut sitemap_urls = Vec::new();
    if let Some(robots_txt) = get_text_bounded(agent, &robots_url) {
        for sm in extract_robots_sitemaps(&robots_txt) {
            if !sitemap_urls.contains(&sm) {
                sitemap_urls.push(sm);
            }
        }
    }
    for candidate in default_sitemap_candidates(base_host) {
        if !sitemap_urls.contains(&candidate) {
            sitemap_urls.push(candidate);
        }
    }

    let mut page_urls = BTreeSet::new();
    let mut child_sitemaps = Vec::new();

    for sm_url in sitemap_urls.into_iter().take(7) {
        let Some(xml) = get_text_bounded(agent, &sm_url) else {
            continue;
        };
        let (is_index, locs) = parse_sitemap_xml_locs(&xml);
        if is_index {
            for child in locs.into_iter().take(4) {
                if !child_sitemaps.contains(&child) {
                    child_sitemaps.push(child);
                }
            }
        } else {
            for loc in locs.into_iter().take(600) {
                page_urls.insert(loc);
            }
        }
    }

    for child_url in child_sitemaps.into_iter().take(4) {
        let Some(xml) = get_text_bounded(agent, &child_url) else {
            continue;
        };
        let (is_index, locs) = parse_sitemap_xml_locs(&xml);
        if !is_index {
            for loc in locs.into_iter().take(500) {
                page_urls.insert(loc);
            }
        }
    }

    page_urls
        .into_iter()
        .map(|url| RawSerpHit {
            url,
            title: String::new(),
            snippet: "XML Sitemap <loc>".to_owned(),
        })
        .collect()
}

/// Returns standard XML sitemap and RSS/Atom feed candidate URLs.
pub fn default_sitemap_candidates(base_host: &str) -> Vec<String> {
    vec![
        format!("https://{base_host}/sitemap.xml"),
        format!("https://{base_host}/sitemap_index.xml"),
        format!("https://{base_host}/sitemap-index.xml"),
        format!("https://{base_host}/wp-sitemap.xml"),
        format!("https://{base_host}/rss.xml"),
        format!("https://{base_host}/feed"),
    ]
}

/// Extracts `Sitemap:` URLs declared in a `robots.txt` body.
pub fn extract_robots_sitemaps(robots_txt: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in robots_txt.lines() {
        let trimmed = line.trim();
        if trimmed.len() > 8 && trimmed[..8].eq_ignore_ascii_case("sitemap:") {
            let url = trimmed[8..].trim();
            if url.starts_with("http://") || url.starts_with("https://") {
                out.push(url.to_owned());
            }
        }
    }
    out
}

/// Extracts `<loc>` URLs from an XML sitemap or sitemap index, falling back to RSS 2.0 / Atom `<link>` extraction.
/// Returns `(is_sitemap_index, urls)`.
pub fn parse_sitemap_xml_locs(xml: &str) -> (bool, Vec<String>) {
    let lower = xml.to_ascii_lowercase();
    let is_index = lower.contains("<sitemapindex");
    let mut urls = Vec::new();
    let mut cursor = xml;

    while let Some(start) = cursor.find("<loc>") {
        let after_start = &cursor[start + 5..];
        let Some(end) = after_start.find("</loc>") else {
            break;
        };
        let raw_url = after_start[..end]
            .trim()
            .trim_start_matches("<![CDATA[")
            .trim_end_matches("]]>")
            .trim()
            .replace("&amp;", "&");
        if raw_url.starts_with("http://") || raw_url.starts_with("https://") {
            urls.push(raw_url);
        }
        cursor = &after_start[end + 6..];
    }

    if urls.is_empty() && (lower.contains("<rss") || lower.contains("<feed")) {
        urls = parse_feed_links(xml);
    }

    (is_index, urls)
}

fn parse_feed_links(xml: &str) -> Vec<String> {
    let mut urls = Vec::new();
    let mut cursor = xml;
    while let Some(start) = cursor.find("<link") {
        let after_tag = &cursor[start + 5..];
        if let Some(rest) = after_tag.strip_prefix('>') {
            let Some(end) = rest.find("</link>") else {
                break;
            };
            let raw = rest[..end]
                .trim()
                .trim_start_matches("<![CDATA[")
                .trim_end_matches("]]>")
                .trim()
                .replace("&amp;", "&");
            if raw.starts_with("http://") || raw.starts_with("https://") {
                urls.push(raw);
            }
            cursor = &rest[end + 7..];
        } else {
            let Some(tag_end) = after_tag.find('>') else {
                break;
            };
            let attrs = &after_tag[..tag_end];
            if !attrs.contains("rel=\"self\"")
                && let Some(href_pos) = attrs.find("href=\"")
            {
                let after_href = &attrs[href_pos + 6..];
                if let Some(q_end) = after_href.find('"') {
                    let raw = after_href[..q_end].trim().replace("&amp;", "&");
                    if raw.starts_with("http://") || raw.starts_with("https://") {
                        urls.push(raw);
                    }
                }
            }
            cursor = &after_tag[tag_end + 1..];
        }
    }
    urls
}

/// SerpApi.com search endpoint.
pub(crate) const SERPAPI_SEARCH: &str = "https://serpapi.com/search.json";

/// Queries SerpApi.com JSON endpoint (`engine=google`) through `api`, so it can run on
/// OctopusOn SEO credits.
pub fn fetch_serpapi(
    api: &mut ApiCalls<'_>,
    api_key: &str,
    query: &str,
    page: u16,
) -> Vec<RawSerpHit> {
    let start = u32::from(page) * 100;
    let url = format!(
        "{SERPAPI_SEARCH}?engine=google&q={}&start={start}&num=100&api_key={}",
        url_encode(query),
        url_encode(api_key.trim())
    );
    api.send(IndexEngine::SerpApi, &HttpRequest::get(url))
        .map(|body| parse_serpapi_json(&body))
        .unwrap_or_default()
}

/// Parses a SerpApi.com JSON response body.
pub fn parse_serpapi_json(json_str: &str) -> Vec<RawSerpHit> {
    let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) else {
        return Vec::new();
    };
    let Some(items) = val
        .get("organic_results")
        .and_then(serde_json::Value::as_array)
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for item in items {
        let Some(link) = item.get("link").and_then(serde_json::Value::as_str) else {
            continue;
        };
        let title = item
            .get("title")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let snippet = item
            .get("snippet")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned();
        out.push(RawSerpHit {
            url: link.to_owned(),
            title,
            snippet,
        });
    }
    out
}

fn get_text_bounded(agent: &Agent, url: &str) -> Option<String> {
    let mut response = agent.get(url).call().ok()?;
    if response.status().as_u16() >= 400 {
        return None;
    }
    let mut buf = Vec::with_capacity(64 * 1024);
    response
        .body_mut()
        .as_reader()
        .take(2 * 1024 * 1024)
        .read_to_end(&mut buf)
        .ok()?;
    Some(String::from_utf8_lossy(&buf).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_commoncrawl_cdx_jsonl_lines() {
        let jsonl = r#"{"url": "https://example.com/eski-kampanya", "mime": "text/html", "status": "200"}
{"url": "https://example.com/logo.png", "mime": "image/png", "status": "200"}
{"url": "https://example.com/blog/yazilar", "mime": "text/html", "status": "301"}"#;
        let hits = parse_commoncrawl_cdx_jsonl(jsonl);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].url, "https://example.com/eski-kampanya");
        assert_eq!(hits[1].url, "https://example.com/blog/yazilar");
    }

    #[test]
    fn extracts_robots_sitemaps_and_parses_sitemap_xml() {
        let robots =
            "User-agent: *\nDisallow: /admin\nSitemap: https://example.com/sitemap_index.xml\n";
        let sitemaps = extract_robots_sitemaps(robots);
        assert_eq!(sitemaps, vec!["https://example.com/sitemap_index.xml"]);
        assert_eq!(default_sitemap_candidates("example.com").len(), 6);

        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
            <url><loc>https://example.com/urun/1</loc></url>
            <url><loc><![CDATA[https://example.com/urun/2?ref=a&amp;b=1]]></loc></url>
        </urlset>"#;
        let (is_index, urls) = parse_sitemap_xml_locs(xml);
        assert!(!is_index);
        assert_eq!(
            urls,
            vec![
                "https://example.com/urun/1",
                "https://example.com/urun/2?ref=a&b=1"
            ]
        );

        let rss = r#"<?xml version="1.0"?><rss version="2.0"><channel>
            <item><link>https://example.com/blog/eski-yazi</link></item>
        </channel></rss>"#;
        let (_, rss_urls) = parse_sitemap_xml_locs(rss);
        assert_eq!(rss_urls, vec!["https://example.com/blog/eski-yazi"]);

        let atom = r#"<feed xmlns="http://www.w3.org/2005/Atom">
            <link rel="self" href="https://example.com/feed.xml" />
            <entry><link rel="alternate" href="https://example.com/blog/atom-yazi" /></entry>
        </feed>"#;
        let (_, atom_urls) = parse_sitemap_xml_locs(atom);
        assert_eq!(atom_urls, vec!["https://example.com/blog/atom-yazi"]);
    }

    #[test]
    fn parses_serpapi_organic_results() {
        let sample = r#"{
            "organic_results": [
                {
                    "position": 1,
                    "title": "Silinmiş Ürün",
                    "link": "https://example.com/urun/silinmis",
                    "snippet": "Stokta kalmayan eski ürün"
                }
            ]
        }"#;
        let hits = parse_serpapi_json(sample);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].url, "https://example.com/urun/silinmis");
        assert_eq!(hits[0].title, "Silinmiş Ürün");
    }
}
