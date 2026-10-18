use octopuson_core::table::{Cell, TableBuffer};

use super::*;
use crate::test_support::Fixture;

fn text(value: &str) -> Cell {
    Cell::Text(value.into())
}

#[test]
fn url_lists_reveal_uncrawled_and_orphan_pages() {
    let fixture = Fixture::imported();
    let list = fixture.write(
        "crawl.csv",
        "\"Address\",\"Status Code\"\n\"https://acme.com/a\",\"200\"\n\"https://acme.com/new\",\"200\"\n\"https://other.com/x\",\"200\"\n",
    );
    let mut table = TableBuffer::default();
    let request = UrlListRequest {
        site: "acme.com".into(),
        sources: vec![UrlSource::File(list)],
        filter: Some(String::new()),
        normalize: NormalizeOptions::default(),
        host: Some("ACME.com".into()),
    };
    let report = compare_url_list(&fixture.workspace, &request, &mut table).unwrap();
    assert_eq!(report.skipped, 1, "other hosts are skipped");
    assert_eq!(
        (
            report.summary.both,
            report.summary.not_in_logs,
            report.summary.not_in_list
        ),
        (1, 1, 2)
    );
    let presence: Vec<(Cell, Cell)> = table
        .rows
        .iter()
        .map(|row| (row[0].clone(), row[1].clone()))
        .collect();
    assert_eq!(
        presence,
        [
            (text("/a"), text("both")),
            (text("/new"), text("not-in-logs")),
            (text("/missing"), text("not-in-list")),
            (text("/a?x=1"), text("not-in-list")),
        ]
    );
    assert_eq!(table.rows[2][4], text("404"));
    // The list's other columns are joined to every URL (numbers typed as numbers).
    assert_eq!(report.list_columns, ["Status Code"]);
    assert_eq!(table.columns.last().unwrap().name, "Status Code");
    assert_eq!(
        table.columns.last().unwrap().kind,
        octopuson_core::table::ColumnKind::Integer
    );
    assert_eq!(table.rows[0].last(), Some(&Cell::Integer(200)));
    assert_eq!(table.rows[2].last(), Some(&Cell::Null));

    // The default filter only counts verified search-engine crawlers.
    let crawlers = UrlListRequest {
        filter: None,
        ..request
    };
    let report =
        compare_url_list(&fixture.workspace, &crawlers, &mut TableBuffer::default()).unwrap();
    assert_eq!(report.filter, DEFAULT_CRAWLER_FILTER);
    assert_eq!(report.summary.not_in_list, 1);
}

#[test]
fn empty_lists_are_rejected() {
    let fixture = Fixture::imported();
    let request = UrlListRequest {
        site: "acme.com".into(),
        sources: vec![UrlSource::File(fixture.write("empty.txt", "\n"))],
        filter: None,
        normalize: NormalizeOptions::default(),
        host: None,
    };
    let result = compare_url_list(&fixture.workspace, &request, &mut TableBuffer::default());
    assert!(matches!(result, Err(AnalysisError::EmptyList(_))));
}

#[test]
fn periods_compare_hits_per_key() {
    let fixture = Fixture::imported();
    let mut table = TableBuffer::default();
    let request = PeriodRequest {
        site: "acme.com".into(),
        before: "2026-10-07".into(),
        after: "2026-10-08".into(),
        group: "url".into(),
        filter: None,
    };
    let report = compare_periods(&fixture.workspace, &request, &mut table).unwrap();
    assert_eq!(
        (report.summary.before_hits, report.summary.after_hits),
        (3, 3)
    );
    assert_eq!((report.summary.lost, report.summary.new), (1, 1));
    assert_eq!(table.columns[0].name, "url");
    let invalid = PeriodRequest {
        before: "yesterday".into(),
        ..request
    };
    assert!(matches!(
        compare_periods(&fixture.workspace, &invalid, &mut table),
        Err(AnalysisError::Query(_))
    ));
}

#[test]
fn overview_summarizes_crawl_budget() {
    let fixture = Fixture::imported();
    let request = OverviewRequest {
        site: "acme.com".into(),
        period: None,
    };
    let overview = crawl_overview(&fixture.workspace, &request)
        .unwrap()
        .overview;
    assert_eq!(overview.total_hits, 6);
    assert_eq!(overview.search.hits, 2);
    assert_eq!(overview.search.unique_urls, 2);
    assert!((overview.search.non_success_ratio - 0.5).abs() < 1e-9);
    assert_eq!(overview.spoofed_hits, 1);
    let googlebot = overview
        .bots
        .iter()
        .find(|b| b.bot == "googlebot-desktop")
        .unwrap();
    assert_eq!((googlebot.verified, googlebot.unverified), (2, 1));
    let browsers = overview
        .categories
        .iter()
        .find(|c| c.name == "browser")
        .unwrap();
    assert_eq!(browsers.value, 2);
}

#[test]
fn robots_compliance_finds_bots_ignoring_disallow_rules() {
    let fixture = Fixture::imported();
    let robots = fixture.write(
        "robots.txt",
        "User-agent: *\nDisallow: /missing\n\nUser-agent: GPTBot\nDisallow: /\n\nSitemap: https://acme.com/sitemap.xml\n",
    );
    let mut table = TableBuffer::default();
    let request = RobotsRequest {
        site: "acme.com".into(),
        robots: Some(robots.display().to_string()),
        filter: None,
    };
    let report = robots_compliance(&fixture.workspace, &request, &mut table).unwrap();
    assert_eq!(report.sitemaps, ["https://acme.com/sitemap.xml"]);
    let gpt = report.bots.iter().find(|b| b.bot == "gptbot").unwrap();
    assert!(gpt.blocked);
    assert_eq!(gpt.group.as_deref(), Some("gptbot"));
    assert_eq!(
        (gpt.hits, gpt.disallowed_hits, gpt.disallowed_urls),
        (1, 1, 1)
    );
    let google = report
        .bots
        .iter()
        .find(|b| b.bot == "googlebot-desktop")
        .unwrap();
    assert_eq!(google.group.as_deref(), Some("*"));
    assert!(!google.blocked);
    // The verified Googlebot request for /missing is disallowed; /a is allowed.
    assert_eq!(
        (
            google.hits,
            google.disallowed_hits,
            google.verified_disallowed_hits
        ),
        (3, 1, 1)
    );
    assert_eq!(report.rows, 2);
    assert!(
        table
            .rows
            .iter()
            .any(|row| row[0] == text("/missing") && row[6] == text("Disallow: /missing"))
    );

    let missing = robots_compliance(
        &fixture.workspace,
        &RobotsRequest {
            robots: Some(fixture.path("nope.txt").display().to_string()),
            ..request
        },
        &mut TableBuffer::default(),
    );
    assert!(matches!(missing, Err(AnalysisError::Read { .. })));
}

#[test]
fn robots_compliance_matches_utf8_rules_and_keeps_the_logged_url_and_rule() {
    let fixture = Fixture::new();
    fixture.import("encoded.log", "66.249.66.1 - - [07/Oct/2026:18:39:53 +0000] \"GET /%C3%B6zel/gizli HTTP/1.1\" 200 535 \"-\" \"Googlebot/2.1\"\n66.249.66.1 - - [07/Oct/2026:18:40:00 +0000] \"GET /%C3%B6zel/a%C3%A7%C4%B1k HTTP/1.1\" 200 535 \"-\" \"Googlebot/2.1\"\n");
    let rules = fixture.write(
        "utf8-robots.txt",
        "\u{feff}User-agent: *\rDisallow: /özel/\rAllow: /özel/açık$\r",
    );
    let mut table = TableBuffer::default();
    let report = robots_compliance(
        &fixture.workspace,
        &RobotsRequest {
            site: "acme.com".into(),
            robots: Some(rules.display().to_string()),
            filter: None,
        },
        &mut table,
    )
    .unwrap();
    assert_eq!(report.rows, 1);
    assert_eq!(table.rows[0][0], text("/%C3%B6zel/gizli"));
    assert_eq!(table.rows[0][6], text("Disallow: /özel/"));
    let google = report
        .bots
        .iter()
        .find(|bot| bot.bot == "googlebot-desktop")
        .unwrap();
    assert_eq!(google.hits, 2);
    assert_eq!(google.disallowed_hits, 1);
    assert_eq!(google.disallowed_urls, 1);
}

#[test]
fn robots_compliance_preserves_shared_groups_across_ignored_records() {
    let fixture = Fixture::new();
    fixture.import("grouped.log", "66.249.66.1 - - [07/Oct/2026:18:39:53 +0000] \"GET /private/hidden HTTP/1.1\" 200 535 \"-\" \"Googlebot/2.1\"\n66.249.66.1 - - [07/Oct/2026:18:40:00 +0000] \"GET /private/public/page HTTP/1.1\" 200 535 \"-\" \"Googlebot/2.1\"\n");
    let rules = fixture.write("grouped-robots.txt", "User-agent: Googlebot\nCrawl-delay: 10\nHost: acme.com\nUser-agent: Bingbot\nDisallow: /private/\nAllow: /private/public/\n");
    let mut table = TableBuffer::default();
    let report = robots_compliance(
        &fixture.workspace,
        &RobotsRequest {
            site: "acme.com".into(),
            robots: Some(rules.display().to_string()),
            filter: None,
        },
        &mut table,
    )
    .unwrap();
    assert_eq!(report.rows, 1);
    assert_eq!(table.rows[0][0], text("/private/hidden"));
    assert_eq!(table.rows[0][6], text("Disallow: /private/"));
    let google = report
        .bots
        .iter()
        .find(|bot| bot.bot == "googlebot-desktop")
        .unwrap();
    assert_eq!(google.group.as_deref(), Some("googlebot"));
    assert_eq!(
        (google.hits, google.disallowed_hits, google.disallowed_urls),
        (2, 1, 1)
    );
}
