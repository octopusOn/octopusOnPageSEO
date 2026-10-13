use std::io::Write;

use super::*;

const TRAEFIK: &str = r#"57.141.20.59 - - [07/Oct/2026:18:39:53 +0000] "GET /a HTTP/2.0" 200 535 "-" "UA" 1 "r@docker" "http://172.18.0.6:3000" 5ms
45.231.253.129 - - [07/Oct/2026:18:39:55 +0000] "GET /b HTTP/2.0" 404 33 "-" "UA" 2 "r@docker" "http://172.18.0.6:3000" 2ms

not a log line
"#;

fn write_log(dir: &tempfile::TempDir, name: &str, body: &str) -> PathBuf {
    let path = dir.path().join(name);
    std::fs::File::create(&path)
        .unwrap()
        .write_all(body.as_bytes())
        .unwrap();
    path
}

fn request(files: Vec<PathBuf>, format: FormatChoice) -> InspectRequest {
    InspectRequest {
        files,
        format,
        max_error_samples: 5,
    }
}

#[test]
fn auto_detects_and_counts() {
    let dir = tempfile::tempdir().unwrap();
    let file = write_log(&dir, "access.log", TRAEFIK);
    let report = inspect(&request(vec![file], FormatChoice::Auto)).unwrap();

    assert_eq!(report.format_id, "traefik-clf");
    assert_eq!(report.summary.total_lines, 4);
    assert_eq!(report.summary.parsed_lines, 2);
    assert_eq!(report.summary.skipped_by_reason.get("empty line"), Some(&1));
    assert_eq!(
        report.summary.error_samples.len(),
        1,
        "empty lines are not sampled"
    );
    assert_eq!(report.summary.error_samples[0].line_number, 4);
    assert_eq!(
        report.summary.first_request.as_deref(),
        Some("2026-10-07T18:39:53Z")
    );
    assert_eq!(
        report.summary.last_request.as_deref(),
        Some("2026-10-07T18:39:55Z")
    );
    assert_eq!(report.summary.status_classes.get("2xx"), Some(&1));
    assert_eq!(report.summary.status_classes.get("4xx"), Some(&1));
}

#[test]
fn explicit_unknown_format_lists_alternatives() {
    let dir = tempfile::tempdir().unwrap();
    let file = write_log(&dir, "access.log", TRAEFIK);
    let error = inspect(&request(
        vec![file],
        FormatChoice::Builtin("lighttpd-xyz".into()),
    ))
    .unwrap_err();
    let message = error.to_string();
    assert!(
        message.contains("lighttpd-xyz") && message.contains("traefik-clf"),
        "{message}"
    );
}

#[test]
fn undetectable_input_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let file = write_log(&dir, "notes.txt", "hello\nworld\n");
    let error = inspect(&request(vec![file], FormatChoice::Auto)).unwrap_err();
    assert!(matches!(
        error,
        InspectError::Format(FormatError::NotDetected(_))
    ));
}

#[test]
fn no_files_is_an_error() {
    assert!(matches!(
        inspect(&request(vec![], FormatChoice::Auto)),
        Err(InspectError::NoFiles)
    ));
}
