//! Drawing the main window.

use crate::app::{BottomTab, CenterView, Confirm, GbadeApp};
use crate::editor::{self, EditorEvent, Marks};
use crate::resources;
use eframe::egui::{self, Align, Color32, Layout, RichText, Sense, Ui, Vec2};
use gbade_emu::LogLevel;
use gbade_toolchain::{MemoryUsage, Severity};
use std::path::{Path, PathBuf};
use std::time::Duration;

const ACCENT: Color32 = Color32::from_rgb(110, 90, 230);
const OK_GREEN: Color32 = Color32::from_rgb(80, 190, 110);
const ERR_RED: Color32 = Color32::from_rgb(235, 80, 80);
const WARN_YELLOW: Color32 = Color32::from_rgb(230, 180, 40);

impl GbadeApp {
    pub fn draw(&mut self, ctx: &egui::Context) {
        self.menu_bar(ctx);
        self.toolbar(ctx);
        self.status_bar(ctx);
        if self.project.is_some() {
            self.bottom_panel(ctx);
            self.explorer(ctx);
            self.game_panel(ctx);
        }
        egui::CentralPanel::default().show(ctx, |ui| match (self.center, self.project.is_some()) {
            (CenterView::Learn, _) => {
                let dir = resources::docs_dir();
                self.docs_viewer.ui(ui, dir.as_deref());
            }
            (CenterView::Editor, true) => self.editor_area(ui),
            (CenterView::Editor, false) => self.welcome(ui),
        });
        self.dialogs(ctx);
    }

    // ------------------------------------------------------------------
    fn menu_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button(crate::platform::k("New Project…   ⇧⌘N")).clicked() {
                        self.new_project_open = true;
                    }
                    if ui.button(crate::platform::k("Open Project…   ⌘O")).clicked() {
                        if let Some(dir) = rfd::FileDialog::new().set_title("Open a GBADE project folder").pick_folder() {
                            self.request_open_project(dir);
                        }
                    }
                    ui.menu_button("Open Recent", |ui| {
                        let recents = self.settings.recent_projects.clone();
                        if recents.is_empty() {
                            ui.label("(none)");
                        }
                        for r in recents {
                            if ui.button(r.display().to_string()).clicked() {
                                self.request_open_project(r);
                            }
                        }
                    });
                    ui.separator();
                    if ui.add_enabled(self.project.is_some(), egui::Button::new(crate::platform::k("New File…   ⌘N"))).clicked() {
                        self.new_file_open = true;
                    }
                    if ui.add_enabled(self.project.is_some(), egui::Button::new(crate::platform::k("Save All   ⌘S"))).clicked() {
                        self.save_all();
                    }
                    ui.separator();
                    if ui.add_enabled(self.project.is_some(), egui::Button::new("Export ROM (.gba)…")).clicked() {
                        self.export_rom();
                    }
                    if ui.add_enabled(self.project.is_some(), egui::Button::new(crate::platform::k("Show Project in Finder"))).clicked() {
                        if let Some(p) = &self.project {
                            crate::platform::open(&p.root);
                        }
                    }
                    ui.separator();
                    if ui.button("Settings…").clicked() {
                        self.settings_open = true;
                    }
                });
                ui.menu_button("Game", |ui| {
                    let has = self.project.is_some();
                    if ui.add_enabled(has, egui::Button::new(crate::platform::k("Build and Run   ⌘R"))).clicked() {
                        self.start_build(true);
                    }
                    if ui.add_enabled(has, egui::Button::new(crate::platform::k("Build Only   ⌘B"))).clicked() {
                        self.start_build(false);
                    }
                    ui.separator();
                    let running = self.session.is_some();
                    if ui.add_enabled(running, egui::Button::new("Pause / Continue   F5")).clicked() {
                        self.toggle_pause();
                    }
                    if ui.add_enabled(running, egui::Button::new("Step One Line   F10")).clicked() {
                        if let Some(s) = &mut self.session {
                            s.pause_user();
                            s.step_line();
                        }
                        self.show_pause_location();
                    }
                    if ui.add_enabled(running, egui::Button::new(crate::platform::k("Reset   ⇧⌘R"))).clicked() {
                        if let Some(s) = &mut self.session {
                            s.reset();
                        }
                    }
                    if ui.add_enabled(running, egui::Button::new(crate::platform::k("Stop   ⌘."))).clicked() {
                        self.stop_game();
                    }
                    ui.separator();
                    if ui.add_enabled(running, egui::Button::new("Save State   F2")).clicked() {
                        if let Some(s) = &mut self.session {
                            self.save_state = s.emu.save_state();
                        }
                    }
                    if ui.add_enabled(running && self.save_state.is_some(), egui::Button::new("Load State   F4")).clicked() {
                        if let (Some(s), Some(st)) = (&mut self.session, &self.save_state) {
                            s.emu.load_state(st);
                        }
                    }
                    if ui.add_enabled(running, egui::Button::new("Save Screenshot")).clicked() {
                        self.save_screenshot();
                    }
                    ui.separator();
                    if ui.add_enabled(!self.breakpoints.is_empty(), egui::Button::new("Remove All Breakpoints")).clicked() {
                        self.breakpoints.clear();
                        if let Some(s) = &mut self.session {
                            s.set_breakpoints(&self.breakpoints);
                        }
                    }
                });
                ui.menu_button("View", |ui| {
                    if ui.checkbox(&mut self.settings.dark_mode, "Dark mode").changed() {
                        self.apply_theme(ui.ctx());
                    }
                    ui.checkbox(&mut self.settings.show_inspector, "Hardware inspector");
                    ui.checkbox(&mut self.settings.show_emulator_warnings, "Emulator warnings in Console");
                    ui.separator();
                    if ui.button(crate::platform::k("Bigger editor font   ⌘+")).clicked() {
                        self.settings.editor_font_size = (self.settings.editor_font_size + 1.0).min(32.0);
                    }
                    if ui.button(crate::platform::k("Smaller editor font   ⌘-")).clicked() {
                        self.settings.editor_font_size = (self.settings.editor_font_size - 1.0).max(9.0);
                    }
                });
                ui.menu_button("Help", |ui| {
                    if ui.button("Learn GBA programming (guides)").clicked() {
                        self.center = CenterView::Learn;
                    }
                    if ui.button("Tonc tutorial (website)").clicked() {
                        ui.ctx().open_url(egui::OpenUrl::new_tab("https://gbadev.net/tonc/"));
                    }
                    if ui.button("GBATEK hardware reference (website)").clicked() {
                        ui.ctx().open_url(egui::OpenUrl::new_tab("https://problemkaputt.de/gbatek.htm"));
                    }
                    ui.separator();
                    if ui.button("About GBADE & credits").clicked() {
                        self.about_open = true;
                    }
                });
            });
        });
    }

    // ------------------------------------------------------------------
    fn toolbar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.add_space(3.0);
            ui.horizontal(|ui| {
                let has_project = self.project.is_some();
                let building = self.build_job.is_some();
                let run_btn = egui::Button::new(RichText::new("▶  Run").strong().color(Color32::WHITE)).fill(ACCENT);
                if ui
                    .add_enabled(has_project && !building, run_btn)
                    .on_hover_text(crate::platform::k("Save, build and run the game (⌘R)"))
                    .clicked()
                {
                    self.start_build(true);
                }
                if ui.add_enabled(has_project && !building, egui::Button::new("🔨 Build")).on_hover_text(crate::platform::k("Build without running (⌘B)")).clicked() {
                    self.start_build(false);
                }
                ui.separator();
                let running = self.session.is_some();
                let paused = self.session.as_ref().is_some_and(|s| s.is_paused());
                let pause_label = if paused { "▶ Continue" } else { "⏸ Pause" };
                if ui.add_enabled(running, egui::Button::new(pause_label)).on_hover_text("F5").clicked() {
                    self.toggle_pause();
                }
                if ui
                    .add_enabled(running && paused, egui::Button::new("⤵ Step line"))
                    .on_hover_text("Run until the next source line (F10)")
                    .clicked()
                {
                    if let Some(s) = &mut self.session {
                        s.step_line();
                    }
                    self.show_pause_location();
                }
                if ui
                    .add_enabled(running && paused, egui::Button::new("⏭ Frame"))
                    .on_hover_text("Run exactly one frame")
                    .clicked()
                {
                    let keys = self.game_keys(ctx);
                    if let Some(s) = &mut self.session {
                        s.step_frame(keys);
                    }
                }
                if ui.add_enabled(running, egui::Button::new("⟲ Reset")).on_hover_text(crate::platform::k("Restart the game (⇧⌘R)")).clicked() {
                    if let Some(s) = &mut self.session {
                        s.reset();
                    }
                }
                if ui.add_enabled(running, egui::Button::new("⏹ Stop")).on_hover_text(crate::platform::k("⌘.")).clicked() {
                    self.stop_game();
                }
                if let Some(s) = &mut self.session {
                    ui.separator();
                    egui::ComboBox::from_id_salt("speed")
                        .width(70.0)
                        .selected_text(format!("{}×", s.speed))
                        .show_ui(ui, |ui| {
                            for sp in [0.25, 0.5, 1.0, 2.0, 4.0] {
                                ui.selectable_value(&mut s.speed, sp, format!("{sp}×"));
                            }
                        })
                        .response
                        .on_hover_text("Emulation speed (sound is muted when faster than 1×)");
                }
                ui.separator();
                let learn = self.center == CenterView::Learn;
                if ui.selectable_label(learn, "📖 Learn").on_hover_text("Guides and reference").clicked() {
                    self.center = if learn { CenterView::Editor } else { CenterView::Learn };
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if let Some(job) = &self.build_job {
                        ui.spinner();
                        ui.label(format!("Building… {:.1}s", job.started.elapsed().as_secs_f32()));
                    } else if let Some(b) = &self.last_build {
                        if let Some(m) = b.memory {
                            memory_bars(ui, m);
                        }
                    }
                });
            });
            ui.add_space(3.0);
        });
    }

    // ------------------------------------------------------------------
    fn status_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some((msg, at, err)) = &self.status {
                    if at.elapsed() < Duration::from_secs(12) {
                        ui.label(RichText::new(msg).color(if *err { ERR_RED } else { ui.visuals().text_color() }));
                    }
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    match &self.toolchain {
                        Ok(_) => ui.label(RichText::new("clang ✓").small().weak()).on_hover_text("Bundled LLVM toolchain found"),
                        Err(e) => ui.label(RichText::new("compiler missing").small().color(ERR_RED)).on_hover_text(e.as_str()),
                    };
                    if let Some(s) = &self.session {
                        ui.label(
                            RichText::new(format!(
                                "{:.0} fps · frame {} · {:.2} ms/frame",
                                self.fps.2,
                                s.emu.frame_counter(),
                                s.last_frame_cost.as_secs_f64() * 1000.0
                            ))
                            .small()
                            .weak(),
                        );
                    }
                    if let Some(e) = &self.audio.error {
                        ui.label(RichText::new("no sound").small().weak()).on_hover_text(e.as_str());
                    }
                });
            });
        });
    }

    // ------------------------------------------------------------------
    fn explorer(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("explorer").resizable(true).default_width(190.0).show(ctx, |ui| {
            let Some(project) = self.project.clone() else { return };
            ui.add_space(4.0);
            ui.label(RichText::new(&project.config.game.name).strong().size(15.0));
            ui.horizontal(|ui| {
                if ui.small_button("+ New file").clicked() {
                    self.new_file_open = true;
                }
                if ui.small_button(crate::platform::k("Finder")).on_hover_text(crate::platform::k("Show the project folder in Finder")).clicked() {
                    crate::platform::open(&project.root);
                }
            });
            ui.separator();
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                for (title, dir) in [("src", project.src_dir()), ("include", project.include_dir()), ("assets", project.assets_dir())] {
                    if !dir.exists() && title != "src" {
                        continue;
                    }
                    egui::CollapsingHeader::new(RichText::new(format!("📁 {title}")).strong())
                        .default_open(true)
                        .show(ui, |ui| self.file_tree(ui, &dir));
                }
                let toml = project.root.join(gbade_toolchain::project::PROJECT_FILE);
                if ui.selectable_label(self.is_active(&toml), "⚙ gbade.toml").on_hover_text("Project settings: name, ROM title, compiler options").clicked() {
                    self.open_file(&toml, None);
                }
                if !project.assets_dir().exists() {
                    ui.add_space(8.0);
                    ui.label(RichText::new("Tip: create an assets/ folder and drop PNG images in it; they become C arrays automatically.").small().weak());
                }
            });
        });
    }

    fn is_active(&self, p: &Path) -> bool {
        self.docs.get(self.active_doc).is_some_and(|d| crate::app::same_file(&d.path, p))
    }

    fn file_tree(&mut self, ui: &mut Ui, dir: &Path) {
        let mut entries: Vec<PathBuf> =
            std::fs::read_dir(dir).map(|r| r.flatten().map(|e| e.path()).collect()).unwrap_or_default();
        entries.sort_by_key(|p| (!p.is_dir(), p.file_name().map(|n| n.to_ascii_lowercase())));
        if entries.is_empty() {
            ui.label(RichText::new("(empty)").weak().small());
        }
        for p in entries {
            let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            if p.is_dir() {
                egui::CollapsingHeader::new(format!("📁 {name}")).id_salt(&p).default_open(true).show(ui, |ui| self.file_tree(ui, &p));
                continue;
            }
            let dirty = self.docs.iter().any(|d| d.path == p && d.is_dirty());
            let errors = self.diagnostics.iter().filter(|d| d.severity == Severity::Error && d.file.as_deref() == Some(&p)).count();
            let icon = match p.extension().and_then(|e| e.to_str()) {
                Some("c") => "📄",
                Some("h") => "📑",
                Some("png") => "🖼",
                Some("s") | Some("S") => "⚙",
                _ => "📄",
            };
            let mut label = RichText::new(format!("{icon} {name}{}", if dirty { " ●" } else { "" }));
            if errors > 0 {
                label = label.color(ERR_RED);
            }
            let is_png = p.extension().is_some_and(|e| e.eq_ignore_ascii_case("png"));
            let resp = ui.selectable_label(self.is_active(&p), label);
            if resp.clicked() {
                if is_png {
                    crate::platform::open(&p);
                } else {
                    self.open_file(&p, None);
                }
            }
            resp.context_menu(|ui| {
                if ui.button(crate::platform::k("Show in Finder")).clicked() {
                    crate::platform::reveal(&p);
                }
                if ui.button("Delete…").clicked() {
                    self.confirm = Some(Confirm::DeleteFile(p.clone()));
                }
            });
        }
    }

    // ------------------------------------------------------------------
    fn editor_area(&mut self, ui: &mut Ui) {
        if self.docs.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label(RichText::new("Open a file from the left panel.").weak());
            });
            return;
        }
        // Tabs.
        let mut close: Option<usize> = None;
        ui.horizontal_wrapped(|ui| {
            for (i, d) in self.docs.iter().enumerate() {
                let name = format!("{}{}", d.file_name(), if d.is_dirty() { " ●" } else { "" });
                let selected = i == self.active_doc;
                let resp = ui.selectable_label(selected, name).on_hover_text(d.path.display().to_string());
                if resp.clicked() {
                    self.active_doc = i;
                }
                if resp.middle_clicked() || (selected && ui.small_button("×").on_hover_text("Close").clicked()) {
                    close = Some(i);
                }
            }
        });
        if let Some(i) = close {
            if self.docs[i].is_dirty() {
                if let Err(e) = self.docs[i].save() {
                    self.set_status(format!("Cannot save: {e}"), true);
                    return;
                }
            }
            self.close_doc(i);
            return;
        }
        ui.separator();

        // Banner when paused.
        if let Some(info) = self.session.as_ref().and_then(|s| s.pause.clone()) {
            let (bg, fg) = match info.reason {
                crate::session::PauseReason::Panic(_) | crate::session::PauseReason::Crash(_) => {
                    (Color32::from_rgb(120, 30, 30), Color32::WHITE)
                }
                _ => (Color32::from_rgb(110, 90, 10), Color32::WHITE),
            };
            egui::Frame::new().fill(bg).inner_margin(6.0).corner_radius(4.0).show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(info.title()).color(fg).strong());
                    if let Some((f, l)) = &info.location {
                        ui.label(RichText::new(format!("— {}:{l}", f.file_name().unwrap_or_default().to_string_lossy())).color(fg));
                    }
                    if let Some(func) = &info.function {
                        ui.label(RichText::new(format!("in {func}()")).color(fg));
                    }
                    ui.label(RichText::new("   F5 continue · F10 step").color(fg).small());
                });
            });
        }

        let dark = self.settings.dark_mode;
        let font_size = self.settings.editor_font_size;
        let i = self.active_doc.min(self.docs.len() - 1);
        let path = self.docs[i].path.clone();
        let messages = self.messages_for(&path);
        let current_line = self
            .session
            .as_ref()
            .and_then(|s| s.pause.as_ref())
            .and_then(|p| p.location.as_ref())
            .filter(|(f, _)| crate::app::same_file(f, &path))
            .map(|(_, l)| *l);
        let bps = self.breakpoints.get(&path).cloned().unwrap_or_default();
        let is_c = matches!(path.extension().and_then(|e| e.to_str()), Some("c"));
        let marks = Marks { breakpoints: Some(&bps), messages, current_line };
        let events = editor::editor_ui(ui, &mut self.docs[i], &marks, dark, font_size);
        for e in events {
            match e {
                EditorEvent::ToggleBreakpoint(line) => {
                    if is_c {
                        self.toggle_breakpoint(&path, line);
                    } else {
                        self.set_status("Breakpoints can only be set in .c files.", true);
                    }
                }
            }
        }
    }

    // ------------------------------------------------------------------
    fn game_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("game").resizable(true).default_width(520.0).min_width(260.0).show(ctx, |ui| {
            ui.add_space(4.0);
            let avail = ui.available_width();
            let scale = if self.settings.screen_scale > 0 {
                self.settings.screen_scale as f32
            } else {
                (avail / 240.0).floor().clamp(1.0, 4.0)
            };
            let size = Vec2::new(240.0 * scale, 160.0 * scale);
            let focused = !ctx.wants_keyboard_input();
            ui.vertical_centered(|ui| {
                let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
                let painter = ui.painter_at(rect.expand(3.0));
                painter.rect_filled(rect.expand(3.0), 4.0, Color32::from_rgb(20, 20, 24));
                match (&self.session, &self.screen_tex) {
                    (Some(_), Some(tex)) => {
                        painter.image(tex.id(), rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
                        if !focused {
                            painter.rect_filled(rect, 0.0, Color32::from_black_alpha(90));
                            painter.text(rect.center(), egui::Align2::CENTER_CENTER, "Click here to play", egui::FontId::proportional(16.0), Color32::WHITE);
                        }
                    }
                    _ => {
                        painter.text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            crate::platform::k("Press ▶ Run (⌘R)"),
                            egui::FontId::proportional(18.0),
                            Color32::from_gray(160),
                        );
                    }
                }
                if focused && self.session.is_some() {
                    painter.rect_stroke(rect.expand(2.0), 4.0, egui::Stroke::new(2.0, ACCENT), egui::StrokeKind::Outside);
                }
                if resp.clicked() {
                    // Give the keyboard to the game.
                    if let Some(id) = ctx.memory(|m| m.focused()) {
                        ctx.memory_mut(|m| m.surrender_focus(id));
                    }
                    if self.session.is_none() && self.project.is_some() {
                        self.start_build(true);
                    }
                }
                let km = &self.settings.keymap;
                ui.label(
                    RichText::new(format!(
                        "D-pad: arrows · A: {} · B: {} · L: {} · R: {} · Start: {} · Select: {}",
                        km.a, km.b, km.l, km.r, km.start, km.select
                    ))
                    .small()
                    .weak(),
                );
            });
            if self.settings.show_inspector {
                if let Some(s) = &self.session {
                    ui.separator();
                    self.inspector.ui(ui, s);
                }
            }
        });
    }

    // ------------------------------------------------------------------
    fn bottom_panel(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("bottom").resizable(true).default_height(190.0).min_height(80.0).show(ctx, |ui| {
            ui.horizontal(|ui| {
                let errs = self.diagnostics.iter().filter(|d| d.severity == Severity::Error).count();
                let warns = self.diagnostics.iter().filter(|d| d.severity == Severity::Warning).count();
                let ptitle = if errs + warns > 0 { format!("Problems ({})", errs + warns) } else { "Problems".into() };
                ui.selectable_value(&mut self.bottom_tab, BottomTab::Problems, ptitle);
                ui.selectable_value(&mut self.bottom_tab, BottomTab::Console, "Console");
                ui.selectable_value(&mut self.bottom_tab, BottomTab::BuildLog, "Build log");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if self.bottom_tab == BottomTab::Console {
                        if ui.small_button("Clear").clicked() {
                            if let Some(s) = &mut self.session {
                                s.clear_console();
                            }
                        }
                        ui.checkbox(&mut self.console_game_only, "Game output only");
                    }
                });
            });
            ui.separator();
            match self.bottom_tab {
                BottomTab::Problems => self.problems_ui(ui),
                BottomTab::Console => self.console_ui(ui),
                BottomTab::BuildLog => {
                    egui::ScrollArea::both().auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
                        if self.build_log.is_empty() {
                            ui.label(RichText::new(crate::platform::k("No build yet. Press ▶ Run or ⌘B.")).weak());
                        }
                        ui.label(RichText::new(&self.build_log).monospace());
                    });
                }
            }
        });
    }

    fn problems_ui(&mut self, ui: &mut Ui) {
        if self.diagnostics.is_empty() {
            let msg = if self.last_build.as_ref().is_some_and(|b| b.success) { "No problems. ✓" } else { "Build the project to see errors and warnings here." };
            ui.label(RichText::new(msg).weak());
            return;
        }
        let mut goto: Option<(PathBuf, u32)> = None;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for d in &self.diagnostics {
                if d.severity == Severity::Note {
                    continue;
                }
                let (icon, color) = match d.severity {
                    Severity::Error => ("⛔", ERR_RED),
                    Severity::Warning => ("⚠", WARN_YELLOW),
                    Severity::Note => ("ℹ", Color32::GRAY),
                };
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(icon).color(color));
                    let loc = match &d.file {
                        Some(f) if d.line > 0 => format!("{}:{}", f.file_name().unwrap_or_default().to_string_lossy(), d.line),
                        Some(f) => f.file_name().unwrap_or_default().to_string_lossy().to_string(),
                        None => String::new(),
                    };
                    if !loc.is_empty() && ui.link(RichText::new(&loc).monospace()).clicked() {
                        if let Some(f) = &d.file {
                            goto = Some((f.clone(), d.line.max(1)));
                        }
                    }
                    ui.label(&d.message);
                });
                if let Some(h) = &d.hint {
                    ui.horizontal_wrapped(|ui| {
                        ui.add_space(22.0);
                        ui.label(RichText::new(format!("💡 {h}")).color(Color32::from_rgb(140, 170, 230)));
                    });
                }
                ui.add_space(2.0);
            }
        });
        if let Some((f, l)) = goto {
            if f.exists() {
                self.open_file(&f, Some(l));
            }
        }
    }

    fn console_ui(&mut self, ui: &mut Ui) {
        let Some(s) = &self.session else {
            ui.label(RichText::new("Run the game to see its output. Use printf() in your code to print here.").weak());
            return;
        };
        let show_warn = self.settings.show_emulator_warnings;
        let game_only = self.console_game_only;
        let lines: Vec<_> = s
            .console
            .iter()
            .filter(|l| {
                if game_only {
                    return l.from_game;
                }
                l.from_game || l.category == "gbade" || (show_warn && l.level <= LogLevel::Warn) || (show_warn && l.level == LogLevel::GameError)
            })
            .collect();
        let row_h = ui.text_style_height(&egui::TextStyle::Monospace);
        let mut goto: Option<(PathBuf, u32)> = None;
        egui::ScrollArea::vertical().auto_shrink([false, false]).stick_to_bottom(true).show_rows(ui, row_h, lines.len(), |ui, range| {
            for l in &lines[range] {
                let color = match l.level {
                    LogLevel::Fatal | LogLevel::Error => ERR_RED,
                    LogLevel::Warn | LogLevel::GameError => WARN_YELLOW,
                    LogLevel::Stub => Color32::GRAY,
                    _ => ui.visuals().text_color(),
                };
                let prefix = if l.from_game { String::new() } else { format!("[{}] ", l.category) };
                let text = format!("{prefix}{}", l.text);
                let resp = ui.add(egui::Label::new(RichText::new(&text).monospace().color(color)).sense(Sense::click()));
                let resp = resp.on_hover_text(format!("frame {}", l.frame));
                if resp.clicked() {
                    // "PANIC: /path/file.c:12: msg" and similar are clickable.
                    let body = text.split_once(": ").map(|(_, b)| b).unwrap_or(&text);
                    let body = body.strip_prefix("BREAKPOINT() at ").unwrap_or(body);
                    if let (Some((f, line)), _) = crate::session::parse_marker(body) {
                        goto = Some((f, line));
                    }
                }
            }
        });
        if let Some((f, l)) = goto {
            if f.exists() {
                self.open_file(&f, Some(l));
            }
        }
    }

    // ------------------------------------------------------------------
    fn welcome(&mut self, ui: &mut Ui) {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(30.0);
                ui.label(RichText::new("GBADE").size(44.0).strong().color(ACCENT));
                ui.label(RichText::new("Make your own Game Boy Advance games. No setup required.").size(17.0));
                ui.add_space(20.0);
                ui.horizontal(|ui| {
                    let w = 560.0f32.min(ui.available_width());
                    ui.add_space(((ui.available_width() - w) / 2.0).max(0.0));
                    if ui.add_sized([180.0, 44.0], egui::Button::new(RichText::new("✨ New Project").size(16.0)).fill(ACCENT)).clicked() {
                        self.new_project_open = true;
                    }
                    if ui.add_sized([180.0, 44.0], egui::Button::new(RichText::new("📂 Open Project").size(16.0))).clicked() {
                        if let Some(dir) = rfd::FileDialog::new().set_title("Open a GBADE project folder").pick_folder() {
                            self.request_open_project(dir);
                        }
                    }
                    if ui.add_sized([180.0, 44.0], egui::Button::new(RichText::new("📖 Learn").size(16.0))).clicked() {
                        self.center = CenterView::Learn;
                    }
                });
                ui.add_space(24.0);
            });
            let w = 720.0f32.min(ui.available_width());
            ui.horizontal(|ui| {
                ui.add_space(((ui.available_width() - w) / 2.0).max(0.0));
                ui.vertical(|ui| {
                    ui.set_max_width(w);
                    ui.label(RichText::new("Start from an example").size(18.0).strong());
                    ui.label(RichText::new("Each example is copied to your projects folder, so you can change it freely.").weak());
                    ui.add_space(6.0);
                    for (i, ex) in resources::examples().iter().enumerate() {
                        egui::Frame::group(ui.style()).inner_margin(10.0).show(ui, |ui| {
                            ui.set_min_width(w - 24.0);
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.label(RichText::new(&ex.name).strong().size(15.0));
                                    ui.label(&ex.description);
                                });
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if ui.button("Create from this").clicked() {
                                        self.new_project_template = i;
                                        self.new_project_name = ex.name.clone();
                                        self.new_project_open = true;
                                    }
                                });
                            });
                        });
                    }
                    if !self.settings.recent_projects.is_empty() {
                        ui.add_space(16.0);
                        ui.label(RichText::new("Recent projects").size(18.0).strong());
                        for r in self.settings.recent_projects.clone() {
                            if r.exists() && ui.link(r.display().to_string()).clicked() {
                                self.request_open_project(r);
                            }
                        }
                    }
                    if let Err(e) = &self.toolchain {
                        ui.add_space(16.0);
                        ui.colored_label(ERR_RED, format!("Toolchain problem: {e}"));
                    }
                });
            });
        });
    }
}

fn memory_bars(ui: &mut Ui, m: MemoryUsage) {
    let bar = |ui: &mut Ui, name: &str, used: u32, total: u32, help: &str| {
        let frac = used as f32 / total as f32;
        let color = if frac > 0.9 { ERR_RED } else if frac > 0.7 { WARN_YELLOW } else { OK_GREEN };
        // A fixed-size box so the right-to-left toolbar layout cannot squeeze
        // the text (it would wrap letter by letter).
        let resp = ui
            .allocate_ui_with_layout(Vec2::new(92.0, 26.0), Layout::top_down(Align::Min), |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.add(
                    egui::Label::new(RichText::new(format!("{name} {}", gbade_toolchain::builder::human_size(used))).small())
                        .wrap_mode(egui::TextWrapMode::Extend),
                );
                ui.add(egui::ProgressBar::new(frac.min(1.0).max(0.02)).desired_width(88.0).desired_height(4.0).fill(color));
            })
            .response;
        resp.on_hover_text(format!(
            "{name}: {} of {} ({:.1}%)\n{help}",
            gbade_toolchain::builder::human_size(used),
            gbade_toolchain::builder::human_size(total),
            frac * 100.0
        ));
    };
    bar(ui, "EWRAM", m.ewram, MemoryUsage::EWRAM_TOTAL, "256 KB of slower RAM: EWRAM_DATA / EWRAM_BSS variables.");
    bar(ui, "IWRAM", m.iwram, MemoryUsage::IWRAM_TOTAL, "32 KB of fast RAM: global variables and IWRAM_CODE. The stack also lives here.");
    bar(ui, "ROM", m.rom, MemoryUsage::ROM_TOTAL, "Cartridge ROM: code and const data (max 32 MB).");
}
