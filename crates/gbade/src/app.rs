//! The GBADE application: state and actions. Drawing is in `panels.rs` and
//! `dialogs.rs`.

use crate::audio::AudioOut;
use crate::docs::DocsViewer;
use crate::editor::Document;
use crate::inspector::Inspector;
use crate::session::Session;
use crate::settings::Settings;
use eframe::egui::{self, Key, KeyboardShortcut, Modifiers, TextureHandle};
use gbade_emu::Keys;
use gbade_toolchain::{BuildOptions, BuildOutput, Diagnostic, Project, Severity, Toolchain};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BottomTab {
    Problems,
    Console,
    BuildLog,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CenterView {
    Editor,
    Learn,
    /// The hands-on tutorials.
    Tutorials,
    /// A git diff (changes of a file, or of a commit).
    Diff,
}

pub struct BuildJob {
    rx: mpsc::Receiver<BuildOutput>,
    run_after: bool,
    pub started: Instant,
}

/// A modal question waiting for an answer.
pub enum Confirm {
    DeleteFile(PathBuf),
    CloseWithUnsaved,
    OpenProjectWithUnsaved(PathBuf),
    /// Restore a file from the last git commit (path relative to the repo).
    DiscardFile(PathBuf),
}

pub struct GbadeApp {
    pub settings: Settings,
    pub project: Option<Project>,
    pub toolchain: Result<Toolchain, String>,
    pub docs: Vec<Document>,
    pub active_doc: usize,
    pub breakpoints: BTreeMap<PathBuf, BTreeSet<u32>>,
    pub diagnostics: Vec<Diagnostic>,
    pub build_log: String,
    pub build_job: Option<BuildJob>,
    pub last_build: Option<BuildOutput>,
    pub session: Option<Session>,
    pub screen_tex: Option<TextureHandle>,
    pub audio: AudioOut,
    pub inspector: Inspector,
    pub docs_viewer: DocsViewer,
    pub tutorials_viewer: crate::tutorials::TutorialsViewer,
    pub bottom_tab: BottomTab,
    pub center: CenterView,
    pub last_tick: Instant,
    pub status: Option<(String, Instant, bool)>,
    pub confirm: Option<Confirm>,
    pub allow_close: bool,
    // dialogs
    pub new_project_open: bool,
    pub new_project_name: String,
    pub new_project_template: usize,
    pub new_project_dir: Option<PathBuf>,
    pub new_file_open: bool,
    pub new_file_name: String,
    pub settings_open: bool,
    pub about_open: bool,
    /// Where the ROM was just exported to: shown in a small notice until dismissed.
    pub exported_to: Option<PathBuf>,
    pub binding_key: Option<usize>,
    pub console_game_only: bool,
    pub save_state: Option<Vec<u8>>,
    pub fps: (Instant, u32, f32),
    pub dirty_since_build: bool,
    /// The C language server (clangd), while a project is open.
    pub lsp: Option<crate::code_intel::Lsp>,
    /// Live problems from clangd, per file (replace the build's for that file).
    pub live_diagnostics: std::collections::HashMap<PathBuf, Vec<Diagnostic>>,
    /// The project's git repository, if it is tracked.
    pub git: Option<gbade_git::Repo>,
    pub git_ui: crate::git_panel::GitUi,
}

impl GbadeApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> GbadeApp {
        let settings: Settings = cc.storage.and_then(|s| eframe::get_value(s, eframe::APP_KEY)).unwrap_or_default();
        Self::with_settings(&cc.egui_ctx, settings)
    }

    pub fn with_settings(ctx: &egui::Context, settings: Settings) -> GbadeApp {
        let audio = AudioOut::new();
        audio.set_volume(if settings.muted { 0.0 } else { settings.volume });
        let mut app = GbadeApp {
            toolchain: Toolchain::locate().map_err(|e| e.0),
            settings,
            project: None,
            docs: Vec::new(),
            active_doc: 0,
            breakpoints: BTreeMap::new(),
            diagnostics: Vec::new(),
            build_log: String::new(),
            build_job: None,
            last_build: None,
            session: None,
            screen_tex: None,
            audio,
            inspector: Inspector::default(),
            docs_viewer: DocsViewer::default(),
            tutorials_viewer: Default::default(),
            bottom_tab: BottomTab::Console,
            center: CenterView::Editor,
            last_tick: Instant::now(),
            status: None,
            confirm: None,
            allow_close: false,
            new_project_open: false,
            new_project_name: "My Game".into(),
            new_project_template: 0,
            new_project_dir: None,
            new_file_open: false,
            new_file_name: String::new(),
            settings_open: false,
            about_open: false,
            exported_to: None,
            binding_key: None,
            console_game_only: false,
            save_state: None,
            fps: (Instant::now(), 0, 0.0),
            dirty_since_build: false,
            lsp: None,
            live_diagnostics: Default::default(),
            git: None,
            git_ui: Default::default(),
        };
        app.apply_theme(ctx);
        // Reopen the last project.
        if let Some(last) = app.settings.recent_projects.first().cloned() {
            if last.join(gbade_toolchain::project::PROJECT_FILE).exists() {
                app.open_project(&last);
            }
        }
        app
    }

    /// One selectable entry per theme; true when the theme changed.
    pub fn theme_picker(&mut self, ui: &mut egui::Ui) -> bool {
        let current = self.settings.theme();
        let mut changed = false;
        for id in crate::themes::ThemeId::ALL {
            if ui.selectable_label(current == id, id.name()).clicked() && current != id {
                self.settings.set_theme(id);
                changed = true;
            }
        }
        changed
    }

    pub fn apply_theme(&self, ctx: &egui::Context) {
        crate::themes::apply(ctx, self.settings.theme());
    }

    pub fn set_status(&mut self, msg: impl Into<String>, error: bool) {
        self.status = Some((msg.into(), Instant::now(), error));
    }

    // ------------------------------------------------------------------
    // Projects & files
    // ------------------------------------------------------------------

    pub fn open_project(&mut self, dir: &Path) {
        match Project::open(dir) {
            Ok(p) => {
                self.stop_game();
                self.docs.clear();
                self.breakpoints.clear();
                self.diagnostics.clear();
                self.build_log.clear();
                self.last_build = None;
                self.settings.add_recent(p.root.clone());
                // Open main.c (or the first source file).
                let main = p.src_dir().join("main.c");
                let first = if main.exists() { Some(main) } else { p.sources().first().map(|(f, _)| f.clone()) };
                self.project = Some(p);
                self.start_lsp();
                self.open_git();
                if let Some(f) = first {
                    self.open_file(&f, None);
                }
                self.center = CenterView::Editor;
                self.set_status(crate::platform::k("Project opened. Press ⌘R to build and run."), false);
            }
            Err(e) => self.set_status(e.0, true),
        }
    }

    /// Ask for confirmation if there are unsaved files, then open.
    pub fn request_open_project(&mut self, dir: PathBuf) {
        if self.docs.iter().any(Document::is_dirty) {
            self.confirm = Some(Confirm::OpenProjectWithUnsaved(dir));
        } else {
            self.open_project(&dir);
        }
    }

    pub fn open_file(&mut self, path: &Path, line: Option<u32>) {
        let path = gbade_toolchain::paths::canonical(path);
        if let Some(i) = self.docs.iter().position(|d| d.path == path) {
            self.active_doc = i;
        } else {
            match Document::open(&path) {
                Ok(mut d) => {
                    // Files outside the project (SDK headers reached with "go
                    // to definition") are read-only.
                    d.read_only = self.project.as_ref().is_some_and(|p| !path.starts_with(&p.root));
                    self.docs.push(d);
                    self.active_doc = self.docs.len() - 1;
                }
                Err(e) => {
                    self.set_status(format!("Cannot open {}: {e}", path.display()), true);
                    return;
                }
            }
        }
        if let Some(l) = line {
            self.docs[self.active_doc].goto_line = Some(l);
        }
        self.center = CenterView::Editor;
    }

    pub fn close_doc(&mut self, i: usize) {
        if i < self.docs.len() {
            self.docs.remove(i);
            if self.active_doc >= self.docs.len() {
                self.active_doc = self.docs.len().saturating_sub(1);
            }
        }
    }

    pub fn save_all(&mut self) -> bool {
        let mut ok = true;
        let mut saved = 0;
        for d in &mut self.docs {
            if d.is_dirty() {
                match d.save() {
                    Ok(()) => saved += 1,
                    Err(e) => {
                        ok = false;
                        self.status = Some((format!("Cannot save {}: {e}", d.path.display()), Instant::now(), true));
                    }
                }
            }
        }
        if saved > 0 && ok {
            self.dirty_since_build = true;
            self.set_status(format!("Saved {saved} file(s)."), false);
        }
        self.refresh_git();
        ok
    }

    /// "Try this example" in the Tutorials panel: copy the tutorial's project
    /// into the projects folder (a new folder every time, so earlier attempts
    /// are kept), open it, then build and run it. Returns the new project's
    /// folder.
    pub fn try_tutorial(&mut self, t: &crate::tutorials::Tutorial) -> Result<PathBuf, String> {
        let base = self.settings.projects_dir();
        std::fs::create_dir_all(&base).map_err(|e| format!("cannot create {}: {e}", base.display()))?;
        let slug = gbade_toolchain::project::slugify(&t.name);
        let mut dest = base.join(&slug);
        let mut n = 2;
        while dest.exists() {
            dest = base.join(format!("{slug}-{n}"));
            n += 1;
        }
        let project = Project::create_from_template(&t.dir, &dest, &t.name).map_err(|e| e.0)?;
        let root = project.root.clone();
        if self.docs.iter().any(Document::is_dirty) {
            // Never lose unsaved work: ask first, like Open Project does.
            self.confirm = Some(Confirm::OpenProjectWithUnsaved(root.clone()));
        } else {
            self.open_project(&root);
            self.start_build(true);
        }
        Ok(root)
    }

    pub fn create_file(&mut self, name: &str) -> Result<PathBuf, String> {
        let Some(p) = &self.project else { return Err("no project open".into()) };
        let name = name.trim();
        if name.is_empty() || name.contains("..") {
            return Err("invalid file name".into());
        }
        let ext = Path::new(name).extension().and_then(|e| e.to_str()).unwrap_or("");
        let dir = match ext {
            "c" | "s" | "S" => p.src_dir(),
            "h" => p.include_dir(),
            "png" => return Err("to add an image, copy the PNG file into the project's assets/ folder".into()),
            _ => return Err("use a .c, .h or .s extension (e.g. player.c)".into()),
        };
        let path = dir.join(name);
        if path.exists() {
            return Err(format!("{name} already exists"));
        }
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        let stem = Path::new(name).file_stem().unwrap().to_string_lossy().to_string();
        let content = match ext {
            "h" => {
                let guard = format!("{}_H", gbade_toolchain::assets::c_ident(&stem).to_ascii_uppercase());
                format!("#ifndef {guard}\n#define {guard}\n\n#include <gba.h>\n\n\n#endif\n")
            }
            "c" => format!("#include <gba.h>\n\n// {stem}.c\n\n"),
            _ => "@ ARM/Thumb assembly\n    .syntax unified\n    .text\n".into(),
        };
        std::fs::write(&path, content).map_err(|e| e.to_string())?;
        Ok(path)
    }

    // ------------------------------------------------------------------
    // Building & running
    // ------------------------------------------------------------------

    pub fn start_build(&mut self, run_after: bool) {
        if self.build_job.is_some() {
            return;
        }
        let Some(project) = self.project.clone() else {
            self.set_status("Open or create a project first.", true);
            return;
        };
        let tc = match &self.toolchain {
            Ok(t) => t.clone(),
            Err(e) => {
                self.build_log = e.clone();
                self.bottom_tab = BottomTab::BuildLog;
                self.set_status("The bundled compiler was not found (see Build log).", true);
                return;
            }
        };
        self.save_all();
        // Reload the project config in case gbade.toml was edited.
        let project = Project::open(&project.root).unwrap_or(project);
        self.project = Some(project.clone());
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let out = gbade_toolchain::build(&project, &tc, &BuildOptions::default());
            let _ = tx.send(out);
        });
        self.build_job = Some(BuildJob { rx, run_after, started: Instant::now() });
        self.set_status("Building…", false);
    }

    pub fn poll_build(&mut self) {
        let Some(job) = &self.build_job else { return };
        let Ok(out) = job.rx.try_recv() else { return };
        let run_after = job.run_after;
        self.build_job = None;
        self.dirty_since_build = false;
        self.diagnostics = out.diagnostics.clone();
        self.build_log = out.log.clone();
        if out.success {
            let w = out.warnings().count();
            self.set_status(
                format!(
                    "Build succeeded in {:.1}s{}",
                    out.duration.as_secs_f32(),
                    if w > 0 { format!(" ({w} warning(s))") } else { String::new() }
                ),
                false,
            );
            if run_after {
                self.launch(&out);
            }
            if w > 0 && !run_after {
                self.bottom_tab = BottomTab::Problems;
            }
        } else {
            let n = out.errors().count();
            self.set_status(format!("Build failed with {n} error(s). See Problems."), true);
            self.bottom_tab = BottomTab::Problems;
            if let Some(d) = out.errors().find(|d| d.file.is_some()).cloned() {
                if let Some(f) = &d.file {
                    if f.exists() {
                        self.open_file(f, Some(d.line.max(1)));
                    }
                }
            }
        }
        self.last_build = Some(out);
    }

    fn launch(&mut self, out: &BuildOutput) {
        let (Some(rom_path), Some(project)) = (&out.rom_path, &self.project) else { return };
        if let Some(s) = &self.session {
            s.persist_save();
        }
        let rom = match std::fs::read(rom_path) {
            Ok(r) => r,
            Err(e) => {
                self.set_status(format!("Cannot read ROM: {e}"), true);
                return;
            }
        };
        let elf = out.elf_path.as_ref().and_then(|p| std::fs::read(p).ok());
        match Session::new(rom, elf.as_deref(), Some(project.save_path()), self.audio.sample_rate) {
            Ok(mut s) => {
                let report = s.set_breakpoints(&self.breakpoints);
                for (file, line, actual) in report {
                    let name = file.file_name().unwrap_or_default().to_string_lossy().to_string();
                    match actual {
                        Some(a) => s.console.push(info_line(format!("Breakpoint {name}:{line} moved to line {a} (no code on line {line})."))),
                        None => s.console.push(info_line(format!("Breakpoint {name}:{line} ignored: no code there."))),
                    }
                }
                self.session = Some(s);
                self.audio.clear();
                self.last_tick = Instant::now();
                self.bottom_tab = BottomTab::Console;
            }
            Err(e) => self.set_status(format!("The emulator could not load the ROM: {e}"), true),
        }
    }

    pub fn stop_game(&mut self) {
        if let Some(s) = self.session.take() {
            s.persist_save();
        }
        self.audio.clear();
    }

    pub fn toggle_pause(&mut self) {
        if let Some(s) = &mut self.session {
            if s.is_paused() {
                s.resume();
                self.last_tick = Instant::now();
            } else {
                s.pause_user();
                self.show_pause_location();
            }
        }
    }

    pub fn toggle_breakpoint(&mut self, file: &Path, line: u32) {
        let set = self.breakpoints.entry(file.to_path_buf()).or_default();
        if !set.insert(line) {
            set.remove(&line);
        }
        if let Some(s) = &mut self.session {
            s.set_breakpoints(&self.breakpoints);
        }
    }

    /// After a pause, open the source file at the paused line.
    pub fn show_pause_location(&mut self) {
        let loc = self.session.as_ref().and_then(|s| s.pause.as_ref()).and_then(|p| p.location.clone());
        if let Some((file, line)) = loc {
            if file.exists() {
                self.open_file(&file, Some(line));
            }
        }
        self.inspector.tab = crate::inspector::Tab::Cpu;
    }

    /// Game keys for this frame (only when no text field has focus).
    pub fn game_keys(&self, ctx: &egui::Context) -> Keys {
        if ctx.wants_keyboard_input() {
            return Keys::NONE;
        }
        ctx.input(|i| self.settings.keymap.keys_down(|k| i.key_down(k)))
    }

    /// Advance the emulator according to elapsed real time.
    pub fn tick(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_tick).min(Duration::from_millis(100));
        self.last_tick = now;
        let keys = self.game_keys(ctx);
        let mut audio = Vec::new();
        let mut paused_now = false;
        if let Some(s) = &mut self.session {
            let was_running = !s.is_paused();
            let frames = s.tick(dt, keys, &mut audio);
            self.fps.1 += frames;
            paused_now = was_running && s.is_paused();
        }
        if !audio.is_empty() {
            self.audio.push(&audio);
        }
        if paused_now {
            self.show_pause_location();
        }
        let el = self.fps.0.elapsed();
        if el >= Duration::from_secs(1) {
            self.fps.2 = self.fps.1 as f32 / el.as_secs_f32();
            self.fps = (Instant::now(), 0, self.fps.2);
        }
    }

    pub fn update_screen_texture(&mut self, ctx: &egui::Context) {
        let Some(s) = &self.session else { return };
        let img = egui::ColorImage::from_rgba_unmultiplied([240, 160], &s.emu.framebuffer_rgba());
        match &mut self.screen_tex {
            Some(t) => t.set(img, egui::TextureOptions::NEAREST),
            None => self.screen_tex = Some(ctx.load_texture("gba-screen", img, egui::TextureOptions::NEAREST)),
        }
    }

    pub fn save_screenshot(&mut self) {
        let (Some(s), Some(p)) = (&self.session, &self.project) else { return };
        let dir = p.build_dir().join("screenshots");
        let _ = std::fs::create_dir_all(&dir);
        let n = std::fs::read_dir(&dir).map(|r| r.count()).unwrap_or(0) + 1;
        let path = dir.join(format!("{}-{n:03}.png", p.slug()));
        let img = image::RgbaImage::from_raw(240, 160, s.emu.framebuffer_rgba()).unwrap();
        match img.save(&path) {
            Ok(()) => self.set_status(format!("Screenshot saved to {}", path.display()), false),
            Err(e) => self.set_status(format!("Screenshot failed: {e}"), true),
        }
    }

    pub fn export_rom(&mut self) {
        let Some(p) = &self.project else { return };
        let rom = p.rom_path();
        if !rom.exists() {
            self.set_status(crate::platform::k("Build the project first (⌘B)."), true);
            return;
        }
        if let Some(dest) = rfd::FileDialog::new()
            .set_file_name(rom.file_name().unwrap().to_string_lossy())
            .add_filter("GBA ROM", &["gba"])
            .save_file()
        {
            self.copy_rom_to(&rom, dest);
        }
    }

    /// Copy the built ROM to `dest` and tell the user where it went.
    pub fn copy_rom_to(&mut self, rom: &Path, dest: PathBuf) {
        match std::fs::copy(rom, &dest) {
            Ok(_) => {
                self.set_status(format!("ROM exported to {}", dest.display()), false);
                self.exported_to = Some(dest);
            }
            Err(e) => self.set_status(format!("Export failed: {e}"), true),
        }
    }

    /// Diagnostics that point at `file`, for the editor margin.
    pub fn messages_for(&self, file: &Path) -> Vec<crate::editor::LineMessage> {
        self.current_diagnostics()
            .iter()
            .filter(|d| d.file.as_deref().map(|f| same_file(f, file)).unwrap_or(false) && d.line > 0)
            .map(|d| crate::editor::LineMessage {
                line: d.line,
                severity: d.severity,
                text: match &d.hint {
                    Some(h) => format!("{}\n\n💡 {h}", d.message),
                    None => d.message.clone(),
                },
            })
            .collect()
    }

    // ------------------------------------------------------------------
    // Keyboard shortcuts
    // ------------------------------------------------------------------

    pub fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let sc = |m: Modifiers, k: Key| KeyboardShortcut::new(m, k);
        let cmd = Modifiers::COMMAND;
        let shift_cmd = Modifiers::COMMAND | Modifiers::SHIFT;
        if ctx.input_mut(|i| i.consume_shortcut(&sc(cmd, Key::R))) {
            self.start_build(true);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&sc(cmd, Key::B))) {
            self.start_build(false);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&sc(cmd, Key::S))) {
            self.save_all();
            if self.settings.run_on_save && self.project.is_some() {
                self.start_build(true);
            }
        }
        if ctx.input_mut(|i| i.consume_shortcut(&sc(shift_cmd, Key::N))) {
            self.new_project_open = true;
        }
        if ctx.input_mut(|i| i.consume_shortcut(&sc(cmd, Key::N))) && self.project.is_some() {
            self.new_file_open = true;
        }
        if ctx.input_mut(|i| i.consume_shortcut(&sc(cmd, Key::O))) {
            if let Some(dir) = rfd::FileDialog::new().set_title("Open a GBADE project folder").pick_folder() {
                self.request_open_project(dir);
            }
        }
        if ctx.input_mut(|i| i.consume_shortcut(&sc(cmd, Key::W))) && !self.docs.is_empty() {
            let i = self.active_doc;
            if !self.docs[i].is_dirty() {
                self.close_doc(i);
            } else {
                self.set_status(crate::platform::k("Save the file first (⌘S), or close it from its tab."), true);
            }
        }
        if ctx.input_mut(|i| i.consume_shortcut(&sc(cmd, Key::Period))) {
            self.stop_game();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&sc(shift_cmd, Key::R))) {
            if let Some(s) = &mut self.session {
                s.reset();
            }
        }
        if ctx.input_mut(|i| i.consume_shortcut(&sc(Modifiers::NONE, Key::F5))) {
            if self.session.is_some() {
                self.toggle_pause();
            } else {
                self.start_build(true);
            }
        }
        if ctx.input_mut(|i| i.consume_shortcut(&sc(Modifiers::NONE, Key::F10))) {
            if let Some(s) = &mut self.session {
                if s.is_paused() {
                    s.step_line();
                    self.show_pause_location();
                }
            }
        }
        if ctx.input_mut(|i| i.consume_shortcut(&sc(Modifiers::NONE, Key::F2))) {
            if let Some(s) = &mut self.session {
                self.save_state = s.emu.save_state();
                self.set_status("State saved (F4 to load it back).", false);
            }
        }
        if ctx.input_mut(|i| i.consume_shortcut(&sc(Modifiers::NONE, Key::F4))) {
            if let (Some(s), Some(st)) = (&mut self.session, &self.save_state) {
                s.emu.load_state(st);
                self.set_status("State loaded.", false);
            }
        }
        if ctx.input_mut(|i| i.consume_shortcut(&sc(cmd, Key::Plus)) || i.consume_shortcut(&sc(cmd, Key::Equals))) {
            self.settings.editor_font_size = (self.settings.editor_font_size + 1.0).min(32.0);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&sc(cmd, Key::Minus))) {
            self.settings.editor_font_size = (self.settings.editor_font_size - 1.0).max(9.0);
        }
    }
}

fn info_line(text: String) -> crate::session::ConsoleLine {
    crate::session::ConsoleLine { level: gbade_emu::LogLevel::Info, from_game: false, category: "gbade".into(), text, frame: 0 }
}

pub fn same_file(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    match (dunce::canonicalize(a), dunce::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

impl GbadeApp {
    /// One UI frame (everything `eframe::App::update` does). Separate so the
    /// UI tests can drive it without a real window.
    pub fn frame(&mut self, ctx: &egui::Context) {
        // Window close with unsaved changes -> ask.
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close && self.docs.iter().any(Document::is_dirty) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm = Some(Confirm::CloseWithUnsaved);
        }
        self.handle_shortcuts(ctx);
        self.poll_build();
        self.lsp_tick();
        self.git_tick(ctx);
        self.tick(ctx);
        self.update_screen_texture(ctx);

        // Files changed on disk in another editor.
        if ctx.input(|i| i.focused) && ctx.cumulative_frame_nr() % 60 == 0 {
            for d in &mut self.docs {
                d.reload_if_changed();
            }
        }

        let title = match &self.project {
            Some(p) => format!("{} — GBADE", p.config.game.name),
            None => "GBADE".into(),
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));

        self.draw(ctx);

        let running = self.session.as_ref().is_some_and(|s| !s.is_paused());
        if running || self.build_job.is_some() {
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(Duration::from_millis(500));
        }
    }
}

impl eframe::App for GbadeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame(ctx);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.settings);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.stop_game();
    }
}

#[allow(dead_code)]
pub fn is_error(d: &Diagnostic) -> bool {
    d.severity == Severity::Error
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An app with no project, its runtime cache inside `target/`.
    fn app() -> GbadeApp {
        // SAFETY: every test sets the same value.
        unsafe {
            std::env::set_var("GBADE_CACHE_DIR", Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/gbade-test-cache"));
        }
        GbadeApp::with_settings(&egui::Context::default(), Settings::default())
    }

    /// An app with a fresh project whose main.c is `main_c`.
    fn app_with_project(main_c: &str) -> (tempfile::TempDir, GbadeApp) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("gbade.toml"), "[game]\nname = \"App Test\"\n").unwrap();
        std::fs::write(dir.path().join("src/main.c"), main_c).unwrap();
        let mut a = app();
        a.open_project(dir.path());
        assert!(a.project.is_some());
        (dir, a)
    }

    fn wait_build(a: &mut GbadeApp) {
        let start = Instant::now();
        while a.build_job.is_some() {
            assert!(start.elapsed() < Duration::from_secs(120), "build never finished");
            std::thread::sleep(Duration::from_millis(10));
            a.poll_build();
        }
    }

    fn status(a: &GbadeApp) -> (&str, bool) {
        let (m, _, e) = a.status.as_ref().expect("a status message");
        (m.as_str(), *e)
    }

    const GAME: &str = "#include <gba.h>\n#include <stdio.h>\nint main(void) {\n\n    printf(\"hi\\n\");\n    while (1) vsync();\n}\n";

    #[test]
    fn new_files_go_in_the_right_folder_with_a_starter_text() {
        let (_dir, mut a) = app_with_project(GAME);
        let root = a.project.as_ref().unwrap().root.clone();

        let c = a.create_file("player.c").unwrap();
        assert_eq!(c, root.join("src/player.c"));
        assert!(std::fs::read_to_string(&c).unwrap().starts_with("#include <gba.h>"));

        let h = a.create_file("  player-state.h ").unwrap();
        assert_eq!(h, root.join("include/player-state.h"));
        let text = std::fs::read_to_string(&h).unwrap();
        assert!(text.contains("#ifndef PLAYER_STATE_H") && text.contains("#endif"), "{text}");

        let s = a.create_file("fast.s").unwrap();
        assert!(std::fs::read_to_string(s).unwrap().contains(".syntax unified"));

        let nested = a.create_file("enemies/boss.c").unwrap();
        assert_eq!(nested, root.join("src/enemies/boss.c"));

        assert!(a.create_file("player.c").unwrap_err().contains("already exists"));
        assert!(a.create_file("hero.png").unwrap_err().contains("assets/"));
        assert!(a.create_file("notes.txt").unwrap_err().contains(".c, .h or .s"));
        assert!(a.create_file("../escape.c").is_err());
        assert!(a.create_file("   ").is_err());
        // The new files are part of the next build.
        assert!(a.project.as_ref().unwrap().sources().iter().any(|(f, _)| f.ends_with("boss.c")));
    }

    #[test]
    fn no_project_no_files_no_build() {
        let mut a = app();
        assert!(a.create_file("main.c").unwrap_err().contains("no project"));
        a.start_build(true);
        assert!(a.build_job.is_none());
        assert_eq!(status(&a), ("Open or create a project first.", true));
    }

    #[test]
    fn opening_files_reuses_tabs() {
        let (_dir, mut a) = app_with_project(GAME);
        assert_eq!(a.docs.len(), 1, "main.c opens with the project");
        let other = a.create_file("other.c").unwrap();
        a.open_file(&other, Some(3));
        assert_eq!((a.docs.len(), a.active_doc), (2, 1));
        assert_eq!(a.docs[1].goto_line, Some(3));
        // The same file through another spelling of its path: same tab.
        let main = a.project.as_ref().unwrap().src_dir().join("../src/main.c");
        a.open_file(&main, None);
        assert_eq!((a.docs.len(), a.active_doc), (2, 0));
        // A missing file: an error, no new tab.
        a.open_file(Path::new("/definitely/missing.c"), None);
        assert_eq!(a.docs.len(), 2);
        assert!(status(&a).1);

        a.close_doc(1);
        assert_eq!((a.docs.len(), a.active_doc), (1, 0));
        a.close_doc(0);
        assert!(a.docs.is_empty());
        a.close_doc(5); // out of range: nothing happens
    }

    #[test]
    fn save_all_writes_only_changed_files() {
        let (_dir, mut a) = app_with_project(GAME);
        assert!(a.save_all());
        assert!(a.status.as_ref().is_none_or(|(m, _, _)| !m.starts_with("Saved")), "nothing to save");
        a.docs[0].text.push_str("// edited\n");
        assert!(a.docs[0].is_dirty());
        assert!(a.save_all());
        assert_eq!(status(&a), ("Saved 1 file(s).", false));
        assert!(!a.docs[0].is_dirty());
        assert!(a.dirty_since_build);
        assert!(std::fs::read_to_string(&a.docs[0].path).unwrap().ends_with("// edited\n"));
    }

    #[test]
    fn switching_projects_asks_before_losing_changes() {
        let (_dir, mut a) = app_with_project(GAME);
        let (other_dir, _) = app_with_project(GAME);
        a.docs[0].text.push_str("// unsaved\n");
        a.request_open_project(other_dir.path().to_path_buf());
        assert!(matches!(a.confirm, Some(Confirm::OpenProjectWithUnsaved(_))));
        assert_ne!(a.project.as_ref().unwrap().root, gbade_toolchain::paths::canonical(other_dir.path()));

        a.confirm = None;
        a.save_all();
        a.request_open_project(other_dir.path().to_path_buf());
        assert!(a.confirm.is_none());
        assert_eq!(a.project.as_ref().unwrap().root, gbade_toolchain::paths::canonical(other_dir.path()));
        assert_eq!(a.settings.recent_projects[0], gbade_toolchain::paths::canonical(other_dir.path()));
    }

    #[test]
    fn opening_a_folder_that_is_not_a_project_keeps_the_current_one() {
        let (dir, mut a) = app_with_project(GAME);
        a.toggle_breakpoint(&a.docs[0].path.clone(), 5);
        let not_a_project = tempfile::tempdir().unwrap();
        a.open_project(not_a_project.path());
        assert!(status(&a).1);
        assert_eq!(a.project.as_ref().unwrap().root, gbade_toolchain::paths::canonical(dir.path()));
        assert_eq!(a.breakpoints.len(), 1, "nothing was reset");
    }

    #[test]
    fn a_project_without_main_c_opens_its_first_source() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("gbade.toml"), "[game]\nname = \"X\"\n").unwrap();
        std::fs::write(dir.path().join("src/game.c"), "int main(void) { for (;;); }\n").unwrap();
        let mut a = app();
        a.open_project(dir.path());
        assert_eq!(a.docs.len(), 1);
        assert!(a.docs[0].path.ends_with("game.c"));
    }

    #[test]
    fn breakpoints_toggle_and_problems_show_in_their_file() {
        let (_dir, mut a) = app_with_project(GAME);
        let main = a.docs[0].path.clone();
        a.toggle_breakpoint(&main, 5);
        a.toggle_breakpoint(&main, 6);
        a.toggle_breakpoint(&main, 5);
        assert_eq!(a.breakpoints[&main].iter().copied().collect::<Vec<_>>(), vec![6]);

        let mut d = Diagnostic::error("expected ';'");
        d.file = Some(main.clone());
        d.line = 5;
        d.hint = Some("add a semicolon".into());
        a.diagnostics = vec![d, Diagnostic::error("no file")];
        let m = a.messages_for(&main);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].line, 5);
        assert!(m[0].text.contains("expected ';'") && m[0].text.contains("add a semicolon"));
        assert!(a.messages_for(Path::new("/other.c")).is_empty());
    }

    #[test]
    fn a_failed_build_opens_the_error() {
        let (_dir, mut a) = app_with_project("#include <gba.h>\nint main(void) {\n    int x = 1\n    while (1) vsync();\n}\n");
        a.docs.clear();
        a.start_build(true);
        a.start_build(true); // a second click while building is ignored
        wait_build(&mut a);
        assert!(!a.last_build.as_ref().unwrap().success);
        assert!(a.session.is_none(), "nothing to run");
        assert_eq!(a.bottom_tab, BottomTab::Problems);
        assert!(status(&a).0.starts_with("Build failed with 1 error(s)"), "{:?}", a.status);
        assert_eq!(a.docs.len(), 1, "the file with the error is opened");
        assert_eq!(a.docs[0].goto_line, Some(3), "at the error's line");
    }

    #[test]
    fn a_good_build_runs_and_reports_moved_breakpoints() {
        let (dir, mut a) = app_with_project(GAME);
        let main = a.docs[0].path.clone();
        a.toggle_breakpoint(&main, 4); // blank line: moves to 5
        a.start_build(true);
        wait_build(&mut a);
        assert!(a.last_build.as_ref().unwrap().success, "{}", a.build_log);
        assert!(!a.dirty_since_build);
        let s = a.session.as_ref().expect("the game runs");
        assert!(s.console.iter().any(|l| l.text == "Breakpoint main.c:4 moved to line 5 (no code on line 4)."), "{:?}", s.console);
        assert_eq!(a.bottom_tab, BottomTab::Console);

        // Pause, from the toolbar: the CPU inspector shows where.
        a.toggle_pause();
        assert!(a.session.as_ref().unwrap().is_paused());
        assert_eq!(a.inspector.tab, crate::inspector::Tab::Cpu);
        a.toggle_pause();
        assert!(!a.session.as_ref().unwrap().is_paused());

        // Screenshots are numbered.
        a.save_screenshot();
        a.save_screenshot();
        let shots = dir.path().join("build/screenshots");
        assert!(shots.join("app-test-001.png").exists() && shots.join("app-test-002.png").exists());

        a.stop_game();
        assert!(a.session.is_none());
        a.toggle_pause(); // no game: nothing happens
    }

    #[test]
    fn a_missing_compiler_is_explained_in_the_build_log() {
        let (_dir, mut a) = app_with_project(GAME);
        a.toolchain = Err("Could not find the bundled compiler (clang).".into());
        a.start_build(true);
        assert!(a.build_job.is_none());
        assert_eq!(a.bottom_tab, BottomTab::BuildLog);
        assert!(a.build_log.contains("bundled compiler"));
    }

    #[test]
    fn export_needs_a_build_first() {
        let (_dir, mut a) = app_with_project(GAME);
        a.export_rom(); // no ROM yet: no file dialog, an explanation
        assert!(status(&a).0.contains("Build the project first"), "{:?}", a.status);
    }

    #[test]
    fn exporting_shows_a_notice_with_the_path() {
        let (dir, mut a) = app_with_project(GAME);
        let rom = dir.path().join("fake.gba");
        std::fs::write(&rom, b"rom").unwrap();
        let dest = dir.path().join("out.gba");
        a.copy_rom_to(&rom, dest.clone());
        assert_eq!(a.exported_to.as_deref(), Some(dest.as_path()));
        assert_eq!(std::fs::read(&dest).unwrap(), b"rom");
        // A failed copy shows no notice.
        a.exported_to = None;
        a.copy_rom_to(&rom, dir.path().join("no/such/dir/out.gba"));
        assert!(a.exported_to.is_none());
        assert!(status(&a).1);
    }

    #[test]
    fn the_last_project_reopens_at_start() {
        let (dir, _a) = app_with_project(GAME);
        let settings = Settings { recent_projects: vec![dir.path().to_path_buf()], ..Settings::default() };
        let a = GbadeApp::with_settings(&egui::Context::default(), settings);
        assert!(a.project.is_some());
        // A project that was deleted since is skipped.
        let gone = Settings { recent_projects: vec![PathBuf::from("/no/such/project")], ..Settings::default() };
        assert!(GbadeApp::with_settings(&egui::Context::default(), gone).project.is_none());
    }

    #[test]
    fn same_file_sees_through_path_spellings() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("a.c");
        std::fs::write(&f, "").unwrap();
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        assert!(same_file(&f, &dir.path().join("./sub/../a.c")));
        assert!(!same_file(&f, &dir.path().join("b.c")));
        assert!(is_error(&Diagnostic::error("x")));
    }
}
