//! The sample report the interface's demo shows, drawn by the real renderer in every design.
//! `UPDATE_DEMO_REPORTS=1 cargo test -p octopuson-platform-app demo_reports` writes them to
//! `apps/geo-seo-platform/src/api/demo-reports/`; the test fails while they are out of date.

use std::path::PathBuf;

use octopuson_platform::{
    Branding, ProjectRow, REPORT_DESIGNS, ReportColors, ReportRow, ReportSpec,
};
use serde_json::{Value, json};

use super::html::{fonts, render_without_fonts};
use super::{Sources, build};
use crate::modules::snapshots::Compared;
use crate::snapshot::compare::compare;
use crate::snapshot::summary::summarize;
use crate::snapshot::tests::{after, before, side};

const KEYWORDS: &[(&str, u32, i32, u32, u32, &str)] = &[
    ("spor ayakkabı", 4, 3, 3, 49_500, "/spor-ayakkabi"),
    ("koşu ayakkabısı", 3, 2, 2, 33_100, "/kosu"),
    ("sneaker", 11, 6, 9, 27_100, "/sneaker"),
    (
        "kadın spor ayakkabı",
        6,
        1,
        5,
        22_200,
        "/kadin/spor-ayakkabi",
    ),
    ("yürüyüş ayakkabısı", 8, -2, 5, 14_800, "/yuruyus"),
    ("erkek bot", 14, 4, 12, 12_100, "/erkek/bot"),
    ("çocuk ayakkabı", 19, -3, 15, 9_900, "/cocuk"),
    ("outdoor ayakkabı", 7, 5, 7, 6_600, "/outdoor"),
    (
        "ayakkabı numara tablosu",
        2,
        0,
        1,
        5_400,
        "/blog/numara-tablosu",
    ),
    ("su geçirmez bot", 12, 8, 12, 4_400, "/su-gecirmez-bot"),
];

fn rank() -> Value {
    let points = [
        ("2026-07-29", 10.8),
        ("2026-08-05", 11.2),
        ("2026-08-12", 11.0),
        ("2026-08-19", 11.9),
        ("2026-08-26", 12.4),
        ("2026-09-02", 12.1),
        ("2026-09-09", 12.9),
        ("2026-09-16", 13.4),
        ("2026-09-23", 13.9),
        ("2026-09-30", 14.6),
        ("2026-10-07", 15.1),
        ("2026-10-14", 15.8),
    ];
    json!({
        "summary": { "visibility": 15.8, "visibility_change": 2.4, "average_position": 8.2, "ranked": 46, "checked": 50,
            "top3": 7, "top10": 21, "improved": 18, "declined": 6 },
        "visibility": points.iter().map(|(date, value)| json!({ "date": date, "visibility": value })).collect::<Vec<_>>(),
        "competitors": [
            { "domain": "rakip-magaza.com", "own": false, "visibility": 22.4, "keywords": 48, "average_position": 6.1 },
            { "domain": "ornekmagaza.com", "own": true, "visibility": 15.8, "keywords": 46, "average_position": 8.2 },
            { "domain": "digerdukkan.com.tr", "own": false, "visibility": 11.3, "keywords": 39, "average_position": 10.4 },
            { "domain": "ayakkabidunyasi.com", "own": false, "visibility": 7.9, "keywords": 31, "average_position": 13.8 }
        ],
        "rows": KEYWORDS.iter().map(|(keyword, position, change, best, volume, path)| json!({
            "keyword": keyword, "position": position, "change": change, "best": best, "volume": volume,
            "url": format!("https://ornekmagaza.com{path}")
        })).collect::<Vec<_>>()
    })
}

fn gsc() -> Value {
    let daily: Vec<Value> = (0..28)
        .flat_map(|day| {
            let wave = f64::from((day * 7) % 11) * 9.0;
            [
                json!({ "date": format!("2026-09-{:02}", day % 28 + 1), "period": "before", "clicks": 118.0 + wave }),
                json!({ "date": format!("2026-10-{:02}", day % 28 + 1), "period": "now", "clicks": 132.0 + wave + f64::from(day) * 1.6 }),
            ]
        })
        .collect();
    let queries = [
        ("spor ayakkabı", 620.0, 38_500.0, 1.6, 6.8),
        ("koşu ayakkabısı", 540.0, 21_300.0, 2.5, 3.9),
        ("örnek mağaza", 488.0, 1_960.0, 24.9, 1.1),
        ("sneaker", 212.0, 18_200.0, 1.2, 10.4),
        ("kadın spor ayakkabı", 190.0, 9_400.0, 2.0, 6.2),
        ("ayakkabı numara tablosu", 176.0, 4_100.0, 4.3, 2.4),
    ];
    let pages = [
        ("/spor-ayakkabi", 940.0, 52_300.0, 6.5, 1_150.0),
        ("/kosu", 610.0, 24_800.0, 4.2, 720.0),
        ("/", 520.0, 3_900.0, 1.8, 980.0),
        ("/blog/numara-tablosu", 210.0, 5_200.0, 2.6, 260.0),
        ("/sneaker", 198.0, 19_100.0, 10.1, 230.0),
    ];
    json!({
        "period": ["2026-09-24", "2026-10-21"],
        "now": { "clicks": 4_212.0, "impressions": 169_208.0, "ctr": 2.5, "position": 7.4 },
        "before": { "clicks": 3_840.0, "impressions": 155_100.0, "ctr": 2.48, "position": 8.1 },
        "traffic_now": { "sessions": 5_120.0, "key_events": 132.0 },
        "traffic_before": { "sessions": 4_610.0 },
        "daily": daily,
        "queries": queries.iter().map(|(query, clicks, impressions, ctr, position)| json!({
            "query": query, "clicks": clicks, "impressions": impressions, "ctr": ctr, "position": position
        })).collect::<Vec<_>>(),
        "pages": pages.iter().map(|(path, clicks, impressions, position, sessions)| json!({
            "page": format!("https://ornekmagaza.com{path}"), "clicks": clicks, "impressions": impressions,
            "position": position, "sessions": sessions
        })).collect::<Vec<_>>()
    })
}

fn audit() -> Value {
    json!({
        "audit": { "health": 84.0, "geo": 72.5, "pages": 1_240, "issues": 37, "finished_at": "2026-10-20T08:00:00Z" },
        "previous": { "health": 79.5, "geo": 66.0 },
        "categories": [
            { "name": "Bağlantılar", "issues": [
                { "title": "Kırık iç linkler", "kind": "issue", "priority": "high", "count": 4 },
                { "title": "Yönlendirme zincirleri", "kind": "issue", "priority": "medium", "count": 11 }
            ]},
            { "name": "İçerik", "issues": [
                { "title": "H1 etiketi olmayan sayfalar", "kind": "issue", "priority": "high", "count": 3 },
                { "title": "Kısa başlıklar", "kind": "issue", "priority": "low", "count": 14 },
                { "title": "Yinelenen açıklamalar", "kind": "issue", "priority": "medium", "count": 9 }
            ]},
            { "name": "Performans", "issues": [
                { "title": "Yavaş sayfalar", "kind": "issue", "priority": "medium", "count": 6 },
                { "title": "Sıkıştırılmamış görseller", "kind": "warning", "priority": "low", "count": 40 }
            ]}
        ]
    })
}

fn ai() -> Value {
    json!({
        "summary": { "answers": 60, "mention_rate": 51.0, "citation_rate": 34.0, "share_of_voice": 28.6, "average_position": 2.1 },
        "engines": [
            { "engine": "chatgpt", "answers": 12, "mention_rate": 58.3 },
            { "engine": "gemini", "answers": 12, "mention_rate": 50.0 },
            { "engine": "google_ai_mode", "answers": 12, "mention_rate": 41.7 },
            { "engine": "perplexity", "answers": 12, "mention_rate": 66.7 },
            { "engine": "claude", "answers": 12, "mention_rate": 33.3 }
        ],
        "brands": [
            { "key": "rakip-magaza.com", "share": 34.1, "mentions": 41 },
            { "key": "ornekmagaza.com", "share": 28.6, "mentions": 34 },
            { "key": "digerdukkan.com.tr", "share": 21.0, "mentions": 25 },
            { "key": "ayakkabidunyasi.com", "share": 16.3, "mentions": 19 }
        ],
        "history": [
            { "date": "2026-09-30", "mention_rate": 30.0, "citation_rate": 18.0, "share_of_voice": 19.2 },
            { "date": "2026-10-07", "mention_rate": 38.0, "citation_rate": 24.0, "share_of_voice": 22.5 },
            { "date": "2026-10-14", "mention_rate": 44.0, "citation_rate": 29.0, "share_of_voice": 25.8 },
            { "date": "2026-10-21", "mention_rate": 51.0, "citation_rate": 34.0, "share_of_voice": 28.6 }
        ],
        "top_domains": [
            { "key": "rakip-magaza.com", "answers": 22 },
            { "key": "ornekmagaza.com", "own": true, "answers": 18 },
            { "key": "kosuforum.net", "answers": 11 },
            { "key": "sporhaber.com.tr", "answers": 7 }
        ]
    })
}

fn organic() -> Value {
    let keywords = [
        (
            "spor ayakkabı",
            3,
            Some(6),
            "up",
            49_500,
            6_180.0,
            "/spor-ayakkabi",
        ),
        (
            "koşu ayakkabısı",
            2,
            Some(4),
            "up",
            33_100,
            5_020.0,
            "/kosu",
        ),
        ("sneaker", 11, Some(7), "down", 27_100, 410.0, "/sneaker"),
        (
            "kadın spor ayakkabı",
            6,
            Some(6),
            "same",
            22_200,
            1_320.0,
            "/kadin/spor-ayakkabi",
        ),
        (
            "yürüyüş ayakkabısı",
            8,
            Some(5),
            "down",
            14_800,
            520.0,
            "/yuruyus",
        ),
        ("outdoor ayakkabı", 7, None, "new", 6_600, 330.0, "/outdoor"),
        (
            "su geçirmez bot",
            12,
            None,
            "new",
            4_400,
            90.0,
            "/su-gecirmez-bot",
        ),
    ];
    let months = [
        (2025, 11, 12_900.0),
        (2025, 12, 13_400.0),
        (2026, 1, 13_100.0),
        (2026, 2, 14_200.0),
        (2026, 3, 14_900.0),
        (2026, 4, 15_300.0),
        (2026, 5, 15_800.0),
        (2026, 6, 16_400.0),
        (2026, 7, 16_100.0),
        (2026, 8, 17_200.0),
        (2026, 9, 17_900.0),
        (2026, 10, 18_420.0),
    ];
    json!({
        "summary": { "target": "ornekmagaza.com", "organic": { "keywords": 2_310, "traffic": 18_420.0, "traffic_value_usd": 9_640.0,
            "pos_1": 48, "pos_2_3": 102, "pos_4_10": 336, "new": 132, "up": 361, "down": 198, "lost": 87 } },
        "fetched_at": "2026-10-19T08:00:00Z",
        "keywords": { "total": 2_310, "keywords": keywords.iter().map(|(keyword, position, previous, change, volume, traffic, path)| json!({
            "keyword": keyword, "position": position, "previous_position": previous, "change": change, "volume": volume,
            "traffic": traffic, "url": format!("https://ornekmagaza.com{path}")
        })).collect::<Vec<_>>() },
        "lost": { "total": 87, "keywords": [
            { "keyword": "kışlık bot kadın", "previous_position": 14, "volume": 5_400, "url": "https://ornekmagaza.com/kadin/bot" },
            { "keyword": "deri bot erkek", "previous_position": 18, "volume": 4_400, "url": "https://ornekmagaza.com/erkek/bot" }
        ]},
        "history": months.iter().map(|(year, month, traffic)| json!({ "year": year, "month": month, "organic": { "traffic": traffic } })).collect::<Vec<_>>(),
        "points": []
    })
}

fn links() -> Value {
    let history: Vec<Value> = (0..13)
        .map(|index| {
            let (year, month) = if index < 3 { (2025, 10 + index) } else { (2026, index - 2) };
            json!({ "month": format!("{year}-{month:02}"), "rank": 31 + index / 2, "referring_domains": 402 + index * 7 })
        })
        .collect();
    json!({
        "summary": { "rank": 38, "backlinks": 6_120, "referring_domains": 486, "broken_backlinks": 12 },
        "fetched_at": "2026-10-19T08:00:00Z",
        "history": history,
        "new_lost": [
            { "date": "2026-07-31", "new_backlinks": 210, "lost_backlinks": 64, "new_referring_domains": 14, "lost_referring_domains": 5 },
            { "date": "2026-08-31", "new_backlinks": 180, "lost_backlinks": 91, "new_referring_domains": 12, "lost_referring_domains": 7 },
            { "date": "2026-09-30", "new_backlinks": 240, "lost_backlinks": 58, "new_referring_domains": 17, "lost_referring_domains": 4 },
            { "date": "2026-10-31", "new_backlinks": 160, "lost_backlinks": 70, "new_referring_domains": 11, "lost_referring_domains": 6 }
        ],
        "audit": [
            { "domain": "seo-links-4u.xyz", "spam_score": 84, "score": 92, "risk": "high" },
            { "domain": "bedava-dizin.top", "spam_score": 71, "score": 80, "risk": "high" },
            { "domain": "linkciftligi.net", "spam_score": 38, "score": 55, "risk": "medium" },
            { "domain": "modablog.com.tr", "spam_score": 3, "score": 6, "risk": "low" }
        ],
        "points": [],
        "decisions": [
            { "domain": "seo-links-4u.xyz", "decision": "disavow", "decided_at": "2026-10-12T08:00:00Z" },
            { "domain": "linkciftligi.net", "decision": "watch", "decided_at": "2026-10-12T08:00:00Z" }
        ],
        "prospects": [
            { "domain": "kosuforum.net", "source": "lost", "status": "won" },
            { "domain": "sporhaber.com.tr", "source": "gap", "status": "contacted" },
            { "domain": "ayakkabirehberi.com", "source": "gap", "status": "prospect" }
        ]
    })
}

fn gap() -> Value {
    let keyword = |keyword: &str,
                   volume: u32,
                   difficulty: u32,
                   own: Option<u32>,
                   rival: u32,
                   groups: &[&str]| {
        json!({
            "keyword": keyword, "volume": volume, "difficulty": difficulty,
            "rankings": [
                own.map(|position| json!({ "domain": "ornekmagaza.com", "position": position })),
                { "domain": "rakip-magaza.com", "position": rival }
            ],
            "groups": groups
        })
    };
    json!({
        "own": "ornekmagaza.com", "competitors": ["rakip-magaza.com"],
        "counts": { "shared": 412, "missing": 286, "weak": 74, "strong": 118, "untapped": 0, "unique": 1_640 },
        "keywords": [
            keyword("trail koşu ayakkabısı", 12_100, 38, None, 3, &["missing"]),
            keyword("basketbol ayakkabısı", 9_900, 44, None, 5, &["missing"]),
            keyword("tenis ayakkabısı", 8_100, 35, None, 7, &["missing"]),
            keyword("sneaker", 27_100, 55, Some(11), 4, &["shared", "weak"]),
            keyword("yürüyüş ayakkabısı", 14_800, 40, Some(8), 2, &["shared", "weak"])
        ]
    })
}

fn opportunities() -> Value {
    json!({
        "open": 14, "gain": 4_380.0,
        "items": [
            { "rule": "near_top", "subject": "spor ayakkabı", "detail": {}, "gain": 2_740.0, "effort": "medium", "state": "open" },
            { "rule": "cannibal", "subject": "koşu ayakkabısı", "detail": {}, "gain": 770.0, "effort": "medium", "state": "open" },
            { "rule": "low_ctr", "subject": "kışlık bot", "detail": {}, "gain": 310.0, "effort": "low", "state": "open" },
            { "rule": "audit_issue", "subject": "structured-data-missing", "detail": { "title": "Yapısal veri yok" }, "gain": null, "effort": "medium", "state": "open" },
            { "rule": "orphans", "subject": "orphans", "detail": { "count": 6 }, "gain": null, "effort": "medium", "state": "open" },
            { "rule": "ai_gap", "subject": "En iyi koşu ayakkabısı markası hangisi?", "detail": {}, "gain": null, "effort": "high", "state": "open" },
            { "rule": "lost_links", "subject": "lost", "detail": { "count": 3 }, "gain": null, "effort": "medium", "state": "open" }
        ]
    })
}

fn compared() -> Compared {
    let (mut from, mut to) = (before(), after());
    // The comparison fixtures, dressed as the sample shop.
    for data in [&mut from, &mut to] {
        data.domain = "ornekmagaza.com".to_owned();
        data.competitors[0].domain = "rakip-magaza.com".to_owned();
        for keyword in data.keywords.iter_mut().flatten() {
            keyword.url = keyword
                .url
                .as_ref()
                .map(|url| url.replace("ornek.com", "ornekmagaza.com"));
        }
        for page in data.pages.iter_mut().flatten() {
            page.url = page
                .url
                .replace("ornek.com/a", "ornekmagaza.com/spor-ayakkabi")
                .replace("ornek.com/b", "ornekmagaza.com/kosu");
        }
        for domain in data.domains.iter_mut().flatten() {
            domain.domain = match domain.domain.as_str() {
                "eski.com" => "eskidizin.net",
                "kalan.com" => "modablog.com.tr",
                "yeni.com" => "kosuforum.net",
                other => other,
            }
            .to_owned();
        }
        for check in data
            .audit
            .iter_mut()
            .flat_map(|audit| audit.checks.iter_mut())
        {
            check.title = match check.id.as_str() {
                "title_missing" => "Başlığı olmayan sayfalar",
                "h1_missing" => "H1 etiketi olmayan sayfalar",
                "slow" => "Yavaş sayfalar",
                other => other,
            }
            .to_owned();
        }
    }
    let comparison = compare(
        (&side(1, "2026-07-24T09:00:00Z"), &from),
        (&side(4, "2026-10-21T09:00:00Z"), &to),
    );
    Compared {
        summary: summarize(&comparison, "tr"),
        comparison,
    }
}

fn report(design: &str) -> ReportRow {
    ReportRow {
        id: 1,
        project_id: 1,
        spec: ReportSpec {
            name: "Aylık SEO raporu".to_owned(),
            language: "tr".to_owned(),
            days: 28,
            widgets: [
                "summary",
                "comparison",
                "opportunities",
                "rankings",
                "organic",
                "competitors",
                "gap",
                "search_console",
                "site_audit",
                "ai_visibility",
                "backlinks",
                "link_history",
            ]
            .map(str::to_owned)
            .to_vec(),
            schedule: "monthly".to_owned(),
            recipients: Vec::new(),
            design: design.to_owned(),
            colors: ReportColors::default(),
        },
        share_token: None,
        share_views: 0,
        last_sent_at: None,
        created_at: "2026-10-01T00:00:00Z".to_owned(),
        updated_at: "2026-10-01T00:00:00Z".to_owned(),
    }
}

/// The sample report in `design`, without the embedded font.
fn page(design: &str) -> String {
    let project = ProjectRow {
        id: 1,
        name: "Örnek Mağaza".to_owned(),
        domain: "ornekmagaza.com".to_owned(),
        country: "tr".to_owned(),
        language: "tr".to_owned(),
        competitors: vec!["rakip-magaza.com".to_owned()],
        brands: Vec::new(),
        created_at: "2026-10-01T00:00:00Z".to_owned(),
        updated_at: "2026-10-01T00:00:00Z".to_owned(),
    };
    let branding = Branding {
        company: Some("Örnek Ajans".to_owned()),
        website: Some("https://ornekajans.com".to_owned()),
        footer: Some("Gizli · yalnızca müşterimiz içindir".to_owned()),
        ..Branding::default()
    };
    let sources = Sources {
        rank: Some(rank()),
        gsc: Some(gsc()),
        audit: Some(audit()),
        ai: Some(ai()),
        overview: Some(
            json!({ "data": { "organic": { "traffic": 18_420.0, "keywords": 2_310.0 } }, "fetched_at": "2026-10-19T08:00:00Z" }),
        ),
        backlinks: Some(
            json!({ "data": { "rank": 38, "referring_domains": 486, "backlinks": 6_120, "broken_backlinks": 12 }, "fetched_at": "2026-10-19T08:00:00Z" }),
        ),
        comparison: Some(compared()),
        comparison_previous: None,
        local: None,
        organic: Some(organic()),
        links: Some(links()),
        gap: Some(gap()),
        lists: None,
        opportunities: Some(opportunities()),
    };
    let view = build(
        &report(design),
        &project,
        branding,
        &sources,
        (
            ("2026-09-24".to_owned(), "2026-10-21".to_owned()),
            "2026-10-21".to_owned(),
        ),
    );
    render_without_fonts(&view)
}

#[test]
fn demo_reports_are_up_to_date() {
    let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../apps/geo-seo-platform/src/api/demo-reports");
    let mut files: Vec<(String, String)> = REPORT_DESIGNS
        .iter()
        .map(|design| (format!("{design}.html"), page(design)))
        .collect();
    files.push(("fonts.css".to_owned(), fonts().to_owned()));
    if std::env::var_os("UPDATE_DEMO_REPORTS").is_some() {
        std::fs::create_dir_all(&folder).unwrap();
        for (name, text) in &files {
            std::fs::write(folder.join(name), text).unwrap();
        }
        return;
    }
    for (name, text) in &files {
        let kept = std::fs::read_to_string(folder.join(name)).unwrap_or_default();
        assert!(
            kept == *text,
            "{name} is out of date: run UPDATE_DEMO_REPORTS=1 cargo test -p octopuson-platform-app demo_reports"
        );
    }
}
