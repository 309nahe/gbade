//! Git in the IDE: the "Changes" tab of the left panel (start tracking,
//! changed files, commit, history), the diff view, and the git markers in the
//! editor margin. All repository work is done by `gbade-git`.

use crate::app::{CenterView, Confirm, GbadeApp};
use crate::editor::hash_str;
use eframe::egui::{self, Color32, RichText, Ui};
use gbade_git::{CommitInfo, FileChange, FileDiff, FileState, Repo};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LeftTab {
    #[default]
    Files,
    Changes,
}

/// What the diff view shows.
pub struct DiffView {
    pub title: String,
    pub subtitle: String,
    pub files: Vec<FileDiff>,
}

#[derive(Default)]
pub struct GitUi {
    pub tab: LeftTab,
    pub changes: Vec<FileChange>,
    pub history: Vec<CommitInfo>,
    pub message: String,
    pub name: String,
    pub email: String,
    pub diff: Option<DiffView>,
    last_refresh: Option<Instant>,
}

pub fn state_color(s: FileState) -> Color32 {
    match s {
        FileState::New => Color32::from_rgb(90, 180, 100),
        FileState::Modified | FileState::Renamed => Color32::from_rgb(220, 170, 60),
        FileState::Deleted | FileState::Conflicted => Color32::from_rgb(230, 90, 90),
    }
}

/// "3 minutes ago", "yesterday", "2026-11-02".
pub fn relative_time(t: i64, now: i64) -> String {
    let d = now - t;
    match d {
        d if d < 60 => "just now".into(),
        d if d < 3600 => format!("{} min ago", d / 60),
        d if d < 86_400 => format!("{} h ago", d / 3600),
        d if d < 2 * 86_400 => "yesterday".into(),
        d if d < 7 * 86_400 => format!("{} days ago", d / 86_400),
        _ => {
            // Civil date from days since the epoch (Howard Hinnant's algorithm).
            let z = t.div_euclid(86_400) + 719_468;
            let era = z.div_euclid(146_097);
            let doe = z - era * 146_097;
            let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
            let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
            let mp = (5 * doy + 2) / 153;
            let day = doy - (153 * mp + 2) / 5 + 1;
            let month = if mp < 10 { mp + 3 } else { mp - 9 };
            let year = yoe + era * 400 + if month <= 2 { 1 } else { 0 };
            format!("{year}-{month:02}-{day:02}")
        }
    }
}

fn now_secs() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

impl GbadeApp {
    /// Open the project's repository (if it has one) and read its state.
    pub fn open_git(&mut self) {
        self.git = self.project.as_ref().and_then(|p| Repo::open(&p.root));
        let ident = self.git.as_ref().and_then(Repo::identity).or_else(gbade_git::global_identity);
        let (name, email) = ident.unwrap_or_default();
        self.git_ui = crate::git_panel::GitUi { name, email, ..Default::default() };
        self.refresh_git();
    }

    pub fn refresh_git(&mut self) {
        let Some(repo) = &self.git else { return };
        self.git_ui.changes = repo.status().unwrap_or_default();
        self.git_ui.history = repo.history(50).unwrap_or_default();
        self.git_ui.last_refresh = Some(Instant::now());
        for d in &mut self.docs {
            d.git_lines_hash = 0; // recompute margin markers
        }
    }

    /// Called every frame: refresh now and then, update the margin markers of
    /// the visible file when its text changed.
    pub fn git_tick(&mut self, ctx: &egui::Context) {
        if self.git.is_none() {
            return;
        }
        let stale = self.git_ui.last_refresh.is_none_or(|t| t.elapsed() > Duration::from_secs(3));
        if stale && ctx.input(|i| i.focused) {
            self.refresh_git();
        }
        let Some(repo) = &self.git else { return };
        if let Some(d) = self.docs.get_mut(self.active_doc) {
            let h = hash_str(&d.text);
            if d.git_lines_hash != h {
                d.git_lines_hash = h;
                d.git_lines = repo.relative(&d.path).map(|rel| repo.line_changes(&rel, &d.text)).unwrap_or_default();
            }
        }
    }

    /// The git state of a file (absolute path), for the file list colours.
    pub fn git_state(&self, path: &Path) -> Option<FileState> {
        let repo = self.git.as_ref()?;
        let rel = repo.relative(path)?;
        self.git_ui.changes.iter().find(|c| c.path == rel).map(|c| c.state)
    }

    fn init_git(&mut self) {
        let Some(root) = self.project.as_ref().map(|p| p.root.clone()) else { return };
        let (name, email) = (self.git_ui.name.trim().to_string(), self.git_ui.email.trim().to_string());
        if name.is_empty() || email.is_empty() {
            self.set_status("Enter your name and email first: every snapshot records who made it.", true);
            return;
        }
        self.save_all();
        match Repo::init(&root, &name, &email) {
            Ok(r) => {
                self.git = Some(r);
                self.refresh_git();
                self.set_status("Your project is now tracked: a first snapshot was saved.", false);
            }
            Err(e) => self.set_status(format!("Could not start tracking: {}", e.message()), true),
        }
    }

    fn commit(&mut self) {
        self.save_all();
        let Some(repo) = &self.git else { return };
        if repo.identity().is_none() {
            let (n, e) = (self.git_ui.name.trim(), self.git_ui.email.trim());
            if n.is_empty() || e.is_empty() {
                self.set_status("Enter your name and email before the first commit.", true);
                return;
            }
            if let Err(err) = repo.set_identity(n, e) {
                self.set_status(format!("Could not save your name: {}", err.message()), true);
                return;
            }
        }
        match repo.commit_all(&self.git_ui.message) {
            Ok(id) => {
                let short = id.get(..7).unwrap_or(&id).to_string();
                self.git_ui.message.clear();
                self.refresh_git();
                self.set_status(format!("Snapshot {short} saved."), false);
            }
            Err(e) => self.set_status(e.message().to_string(), true),
        }
    }

    pub fn show_file_diff(&mut self, rel: &Path) {
        let Some(repo) = &self.git else { return };
        match repo.file_diff(rel) {
            Ok(Some(d)) => {
                self.git_ui.diff = Some(DiffView {
                    title: rel.display().to_string(),
                    subtitle: "Changes since the last commit".into(),
                    files: vec![d],
                });
                self.center = CenterView::Diff;
            }
            Ok(None) => self.set_status("No changes in this file.", false),
            Err(e) => self.set_status(e.message().to_string(), true),
        }
    }

    pub fn show_commit(&mut self, c: &CommitInfo) {
        let Some(repo) = &self.git else { return };
        match repo.commit_diff(&c.id) {
            Ok(files) => {
                self.git_ui.diff = Some(DiffView {
                    title: c.summary.clone(),
                    subtitle: format!("Commit {} by {}, {}", c.short_id, c.author, relative_time(c.time, now_secs())),
                    files,
                });
                self.center = CenterView::Diff;
            }
            Err(e) => self.set_status(e.message().to_string(), true),
        }
    }

    /// "Discard changes" confirmed: restore the file from the last commit.
    pub fn discard_file(&mut self, rel: &Path) {
        let Some(repo) = &self.git else { return };
        let abs = repo.root().join(rel);
        match repo.discard(rel) {
            Ok(()) => {
                if let Some(i) = self.docs.iter().position(|d| crate::app::same_file(&d.path, &abs) || d.path == abs) {
                    if abs.exists() {
                        self.docs[i].reload_from_disk();
                    } else {
                        self.close_doc(i);
                    }
                }
                self.refresh_git();
                self.set_status(format!("{} restored to the last commit.", rel.display()), false);
            }
            Err(e) => self.set_status(e.message().to_string(), true),
        }
    }

    /// The "Changes" tab of the left panel.
    pub fn git_changes_ui(&mut self, ui: &mut Ui) {
        let Some(repo) = &self.git else {
            ui.label("This project's history is not tracked yet.");
            ui.label(
                RichText::new(
                    "Git saves snapshots (\"commits\") of your project, so you can see what changed and get back an \
                     older version of a file if an experiment goes wrong.",
                )
                .small()
                .weak(),
            );
            ui.add_space(6.0);
            identity_fields(ui, &mut self.git_ui.name, &mut self.git_ui.email);
            ui.add_space(4.0);
            if ui.button("Start tracking changes").clicked() {
                self.init_git();
            }
            return;
        };
        let branch = repo.branch();
        let needs_identity = repo.identity().is_none();
        ui.label(RichText::new(format!("On branch {branch}")).small().weak());
        ui.add_space(4.0);

        let mut open_diff: Option<PathBuf> = None;
        let mut open_file: Option<PathBuf> = None;
        let mut discard: Option<PathBuf> = None;
        if self.git_ui.changes.is_empty() {
            ui.label(RichText::new("No changes since the last commit.").weak());
        } else {
            ui.label(RichText::new(format!("{} changed file(s)", self.git_ui.changes.len())).strong());
            for c in &self.git_ui.changes {
                let text = RichText::new(format!("{}  {}", c.state.letter(), c.path.display())).color(state_color(c.state));
                let resp = ui.selectable_label(false, text).on_hover_text(format!("{} · click to see the changes", c.state.describe()));
                if resp.clicked() {
                    open_diff = Some(c.path.clone());
                }
                resp.context_menu(|ui| {
                    if c.state != FileState::Deleted && ui.button("Open file").clicked() {
                        open_file = Some(c.path.clone());
                    }
                    if ui.button("Show changes").clicked() {
                        open_diff = Some(c.path.clone());
                    }
                    if ui.button("Discard changes…").clicked() {
                        discard = Some(c.path.clone());
                    }
                });
            }
        }

        ui.add_space(8.0);
        if needs_identity {
            identity_fields(ui, &mut self.git_ui.name, &mut self.git_ui.email);
        }
        ui.add(
            egui::TextEdit::multiline(&mut self.git_ui.message)
                .hint_text("Describe what you changed, e.g. \"Add jumping\"")
                .desired_rows(2)
                .desired_width(f32::INFINITY),
        );
        let can_commit = !self.git_ui.changes.is_empty() && !self.git_ui.message.trim().is_empty();
        let label = format!("Commit all ({})", self.git_ui.changes.len());
        if ui
            .add_enabled(can_commit, egui::Button::new(label))
            .on_hover_text("Save a snapshot of every changed file, with your message")
            .clicked()
        {
            self.commit();
        }

        ui.add_space(10.0);
        ui.label(RichText::new("History").strong());
        let now = now_secs();
        let mut show: Option<CommitInfo> = None;
        egui::ScrollArea::vertical().id_salt("git-history").auto_shrink([false, true]).show(ui, |ui| {
            for c in &self.git_ui.history {
                let resp = ui
                    .selectable_label(false, RichText::new(&c.summary))
                    .on_hover_text(format!("{}\n{} · {}", c.message.trim(), c.author, c.short_id));
                ui.label(RichText::new(format!("{} · {}", c.short_id, relative_time(c.time, now))).small().weak());
                if resp.clicked() {
                    show = Some(c.clone());
                }
            }
        });

        if let Some(rel) = open_diff {
            self.show_file_diff(&rel);
        }
        if let Some(rel) = open_file {
            let abs = self.git.as_ref().map(|r| r.root().join(&rel));
            if let Some(abs) = abs {
                self.open_file(&abs, None);
            }
        }
        if let Some(rel) = discard {
            self.confirm = Some(Confirm::DiscardFile(rel));
        }
        if let Some(c) = show {
            self.show_commit(&c);
        }
    }

    /// The diff view in the centre of the window.
    pub fn diff_view_ui(&mut self, ui: &mut Ui) {
        let Some(view) = &self.git_ui.diff else {
            self.center = CenterView::Editor;
            return;
        };
        let mut back = false;
        ui.horizontal(|ui| {
            if ui.button("← Back to the editor").clicked() {
                back = true;
            }
            ui.label(RichText::new(&view.title).strong().size(15.0));
        });
        ui.label(RichText::new(&view.subtitle).weak());
        ui.separator();
        let dark = ui.visuals().dark_mode;
        let (add_bg, del_bg, hunk_fg) = if dark {
            (Color32::from_rgb(28, 60, 36), Color32::from_rgb(70, 30, 30), Color32::from_rgb(120, 150, 220))
        } else {
            (Color32::from_rgb(220, 245, 222), Color32::from_rgb(252, 225, 225), Color32::from_rgb(60, 90, 170))
        };
        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            for f in &view.files {
                egui::CollapsingHeader::new(
                    RichText::new(format!("{}  {}  ({})", f.state.letter(), f.path.display(), f.state.describe()))
                        .color(state_color(f.state)),
                )
                .id_salt(&f.path)
                .default_open(view.files.len() <= 8)
                .show(ui, |ui| {
                    if f.binary {
                        ui.label(RichText::new("Binary file (an image or a ROM): no text to compare.").weak());
                        return;
                    }
                    for l in f.lines.iter().take(4000) {
                        let num = match (l.old_line, l.new_line) {
                            (_, Some(n)) if l.origin != '-' => format!("{n:>4} "),
                            (Some(o), _) => format!("{o:>4} "),
                            _ => "     ".into(),
                        };
                        let (bg, prefix) = match l.origin {
                            '+' => (add_bg, "+"),
                            '-' => (del_bg, "-"),
                            '@' => (Color32::TRANSPARENT, ""),
                            _ => (Color32::TRANSPARENT, " "),
                        };
                        let mut text = RichText::new(format!("{num}{prefix} {}", l.text)).monospace();
                        if l.origin == '@' {
                            text = RichText::new(&l.text).monospace().color(hunk_fg);
                        }
                        egui::Frame::new().fill(bg).show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            ui.label(text);
                        });
                    }
                });
            }
        });
        if back {
            self.center = CenterView::Editor;
        }
    }
}

fn identity_fields(ui: &mut Ui, name: &mut String, email: &mut String) {
    ui.label(RichText::new("Your name and email are saved in each snapshot (only in this project).").small().weak());
    egui::Grid::new("git-identity").num_columns(2).show(ui, |ui| {
        ui.label("Name");
        ui.add(egui::TextEdit::singleline(name).desired_width(140.0));
        ui.end_row();
        ui.label("Email");
        ui.add(egui::TextEdit::singleline(email).desired_width(140.0));
        ui.end_row();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_times() {
        let now = 1_763_200_000; // 2025-11-15
        assert_eq!(relative_time(now - 10, now), "just now");
        assert_eq!(relative_time(now - 300, now), "5 min ago");
        assert_eq!(relative_time(now - 7200, now), "2 h ago");
        assert_eq!(relative_time(now - 90_000, now), "yesterday");
        assert_eq!(relative_time(now - 3 * 86_400, now), "3 days ago");
        assert_eq!(relative_time(0, now), "1970-01-01");
        assert_eq!(relative_time(1_700_000_000, now), "2023-11-14");
    }
}
