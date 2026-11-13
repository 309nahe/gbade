//! Dialog windows: new project, new file, settings, about, confirmations.

use crate::app::{Confirm, GbadeApp};
use crate::resources;
use eframe::egui::{self, RichText};
use gbade_toolchain::Project;
use gbade_toolchain::project::slugify;

impl GbadeApp {
    pub fn dialogs(&mut self, ctx: &egui::Context) {
        self.new_project_dialog(ctx);
        self.new_file_dialog(ctx);
        self.settings_dialog(ctx);
        self.about_dialog(ctx);
        self.confirm_dialog(ctx);
    }

    fn new_project_dialog(&mut self, ctx: &egui::Context) {
        if !self.new_project_open {
            return;
        }
        let examples = resources::examples();
        let mut open = true;
        let mut create = false;
        egui::Window::new("New Project")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.set_width(460.0);
                ui.label("Name of your game:");
                let r = ui.text_edit_singleline(&mut self.new_project_name);
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    create = true;
                }
                ui.add_space(6.0);
                ui.label("Start from:");
                if examples.is_empty() {
                    ui.colored_label(egui::Color32::RED, "No templates found (examples folder missing).");
                }
                for (i, ex) in examples.iter().enumerate() {
                    ui.radio_value(&mut self.new_project_template, i, RichText::new(&ex.name).strong())
                        .on_hover_text(&ex.description);
                }
                ui.add_space(6.0);
                let base = self.new_project_dir.clone().unwrap_or_else(|| self.settings.projects_dir());
                let dest = base.join(slugify(&self.new_project_name));
                ui.horizontal(|ui| {
                    ui.label("Location:");
                    ui.label(RichText::new(dest.display().to_string()).monospace().small());
                });
                if ui.small_button("Change folder…").clicked() {
                    if let Some(d) = rfd::FileDialog::new().set_directory(&base).pick_folder() {
                        self.new_project_dir = Some(d);
                    }
                }
                if dest.exists() {
                    ui.colored_label(egui::Color32::from_rgb(230, 180, 40), "This folder already exists: pick another name.");
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let ok = !self.new_project_name.trim().is_empty() && !dest.exists() && !examples.is_empty();
                    if ui.add_enabled(ok, egui::Button::new(RichText::new("Create").strong())).clicked() {
                        create = true;
                    }
                });
                if create {
                    let tpl = &examples[self.new_project_template.min(examples.len().saturating_sub(1))];
                    match Project::create_from_template(&tpl.dir, &dest, self.new_project_name.trim()) {
                        Ok(p) => {
                            let root = p.root.clone();
                            self.open_project(&root);
                            self.new_project_open = false;
                        }
                        Err(e) => self.set_status(e.0, true),
                    }
                }
            });
        if !open {
            self.new_project_open = false;
        }
    }

    fn new_file_dialog(&mut self, ctx: &egui::Context) {
        if !self.new_file_open {
            return;
        }
        let mut open = true;
        let mut create = false;
        egui::Window::new("New File")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label("File name (player.c, player.h, fast.s):");
                let r = ui.text_edit_singleline(&mut self.new_file_name);
                r.request_focus();
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    create = true;
                }
                ui.label(
                    RichText::new(".c and .s files go in src/, headers (.h) in include/. Every .c file in src/ is compiled automatically.")
                        .small()
                        .weak(),
                );
                if ui.button("Create").clicked() {
                    create = true;
                }
            });
        if create {
            let name = self.new_file_name.clone();
            match self.create_file(&name) {
                Ok(p) => {
                    self.open_file(&p, None);
                    self.new_file_open = false;
                    self.new_file_name.clear();
                }
                Err(e) => self.set_status(e, true),
            }
        }
        if !open {
            self.new_file_open = false;
        }
    }

    fn settings_dialog(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }
        let mut open = true;
        // Capture a key press for re-binding.
        if let Some(idx) = self.binding_key {
            let pressed = ctx.input(|i| {
                i.events.iter().find_map(|e| match e {
                    egui::Event::Key { key, pressed: true, .. } => Some(*key),
                    _ => None,
                })
            });
            if let Some(k) = pressed {
                if k != egui::Key::Escape {
                    let entries = self.settings.keymap.entries_mut();
                    *entries[idx].1 = k.name().to_string();
                }
                self.binding_key = None;
            }
        }
        egui::Window::new("Settings").open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
            ui.heading("Editor");
            if ui.checkbox(&mut self.settings.dark_mode, "Dark mode").changed() {
                self.apply_theme(ctx);
            }
            ui.add(egui::Slider::new(&mut self.settings.editor_font_size, 9.0..=32.0).text("Font size"));
            ui.checkbox(&mut self.settings.run_on_save, crate::platform::k("Rebuild and restart the game when I save (⌘S)"));
            ui.separator();
            ui.heading("Game");
            ui.horizontal(|ui| {
                ui.label("Screen scale:");
                for (v, n) in [(0, "Fit"), (1, "1×"), (2, "2×"), (3, "3×"), (4, "4×")] {
                    ui.selectable_value(&mut self.settings.screen_scale, v, n);
                }
            });
            let mut changed = ui.add(egui::Slider::new(&mut self.settings.volume, 0.0..=1.0).text("Volume")).changed();
            changed |= ui.checkbox(&mut self.settings.muted, "Mute").changed();
            if changed {
                self.audio.set_volume(if self.settings.muted { 0.0 } else { self.settings.volume });
            }
            ui.checkbox(&mut self.settings.show_emulator_warnings, "Show emulator warnings in the Console (bad memory access…)");
            ui.separator();
            ui.heading("Controls");
            ui.label(RichText::new("Click a key, then press the keyboard key to use.").small().weak());
            egui::Grid::new("keymap").num_columns(2).show(ui, |ui| {
                let names = ["A", "B", "Select", "Start", "Right", "Left", "Up", "Down", "R", "L"];
                let current: Vec<String> = self.settings.keymap.entries().iter().map(|(_, k)| k.to_string()).collect();
                for (i, name) in names.iter().enumerate() {
                    ui.label(*name);
                    let label = if self.binding_key == Some(i) { "press a key…".to_string() } else { current[i].clone() };
                    if ui.button(label).clicked() {
                        self.binding_key = Some(i);
                    }
                    ui.end_row();
                }
            });
            if ui.button("Restore default controls").clicked() {
                self.settings.keymap = Default::default();
            }
            ui.separator();
            ui.heading("Projects");
            ui.horizontal(|ui| {
                ui.label("New projects go in:");
                ui.label(RichText::new(self.settings.projects_dir().display().to_string()).monospace().small());
            });
            if ui.button("Change…").clicked() {
                if let Some(d) = rfd::FileDialog::new().pick_folder() {
                    self.settings.projects_dir = Some(d);
                }
            }
        });
        if !open {
            self.settings_open = false;
            self.binding_key = None;
        }
    }

    fn about_dialog(&mut self, ctx: &egui::Context) {
        if !self.about_open {
            return;
        }
        let mut open = true;
        egui::Window::new("About GBADE").open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
            ui.set_width(440.0);
            ui.heading(format!("GBADE {}", env!("CARGO_PKG_VERSION")));
            ui.label("A Game Boy Advance development environment: editor, compiler and emulator in one app.");
            ui.add_space(6.0);
            ui.label(RichText::new("Built on the shoulders of:").strong());
            for line in [
                "mGBA by Vicki Pfau and contributors (MPL-2.0): the emulator core",
                "LLVM clang & lld (Apache-2.0 with LLVM exception): the compiler and linker",
                "LLVM compiler-rt builtins (Apache-2.0 with LLVM exception): soft-float & division",
                "printf by Marco Paland (MIT): printf/snprintf",
                "font8x8 by Daniel Hepper (public domain): the text font",
                "agb-gbafix from the agb project (MPL-2.0): cartridge header logo bytes",
                "egui/eframe by Emil Ernerfeldt (MIT/Apache-2.0): the user interface",
                "cpal, rfd, image, object, gimli, serde, toml (MIT/Apache-2.0)",
                "Tonc by Jasper Vijn and GBATEK by Martin Korth: the knowledge",
            ] {
                ui.label(format!("• {line}"));
            }
            ui.add_space(6.0);
            if ui.link("Full credits and licenses").clicked() {
                if let Some(d) = resources::docs_dir() {
                    self.docs_viewer.open(&d.join("CREDITS.md"));
                    self.center = crate::app::CenterView::Learn;
                }
            }
            if let Ok(tc) = &self.toolchain {
                if let Some(v) = tc.clang_version() {
                    ui.label(RichText::new(v).small().weak());
                }
            }
        });
        if !open {
            self.about_open = false;
        }
    }

    fn confirm_dialog(&mut self, ctx: &egui::Context) {
        let Some(c) = &self.confirm else { return };
        let (title, text) = match c {
            Confirm::DeleteFile(p) => ("Delete file?", format!("Delete {}? This cannot be undone.", p.display())),
            Confirm::CloseWithUnsaved => ("Unsaved changes", "Some files have unsaved changes.".to_string()),
            Confirm::OpenProjectWithUnsaved(_) => ("Unsaved changes", "Some files have unsaved changes.".to_string()),
        };
        let mut answer: Option<u8> = None; // 0 cancel, 1 primary, 2 secondary
        egui::Window::new(title).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            ui.label(text);
            ui.add_space(8.0);
            ui.horizontal(|ui| match c {
                Confirm::DeleteFile(_) => {
                    if ui.button("Delete").clicked() {
                        answer = Some(1);
                    }
                    if ui.button("Cancel").clicked() {
                        answer = Some(0);
                    }
                }
                _ => {
                    if ui.button("Save all").clicked() {
                        answer = Some(1);
                    }
                    if ui.button("Discard changes").clicked() {
                        answer = Some(2);
                    }
                    if ui.button("Cancel").clicked() {
                        answer = Some(0);
                    }
                }
            });
        });
        let Some(a) = answer else { return };
        let c = self.confirm.take().unwrap();
        if a == 0 {
            return;
        }
        match c {
            Confirm::DeleteFile(p) => {
                if let Some(i) = self.docs.iter().position(|d| d.path == p) {
                    self.close_doc(i);
                }
                match std::fs::remove_file(&p) {
                    Ok(()) => self.set_status(format!("Deleted {}", p.display()), false),
                    Err(e) => self.set_status(format!("Cannot delete: {e}"), true),
                }
            }
            Confirm::CloseWithUnsaved => {
                if a == 1 && !self.save_all() {
                    return;
                }
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Confirm::OpenProjectWithUnsaved(dir) => {
                if a == 1 && !self.save_all() {
                    return;
                }
                self.open_project(&dir);
            }
        }
    }
}
