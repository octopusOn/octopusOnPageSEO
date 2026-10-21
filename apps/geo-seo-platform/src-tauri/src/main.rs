//! Desktop entry point.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() {
    // AI assistants start the program with `mcp` to talk to it over standard input and output;
    // no window opens then.
    if std::env::args().nth(1).as_deref() == Some("mcp") {
        std::process::exit(octopuson_platform_desktop_lib::mcp());
    }
    octopuson_platform_desktop_lib::run();
}
