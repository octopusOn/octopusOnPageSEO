use super::*;
use crate::opportunities::{DataCoverage, DataGrain, DateRange, PerformanceRow, now_ms};
use seonexus_core::Date;
fn setup() -> (tempfile::TempDir, OpportunityWorkspace, OpportunityProject) {
    let dir = tempfile::tempdir().unwrap();
    let workspace = OpportunityWorkspace::new(dir.path().join("opportunities"));
    let project = workspace
        .save_project(OpportunityProject {
            name: "Fixture".into(),
            site_url: "https://example.com/".into(),
            ..Default::default()
        })
        .unwrap();
    (dir, workspace, project)
}
fn dataset(project: &OpportunityProject, origin: &str) -> PerformanceDataset {
    PerformanceDataset {
        id: origin.into(),
        project_id: project.id.clone(),
        source: DataSource::Gsc,
        grain: DataGrain::Pages,
        range: DateRange {
            start: Date::parse("2026-09-01").unwrap(),
            end: Date::parse("2026-09-28").unwrap(),
        },
        coverage: DataCoverage {
            site_url: project.site_url.clone(),
            property: project.gsc_property.clone(),
            ..Default::default()
        },
        rows: vec![PerformanceRow {
            url: Some("https://example.com/tr/".into()),
            clicks: Some(1.0),
            impressions: Some(1000.0),
            position: Some(8.0),
            ..Default::default()
        }],
        origin: origin.into(),
        created_at_ms: now_ms(),
    }
}
#[test]
fn transaction_replaces_identical_slot_and_isolates_projects() {
    let (_dir, w, p) = setup();
    w.commit_datasets(&p, vec![dataset(&p, "file-import")])
        .unwrap();
    w.commit_datasets(&p, vec![dataset(&p, "file-import")])
        .unwrap();
    assert_eq!(w.status().unwrap().datasets, 1);
    let p2 = w
        .save_project(OpportunityProject {
            name: "Other".into(),
            site_url: "https://other.com".into(),
            ..Default::default()
        })
        .unwrap();
    w.commit_datasets(&p2, vec![dataset(&p2, "file-import")])
        .unwrap();
    w.remove_project(&p.id).unwrap();
    assert_eq!(w.status().unwrap().datasets, 1);
    assert_eq!(w.read().unwrap().datasets[0].project_id, p2.id);
}
#[test]
fn failed_commit_preserves_bytes_and_no_duckdb_files_are_created() {
    let (_dir, w, p) = setup();
    w.commit_datasets(&p, vec![dataset(&p, "google-api")])
        .unwrap();
    let before = fs::read(w.path()).unwrap();
    let mut invalid = dataset(&p, "bad");
    invalid.range.end = invalid.range.start.add_days(-1);
    assert!(
        w.commit_datasets(&p, vec![dataset(&p, "first"), invalid])
            .is_err()
    );
    assert_eq!(fs::read(w.path()).unwrap(), before);
    let names: Vec<_> = fs::read_dir(&w.root)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names.len(), 1);
    assert_eq!(names[0], "opportunities.json.zst");
}
#[test]
fn stale_project_settings_reject_sync_and_cache_clear_preserves_imports() {
    let (_dir, w, p) = setup();
    w.commit_datasets(&p, vec![dataset(&p, "file-import")])
        .unwrap();
    let mut api = dataset(&p, "google-api");
    api.range.start = api.range.start.add_days(1);
    w.commit_datasets(&p, vec![api]).unwrap();
    w.clear_api_cache(&p.id).unwrap();
    assert_eq!(w.status().unwrap().datasets, 1);
    let mut changed = p.clone();
    changed.name = "Changed".into();
    w.save_project(changed).unwrap();
    assert!(
        w.commit_datasets(&p, vec![dataset(&p, "google-api")])
            .is_err()
    );
}
#[test]
fn project_binding_rejects_unrelated_property() {
    let (_dir, w, mut p) = setup();
    p.gsc_property = "sc-domain:unrelated.com".into();
    assert!(w.save_project(p.clone()).is_err());
    p.gsc_property = "sc-domain:example.com".into();
    p.brand_terms = vec!["  TEST ".into(), "test".into()];
    assert_eq!(w.save_project(p).unwrap().brand_terms, vec!["test"]);
}

#[test]
fn cancellation_after_encoding_preserves_original_store_and_cleans_temporary_file() {
    let (_dir, w, p) = setup();
    w.commit_datasets(&p, vec![dataset(&p, "file-import")])
        .unwrap();
    let before = fs::read(w.path()).unwrap();
    let mut replacement = w.read().unwrap();
    replacement.datasets.clear();
    let calls = std::cell::Cell::new(0);
    let cancel = || {
        let count = calls.get();
        calls.set(count + 1);
        count > 0
    };
    assert!(
        w.write_with_cancel(&replacement, &cancel)
            .unwrap_err()
            .contains("cancelled")
    );
    assert_eq!(calls.get(), 2);
    assert_eq!(fs::read(w.path()).unwrap(), before);
    assert_eq!(fs::read_dir(&w.root).unwrap().count(), 1);
}
