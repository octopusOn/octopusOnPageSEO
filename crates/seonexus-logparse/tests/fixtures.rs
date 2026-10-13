//! Fixture-driven tests through the public API: detection + parsing + snapshots.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers fail loudly"
)]

use std::path::Path;

use seonexus_logparse::{FormatRegistry, detect};

fn fixture_lines(name: &str) -> Vec<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

/// Detects the format, asserts it is the expected one and every line parses.
fn parse_fixture(name: &str, expected_format: &str) -> Vec<String> {
    let lines = fixture_lines(name);
    let registry = FormatRegistry::builtin();
    let detection = detect(&registry, &lines).expect("a format must be detected");
    assert_eq!(
        detection.format.id(),
        expected_format,
        "wrong format for {name}"
    );
    assert_eq!(detection.matched, lines.len(), "every line must parse");
    lines
        .iter()
        .map(|line| format!("{:#?}", detection.format.parse(line).unwrap()))
        .collect()
}

#[test]
fn traefik_clf_fixture() {
    insta::assert_snapshot!(parse_fixture("traefik-clf.log", "traefik-clf").join("\n"));
}

#[test]
fn combined_fixture() {
    insta::assert_snapshot!(parse_fixture("combined.log", "combined").join("\n"));
}

#[test]
fn common_fixture() {
    insta::assert_snapshot!(parse_fixture("common.log", "common").join("\n"));
}

#[test]
fn traefik_json_fixture() {
    insta::assert_snapshot!(parse_fixture("traefik-json.log", "traefik-json").join("\n"));
}

#[test]
fn detection_tolerates_a_few_bad_lines() {
    let mut lines = fixture_lines("combined.log");
    lines.push("garbage line".to_owned());
    lines.push(String::new());
    let detection = detect(&FormatRegistry::builtin(), &lines).unwrap();
    assert_eq!(detection.format.id(), "combined");
    assert_eq!(detection.sampled, 5, "empty lines are not sampled");
    assert!((detection.confidence() - 0.8).abs() < f64::EPSILON);
}

#[test]
fn detection_returns_none_for_unknown_content() {
    assert!(detect(&FormatRegistry::builtin(), &["hello", "world"]).is_none());
}
