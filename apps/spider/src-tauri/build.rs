//! Tauri build script.

use std::error::Error;
use std::path::Path;

fn main() -> Result<(), Box<dyn Error>> {
    // `generate_context!` embeds the frontend build. Lints and tests of the Rust side must not
    // require a Node toolchain, so an empty placeholder stands in until `pnpm build` ran.
    let dist = Path::new("../dist");
    if !dist.exists() {
        let _ = std::fs::create_dir_all(dist);
        let _ = std::fs::write(
            dist.join("index.html"),
            "<!doctype html><title>OctopusOn SEO</title><p>Frontend not built: run pnpm build.</p>",
        );
    }

    let mut attributes = tauri_build::Attributes::new();
    // Tauri embeds the manifest enabling Common Controls v6 into the application binary only, so
    // test binaries fail to start on Windows (STATUS_ENTRYPOINT_NOT_FOUND). Link the same
    // manifest into every binary of the crate instead.
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        attributes = attributes
            .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
        let manifest =
            Path::new(&std::env::var("CARGO_MANIFEST_DIR")?).join("windows-app-manifest.xml");
        cargo_directive(&format!("rerun-if-changed={}", manifest.display()));
        cargo_directive("rustc-link-arg=/MANIFEST:EMBED");
        cargo_directive(&format!(
            "rustc-link-arg=/MANIFESTINPUT:{}",
            manifest.display()
        ));
    }
    tauri_build::try_build(attributes)?;
    Ok(())
}

/// Build scripts talk to Cargo through standard output.
#[allow(clippy::print_stdout)]
fn cargo_directive(directive: &str) {
    println!("cargo:{directive}");
}
