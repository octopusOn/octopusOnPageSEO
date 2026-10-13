//! URL normalization must never panic and must be idempotent.
#![no_main]

use libfuzzer_sys::fuzz_target;
use seonexus_url::{NormalizeOptions, QueryMode, normalize};

fuzz_target!(|raw: &str| {
    for options in [
        NormalizeOptions::default(),
        NormalizeOptions {
            query: QueryMode::DropAll,
            ignore_trailing_slash: true,
            ignore_case: true,
        },
    ] {
        if let Some(once) = normalize(raw, &options) {
            let twice = normalize(&once.key, &options).map(|url| url.key);
            assert_eq!(twice.as_deref(), Some(once.key.as_str()));
        }
    }
});
