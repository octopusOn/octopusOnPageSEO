//! The platform's version is the desktop program's (`tauri.conf.json`), which the release script
//! sets for every release; the crates keep the workspace's. The program shows it and sends it to
//! the account service.

use std::path::Path;

fn main() {
    let config = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../apps/geo-seo-platform/src-tauri/tauri.conf.json");
    println!("cargo:rerun-if-changed={}", config.display());
    let version = std::fs::read_to_string(&config)
        .ok()
        .and_then(|text| version_of(&text))
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_owned());
    println!("cargo:rustc-env=PLATFORM_VERSION={version}");
}

/// The first `"version": "…"` of a JSON text.
fn version_of(text: &str) -> Option<String> {
    let after = text.split_once("\"version\"")?.1;
    let value = after
        .trim_start()
        .strip_prefix(':')?
        .trim_start()
        .strip_prefix('"')?;
    Some(value.split_once('"')?.0.to_owned())
}
