//! OctopusOn Log Analyser desktop application.

// Release builds on Windows must not open a console window next to the app.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    octopuson_log_desktop_lib::run();
}
