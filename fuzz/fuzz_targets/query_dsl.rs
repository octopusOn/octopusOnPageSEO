//! The query language must reject arbitrary text gracefully, and every query it accepts must
//! print back to text that parses to the same syntax tree.
#![no_main]

use libfuzzer_sys::fuzz_target;
use seonexus_query::{parse, to_dsl, validate};

fuzz_target!(|text: &str| {
    if let Ok(query) = parse(text) {
        let printed = to_dsl(&query);
        if validate(&query).is_ok() {
            assert_eq!(parse(&printed).ok(), Some(query), "round trip of {printed:?}");
        }
    }
});
