//! Code assistance in the editor, powered by clangd through `gbade-lsp`:
//! the completion popup, hover documentation and go-to-definition.
//!
//! The editor (`editor.rs`) calls the functions here at a few hook points and
//! reports requests as `EditorEvent`s; the app (`app.rs`) forwards them to the
//! language server and stores the answers back in the `Document`.

use crate::editor::{char_to_byte, set_cursor};
use eframe::egui::{self, Key, Modifiers, RichText, Ui};
use gbade_lsp::CompletionItem;
use std::time::{Duration, Instant};

/// An open completion popup.
#[derive(Debug, Clone)]
pub struct Completion {
    pub items: Vec<CompletionItem>,
    pub selected: usize,
    /// Char index where the word being completed starts.
    pub start: usize,
}

/// Hover state of a document.
#[derive(Debug, Clone, Default)]
pub struct HoverState {
    /// Word under the pointer (its start char index) and since when.
    probe: Option<(usize, Instant)>,
    /// Hover request already sent for this word start.
    requested: Option<usize>,
    /// Answer from clangd: (word start, text).
    pub answer: Option<(usize, String)>,
}

pub fn is_ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Start (char index) of the identifier ending at `ci`.
pub fn word_start(chars: &[char], ci: usize) -> usize {
    let mut s = ci.min(chars.len());
    while s > 0 && is_ident(chars[s - 1]) {
        s -= 1;
    }
    s
}

/// What to do after the user typed something (cursor now at `ci`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypingAction {
    /// Ask clangd for completions here.
    Request,
    /// Keep the popup, refiltered with the longer prefix.
    Refilter,
    Close,
}

/// Decide whether typing should open, keep or close the completion popup.
pub fn after_typing(chars: &[char], ci: usize, popup_open: bool) -> TypingAction {
    let prev = if ci > 0 { chars.get(ci - 1).copied() } else { None };
    match prev {
        Some(c) if is_ident(c) => {
            let start = word_start(chars, ci);
            let starts_with_digit = chars.get(start).is_some_and(|c| c.is_ascii_digit());
            if starts_with_digit {
                TypingAction::Close
            } else if popup_open {
                TypingAction::Refilter
            } else if ci - start >= 2 {
                TypingAction::Request
            } else {
                TypingAction::Close
            }
        }
        // Member access: `player.` and `player->`
        Some('.') => TypingAction::Request,
        Some('>') if ci >= 2 && chars[ci - 2] == '-' => TypingAction::Request,
        _ => TypingAction::Close,
    }
}

impl Completion {
    /// Items matching the typed prefix: prefix matches first, then the ones
    /// containing it, in clangd's order.
    pub fn visible<'a>(&'a self, prefix: &str) -> Vec<&'a CompletionItem> {
        let p = prefix.to_ascii_lowercase();
        let mut first: Vec<&CompletionItem> = Vec::new();
        let mut other: Vec<&CompletionItem> = Vec::new();
        for it in &self.items {
            let name = it.insert.to_ascii_lowercase();
            if name.starts_with(&p) {
                first.push(it);
            } else if !p.is_empty() && name.contains(&p) {
                other.push(it);
            }
        }
        first.extend(other);
        first.truncate(200);
        first
    }
}

/// Replace the word being completed with `insert` and move the cursor.
pub fn accept(ui: &Ui, id: egui::Id, text: &mut String, start: usize, cursor: usize, insert: &str) {
    let (a, b) = (start.min(cursor), cursor.max(start));
    let ba = char_to_byte(text, a);
    let bb = char_to_byte(text, b);
    text.replace_range(ba..bb, insert);
    set_cursor(ui, id, a + insert.chars().count());
}

/// Keys for the popup, handled before the text field sees them.
/// Returns true if the popup consumed a key.
pub fn popup_keys(ui: &Ui, completion: &mut Option<Completion>, visible_len: usize) -> Option<PopupKey> {
    let c = completion.as_mut()?;
    if visible_len == 0 {
        return None;
    }
    let pressed = |k: Key| ui.input_mut(|i| i.consume_key(Modifiers::NONE, k));
    if pressed(Key::ArrowDown) {
        c.selected = (c.selected + 1) % visible_len;
        Some(PopupKey::Moved)
    } else if pressed(Key::ArrowUp) {
        c.selected = (c.selected + visible_len - 1) % visible_len;
        Some(PopupKey::Moved)
    } else if pressed(Key::Enter) || pressed(Key::Tab) {
        Some(PopupKey::Accept)
    } else if pressed(Key::Escape) {
        *completion = None;
        Some(PopupKey::Closed)
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupKey {
    Moved,
    Accept,
    Closed,
}

/// Draw the completion list under the cursor. Returns the index of an item
/// the user clicked.
pub fn popup_ui(ctx: &egui::Context, id: egui::Id, at: egui::Pos2, items: &[&CompletionItem], selected: usize) -> Option<usize> {
    let mut clicked = None;
    egui::Area::new(id.with("completion"))
        .order(egui::Order::Foreground)
        .fixed_pos(at)
        .constrain(true)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).inner_margin(4.0).show(ui, |ui| {
                ui.set_max_width(460.0);
                // Show a window of 10 items around the selection.
                let first = selected.saturating_sub(4).min(items.len().saturating_sub(10));
                for (i, it) in items.iter().enumerate().skip(first).take(10) {
                    let text = RichText::new(format!("{:<28} {}", it.label, it.detail)).monospace();
                    let resp = ui.selectable_label(i == selected, text).on_hover_text(it.kind);
                    if i == selected {
                        resp.scroll_to_me(None);
                    }
                    if resp.clicked() {
                        clicked = Some(i);
                    }
                }
                ui.label(RichText::new(format!("{} matches · Up/Down choose · Enter/Tab insert · Esc close", items.len())).small().weak());
            });
        });
    clicked
}

impl HoverState {
    /// Track the word under the pointer. Returns `Some(char index)` when the
    /// pointer has rested on a word long enough to ask clangd about it.
    pub fn update(&mut self, word: Option<usize>, ctx: &egui::Context) -> Option<usize> {
        const DELAY: Duration = Duration::from_millis(450);
        match (word, self.probe) {
            (None, _) => {
                self.probe = None;
                None
            }
            (Some(w), Some((p, since))) if p == w => {
                if self.requested != Some(w) && since.elapsed() >= DELAY {
                    self.requested = Some(w);
                    self.answer = None;
                    return Some(w);
                }
                if self.requested != Some(w) {
                    ctx.request_repaint_after(DELAY);
                }
                None
            }
            (Some(w), _) => {
                self.probe = Some((w, Instant::now()));
                self.requested = None;
                ctx.request_repaint_after(DELAY);
                None
            }
        }
    }

    /// The answer for this word, if it arrived.
    pub fn text_for(&self, word: usize) -> Option<&str> {
        self.answer.as_ref().filter(|(w, t)| *w == word && !t.is_empty()).map(|(_, t)| t.as_str())
    }

    /// The text changed: forget everything.
    pub fn reset(&mut self) {
        *self = HoverState::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    #[test]
    fn typing_rules() {
        let c = chars("int x = ke");
        assert_eq!(after_typing(&c, c.len(), false), TypingAction::Request);
        let c = chars("int x = k");
        assert_eq!(after_typing(&c, c.len(), false), TypingAction::Close, "one letter is not enough");
        assert_eq!(after_typing(&c, c.len(), true), TypingAction::Refilter);
        let c = chars("player.");
        assert_eq!(after_typing(&c, c.len(), false), TypingAction::Request);
        let c = chars("p->");
        assert_eq!(after_typing(&c, c.len(), false), TypingAction::Request);
        let c = chars("a > ");
        assert_eq!(after_typing(&c, c.len(), true), TypingAction::Close);
        let c = chars("x = 12");
        assert_eq!(after_typing(&c, c.len(), false), TypingAction::Close, "numbers do not complete");
        assert_eq!(word_start(&chars("foo key_hi"), 10), 4);
    }

    #[test]
    fn filtering_prefers_prefix_matches() {
        let item = |n: &str| CompletionItem { label: n.into(), insert: n.into(), detail: String::new(), kind: "function" };
        let c = Completion { items: vec![item("sprite_hide"), item("oam_hide"), item("sprite_show"), item("vsync")], selected: 0, start: 0 };
        let v: Vec<&str> = c.visible("spr").iter().map(|i| i.insert.as_str()).collect();
        assert_eq!(v, vec!["sprite_hide", "sprite_show"]);
        let v: Vec<&str> = c.visible("hide").iter().map(|i| i.insert.as_str()).collect();
        assert_eq!(v, vec!["sprite_hide", "oam_hide"]);
        assert_eq!(c.visible("").len(), 4);
        assert!(c.visible("zzz").is_empty());
    }
}
