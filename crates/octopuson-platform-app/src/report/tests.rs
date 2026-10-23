//! Reports drawn from module answers shaped like the commands return them.

use octopuson_platform::{Branding, ProjectRow, ReportColors, ReportRow, ReportSpec};
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
            design: "clean".to_owned(),
            colors: ReportColors::default(),
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
        comparison: None,
        comparison_previous: None,
        local: None,
        ..Sources::default()
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
    assert!(
        page.contains(
            "default-src 'none'; img-src data:; font-src data:; style-src 'unsafe-inline'"
        )
    );
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

#[test]
fn the_before_after_part_tells_and_lists_what_moved() {
    use crate::modules::snapshots::Compared;
    use crate::snapshot::compare::compare;
    use crate::snapshot::summary::summarize;
    use crate::snapshot::tests::{after, before, side};

    let comparison = compare(
        (&side(1, "2026-10-01T09:00:00Z"), &before()),
        (&side(2, "2026-11-01T09:00:00Z"), &after()),
    );
    let mut data = sources();
    data.comparison = Some(Compared {
        summary: summarize(&comparison, "tr"),
        comparison,
    });
    let view = build(
        &report("tr", &["comparison", "comparison_previous"]),
        &project(),
        Branding::default(),
        &data,
        period(),
    );
    let part = &view.widgets[0];
    assert!(!part.empty);
    assert_eq!(part.title, "Önce / sonra: başlangıçtan bugüne");
    assert_eq!(
        part.note.as_deref(),
        Some("01.10.2026 → 01.11.2026 · 31 gün")
    );
    assert_eq!(
        part.lines[0].text,
        "Organik trafik %23 arttı (1.240 → 1.525)."
    );
    assert_eq!(
        (part.tiles[0].label.as_str(), part.tiles[0].value.as_str()),
        ("Organik trafik (tahmini)", "1.525")
    );
    let change = part.tiles[0].change.as_ref().unwrap();
    assert_eq!(change.text, "+%23 (önce 1.240)");
    assert!(change.good);
    assert!(part.tiles.len() <= 8);
    let titles: Vec<&str> = part
        .tables
        .iter()
        .map(|table| table.title.as_str())
        .collect();
    assert_eq!(titles[0], "Tüm ölçütler");
    for title in [
        "Siz ve rakipleriniz",
        "Yükselen kelimeler",
        "Yeni sıralanan kelimeler",
        "Yeni link veren alan adları",
        "Düzelen sorunlar",
        "Markanın yeni anıldığı AI yanıtları",
    ] {
        assert!(titles.contains(&title), "{title}");
    }
    let sites = &part.tables[1];
    assert_eq!(sites.rows[0][0], Cell::Text("ornek.com".to_owned()));
    assert_eq!(
        sites.rows[0][1],
        Cell::Number {
            value: 1525.0,
            text: "1.525 (+%23)".to_owned()
        }
    );

    let missing = &view.widgets[1];
    assert!(missing.empty, "no snapshot before the latest was given");
    assert_eq!(
        missing.note.as_deref(),
        Some("Karşılaştırma için projede en az iki anlık görüntü olmalı.")
    );
    let page = render(&view);
    assert!(page.contains("<span>Organik trafik %23 arttı (1.240 → 1.525).</span></li>"));
    assert!(page.contains("<li class=\"tone-good\"><span class=\"icon\">"));
    assert!(super::xlsx::workbook(&view).unwrap().starts_with(b"PK"));
}

#[test]
fn every_design_has_its_cover_numbered_parts_and_the_font() {
    let mut row = report("tr", &["summary", "rankings", "site_audit"]);
    for (design, accent, cover_ink) in [
        ("clean", "#1e6fd9", "#0f1726"),
        ("ocean", "#3354d1", "#ffffff"),
        ("accent", "#1e6fd9", "#ffffff"),
        ("elegant", "#9a7b4f", "#0f1726"),
    ] {
        row.spec.design = design.to_owned();
        let page = render(&build(
            &row,
            &project(),
            Branding::default(),
            &sources(),
            period(),
        ));
        assert!(
            page.contains(&format!("class=\"report design-{design}\"")),
            "{design}"
        );
        assert!(
            page.contains(&format!(
                "--accent:{accent};--on-accent:#ffffff;--cover-ink:{cover_ink}"
            )),
            "{design}"
        );
        assert!(page.contains("font-family:\"Geist\";src:url(data:font/woff2;base64,"));
        assert!(page.contains(
            "<span class=\"kicker\">SEO ve GEO performans raporu</span><h1>Aylık rapor</h1>"
        ));
        assert!(page.contains("<li><b>02</b><span>Sıralamalar</span></li>"));
        assert!(page.contains("<span class=\"num\">03</span><div><h2>Site denetimi</h2>"));
        assert_eq!(
            page.matches("class=\"highlight\"").count(),
            4,
            "four highlights on the cover"
        );
        assert!(
            !page.contains("http://"),
            "nothing is loaded from elsewhere"
        );
    }
    // A light accent gets dark ink on the agency-coloured cover; an unknown design reads as white.
    row.spec.design = "neon".to_owned();
    let branding = Branding {
        accent: Some("#ffd84d".to_owned()),
        ..Branding::default()
    };
    let page = render(&build(&row, &project(), branding, &sources(), period()));
    assert!(
        page.contains(
            "class=\"report design-clean\" style=\"--accent:#ffd84d;--on-accent:#0f1726;--cover-ink:#0f1726\""
        )
    );
    assert!(
        page.contains("<strong>OctopusOn SEO</strong>"),
        "unbranded reports carry the program's mark"
    );
}

#[test]
fn a_report_can_have_colours_of_its_own() {
    let mut row = report("tr", &["summary", "rankings"]);
    row.spec.design = "ocean".to_owned();
    row.spec.colors = ReportColors {
        accent: Some("#aa3300".to_owned()),
        accent2: Some("#00aa88".to_owned()),
        cover: Some("#fafafa".to_owned()),
        heading: Some("#222222".to_owned()),
        table: Some("#102030".to_owned()),
    };
    let branding = Branding {
        accent: Some("#123456".to_owned()),
        ..Branding::default()
    };
    let page = render(&build(&row, &project(), branding, &sources(), period()));
    assert!(page.contains(
        "class=\"report design-ocean custom-cover cover-light custom-table\" style=\"--accent:#aa3300;--on-accent:#ffffff;--cover-ink:#0f1726;--cover-bg:#fafafa;--table-head:#102030;--table-ink:#ffffff;--accent-2:#00aa88;--heading:#222222\""
    ), "the report's own accent wins over the agency's");
    assert!(page.contains(".report.custom-cover .cover { background: var(--cover-bg)"));
    // A dark cover of its own carries the light mark and light ink.
    row.spec.design = "clean".to_owned();
    row.spec.colors = ReportColors {
        cover: Some("#14213d".to_owned()),
        ..ReportColors::default()
    };
    let page = render(&build(
        &row,
        &project(),
        Branding::default(),
        &sources(),
        period(),
    ));
    assert!(page.contains("class=\"report design-clean custom-cover cover-dark\""));
    assert!(page.contains("--cover-ink:#ffffff;--cover-bg:#14213d"));
    assert!(
        page.contains("<g fill=\"#ffffff\">"),
        "the light mark on a dark cover"
    );
}

#[test]
fn the_local_part_draws_each_keyword_s_grid() {
    let cell = |row: u32, col: u32, position: Option<u32>| json!({ "row": row, "col": col, "lat": 41.0, "lng": 29.0, "position": position, "top": [], "error": null });
    let local = json!({
        "setup": { "business": { "title": "Örnek Kahve <Kadıköy>" } },
        "grid": {
            "run": { "id": 2, "grid": 3, "radius_km": 1.5 },
            "summary": { "points": 9, "found": 8, "top3": 4, "arp": 4.1, "atrp": 5.9, "solv": 44.4 },
            "keywords": [{
                "keyword": "kahve",
                "summary": { "arp": 4.1, "solv": 44.4 },
                "cells": [cell(0, 0, Some(1)), cell(0, 1, Some(5)), cell(0, 2, Some(14)), cell(1, 1, None)]
            }],
            "rivals": [{ "title": "Rakip Kahve", "top3": 7, "average": 1.4 }],
            "trend": [
                { "run_id": 1, "keyword": "kahve", "arp": 5.0, "solv": 30.0 },
                { "run_id": 2, "keyword": "kahve", "arp": 4.1, "solv": 44.4 }
            ]
        },
        "reviews": { "ratings": [{ "day": "2026-10-01", "rating": 4.4, "votes": 600 }, { "day": "2026-10-20", "rating": 4.5, "votes": 640 }] }
    });
    let mut sources = sources();
    sources.local = Some(local);
    let view = build(
        &report("tr", &["local"]),
        &project(),
        Branding::default(),
        &sources,
        period(),
    );
    let widget = &view.widgets[0];
    assert!(!widget.empty);
    assert_eq!(widget.title, "Yerel SEO");
    assert_eq!(widget.tiles.len(), 4);
    assert_eq!(widget.tiles[0].value, "4,1");
    // The average rank fell by 0.9: better.
    let arp_change = widget.tiles[0].change.as_ref().unwrap();
    assert!(arp_change.good && arp_change.text.starts_with('-'));
    assert_eq!(widget.tiles[3].value, "4,5 (640)");
    assert_eq!(widget.grids.len(), 1);
    let labels: Vec<&str> = widget.grids[0]
        .cells
        .iter()
        .map(|cell| cell.2.as_str())
        .collect();
    assert_eq!(labels, ["1", "5", "14", "20+"]);
    let bands: Vec<&str> = widget.grids[0].cells.iter().map(|cell| cell.3).collect();
    assert_eq!(bands, ["top3", "top10", "top20", "none"]);
    let html = render(&view);
    assert!(html.contains("class=\"band-top3\""));
    assert!(
        html.contains("Örnek Kahve &lt;Kadıköy&gt;"),
        "the business's name is escaped"
    );
    assert!(html.contains("Rakip Kahve"));
    // Without a run the part says so.
    let mut empty = sources;
    empty.local = Some(json!({ "setup": {}, "grid": { "run": null }, "reviews": null }));
    let view = build(
        &report("en", &["local"]),
        &project(),
        Branding::default(),
        &empty,
        period(),
    );
    assert!(view.widgets[0].empty);
}

/// Kept research answers and the user's own link work, as the modules answer them.
fn research() -> Sources {
    Sources {
        organic: Some(json!({
            "summary": { "target": "ornekmagaza.com", "organic": { "keywords": 2100, "traffic": 15400.0, "traffic_value_usd": 8200.0,
                "pos_1": 40, "pos_2_3": 90, "pos_4_10": 300, "new": 120, "up": 340, "down": 210, "lost": 95 } },
            "fetched_at": "2026-10-19T08:00:00Z",
            "keywords": { "total": 2100, "keywords": [
                { "keyword": "spor ayakkabı", "position": 3, "previous_position": 6, "change": "up", "volume": 49500, "traffic": 4100.0, "url": "https://ornekmagaza.com/spor" },
                { "keyword": "sneaker", "position": 11, "previous_position": 7, "change": "down", "volume": 27100, "traffic": 380.0, "url": "https://ornekmagaza.com/sneaker" },
                { "keyword": "bot", "position": 9, "previous_position": null, "change": "new", "volume": 9900, "traffic": 210.0, "url": "https://ornekmagaza.com/bot" }
            ]},
            "lost": { "total": 95, "keywords": [{ "keyword": "kışlık bot", "position": null, "previous_position": 14, "change": "same", "volume": 4400, "traffic": 0.0, "url": null }] },
            "history": [
                { "year": 2026, "month": 8, "organic": { "traffic": 12000.0 } },
                { "year": 2026, "month": 9, "organic": { "traffic": 14000.0 } }
            ],
            "points": []
        })),
        links: Some(json!({
            "summary": { "rank": 41, "backlinks": 6100, "referring_domains": 500 },
            "fetched_at": "2026-10-19T08:00:00Z",
            "history": (1..=13).map(|month| json!({ "month": format!("2025-{month:02}"), "rank": 30 + month, "referring_domains": 380 + month * 5 })).collect::<Vec<_>>(),
            "new_lost": [
                { "date": "2026-09-30", "new_backlinks": 120, "lost_backlinks": 40, "new_referring_domains": 18, "lost_referring_domains": 6 },
                { "date": "2026-10-31", "new_backlinks": 90, "lost_backlinks": 70, "new_referring_domains": 11, "lost_referring_domains": 9 }
            ],
            "audit": [
                { "domain": "spam.xyz", "spam_score": 80, "score": 90, "risk": "high" },
                { "domain": "iffy.net", "spam_score": 35, "score": 50, "risk": "medium" },
                { "domain": "good.com", "spam_score": 2, "score": 5, "risk": "low" }
            ],
            "points": [],
            "decisions": [{ "domain": "spam.xyz", "decision": "disavow", "decided_at": "2026-10-20T08:00:00Z" }],
            "prospects": [
                { "domain": "blog.com", "source": "gap", "status": "contacted" },
                { "domain": "forum.net", "source": "lost", "status": "won" }
            ]
        })),
        gap: Some(json!({
            "own": "ornekmagaza.com", "competitors": ["rakip.com"],
            "counts": { "shared": 10, "missing": 25, "weak": 4, "strong": 6, "untapped": 3, "unique": 40 },
            "keywords": [
                { "keyword": "koşu ayakkabısı", "volume": 33100, "difficulty": 41, "rankings": [null, { "domain": "rakip.com", "position": 2 }], "groups": ["missing"] },
                { "keyword": "sneaker", "volume": 27100, "difficulty": 55, "rankings": [{ "domain": "ornekmagaza.com", "position": 11 }, { "domain": "rakip.com", "position": 4 }], "groups": ["shared", "weak"] }
            ]
        })),
        lists: Some(json!([
            { "name": "Kış sezonu", "keywords": [
                { "keyword": "kışlık bot", "data": { "volume": 4400, "difficulty": 22, "cpc": 0.41, "intent": "commercial" } },
                { "keyword": "kar botu", "data": { "volume": 9900, "difficulty": 30, "cpc": null, "intent": "transactional" } }
            ]},
            { "name": "Boş", "keywords": [] }
        ])),
        opportunities: Some(json!({
            "open": 3, "gain": 950.0,
            "items": [
                { "rule": "near_top", "subject": "spor ayakkabı", "detail": {}, "gain": 640.4, "effort": "medium", "state": "open" },
                { "rule": "audit_issue", "subject": "h1_missing", "detail": { "title": "H1 eksik", "count": 3 }, "gain": null, "effort": "low", "state": "open" },
                { "rule": "orphans", "subject": "orphans", "detail": { "count": 6 }, "gain": null, "effort": "medium", "state": "open" },
                { "rule": "low_ctr", "subject": "bot", "detail": {}, "gain": 80.0, "effort": "low", "state": "done" }
            ]
        })),
        ..Sources::default()
    }
}

#[test]
#[allow(clippy::too_many_lines)] // five parts in turn
fn the_research_parts_read_kept_answers_and_the_user_s_own_work() {
    let widgets = [
        "organic",
        "link_history",
        "gap",
        "keyword_lists",
        "opportunities",
    ];
    let view = build(
        &report("tr", &widgets),
        &project(),
        Branding::default(),
        &research(),
        period(),
    );
    assert!(view.widgets.iter().all(|widget| !widget.empty));
    assert!(
        view.widgets.iter().all(|widget| widget.title != widget.id),
        "every part has a title"
    );

    let organic = &view.widgets[0];
    assert_eq!(organic.tiles[0].value, "15.400");
    assert_eq!(organic.tiles[0].change.as_ref().unwrap().text, "+%17");
    assert_eq!(
        organic.chart.as_ref().unwrap().labels,
        ["08.2026", "09.2026"]
    );
    let titles: Vec<&str> = organic
        .tables
        .iter()
        .map(|table| table.title.as_str())
        .collect();
    assert_eq!(
        titles,
        [
            "En çok trafik getiren kelimeler",
            "Yükselen kelimeler",
            "Düşen kelimeler",
            "Yeni sıralanan kelimeler",
            "Sıralaması kaybolan kelimeler"
        ]
    );
    assert_eq!(
        organic.tables[4].rows[0][1],
        Cell::Number {
            value: 14.0,
            text: "14".to_owned()
        }
    );

    let links = &view.widgets[1];
    let tile = |label: &str| {
        links
            .tiles
            .iter()
            .find(|tile| tile.label == label)
            .unwrap_or_else(|| panic!("{label}"))
    };
    assert_eq!(
        tile("Referans alan adı").change.as_ref().unwrap().text,
        "+%30"
    );
    assert_eq!(tile("Kazanılan alan adları").value, "11");
    assert_eq!(tile("Riskli siteler (yüksek / incelenen)").value, "1 / 3");
    assert_eq!(tile("Reddedilen siteler").value, "1");
    assert_eq!(tile("Süren link çalışmaları").value, "1 (1 iletişimde)");
    assert_eq!(tile("Kazanılan linkler").value, "1");
    let risky = links
        .tables
        .iter()
        .find(|table| table.title == "En riskli link veren siteler")
        .unwrap();
    assert_eq!(risky.rows.len(), 2, "low risk sites are left out");
    assert_eq!(risky.rows[0][3], Cell::Text("Reddet".to_owned()));
    assert_eq!(risky.rows[1][3], Cell::Text("Karar yok".to_owned()));

    let gap = &view.widgets[2];
    assert_eq!(gap.tiles[1].value, "25");
    assert_eq!(gap.tables[0].rows[0][3], Cell::Text("—".to_owned()));
    assert_eq!(
        gap.tables[0].rows[0][4],
        Cell::Text("rakip.com · 2".to_owned())
    );
    assert_eq!(
        gap.tables[1].rows[0][3],
        Cell::Number {
            value: 11.0,
            text: "11".to_owned()
        }
    );

    let lists = &view.widgets[3];
    assert_eq!(lists.tiles[0].value, "1", "empty lists are left out");
    assert_eq!(lists.tables[0].title, "Kış sezonu · 2 kelime");
    assert_eq!(
        lists.tables[0].rows[0][0],
        Cell::Text("kar botu".to_owned()),
        "most searched first"
    );
    assert_eq!(lists.tables[0].rows[0][4], Cell::Text("İşlem".to_owned()));

    let next = &view.widgets[4];
    assert_eq!(next.tables[0].rows.len(), 3, "only open ones");
    assert_eq!(
        next.tables[0].rows[0][0],
        Cell::Text("İlk sıralara yakın".to_owned())
    );
    assert_eq!(
        next.tables[0].rows[0][2],
        Cell::Number {
            value: 640.0,
            text: "640".to_owned()
        }
    );
    assert_eq!(next.tables[0].rows[1][1], Cell::Text("H1 eksik".to_owned()));
    assert_eq!(next.tables[0].rows[2][1], Cell::Text("6 sayfa".to_owned()));

    let html = render(&view);
    assert!(html.contains("Rakiplerle kelime boşluğu"));

    // Nothing kept: each part says so instead of failing.
    let empty = build(
        &report("en", &widgets),
        &project(),
        Branding::default(),
        &Sources::default(),
        period(),
    );
    assert!(empty.widgets.iter().all(|widget| widget.empty));
}

#[test]
fn every_part_has_its_words_in_every_language() {
    for lang in ["tr", "en", "de", "es"] {
        for widget in octopuson_platform::REPORT_WIDGETS {
            assert_ne!(super::texts::text(lang, widget), *widget, "{lang} {widget}");
        }
    }
}
