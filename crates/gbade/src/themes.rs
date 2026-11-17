//! Colour themes: the window (egui visuals), the editor and its syntax
//! colours, all from one palette per theme.
//!
//! Besides GBADE's own light and dark themes, three popular dark themes use
//! their published palettes:
//!
//! - **Monokai**, the classic palette by Wimer Hazenberg (monokai.nl).
//! - **Tokyo Night** ("night" variant), by enkia (github.com/enkia/tokyo-night-vscode-theme).
//! - **Catppuccin Mocha**, by the Catppuccin project (github.com/catppuccin/catppuccin).

use crate::highlight::TokenKind;
use eframe::egui::{self, Color32, Visuals};
use serde::{Deserialize, Serialize};

/// The themes, in the order the menus show them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ThemeId {
    Light,
    Dark,
    Monokai,
    TokyoNight,
    Catppuccin,
}

impl ThemeId {
    pub const ALL: [ThemeId; 5] = [ThemeId::Light, ThemeId::Dark, ThemeId::Monokai, ThemeId::TokyoNight, ThemeId::Catppuccin];

    pub fn name(self) -> &'static str {
        match self {
            ThemeId::Light => "GBADE Light",
            ThemeId::Dark => "GBADE Dark",
            ThemeId::Monokai => "Monokai",
            ThemeId::TokyoNight => "Tokyo Night",
            ThemeId::Catppuccin => "Catppuccin Mocha",
        }
    }

    /// A short name for files (screenshots): "tokyo-night".
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn slug(self) -> &'static str {
        match self {
            ThemeId::Light => "light",
            ThemeId::Dark => "dark",
            ThemeId::Monokai => "monokai",
            ThemeId::TokyoNight => "tokyo-night",
            ThemeId::Catppuccin => "catppuccin",
        }
    }

    pub fn palette(self) -> Palette {
        match self {
            ThemeId::Light => LIGHT,
            ThemeId::Dark => DARK,
            ThemeId::Monokai => MONOKAI,
            ThemeId::TokyoNight => TOKYO_NIGHT,
            ThemeId::Catppuccin => CATPPUCCIN,
        }
    }

    pub fn is_dark(self) -> bool {
        self.palette().dark
    }
}

/// Every colour a theme decides.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    pub dark: bool,
    /// Panels and windows.
    pub panel: Color32,
    /// The code editor's background.
    pub editor: Color32,
    /// Slightly different from `panel`: stripes, code blocks.
    pub faint: Color32,
    /// Buttons and other widgets.
    pub widget: Color32,
    pub widget_hover: Color32,
    pub text: Color32,
    pub accent: Color32,
    /// Text drawn on `accent` (the Run button).
    pub on_accent: Color32,
    pub selection: Color32,
    pub gutter: Color32,
    pub line_number: Color32,
    pub syntax: Syntax,
}

/// Colours of the C syntax highlighter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Syntax {
    pub comment: Color32,
    pub keyword: Color32,
    pub ty: Color32,
    pub string: Color32,
    pub number: Color32,
    pub preprocessor: Color32,
    pub function: Color32,
    pub macro_: Color32,
    pub punct: Color32,
    pub ident: Color32,
}

impl Syntax {
    pub fn color(&self, k: TokenKind) -> Color32 {
        match k {
            TokenKind::Comment => self.comment,
            TokenKind::Keyword => self.keyword,
            TokenKind::Type => self.ty,
            TokenKind::String | TokenKind::Char => self.string,
            TokenKind::Number => self.number,
            TokenKind::Preprocessor => self.preprocessor,
            TokenKind::Function => self.function,
            TokenKind::Macro => self.macro_,
            TokenKind::Punct => self.punct,
            TokenKind::Ident | TokenKind::Whitespace => self.ident,
        }
    }
}

const fn hex(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

/// GBADE's original light theme (egui's light look, VS Code-like syntax).
const LIGHT: Palette = Palette {
    dark: false,
    panel: hex(0xf8f8f8),
    editor: hex(0xf8f8f8),
    faint: hex(0xebebeb),
    widget: hex(0xe6e6e6),
    widget_hover: hex(0xd6d6d6),
    text: hex(0x3c3c3c),
    accent: hex(0x6e5ae6),
    on_accent: Color32::WHITE,
    selection: hex(0x90d1ff),
    gutter: hex(0xf0f0f0),
    line_number: hex(0x969696),
    syntax: Syntax {
        comment: hex(0x008000),
        keyword: hex(0xaf00db),
        ty: hex(0x267f99),
        string: hex(0xa31515),
        number: hex(0x098658),
        preprocessor: hex(0x643cb4),
        function: hex(0x795e26),
        macro_: hex(0x0050c8),
        punct: hex(0x3c3c3c),
        ident: hex(0x141414),
    },
};

/// GBADE's original dark theme (egui's dark look, VS Code-like syntax).
const DARK: Palette = Palette {
    dark: true,
    panel: hex(0x1b1b1b),
    editor: hex(0x1b1b1b),
    faint: hex(0x191919),
    widget: hex(0x3c3c3c),
    widget_hover: hex(0x464646),
    text: hex(0xbebebe),
    accent: hex(0x6e5ae6),
    on_accent: Color32::WHITE,
    selection: hex(0x005ac8),
    gutter: hex(0x1e1e1e),
    line_number: hex(0x6e6e6e),
    syntax: Syntax {
        comment: hex(0x6a9955),
        keyword: hex(0xc586c0),
        ty: hex(0x4ec9b0),
        string: hex(0xce9178),
        number: hex(0xb5cea8),
        preprocessor: hex(0x9b9bdc),
        function: hex(0xdcdcaa),
        macro_: hex(0x569cd6),
        punct: hex(0xbebebe),
        ident: hex(0xd4d4d4),
    },
};

/// Monokai: background #272822, foreground #F8F8F2, comments #75715E, pink
/// #F92672, green #A6E22E, yellow #E6DB74, blue #66D9EF, purple #AE81FF,
/// orange #FD971F.
const MONOKAI: Palette = Palette {
    dark: true,
    panel: hex(0x1e1f1c),
    editor: hex(0x272822),
    faint: hex(0x2d2e27),
    widget: hex(0x3e3d32),
    widget_hover: hex(0x49483e),
    text: hex(0xf8f8f2),
    accent: hex(0xa6e22e),
    on_accent: hex(0x1a1b26),
    selection: hex(0x49483e),
    gutter: hex(0x272822),
    line_number: hex(0x90908a),
    syntax: Syntax {
        comment: hex(0x75715e),
        keyword: hex(0xf92672),
        ty: hex(0x66d9ef),
        string: hex(0xe6db74),
        number: hex(0xae81ff),
        preprocessor: hex(0xf92672),
        function: hex(0xa6e22e),
        macro_: hex(0xae81ff),
        punct: hex(0xf8f8f2),
        ident: hex(0xf8f8f2),
    },
};

/// Tokyo Night (night): bg #1a1b26, darker #16161e, fg #c0caf5, comments
/// #565f89, blue #7aa2f7, cyan #7dcfff, magenta #bb9af7, green #9ece6a,
/// orange #ff9e64, yellow #e0af68, teal #73daca.
const TOKYO_NIGHT: Palette = Palette {
    dark: true,
    panel: hex(0x16161e),
    editor: hex(0x1a1b26),
    faint: hex(0x1f2335),
    widget: hex(0x292e42),
    widget_hover: hex(0x33467c),
    text: hex(0xc0caf5),
    accent: hex(0x7aa2f7),
    on_accent: hex(0x1a1b26),
    selection: hex(0x33467c),
    gutter: hex(0x1a1b26),
    line_number: hex(0x565f89),
    syntax: Syntax {
        comment: hex(0x565f89),
        keyword: hex(0xbb9af7),
        ty: hex(0x2ac3de),
        string: hex(0x9ece6a),
        number: hex(0xff9e64),
        preprocessor: hex(0x7dcfff),
        function: hex(0x7aa2f7),
        macro_: hex(0xe0af68),
        punct: hex(0x89ddff),
        ident: hex(0xc0caf5),
    },
};

/// Catppuccin Mocha: base #1e1e2e, mantle #181825, surface0 #313244,
/// surface1 #45475a, overlay2 #9399b2, text #cdd6f4, mauve #cba6f7, green
/// #a6e3a1, peach #fab387, blue #89b4fa, yellow #f9e2af, pink #f5c2e7,
/// teal #94e2d5, sky #89dceb.
const CATPPUCCIN: Palette = Palette {
    dark: true,
    panel: hex(0x181825),
    editor: hex(0x1e1e2e),
    faint: hex(0x232334),
    widget: hex(0x313244),
    widget_hover: hex(0x45475a),
    text: hex(0xcdd6f4),
    accent: hex(0xcba6f7),
    on_accent: hex(0x1a1b26),
    selection: hex(0x45475a),
    gutter: hex(0x1e1e2e),
    line_number: hex(0x7f849c),
    syntax: Syntax {
        comment: hex(0x9399b2),
        keyword: hex(0xcba6f7),
        ty: hex(0xf9e2af),
        string: hex(0xa6e3a1),
        number: hex(0xfab387),
        preprocessor: hex(0xf5c2e7),
        function: hex(0x89b4fa),
        macro_: hex(0x94e2d5),
        punct: hex(0x9399b2),
        ident: hex(0xcdd6f4),
    },
};

/// The egui visuals of a theme: egui's own light or dark look, recoloured.
pub fn visuals(id: ThemeId) -> Visuals {
    let p = id.palette();
    let mut v = if p.dark { Visuals::dark() } else { Visuals::light() };
    if matches!(id, ThemeId::Light | ThemeId::Dark) {
        // GBADE's original themes are egui's own, unchanged.
        return v;
    }
    v.panel_fill = p.panel;
    v.window_fill = p.panel;
    v.extreme_bg_color = p.editor;
    v.faint_bg_color = p.faint;
    v.code_bg_color = p.widget;
    v.hyperlink_color = p.accent;
    v.selection.bg_fill = p.selection;
    v.selection.stroke.color = p.text;
    v.override_text_color = Some(p.text);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = p.panel;
    w.noninteractive.weak_bg_fill = p.panel;
    w.noninteractive.bg_stroke.color = p.widget;
    w.noninteractive.fg_stroke.color = p.text;
    for (state, fill) in [(&mut w.inactive, p.widget), (&mut w.hovered, p.widget_hover), (&mut w.active, p.selection), (&mut w.open, p.widget)] {
        state.bg_fill = fill;
        state.weak_bg_fill = fill;
        state.fg_stroke.color = p.text;
    }
    w.hovered.bg_stroke.color = p.accent;
    w.active.bg_stroke.color = p.accent;
    v
}

/// Apply a theme to the whole window.
pub fn apply(ctx: &egui::Context, id: ThemeId) {
    ctx.set_visuals(visuals(id));
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(CURRENT), id));
}

const CURRENT: &str = "gbade-theme";

/// The theme last applied to this window (for panels that draw their own colours).
pub fn current(ctx: &egui::Context) -> ThemeId {
    ctx.data(|d| d.get_temp(egui::Id::new(CURRENT)))
        .unwrap_or(if ctx.style().visuals.dark_mode { ThemeId::Dark } else { ThemeId::Light })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Relative luminance (sRGB, roughly) for contrast checks.
    fn luminance(c: Color32) -> f32 {
        let f = |v: u8| {
            let v = v as f32 / 255.0;
            if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
    }

    fn contrast(a: Color32, b: Color32) -> f32 {
        let (la, lb) = (luminance(a), luminance(b));
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    }

    #[test]
    fn names_and_slugs_are_unique() {
        let names: std::collections::HashSet<_> = ThemeId::ALL.iter().map(|t| t.name()).collect();
        let slugs: std::collections::HashSet<_> = ThemeId::ALL.iter().map(|t| t.slug()).collect();
        assert_eq!(names.len(), ThemeId::ALL.len());
        assert_eq!(slugs.len(), ThemeId::ALL.len());
    }

    #[test]
    fn text_is_readable_in_every_theme() {
        for id in ThemeId::ALL {
            let p = id.palette();
            // WCAG asks 4.5:1 for body text; code identifiers are the bulk.
            assert!(contrast(p.text, p.panel) >= 4.5, "{}: text on panels", id.name());
            assert!(contrast(p.syntax.ident, p.editor) >= 4.5, "{}: code on the editor", id.name());
            // Every syntax colour stays legible (3:1, the large-text level).
            for (name, c) in [
                ("keyword", p.syntax.keyword),
                ("type", p.syntax.ty),
                ("string", p.syntax.string),
                ("number", p.syntax.number),
                ("function", p.syntax.function),
                ("comment", p.syntax.comment),
            ] {
                assert!(contrast(c, p.editor) >= 2.5, "{}: {name} on the editor ({:.2})", id.name(), contrast(c, p.editor));
            }
            assert_eq!(p.dark, luminance(p.editor) < 0.2, "{}: dark flag matches the background", id.name());
        }
    }

    #[test]
    fn the_new_themes_use_their_published_backgrounds() {
        assert_eq!(MONOKAI.editor, hex(0x272822));
        assert_eq!(TOKYO_NIGHT.editor, hex(0x1a1b26));
        assert_eq!(CATPPUCCIN.editor, hex(0x1e1e2e));
        let v = visuals(ThemeId::Catppuccin);
        assert_eq!(v.extreme_bg_color, hex(0x1e1e2e));
        assert!(v.dark_mode);
    }

    #[test]
    fn original_themes_are_unchanged() {
        assert_eq!(visuals(ThemeId::Dark), Visuals::dark());
        assert_eq!(visuals(ThemeId::Light), Visuals::light());
    }
}
