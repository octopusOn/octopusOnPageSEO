//! Formats built from a header in the input (W3C `#Fields:`, CSV header rows) must handle any
//! header and any following line without panicking, and detection must stay total.
#![no_main]

use libfuzzer_sys::fuzz_target;
use octopuson_logparse::{FormatRegistry, detect, header_formats};

fuzz_target!(|text: &str| {
    let lines: Vec<&str> = text.lines().collect();
    for format in header_formats(&lines) {
        for line in &lines {
            let _ = format.parse(line);
        }
    }
    let _ = detect(&FormatRegistry::builtin(), &lines);
});
