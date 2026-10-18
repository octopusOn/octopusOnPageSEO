//! XML and plain-text sitemaps, gzip-compressed or not.

use std::borrow::Cow;
use std::io::Read;

use flate2::read::MultiGzDecoder;

use crate::record::{SitemapEntry, SitemapKind};

mod xml;

/// Uncompressed sitemaps may not exceed 50 MB (sitemaps.org).
const MAX_UNCOMPRESSED: u64 = 50 * 1024 * 1024;

/// What a sitemap lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSitemap {
    /// The format found.
    pub kind: SitemapKind,
    /// Page URLs (`<urlset>` or text sitemaps).
    pub entries: Vec<SitemapEntry>,
    /// Child sitemaps (`<sitemapindex>`).
    pub children: Vec<String>,
}

/// Reads a sitemap body. Gzip is detected from the content, not the file name.
///
/// # Errors
/// A message when the body is neither a sitemap XML document nor a list of URLs.
pub fn parse_sitemap(body: &[u8]) -> Result<ParsedSitemap, String> {
    let text = sitemap_text(body, MAX_UNCOMPRESSED)?;
    let text = text.trim_start_matches('\u{feff}');
    if text.trim_start().starts_with('<') {
        return xml::parse(text);
    }
    let entries: Vec<SitemapEntry> = text
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("http://") || line.starts_with("https://"))
        .map(|line| SitemapEntry {
            url: line.to_owned(),
            lastmod: None,
        })
        .collect();
    if entries.is_empty() {
        return Err("not a sitemap: no URLs".to_owned());
    }
    Ok(ParsedSitemap {
        kind: SitemapKind::Text,
        entries,
        children: Vec::new(),
    })
}

/// Read one byte beyond the cap to distinguish an exact-size document from a cut one.
fn sitemap_text(body: &[u8], limit: u64) -> Result<Cow<'_, str>, String> {
    if body.starts_with(&[0x1f, 0x8b]) {
        let mut text = String::new();
        MultiGzDecoder::new(body)
            .take(limit.saturating_add(1))
            .read_to_string(&mut text)
            .map_err(|error| format!("invalid gzip sitemap: {error}"))?;
        if text.len() as u64 > limit {
            return Err("sitemap exceeds uncompressed size limit".into());
        }
        Ok(Cow::Owned(text))
    } else {
        if body.len() as u64 > limit {
            return Err("sitemap exceeds uncompressed size limit".into());
        }
        std::str::from_utf8(body)
            .map(Cow::Borrowed)
            .map_err(|error| format!("invalid UTF-8 sitemap: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use flate2::Compression;
    use flate2::write::GzEncoder;

    use super::*;

    #[test]
    fn url_sets_and_indexes() {
        let xml = r#"<?xml version="1.0"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
            <url><loc>https://example.com/a?x=1&amp;y=2</loc><lastmod>2026-10-01</lastmod></url>
            <url><loc> <![CDATA[https://example.com/b]]> </loc></url><url><loc></loc></url></urlset>"#;
        let parsed = parse_sitemap(xml.as_bytes()).unwrap();
        assert_eq!(parsed.kind, SitemapKind::UrlSet);
        assert_eq!(
            parsed.entries,
            [
                SitemapEntry {
                    url: "https://example.com/a?x=1&y=2".into(),
                    lastmod: Some("2026-10-01".into())
                },
                SitemapEntry {
                    url: "https://example.com/b".into(),
                    lastmod: None
                },
            ]
        );
        let index = "<sitemapindex><sitemap><loc>https://example.com/s1.xml.gz</loc></sitemap></sitemapindex>";
        let parsed = parse_sitemap(index.as_bytes()).unwrap();
        assert_eq!(parsed.kind, SitemapKind::Index);
        assert_eq!(parsed.children, ["https://example.com/s1.xml.gz"]);
    }

    #[test]
    fn gzip_and_text_sitemaps() {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder
            .write_all(b"https://example.com/a\n\nnot a url\nhttps://example.com/b\n")
            .unwrap();
        let parsed = parse_sitemap(&encoder.finish().unwrap()).unwrap();
        assert_eq!(parsed.kind, SitemapKind::Text);
        assert_eq!(parsed.entries.len(), 2);
        assert!(parse_sitemap(b"<html><body>404</body></html>").is_err());
        assert!(parse_sitemap(b"").is_err());
    }

    #[test]
    fn prefixed_sitemaps_read_core_values_without_extension_urls() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
          <sm:urlset xmlns:sm="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:image="urn:images">
            <sm:url><image:loc>https://example.com/not-a-page.png</image:loc>
              <sm:loc xml:space="preserve">https://example.com/a&#x2F;b?x=1&#38;y=2</sm:loc>
              <sm:lastmod>2026-10-17</sm:lastmod></sm:url>
          </sm:urlset>"#;
        let parsed = parse_sitemap(xml.as_bytes()).unwrap();
        assert_eq!(parsed.kind, SitemapKind::UrlSet);
        assert_eq!(
            parsed.entries,
            [SitemapEntry {
                url: "https://example.com/a/b?x=1&y=2".into(),
                lastmod: Some("2026-10-17".into()),
            }]
        );
        let index = br#"<s:sitemapindex xmlns:s="http://www.sitemaps.org/schemas/sitemap/0.9"><s:sitemap><s:loc>https://example.com/map.xml.gz</s:loc></s:sitemap></s:sitemapindex>"#;
        assert_eq!(
            parse_sitemap(index).unwrap().children,
            ["https://example.com/map.xml.gz"]
        );
    }

    #[test]
    fn comments_nested_extensions_and_cdata_do_not_create_phantom_entries() {
        let xml = br"<urlset>
          <!-- <url><loc>https://example.com/comment</loc></url> -->
          <extension><url><loc>https://example.com/nested</loc></url></extension>
          <url><loc><![CDATA[https://example.com/real?literal=&amp;]]></loc></url>
          <url><loc>https://example.com/<![CDATA[a]]><!-- ignored -->?x=1&amp;y=2</loc></url>
        </urlset>";
        let parsed = parse_sitemap(xml).unwrap();
        assert_eq!(
            parsed
                .entries
                .iter()
                .map(|entry| entry.url.as_str())
                .collect::<Vec<_>>(),
            [
                "https://example.com/real?literal=&amp;",
                "https://example.com/a?x=1&y=2",
            ]
        );
        assert!(parse_sitemap(br"<html><!-- <urlset><url><loc>https://example.com/fake</loc></url></urlset> --></html>").is_err());
    }

    #[test]
    fn malformed_xml_is_not_a_successful_partial_sitemap() {
        for xml in [
            "<urlset><url><loc>https://example.com/valid</loc></url>",
            "<urlset><url><loc>https://example.com/</url></loc></urlset>",
            "<urlset/><urlset/>",
            "<urlset><url><loc>https://example.com/?x=&unknown;</loc></url></urlset>",
            "<urlset><url><loc>https://example.com/&#0;</loc></url></urlset>",
            "<urlset><url><loc>https://example.com/&#x1;</loc></url></urlset>",
            "<urlset><url><loc>https://example.com/a&broken</loc></url></urlset>",
            "<urlset><url><loc>https://example.com/<b>a</b></loc></url></urlset>",
            "<urlset><url attr='1' attr='2'><loc>https://example.com/</loc></url></urlset>",
            "<urlset xmlns='urn:not-sitemaps'><url><loc>https://example.com/</loc></url></urlset>",
            "<p:urlset><p:url><p:loc>https://example.com/</p:loc></p:url></p:urlset>",
            "<!DOCTYPE urlset [<!ENTITY custom 'https://example.com/'>]><urlset><url><loc>&custom;</loc></url></urlset>",
        ] {
            assert!(parse_sitemap(xml.as_bytes()).is_err(), "{xml}");
        }
        assert!(
            parse_sitemap(b"<urlset><url><loc>https://example.com/\xff</loc></url></urlset>")
                .is_err()
        );
    }

    fn gzip(bytes: &[u8]) -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(bytes).unwrap();
        encoder.finish().unwrap()
    }

    #[test]
    fn size_boundaries_do_not_silently_accept_partial_documents() {
        let text = b"https://example.com/a\nhttps://example.com/b\n";
        let limit = text.len() as u64;
        let compressed = gzip(text);
        for body in [text.as_slice(), compressed.as_slice()] {
            assert_eq!(
                sitemap_text(body, limit).unwrap(),
                std::str::from_utf8(text).unwrap()
            );
            assert!(
                sitemap_text(body, limit - 1)
                    .unwrap_err()
                    .contains("size limit")
            );
        }
        let mut corrupt = compressed.clone();
        let checksum = corrupt.len() - 8;
        corrupt[checksum] ^= 1;
        assert!(sitemap_text(&corrupt, limit).is_err());
        assert!(sitemap_text(&compressed[..compressed.len() - 3], limit).is_err());
    }

    #[test]
    fn gzip_members_are_read_completely_and_trailing_corruption_is_not_ignored() {
        let mut compressed = gzip(b"https://example.com/a\n");
        compressed.extend(gzip(b"https://example.com/b\n"));
        let parsed = parse_sitemap(&compressed).unwrap();
        assert_eq!(parsed.entries.len(), 2);
        assert_eq!(parsed.entries[1].url, "https://example.com/b");
        compressed.extend(b"not another gzip member");
        assert!(parse_sitemap(&compressed).is_err());
    }

    #[test]
    fn default_namespaces_empty_maps_and_nesting_are_handled_explicitly() {
        let xml = br#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:ext="urn:extension">
          <url><ext:loc>https://example.com/extension</ext:loc><loc>https://example.com/page</loc></url>
          <ext:url><loc>https://example.com/not-core-entry</loc></ext:url>
          <url><loc xmlns="urn:extension">https://example.com/foreign-loc</loc></url>
        </urlset>"#;
        let parsed = parse_sitemap(xml).unwrap();
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].url, "https://example.com/page");
        for xml in [
            "<urlset/>",
            "<sitemapindex/>",
            "<urlset><url><loc/></url></urlset>",
        ] {
            let parsed = parse_sitemap(xml.as_bytes()).unwrap();
            assert_eq!(parsed.entries, Vec::<SitemapEntry>::new());
            assert_eq!(parsed.children, Vec::<String>::new());
        }
        let deep = format!(
            "<urlset>{}{}</urlset>",
            "<extension>".repeat(129),
            "</extension>".repeat(129)
        );
        assert!(
            parse_sitemap(deep.as_bytes())
                .unwrap_err()
                .contains("nesting")
        );
    }
}
