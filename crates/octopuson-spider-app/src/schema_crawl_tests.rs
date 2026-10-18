//! Real crawl → saved evidence → optional scoring, with no network or source retention.
use super::*;
use octopuson_crawl::{
    FetchError, FetchRequest, FetchResponse, Fetcher, RenderRequest, RenderedPage, Renderer,
};
use std::time::Duration;

struct Site;
impl Fetcher for Site {
    fn fetch(&self, request: &FetchRequest<'_>) -> Result<FetchResponse, FetchError> {
        let path = request.url.rsplit('/').next().unwrap();
        let json = match path {
            "required" | "generated" | "truncated" | "failed" => {
                r#"{"@context":"https://schema.org","@type":"Product"}"#
            }
            "syntax" => "{broken",
            "recommended" => r#"{"@context":"https://schema.org","@type":"Article"}"#,
            "unsupported" => r#"{"@context":"https://schema.org","@type":"Service"}"#,
            "page" => r#"{"@context":"https://schema.org","@type":"WebPage"}"#,
            "faq-valid" => {
                r#"{"@context":"https://schema.org","@graph":[{"@type":"WebPage","url":"/faq-valid"},{"@type":"FAQPage","mainEntity":[{"@type":"Question","name":"Nasıl?","acceptedAnswer":{"@type":"Answer","text":"Böyle."}}]}]}"#
            }
            "faq-advice" => {
                r#"{"@context":"https://schema.org","@type":"FAQPage","mainEntity":{"@type":"Question","name":"Nasıl?","acceptedAnswer":{"@type":"Answer","text":"<p></p>"}}}"#
            }
            "faq-ref" => {
                r##"{"@context":"https://schema.org","@type":"FAQPage","mainEntity":{"@id":"#unresolved"}}"##
            }
            _ => "",
        };
        let html = format!(
            "<html lang='en'><head><title>Unique {path} page title for a test</title><meta name='description' content='A complete and accurate test description for the current page and its structured data fields.'></head><body><h1>Test {path}</h1><p>Recorded page content for {path}.</p>{}</body></html>",
            if json.is_empty() {
                String::new()
            } else {
                format!("<script type='application/ld+json'>{json}</script>")
            }
        );
        let mut response = FetchResponse::new(200, &[("Content-Type", "text/html")], html);
        response.truncated = path == "truncated";
        Ok(response)
    }
}
struct Script;
impl Renderer for Script {
    fn render(&self, request: &RenderRequest<'_>) -> Result<RenderedPage, FetchError> {
        if request.url.ends_with("failed") {
            return Err(FetchError::Timeout);
        }
        Ok(RenderedPage {
            url: request.url.into(),
            html: String::from_utf8_lossy(&request.response.body)
                .replace("\"@type\":\"Product\"", "\"@type\":\"Service\""),
            truncated: false,
            elapsed: Duration::ZERO,
        })
    }
}
fn crawl(
    enabled: bool,
    mode: RenderMode,
    paths: &[&str],
) -> (tempfile::TempDir, SpiderWorkspace, CrawlMeta) {
    let dir = tempfile::tempdir().unwrap();
    let workspace = SpiderWorkspace::resolve(Some(dir.path().into())).unwrap();
    let mut config = CrawlConfig {
        mode: CrawlMode::List,
        robots: RobotsPolicy::Ignore,
        start_urls: paths
            .iter()
            .map(|p| format!("https://example.com/{p}"))
            .collect(),
        schema_validation: enabled,
        ..Default::default()
    };
    config.follow.sitemaps = false;
    config.rendering.mode = mode;
    config.speed.max_requests_per_second = 0.0;
    let meta = workspace
        .crawl_with_renderer(
            &CrawlPlan::new(config).unwrap(),
            &Site,
            Some(&Script),
            &CrawlControl::new(),
            |_| {},
        )
        .unwrap();
    (dir, workspace, meta)
}
#[test]
fn optional_schema_crawl_scores_required_fields_and_keeps_absence_and_advice_neutral() {
    let (_dir, workspace, meta) = crawl(
        true,
        RenderMode::Html,
        &[
            "required",
            "syntax",
            "recommended",
            "missing",
            "unsupported",
            "truncated",
        ],
    );
    let reader = workspace.open(&meta.id).unwrap();
    let score = reader.overview().unwrap().seo_score;
    assert_eq!(score.version, 5);
    let coverage = score.schema_validation.unwrap();
    assert_eq!(
        (
            coverage.checked_urls,
            coverage.scored_urls,
            coverage.error_urls,
            coverage.missing_urls,
            coverage.unverified_urls
        ),
        (5, 3, 2, 1, 2)
    );
    assert!(
        score
            .deductions
            .iter()
            .any(|d| d.id == "schema-required-invalid" && d.count == 1 && d.points > 0.0)
    );
    assert!(
        score
            .deductions
            .iter()
            .any(|d| d.id == "structured-data-invalid" && d.count == 1)
    );
    assert!(!score.deductions.iter().any(|d| {
        [
            "structured-data-missing",
            "schema-recommended",
            "schema-unverified",
        ]
        .contains(&d.id.as_str())
    }));
    let url = "https://example.com/required";
    let detail = reader.detail(url).unwrap();
    assert!(detail.html.is_none());
    let report = detail.schema_report.unwrap();
    assert_eq!(report["items"][0]["issues"][0]["path"], "/name");
    let remediation = reader
        .remediation(&RemediationRequest {
            issue_id: "schema-required-invalid".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(remediation.selected_url.as_deref(), Some(url));
    assert_eq!(remediation.schema_report, Some(report));
    assert!(
        remediation
            .evidence
            .columns
            .iter()
            .any(|c| c.name == "json_syntax_valid")
    );
    assert!(
        reader
            .detail("https://example.com/truncated")
            .unwrap()
            .schema_report
            .is_none()
    );
}
#[test]
fn disabled_schema_crawl_does_not_create_property_findings_or_change_the_score_model() {
    let (_dir, workspace, meta) = crawl(false, RenderMode::Html, &["required"]);
    let reader = workspace.open(&meta.id).unwrap();
    let score = reader.overview().unwrap().seo_score;
    assert_eq!(score.version, 4);
    assert!(score.schema_validation.is_none());
    assert!(
        !score
            .deductions
            .iter()
            .any(|d| d.id == "schema-required-invalid")
    );
    assert!(
        reader
            .detail("https://example.com/required")
            .unwrap()
            .schema_report
            .is_none()
    );
}
#[test]
fn javascript_schema_uses_the_dom_and_failed_rendering_stays_unverified() {
    let (_dir, workspace, meta) = crawl(true, RenderMode::Javascript, &["generated", "failed"]);
    let reader = workspace.open(&meta.id).unwrap();
    let generated = reader.detail("https://example.com/generated").unwrap();
    assert_eq!(
        generated.schema_report.unwrap()["items"][0]["types"][0],
        "Service"
    );
    let failed = reader.detail("https://example.com/failed").unwrap();
    assert_eq!(failed.schema_error.as_deref(), Some("incomplete-document"));
    assert!(failed.schema_report.is_none());
    assert!(
        !reader
            .overview()
            .unwrap()
            .seo_score
            .deductions
            .iter()
            .any(|d| d.id == "schema-required-invalid")
    );
}

#[test]
fn page_and_faq_crawl_coverage_and_saved_advice_never_create_score_deductions() {
    let (_dir, workspace, meta) = crawl(
        true,
        RenderMode::Html,
        &["page", "faq-valid", "faq-advice", "faq-ref"],
    );
    let reader = workspace.open(&meta.id).unwrap();
    let score = reader.overview().unwrap().seo_score;
    let coverage = score.schema_validation.unwrap();
    assert_eq!(
        (
            coverage.checked_urls,
            coverage.scored_urls,
            coverage.error_urls,
            coverage.warning_urls,
            coverage.unverified_urls
        ),
        (4, 3, 0, 1, 1)
    );
    assert!(
        !score
            .deductions
            .iter()
            .any(|d| d.id.starts_with("schema-") || d.id.starts_with("structured-data-"))
    );
    let branch = score
        .categories
        .iter()
        .find(|c| c.id == "structured-data")
        .unwrap();
    assert_eq!(branch.points, 0.0);
    let valid = reader
        .detail("https://example.com/faq-valid")
        .unwrap()
        .schema_report
        .unwrap();
    assert_eq!(valid["rule_version"], "2026-10-18.2");
    assert_eq!(valid["items"].as_array().unwrap().len(), 2);
    assert!(
        valid["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["status"] == "checked")
    );
    let advice = reader
        .detail("https://example.com/faq-advice")
        .unwrap()
        .schema_report
        .unwrap();
    assert_eq!(advice["items"][0]["issues"][0]["code"], "faq-answer-text");
    assert_eq!(
        advice["items"][0]["issues"][0]["path"],
        "/mainEntity/acceptedAnswer/text"
    );
    assert_eq!(advice["items"][0]["issues"][0]["severity"], "warning");
    assert_eq!(advice["items"].as_array().unwrap().len(), 1);
}
