use super::*;
use crate::opportunities::{
    DataCoverage, DataGrain, DataSource, Metrics, OpportunityProject, OpportunitySignal,
    PerformanceDataset, PerformanceRow, RULE_VERSION,
};
#[test]
fn filtered_pagination_and_export_cover_all_matching_rows() {
    let rows: Vec<_> = (0..310)
        .map(|n| Opportunity {
            url: format!("https://example.com/{n}"),
            current: Metrics::default(),
            previous: None,
            queries: Vec::new(),
            signals: vec![OpportunitySignal {
                kind: "traffic-loss".into(),
                query: None,
                current: Metrics::default(),
                previous: None,
                benchmark_ctr: None,
                peer_urls: 0,
                estimated_clicks: Some(10.0),
                review_only: false,
            }],
            confidence: "observed".into(),
            priority: 10.0,
            priority_basis: "click-impact".into(),
            impact_clicks: Some(10.0),
            business: None,
            actions: Vec::new(),
        })
        .collect();
    let report = OpportunityReport {
        rule_version: RULE_VERSION.into(),
        request: AnalysisRequest::default(),
        current: None,
        previous: None,
        sessions: None,
        key_events: None,
        revenue: None,
        currency: None,
        opportunities: Vec::new(),
        coverage: Vec::new(),
        warnings: Vec::new(),
        previous_ga4: None,
        unknown_current_urls: 0,
        daily: Vec::new(),
    };
    let jobs = OpportunityJobs::default();
    jobs.inner.lock().unwrap().snapshots.insert(
        "snapshot".into(),
        Arc::new(Snapshot {
            summary: AnalysisSummary {
                id: "snapshot".into(),
                total: rows.len(),
                report,
                created_at_ms: 0,
            },
            rows,
        }),
    );
    let mut request = OpportunityPageRequest {
        analysis_id: "snapshot".into(),
        limit: 999,
        kind: "traffic-loss".into(),
        ..Default::default()
    };
    assert_eq!(jobs.page(&request).unwrap().rows.len(), 250);
    request.offset = 300;
    assert_eq!(jobs.page(&request).unwrap().rows.len(), 10);
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        jobs.export(&request, &dir.path().join("all.csv")).unwrap(),
        310
    );
    assert_eq!(
        jobs.export(&request, &dir.path().join("all.xlsx")).unwrap(),
        310
    );
    request.kind = "invalid".into();
    assert!(jobs.page(&request).is_err());
}
#[test]
fn cooperative_cancel_preserves_existing_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = OpportunityWorkspace::new(dir.path().into());
    let project = workspace
        .save_project(OpportunityProject {
            name: "Cancellation fixture".into(),
            site_url: "https://example.com/".into(),
            ..Default::default()
        })
        .unwrap();
    let request = AnalysisRequest {
        project_id: project.id.clone(),
        ..Default::default()
    };
    workspace
        .commit_datasets(
            &project,
            vec![PerformanceDataset {
                id: "fixture".into(),
                project_id: project.id.clone(),
                source: DataSource::Gsc,
                grain: DataGrain::Pages,
                range: request.current,
                coverage: DataCoverage {
                    site_url: project.site_url.clone(),
                    ..Default::default()
                },
                rows: vec![PerformanceRow {
                    url: Some("https://example.com/a".into()),
                    clicks: Some(0.0),
                    impressions: Some(1000.0),
                    position: Some(8.0),
                    ..Default::default()
                }],
                origin: "fixture".into(),
                created_at_ms: 0,
            }],
        )
        .unwrap();
    let jobs = OpportunityJobs::default();
    let before = jobs.analyze(&workspace, &request).unwrap();
    assert_eq!(before.total, 1);
    let error = jobs
        .analyze_controlled(&workspace, &request, &|| true)
        .unwrap_err();
    assert!(error.contains("cancelled"));
    assert_eq!(jobs.result(&before.id).unwrap().total, 1);
}

#[test]
fn analysis_and_sync_share_concurrency_limits_and_project_history_is_retained() {
    let jobs = OpportunityJobs::default();
    let (first, _) = jobs.reserve("a", "analysis").unwrap();
    let (second, _) = jobs.reserve("b", "sync").unwrap();
    assert!(jobs.reserve("c", "analysis").is_err());
    assert!(jobs.reserve("a", "sync").is_err());
    jobs.inner
        .lock()
        .unwrap()
        .jobs
        .get_mut(&first)
        .unwrap()
        .0
        .status = "completed".into();
    let (third, _) = jobs.reserve("c", "analysis").unwrap();
    assert!(jobs.jobs().iter().any(|job| job.id == first));
    jobs.inner
        .lock()
        .unwrap()
        .jobs
        .get_mut(&second)
        .unwrap()
        .0
        .status = "completed".into();
    jobs.inner
        .lock()
        .unwrap()
        .jobs
        .get_mut(&third)
        .unwrap()
        .0
        .status = "completed".into();
    let (next, _) = jobs.reserve("a", "sync").unwrap();
    assert!(!jobs.jobs().iter().any(|job| job.id == first));
    assert!(jobs.jobs().iter().any(|job| job.id == next));
    assert!(jobs.jobs().iter().any(|job| job.id == second));
}
