//! Command-line interface: everything the IDE does, without a window.
//!
//! ```text
//! gbade                         open the IDE
//! gbade build [DIR]             build the project in DIR (default: current dir)
//! gbade run [DIR] [options]     build, then run headless and print the console
//!       --frames N              how many frames to run (default 120)
//!       --screenshot FILE.png   save the last frame
//!       --press KEY@FRAME       hold a button for 5 frames (KEY: a b start select up down left right l r)
//! gbade new DIR [--template T] [--name NAME]
//! gbade examples                list the templates
//! gbade --version
//! ```

use crate::resources;
use gbade_emu::{Emulator, Keys, StopReason};
use gbade_toolchain::{BuildOptions, Project, Severity, Toolchain, build};
use std::path::{Path, PathBuf};

pub const USAGE: &str = "GBADE - Game Boy Advance development environment

Usage:
  gbade                          open the IDE
  gbade build [DIR]              build the project in DIR (default: current folder)
  gbade run [DIR] [OPTIONS]      build and run without a window, print the console
      --frames N                 frames to run (default 120, ~2 seconds)
      --screenshot FILE.png      save the last frame as a PNG
      --press KEY@FRAME          hold a button for 5 frames from FRAME
                                 (KEY: a b start select up down left right l r)
  gbade new DIR [--template NAME] [--name \"Game Name\"]
                                 create a project from an example
  gbade examples                 list the examples/templates
  gbade install-desktop          Linux: add GBADE to the applications menu
  gbade --version
";

/// Run a CLI command. Returns the process exit code, or None to start the GUI.
pub fn run(args: &[String]) -> Option<i32> {
    let cmd = args.first().map(String::as_str)?;
    // macOS passes -psn_... when launched from Finder on old systems.
    if cmd.starts_with("-psn") {
        return None;
    }
    Some(match cmd {
        "--version" | "-V" | "version" => {
            println!("gbade {}", env!("CARGO_PKG_VERSION"));
            0
        }
        "help" | "--help" | "-h" => {
            print!("{USAGE}");
            0
        }
        "build" => cmd_build(args.get(1).map(PathBuf::from)),
        "run" => cmd_run(&args[1..]),
        "new" => cmd_new(&args[1..]),
        "install-desktop" => cmd_install_desktop(),
        "examples" => {
            for e in resources::examples() {
                println!(
                    "{:<20} {}",
                    e.dir.file_name().unwrap_or_default().to_string_lossy(),
                    e.description
                );
            }
            0
        }
        other => {
            eprintln!("unknown command `{other}`\n\n{USAGE}");
            2
        }
    })
}

fn find_project(dir: Option<PathBuf>) -> Result<Project, String> {
    let dir = dir.unwrap_or_else(|| std::env::current_dir().unwrap());
    let root = Project::find(&dir.canonicalize().unwrap_or(dir.clone()))
        .ok_or_else(|| format!("no gbade.toml found in {} or its parents", dir.display()))?;
    Project::open(&root).map_err(|e| e.0)
}

fn do_build(project: &Project) -> Option<gbade_toolchain::BuildOutput> {
    let tc = match Toolchain::locate() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{e}");
            return None;
        }
    };
    let out = build(project, &tc, &BuildOptions::default());
    for d in &out.diagnostics {
        if d.severity == Severity::Note {
            continue;
        }
        eprintln!("{}", d.summary());
        if let Some(h) = &d.hint {
            eprintln!("  hint: {h}");
        }
    }
    if out.success {
        if let Some(m) = out.memory {
            eprintln!(
                "built {} ({}, IWRAM {}, EWRAM {}) in {:.2}s",
                out.rom_path.as_ref().unwrap().display(),
                gbade_toolchain::builder::human_size(m.rom),
                gbade_toolchain::builder::human_size(m.iwram),
                gbade_toolchain::builder::human_size(m.ewram),
                out.duration.as_secs_f32()
            );
        }
    } else {
        eprintln!("build failed");
    }
    Some(out)
}

fn cmd_build(dir: Option<PathBuf>) -> i32 {
    let project = match find_project(dir) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    match do_build(&project) {
        Some(o) if o.success => 0,
        _ => 1,
    }
}

pub fn parse_key(name: &str) -> Option<Keys> {
    Some(match name.to_ascii_lowercase().as_str() {
        "a" => Keys::A,
        "b" => Keys::B,
        "start" => Keys::START,
        "select" => Keys::SELECT,
        "up" => Keys::UP,
        "down" => Keys::DOWN,
        "left" => Keys::LEFT,
        "right" => Keys::RIGHT,
        "l" => Keys::L,
        "r" => Keys::R,
        _ => return None,
    })
}

fn cmd_run(args: &[String]) -> i32 {
    let mut dir = None;
    let mut frames = 120u32;
    let mut screenshot: Option<PathBuf> = None;
    let mut presses: Vec<(Keys, u32)> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--frames" => {
                frames = args.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(frames);
                i += 1;
            }
            "--screenshot" => {
                screenshot = args.get(i + 1).map(PathBuf::from);
                i += 1;
            }
            "--press" => {
                if let Some((k, f)) = args.get(i + 1).and_then(|s| s.split_once('@')) {
                    match (parse_key(k), f.parse::<u32>()) {
                        (Some(k), Ok(f)) => presses.push((k, f)),
                        _ => {
                            eprintln!("bad --press value {k}@{f} (example: --press a@30)");
                            return 2;
                        }
                    }
                }
                i += 1;
            }
            other if !other.starts_with("--") && dir.is_none() => dir = Some(PathBuf::from(other)),
            other => {
                eprintln!("unknown option {other}");
                return 2;
            }
        }
        i += 1;
    }
    let project = match find_project(dir) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    let Some(out) = do_build(&project) else { return 1 };
    if !out.success {
        return 1;
    }
    let rom = std::fs::read(out.rom_path.as_ref().unwrap()).unwrap();
    let mut emu = Emulator::new();
    let save = std::fs::read(project.save_path()).ok();
    if let Err(e) = emu.load_rom(&rom, save.as_deref()) {
        eprintln!("{e}");
        return 1;
    }
    let mut code = 0;
    for f in 0..frames {
        let mut keys = Keys::NONE;
        for (k, at) in &presses {
            if f >= *at && f < at + 5 {
                keys |= *k;
            }
        }
        emu.set_keys(keys);
        let r = emu.run_frame();
        let mut audio = Vec::new();
        emu.drain_audio(&mut audio);
        for l in emu.take_logs() {
            if l.is_game_output() {
                println!("{}", l.message.trim_start_matches('\u{1}'));
            } else if l.level <= gbade_emu::LogLevel::Warn {
                eprintln!("[{}] {}", l.category, l.message);
            }
            if l.message.contains("GBADE:PANIC") {
                code = 3;
            }
        }
        if r != StopReason::FrameDone {
            eprintln!("emulation stopped at frame {f}: {r:?}");
            code = 3;
            break;
        }
        if code != 0 {
            break;
        }
    }
    let data = emu.save_data();
    if !data.is_empty() {
        let _ = std::fs::write(project.save_path(), data);
    }
    if let Some(path) = screenshot {
        let img = image::RgbaImage::from_raw(240, 160, emu.framebuffer_rgba()).unwrap();
        if let Err(e) = img.save(&path) {
            eprintln!("cannot save screenshot: {e}");
            return 1;
        }
        eprintln!("screenshot saved to {}", path.display());
    }
    code
}

fn cmd_new(args: &[String]) -> i32 {
    let mut dir: Option<PathBuf> = None;
    let mut template = "hello-world".to_string();
    let mut name: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--template" => {
                template = args.get(i + 1).cloned().unwrap_or(template);
                i += 1;
            }
            "--name" => {
                name = args.get(i + 1).cloned();
                i += 1;
            }
            other => dir = Some(PathBuf::from(other)),
        }
        i += 1;
    }
    let Some(dir) = dir else {
        eprintln!("usage: gbade new DIR [--template NAME] [--name \"Game Name\"]");
        return 2;
    };
    let examples = resources::examples();
    let Some(tpl) = examples.iter().find(|e| e.dir.file_name().is_some_and(|n| n == template.as_str())) else {
        eprintln!("unknown template `{template}`. Available:");
        for e in &examples {
            eprintln!("  {}", e.dir.file_name().unwrap().to_string_lossy());
        }
        return 2;
    };
    let name = name.unwrap_or_else(|| dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or("My Game".into()));
    match Project::create_from_template(&tpl.dir, &dir, &name) {
        Ok(p) => {
            println!("created {} in {}", p.config.game.name, p.root.display());
            0
        }
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}

/// Write a freedesktop.org launcher entry and icon for the current executable,
/// so GBADE appears in the applications menu (GNOME, KDE, ...).
fn cmd_install_desktop() -> i32 {
    if cfg!(target_os = "macos") {
        eprintln!("On macOS, drag GBADE.app to Applications instead.");
        return 2;
    }
    let Ok(exe) = std::env::current_exe().and_then(|e| e.canonicalize()) else {
        eprintln!("cannot find the path of this executable");
        return 1;
    };
    let data = match std::env::var_os("XDG_DATA_HOME") {
        Some(d) => PathBuf::from(d),
        None => PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share"),
    };
    let icon = data.join("icons/hicolor/512x512/apps/gbade.png");
    let entry = data.join("applications/gbade.desktop");
    let desktop = format!(
        "[Desktop Entry]\nType=Application\nName=GBADE\nGenericName=Game Boy Advance IDE\n\
         Comment=Make Game Boy Advance games: editor, compiler and emulator\n\
         Exec=\"{}\" %F\nIcon=gbade\nTerminal=false\nCategories=Development;IDE;\nStartupWMClass=gbade\n",
        exe.display()
    );
    let result = (|| -> std::io::Result<()> {
        std::fs::create_dir_all(icon.parent().unwrap())?;
        std::fs::write(&icon, include_bytes!("../assets/icon.png"))?;
        std::fs::create_dir_all(entry.parent().unwrap())?;
        std::fs::write(&entry, desktop)?;
        Ok(())
    })();
    match result {
        Ok(()) => {
            println!("installed {}", entry.display());
            println!("GBADE now appears in your applications menu (it runs {}).", exe.display());
            0
        }
        Err(e) => {
            eprintln!("could not install the desktop entry: {e}");
            1
        }
    }
}

#[allow(dead_code)]
pub fn is_project(p: &Path) -> bool {
    p.join(gbade_toolchain::project::PROJECT_FILE).exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_names() {
        assert_eq!(parse_key("A"), Some(Keys::A));
        assert_eq!(parse_key("start"), Some(Keys::START));
        assert_eq!(parse_key("x"), None);
    }

    #[test]
    fn version_and_unknown() {
        assert_eq!(run(&["--version".into()]), Some(0));
        assert_eq!(run(&["frobnicate".into()]), Some(2));
        assert_eq!(run(&[]), None);
    }
}
