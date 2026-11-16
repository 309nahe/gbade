//! GBADE - a Game Boy Advance development environment.
//!
//! Without arguments this opens the IDE. With arguments it is a command-line
//! tool (see `gbade --help`).
//!
//! On Windows the program is built for the "windows" subsystem, so double
//! clicking `gbade.exe` opens the IDE without a console window. Terminals use
//! `gbade.com` instead (see the `gbade-console` crate), which waits for the
//! command and forwards its output and exit code.

// Not for `cargo test`: the test harness prints to the console.
#![cfg_attr(all(windows, not(test)), windows_subsystem = "windows")]

mod app;
mod assist;
mod code_intel;
mod git_panel;
mod audio;
mod cli;
mod dialogs;
mod docs;
mod editor;
mod highlight;
mod inspector;
mod panels;
mod platform;
mod resources;
mod session;
mod settings;
mod tutorials;
#[cfg(test)]
mod ui_tests;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        platform::attach_parent_console();
    }
    if let Some(code) = cli::run(&args) {
        std::process::exit(code);
    }

    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")).ok();
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_title("GBADE")
        .with_app_id("gbade")
        .with_inner_size([1400.0, 880.0])
        .with_min_inner_size([900.0, 600.0]);
    if let Some(icon) = icon {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    if let Err(e) = eframe::run_native("GBADE", options, Box::new(|cc| Ok(Box::new(app::GbadeApp::new(cc))))) {
        eprintln!("GBADE could not start: {e}");
        std::process::exit(1);
    }
}
