use crate::rank::*;

fn check(checked_at: &str, position: Option<u16>) -> Check<'_> {
    Check {
        checked_at,
        position,
        url: position.map(|_| "https://example.com/page"),
        pages: usize::from(position.is_some()),
        error: None,
    }
}

#[test]
fn schedules_click_shares_and_percentages() {
    assert_eq!(Schedule::parse("weekly"), Schedule::Weekly);
    assert_eq!(Schedule::parse("manual"), Schedule::Manual);
    assert_eq!(Schedule::parse("anything"), Schedule::Daily);
    assert_eq!(Schedule::Weekly.as_str(), "weekly");
    assert_eq!(Schedule::Manual.interval_secs(), None);
    assert_eq!(Schedule::Daily.per_month(), 30);
    assert!(ctr(1) > ctr(2));
    assert_eq!(ctr(15), 0.01);
    assert_eq!(ctr(0), 0.0);
    assert_eq!(ctr(40), 0.0);
    assert_eq!(share_of(1.0, 3.0), 33.3);
    assert_eq!(share_of(1.0, 0.0), 0.0);
}

#[test]
fn reads_utc_timestamps() {
    assert_eq!(seconds_of("1970-01-01T00:00:00Z"), Some(0));
    assert_eq!(seconds_of("2000-03-01T00:00:00Z"), Some(951_868_800));
    assert_eq!(seconds_of("2024-02-29T12:30:15Z"), Some(1_709_209_815));
    let day = seconds_of("2026-10-18T00:00:00Z").unwrap();
    assert_eq!(seconds_of("2026-10-19T00:00:01Z"), Some(day + 86_401));
    assert_eq!(seconds_of("2026-02-29T00:00:00Z"), None);
    assert_eq!(seconds_of("2026-13-01T00:00:00Z"), None);
    assert_eq!(seconds_of("2026-10-18"), None);
    assert_eq!(seconds_of("nonsense"), None);
}

#[test]
fn a_trend_reads_positions_and_keeps_the_last_failure() {
    let mut second = check("2026-10-19T08:00:00Z", Some(3));
    second.pages = 2;
    let failed = Check {
        checked_at: "2026-10-20T08:00:00Z",
        position: None,
        url: None,
        pages: 0,
        error: Some("quota"),
    };
    let trend = trend(&[check("2026-10-18T08:00:00Z", Some(5)), second, failed]);
    assert_eq!(trend.position, Some(3));
    assert_eq!(trend.previous, Some(5));
    assert_eq!(trend.change, Some(2));
    assert_eq!(trend.best, Some(3));
    assert!(trend.cannibalised);
    assert_eq!(trend.checked_at.as_deref(), Some("2026-10-20T08:00:00Z"));
    assert_eq!(trend.error.as_deref(), Some("quota"));
    assert_eq!(
        trend.history,
        vec![
            ("2026-10-18".to_owned(), Some(5)),
            ("2026-10-19".to_owned(), Some(3))
        ]
    );
    assert_eq!(super::rank::trend(&[]), Trend::default());
}

#[test]
fn visibility_weighs_keywords_by_volume_and_takes_the_last_check_of_a_day() {
    let volumes = [100.0, 1.0];
    let weight = |key: &usize| volumes[*key];
    let points = visibility(
        [
            (0, check("2026-10-18T08:00:00Z", Some(4))),
            (0, check("2026-10-18T20:00:00Z", Some(1))),
            (1, check("2026-10-18T08:00:00Z", None)),
            (0, check("2026-10-19T08:00:00Z", Some(12))),
        ],
        &weight,
    );
    assert_eq!(points.len(), 2);
    assert_eq!(points[0].date, "2026-10-18");
    assert_eq!(points[0].visibility, share_of(28.0, 28.28));
    assert_eq!(
        (points[0].top3, points[0].top10, points[0].ranked),
        (1, 1, 1)
    );
    assert_eq!(points[1].visibility, share_of(1.0, 28.0));
    assert_eq!(
        (points[1].top3, points[1].top10, points[1].ranked),
        (0, 0, 1)
    );
}

#[test]
fn shares_count_a_domain_once_per_page_and_pick_the_most_visible() {
    let pages = vec![
        ResultPage {
            weight: 10.0,
            domains: vec![
                (1, "rival.com".to_owned()),
                (2, "example.com".to_owned()),
                (3, "rival.com".to_owned()),
            ],
        },
        ResultPage {
            weight: 1.0,
            domains: vec![(1, "example.com".to_owned()), (2, "other.com".to_owned())],
        },
    ];
    let shares = competitor_shares(&pages, "example.com", &[]);
    let domains: Vec<&str> = shares.iter().map(|share| share.domain.as_str()).collect();
    assert_eq!(domains, ["example.com", "rival.com", "other.com"]);
    assert!(shares[0].own);
    assert_eq!(shares[0].keywords, 2);
    assert_eq!((shares[0].top3, shares[0].top10), (2, 2));
    assert_eq!(
        (shares[1].top3, shares[1].top10),
        (1, 1),
        "one place per page"
    );
    assert_eq!(shares[0].average_position, Some(1.5));
    assert_eq!(shares[1].keywords, 1);
    assert_eq!(shares[1].visibility, share_of(2.8, 3.08));
    let listed = competitor_shares(&pages, "example.com", &["missing.com".to_owned()]);
    assert_eq!(listed[1].domain, "missing.com");
    assert_eq!(listed[1].visibility, 0.0);
    assert_eq!(listed[1].average_position, None);
    let fixed = shares_of(&pages, "example.com", &["other.com".to_owned()]);
    assert_eq!(fixed.len(), 1);
    assert!(!fixed[0].own);
}
