//! Desktop entry point.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() {
    octopuson_crawler_desktop_lib::run();
}
