use super::*;
use crate::{CrawlConfig, CrawlControl, CrawlPlan, RobotsPolicy, SpiderWorkspace};
use seonexus_crawl::{FetchError, FetchRequest, FetchResponse, Fetcher};
struct Page;
impl Fetcher for Page {
    fn fetch(&self, _request: &FetchRequest<'_>) -> Result<FetchResponse, FetchError> {
        Ok(FetchResponse::new(
            200,
            &[("content-type", "text/html")],
            "<title>Saved technical evidence</title><link rel='canonical' href='https://example.com/'><p>Fixture</p>",
        ))
    }
}
#[test]
fn evidence_uses_saved_completed_crawl_without_mutating_archive_or_fetching_again() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = SpiderWorkspace::resolve(Some(dir.path().into())).unwrap();
    let mut config = CrawlConfig {
        start_urls: vec!["https://example.com/".into()],
        robots: RobotsPolicy::Ignore,
        ..Default::default()
    };
    config.follow.sitemaps = false;
    let meta = workspace
        .crawl_with(
            &CrawlPlan::new(config).unwrap(),
            &Page,
            &CrawlControl::new(),
            |_| {},
        )
        .unwrap();
    let path = workspace.crawls_dir().join(&meta.id).join("crawl.duckdb");
    let before = std::fs::read(&path).unwrap();
    let evidence = crawl_evidence(&workspace, &meta.id, "https://example.com/").unwrap();
    assert_eq!(evidence.created_at_ms, meta.created_at_ms);
    assert_eq!(evidence.facts.rows.len(), 1);
    let title = evidence
        .facts
        .columns
        .iter()
        .position(|c| c.name == "title")
        .unwrap();
    assert_eq!(
        evidence.facts.rows[0][title],
        Cell::Text("Saved technical evidence".into())
    );
    assert_eq!(std::fs::read(path).unwrap(), before);
    assert_eq!(workspace.crawls().unwrap().len(), 1);
}
