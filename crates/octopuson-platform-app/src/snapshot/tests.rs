//! Comparing snapshots and telling the comparison, from snapshots made up here.

use super::compare::{Side, compare};
use super::summary::summarize;
use super::*;

pub(crate) fn side(id: i64, at: &str) -> Side {
    Side {
        id,
        label: None,
        taken_at: at.to_owned(),
    }
}

fn keyword(word: &str, position: Option<u32>, traffic: f64) -> KeywordPoint {
    KeywordPoint {
        keyword: word.to_owned(),
        position,
        url: Some(format!("https://ornek.com/{}", word.replace(' ', "-"))),
        volume: Some(1000),
        traffic,
    }
}

fn domain(name: &str, rank: f64) -> DomainPoint {
    DomainPoint {
        domain: name.to_owned(),
        rank: Some(rank),
        backlinks: 3.0,
    }
}

fn metrics(traffic: f64, top10: f64) -> Metrics {
    Metrics {
        keywords: 300.0,
        traffic,
        traffic_value_usd: traffic * 0.4,
        pos_1: 2.0,
        pos_2_3: 3.0,
        pos_4_10: top10 - 5.0,
        pos_11_20: 20.0,
        pos_21_100: 100.0,
    }
}

pub(crate) fn before() -> SnapshotData {
    SnapshotData {
        version: DATA_VERSION,
        domain: "ornek.com".to_owned(),
        market: "tr-tr".to_owned(),
        overview: Some(metrics(1240.0, 20.0)),
        keywords: Some(vec![
            keyword("spor ayakkabı", Some(12), 40.0),
            keyword("koşu ayakkabısı", Some(4), 90.0),
            keyword("bot", Some(30), 2.0),
        ]),
        keywords_total: Some(300.0),
        pages: Some(vec![PagePoint {
            url: "https://ornek.com/a".to_owned(),
            traffic: 100.0,
            keywords: 10.0,
        }]),
        links: Some(LinkPoint {
            rank: Some(31.0),
            backlinks: 900.0,
            referring_domains: 120.0,
            broken_backlinks: 4.0,
        }),
        domains: Some(vec![domain("eski.com", 40.0), domain("kalan.com", 20.0)]),
        competitors: vec![CompetitorPoint {
            domain: "rakip.com".to_owned(),
            overview: Some(metrics(5000.0, 60.0)),
            links: None,
        }],
        rank: Some(RankPoint {
            visibility: Some(12.0),
            average_position: Some(18.0),
            top10: 3.0,
            keywords: vec![TrackedPoint {
                keyword: "spor ayakkabı".to_owned(),
                position: Some(12),
                url: None,
            }],
            ..RankPoint::default()
        }),
        audit: Some(AuditPoint {
            health: Some(71.0),
            geo: Some(40.0),
            pages: 300.0,
            issues: 50.0,
            finished_at: None,
            checks: vec![
                IssuePoint {
                    id: "title_missing".to_owned(),
                    title: "Missing title".to_owned(),
                    count: 12.0,
                },
                IssuePoint {
                    id: "slow".to_owned(),
                    title: "Slow".to_owned(),
                    count: 5.0,
                },
            ],
        }),
        ai: Some(AiPoint {
            answers: 10.0,
            mention_rate: 20.0,
            citation_rate: 5.0,
            share_of_voice: 10.0,
            average_position: None,
            cells: vec![CellPoint {
                prompt: "En iyi spor ayakkabı?".to_owned(),
                engine: "chatgpt".to_owned(),
                mentioned: false,
                position: None,
            }],
            brands: vec![BrandPoint {
                key: "ornek.com".to_owned(),
                own: true,
                share: 10.0,
            }],
        }),
        gsc: None,
        local: None,
        link_audit: None,
        missing: Vec::new(),
    }
}

pub(crate) fn after() -> SnapshotData {
    let mut data = before();
    data.overview = Some(metrics(1525.0, 26.0));
    data.keywords = Some(vec![
        keyword("spor ayakkabı", Some(6), 120.0),
        keyword("koşu ayakkabısı", Some(5), 80.0),
        keyword("yeni kelime", Some(9), 30.0),
    ]);
    data.pages = Some(vec![
        PagePoint {
            url: "https://ornek.com/a".to_owned(),
            traffic: 160.0,
            keywords: 12.0,
        },
        PagePoint {
            url: "https://ornek.com/b".to_owned(),
            traffic: 20.0,
            keywords: 2.0,
        },
    ]);
    data.links = Some(LinkPoint {
        rank: Some(34.0),
        backlinks: 1100.0,
        referring_domains: 132.0,
        broken_backlinks: 2.0,
    });
    data.domains = Some(vec![domain("kalan.com", 20.0), domain("yeni.com", 55.0)]);
    data.competitors[0].overview = Some(metrics(6000.0, 70.0));
    data.rank.as_mut().unwrap().visibility = Some(19.0);
    data.rank.as_mut().unwrap().keywords[0].position = Some(6);
    let audit = data.audit.as_mut().unwrap();
    audit.health = Some(84.0);
    audit.checks = vec![
        IssuePoint {
            id: "slow".to_owned(),
            title: "Slow".to_owned(),
            count: 7.0,
        },
        IssuePoint {
            id: "h1_missing".to_owned(),
            title: "Missing H1".to_owned(),
            count: 3.0,
        },
    ];
    let ai = data.ai.as_mut().unwrap();
    ai.mention_rate = 35.0;
    ai.cells[0].mentioned = true;
    ai.cells[0].position = Some(2.0);
    ai.brands[0].share = 18.0;
    data
}

#[test]
fn compares_every_number_and_what_moved() {
    let (from, to) = (before(), after());
    let comparison = compare(
        (&side(1, "2026-10-01T09:00:00Z"), &from),
        (&side(2, "2026-11-01T09:00:00Z"), &to),
    );
    assert_eq!(comparison.days, 31);
    let traffic = comparison
        .kpis
        .iter()
        .find(|kpi| kpi.key == "traffic")
        .unwrap();
    assert_eq!(
        (traffic.before, traffic.after),
        (Some(1240.0), Some(1525.0))
    );
    assert!((traffic.percent.unwrap() - 22.983_870_967_741_936).abs() < 1e-9);
    assert_eq!(traffic.good(), Some(true));
    let issues = comparison
        .kpis
        .iter()
        .find(|kpi| kpi.key == "issues")
        .unwrap();
    assert_eq!(issues.good(), None);
    // Search Console was never taken: not compared.
    assert!(comparison.kpis.iter().all(|kpi| kpi.group != "gsc"));

    let keywords = comparison.keywords.unwrap();
    assert_eq!(
        (
            keywords.gained,
            keywords.lost,
            keywords.improved,
            keywords.declined
        ),
        (1, 1, 1, 1)
    );
    assert_eq!((keywords.entered_top10, keywords.left_top10), (2, 0));
    assert_eq!(keywords.new_keywords[0].keyword, "yeni kelime");
    assert_eq!(keywords.lost_keywords[0].keyword, "bot");
    assert_eq!(keywords.improved_keywords[0].change, Some(6));
    assert_eq!(keywords.declined_keywords[0].change, Some(-1));

    let pages = comparison.pages.unwrap();
    assert_eq!(pages.up[0].url, "https://ornek.com/a");
    assert_eq!(pages.up.len(), 2);
    let domains = comparison.domains.unwrap();
    assert_eq!((domains.gained, domains.lost), (1, 1));
    assert_eq!(domains.gained_domains[0].domain, "yeni.com");
    assert_eq!(domains.lost_domains[0].domain, "eski.com");
    let tracked = comparison.tracked.unwrap();
    assert_eq!((tracked.improved, tracked.declined), (1, 0));
    let audit = comparison.audit.unwrap();
    assert_eq!(audit.fixed[0].id, "title_missing");
    assert_eq!(audit.new_issues[0].id, "h1_missing");
    assert_eq!(audit.worse[0].id, "slow");
    let ai = comparison.ai.unwrap();
    assert_eq!(ai.gained.len(), 1);
    assert_eq!(ai.brands[0].after, 18.0);
    assert_eq!(comparison.sites.len(), 2);
    assert!(comparison.sites[0].own);
    assert_eq!(comparison.sites[1].domain, "rakip.com");
}

#[test]
fn tells_the_comparison_in_four_languages() {
    let comparison = compare(
        (&side(1, "2026-10-01T09:00:00Z"), &before()),
        (&side(2, "2026-11-01T09:00:00Z"), &after()),
    );
    let turkish = summarize(&comparison, "tr");
    assert_eq!(turkish[0].tone, "good");
    assert_eq!(turkish[0].text, "Organik trafik %23 arttı (1.240 → 1.525).");
    assert!(turkish.iter().any(|s| s.text == "2 kelime ilk 10'a girdi."));
    assert!(
        turkish
            .iter()
            .any(|s| s.text == "Site sağlığı 71 puandan 84 puana çıktı.")
    );
    assert!(
        turkish
            .iter()
            .any(|s| s.tone == "bad" && s.text == "1 alan adının linki artık yok.")
    );
    assert!(
        turkish
            .iter()
            .any(|s| s.tone == "bad" && s.text == "Rakip rakip.com trafiğini %20 artırdı.")
    );
    assert!(turkish.len() <= 8);
    let english = summarize(&comparison, "en");
    assert_eq!(english[0].text, "Organic traffic rose 23% (1,240 → 1,525).");
    assert!(
        summarize(&comparison, "de")[0]
            .text
            .starts_with("Der organische Traffic stieg")
    );
    assert!(
        summarize(&comparison, "es")[0]
            .text
            .starts_with("El tráfico orgánico subió")
    );

    let same = compare(
        (&side(1, "2026-10-01"), &before()),
        (&side(2, "2026-10-02"), &before()),
    );
    assert_eq!(summarize(&same, "tr")[0].tone, "note");
}

#[test]
fn reads_the_modules_answers() {
    let overview = serde_json::json!({ "target": "ornek.com", "organic": { "keywords": 300, "traffic": 1240.5, "traffic_value_usd": 99.0, "pos_1": 2, "pos_2_3": 3, "pos_4_10": 15, "pos_11_20": 20, "pos_21_100": 100 }, "paid": {} });
    let read = metrics_of(&overview).unwrap();
    assert_eq!((read.traffic, read.top10()), (1240.5, 20.0));
    let rank = serde_json::json!({ "summary": { "visibility": 12.5, "checked": 4, "top10": 2 }, "rows": [{ "keyword": "bot", "position": 4, "url": "https://ornek.com/bot" }], "competitors": [{ "domain": "rakip.com", "own": false, "visibility": 30, "top10": 3 }] });
    let point = rank_of(&rank).unwrap();
    assert_eq!(point.keywords[0].position, Some(4));
    assert_eq!(point.competitors[0].domain, "rakip.com");
    assert_eq!(
        rank_of(&serde_json::json!({ "summary": { "checked": 0 }, "rows": [] })),
        None
    );
    let ai = serde_json::json!({ "summary": { "answers": 6, "mention_rate": 50 }, "prompts": [{ "prompt": "soru", "results": { "chatgpt": { "mentioned": true, "position": 1, "error": null }, "gemini": { "mentioned": false, "error": "refused" } } }], "brands": [] });
    assert_eq!(ai_of(&ai).unwrap().cells.len(), 1);
    let parts = parts(7, &["rakip.com".to_owned()]);
    assert_eq!(parts.len(), 7);
    assert_eq!(parts[1].arguments["limit"], LIST_ROWS);
    assert_eq!(parts[6].arguments["target"], "rakip.com");
}

fn grid(solv: f64, rating: f64, keywords: &[(&str, f64, f64)]) -> LocalPoint {
    LocalPoint {
        arp: Some(8.0),
        solv,
        found: 30.0,
        points: 49.0,
        rating: Some(rating),
        votes: Some(120.0),
        keywords: keywords
            .iter()
            .map(|(keyword, arp, solv)| LocalKeywordPoint {
                keyword: (*keyword).to_owned(),
                arp: Some(*arp),
                solv: *solv,
            })
            .collect(),
    }
}

#[test]
#[allow(clippy::too_many_lines)] // reading, comparing and telling both parts
fn the_map_grid_and_the_link_audit_are_compared_and_told() {
    let grid_answer = json!({
        "run": { "id": 3 },
        "summary": { "arp": 6.4, "solv": 41.0, "found": 40, "points": 49 },
        "keywords": [{ "keyword": "kunduracı", "summary": { "arp": 3.1, "solv": 60.0 } }]
    });
    let reviews =
        json!({ "ratings": [{ "rating": 4.1, "votes": 80 }, { "rating": 4.3, "votes": 96 }] });
    let local = local_of(&grid_answer, Some(&reviews)).unwrap();
    assert_eq!(
        (local.solv, local.rating, local.votes),
        (41.0, Some(4.3), Some(96.0))
    );
    assert_eq!(local.keywords[0].arp, Some(3.1));
    assert!(
        local_of(&json!({ "run": null }), None).is_none(),
        "no grid run yet"
    );

    let audit = json!([
        { "domain": "a.xyz", "risk": "high", "score": 90 },
        { "domain": "b.top", "risk": "high", "score": 70 },
        { "domain": "c.net", "risk": "medium", "score": 40 },
        { "domain": "d.com", "risk": "low", "score": 2 }
    ]);
    let decisions = json!([{ "domain": "b.top", "decision": "disavow" }]);
    let links = link_audit_of(&audit, &decisions).unwrap();
    assert_eq!(
        (links.judged, links.high, links.medium, links.disavowed),
        (4.0, 2.0, 1.0, 1.0)
    );
    assert_eq!(
        links.risky,
        ["a.xyz"],
        "disavowed sites are not risky any more"
    );
    assert!(link_audit_of(&json!([]), &Value::Null).is_none());

    let mut from = before();
    let mut to = after();
    from.local = Some(grid(
        30.0,
        4.1,
        &[("kunduracı", 9.0, 20.0), ("ayakkabıcı", 4.0, 70.0)],
    ));
    to.local = Some(grid(
        45.0,
        4.3,
        &[("kunduracı", 4.5, 55.0), ("ayakkabıcı", 4.0, 65.0)],
    ));
    from.link_audit = Some(LinkAuditPoint {
        judged: 100.0,
        high: 3.0,
        medium: 5.0,
        disavowed: 0.0,
        risky: vec!["a.xyz".to_owned(), "b.top".to_owned(), "c.icu".to_owned()],
    });
    to.link_audit = Some(LinkAuditPoint {
        judged: 110.0,
        high: 4.0,
        medium: 5.0,
        disavowed: 2.0,
        risky: vec!["a.xyz".to_owned(), "e.biz".to_owned()],
    });
    let comparison = compare(
        (&side(1, "2026-09-01T09:00:00Z"), &from),
        (&side(2, "2026-10-01T09:00:00Z"), &to),
    );
    let kpi = |key: &str| comparison.kpis.iter().find(|kpi| kpi.key == key).unwrap();
    assert_eq!(kpi("local_solv").good(), Some(true));
    assert_eq!(
        kpi("local_rating")
            .change
            .map(|c| (c * 10.0).round() / 10.0),
        Some(0.2)
    );
    assert_eq!(kpi("risky_links").before, Some(3.0));
    assert_eq!(kpi("risky_links").after, Some(2.0));
    assert_eq!(
        kpi("risky_links").good(),
        Some(true),
        "fewer risky sites is better"
    );
    let local = comparison.local.as_ref().unwrap();
    assert_eq!((local.improved, local.declined), (1, 1));
    assert_eq!(
        local.keywords[0].keyword, "kunduracı",
        "the largest move first"
    );
    let links = comparison.link_audit.as_ref().unwrap();
    assert_eq!(links.new_risky, ["e.biz"]);
    assert_eq!(links.cleared, ["b.top", "c.icu"]);

    let told: Vec<String> = summarize(&comparison, "en")
        .into_iter()
        .map(|s| s.text)
        .collect();
    assert!(
        told.iter()
            .any(|t| t.starts_with("Share of local voice on the map rose from")),
        "{told:?}"
    );
    assert!(
        told.contains(
            &"1 new risky sites link to the site; review them and disavow where needed.".to_owned()
        ),
        "{told:?}"
    );
    assert_eq!(to.headline().risky_links, Some(2.0));
    assert_eq!(to.headline().solv, Some(45.0));
}
