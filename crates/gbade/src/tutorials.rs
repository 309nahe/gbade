//! The "Tutorials" panel: short, hands-on lessons, each ending with an
//! example the reader runs with one click.
//!
//! A tutorial is a folder in `tutorials/` (shipped in the resources like the
//! examples):
//!
//! ```text
//! tutorials/03-buttons/
//! ├── README.md     the lesson (Markdown, rendered by `docs::render`)
//! ├── gbade.toml    a complete project...
//! └── src/main.c    ...that "Try this example" copies and runs
//! ```
//!
//! Folders are listed in name order, so the number prefix sets the order.

use crate::docs::{self, Block, DocAction};
use eframe::egui::{self, RichText, Ui};
use std::path::{Path, PathBuf};

/// One tutorial on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tutorial {
    pub dir: PathBuf,
    /// "3. Buttons": the README's first `# ` heading.
    pub title: String,
    /// The title without its number: "Buttons" (used to name projects).
    pub name: String,
    /// The first paragraph of the README.
    pub summary: String,
}

/// Every tutorial in `dir`, in order. A folder counts when it has both a
/// README.md and a gbade.toml.
pub fn list(dir: &Path) -> Vec<Tutorial> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut dirs: Vec<PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join("README.md").is_file() && p.join(gbade_toolchain::project::PROJECT_FILE).is_file())
        .collect();
    dirs.sort();
    dirs.into_iter()
        .map(|d| {
            let text = std::fs::read_to_string(d.join("README.md")).unwrap_or_default();
            let (title, summary) = title_and_summary(&text);
            let title = title.unwrap_or_else(|| d.file_name().unwrap_or_default().to_string_lossy().into_owned());
            let name = strip_number(&title).to_string();
            Tutorial { dir: d, title, name, summary }
        })
        .collect()
}

/// The `# ` heading and the first paragraph after it.
fn title_and_summary(md: &str) -> (Option<String>, String) {
    let blocks = docs::parse(md);
    let title = blocks.iter().find_map(|b| match b {
        Block::Heading(1, t) => Some(t.clone()),
        _ => None,
    });
    let summary = blocks
        .iter()
        .find_map(|b| match b {
            Block::Paragraph(p) => Some(p.clone()),
            _ => None,
        })
        .unwrap_or_default();
    (title, summary)
}

/// "3. Buttons" -> "Buttons".
pub fn strip_number(title: &str) -> &str {
    match title.split_once(". ") {
        Some((n, rest)) if !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) => rest,
        _ => title,
    }
}

/// What the user asked for in the panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TutorialAction {
    /// Copy this tutorial's example into a new project and run it.
    TryExample(Tutorial),
}

/// The panel state: which tutorial is shown.
#[derive(Default)]
pub struct TutorialsViewer {
    pub current: Option<PathBuf>,
    blocks: Vec<Block>,
}

impl TutorialsViewer {
    pub fn open(&mut self, t: &Tutorial) {
        let text = std::fs::read_to_string(t.dir.join("README.md")).unwrap_or_default();
        // The title is shown in the header with the Try button.
        self.blocks = docs::parse(&text).into_iter().filter(|b| !matches!(b, Block::Heading(1, _))).collect();
        self.current = Some(t.dir.clone());
    }

    pub fn ui(&mut self, ui: &mut Ui, dir: Option<&Path>) -> Option<TutorialAction> {
        let Some(dir) = dir else {
            ui.label("Tutorials not found.");
            return None;
        };
        let all = list(dir);
        if all.is_empty() {
            ui.label("No tutorials found.");
            return None;
        }
        let index = self.current.as_ref().and_then(|c| all.iter().position(|t| &t.dir == c));
        let index = match index {
            Some(i) => i,
            None => {
                self.open(&all[0]);
                0
            }
        };
        let mut action = None;
        let mut go_to: Option<usize> = None;

        egui::SidePanel::left("tutorials-index").resizable(true).default_width(220.0).show_inside(ui, |ui| {
            ui.label(RichText::new("Tutorials").strong());
            ui.label(RichText::new("Short lessons, each with an example to run.").small().weak());
            ui.add_space(4.0);
            egui::ScrollArea::vertical().show(ui, |ui| {
                for (i, t) in all.iter().enumerate() {
                    if ui.selectable_label(i == index, &t.title).on_hover_text(&t.summary).clicked() {
                        go_to = Some(i);
                    }
                }
            });
        });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            let t = &all[index];
            ui.horizontal(|ui| {
                ui.label(RichText::new(&t.title).size(24.0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(egui::Button::new(RichText::new("▶ Try this example").size(15.0)).fill(crate::panels::ACCENT))
                        .on_hover_text("Copy this tutorial's example into a new project in your projects folder, then build and run it")
                        .clicked()
                    {
                        action = Some(TutorialAction::TryExample(t.clone()));
                    }
                });
            });
            ui.separator();
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.set_max_width(760.0);
                for a in docs::render(ui, &self.blocks, &t.dir) {
                    match a {
                        DocAction::OpenUrl(u) => ui.ctx().open_url(egui::OpenUrl::new_tab(u)),
                        DocAction::OpenDoc(_) => {}
                    }
                }
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if index > 0 && ui.button(format!("← {}", all[index - 1].title)).clicked() {
                        go_to = Some(index - 1);
                    }
                    if index + 1 < all.len() && ui.button(format!("{} →", all[index + 1].title)).clicked() {
                        go_to = Some(index + 1);
                    }
                });
            });
        });

        if let Some(i) = go_to {
            self.open(&all[i]);
        }
        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make(dir: &Path, folder: &str, readme: &str) {
        let d = dir.join(folder);
        std::fs::create_dir_all(d.join("src")).unwrap();
        std::fs::write(d.join("README.md"), readme).unwrap();
        std::fs::write(d.join("gbade.toml"), "[game]\nname = \"T\"\n").unwrap();
        std::fs::write(d.join("src/main.c"), "int main(void) { for (;;); }\n").unwrap();
    }

    #[test]
    fn tutorials_are_listed_in_folder_order_with_titles() {
        let dir = tempfile::tempdir().unwrap();
        make(dir.path(), "02-loop", "# 2. The Game Loop\n\nEvery game is a loop.\nRead, update, draw.\n\n## Next\n");
        make(dir.path(), "01-pixels", "# 1. First Pixels\n\nPut colours on the screen.\n");
        std::fs::create_dir_all(dir.path().join("03-unfinished")).unwrap(); // no README: skipped
        let all = list(dir.path());
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].title, "1. First Pixels");
        assert_eq!(all[0].name, "First Pixels");
        assert_eq!(all[1].summary, "Every game is a loop. Read, update, draw.");
    }

    #[test]
    fn numbers_are_stripped_from_names_only_when_present() {
        assert_eq!(strip_number("10. Sound Effects"), "Sound Effects");
        assert_eq!(strip_number("Sound Effects"), "Sound Effects");
        assert_eq!(strip_number("v2. Thing"), "v2. Thing");
    }

    #[test]
    fn the_shipped_tutorials_are_found() {
        let dir = crate::resources::tutorials_dir().expect("tutorials/ in the checkout");
        let all = list(&dir);
        assert!(all.len() >= 10, "{all:?}");
        for t in &all {
            assert!(t.title.starts_with(char::is_numeric), "{}", t.title);
            assert!(!t.summary.is_empty(), "{} has no summary", t.title);
        }
    }

    #[test]
    fn opening_a_tutorial_keeps_its_title_out_of_the_body() {
        let dir = tempfile::tempdir().unwrap();
        make(dir.path(), "01-a", "# 1. A\n\nIntro.\n\n## Part\n\nText.\n");
        let t = &list(dir.path())[0];
        let mut v = TutorialsViewer::default();
        v.open(t);
        assert_eq!(v.current.as_deref(), Some(t.dir.as_path()));
        assert!(!v.blocks.iter().any(|b| matches!(b, Block::Heading(1, _))));
        assert!(v.blocks.iter().any(|b| matches!(b, Block::Heading(2, h) if h == "Part")));
    }
}
