//! Desktop entry point.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() {
    seonexus_serp_desktop_lib::run();
}
