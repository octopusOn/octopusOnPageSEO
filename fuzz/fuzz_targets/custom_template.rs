//! User-supplied Nginx/Apache format strings are untrusted input: compiling them and parsing
//! lines with the result must never panic.
#![no_main]

use libfuzzer_sys::fuzz_target;
use seonexus_logparse::{LogFormat, TemplateFormat};

fuzz_target!(|input: (&str, &str)| {
    let (format, line) = input;
    if let Ok(template) = TemplateFormat::from_nginx("fuzz", "Fuzz", format) {
        let _ = template.parse(line);
    }
    if let Ok(template) = TemplateFormat::from_apache("fuzz", "Fuzz", format) {
        let _ = template.parse(line);
    }
});
