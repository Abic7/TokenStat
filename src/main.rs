#![cfg_attr(windows, windows_subsystem = "windows")]

mod hook;
#[cfg_attr(not(windows), allow(dead_code))]
mod icon;
#[cfg_attr(not(windows), allow(dead_code))]
mod refresh;
mod snapshot;
mod sysstats;
mod view;
#[cfg(windows)]
mod app;

#[cfg(windows)]
fn main() {
    if std::env::args().any(|a| a == "--statusline") {
        hook::run();
    } else if std::env::args().any(|a| a == "--activity") {
        hook::activity();
    } else {
        app::run();
    }
}

#[cfg(not(windows))]
fn main() {
    if std::env::args().any(|a| a == "--statusline") {
        hook::run();
    } else if std::env::args().any(|a| a == "--activity") {
        hook::activity();
    } else {
        eprintln!("TokenStat runs on Windows only.");
    }
}
