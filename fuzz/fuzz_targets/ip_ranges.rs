//! Parsing downloaded crawler IP lists (untrusted network input) must never panic.
#![no_main]

use libfuzzer_sys::fuzz_target;
use seonexus_botverify::ranges::parse_prefix_list;
use seonexus_botverify::IpRangeSet;

fuzz_target!(|text: &str| {
    if let Ok(prefixes) = parse_prefix_list(text) {
        let set = IpRangeSet::new(prefixes.iter().copied());
        for prefix in &prefixes {
            assert!(set.contains(prefix.address()));
        }
    }
});
