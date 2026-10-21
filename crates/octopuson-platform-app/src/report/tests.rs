//! Reports drawn from module answers shaped like the commands return them.

use octopuson_platform::{Branding, ProjectRow, ReportRow, ReportSpec};
use serde_json::json;

use super::html::{escape, render};
use super::{Cell, Sources, build};

fn project() -> ProjectRow {
    ProjectRow {
        id: 1,
        name: "Örnek <Mağaza>".to_owned(),
        domain: "ornekmagaza.com".to_owned(),
        country: "tr".to_owned(),
        language: "tr".to_owned(),
        competitors: vec!["rakip.com".to_owned()],
        brands: Vec::new(),
        created_at: "2026-10-01T00:00:00Z".to_owned(),
        updated_at: "2026-10-01T00:00:00Z".to_owned(),
    }
}

fn report(language: &str, widgets: &[&str]) -> ReportRow {
    ReportRow {
        id: 7,
        project_id: 1,
        spec: ReportSpec {
            name: "Aylık rapor".to_owned(),
            language: language.to_owned(),
            days: 28,
            widgets: widgets.iter().map(|&widget| widget.to_owned()).collect(),
            schedule: "manual".to_owned(),
            recipients: Vec::new(),
        },
        share_token: None,
        share_views: 0,
        last_sent_at: None,
        created_at: "2026-10-01T00:00:00Z".to_owned(),
        updated_at: "2026-10-01T00:00:00Z".to_owned(),
    }
}

fn sources() -> Sources {
    Sources {
        rank: Some(json!({
            "summary": { "visibility": 12.4, "visibility_change": 1.6, "average_position": 8.2, "ranked": 40, "checked": 50,
                "top3": 5, "top10": 18, "improved": 9, "declined": 3 },
            "visibility": [{ "date": "2026-10-01", "visibility": 10.8 }, { "date": "2026-10-08", "visibility": 12.4 }],
            "competitors": [
                { "domain": "ornekmagaza.com", "own": true, "visibility": 12.4, "keywords": 40, "average_position": 8.2 },
                { "domain": "rakip.com", "own": false, "visibility": 20.1, "keywords": 44, "average_position": 6.0 }
            ],
            "rows": [
                { "keyword": "spor ayakkabı", "position": 4, "change": 2, "best": 3, "volume": 9900, "url": "https://ornekmagaza.com/spor" },
                { "keyword": "<b>bot</b>", "position": null, "change": null, "best": null, "volume": 100, "url": null }
            ]
        })),
        gsc: Some(json!({
            "period": ["2026-09-21", "2026-10-18"],
            "now": { "clicks": 4200.0, "impressions": 120_000.0, "ctr": 3.5, "position": 7.4 },
            "before": { "clicks": 3500.0, "impressions": 100_000.0, "ctr": 3.5, "position": 8.1 },
            "traffic_now": { "sessions": 5000.0, "key_events": 120.0 },
            "traffic_before": { "sessions": 4000.0 },
            "daily": [
                { "date": "2026-09-20", "period": "before", "clicks": 90.0 },
                { "date": "2026-09-21", "period": "now", "clicks": 140.0 },
                { "date": "2026-09-22", "period": "now", "clicks": 150.0 }
            ],
            "queries": [{ "query": "spor ayakkabı", "clicks": 620.0, "impressions": 38500.0, "ctr": 1.6, "position": 6.8 }],
            "pages": [{ "page": "https://ornekmagaza.com/spor-ayakkabi", "clicks": 940.0, "impressions": 52300.0, "position": 6.5, "sessions": 1150.0 }]
        })),
        audit: Some(json!({
            "audit": { "health": 82.5, "geo": 61.0, "pages": 480, "issues": 37, "finished_at": "2026-10-20T08:00:00Z" },
            "previous": { "health": 78.0, "geo": 63.0 },
            "categories": [{ "name": "Links", "issues": [
                { "title": "Broken links", "kind": "issue", "priority": "high", "count": 4 },
                { "title": "Missing alt", "kind": "warning", "priority": "low", "count": 90 },
                { "title": "Fixed one", "kind": "issue", "priority": "high", "count": 0 }
            ]}]
        })),
        ai: Some(json!({
            "summary": { "answers": 40, "mention_rate": 51.3, "citation_rate": 35.9, "share_of_voice": 28.6, "average_position": 1.9 },
            "engines": [{ "engine": "chatgpt", "answers": 8, "mention_rate": 25.0 }, { "engine": "gemini", "answers": 0, "mention_rate": 0.0 }],
            "brands": [{ "key": "ornekmagaza.com", "share": 28.6, "mentions": 20 }],
            "history": [{ "mention_rate": 45.0 }, { "mention_rate": 51.3 }]
        })),
        overview: Some(
            json!({ "data": { "organic": { "traffic": 15400.0, "keywords": 2100.0 } }, "fetched_at": "2026-10-19T08:00:00Z" }),
        ),
        backlinks: None,
    }
}

fn period() -> ((String, String), String) {
    (
        ("2026-09-24".to_owned(), "2026-10-21".to_owned()),
        "2026-10-21".to_owned(),
    )
}

#[test]
fn every_part_is_drawn_from_its_module() {
    let widgets = [
        "summary",
        "rankings",
        "competitors",
        "search_console",
        "site_audit",
        "ai_visibility",
        "backlinks",
    ];
    let view = build(
        &report("tr", &widgets),
        &project(),
        Branding::default(),
        &sources(),
        period(),
    );
    assert_eq!(view.widgets.len(), 7);
    assert!(view.widgets.iter().all(|widget| !widget.empty));
    let summary = &view.widgets[0];
    assert_eq!(
        summary.tiles.len(),
        8,
        "two headline numbers of four modules"
    );
    let rankings = &view.widgets[1];
    assert_eq!(rankings.tiles[0].value, "%12");
    assert_eq!(rankings.tiles[0].change.as_ref().unwrap().text, "+1,6");
    assert_eq!(
        rankings.tables[0].rows[0][0],
        Cell::Text("spor ayakkabı".to_owned()),
        "most searched first"
    );
    assert_eq!(
        rankings.tables[0].rows[0][5],
        Cell::Text("/spor".to_owned())
    );
    assert!(rankings.chart.is_some());
    let competitors = &view.widgets[2];
    assert!(competitors.bars[0].own);
    let console = &view.widgets[3];
    assert_eq!(console.tiles[0].value, "4.200");
    assert_eq!(console.tiles[0].change.as_ref().unwrap().text, "+%20");
    let position = console.tiles[3].change.as_ref().unwrap();
    assert!(position.good, "a lower position is good news");
    assert_eq!(
        console.tiles.len(),
        6,
        "with Analytics' sessions and key events"
    );
    assert_eq!(
        console.chart.as_ref().unwrap().series[1].1,
        [Some(90.0), None]
    );
    let audit = &view.widgets[4];
    assert!(
        !audit.tiles[1].change.as_ref().unwrap().good,
        "GEO readiness fell"
    );
    assert_eq!(audit.tables[0].rows.len(), 1, "only issues still found");
    let ai = &view.widgets[5];
    assert_eq!(ai.bars.len(), 1, "assistants without answers are left out");
    assert_eq!(ai.tiles[0].change.as_ref().unwrap().text, "+6,3");
    let links = &view.widgets[6];
    assert_eq!(links.tiles[0].value, "15.400");

    let nothing = build(
        &report("en", &widgets),
        &project(),
        Branding::default(),
        &Sources::default(),
        period(),
    );
    assert!(nothing.widgets.iter().all(|widget| widget.empty));
}

#[test]
fn the_page_is_self_contained_branded_and_escaped() {
    let branding = Branding {
        company: Some("Ajans & Co".to_owned()),
        logo: Some("data:image/png;base64,iVBORw0KGgo=".to_owned()),
        accent: Some("#aa3300".to_owned()),
        website: Some("https://ajans.example/".to_owned()),
        footer: Some("Gizlidir".to_owned()),
        hide_octopuson: true,
    };
    let view = build(
        &report("en", &["rankings", "search_console"]),
        &project(),
        branding,
        &sources(),
        period(),
    );
    let page = render(&view);
    assert!(page.starts_with("<!doctype html>"));
    assert!(page.contains("default-src 'none'; img-src data:; style-src 'unsafe-inline'"));
    assert!(page.contains("--accent:#aa3300"));
    assert!(page.contains("Ajans &amp; Co"));
    assert!(page.contains("<span>ajans.example</span>"));
    assert!(page.contains("Örnek &lt;Mağaza&gt;"));
    assert!(page.contains("&lt;b&gt;bot&lt;/b&gt;"));
    assert!(!page.contains("<b>bot</b>"));
    assert!(page.contains("<svg viewBox"));
    assert!(page.contains("Period: 24/09/2026 – 21/10/2026"));
    assert!(!page.contains("Prepared with OctopusOn SEO"));
    assert!(!page.contains("<script"));
    assert!(
        !page.contains("http://") && !page.contains("src=\"https"),
        "nothing is loaded from elsewhere"
    );
    assert_eq!(escape("\"'"), "&quot;&#39;");

    let plain = render(&build(
        &report("de", &["site_audit"]),
        &project(),
        Branding::default(),
        &Sources::default(),
        period(),
    ));
    assert!(plain.contains("--accent:#1e6fd9"));
    assert!(plain.contains("Erstellt mit OctopusOn SEO"));
    assert!(plain.contains("Für diesen Teil gibt es noch keine Daten."));
}

#[test]
fn the_workbook_has_a_summary_and_a_sheet_per_table() {
    let view = build(
        &report("tr", &["summary", "rankings", "search_console"]),
        &project(),
        Branding::default(),
        &sources(),
        period(),
    );
    let bytes = super::xlsx::workbook(&view).unwrap();
    assert!(bytes.starts_with(b"PK"), "an xlsx file is a zip");
    assert!(bytes.len() > 2_000);
}
