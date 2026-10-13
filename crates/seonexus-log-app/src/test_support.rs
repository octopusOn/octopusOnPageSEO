//! Shared fixtures for use-case tests.

use std::path::PathBuf;

use seonexus_core::table::TableBuffer;

use crate::catalog::{AddSiteRequest, add_client, add_site};
use crate::import::{ImportRequest, NoProgress, import_logs};
use crate::query::{QueryOutcome, QueryRequest, QuerySpec, run_query};
use crate::{CancelToken, FormatChoice, ImportOptions, Workspace};

/// Combined-format log: verified Googlebot, a spoofed GPTBot, Googlebot outside Google's
/// ranges and browsers, over two days.
pub(crate) const LOG: &str = r#"66.249.66.1 - - [07/Oct/2026:18:39:53 +0000] "GET /a HTTP/1.1" 200 535 "-" "Googlebot/2.1"
66.249.66.1 - - [07/Oct/2026:18:40:00 +0000] "GET /missing HTTP/1.1" 404 33 "-" "Googlebot/2.1"
10.0.0.1 - - [07/Oct/2026:18:41:00 +0000] "GET /missing HTTP/1.1" 404 33 "-" "Mozilla/5.0"
10.0.0.1 - - [08/Oct/2026:09:00:00 +0000] "GET /a?x=1 HTTP/1.1" 200 535 "-" "Mozilla/5.0"
10.0.0.2 - - [08/Oct/2026:09:00:01 +0000] "GET /a HTTP/1.1" 200 535 "-" "Googlebot/2.1"
10.0.0.3 - - [08/Oct/2026:09:00:02 +0000] "GET /a HTTP/1.1" 200 535 "-" "GPTBot/1.2"
"#;

/// A workspace with client `Acme` and site `acme.com` (combined format).
pub(crate) struct Fixture {
    pub(crate) dir: tempfile::TempDir,
    pub(crate) workspace: Workspace,
}

impl Fixture {
    pub(crate) fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let workspace = Workspace::at(dir.path().join("data"));
        add_client(&workspace, "Acme").unwrap();
        add_site(
            &workspace,
            &AddSiteRequest {
                client: "Acme".into(),
                name: "acme.com".into(),
                format: FormatChoice::Builtin("combined".into()),
            },
        )
        .unwrap();
        Self { dir, workspace }
    }

    /// The fixture with [`LOG`] imported (DNS disabled).
    pub(crate) fn imported() -> Self {
        let fixture = Self::new();
        fixture.import("access.log", LOG);
        fixture
    }

    /// Imports `body` as another log file of `acme.com` (DNS disabled).
    pub(crate) fn import(&self, name: &str, body: &str) {
        let path = self.write(name, body);
        let request = ImportRequest {
            site: "acme.com".into(),
            paths: vec![path],
            max_error_samples: 5,
            options: ImportOptions::default(),
            verify_dns: false,
        };
        import_logs(&self.workspace, &request, &NoProgress, &CancelToken::new()).unwrap();
    }

    pub(crate) fn write(&self, name: &str, body: &str) -> PathBuf {
        let path = self.path(name);
        std::fs::write(&path, body).unwrap();
        path
    }

    pub(crate) fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    pub(crate) fn run(&self, text: &str) -> (QueryOutcome, TableBuffer) {
        let mut buffer = TableBuffer::default();
        let request = QueryRequest {
            site: "acme.com".into(),
            spec: QuerySpec::text(text),
            page: None,
        };
        let outcome = run_query(&self.workspace, &request, &mut buffer).unwrap();
        (outcome, buffer)
    }
}
