use std::io::Write;
use std::path::Path;

use super::*;
use crate::catalog::{AddSiteRequest, add_client, add_site};

const DAY_ONE: &str = r#"57.141.20.59 - - [07/Oct/2026:18:39:53 +0000] "GET /a HTTP/2.0" 200 535 "-" "Googlebot/2.1" 1 "r@docker" "http://172.18.0.6:3000" 5ms
45.231.253.129 - - [07/Oct/2026:18:39:55 +0000] "GET /b HTTP/2.0" 404 33 "-" "UA" 2 "r@docker" "http://172.18.0.6:3000" 2ms
not a log line
"#;
const DAY_ONE_AND_TWO: &str = r#"45.231.253.129 - - [07/Oct/2026:18:39:55 +0000] "GET /b HTTP/2.0" 404 33 "-" "UA" 2 "r@docker" "http://172.18.0.6:3000" 2ms
57.141.20.59 - - [08/Oct/2026:09:00:00 +0000] "GET /c HTTP/2.0" 301 0 "-" "Googlebot/2.1" 3 "r@docker" "http://172.18.0.6:3000" 1ms
"#;

struct Fixture {
    dir: tempfile::TempDir,
    workspace: Workspace,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let workspace = Workspace::at(dir.path().join("data"));
        add_client(&workspace, "Acme").unwrap();
        add_site(
            &workspace,
            &AddSiteRequest {
                client: "Acme".into(),
                name: "acme.com".into(),
                format: FormatChoice::Auto,
            },
        )
        .unwrap();
        Self { dir, workspace }
    }

    fn write(&self, name: &str, body: &str) -> PathBuf {
        let path = self.dir.path().join(name);
        std::fs::File::create(&path)
            .unwrap()
            .write_all(body.as_bytes())
            .unwrap();
        path
    }

    fn import(&self, paths: &[&Path]) -> Result<ImportReport, ImportError> {
        self.import_with(paths, &CancelToken::new())
    }

    fn import_with(
        &self,
        paths: &[&Path],
        cancel: &CancelToken,
    ) -> Result<ImportReport, ImportError> {
        let request = ImportRequest {
            site: "acme.com".into(),
            paths: paths.iter().map(|path| path.to_path_buf()).collect(),
            max_error_samples: 5,
            options: ImportOptions::default(),
            verify_dns: false,
        };
        import_logs(&self.workspace, &request, &NoProgress, cancel)
    }

    fn stored_hits(&self) -> u64 {
        let site = Catalog::open(self.workspace.data_dir())
            .unwrap()
            .site_by_name("acme.com")
            .unwrap();
        SiteStore::open(
            self.workspace.data_dir(),
            site.id,
            self.workspace.settings(),
        )
        .unwrap()
        .count_stored_hits()
        .unwrap()
    }
}

#[test]
fn imports_then_skips_the_same_file() {
    let fixture = Fixture::new();
    let file = fixture.write("access.log", DAY_ONE);

    let first = fixture.import(&[&file]).unwrap();
    assert_eq!(first.format_id, "traefik-clf");
    assert_eq!(first.inserted_rows, 2);
    assert_eq!(first.total_lines, 3);
    let FileOutcome::Imported { summary, .. } = &first.files[0].outcome else {
        panic!("expected an import");
    };
    assert_eq!(summary.skipped_lines(), 1);

    let second = fixture.import(&[&file]).unwrap();
    assert_eq!(second.inserted_rows, 0);
    assert_eq!(second.skipped_files(), 1);
    assert_eq!(fixture.stored_hits(), 2);
}

#[test]
fn overlapping_files_store_each_hit_once() {
    let fixture = Fixture::new();
    let older = fixture.write("access.log.1", DAY_ONE);
    let newer = fixture.write("access.log", DAY_ONE_AND_TWO);

    let report = fixture.import(&[&older, &newer]).unwrap();
    assert_eq!(report.inserted_rows, 3);
    assert_eq!(report.duplicate_rows, 1);
    assert_eq!(fixture.stored_hits(), 3);
}

#[test]
fn cancellation_rolls_back_the_current_file() {
    let fixture = Fixture::new();
    let file = fixture.write("access.log", DAY_ONE);
    let cancel = CancelToken::new();
    cancel.cancel();

    assert!(matches!(
        fixture.import_with(&[&file], &cancel),
        Err(ImportError::Cancelled)
    ));
    assert_eq!(fixture.stored_hits(), 0);
    let report = fixture.import(&[&file]).unwrap();
    assert_eq!(
        report.inserted_rows, 2,
        "a cancelled file is not marked as imported"
    );
}

#[test]
fn unknown_site_is_reported() {
    let fixture = Fixture::new();
    let file = fixture.write("access.log", DAY_ONE);
    let request = ImportRequest {
        site: "nope.com".into(),
        paths: vec![file],
        max_error_samples: 5,
        options: ImportOptions::default(),
        verify_dns: false,
    };
    let error = import_logs(
        &fixture.workspace,
        &request,
        &NoProgress,
        &CancelToken::new(),
    );
    assert!(matches!(
        error,
        Err(ImportError::Store(StoreError::SiteNotFound(_)))
    ));
}

#[test]
fn files_without_matching_lines_are_not_remembered() {
    let fixture = Fixture::new();
    let log = fixture.write("access.log", DAY_ONE);
    let notes = fixture.write("notes.txt", "hello\nworld\n");

    let report = fixture.import(&[&log, &notes]).unwrap();
    assert_eq!(report.imported_files(), 1);
    assert_eq!(report.unrecognized_files(), 1);
    assert!(matches!(
        &report.files[1].outcome,
        FileOutcome::Unrecognized { summary } if summary.skipped_lines() == 2
    ));
    let again = fixture.import(&[&notes]);
    assert!(
        again.is_err(),
        "auto-detection fails on the unrecognized file alone"
    );
    let again = fixture.import(&[&log, &notes]).unwrap();
    assert_eq!(again.unrecognized_files(), 1, "the file is tried again");
}

#[test]
fn header_described_formats_are_detected_and_imported() {
    let fixture = Fixture::new();
    let iis = fixture.write(
        "u_ex261007.log",
        "#Software: Microsoft Internet Information Services 10.0
#Version: 1.0
#Fields: date time s-ip cs-method cs-uri-stem cs-uri-query s-port cs-username c-ip cs(User-Agent) cs(Referer) sc-status sc-substatus sc-win32-status time-taken
2026-10-07 18:39:53 10.0.0.5 GET /a - 443 - 66.249.66.1 Mozilla/5.0+(compatible;+Googlebot/2.1) - 200 0 0 15
2026-10-07 18:40:00 10.0.0.5 GET /b x=1 443 - 10.0.0.9 Mozilla/5.0 https://www.google.com/ 404 0 0 3
",
    );
    let report = fixture.import(&[&iis]).unwrap();
    assert_eq!(report.format_id, "w3c");
    assert_eq!(report.inserted_rows, 2);
    let FileOutcome::Imported { summary, .. } = &report.files[0].outcome else {
        panic!("expected an import");
    };
    // The three directive lines are skipped as comments, not reported as broken lines.
    assert_eq!(summary.skipped_lines(), 3);
    assert_eq!(summary.error_samples.len(), 0);

    let csv = fixture.write(
        "export.csv",
        "timestamp,client_ip,method,url,status,bytes,user_agent\n\
2026-10-08T09:00:00Z,66.249.66.1,GET,/c,301,0,Googlebot/2.1\n",
    );
    let report = fixture.import(&[&csv]).unwrap();
    assert_eq!(report.format_id, "csv");
    assert_eq!(report.inserted_rows, 1);
    assert_eq!(fixture.stored_hits(), 3);
}

#[test]
fn site_settings_filter_and_rewrite_imported_requests() {
    use crate::settings::{SiteSettings, TrafficScope, set_site_settings, site_settings};
    let fixture = Fixture::new();
    let saved = set_site_settings(
        &fixture.workspace,
        "acme.com",
        SiteSettings {
            traffic: TrafficScope::Bots,
            exclude_urls: vec!["^/admin".into()],
            strip_parameters: vec!["sid".into()],
            ..SiteSettings::default()
        },
    )
    .unwrap();
    assert_eq!(
        site_settings(&fixture.workspace, "acme.com").unwrap(),
        saved
    );

    let log = fixture.write(
        "access.log",
        r#"66.249.66.1 - - [07/Oct/2026:18:39:53 +0000] "GET /a?sid=42&page=2 HTTP/1.1" 200 535 "-" "Googlebot/2.1"
66.249.66.1 - - [07/Oct/2026:18:39:54 +0000] "GET /admin/login HTTP/1.1" 200 535 "-" "Googlebot/2.1"
10.0.0.1 - - [07/Oct/2026:18:39:55 +0000] "GET /a HTTP/1.1" 200 535 "-" "Mozilla/5.0"
"#,
    );
    let report = fixture.import(&[&log]).unwrap();
    assert_eq!(report.inserted_rows, 1);
    let FileOutcome::Imported {
        summary, import_id, ..
    } = &report.files[0].outcome
    else {
        panic!("expected an import");
    };
    assert_eq!(summary.filtered_lines, 2);
    assert_eq!(summary.skipped_lines(), 0);

    let mut table = octopuson_core::table::TableBuffer::default();
    let request = crate::query::QueryRequest {
        site: "acme.com".into(),
        spec: crate::query::QuerySpec::text("group:url"),
        page: None,
    };
    crate::query::run_query(&fixture.workspace, &request, &mut table).unwrap();
    assert_eq!(
        table.rows,
        [[
            octopuson_core::table::Cell::Text("/a?page=2".into()),
            octopuson_core::table::Cell::Integer(1)
        ]]
    );

    crate::catalog::delete_import(&fixture.workspace, "acme.com", *import_id).unwrap();
    assert_eq!(fixture.stored_hits(), 0);
    // The deleted file can be imported again.
    assert_eq!(fixture.import(&[&log]).unwrap().inserted_rows, 1);
}
