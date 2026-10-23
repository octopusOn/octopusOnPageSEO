//! Copies kept in step through a hub, all in this process: what a desktop and the web do over
//! the network, without it.

use serde_json::json;
use tempfile::TempDir;

use super::*;
use crate::{
    AdSeen, Branding, CacheClass, CacheEntry, CheckResult, GscData, LinkProspect, LocalPointRow,
    LocalReviewRow, LocalSettings, MetricPoint, Period, ProjectSpec, RankCheckRow, RankSettings,
    ReportColors, ReportSpec, RunEnd, ScoreToSave, SearchKind, SearchRow, TrackerSpec,
};

const T0: &str = "2026-10-22T09:00:00Z";

struct Copy {
    _dir: TempDir,
    store: PlatformStore,
    name: &'static str,
}

fn workspace(name: &'static str) -> Copy {
    let dir = tempfile::tempdir().unwrap();
    let store = PlatformStore::open(dir.path()).unwrap();
    Copy {
        _dir: dir,
        store,
        name,
    }
}

fn number(store: &PlatformStore, name: &str) -> i64 {
    store
        .sync_value(name)
        .unwrap()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}

/// One sync of `copy` with `hub` at `now`: scan both, send the copy's changes, take the hub's.
/// Batches are small so they continue across calls.
fn round(copy: &Copy, hub: &Copy, now: i64) -> (SyncApplied, SyncApplied) {
    copy.store.sync_scan(now).unwrap();
    hub.store.sync_scan(now).unwrap();
    let mut sent = SyncApplied::default();
    loop {
        let pushed = number(&copy.store, "pushed");
        let batch = copy.store.sync_local_changes(pushed, 0, 2_000).unwrap();
        let applied = hub
            .store
            .sync_apply(
                SyncSide::Hub { replica: copy.name },
                &batch.changes,
                now,
                !batch.more,
            )
            .unwrap();
        let last = batch.last.max(pushed);
        copy.store
            .set_sync_value("pushed", Some(&last.to_string()))
            .unwrap();
        copy.store.sync_acknowledge(&applied.accepted).unwrap();
        let back = hub.store.sync_versions(&applied.rejected).unwrap();
        copy.store
            .sync_apply(SyncSide::Copy { pushed: last }, &back, now, false)
            .unwrap();
        sent.absorb(applied);
        if !batch.more {
            break;
        }
    }
    let mut taken = SyncApplied::default();
    loop {
        let cursor = number(&copy.store, "cursor");
        let batch = hub
            .store
            .sync_hub_changes(cursor, copy.name, 2_000)
            .unwrap();
        let pushed = number(&copy.store, "pushed");
        let applied = copy
            .store
            .sync_apply(SyncSide::Copy { pushed }, &batch.changes, now, !batch.more)
            .unwrap();
        copy.store
            .set_sync_value("cursor", Some(&batch.last.to_string()))
            .unwrap();
        taken.absorb(applied);
        if !batch.more {
            break;
        }
    }
    (sent, taken)
}

fn project(store: &PlatformStore, domain: &str) -> i64 {
    store
        .create_project(
            &ProjectSpec {
                name: String::new(),
                domain: domain.to_owned(),
                country: "tr".to_owned(),
                language: "tr".to_owned(),
                competitors: vec!["rakip.com".to_owned()],
                brands: vec!["Örnek".to_owned()],
            },
            T0,
        )
        .unwrap()
        .id
}

fn project_id(store: &PlatformStore, domain: &str) -> i64 {
    store
        .projects()
        .unwrap()
        .into_iter()
        .find(|project| project.domain == domain)
        .unwrap()
        .id
}

fn keywords(store: &PlatformStore, project: i64) -> Vec<String> {
    let mut words: Vec<String> = store
        .tracked_keywords(project)
        .unwrap()
        .into_iter()
        .map(|keyword| keyword.keyword)
        .collect();
    words.sort();
    words
}

fn tracker() -> TrackerSpec {
    TrackerSpec {
        engine: "google".to_owned(),
        country: "tr".to_owned(),
        language: "tr".to_owned(),
        device: "desktop".to_owned(),
        location: None,
    }
}

fn report_spec(name: &str, recipients: &[&str]) -> ReportSpec {
    ReportSpec {
        name: name.to_owned(),
        language: "tr".to_owned(),
        days: 28,
        widgets: vec!["summary".to_owned()],
        schedule: "manual".to_owned(),
        recipients: recipients
            .iter()
            .map(|&address| address.to_owned())
            .collect(),
        design: "clean".to_owned(),
        colors: ReportColors::default(),
    }
}

#[test]
fn a_project_with_everything_it_holds_reaches_every_copy() {
    let desktop = workspace("desktop");
    let hub = workspace("hub");
    let laptop = workspace("laptop");
    let store = &desktop.store;
    let id = project(store, "ornek.com");
    store
        .save_rank_settings(
            id,
            &RankSettings {
                schedule: "weekly".to_owned(),
                ..RankSettings::default()
            },
        )
        .unwrap();
    store.add_rank_tracker(id, &tracker(), T0).unwrap();
    store
        .track_keywords(
            id,
            &[
                ("spor ayakkabı".to_owned(), Some(1300)),
                ("koşu".to_owned(), None),
            ],
            &["marka".to_owned()],
            T0,
        )
        .unwrap();
    store
        .add_ai_prompts(id, &["En iyi spor ayakkabı?".to_owned()], &[], T0)
        .unwrap();
    let list = store.create_keyword_list(id, "Fikirler", T0).unwrap();
    store
        .add_to_keyword_list(list.id, &[("bot".to_owned(), json!({ "volume": 10 }))], T0)
        .unwrap();
    store
        .save_report(
            id,
            None,
            &ReportSpec {
                design: "elegant".to_owned(),
                colors: ReportColors {
                    cover: Some("#102030".to_owned()),
                    ..ReportColors::default()
                },
                ..report_spec("Aylık", &["a@ornek.com"])
            },
            T0,
        )
        .unwrap();
    store
        .save_report_branding(&Branding {
            company: Some("Nexus Medya".to_owned()),
            ..Branding::default()
        })
        .unwrap();

    let (sent, _) = round(&desktop, &hub, 1_000);
    assert!(sent.accepted.len() >= 9, "{sent:?}");
    assert_eq!(sent.parked, 0);
    round(&laptop, &hub, 2_000);

    for copy in [&hub, &laptop] {
        let here = &copy.store;
        let id = project_id(here, "ornek.com");
        let project = here.project(id).unwrap();
        assert_eq!(project.competitors, ["rakip.com"]);
        assert_eq!(here.rank_settings(id).unwrap().schedule, "weekly");
        assert_eq!(here.rank_trackers(id).unwrap().len(), 1);
        assert_eq!(keywords(here, id), ["koşu", "spor ayakkabı"]);
        let tracked = here.tracked_keywords(id).unwrap();
        let spor = tracked
            .iter()
            .find(|keyword| keyword.keyword == "spor ayakkabı")
            .unwrap();
        assert_eq!(spor.volume, Some(1300));
        assert_eq!(spor.tags, ["marka"]);
        assert_eq!(here.ai_prompts(id).unwrap().len(), 1);
        let lists = here.keyword_lists(id).unwrap();
        assert_eq!(lists[0].name, "Fikirler");
        assert_eq!(here.list_keywords(lists[0].id).unwrap()[0].keyword, "bot");
        assert_eq!(
            here.reports(id).unwrap()[0].spec.recipients,
            ["a@ornek.com"]
        );
        assert_eq!(here.reports(id).unwrap()[0].spec.design, "elegant");
        assert_eq!(
            here.reports(id).unwrap()[0].spec.colors.cover.as_deref(),
            Some("#102030")
        );
        assert_eq!(
            here.report_branding().unwrap().company.as_deref(),
            Some("Nexus Medya")
        );
    }

    // Nothing echoes: the next syncs find nothing to send either way.
    let (sent, taken) = round(&desktop, &hub, 3_000);
    assert_eq!((sent.applied, taken.applied), (0, 0));
    let (sent, taken) = round(&laptop, &hub, 3_000);
    assert_eq!((sent.applied, taken.applied), (0, 0));
    assert_eq!(desktop.store.sync_scan(4_000).unwrap(), 0);
    assert_eq!(hub.store.sync_scan(4_000).unwrap(), 0);
}

#[test]
fn edits_and_deletions_travel_both_ways() {
    let desktop = workspace("desktop");
    let hub = workspace("hub");
    let id = project(&desktop.store, "ornek.com");
    desktop
        .store
        .track_keywords(
            id,
            &[("bir".to_owned(), None), ("iki".to_owned(), None)],
            &[],
            T0,
        )
        .unwrap();
    round(&desktop, &hub, 1_000);

    // The web renames the project and drops a keyword.
    let on_hub = project_id(&hub.store, "ornek.com");
    let mut spec = ProjectSpec {
        name: "Örnek Mağaza".to_owned(),
        domain: "ornek.com".to_owned(),
        country: "tr".to_owned(),
        language: "tr".to_owned(),
        ..ProjectSpec::default()
    };
    hub.store.update_project(on_hub, &spec, T0).unwrap();
    let iki = hub
        .store
        .tracked_keywords(on_hub)
        .unwrap()
        .into_iter()
        .find(|keyword| keyword.keyword == "iki")
        .unwrap();
    hub.store.untrack_keywords(on_hub, &[iki.id]).unwrap();
    round(&desktop, &hub, 2_000);
    assert_eq!(desktop.store.project(id).unwrap().name, "Örnek Mağaza");
    assert_eq!(keywords(&desktop.store, id), ["bir"]);

    // The desktop adds a competitor; then deletes the project.
    spec.competitors = vec!["yeni.com".to_owned()];
    desktop.store.update_project(id, &spec, T0).unwrap();
    round(&desktop, &hub, 3_000);
    assert_eq!(hub.store.project(on_hub).unwrap().competitors, ["yeni.com"]);
    desktop.store.delete_project(id).unwrap();
    let (sent, _) = round(&desktop, &hub, 4_000);
    assert_eq!(hub.store.project_count().unwrap(), 0);
    // Only the project's deletion travels: its keyword went with it.
    assert_eq!(sent.applied, 1);
}

#[test]
fn the_later_edit_wins_and_times_the_program_writes_stay() {
    let desktop = workspace("desktop");
    let hub = workspace("hub");
    let id = project(&desktop.store, "ornek.com");
    round(&desktop, &hub, 1_000);
    let on_hub = project_id(&hub.store, "ornek.com");

    // The desktop ran a check by hand; later, the web changed the schedule.
    desktop
        .store
        .mark_rank_run(id, "2026-10-22T10:00:00Z")
        .unwrap();
    desktop.store.sync_scan(2_000).unwrap();
    hub.store
        .save_rank_settings(
            on_hub,
            &RankSettings {
                schedule: "daily".to_owned(),
                ..RankSettings::default()
            },
        )
        .unwrap();
    hub.store.sync_scan(3_000).unwrap();
    round(&desktop, &hub, 4_000);
    for (store, id) in [(&desktop.store, id), (&hub.store, on_hub)] {
        let settings = store.rank_settings(id).unwrap();
        assert_eq!(settings.schedule, "daily");
        assert_eq!(
            settings.last_run_at.as_deref(),
            Some("2026-10-22T10:00:00Z")
        );
    }
    round(&desktop, &hub, 5_000);
    assert_eq!(desktop.store.sync_scan(6_000).unwrap(), 0);
    assert_eq!(hub.store.sync_scan(6_000).unwrap(), 0);
}

#[test]
fn a_deletion_wins_over_an_edit_of_a_record_made_apart() {
    let desktop = workspace("desktop");
    let hub = workspace("hub");
    let id = project(&desktop.store, "ornek.com");
    let report = desktop
        .store
        .save_report(id, None, &report_spec("Aylık", &[]), T0)
        .unwrap();
    round(&desktop, &hub, 1_000);
    let on_hub = project_id(&hub.store, "ornek.com");
    let hub_report = hub.store.reports(on_hub).unwrap()[0].id;
    hub.store.delete_report(on_hub, hub_report).unwrap();
    hub.store.sync_scan(2_000).unwrap();
    desktop
        .store
        .save_report(id, Some(report.id), &report_spec("Haftalık", &[]), T0)
        .unwrap();
    round(&desktop, &hub, 3_000);
    assert_eq!(desktop.store.reports(id).unwrap().len(), 0);
    assert_eq!(hub.store.reports(on_hub).unwrap().len(), 0);
}

#[test]
fn the_same_keyword_added_on_two_copies_is_one() {
    let desktop = workspace("desktop");
    let hub = workspace("hub");
    let id = project(&desktop.store, "ornek.com");
    round(&desktop, &hub, 1_000);
    let on_hub = project_id(&hub.store, "ornek.com");
    desktop
        .store
        .track_keywords(id, &[("ortak".to_owned(), Some(5))], &[], T0)
        .unwrap();
    hub.store
        .track_keywords(
            on_hub,
            &[("ortak".to_owned(), None), ("web".to_owned(), None)],
            &[],
            T0,
        )
        .unwrap();
    round(&desktop, &hub, 2_000);
    round(&desktop, &hub, 3_000);
    assert_eq!(keywords(&desktop.store, id), ["ortak", "web"]);
    assert_eq!(keywords(&hub.store, on_hub), ["ortak", "web"]);
}

fn check(run: i64, project: i64, keyword: i64, tracker: i64) -> RankCheckRow {
    RankCheckRow {
        run_id: run,
        project_id: project,
        keyword_id: keyword,
        tracker_id: tracker,
        checked_at: T0.to_owned(),
        position: Some(3),
        url: Some("https://ornek.com/a".to_owned()),
        pages: vec![(3, "https://ornek.com/a".to_owned())],
        features: vec!["ai_overview".to_owned()],
        ai_overview: true,
        ai_cited: false,
        snippet: false,
        owned: Vec::new(),
        results: vec![CheckResult {
            position: 1,
            domain: "rakip.com".to_owned(),
            url: "https://rakip.com/".to_owned(),
            title: Some("Rakip".to_owned()),
        }],
        error: None,
    }
}

#[test]
fn runs_travel_whole_once_finished() {
    let desktop = workspace("desktop");
    let hub = workspace("hub");
    let store = &desktop.store;
    let id = project(store, "ornek.com");
    let place = store.add_rank_tracker(id, &tracker(), T0).unwrap();
    store
        .track_keywords(
            id,
            &[("bir".to_owned(), None), ("iki".to_owned(), None)],
            &[],
            T0,
        )
        .unwrap();
    let words = store.tracked_keywords(id).unwrap();
    let run = store.begin_rank_run(id, false, 2, T0).unwrap();
    store
        .add_rank_check(&check(run, id, words[0].id, place.id))
        .unwrap();
    round(&desktop, &hub, 1_000);
    let on_hub = project_id(&hub.store, "ornek.com");
    assert_eq!(hub.store.rank_runs(on_hub, 10).unwrap().len(), 0);

    store
        .add_rank_check(&check(run, id, words[1].id, place.id))
        .unwrap();
    store
        .finish_rank_run(
            run,
            &RunEnd {
                status: "done",
                checked: 2,
                failed: 0,
                cost_usd: 0.004,
                error: None,
            },
            T0,
        )
        .unwrap();
    round(&desktop, &hub, 2_000);
    let runs = hub.store.rank_runs(on_hub, 10).unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].status, "done");
    let hub_tracker = hub.store.rank_trackers(on_hub).unwrap()[0].id;
    let checks = hub
        .store
        .rank_checks(on_hub, hub_tracker, None, None)
        .unwrap();
    assert_eq!(checks.len(), 2);
    assert_eq!(checks[0].results[0].domain, "rakip.com");
    let hub_words = hub.store.tracked_keywords(on_hub).unwrap();
    assert!(
        checks
            .iter()
            .all(|check| hub_words.iter().any(|word| word.id == check.keyword_id))
    );
}

#[test]
fn a_change_waits_for_the_record_it_belongs_to() {
    let desktop = workspace("desktop");
    let hub = workspace("hub");
    let id = project(&desktop.store, "ornek.com");
    desktop
        .store
        .track_keywords(id, &[("bir".to_owned(), None)], &[], T0)
        .unwrap();
    desktop.store.sync_scan(1_000).unwrap();
    let changes = desktop
        .store
        .sync_local_changes(0, 0, 1 << 20)
        .unwrap()
        .changes;
    let (projects, rest): (Vec<_>, Vec<_>) = changes
        .into_iter()
        .partition(|change| change.kind == "project");
    let side = SyncSide::Hub { replica: "desktop" };
    let applied = hub.store.sync_apply(side, &rest, 1_000, false).unwrap();
    assert!(applied.parked >= 1);
    assert_eq!(hub.store.sync_parked().unwrap(), applied.parked as u64);
    let applied = hub.store.sync_apply(side, &projects, 1_000, true).unwrap();
    assert_eq!(hub.store.sync_parked().unwrap(), 0);
    assert!(applied.applied >= 2);
    let on_hub = project_id(&hub.store, "ornek.com");
    assert_eq!(keywords(&hub.store, on_hub), ["bir"]);
}

#[test]
fn search_console_numbers_and_kept_answers_travel() {
    let desktop = workspace("desktop");
    let hub = workspace("hub");
    let id = project(&desktop.store, "ornek.com");
    desktop
        .store
        .replace_gsc_data(
            id,
            &GscData {
                search: vec![(
                    SearchKind::Query,
                    Period::Now,
                    SearchRow {
                        key: "spor ayakkabı".to_owned(),
                        clicks: 12.0,
                        impressions: 340.0,
                        position: 4.5,
                    },
                )],
                ..GscData::default()
            },
        )
        .unwrap();
    let entry = CacheEntry {
        body: json!([{ "items": [] }]),
        fetched_at: T0.to_owned(),
        expires_at: 4_000_000_000,
        cost_usd: 0.02,
        via: "credit".to_owned(),
    };
    desktop
        .store
        .keep(
            "k1",
            "dataforseo_labs/x",
            CacheClass::Domain,
            Some(id),
            &entry,
        )
        .unwrap();
    desktop
        .store
        .keep("k2", "serp/x", CacheClass::Serp, Some(id), &entry)
        .unwrap();
    // A filtered page of a list is kept where it was asked.
    desktop
        .store
        .keep(
            "k3",
            "dataforseo_labs/x",
            CacheClass::Domain,
            Some(id),
            &entry,
        )
        .unwrap();
    desktop.store.keep_here_only("k3").unwrap();
    round(&desktop, &hub, 1_000);
    let on_hub = project_id(&hub.store, "ornek.com");
    let rows = hub
        .store
        .gsc_rows(on_hub, SearchKind::Query, Period::Now, 10)
        .unwrap();
    assert_eq!(rows[0].key, "spor ayakkabı");
    assert!(hub.store.cached("k1").unwrap().is_some());
    // Live result pages stay where they were fetched.
    assert!(hub.store.cached("k2").unwrap().is_none());
    assert!(hub.store.cached("k3").unwrap().is_none());

    // An expired answer leaves without a word; a newer one on the web comes back.
    desktop.store.drop_stale(5_000_000_000, 0).unwrap();
    let newer = CacheEntry {
        fetched_at: "2026-10-23T09:00:00Z".to_owned(),
        ..entry
    };
    hub.store
        .keep(
            "k1",
            "dataforseo_labs/x",
            CacheClass::Domain,
            Some(on_hub),
            &newer,
        )
        .unwrap();
    let (sent, _) = round(&desktop, &hub, 2_000);
    assert_eq!(sent.applied, 0);
    assert_eq!(
        desktop.store.cached("k1").unwrap().unwrap().fetched_at,
        "2026-10-23T09:00:00Z"
    );
}

#[test]
fn links_belong_to_the_web_and_recipients_are_checked_there() {
    let desktop = workspace("desktop");
    let hub = workspace("hub");
    let id = project(&desktop.store, "ornek.com");
    let report = desktop
        .store
        .save_report(id, None, &report_spec("Aylık", &["a@ornek.com"]), T0)
        .unwrap();
    desktop
        .store
        .set_report_share(id, report.id, Some("desktop-token"))
        .unwrap();
    round(&desktop, &hub, 1_000);
    let on_hub = project_id(&hub.store, "ornek.com");
    let hub_report = hub.store.reports(on_hub).unwrap()[0].clone();
    assert_eq!(hub_report.share_token, None);
    // The web's version came back.
    assert_eq!(
        desktop.store.report(id, report.id).unwrap().share_token,
        None
    );

    hub.store
        .set_report_share(on_hub, hub_report.id, Some("web-token"))
        .unwrap();
    round(&desktop, &hub, 2_000);
    assert_eq!(
        desktop
            .store
            .report(id, report.id)
            .unwrap()
            .share_token
            .as_deref(),
        Some("web-token")
    );
    // Stopping the link on the desktop stops it on the web.
    desktop.store.set_report_share(id, report.id, None).unwrap();
    let (sent, _) = round(&desktop, &hub, 3_000);
    assert_eq!(sent.removed_shares, ["web-token"]);
    assert_eq!(
        hub.store.report(on_hub, hub_report.id).unwrap().share_token,
        None
    );
}

#[test]
fn forgetting_starts_over_without_losing_data() {
    let desktop = workspace("desktop");
    let hub = workspace("hub");
    project(&desktop.store, "ornek.com");
    round(&desktop, &hub, 1_000);
    desktop.store.sync_forget(&["replica"]).unwrap();
    hub.store.sync_forget(&[]).unwrap();
    round(&desktop, &hub, 2_000);
    assert_eq!(hub.store.project_count().unwrap(), 1);
    assert_eq!(desktop.store.project_count().unwrap(), 1);
    let crawls = hub.store.sync_clear().unwrap();
    assert_eq!(crawls, [] as [String; 0]);
    assert_eq!(hub.store.project_count().unwrap(), 0);
}

#[test]
fn finished_snapshots_travel_with_their_settings() {
    let desktop = workspace("desktop");
    let hub = workspace("hub");
    let id = project(&desktop.store, "ornek.com");
    desktop
        .store
        .save_snapshot_settings(
            id,
            &crate::SnapshotSettings {
                schedule: "monthly".to_owned(),
                competitors: vec!["rakip.com".to_owned()],
                last_run_at: None,
            },
        )
        .unwrap();
    let snapshot = desktop
        .store
        .begin_snapshot(id, false, Some("Başlangıç"), T0)
        .unwrap();
    round(&desktop, &hub, 1_000);
    let on_hub = project_id(&hub.store, "ornek.com");
    assert_eq!(
        hub.store.snapshot_settings(on_hub).unwrap().competitors,
        ["rakip.com"]
    );
    // Still running: it stays where it is taken.
    assert_eq!(hub.store.snapshots(on_hub).unwrap().len(), 0);
    desktop
        .store
        .finish_snapshot(
            snapshot,
            &crate::SnapshotEnd {
                status: "done",
                cost_usd: 0.3,
                error: None,
                data: &json!({ "keywords": [{ "keyword": "spor ayakkabı", "position": 4 }] }),
            },
            T0,
        )
        .unwrap();
    round(&desktop, &hub, 2_000);
    let rows = hub.store.snapshots(on_hub).unwrap();
    assert_eq!(rows.len(), 1);
    let kept = hub.store.snapshot(on_hub, rows[0].id).unwrap();
    assert_eq!(kept.label.as_deref(), Some("Başlangıç"));
    assert_eq!(kept.data.unwrap()["keywords"][0]["position"], 4);
    assert_eq!(desktop.store.sync_scan(3_000).unwrap(), 0);
}

#[test]
fn metric_points_of_a_day_are_one_record_on_every_copy() {
    let desktop = workspace("desktop");
    let hub = workspace("hub");
    let id = project(&desktop.store, "ornek.com");
    let point = |traffic: f64, taken_at: &str| MetricPoint {
        target: "ornek.com".to_owned(),
        kind: "domain".to_owned(),
        market: "tr-tr".to_owned(),
        day: "2026-10-22".to_owned(),
        metrics: json!({ "traffic": traffic }),
        taken_at: taken_at.to_owned(),
    };
    desktop
        .store
        .note_metrics(id, &point(900.0, "2026-10-22T08:00:00Z"))
        .unwrap();
    round(&desktop, &hub, 1_000);
    let on_hub = project_id(&hub.store, "ornek.com");
    let points = |copy: &Copy, project: i64| {
        copy.store
            .metric_points(project, "ornek.com", "domain", "tr-tr")
            .unwrap()
    };
    assert_eq!(points(&hub, on_hub)[0].metrics["traffic"], 900.0);

    // The web fetched the same day later: its numbers reach the desktop, still one point.
    hub.store
        .note_metrics(on_hub, &point(950.0, "2026-10-22T18:00:00Z"))
        .unwrap();
    round(&desktop, &hub, 2_000);
    let kept = points(&desktop, id);
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].metrics["traffic"], 950.0);
}

#[test]
fn ad_copies_seen_on_two_copies_are_one_with_both_sightings() {
    let desktop = workspace("desktop");
    let hub = workspace("hub");
    let id = project(&desktop.store, "ornek.com");
    let seen = |keyword: &str| AdSeen {
        title: "%40 indirim".to_owned(),
        description: Some("Ücretsiz kargo.".to_owned()),
        breadcrumb: None,
        url: None,
        keyword: keyword.to_owned(),
    };
    desktop
        .store
        .note_ads(id, "rival.com", &[seen("bot")], "2026-10-01T09:00:00Z")
        .unwrap();
    round(&desktop, &hub, 1_000);
    let on_hub = project_id(&hub.store, "ornek.com");
    hub.store
        .note_ads(
            on_hub,
            "rival.com",
            &[seen("terlik")],
            "2026-10-20T09:00:00Z",
        )
        .unwrap();
    round(&desktop, &hub, 2_000);
    let copies = desktop.store.ad_copies(id, "rival.com").unwrap();
    assert_eq!(copies.len(), 1);
    assert_eq!(copies[0].first_seen, "2026-10-01T09:00:00Z");
    assert_eq!(copies[0].last_seen, "2026-10-20T09:00:00Z");
    assert_eq!(copies[0].keywords, ["terlik", "bot"]);
}

#[test]
fn link_decisions_and_prospects_reach_every_copy() {
    let desktop = workspace("desktop");
    let hub = workspace("hub");
    let id = project(&desktop.store, "ornek.com");
    desktop
        .store
        .decide_link(id, "spam.xyz", Some("disavow"), Some("ağ"), T0)
        .unwrap();
    let prospect = LinkProspect {
        domain: "blog.example.org".to_owned(),
        url: None,
        source: "lost".to_owned(),
        status: "prospect".to_owned(),
        note: None,
        added_at: String::new(),
        updated_at: String::new(),
    };
    desktop.store.save_prospect(id, &prospect, T0).unwrap();
    round(&desktop, &hub, 1_000);
    let on_hub = project_id(&hub.store, "ornek.com");
    assert_eq!(
        hub.store.link_decisions(on_hub).unwrap()[0].decision,
        "disavow"
    );
    // Won on the web: the desktop sees it.
    hub.store
        .save_prospect(
            on_hub,
            &LinkProspect {
                status: "won".to_owned(),
                ..prospect
            },
            "2026-10-23T09:00:00Z",
        )
        .unwrap();
    hub.store
        .decide_link(on_hub, "spam.xyz", None, None, T0)
        .unwrap();
    round(&desktop, &hub, 2_000);
    assert_eq!(desktop.store.prospects(id).unwrap()[0].status, "won");
    assert_eq!(desktop.store.link_decisions(id).unwrap().len(), 0);
}

#[test]
fn local_settings_runs_reviews_and_ratings_reach_every_copy() {
    let desktop = workspace("desktop");
    let hub = workspace("hub");
    let store = &desktop.store;
    let id = project(store, "ornek.com");
    store
        .save_local_settings(
            id,
            &LocalSettings {
                business: Some(
                    json!({ "title": "Örnek Kahve", "cid": "222", "lat": 40.99, "lng": 29.03 }),
                ),
                keywords: vec!["kahve".to_owned()],
                grid: 3,
                radius_km: 1.5,
                schedule: "weekly".to_owned(),
                last_run_at: None,
            },
        )
        .unwrap();
    let run = store
        .begin_local_run(id, true, 9, 3, 1.5, (40.99, 29.03), T0)
        .unwrap();
    let point = |row: u32, position: Option<u16>| LocalPointRow {
        run_id: run,
        project_id: id,
        keyword: "kahve".to_owned(),
        row,
        col: 0,
        lat: 40.99,
        lng: 29.03,
        position,
        top: json!([{ "position": 1, "title": "Rakip" }]),
        error: None,
        checked_at: T0.to_owned(),
    };
    store
        .add_local_points(&[point(0, Some(2)), point(1, None)])
        .unwrap();
    store
        .finish_local_run(
            run,
            &RunEnd {
                status: "done",
                checked: 2,
                failed: 0,
                cost_usd: 0.004,
                error: None,
            },
            T0,
        )
        .unwrap();
    let review = LocalReviewRow {
        review_id: "r-1".to_owned(),
        rating: Some(5),
        text: Some("Harika".to_owned()),
        author: Some("Ayşe".to_owned()),
        author_url: None,
        url: None,
        published_at: Some(T0.to_owned()),
        owner_answer: None,
        answered_at: None,
        photos: 0,
        local_guide: false,
        first_seen: T0.to_owned(),
    };
    store.save_local_reviews(id, &[review], T0).unwrap();
    store
        .note_local_profile(
            id,
            "2026-10-22",
            Some(4.5),
            Some(640),
            Some(&json!({ "title": "Örnek Kahve" })),
            T0,
        )
        .unwrap();
    round(&desktop, &hub, 1_000);

    let on_hub = project_id(&hub.store, "ornek.com");
    let settings = hub.store.local_settings(on_hub).unwrap();
    assert_eq!(settings.keywords, ["kahve"]);
    assert_eq!(settings.business.unwrap()["cid"], "222");
    let runs = hub.store.local_runs(on_hub, 10).unwrap();
    assert_eq!((runs[0].status.as_str(), runs[0].grid), ("done", 3));
    let points = hub.store.local_points(runs[0].id).unwrap();
    assert_eq!(points.len(), 2);
    assert_eq!(points[0].position, Some(2));
    assert_eq!(points[0].top[0]["title"], "Rakip");
    assert_eq!(
        hub.store.local_reviews(on_hub, 10).unwrap()[0]
            .text
            .as_deref(),
        Some("Harika")
    );
    assert_eq!(
        hub.store.local_profiles(on_hub).unwrap()[0].votes,
        Some(640)
    );
}

#[test]
fn briefs_and_their_scores_reach_every_copy_and_go_together() {
    let desktop = workspace("desktop");
    let web = workspace("web");
    let hub = workspace("hub");
    let store = &desktop.store;
    let id = project(store, "ornek.com");
    let running = store
        .begin_content_brief(id, "spor ayakkabı", ("tr", "tr"), false, T0)
        .unwrap();
    round(&desktop, &hub, 1_000);
    let on_hub = project_id(&hub.store, "ornek.com");
    assert_eq!(hub.store.content_briefs(on_hub).unwrap().len(), 0);

    store
        .finish_content_brief(
            running,
            &json!({ "keyword": "spor ayakkabı", "terms": [] }),
            &json!({ "pages": [{ "url": "https://rakip.com/" }] }),
            0.002,
            T0,
        )
        .unwrap();
    store
        .note_brief_writing(running, "# Brief", "claude:claude-opus-5-5", 0.05, T0)
        .unwrap();
    let result = json!({ "words": 900 });
    store
        .save_content_score(
            running,
            id,
            ScoreToSave {
                kind: "url",
                url: Some("https://ornek.com/spor"),
                title: None,
                description: None,
                body: None,
                score: 61.0,
                result: &result,
            },
            T0,
        )
        .unwrap();
    round(&desktop, &hub, 1_100);
    round(&web, &hub, 1_200);

    let on_web = project_id(&web.store, "ornek.com");
    let briefs = web.store.content_briefs(on_web).unwrap();
    assert_eq!(briefs.len(), 1);
    assert_eq!(briefs[0].status, "done");
    assert_eq!(briefs[0].ai_text.as_deref(), Some("# Brief"));
    assert!((briefs[0].cost_usd - 0.052).abs() < 1e-9);
    let source = web
        .store
        .content_brief_source(briefs[0].id)
        .unwrap()
        .unwrap();
    assert_eq!(source["pages"][0]["url"], "https://rakip.com/");
    let scores = web.store.content_scores(briefs[0].id).unwrap();
    assert_eq!(scores.len(), 1);
    assert_eq!(scores[0].project_id, on_web);
    assert_eq!(scores[0].result["words"], 900);

    web.store.delete_content_brief(briefs[0].id).unwrap();
    round(&web, &hub, 1_300);
    round(&desktop, &hub, 1_400);
    assert_eq!(store.content_briefs(id).unwrap().len(), 0);
    assert_eq!(store.content_scores(running).unwrap().len(), 0);
}

#[test]
fn vitals_checks_and_the_search_console_comparison_reach_every_copy() {
    let desktop = workspace("desktop");
    let hub = workspace("hub");
    let store = &desktop.store;
    let id = project(store, "ornek.com");
    store
        .save_vital_check(
            id,
            &crate::VitalCheck {
                target: "https://ornek.com".to_owned(),
                scope: "origin".to_owned(),
                device: "phone".to_owned(),
                day: "2026-10-22".to_owned(),
                field: Some(json!({ "lcp": { "p75": 2400 } })),
                lab: None,
                history: Some(json!({ "dates": ["2026-10-19"], "lcp": [2400] })),
                checked_at: T0.to_owned(),
            },
        )
        .unwrap();
    let mut settings = store.gsc_settings(id).unwrap();
    settings.property = Some("sc-domain:ornek.com".to_owned());
    settings.compare = "year".to_owned();
    store.save_gsc_settings(id, &settings).unwrap();
    round(&desktop, &hub, 1_000);
    let on_hub = project_id(&hub.store, "ornek.com");
    let checks = hub.store.latest_vitals(on_hub).unwrap();
    assert_eq!(checks.len(), 1);
    assert_eq!(checks[0].field.as_ref().unwrap()["lcp"]["p75"], 2400);
    assert_eq!(checks[0].history.as_ref().unwrap()["lcp"][0], 2400);
    assert_eq!(hub.store.gsc_settings(on_hub).unwrap().compare, "year");
}

#[test]
fn an_alert_found_on_two_copies_is_one_and_reading_it_travels() {
    let desktop = workspace("desktop");
    let web = workspace("web");
    let hub = workspace("hub");
    let on_desktop = project(&desktop.store, "ornek.com");
    round(&desktop, &hub, 1_000);
    round(&web, &hub, 1_100);
    let on_web = project_id(&web.store, "ornek.com");
    let alert = crate::NewAlert {
        key: "gsc|2026-10-20".to_owned(),
        kind: "gsc_drop".to_owned(),
        severity: "bad".to_owned(),
        page: "gsc".to_owned(),
        params: json!({}),
        detail: json!({ "drop": 25 }),
        created_at: T0.to_owned(),
    };
    desktop.store.add_alert(on_desktop, &alert).unwrap();
    web.store.add_alert(on_web, &alert).unwrap();
    let mut settings = desktop.store.alert_settings(on_desktop).unwrap();
    settings.rules.links = true;
    desktop
        .store
        .save_alert_settings(on_desktop, &settings)
        .unwrap();
    round(&desktop, &hub, 1_200);
    round(&web, &hub, 1_300);
    round(&desktop, &hub, 1_400);
    assert_eq!(web.store.alerts(Some(on_web), false, 10).unwrap().len(), 1);
    assert!(web.store.alert_settings(on_web).unwrap().rules.links);
    web.store
        .mark_alerts_read(Some(on_web), None, "2026-10-22T10:00:00Z")
        .unwrap();
    round(&web, &hub, 1_500);
    round(&desktop, &hub, 1_600);
    let seen = desktop.store.alerts(Some(on_desktop), false, 10).unwrap();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].read_at.as_deref(), Some("2026-10-22T10:00:00Z"));
}
