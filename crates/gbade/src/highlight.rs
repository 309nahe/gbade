//! A small C syntax highlighter for the code editor.
//!
//! It is a hand-written tokenizer (no external grammar): good enough for C,
//! fast enough to re-run on every keystroke for files of a few thousand lines.

use eframe::egui::{self, Color32, FontId, TextFormat, text::LayoutJob};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Comment,
    Keyword,
    Type,
    String,
    Char,
    Number,
    Preprocessor,
    Function,
    Macro,
    Punct,
    Ident,
    Whitespace,
}

const KEYWORDS: &[&str] = &[
    "auto", "break", "case", "const", "continue", "default", "do", "else", "enum", "extern", "for", "goto", "if",
    "inline", "register", "restrict", "return", "sizeof", "static", "struct", "switch", "typedef", "union",
    "volatile", "while", "_Alignas", "_Alignof", "_Atomic", "_Bool", "_Static_assert", "_Noreturn", "__attribute__",
    "__asm__", "asm", "true", "false", "NULL",
];

const TYPES: &[&str] = &[
    "void", "char", "short", "int", "long", "float", "double", "signed", "unsigned", "bool", "u8", "u16", "u32",
    "u64", "s8", "s16", "s32", "s64", "vu8", "vu16", "vu32", "vs16", "vs32", "size_t", "uint8_t", "uint16_t",
    "uint32_t", "uint64_t", "int8_t", "int16_t", "int32_t", "int64_t", "uintptr_t", "intptr_t", "fixed", "OBJ_ATTR",
    "TILE", "TILE8", "SCREENBLOCK", "CHARBLOCK", "SpriteSize", "IrqIndex", "LogLevel",
];

/// Split C source into (kind, byte range) tokens.
pub fn tokenize(src: &str) -> Vec<(TokenKind, std::ops::Range<usize>)> {
    let b = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    let mut line_start = true;
    while i < b.len() {
        let start = i;
        let c = b[i];
        let kind;
        if c == b'\n' {
            i += 1;
            line_start = true;
            out.push((TokenKind::Whitespace, start..i));
            continue;
        } else if c.is_ascii_whitespace() {
            while i < b.len() && b[i].is_ascii_whitespace() && b[i] != b'\n' {
                i += 1;
            }
            out.push((TokenKind::Whitespace, start..i));
            continue;
        } else if c == b'/' && b.get(i + 1) == Some(&b'/') {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            kind = TokenKind::Comment;
        } else if c == b'/' && b.get(i + 1) == Some(&b'*') {
            i += 2;
            while i < b.len() && !(b[i] == b'*' && b.get(i + 1) == Some(&b'/')) {
                i += 1;
            }
            i = (i + 2).min(b.len());
            kind = TokenKind::Comment;
        } else if c == b'#' && line_start {
            // Preprocessor directive: up to end of line (handles \ continuations),
            // but stop before a // comment.
            while i < b.len() && b[i] != b'\n' {
                if b[i] == b'\\' && b.get(i + 1) == Some(&b'\n') {
                    i += 2;
                    continue;
                }
                if b[i] == b'/' && matches!(b.get(i + 1), Some(b'/') | Some(b'*')) {
                    break;
                }
                i += 1;
            }
            kind = TokenKind::Preprocessor;
        } else if c == b'"' || c == b'\'' {
            i += 1;
            while i < b.len() && b[i] != c && b[i] != b'\n' {
                if b[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            i = (i + 1).min(b.len());
            kind = if c == b'"' { TokenKind::String } else { TokenKind::Char };
        } else if c.is_ascii_digit() || (c == b'.' && b.get(i + 1).is_some_and(u8::is_ascii_digit)) {
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'.' || b[i] == b'_') {
                i += 1;
            }
            kind = TokenKind::Number;
        } else if c.is_ascii_alphabetic() || c == b'_' {
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            let word = &src[start..i];
            // Look ahead for '(' to colour function calls/definitions.
            let mut j = i;
            while j < b.len() && (b[j] == b' ' || b[j] == b'\t') {
                j += 1;
            }
            kind = if KEYWORDS.contains(&word) {
                TokenKind::Keyword
            } else if TYPES.contains(&word) {
                TokenKind::Type
            } else if b.get(j) == Some(&b'(') {
                TokenKind::Function
            } else if word.len() > 1 && word.bytes().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_') {
                TokenKind::Macro
            } else {
                TokenKind::Ident
            };
        } else {
            i += src[i..].chars().next().map(char::len_utf8).unwrap_or(1);
            kind = TokenKind::Punct;
        }
        line_start = false;
        out.push((kind, start..i));
    }
    out
}

/// Editor colour theme.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub dark: bool,
}

impl Theme {
    pub fn color(&self, k: TokenKind) -> Color32 {
        if self.dark {
            match k {
                TokenKind::Comment => Color32::from_rgb(106, 153, 85),
                TokenKind::Keyword => Color32::from_rgb(197, 134, 192),
                TokenKind::Type => Color32::from_rgb(78, 201, 176),
                TokenKind::String | TokenKind::Char => Color32::from_rgb(206, 145, 120),
                TokenKind::Number => Color32::from_rgb(181, 206, 168),
                TokenKind::Preprocessor => Color32::from_rgb(155, 155, 220),
                TokenKind::Function => Color32::from_rgb(220, 220, 170),
                TokenKind::Macro => Color32::from_rgb(86, 156, 214),
                TokenKind::Punct => Color32::from_rgb(190, 190, 190),
                TokenKind::Ident | TokenKind::Whitespace => Color32::from_rgb(212, 212, 212),
            }
        } else {
            match k {
                TokenKind::Comment => Color32::from_rgb(0, 128, 0),
                TokenKind::Keyword => Color32::from_rgb(175, 0, 219),
                TokenKind::Type => Color32::from_rgb(38, 127, 153),
                TokenKind::String | TokenKind::Char => Color32::from_rgb(163, 21, 21),
                TokenKind::Number => Color32::from_rgb(9, 134, 88),
                TokenKind::Preprocessor => Color32::from_rgb(100, 60, 180),
                TokenKind::Function => Color32::from_rgb(121, 94, 38),
                TokenKind::Macro => Color32::from_rgb(0, 80, 200),
                TokenKind::Punct => Color32::from_rgb(60, 60, 60),
                TokenKind::Ident | TokenKind::Whitespace => Color32::from_rgb(20, 20, 20),
            }
        }
    }
}

/// Build a coloured layout for the editor.
pub fn layout_job(src: &str, theme: Theme, font: FontId, highlight: bool) -> LayoutJob {
    let mut job = LayoutJob::default();
    if !highlight {
        job.append(src, 0.0, TextFormat::simple(font, theme.color(TokenKind::Ident)));
        return job;
    }
    for (kind, range) in tokenize(src) {
        let mut fmt = TextFormat::simple(font.clone(), theme.color(kind));
        if kind == TokenKind::Comment {
            fmt.italics = true;
        }
        job.append(&src[range], 0.0, fmt);
    }
    job
}

/// Highlighting for a plain-text file (no colours).
pub fn is_c_like(path: &std::path::Path) -> bool {
    matches!(path.extension().and_then(|e| e.to_str()), Some("c" | "h" | "s" | "S" | "inc"))
}

#[allow(dead_code)]
pub fn monospace(size: f32) -> FontId {
    egui::FontId::monospace(size)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<(TokenKind, String)> {
        tokenize(src)
            .into_iter()
            .filter(|(k, _)| *k != TokenKind::Whitespace)
            .map(|(k, r)| (k, src[r].to_string()))
            .collect()
    }

    #[test]
    fn tokens_cover_the_whole_input() {
        let src = "#include <gba.h>\nint main(void) { /* hi */ return 0x1F; } // end\n\"str\" 'c'";
        let toks = tokenize(src);
        let mut pos = 0;
        for (_, r) in &toks {
            assert_eq!(r.start, pos);
            pos = r.end;
        }
        assert_eq!(pos, src.len());
    }

    #[test]
    fn classifies_c() {
        let k = kinds("#define X 1\nu16 color = RGB15(31, 0, 0); // red\nif (x) return;");
        assert_eq!(k[0], (TokenKind::Preprocessor, "#define X 1".into()));
        assert!(k.contains(&(TokenKind::Type, "u16".into())));
        assert!(k.contains(&(TokenKind::Function, "RGB15".into())));
        assert!(k.contains(&(TokenKind::Number, "31".into())));
        assert!(k.contains(&(TokenKind::Comment, "// red".into())));
        assert!(k.contains(&(TokenKind::Keyword, "if".into())));
        assert!(k.contains(&(TokenKind::Keyword, "return".into())));
    }

    #[test]
    fn hash_not_at_line_start_is_not_preprocessor() {
        let k = kinds("x = a # b;");
        assert!(!k.iter().any(|(t, _)| *t == TokenKind::Preprocessor));
    }

    #[test]
    fn unterminated_things_do_not_panic() {
        for src in ["\"abc", "/* never ends", "'", "#define A \\", "0x", "é ü 日本"] {
            let _ = tokenize(src);
        }
    }

    #[test]
    fn macros_are_uppercase_identifiers() {
        let k = kinds("REG_DISPCNT = DCNT_MODE3 | x;");
        assert_eq!(k[0], (TokenKind::Macro, "REG_DISPCNT".into()));
        assert!(k.contains(&(TokenKind::Ident, "x".into())));
    }
}
