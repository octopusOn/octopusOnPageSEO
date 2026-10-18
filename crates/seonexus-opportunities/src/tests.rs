use super::*;
fn project() -> OpportunityProject {
    OpportunityProject {
        id: "test".into(),
        name: "Example".into(),
        site_url: "https://example.com/".into(),
        ..Default::default()
    }
}
fn request() -> AnalysisRequest {
    AnalysisRequest {
        project_id: "test".into(),
        ..Default::default()
    }
}
fn row(path: &str, clicks: f64, impressions: f64, position: f64) -> PerformanceRow {
    PerformanceRow {
        url: Some(format!("https://example.com/{path}")),
        clicks: Some(clicks),
        impressions: Some(impressions),
        position: Some(position),
        ..Default::default()
    }
}
fn data(rows: Vec<PerformanceRow>, current: bool, grain: DataGrain) -> PerformanceDataset {
    PerformanceDataset {
        id: format!("{current}{grain:?}"),
        project_id: "test".into(),
        source: DataSource::Gsc,
        grain,
        range: if current {
            request().current
        } else {
            request().previous
        },
        coverage: DataCoverage::default(),
        rows,
        origin: "test".into(),
        created_at_ms: 0,
    }
}
#[test]
fn loss_requires_observed_comparable_rows_not_missing_urls() {
    let report = analyze(
        &project(),
        &request(),
        &[
            data(vec![row("a", 30.0, 1000.0, 8.0)], true, DataGrain::Pages),
            data(
                vec![
                    row("a", 60.0, 1000.0, 8.0),
                    row("missing", 100.0, 1000.0, 8.0),
                ],
                false,
                DataGrain::Pages,
            ),
        ],
    )
    .unwrap();
    assert_eq!(report.unknown_current_urls, 1);
    assert_eq!(report.opportunities.len(), 1);
    assert!(
        report.opportunities[0]
            .signals
            .iter()
            .any(|s| s.kind == "traffic-loss")
    );
}

#[test]
fn property_overview_uses_daily_totals_without_inventing_url_or_segment_counts() {
    let mut p = project();
    p.gsc_property = "sc-domain:example.com".into();
    let mut daily = data(
        vec![
            PerformanceRow {
                date: Some(request().current.start),
                clicks: Some(57.0),
                impressions: Some(1751.0),
                position: Some(29.8),
                ..Default::default()
            },
            PerformanceRow {
                date: Some(request().previous.start),
                clicks: Some(22.0),
                impressions: Some(2335.0),
                position: Some(39.4),
                ..Default::default()
            },
        ],
        true,
        DataGrain::Daily,
    );
    daily.range.start = request().previous.start;
    let pages = data(vec![row("a", 4.0, 39.0, 21.0)], true, DataGrain::Pages);
    let mut query_row = row("a", 2.0, 20.0, 8.0);
    query_row.query = Some("test query".into());
    query_row.country = Some("tur".into());
    let mut queries = data(vec![query_row], true, DataGrain::QueryPages);
    queries.coverage.warnings.push("gsc-top-rows".into());
    let rows = vec![pages, queries, daily];
    let report = analyze(&p, &request(), &rows).unwrap();
    assert_eq!(report.current.as_ref().unwrap().clicks, 57.0);
    assert_eq!(report.previous.as_ref().unwrap().clicks, 22.0);
    assert_eq!(report.current.as_ref().unwrap().impressions, 1751.0);
    assert!(
        report
            .warnings
            .iter()
            .any(|w| w == "property-overview-totals")
    );
    let mut segment = request();
    segment.country = "tur".into();
    let report = analyze(&p, &segment, &rows).unwrap();
    assert_eq!(report.current.as_ref().unwrap().clicks, 2.0);
    assert!(report.warnings.iter().any(|w| w == "query-subset-totals"));
    assert!(report.warnings.iter().any(|w| w == "loss-query-subset"));
    assert!(report.daily.is_empty());
    p.site_url = "https://example.com/tr/".into();
    let report = analyze(&p, &request(), &rows).unwrap();
    assert!(report.current.is_none());
    assert!(
        !report
            .warnings
            .iter()
            .any(|w| w == "property-overview-totals")
    );
}
#[test]
fn weighted_position_and_ctr_are_recomputed_from_impressions() {
    let report = analyze(
        &project(),
        &request(),
        &[data(
            vec![row("a", 10.0, 100.0, 2.0), row("a", 10.0, 900.0, 10.0)],
            true,
            DataGrain::Pages,
        )],
    )
    .unwrap();
    let metrics = report.current.unwrap();
    assert_eq!(metrics.ctr, Some(0.02));
    assert!((metrics.position.unwrap() - 9.2).abs() < 1e-9);
}
#[test]
fn sparse_ctr_is_review_only_and_unknown_position_never_becomes_rank_zero() {
    let mut unknown = row("unknown", 0.0, 2000.0, 1.0);
    unknown.position = None;
    let report = analyze(
        &project(),
        &request(),
        &[data(
            vec![row("a", 0.0, 2000.0, 8.0), unknown],
            true,
            DataGrain::Pages,
        )],
    )
    .unwrap();
    assert_eq!(report.opportunities.len(), 1);
    assert_eq!(report.opportunities[0].confidence, "review");
    assert_eq!(report.opportunities[0].impact_clicks, None);
}
#[test]
fn own_url_is_excluded_from_ctr_peers_and_brand_country_device_do_not_mix() {
    let mut rows: Vec<_> = (0..10)
        .map(|i| {
            let mut r = row(&format!("peer{i}"), 100.0, 1000.0, 8.0);
            r.query = Some("other search".into());
            r
        })
        .collect();
    let mut candidate = row("candidate", 10.0, 2000.0, 8.0);
    candidate.query = Some("search".into());
    rows.push(candidate);
    let report = analyze(
        &project(),
        &request(),
        &[data(rows, true, DataGrain::QueryPages)],
    )
    .unwrap();
    let item = report
        .opportunities
        .iter()
        .find(|o| o.url.ends_with("candidate"))
        .unwrap();
    let signal = item.signals.iter().find(|s| s.kind == "low-ctr").unwrap();
    assert_eq!(signal.peer_urls, 10);
    assert_eq!(signal.benchmark_ctr, Some(0.1));
    assert_eq!(signal.estimated_clicks, Some(190.0));
}
#[test]
fn three_signals_share_one_url_and_impact_is_maximum_not_sum() {
    let mut current = row("a", 20.0, 2000.0, 8.0);
    current.query = Some("topic".into());
    let report = analyze(
        &project(),
        &request(),
        &[
            data(vec![current], true, DataGrain::QueryPages),
            data(
                vec![row("a", 100.0, 2000.0, 8.0)],
                false,
                DataGrain::QueryPages,
            ),
        ],
    )
    .unwrap();
    assert_eq!(report.opportunities.len(), 1);
    assert_eq!(report.opportunities[0].signals.len(), 3);
    assert_eq!(report.opportunities[0].impact_clicks, Some(80.0));
}
#[test]
fn separate_query_and_page_exports_never_invent_a_query_page_relationship() {
    let mut query = PerformanceRow {
        query: Some("topic".into()),
        clicks: Some(20.0),
        impressions: Some(1000.0),
        position: Some(8.0),
        ..Default::default()
    };
    query.url = None;
    let report = analyze(
        &project(),
        &request(),
        &[
            data(vec![row("a", 10.0, 1000.0, 8.0)], true, DataGrain::Pages),
            data(vec![query], true, DataGrain::Queries),
        ],
    )
    .unwrap();
    assert!(
        report
            .opportunities
            .iter()
            .flat_map(|o| &o.signals)
            .all(|s| s.query.is_none())
    );
}
#[test]
fn unequal_periods_do_not_generate_traffic_loss() {
    let mut r = request();
    r.previous.start = r.previous.start.add_days(1);
    let mut before = data(vec![row("a", 100.0, 1000.0, 8.0)], false, DataGrain::Pages);
    before.range = r.previous;
    let report = analyze(
        &project(),
        &r,
        &[
            data(vec![row("a", 10.0, 1000.0, 8.0)], true, DataGrain::Pages),
            before,
        ],
    )
    .unwrap();
    assert!(
        report
            .opportunities
            .iter()
            .flat_map(|o| &o.signals)
            .all(|s| s.kind != "traffic-loss")
    );
}
#[test]
fn url_identity_keeps_language_query_case_and_encoded_path_meaning() {
    assert_eq!(
        normalized_url("https://example.com/tr/Service?services_page=2#x"),
        Some("https://example.com/tr/Service?services_page=2".into())
    );
    assert_ne!(
        normalized_url("https://example.com/tr/a"),
        normalized_url("https://example.com/en/a")
    );
    assert!(normalized_url("https://user:secret@example.com").is_none());
}
#[test]
fn missing_ga4_is_neutral_and_does_not_zero_out_priority() {
    let report = analyze(
        &project(),
        &request(),
        &[data(
            vec![row("a", 0.0, 1000.0, 8.0)],
            true,
            DataGrain::Pages,
        )],
    )
    .unwrap();
    assert!(report.opportunities[0].priority > 0.0);
    assert!(report.opportunities[0].business.is_none());
    assert_eq!(report.sessions, None);
}

#[test]
fn invalid_and_zero_impressions_never_create_a_missing_page_loss() {
    let mut invalid = row("invalid", 0.0, 0.0, 0.0);
    invalid.impressions = None;
    let report = analyze(
        &project(),
        &request(),
        &[
            data(
                vec![invalid, row("zero", 0.0, 0.0, 0.0)],
                true,
                DataGrain::Pages,
            ),
            data(
                vec![row("invalid", 100.0, 1000.0, 8.0)],
                false,
                DataGrain::Pages,
            ),
        ],
    )
    .unwrap();
    assert!(report.opportunities.is_empty());
    assert_eq!(report.unknown_current_urls, 1);
    assert_eq!(report.current.unwrap().ctr, None);
}
#[test]
fn different_origins_and_language_folders_are_never_merged() {
    let mut outside = row("outside", 10.0, 2000.0, 8.0);
    outside.url = Some("https://other.example/tr/".into());
    let report = analyze(
        &project(),
        &request(),
        &[data(
            vec![
                outside,
                row("tr/a?p=2", 10.0, 2000.0, 8.0),
                row("en/a?p=2", 20.0, 2000.0, 8.0),
            ],
            true,
            DataGrain::Pages,
        )],
    )
    .unwrap();
    assert_eq!(report.current.unwrap().clicks, 30.0);
    assert_eq!(report.opportunities.len(), 2);
}
fn ga4_dataset() -> PerformanceDataset {
    PerformanceDataset {
        source: DataSource::Ga4,
        grain: DataGrain::LandingPages,
        coverage: DataCoverage {
            google_organic: true,
            origin_confirmed: true,
            goal_key: "events:any".into(),
            ..Default::default()
        },
        rows: vec![PerformanceRow {
            url: Some("https://example.com/a".into()),
            sessions: Some(200.0),
            key_events: Some(10.0),
            goal_rate: Some(0.04),
            ..Default::default()
        }],
        ..data(Vec::new(), true, DataGrain::Pages)
    }
}
#[test]
fn ga4_business_values_are_page_only_and_incomparable_filters_remain_unknown() {
    let mut p = project();
    p.ga4_origin_confirmed = true;
    let pages = data(vec![row("a", 10.0, 2000.0, 8.0)], true, DataGrain::Pages);
    let ga = ga4_dataset();
    let report = analyze(&p, &request(), &[pages.clone(), ga.clone()]).unwrap();
    assert_eq!(report.sessions, Some(200.0));
    assert_eq!(
        report.opportunities[0].business.as_ref().unwrap().boost,
        2.0
    );
    let mut filtered = request();
    filtered.device = "mobile".into();
    let report = analyze(&p, &filtered, &[pages, ga]).unwrap();
    assert_eq!(report.sessions, None);
    assert!(report.opportunities.iter().all(|o| o.business.is_none()));
}
#[test]
fn failed_latest_ga4_sync_disables_stale_business_boost() {
    let mut p = project();
    p.ga4_origin_confirmed = true;
    let mut pages = data(vec![row("a", 10.0, 2000.0, 8.0)], true, DataGrain::Pages);
    pages
        .coverage
        .warnings
        .push("ga4-sync-unavailable: permission".into());
    let report = analyze(&p, &request(), &[pages, ga4_dataset()]).unwrap();
    assert_eq!(report.sessions, None);
    assert!(report.opportunities[0].business.is_none());
    assert!(report.warnings.contains(&"ga4-stale".into()));
}
#[test]
fn explicit_cancellation_does_not_return_a_partial_report() {
    assert!(
        analyze_with_cancel(
            &project(),
            &request(),
            &[data(
                vec![row("a", 10.0, 2000.0, 8.0)],
                true,
                DataGrain::Pages
            )],
            &|| true
        )
        .unwrap_err()
        .contains("cancelled")
    );
}
#[test]
fn repeated_daily_query_rows_are_aggregated_before_thresholds() {
    let mut a = row("a", 5.0, 600.0, 8.0);
    a.query = Some("topic".into());
    let report = analyze(
        &project(),
        &request(),
        &[data(vec![a.clone(), a], true, DataGrain::QueryPages)],
    )
    .unwrap();
    let signals = &report.opportunities[0].signals;
    assert_eq!(signals.iter().filter(|s| s.kind == "low-ctr").count(), 1);
    assert_eq!(signals[0].current.impressions, 1200.0);
}

#[test]
fn hard_api_row_caps_never_produce_confirmed_traffic_loss() {
    let mut current = data(vec![row("a", 10.0, 2000.0, 8.0)], true, DataGrain::Pages);
    current.coverage.limited = true;
    current.coverage.warnings.push("row-limit".into());
    let report = analyze(
        &project(),
        &request(),
        &[
            current,
            data(vec![row("a", 100.0, 2000.0, 8.0)], false, DataGrain::Pages),
        ],
    )
    .unwrap();
    assert!(
        report
            .opportunities
            .iter()
            .flat_map(|o| &o.signals)
            .all(|s| s.kind != "traffic-loss")
    );
    assert!(report.warnings.contains(&"loss-incomplete".into()));
}
#[test]
fn revenue_goal_never_falls_back_to_unrelated_event_rates() {
    let mut p = project();
    p.goal = BusinessGoal::Revenue;
    p.ga4_origin_confirmed = true;
    let mut ga = ga4_dataset();
    ga.coverage.goal_key = p.goal_key();
    let report = analyze(
        &p,
        &request(),
        &[
            data(vec![row("a", 10.0, 2000.0, 8.0)], true, DataGrain::Pages),
            ga,
        ],
    )
    .unwrap();
    assert_eq!(
        report.opportunities[0].business.as_ref().unwrap().boost,
        1.0
    );
}

#[test]
fn ga4_comparison_preserves_observed_previous_goal_metrics() {
    let mut p = project();
    p.ga4_origin_confirmed = true;
    let current = ga4_dataset();
    let mut previous = current.clone();
    previous.range = request().previous;
    previous.rows[0].sessions = Some(150.0);
    previous.rows[0].key_events = Some(5.0);
    let report = analyze(
        &p,
        &request(),
        &[
            data(vec![row("a", 10.0, 2000.0, 8.0)], true, DataGrain::Pages),
            current,
            previous,
        ],
    )
    .unwrap();
    assert_eq!(report.sessions, Some(200.0));
    assert_eq!(report.previous_ga4.as_ref().unwrap().sessions, Some(150.0));
    assert_eq!(report.previous_ga4.unwrap().key_events, Some(5.0));
}

#[test]
fn partially_known_position_is_not_a_complete_average_or_near_win() {
    let known = row("a", 10.0, 100.0, 8.0);
    let mut unknown = row("a", 10.0, 1900.0, 1.0);
    unknown.position = None;
    let report = analyze(
        &project(),
        &request(),
        &[data(vec![known, unknown], true, DataGrain::Pages)],
    )
    .unwrap();
    assert_eq!(report.current.unwrap().position, None);
    assert!(report.warnings.contains(&"position-incomplete".into()));
    assert!(report.opportunities.is_empty());
}
#[test]
fn incomplete_ga4_rows_never_become_zero_rate_or_partial_revenue_boost() {
    let mut p = project();
    p.ga4_origin_confirmed = true;
    let mut ga = ga4_dataset();
    let mut missing = ga.rows[0].clone();
    missing.goal_rate = None;
    missing.revenue = None;
    missing.key_events = None;
    ga.rows[0].revenue = Some(1000.0);
    ga.rows.push(missing);
    let pages = data(vec![row("a", 10.0, 2000.0, 8.0)], true, DataGrain::Pages);
    let report = analyze(&p, &request(), &[pages.clone(), ga.clone()]).unwrap();
    let business = report.opportunities[0].business.as_ref().unwrap();
    assert_eq!(business.sessions, 400.0);
    assert_eq!(business.goal_rate, None);
    assert_eq!(business.key_events, None);
    assert_eq!(business.boost, 1.0);
    assert!(report.warnings.contains(&"ga4-business-incomplete".into()));
    p.goal = BusinessGoal::Revenue;
    ga.coverage.goal_key = p.goal_key();
    let report = analyze(&p, &request(), &[pages, ga]).unwrap();
    let business = report.opportunities[0].business.as_ref().unwrap();
    assert_eq!(business.revenue, None);
    assert_eq!(business.boost, 1.0);
}
#[test]
fn thresholded_or_truncated_ga4_observations_never_inflate_priority() {
    let mut p = project();
    p.ga4_origin_confirmed = true;
    let pages = data(vec![row("a", 10.0, 2000.0, 8.0)], true, DataGrain::Pages);
    for warning in ["ga4-limited", "row-limit", "ga4-incomplete"] {
        let mut ga = ga4_dataset();
        ga.coverage.warnings.push(warning.into());
        ga.coverage.limited = true;
        let report = analyze(&p, &request(), &[pages.clone(), ga]).unwrap();
        assert_eq!(
            report.opportunities[0].business.as_ref().unwrap().boost,
            1.0
        );
        if warning != "ga4-limited" {
            assert_eq!(report.sessions, None);
        }
    }
}
