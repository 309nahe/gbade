//! GBADE - a Game Boy Advance development environment.
//!
//! Without arguments this opens the IDE. With arguments it is a command-line
//! tool (see `gbade --help`).

mod app;
mod audio;
mod cli;
mod dialogs;
mod docs;
mod editor;
mod highlight;
mod inspector;
mod panels;
mod resources;
mod session;
mod settings;
#[cfg(test)]
mod ui_tests;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(code) = cli::run(&args) {
        std::process::exit(code);
    }

    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")).ok();
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_title("GBADE")
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
