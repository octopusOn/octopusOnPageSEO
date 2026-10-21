//! Backlink reports read from data shaped like DataForSEO's documented examples.

use serde_json::{Value, json};

use crate::DfsError;
use crate::backlinks::*;

fn result(fixture: &str) -> Vec<Value> {
    let answer: Value = serde_json::from_str(fixture).unwrap();
    answer["tasks"][0]["result"].as_array().unwrap().clone()
}

#[test]
fn a_link_profile_is_read_with_its_shares_most_first() {
    let summary = summary_from_result(&result(include_str!(
        "../tests/fixtures/backlinks_summary.json"
    )));
    assert_eq!(summary.target, "example.com");
    assert_eq!(summary.rank, Some(48));
    assert_eq!(summary.backlinks, 182_340);
    assert_eq!(summary.referring_domains, 2_310);
    assert_eq!(summary.referring_domains_nofollow, 402);
    assert_eq!(summary.broken_backlinks, 312);
    assert_eq!(summary.spam_score, Some(7));
    let tlds: Vec<(&str, u64)> = summary
        .top_level_domains
        .iter()
        .map(|s| (s.name.as_str(), s.count))
        .collect();
    assert_eq!(
        tlds,
        [("com", 1201), ("com.tr", 640), ("net", 120), ("org", 80)]
    );
    // Zero counts are left out.
    assert!(
        summary
            .link_types
            .iter()
            .all(|share| share.name != "canonical")
    );
    assert_eq!(summary.countries[0].name, "TR");
    assert_eq!(summary.attributes[0].name, "nofollow");
}

#[test]
fn backlinks_carry_their_kind_follow_and_state() {
    let rows = backlinks_from_result(&result(include_str!(
        "../tests/fixtures/backlinks_backlinks.json"
    )));
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].domain_from.as_deref(), Some("blog.news.com.tr"));
    assert_eq!(rows[0].anchor.as_deref(), Some("spor ayakkabı"));
    assert!(rows[0].dofollow);
    assert!(rows[0].is_new);
    assert_eq!(rows[0].page_rank, Some(61));
    assert_eq!(rows[0].domain_rank, Some(72));
    // An image link reads its alt text as the anchor.
    assert_eq!(rows[1].anchor.as_deref(), Some("logo"));
    assert_eq!(rows[1].kind.as_deref(), Some("image"));
    assert!(rows[1].is_broken);
    assert_eq!(rows[1].url_to_status, Some(404));
    assert!(!rows[1].dofollow);
}

#[test]
fn domains_anchors_pages_and_time_are_read() {
    let domains = referring_domains_from_result(&result(include_str!(
        "../tests/fixtures/backlinks_referring_domains.json"
    )));
    assert_eq!(domains[0].domain, "news.com.tr");
    assert_eq!(domains[0].rank, Some(72));
    assert!(!domains[0].nofollow_only);
    assert!(domains[1].nofollow_only);

    let anchors = anchors_from_result(&result(include_str!(
        "../tests/fixtures/backlinks_anchors.json"
    )));
    assert_eq!(anchors[0].anchor, "example");
    assert_eq!(anchors[0].referring_domains, 620);
    assert_eq!(anchors[1].anchor, "");

    let pages = pages_from_result(&result(include_str!(
        "../tests/fixtures/backlinks_domain_pages.json"
    )));
    assert_eq!(pages[0].title.as_deref(), Some("Örnek Mağaza"));
    assert_eq!(pages[0].referring_domains, 1_500);
    assert_eq!(pages[0].rank, Some(55));

    let points = new_lost_from_result(&result(include_str!(
        "../tests/fixtures/backlinks_new_lost.json"
    )));
    let dates: Vec<&str> = points.iter().map(|point| point.date.as_str()).collect();
    assert_eq!(dates, ["2026-08-31", "2026-09-30"]);
    assert_eq!(points[1].new_referring_domains, 40);

    let ranks = bulk_ranks_from_result(&result(include_str!(
        "../tests/fixtures/backlinks_bulk_ranks.json"
    )));
    assert_eq!(
        ranks[1],
        RankRow {
            target: "rival.com".to_owned(),
            rank: Some(61)
        }
    );
}

#[test]
fn targets_stay_pages_or_become_domains() {
    assert_eq!(target("https://www.Example.com/").unwrap(), "example.com");
    assert_eq!(target("www.example.com").unwrap(), "example.com");
    assert_eq!(
        target("https://example.com/blog/yazi").unwrap(),
        "https://example.com/blog/yazi"
    );
    assert!(matches!(target("  "), Err(DfsError::Config(_))));
}

#[test]
fn backlink_lists_filter_by_state_and_follow() {
    let live = backlinks_task("example.com", BacklinkQuery::default(), 100).unwrap();
    assert_eq!(live["mode"], "as_is");
    assert_eq!(live["backlinks_status_type"], "live");
    assert_eq!(live["rank_scale"], "one_hundred");
    assert_eq!(live["order_by"], json!(["page_from_rank,desc"]));
    assert_eq!(live.get("filters"), None);

    let new_followed = backlinks_task(
        "example.com",
        BacklinkQuery {
            status: LinkStatus::New,
            dofollow_only: true,
            one_per_domain: true,
        },
        5_000,
    )
    .unwrap();
    assert_eq!(new_followed["limit"], 1_000);
    assert_eq!(new_followed["mode"], "one_per_domain");
    assert_eq!(
        new_followed["filters"],
        json!([["is_new", "=", true], "and", ["dofollow", "=", true]])
    );

    let lost = backlinks_task(
        "example.com",
        BacklinkQuery {
            status: LinkStatus::Lost,
            ..BacklinkQuery::default()
        },
        100,
    )
    .unwrap();
    assert_eq!(lost["backlinks_status_type"], "lost");
    let broken = backlinks_task(
        "example.com",
        BacklinkQuery {
            status: LinkStatus::Broken,
            ..BacklinkQuery::default()
        },
        100,
    )
    .unwrap();
    assert_eq!(broken["filters"], json!(["is_broken", "=", true]));

    let bulk = bulk_ranks_task(&[
        "https://www.example.com/".to_owned(),
        "example.com".to_owned(),
        "rival.com".to_owned(),
    ])
    .unwrap();
    assert_eq!(bulk["targets"], json!(["example.com", "rival.com"]));
    assert!(matches!(bulk_ranks_task(&[]), Err(DfsError::Config(_))));
    assert_eq!(
        new_lost_task("www.example.com", "2025-10-01").unwrap()["group_range"],
        "month"
    );
}
