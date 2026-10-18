use super::*;
use crate::{CrawlConfig, CrawlPlan, CrawlProgress};
use seonexus_crawl::{ContentKind, CrawlSink, CrawlSummary, Indexability, UrlRecord};
use std::io::Read;

fn archive(workspace: &SpiderWorkspace, count: u32) -> crate::CrawlMeta {
    let plan = CrawlPlan::new(CrawlConfig {
        start_urls: vec!["https://example.com/".into()],
        ..CrawlConfig::default()
    })
    .unwrap();
    let mut writer = workspace.library.create(&plan).unwrap();
    for id in 0..count {
        let url = if id == 0 {
            "https://example.com/?q=1&next=2".into()
        } else {
            format!("https://example.com/page/{id:06}")
        };
        let mut record = UrlRecord::new(id, url, Some(0), true);
        record.status = Some(200);
        record.content_kind = ContentKind::Html;
        record.indexability = Indexability::Indexable;
        writer.url(&record, &[]).unwrap();
    }
    writer
        .finish(&CrawlSummary {
            progress: CrawlProgress::default(),
            stopped: false,
            limit_reached: false,
        })
        .unwrap()
}

#[test]
fn single_xml_escapes_query_strings_includes_every_url_and_has_no_fabricated_dates() {
    let directory = tempfile::tempdir().unwrap();
    let workspace = SpiderWorkspace::resolve(Some(directory.path().to_owned())).unwrap();
    let meta = archive(&workspace, 305);
    let database = workspace.crawls_dir().join(&meta.id).join("crawl.duckdb");
    let before = std::fs::read(&database).unwrap();
    let path = directory.path().join("sitemap.xml");
    let result = workspace
        .export_sitemap(
            &meta.id,
            &SitemapSelection::default(),
            &path,
            SitemapFormat::Xml,
        )
        .unwrap();
    assert_eq!((result.urls, result.parts), (305, 1));
    let xml = std::fs::read_to_string(&path).unwrap();
    assert!(xml.starts_with(SITEMAP_HEADER));
    assert!(xml.ends_with(SITEMAP_FOOTER));
    assert_eq!(xml.matches("<url>").count(), 305);
    assert!(xml.contains("?q=1&amp;next=2"));
    for absent in ["<lastmod>", "<priority>", "<changefreq>"] {
        assert!(!xml.contains(absent));
    }
    let plan = workspace
        .sitemap_plan(&meta.id, &SitemapSelection::default())
        .unwrap();
    assert_eq!(plan.xml_bytes, xml.len() as u64);
    assert_eq!(plan.sample.len(), 100);
    assert_eq!(std::fs::read(&database).unwrap(), before);
}

#[test]
fn splits_fifty_thousand_one_urls_into_a_valid_index_and_atomic_zip() {
    let directory = tempfile::tempdir().unwrap();
    let workspace = SpiderWorkspace::resolve(Some(directory.path().to_owned())).unwrap();
    let meta = archive(&workspace, 50_001);
    let path = directory.path().join("sitemaps.zip");
    let result = workspace
        .export_sitemap(
            &meta.id,
            &SitemapSelection::default(),
            &path,
            SitemapFormat::Zip,
        )
        .unwrap();
    assert_eq!((result.urls, result.parts), (50_001, 2));
    let mut zip = zip::ZipArchive::new(std::fs::File::open(&path).unwrap()).unwrap();
    assert_eq!(zip.len(), 3);
    for (name, count) in [("sitemap-00001.xml", 50_000), ("sitemap-00002.xml", 1)] {
        let mut xml = String::new();
        zip.by_name(name).unwrap().read_to_string(&mut xml).unwrap();
        assert_eq!(xml.matches("<url>").count(), count);
        assert!(xml.len() as u64 <= SITEMAP_MAX_BYTES);
        assert!(xml.starts_with(SITEMAP_HEADER));
        assert!(xml.ends_with(SITEMAP_FOOTER));
    }
    let mut index = String::new();
    zip.by_name("sitemap.xml")
        .unwrap()
        .read_to_string(&mut index)
        .unwrap();
    assert!(index.starts_with(INDEX_HEADER));
    assert!(index.ends_with(INDEX_FOOTER));
    for part in ["sitemap-00001.xml", "sitemap-00002.xml"] {
        assert!(index.contains(&format!("https://example.com/{part}")));
    }
    let xml = directory.path().join("sitemap.xml");
    std::fs::write(&xml, "existing sitemap").unwrap();
    assert!(
        workspace
            .export_sitemap(
                &meta.id,
                &SitemapSelection::default(),
                &xml,
                SitemapFormat::Xml
            )
            .is_err()
    );
    assert_eq!(std::fs::read_to_string(xml).unwrap(), "existing sitemap");
}

#[test]
fn rejects_empty_running_and_invalid_output_without_overwriting_files() {
    let directory = tempfile::tempdir().unwrap();
    let workspace = SpiderWorkspace::resolve(Some(directory.path().to_owned())).unwrap();
    let meta = archive(&workspace, 0);
    let path = directory.path().join("sitemap.xml");
    std::fs::write(&path, "original").unwrap();
    assert!(
        workspace
            .export_sitemap(
                &meta.id,
                &SitemapSelection::default(),
                &path,
                SitemapFormat::Xml
            )
            .is_err()
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "original");
    assert!(
        workspace
            .export_sitemap(
                &meta.id,
                &SitemapSelection::default(),
                &path,
                SitemapFormat::Zip
            )
            .is_err()
    );
    assert!(
        workspace
            .sitemap_plan("../escape", &SitemapSelection::default())
            .is_err()
    );
    let plan = CrawlPlan::new(CrawlConfig {
        start_urls: vec!["https://example.com/".into()],
        ..CrawlConfig::default()
    })
    .unwrap();
    let writer = workspace.library.create(&plan).unwrap();
    let running = workspace
        .crawls()
        .unwrap()
        .into_iter()
        .find(|m| m.status == CrawlStatus::Running)
        .unwrap();
    assert!(
        workspace
            .sitemap_plan(&running.id, &SitemapSelection::default())
            .is_err()
    );
    drop(writer);
}
