//! Glue between the app and clangd (through `gbade-lsp`): keeps open files in
//! sync with the language server, turns editor requests into LSP requests,
//! and routes the answers (live diagnostics, completions, hover text,
//! definitions) back to the documents and panels.

use crate::app::{GbadeApp, same_file};
use crate::assist::Completion;
use crate::editor::{EditorEvent, hash_str, line_of_char, line_start_char};
use gbade_lsp::{Event, LspClient, Pos};
use gbade_toolchain::Diagnostic;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Language server state for the open project.
pub struct Lsp {
    client: LspClient,
    /// Text hash last sent to the server, per open file.
    synced: HashMap<PathBuf, u64>,
    /// When each file last changed in the editor (changes are debounced).
    edited: HashMap<PathBuf, Instant>,
    /// Outstanding requests: id -> (file, char index of the word start).
    completion: Option<(i64, PathBuf, usize)>,
    hover: Option<(i64, PathBuf, usize)>,
    definition: Option<i64>,
}

/// Send edits to clangd at most this often while typing.
const DEBOUNCE: Duration = Duration::from_millis(250);

/// Files clangd should look at: C sources and headers of the project.
fn is_c_file(p: &Path) -> bool {
    matches!(p.extension().and_then(|e| e.to_str()), Some("c" | "h"))
}

/// Char index -> LSP position (0-based line, character within the line).
pub fn char_to_pos(text: &str, ci: usize) -> Pos {
    let line = line_of_char(text, ci);
    let start = line_start_char(text, line);
    Pos { line: line - 1, character: (ci - start) as u32 }
}

impl GbadeApp {
    /// Start clangd for the current project (if the toolchain has it).
    pub fn start_lsp(&mut self) {
        self.lsp = None;
        self.live_diagnostics.clear();
        let (Some(project), Ok(tc)) = (&self.project, &self.toolchain) else { return };
        let Some(clangd) = &tc.clangd else { return };
        if let Err(e) = gbade_toolchain::builder::write_compile_commands(project, tc) {
            self.set_status(format!("Code assistance is off: cannot write compile_commands.json ({e})"), true);
            return;
        }
        match LspClient::start(clangd, &project.root, &project.build_dir()) {
            Ok(client) => {
                self.lsp = Some(Lsp {
                    client,
                    synced: HashMap::new(),
                    edited: HashMap::new(),
                    completion: None,
                    hover: None,
                    definition: None,
                })
            }
            Err(e) => self.set_status(format!("Code assistance is off: clangd did not start ({e})"), true),
        }
    }

    /// Called every frame: sync open files, process the server's answers.
    pub fn lsp_tick(&mut self) {
        let Some(lsp) = &mut self.lsp else { return };
        let now = Instant::now();

        // 1. Open files: didOpen once, then debounced didChange.
        let open: Vec<PathBuf> = self.docs.iter().filter(|d| is_c_file(&d.path)).map(|d| d.path.clone()).collect();
        for d in self.docs.iter().filter(|d| is_c_file(&d.path)) {
            let h = hash_str(&d.text);
            match lsp.synced.get(&d.path) {
                None => {
                    lsp.client.did_open(&d.path, &d.text);
                    lsp.synced.insert(d.path.clone(), h);
                }
                Some(&old) if old != h => {
                    let since = *lsp.edited.entry(d.path.clone()).or_insert(now);
                    // Send right away if a completion is waiting for fresh
                    // text, otherwise once typing pauses.
                    if now.duration_since(since) >= DEBOUNCE || lsp.completion.is_some() {
                        lsp.client.did_change(&d.path, &d.text);
                        lsp.synced.insert(d.path.clone(), h);
                        lsp.edited.remove(&d.path);
                    }
                }
                Some(_) => {
                    lsp.edited.remove(&d.path);
                }
            }
        }
        // Closed tabs.
        let closed: Vec<PathBuf> = lsp.synced.keys().filter(|p| !open.contains(p)).cloned().collect();
        for p in closed {
            lsp.client.did_close(&p);
            lsp.synced.remove(&p);
            self.live_diagnostics.remove(&p);
        }

        // 2. Answers.
        for ev in lsp.client.poll() {
            match ev {
                Event::Diagnostics(path, diags) => {
                    if lsp.synced.contains_key(&path) {
                        self.live_diagnostics.insert(path, diags);
                    }
                }
                Event::Completion { id, items } => {
                    if let Some((want, file, start)) = lsp.completion.take() {
                        if want == id {
                            if let Some(d) = self.docs.iter_mut().find(|d| d.path == file) {
                                d.completion = (!items.is_empty()).then_some(Completion { items, selected: 0, start });
                            }
                        } else {
                            lsp.completion = Some((want, file, start));
                        }
                    }
                }
                Event::Hover { id, text } => {
                    if let Some((want, file, word)) = lsp.hover.take() {
                        if want == id {
                            if let Some(d) = self.docs.iter_mut().find(|d| d.path == file) {
                                d.hover.answer = Some((word, text));
                            }
                        } else {
                            lsp.hover = Some((want, file, word));
                        }
                    }
                }
                Event::Definition { id, target } => {
                    if lsp.definition == Some(id) {
                        lsp.definition = None;
                        match target {
                            Some((file, pos)) => {
                                let root = self.project.as_ref().map(|p| p.root.clone());
                                self.open_file(&file, Some(pos.line + 1));
                                if let (Some(root), Some(d)) = (root, self.docs.get_mut(self.active_doc)) {
                                    d.read_only = !d.path.starts_with(&root);
                                }
                            }
                            None => self.set_status("No definition found for this name.", false),
                        }
                        return;
                    }
                }
                Event::Exited => {
                    self.lsp = None;
                    self.live_diagnostics.clear();
                    self.set_status("The code assistant (clangd) stopped. Reopen the project to restart it.", true);
                    return;
                }
            }
        }
    }

    /// Forward an editor request for the active document to clangd.
    pub fn lsp_request(&mut self, ev: &EditorEvent) {
        let Some(doc) = self.docs.get(self.active_doc) else { return };
        let Some(lsp) = &mut self.lsp else {
            if matches!(ev, EditorEvent::GotoDefinition(_)) {
                self.set_status("Go to definition needs the code assistant (clangd), which is not available.", true);
            }
            return;
        };
        if !is_c_file(&doc.path) {
            return;
        }
        // Make sure the server has the text the position refers to.
        let h = hash_str(&doc.text);
        if lsp.synced.get(&doc.path) != Some(&h) {
            if lsp.synced.contains_key(&doc.path) {
                lsp.client.did_change(&doc.path, &doc.text);
            } else {
                lsp.client.did_open(&doc.path, &doc.text);
            }
            lsp.synced.insert(doc.path.clone(), h);
        }
        match *ev {
            EditorEvent::Complete(ci) => {
                let chars: Vec<char> = doc.text.chars().collect();
                let start = crate::assist::word_start(&chars, ci);
                let id = lsp.client.completion(&doc.path, char_to_pos(&doc.text, ci));
                lsp.completion = Some((id, doc.path.clone(), start));
            }
            EditorEvent::Hover(word) => {
                let id = lsp.client.hover(&doc.path, char_to_pos(&doc.text, word));
                lsp.hover = Some((id, doc.path.clone(), word));
            }
            EditorEvent::GotoDefinition(ci) => {
                lsp.definition = Some(lsp.client.definition(&doc.path, char_to_pos(&doc.text, ci)));
            }
            EditorEvent::ToggleBreakpoint(_) => {}
        }
    }

    /// The problems to show: the last build's, except that files clangd has
    /// checked since show clangd's fresh results instead.
    pub fn current_diagnostics(&self) -> Vec<Diagnostic> {
        let mut out: Vec<Diagnostic> = self
            .diagnostics
            .iter()
            .filter(|d| match &d.file {
                Some(f) => !self.live_diagnostics.keys().any(|l| same_file(l, f)) || is_link_error(d),
                None => true,
            })
            .cloned()
            .collect();
        for diags in self.live_diagnostics.values() {
            out.extend(diags.iter().cloned());
        }
        out
    }
}

/// Linker errors ("undefined symbol...") point at a source line but clangd
/// cannot know about them, so they stay until the next build.
fn is_link_error(d: &Diagnostic) -> bool {
    d.message.starts_with("undefined symbol") || d.message.starts_with("duplicate symbol") || d.message.contains("will not fit in region")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_are_zero_based() {
        let t = "ab\ncdé\nx";
        assert_eq!(char_to_pos(t, 0), Pos { line: 0, character: 0 });
        assert_eq!(char_to_pos(t, 4), Pos { line: 1, character: 1 });
        assert_eq!(char_to_pos(t, 6), Pos { line: 1, character: 3 });
        assert_eq!(char_to_pos(t, 7), Pos { line: 2, character: 0 });
    }

    #[test]
    fn link_errors_survive_live_checking() {
        assert!(is_link_error(&Diagnostic::error("undefined symbol: draw_hero")));
        assert!(!is_link_error(&Diagnostic::error("use of undeclared identifier 'x'")));
    }
}
