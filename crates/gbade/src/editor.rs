//! The code editor: a text area with line numbers, C syntax highlighting,
//! clickable breakpoints in the gutter, error/warning markers, auto-indent
//! and comment toggling.

use crate::highlight::{self, Theme};
use eframe::egui::{
    self, Align, Color32, CornerRadius, FontId, Key, Modifiers, Pos2, Rect, Sense, Shape, Stroke, TextBuffer,
    TextEdit, Ui, Vec2,
    text::{CCursor, CCursorRange, LayoutJob},
};
use gbade_toolchain::Severity;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

/// An open file.
pub struct Document {
    pub path: PathBuf,
    pub text: String,
    saved_text: String,
    disk_mtime: Option<SystemTime>,
    pub id: egui::Id,
    /// Scroll to and place the cursor on this line (1-based) next frame.
    pub goto_line: Option<u32>,
    highlight_cache: Option<(u64, bool, f32, Arc<LayoutJobHolder>)>,
}

struct LayoutJobHolder(LayoutJob);

impl Document {
    pub fn open(path: &Path) -> std::io::Result<Document> {
        let text = std::fs::read_to_string(path)?;
        Ok(Document {
            path: path.to_path_buf(),
            saved_text: text.clone(),
            text,
            disk_mtime: mtime(path),
            id: egui::Id::new(("editor", path.to_path_buf())),
            goto_line: None,
            highlight_cache: None,
        })
    }

    pub fn is_dirty(&self) -> bool {
        self.text != self.saved_text
    }

    pub fn save(&mut self) -> std::io::Result<()> {
        std::fs::write(&self.path, &self.text)?;
        self.saved_text = self.text.clone();
        self.disk_mtime = mtime(&self.path);
        Ok(())
    }

    /// If the file changed on disk (edited in another program) and we have no
    /// unsaved changes, reload it. Returns true if reloaded.
    pub fn reload_if_changed(&mut self) -> bool {
        let now = mtime(&self.path);
        if now.is_some() && now != self.disk_mtime && !self.is_dirty() {
            if let Ok(t) = std::fs::read_to_string(&self.path) {
                self.text = t.clone();
                self.saved_text = t;
                self.disk_mtime = now;
                return true;
            }
        }
        false
    }

    pub fn file_name(&self) -> String {
        self.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
    }

    fn highlighted(&mut self, theme: Theme, size: f32) -> LayoutJob {
        let hash = hash_str(&self.text);
        if let Some((h, dark, s, job)) = &self.highlight_cache {
            if *h == hash && *dark == theme.dark && *s == size {
                return job.0.clone();
            }
        }
        let job = highlight::layout_job(
            &self.text,
            theme,
            FontId::monospace(size),
            highlight::is_c_like(&self.path),
        );
        self.highlight_cache = Some((hash, theme.dark, size, Arc::new(LayoutJobHolder(job.clone()))));
        job
    }
}

fn mtime(p: &Path) -> Option<SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

fn hash_str(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// A marker shown on a line.
#[derive(Debug, Clone)]
pub struct LineMessage {
    pub line: u32,
    pub severity: Severity,
    pub text: String,
}

/// What to show in the margin and line backgrounds.
#[derive(Default)]
pub struct Marks<'a> {
    pub breakpoints: Option<&'a BTreeSet<u32>>,
    pub messages: Vec<LineMessage>,
    /// The line where the game is paused (yellow arrow).
    pub current_line: Option<u32>,
}

pub enum EditorEvent {
    ToggleBreakpoint(u32),
}

/// Convert a char index to a byte index.
fn char_to_byte(s: &str, ci: usize) -> usize {
    s.char_indices().nth(ci).map(|(b, _)| b).unwrap_or(s.len())
}

/// Char index of the start of 1-based line `line`.
pub fn line_start_char(s: &str, line: u32) -> usize {
    if line <= 1 {
        return 0;
    }
    let mut l = 1;
    for (ci, c) in s.chars().enumerate() {
        if c == '\n' {
            l += 1;
            if l == line {
                return ci + 1;
            }
        }
    }
    s.chars().count()
}

/// 1-based line of a char index.
pub fn line_of_char(s: &str, ci: usize) -> u32 {
    1 + s.chars().take(ci).filter(|&c| c == '\n').count() as u32
}

/// Leading whitespace of the line that contains byte `b`.
fn indent_of_line_before(s: &str, byte: usize) -> (String, bool) {
    let line_start = s[..byte].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let line = &s[line_start..byte];
    let indent: String = line.chars().take_while(|c| *c == ' ' || *c == '\t').collect();
    let opens = line.trim_end().ends_with('{');
    (indent, opens)
}

/// Toggle `// ` on every line touched by [start, end) (char indices).
/// Returns the new text.
pub fn toggle_comment(text: &str, start_line: u32, end_line: u32) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let range = (start_line.max(1) as usize - 1)..(end_line as usize).min(lines.len());
    let all_commented = lines[range.clone()]
        .iter()
        .filter(|l| !l.trim().is_empty())
        .all(|l| l.trim_start().starts_with("//"));
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    for (i, l) in lines.iter().enumerate() {
        if range.contains(&i) && !l.trim().is_empty() {
            let indent_len = l.len() - l.trim_start().len();
            let (indent, rest) = l.split_at(indent_len);
            if all_commented {
                let rest = rest.strip_prefix("// ").or_else(|| rest.strip_prefix("//")).unwrap_or(rest);
                out.push(format!("{indent}{rest}"));
            } else {
                out.push(format!("{indent}// {rest}"));
            }
        } else {
            out.push(l.to_string());
        }
    }
    out.join("\n")
}

/// Draw the editor. Returns events like breakpoint toggles.
pub fn editor_ui(ui: &mut Ui, doc: &mut Document, marks: &Marks, dark: bool, font_size: f32) -> Vec<EditorEvent> {
    let mut events = Vec::new();
    let theme = Theme { dark };
    let font = FontId::monospace(font_size);
    let row_h = ui.fonts_mut(|f| f.row_height(&font));
    let char_w = ui.fonts_mut(|f| f.glyph_width(&font, '0'));
    let n_lines = doc.text.split('\n').count().max(1);
    let digits = n_lines.to_string().len().max(3);
    let gutter_w = digits as f32 * char_w + 26.0;

    // --- Keyboard shortcuts handled before the TextEdit sees the keys -------
    let has_focus = ui.memory(|m| m.has_focus(doc.id));
    let mut insert_at_cursor: Option<String> = None;
    let mut toggle_comment_req = false;
    if has_focus {
        if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Tab)) {
            insert_at_cursor = Some("    ".into());
        }
        if ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Slash)) {
            toggle_comment_req = true;
        }
    }
    if insert_at_cursor.is_some() || toggle_comment_req {
        if let Some(state) = egui::TextEdit::load_state(ui.ctx(), doc.id) {
            if let Some(range) = state.cursor.char_range() {
                let (a, b) = (range.primary.index.min(range.secondary.index), range.primary.index.max(range.secondary.index));
                if let Some(ins) = &insert_at_cursor {
                    let ba = char_to_byte(&doc.text, a);
                    let bb = char_to_byte(&doc.text, b);
                    doc.text.replace_range(ba..bb, ins);
                    set_cursor(ui, doc.id, a + ins.chars().count());
                } else {
                    let l1 = line_of_char(&doc.text, a);
                    let l2 = line_of_char(&doc.text, b.max(a));
                    let before_len = doc.text.chars().count();
                    doc.text = toggle_comment(&doc.text, l1, l2);
                    let delta = doc.text.chars().count() as isize - before_len as isize;
                    set_cursor(ui, doc.id, (b as isize + delta).max(0) as usize);
                }
            }
        }
    }

    let job = doc.highlighted(theme, font_size);
    let goto = doc.goto_line.take();

    egui::ScrollArea::both()
        .id_salt(("editor-scroll", doc.id))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                let content_h = (n_lines as f32 + 2.0) * row_h;
                let (gutter_rect, gutter_resp) =
                    ui.allocate_exact_size(Vec2::new(gutter_w, content_h.max(ui.available_height())), Sense::click());
                let bg_idx = ui.painter().add(Shape::Noop);

                let mut layouter = |ui: &Ui, _buf: &dyn TextBuffer, _wrap: f32| {
                    let mut j = job.clone();
                    j.wrap.max_width = f32::INFINITY;
                    ui.fonts_mut(|f| f.layout_job(j))
                };
                let before = doc.text.clone();
                let output = TextEdit::multiline(&mut doc.text)
                    .id(doc.id)
                    .font(font.clone())
                    .code_editor()
                    .lock_focus(true)
                    .frame(false)
                    .desired_width(f32::INFINITY)
                    .desired_rows(n_lines.max(20))
                    .margin(Vec2::new(6.0, 0.0))
                    .layouter(&mut layouter)
                    .show(ui);

                // --- Auto-indent: when Enter was pressed, copy indentation. ----
                if output.response.changed() && doc.text.len() == before.len() + 1 {
                    if let Some(r) = output.cursor_range {
                        let ci = r.primary.index;
                        let bi = char_to_byte(&doc.text, ci);
                        if bi > 0 && doc.text.as_bytes()[bi - 1] == b'\n' {
                            let (mut indent, opens) = indent_of_line_before(&doc.text, bi - 1);
                            if opens {
                                indent.push_str("    ");
                            }
                            if !indent.is_empty() {
                                doc.text.insert_str(bi, &indent);
                                set_cursor(ui, doc.id, ci + indent.chars().count());
                            }
                        }
                    }
                }

                // --- Line positions from the galley --------------------------
                let galley = &output.galley;
                let origin = output.galley_pos;
                let mut line_y: Vec<f32> = Vec::with_capacity(n_lines);
                let mut new_line = true;
                for row in &galley.rows {
                    if new_line {
                        line_y.push(origin.y + row.rect().min.y);
                    }
                    new_line = row.row.ends_with_newline;
                }
                let y_of = |line: u32| -> f32 {
                    let i = (line.max(1) - 1) as usize;
                    line_y.get(i).copied().unwrap_or(origin.y + i as f32 * row_h)
                };
                let full_w = ui.clip_rect().right() - gutter_rect.left() + 1000.0;

                // --- Backgrounds: current line, error lines -----------------
                let mut bg = Vec::new();
                for m in &marks.messages {
                    let c = match m.severity {
                        Severity::Error => Color32::from_rgba_unmultiplied(220, 50, 50, 40),
                        Severity::Warning => Color32::from_rgba_unmultiplied(220, 180, 40, 30),
                        Severity::Note => Color32::TRANSPARENT,
                    };
                    let y = y_of(m.line);
                    bg.push(Shape::rect_filled(
                        Rect::from_min_size(Pos2::new(gutter_rect.left(), y), Vec2::new(full_w, row_h)),
                        0.0,
                        c,
                    ));
                }
                if let Some(cl) = marks.current_line {
                    let y = y_of(cl);
                    bg.push(Shape::rect_filled(
                        Rect::from_min_size(Pos2::new(gutter_rect.left(), y), Vec2::new(full_w, row_h)),
                        0.0,
                        Color32::from_rgba_unmultiplied(255, 220, 0, 50),
                    ));
                }
                ui.painter().set(bg_idx, bg);

                // --- Gutter -------------------------------------------------
                let painter = ui.painter_at(gutter_rect.expand2(Vec2::new(0.0, 4.0)));
                let gutter_bg = if dark { Color32::from_gray(30) } else { Color32::from_gray(240) };
                painter.rect_filled(gutter_rect, CornerRadius::ZERO, gutter_bg);
                let num_color = if dark { Color32::from_gray(110) } else { Color32::from_gray(150) };
                let visible = ui.clip_rect();
                for line in 1..=n_lines as u32 {
                    let y = y_of(line);
                    if y + row_h < visible.top() || y > visible.bottom() {
                        continue;
                    }
                    painter.text(
                        Pos2::new(gutter_rect.right() - 6.0, y),
                        egui::Align2::RIGHT_TOP,
                        line.to_string(),
                        font.clone(),
                        num_color,
                    );
                    let cy = y + row_h / 2.0;
                    if marks.breakpoints.is_some_and(|b| b.contains(&line)) {
                        painter.circle_filled(Pos2::new(gutter_rect.left() + 9.0, cy), row_h * 0.32, Color32::from_rgb(220, 40, 40));
                    }
                    if let Some(m) = marks.messages.iter().find(|m| m.line == line && m.severity != Severity::Note) {
                        let c = if m.severity == Severity::Error { Color32::from_rgb(240, 70, 70) } else { Color32::from_rgb(230, 180, 40) };
                        painter.rect_filled(
                            Rect::from_min_size(Pos2::new(gutter_rect.right() - 3.0, y), Vec2::new(3.0, row_h)),
                            0.0,
                            c,
                        );
                    }
                    if marks.current_line == Some(line) {
                        let x = gutter_rect.left() + 3.0;
                        let pts = vec![Pos2::new(x, cy - 5.0), Pos2::new(x + 10.0, cy), Pos2::new(x, cy + 5.0)];
                        painter.add(Shape::convex_polygon(pts, Color32::from_rgb(255, 210, 0), Stroke::NONE));
                    }
                }

                // Hover the gutter: show messages; click: toggle breakpoint.
                if let Some(pos) = gutter_resp.hover_pos() {
                    let line = line_at(&line_y, row_h, origin.y, pos.y).min(n_lines as u32);
                    let msgs: Vec<&LineMessage> = marks.messages.iter().filter(|m| m.line == line).collect();
                    if !msgs.is_empty() {
                        gutter_resp.clone().on_hover_ui_at_pointer(|ui| {
                            ui.set_max_width(420.0);
                            for m in msgs {
                                ui.label(&m.text);
                            }
                        });
                    } else {
                        gutter_resp.clone().on_hover_text_at_pointer("Click to add/remove a breakpoint");
                    }
                }
                if gutter_resp.clicked() {
                    if let Some(pos) = gutter_resp.interact_pointer_pos() {
                        let line = line_at(&line_y, row_h, origin.y, pos.y);
                        if line >= 1 && line as usize <= n_lines {
                            events.push(EditorEvent::ToggleBreakpoint(line));
                        }
                    }
                }
                // Hovering a line with an error inside the text also explains it.
                if let Some(pos) = output.response.hover_pos() {
                    let line = line_at(&line_y, row_h, origin.y, pos.y);
                    let msgs: Vec<&LineMessage> = marks.messages.iter().filter(|m| m.line == line).collect();
                    if !msgs.is_empty() {
                        output.response.clone().on_hover_ui_at_pointer(|ui| {
                            ui.set_max_width(420.0);
                            for m in msgs {
                                ui.label(&m.text);
                            }
                        });
                    }
                }

                if let Some(line) = goto {
                    let ci = line_start_char(&doc.text, line);
                    set_cursor(ui, doc.id, ci);
                    ui.memory_mut(|m| m.request_focus(doc.id));
                    let y = y_of(line);
                    ui.scroll_to_rect(
                        Rect::from_min_size(Pos2::new(gutter_rect.left(), y), Vec2::new(10.0, row_h)),
                        Some(Align::Center),
                    );
                }
            });
        });
    events
}

fn line_at(line_y: &[f32], row_h: f32, origin_y: f32, y: f32) -> u32 {
    if line_y.is_empty() {
        return 1 + ((y - origin_y) / row_h).max(0.0) as u32;
    }
    let i = line_y.partition_point(|&ly| ly <= y);
    if i == 0 { 1 } else if i == line_y.len() && y > line_y[i - 1] + row_h { (i + 1) as u32 } else { i as u32 }
}

fn set_cursor(ui: &Ui, id: egui::Id, ci: usize) {
    let mut state = egui::TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
    state.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(ci))));
    state.store(ui.ctx(), id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_helpers() {
        let s = "ab\ncd\n\nef";
        assert_eq!(line_start_char(s, 1), 0);
        assert_eq!(line_start_char(s, 2), 3);
        assert_eq!(line_start_char(s, 4), 7);
        assert_eq!(line_of_char(s, 0), 1);
        assert_eq!(line_of_char(s, 4), 2);
        assert_eq!(line_of_char(s, 7), 4);
        assert_eq!(char_to_byte("é_x", 1), 2);
    }

    #[test]
    fn indentation_detection() {
        let s = "int main(void) {\n    if (x) {";
        let (ind, opens) = indent_of_line_before(s, s.len());
        assert_eq!(ind, "    ");
        assert!(opens);
        let (ind, opens) = indent_of_line_before(s, 16);
        assert_eq!(ind, "");
        assert!(opens);
    }

    #[test]
    fn comment_toggle_roundtrip() {
        let s = "int a;\n    int b;\n\nint c;";
        let c = toggle_comment(s, 1, 2);
        assert_eq!(c, "// int a;\n    // int b;\n\nint c;");
        let u = toggle_comment(&c, 1, 2);
        assert_eq!(u, s);
        // Blank lines are left alone.
        assert_eq!(toggle_comment("\n", 1, 1), "\n");
    }

    #[test]
    fn line_at_maps_y() {
        let ys = [0.0, 10.0, 20.0];
        assert_eq!(line_at(&ys, 10.0, 0.0, 5.0), 1);
        assert_eq!(line_at(&ys, 10.0, 0.0, 15.0), 2);
        assert_eq!(line_at(&ys, 10.0, 0.0, 25.0), 3);
        assert_eq!(line_at(&ys, 10.0, 0.0, 45.0), 4);
    }

    #[test]
    fn document_dirty_and_reload() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.c");
        std::fs::write(&p, "x").unwrap();
        let mut d = Document::open(&p).unwrap();
        assert!(!d.is_dirty());
        d.text.push('y');
        assert!(d.is_dirty());
        d.save().unwrap();
        assert!(!d.is_dirty());
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "xy");
        // External change is picked up when there are no local edits.
        std::thread::sleep(std::time::Duration::from_millis(1100));
        std::fs::write(&p, "external").unwrap();
        assert!(d.reload_if_changed());
        assert_eq!(d.text, "external");
    }
}
