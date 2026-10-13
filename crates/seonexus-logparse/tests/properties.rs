//! Property tests: parsers never panic and valid generated lines round-trip.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers fail loudly"
)]

use proptest::prelude::*;
use seonexus_logparse::{FormatRegistry, header_formats};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2_000))]

    #[test]
    fn no_format_panics_on_arbitrary_input(line in "\\PC{0,300}") {
        for format in FormatRegistry::builtin().iter() {
            let _ = format.parse(&line);
        }
    }

    #[test]
    fn no_format_panics_on_log_like_input(
        line in r#"[0-9.:a-f]{0,20} [-a-z]{0,3} [-a-z]{0,3} \[[0-9A-Za-z/: +-]{0,30}\] "[A-Z]{0,6} [ -~]{0,40}" [0-9]{0,4} [-0-9]{0,6}( "[ -~]{0,20}"){0,4}[ 0-9a-z]{0,8}"#
    ) {
        for format in FormatRegistry::builtin().iter() {
            let _ = format.parse(&line);
        }
    }

    #[test]
    fn header_formats_never_panic(
        header in r"(#Fields: )?[a-z()_ ,;\t-]{0,80}",
        lines in proptest::collection::vec(r#"[ -~\t"{}\[\]]{0,120}"#, 0..5),
    ) {
        let mut sample = vec![header];
        sample.extend(lines);
        for format in header_formats(&sample) {
            for line in &sample {
                let _ = format.parse(line);
            }
        }
    }

    #[test]
    fn structured_formats_never_panic_on_structured_input(
        line in r#"[{"a-zA-Z_:,0-9 .\[\]}/-]{0,200}"#
    ) {
        for format in FormatRegistry::builtin().iter() {
            let _ = format.parse(&line);
        }
    }

    #[test]
    fn generated_combined_lines_round_trip(
        ip in (0_u8..=255, 0_u8..=255, 0_u8..=255, 1_u8..=254),
        day in 1_u8..=28,
        hour in 0_u8..24,
        path in "/[a-z0-9/_.-]{0,40}(\\?[a-z0-9=&%]{0,20})?",
        status in 100_u16..600,
        bytes in proptest::option::of(0_u64..10_000_000),
        agent in "[ -!#-\\[\\]-~]{1,60}",
    ) {
        let ip_text = format!("{}.{}.{}.{}", ip.0, ip.1, ip.2, ip.3);
        let bytes_text = bytes.map_or_else(|| "-".to_owned(), |b| b.to_string());
        let line = format!(
            r#"{ip_text} - - [{day:02}/Mar/2026:{hour:02}:00:00 +0000] "GET {path} HTTP/1.1" {status} {bytes_text} "-" "{agent}""#
        );
        let registry = FormatRegistry::builtin();
        let record = registry.get("combined").unwrap().parse(&line).unwrap();
        prop_assert_eq!(record.client_ip.to_string(), ip_text);
        prop_assert_eq!(record.target.as_ref(), path.as_str());
        prop_assert_eq!(record.status, status);
        prop_assert_eq!(record.bytes, bytes);
        let expected_agent = Some(agent.as_str()).filter(|a| *a != "-");
        prop_assert_eq!(record.user_agent.as_deref(), expected_agent);
    }
}
