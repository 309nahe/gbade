//! The "Learn" panel: renders the Markdown guides shipped in `docs/`.
//!
//! This is a deliberately small Markdown renderer: headings, paragraphs,
//! bullet/numbered lists, code blocks, inline `code`, **bold**, *italic*,
//! links and tables (shown as monospace text). That is all our docs use.

use eframe::egui::{self, Color32, RichText, Ui};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Heading(u8, String),
    Paragraph(String),
    Bullet(u8, String),
    Numbered(String, String),
    Code(String),
    Table(Vec<String>),
    Quote(String),
    Rule,
}

/// Parse Markdown into blocks.
pub fn parse(md: &str) -> Vec<Block> {
    let mut out = Vec::new();
    let mut para: Vec<String> = Vec::new();
    let mut lines = md.lines().peekable();
    let flush = |para: &mut Vec<String>, out: &mut Vec<Block>| {
        if !para.is_empty() {
            out.push(Block::Paragraph(para.join(" ")));
            para.clear();
        }
    };
    while let Some(line) = lines.next() {
        let t = line.trim_end();
        let trimmed = t.trim_start();
        if trimmed.starts_with("```") {
            flush(&mut para, &mut out);
            let mut code = Vec::new();
            for l in lines.by_ref() {
                if l.trim_start().starts_with("```") {
                    break;
                }
                code.push(l.to_string());
            }
            out.push(Block::Code(code.join("\n")));
        } else if trimmed.is_empty() {
            flush(&mut para, &mut out);
        } else if trimmed.starts_with("<!--") {
            // HTML comments (used for metadata) are skipped.
            flush(&mut para, &mut out);
            if !trimmed.contains("-->") {
                for l in lines.by_ref() {
                    if l.contains("-->") {
                        break;
                    }
                }
            }
        } else if let Some(h) = heading(trimmed) {
            flush(&mut para, &mut out);
            out.push(h);
        } else if trimmed == "---" || trimmed == "***" {
            flush(&mut para, &mut out);
            out.push(Block::Rule);
        } else if trimmed.starts_with('|') {
            flush(&mut para, &mut out);
            let mut rows = vec![trimmed.to_string()];
            while let Some(n) = lines.peek() {
                if n.trim_start().starts_with('|') {
                    rows.push(n.trim().to_string());
                    lines.next();
                } else {
                    break;
                }
            }
            out.push(Block::Table(rows));
        } else if let Some(rest) = trimmed.strip_prefix("> ") {
            flush(&mut para, &mut out);
            out.push(Block::Quote(rest.to_string()));
        } else if let Some(rest) = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* ")) {
            flush(&mut para, &mut out);
            let indent = ((t.len() - trimmed.len()) / 2) as u8;
            out.push(Block::Bullet(indent, rest.to_string()));
        } else if let Some((num, rest)) = numbered(trimmed) {
            flush(&mut para, &mut out);
            out.push(Block::Numbered(num, rest));
        } else {
            para.push(trimmed.to_string());
        }
    }
    flush(&mut para, &mut out);
    out
}

fn heading(t: &str) -> Option<Block> {
    let level = t.chars().take_while(|c| *c == '#').count();
    if (1..=6).contains(&level) && t[level..].starts_with(' ') {
        Some(Block::Heading(level as u8, t[level + 1..].trim().to_string()))
    } else {
        None
    }
}

fn numbered(t: &str) -> Option<(String, String)> {
    let digits: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    let rest = &t[digits.len()..];
    rest.strip_prefix(". ").map(|r| (digits, r.to_string()))
}

/// Inline spans: (text, kind).
#[derive(Debug, Clone, PartialEq)]
pub enum Span {
    Text(String),
    Code(String),
    Bold(String),
    Italic(String),
    Link(String, String),
}

pub fn inline(s: &str) -> Vec<Span> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    let push_text = |cur: &mut String, out: &mut Vec<Span>| {
        if !cur.is_empty() {
            out.push(Span::Text(std::mem::take(cur)));
        }
    };
    while i < chars.len() {
        let c = chars[i];
        if c == '`' {
            if let Some(end) = chars[i + 1..].iter().position(|&x| x == '`') {
                push_text(&mut cur, &mut out);
                out.push(Span::Code(chars[i + 1..i + 1 + end].iter().collect()));
                i += end + 2;
                continue;
            }
        } else if c == '*' && chars.get(i + 1) == Some(&'*') {
            let rest: String = chars[i + 2..].iter().collect();
            if let Some(end) = rest.find("**") {
                push_text(&mut cur, &mut out);
                out.push(Span::Bold(rest[..end].to_string()));
                i += 2 + rest[..end].chars().count() + 2;
                continue;
            }
        } else if c == '*' || c == '_' && (i == 0 || chars[i - 1] == ' ') {
            if let Some(end) = chars[i + 1..].iter().position(|&x| x == c) {
                if end > 0 {
                    push_text(&mut cur, &mut out);
                    out.push(Span::Italic(chars[i + 1..i + 1 + end].iter().collect()));
                    i += end + 2;
                    continue;
                }
            }
        } else if c == '[' {
            let rest: String = chars[i..].iter().collect();
            if let (Some(close), Some(open_p)) = (rest.find("]("), rest.find("](")) {
                if let Some(close_p) = rest[open_p..].find(')') {
                    push_text(&mut cur, &mut out);
                    let text = rest[1..close].to_string();
                    let url = rest[open_p + 2..open_p + close_p].to_string();
                    let consumed = rest[..open_p + close_p + 1].chars().count();
                    out.push(Span::Link(text, url));
                    i += consumed;
                    continue;
                }
            }
        }
        cur.push(c);
        i += 1;
    }
    push_text(&mut cur, &mut out);
    out
}

/// What the user clicked in the rendered document.
pub enum DocAction {
    OpenDoc(PathBuf),
    OpenUrl(String),
}

fn render_inline(ui: &mut Ui, text: &str, size: f32, actions: &mut Vec<DocAction>, base: &Path) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for span in inline(text) {
            match span {
                Span::Text(t) => {
                    ui.label(RichText::new(t).size(size));
                }
                Span::Code(t) => {
                    ui.label(RichText::new(t).code().size(size));
                }
                Span::Bold(t) => {
                    ui.label(RichText::new(t).strong().size(size));
                }
                Span::Italic(t) => {
                    ui.label(RichText::new(t).italics().size(size));
                }
                Span::Link(t, url) => {
                    if ui.link(RichText::new(t).size(size)).on_hover_text(&url).clicked() {
                        if url.starts_with("http") {
                            actions.push(DocAction::OpenUrl(url));
                        } else {
                            let path = url.split('#').next().unwrap_or("");
                            actions.push(DocAction::OpenDoc(base.join(path)));
                        }
                    }
                }
            }
        }
    });
}

/// Render parsed blocks. `base` is the folder of the document (for links).
pub fn render(ui: &mut Ui, blocks: &[Block], base: &Path) -> Vec<DocAction> {
    let mut actions = Vec::new();
    let body = 14.5;
    for b in blocks {
        match b {
            Block::Heading(level, t) => {
                ui.add_space(if *level <= 2 { 10.0 } else { 6.0 });
                let size = match level {
                    1 => 26.0,
                    2 => 21.0,
                    3 => 17.0,
                    _ => 15.0,
                };
                ui.label(RichText::new(t.replace('`', "")).size(size).strong());
                if *level <= 2 {
                    ui.separator();
                }
            }
            Block::Paragraph(t) => {
                render_inline(ui, t, body, &mut actions, base);
                ui.add_space(4.0);
            }
            Block::Bullet(indent, t) => {
                ui.horizontal_top(|ui| {
                    ui.add_space(8.0 + *indent as f32 * 16.0);
                    ui.label(RichText::new("•").size(body));
                    ui.vertical(|ui| render_inline(ui, t, body, &mut actions, base));
                });
            }
            Block::Numbered(n, t) => {
                ui.horizontal_top(|ui| {
                    ui.add_space(8.0);
                    ui.label(RichText::new(format!("{n}.")).size(body));
                    ui.vertical(|ui| render_inline(ui, t, body, &mut actions, base));
                });
            }
            Block::Code(code) => {
                egui::Frame::new()
                    .fill(if ui.visuals().dark_mode { Color32::from_gray(25) } else { Color32::from_gray(235) })
                    .inner_margin(8.0)
                    .corner_radius(4.0)
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.label(RichText::new(code).monospace().size(13.0));
                    });
                ui.add_space(4.0);
            }
            Block::Table(rows) => {
                let cells: Vec<Vec<String>> = rows
                    .iter()
                    .filter(|r| !r.chars().all(|c| matches!(c, '|' | '-' | ':' | ' ')))
                    .map(|r| r.trim_matches('|').split('|').map(|c| c.trim().to_string()).collect())
                    .collect();
                let cols = cells.iter().map(Vec::len).max().unwrap_or(0);
                egui::Grid::new(ui.next_auto_id()).striped(true).num_columns(cols).show(ui, |ui| {
                    for (ri, row) in cells.iter().enumerate() {
                        for c in row {
                            if ri == 0 {
                                ui.label(RichText::new(c.replace('`', "")).strong());
                            } else {
                                render_inline(ui, c, 13.5, &mut actions, base);
                            }
                        }
                        ui.end_row();
                    }
                });
                ui.add_space(4.0);
            }
            Block::Quote(t) => {
                egui::Frame::new()
                    .fill(Color32::from_rgba_unmultiplied(80, 140, 255, 30))
                    .inner_margin(8.0)
                    .corner_radius(4.0)
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        render_inline(ui, t, body, &mut actions, base);
                    });
            }
            Block::Rule => {
                ui.separator();
            }
        }
    }
    actions
}

/// The docs panel state.
pub struct DocsViewer {
    pub current: Option<PathBuf>,
    blocks: Vec<Block>,
    history: Vec<PathBuf>,
}

impl Default for DocsViewer {
    fn default() -> Self {
        DocsViewer { current: None, blocks: Vec::new(), history: Vec::new() }
    }
}

impl DocsViewer {
    pub fn open(&mut self, path: &Path) {
        if let Ok(text) = std::fs::read_to_string(path) {
            if let Some(c) = self.current.take() {
                self.history.push(c);
            }
            self.blocks = parse(&text);
            self.current = Some(path.to_path_buf());
        }
    }

    /// All guides, in reading order (sorted by file name: 01-..., 02-...).
    pub fn index(docs_dir: &Path) -> Vec<(String, PathBuf)> {
        let mut v = Vec::new();
        for sub in ["guide", "."] {
            let dir = docs_dir.join(sub);
            let Ok(rd) = std::fs::read_dir(&dir) else { continue };
            let mut files: Vec<PathBuf> =
                rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "md")).collect();
            files.sort();
            for f in files {
                let title = std::fs::read_to_string(&f)
                    .ok()
                    .and_then(|t| t.lines().find(|l| l.starts_with("# ")).map(|l| l[2..].trim().to_string()))
                    .unwrap_or_else(|| f.file_stem().unwrap().to_string_lossy().into_owned());
                v.push((title, f));
            }
        }
        v
    }

    pub fn ui(&mut self, ui: &mut Ui, docs_dir: Option<&Path>) {
        let Some(docs_dir) = docs_dir else {
            ui.label("Documentation not found.");
            return;
        };
        if self.current.is_none() {
            let first = Self::index(docs_dir).into_iter().next();
            if let Some((_, p)) = first {
                self.open(&p);
            }
        }
        egui::SidePanel::left("docs-index").resizable(true).default_width(200.0).show_inside(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                let mut last_dir: Option<PathBuf> = None;
                for (title, path) in Self::index(docs_dir) {
                    let dir = path.parent().map(Path::to_path_buf);
                    if dir != last_dir {
                        let label = if dir.as_ref().is_some_and(|d| d.ends_with("guide")) { "Guide" } else { "Reference" };
                        ui.label(RichText::new(label).strong());
                        last_dir = dir;
                    }
                    let selected = self.current.as_ref() == Some(&path);
                    if ui.selectable_label(selected, title).clicked() {
                        self.open(&path);
                    }
                }
            });
        });
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.add_enabled(!self.history.is_empty(), egui::Button::new("← Back")).clicked() {
                    if let Some(prev) = self.history.pop() {
                        if let Ok(text) = std::fs::read_to_string(&prev) {
                            self.blocks = parse(&text);
                            self.current = Some(prev);
                        }
                    }
                }
            });
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.set_max_width(760.0);
                let base = self.current.as_ref().and_then(|p| p.parent()).map(Path::to_path_buf).unwrap_or_default();
                let actions = render(ui, &self.blocks, &base);
                for a in actions {
                    match a {
                        DocAction::OpenDoc(p) => self.open(&p),
                        DocAction::OpenUrl(u) => ui.ctx().open_url(egui::OpenUrl::new_tab(u)),
                    }
                }
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_blocks() {
        let md = "# Title\n\nSome *text* and `code`.\nSecond line.\n\n- a\n  - b\n1. one\n\n```c\nint x;\n```\n| a | b |\n|---|---|\n| 1 | 2 |\n> note\n---\n";
        let b = parse(md);
        assert_eq!(b[0], Block::Heading(1, "Title".into()));
        assert_eq!(b[1], Block::Paragraph("Some *text* and `code`. Second line.".into()));
        assert_eq!(b[2], Block::Bullet(0, "a".into()));
        assert_eq!(b[3], Block::Bullet(1, "b".into()));
        assert_eq!(b[4], Block::Numbered("1".into(), "one".into()));
        assert_eq!(b[5], Block::Code("int x;".into()));
        assert!(matches!(&b[6], Block::Table(r) if r.len() == 3));
        assert_eq!(b[7], Block::Quote("note".into()));
        assert_eq!(b[8], Block::Rule);
    }

    #[test]
    fn parses_inline() {
        let s = inline("Use `vsync()` **every** frame, see [Tonc](https://gbadev.net/tonc/) *now*.");
        assert_eq!(
            s,
            vec![
                Span::Text("Use ".into()),
                Span::Code("vsync()".into()),
                Span::Text(" ".into()),
                Span::Bold("every".into()),
                Span::Text(" frame, see ".into()),
                Span::Link("Tonc".into(), "https://gbadev.net/tonc/".into()),
                Span::Text(" ".into()),
                Span::Italic("now".into()),
                Span::Text(".".into()),
            ]
        );
    }

    #[test]
    fn snake_case_is_not_italic() {
        let s = inline("call key_poll_now please");
        assert_eq!(s, vec![Span::Text("call key_poll_now please".into())]);
    }

    #[test]
    fn html_comments_are_skipped() {
        let b = parse("<!-- order: 2 -->\n# T\n<!--\nmulti\n-->\ntext");
        assert_eq!(b, vec![Block::Heading(1, "T".into()), Block::Paragraph("text".into())]);
    }
}
