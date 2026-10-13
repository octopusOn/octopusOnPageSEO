//! Every built-in format must reject arbitrary input gracefully (no panics, no hangs).
#![no_main]

use libfuzzer_sys::fuzz_target;
use seonexus_logparse::FormatRegistry;

fuzz_target!(|line: &str| {
    let registry = FormatRegistry::builtin();
    for format in registry.iter() {
        let _ = format.parse(line);
    }
});
