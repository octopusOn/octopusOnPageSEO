use std::borrow::Cow;
use std::fs;

use seonexus_core::Timestamp;
use seonexus_logparse::LogRecord;

use super::*;
use crate::batch::{HitBatch, HitBatchBuilder};

pub(super) const HOUR_US: i64 = 3_600_000_000;
pub(super) const BASE_US: i64 = 1_791_400_000_000_000; // 2026-10-07

pub(super) struct Fixture {
    _dir: tempfile::TempDir,
    pub(super) data: DataDir,
}

impl Fixture {
    pub(super) fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let data = DataDir::new(dir.path());
        Self { _dir: dir, data }
    }

    pub(super) fn open(&self) -> SiteStore {
        SiteStore::open(&self.data, SiteId::new(1), StoreSettings::default()).unwrap()
    }
}

pub(super) const GOOGLEBOT: BotTag = BotTag {
    id: "googlebot-desktop",
    category: "search",
};

/// A hit identified by `n`; equal `n` means an identical log line. Even ids are Googlebot
/// hits, tagged as such.
pub(super) fn hit_batch(ids: &[i64]) -> HitBatch {
    hit_batch_tagged(ids, true)
}

pub(super) fn hit_batch_tagged(ids: &[i64], tagged: bool) -> HitBatch {
    let mut builder = HitBatchBuilder::new();
    for &n in ids {
        let target = format!("/page/{}", n % 7);
        let record = LogRecord {
            timestamp: Timestamp::from_unix_micros(BASE_US + n * HOUR_US / 4),
            client_ip: "66.249.66.1".parse().unwrap(),
            method: Cow::Borrowed("GET"),
            target: Cow::Owned(target),
            protocol: Cow::Borrowed("HTTP/1.1"),
            status: if n % 5 == 0 { 404 } else { 200 },
            bytes: Some(100),
            referer: None,
            user_agent: (n % 2 == 0).then_some(Cow::Borrowed("Googlebot/2.1")),
            duration_micros: Some(1_000),
            host: None,
            router: None,
            upstream: None,
            request_seq: None,
        };
        let tag = (tagged && n % 2 == 0).then_some(GOOGLEBOT);
        builder.push(&record, n.unsigned_abs(), tag);
    }
    builder.finish().unwrap()
}

pub(super) fn file(name: &str) -> ImportedFile {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(name);
    fs::write(&path, name).unwrap();
    ImportedFile {
        fingerprint: FileFingerprint::of_file(&path).unwrap(),
        path: name.to_owned(),
    }
}

pub(super) fn import(
    store: &mut SiteStore,
    name: &str,
    batches: &[&[i64]],
    options: ImportOptions,
) -> ImportOutcome {
    let source = file(name);
    let mut session = store.begin_import(options, &NoBotVerification).unwrap();
    for ids in batches {
        session.append(hit_batch(ids)).unwrap();
    }
    session.commit(&record(&source)).unwrap()
}

pub(super) fn record(file: &ImportedFile) -> ImportRecord<'_> {
    ImportRecord {
        format_spec: "builtin:combined",
        file,
        total_lines: 0,
        parsed_lines: 0,
        first_request: None,
        last_request: None,
        skipped: &[],
        samples: &[],
    }
}

pub(super) fn range(from: i64, to: i64) -> Vec<i64> {
    (from..to).collect()
}

#[test]
fn import_writes_parquet_rollups_and_history() {
    let fixture = Fixture::new();
    let mut store = fixture.open();
    let source = file("a.log");
    let mut session = store
        .begin_import(ImportOptions::default(), &NoBotVerification)
        .unwrap();
    session.append(hit_batch(&range(0, 100))).unwrap();
    let outcome = session.commit(&record(&source)).unwrap();

    assert_eq!(outcome.inserted_rows, 100);
    assert_eq!(outcome.duplicate_rows, 0);
    assert!(store.is_imported(&source.fingerprint).unwrap());
    let totals = store.totals().unwrap();
    assert_eq!(totals.imports, 1);
    assert_eq!(totals.hits, 100);
    assert_eq!(totals.urls, 7);
    assert_eq!(totals.user_agents, 1);
    assert_eq!(store.count_stored_hits().unwrap(), 100);
    assert!(!fixture.data.site(SiteId::new(1)).staging(1).exists());
}

#[test]
fn overlapping_import_only_adds_new_lines() {
    let fixture = Fixture::new();
    let mut store = fixture.open();
    import(
        &mut store,
        "a.log",
        &[&range(0, 100)],
        ImportOptions::default(),
    );
    let outcome = import(
        &mut store,
        "b.log",
        &[&range(50, 150)],
        ImportOptions::default(),
    );

    assert_eq!(outcome.inserted_rows, 50);
    assert_eq!(outcome.duplicate_rows, 50);
    assert_eq!(store.totals().unwrap().hits, 150);
    assert_eq!(store.count_stored_hits().unwrap(), 150);
}

#[test]
fn identical_lines_are_counted_as_a_multiset() {
    let fixture = Fixture::new();
    let mut store = fixture.open();
    let first = import(&mut store, "a.log", &[&[1, 1]], ImportOptions::default());
    assert_eq!(
        first.inserted_rows, 2,
        "repeats within one import are real hits"
    );

    let second = import(&mut store, "b.log", &[&[1, 1, 1]], ImportOptions::default());
    assert_eq!((second.inserted_rows, second.duplicate_rows), (1, 2));
    assert_eq!(store.count_stored_hits().unwrap(), 3);
}

#[test]
fn duplicates_spanning_flushes_are_matched_once() {
    let fixture = Fixture::new();
    let mut store = fixture.open();
    import(&mut store, "a.log", &[&[1]], ImportOptions::default());
    let small = ImportOptions { flush_rows: 1 };
    let outcome = import(&mut store, "b.log", &[&[1], &[1]], small);
    assert_eq!((outcome.inserted_rows, outcome.duplicate_rows), (1, 1));
}

#[test]
fn dropped_session_leaves_no_trace() {
    let fixture = Fixture::new();
    let mut store = fixture.open();
    {
        let mut session = store
            .begin_import(ImportOptions { flush_rows: 10 }, &NoBotVerification)
            .unwrap();
        session.append(hit_batch(&range(0, 50))).unwrap();
    }
    assert_eq!(store.totals().unwrap(), SiteTotals::default());
    assert_eq!(store.count_stored_hits().unwrap(), 0);
    let hits = fixture.data.site(SiteId::new(1)).hits();
    assert_eq!(fs::read_dir(hits).map_or(0, Iterator::count), 0);

    let outcome = import(
        &mut store,
        "a.log",
        &[&range(0, 10)],
        ImportOptions::default(),
    );
    assert_eq!(outcome.inserted_rows, 10);
}

#[test]
fn second_open_of_a_site_is_refused() {
    let fixture = Fixture::new();
    let _store = fixture.open();
    assert!(matches!(
        SiteStore::open(&fixture.data, SiteId::new(1), StoreSettings::default()),
        Err(StoreError::SiteLocked(_))
    ));
}

#[test]
fn recovery_publishes_committed_and_discards_unfinished_imports() {
    let fixture = Fixture::new();
    let paths = fixture.data.site(SiteId::new(1));
    {
        let mut store = fixture.open();
        import(
            &mut store,
            "a.log",
            &[&range(0, 10)],
            ImportOptions::default(),
        );
    }
    // Simulate a crash after COMMIT but before publishing: move import 1's files back.
    let month_dir = fs::read_dir(paths.hits())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.is_dir())
        .unwrap();
    let staged = paths
        .staging(1)
        .join("flush-0")
        .join(month_dir.file_name().unwrap());
    fs::create_dir_all(staged.parent().unwrap()).unwrap();
    fs::rename(&month_dir, &staged).unwrap();
    // And an unfinished import 99.
    fs::create_dir_all(paths.staging(99).join("flush-0")).unwrap();

    let store = fixture.open();
    assert_eq!(store.count_stored_hits().unwrap(), 10);
    assert!(!paths.staging(1).exists());
    assert!(!paths.staging(99).exists());
}

#[test]
fn destroy_removes_all_site_files() {
    let fixture = Fixture::new();
    let mut store = fixture.open();
    import(
        &mut store,
        "a.log",
        &[&range(0, 10)],
        ImportOptions::default(),
    );
    store.destroy().unwrap();
    assert!(!fixture.data.site(SiteId::new(1)).dir().exists());
    let reopened = fixture.open();
    assert_eq!(reopened.totals().unwrap(), SiteTotals::default());
}

#[test]
fn deleting_an_import_removes_its_rollups_files_and_history() {
    let fixture = Fixture::new();
    let mut store = fixture.open();
    let first = import(
        &mut store,
        "a.log",
        &[&range(0, 40)],
        ImportOptions::default(),
    );
    let second = import(
        &mut store,
        "b.log",
        &[&range(100, 110)],
        ImportOptions::default(),
    );
    assert_eq!(store.totals().unwrap().hits, 50);

    store.delete_import(first.import_id).unwrap();
    let totals = store.totals().unwrap();
    assert_eq!(totals.imports, 1);
    assert_eq!(totals.hits, 10);
    assert_eq!(store.count_stored_hits().unwrap(), 10);
    // The file may be imported again.
    assert!(!store.is_imported(&file("a.log").fingerprint).unwrap());
    assert!(matches!(
        store.delete_import(first.import_id),
        Err(StoreError::ImportNotFound(_))
    ));

    store.delete_import(second.import_id).unwrap();
    assert_eq!(store.totals().unwrap().hits, 0);
    assert_eq!(store.count_stored_hits().unwrap(), 0);
}

#[test]
fn files_of_deleted_imports_are_removed_on_next_open() {
    let fixture = Fixture::new();
    let mut store = fixture.open();
    let outcome = import(
        &mut store,
        "a.log",
        &[&range(0, 10)],
        ImportOptions::default(),
    );
    // Simulate a crash between the committed deletion and the file removal.
    store
        .connection
        .execute(
            "DELETE FROM imports WHERE id = ?",
            params![outcome.import_id],
        )
        .unwrap();
    drop(store);
    let store = fixture.open();
    assert_eq!(store.count_stored_hits().unwrap(), 0);
}

#[test]
fn snapshot_lists_database_and_hits_and_restore_paths_are_safe() {
    let fixture = Fixture::new();
    let mut store = fixture.open();
    import(
        &mut store,
        "a.log",
        &[&range(0, 10)],
        ImportOptions::default(),
    );
    let snapshot = store.into_snapshot().unwrap();
    let files = snapshot.files();
    assert_eq!(files[0].0, "site.duckdb");
    // The database is closed (readable on Windows too) while the site stays locked.
    assert!(std::fs::read(&files[0].1).is_ok_and(|bytes| !bytes.is_empty()));
    assert!(matches!(
        SiteStore::open(&fixture.data, SiteId::new(1), StoreSettings::default()),
        Err(StoreError::SiteLocked(_))
    ));
    assert!(files.len() > 1);
    assert!(
        files[1..]
            .iter()
            .all(|(name, path)| name.starts_with("hits/month=2026-10/") && path.exists())
    );

    let data = &fixture.data;
    let site = SiteId::new(9);
    assert!(data.restore_path(site, "site.duckdb").is_some());
    assert!(
        data.restore_path(site, "hits/month=2026-10/import_1_f0_0.parquet")
            .is_some()
    );
    for bad in [
        "../x",
        "hits/month=2026-10/../../etc",
        "hits/month=26-10/a.parquet",
        "hits/month=2026-10/.hidden.parquet",
        "catalog.duckdb",
        "hits/month=2026-10/a.exe",
    ] {
        assert!(data.restore_path(site, bad).is_none(), "{bad}");
    }
}
