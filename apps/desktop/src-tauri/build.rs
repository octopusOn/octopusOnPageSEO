//! Tauri build script.

fn main() {
    // `generate_context!` embeds the frontend build. Lints and tests of the Rust side must not
    // require a Node toolchain, so an empty placeholder stands in until `pnpm build` ran.
    let dist = std::path::Path::new("../dist");
    if !dist.exists() {
        let _ = std::fs::create_dir_all(dist);
        let _ = std::fs::write(
            dist.join("index.html"),
            "<!doctype html><title>octopusSEO</title><p>Frontend not built: run pnpm build.</p>",
        );
    }
    tauri_build::build();
}
